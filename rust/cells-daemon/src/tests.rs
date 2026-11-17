//! The daemon against a simulated world: one home folder in memory, each file with a system
//! ID as a disk gives one (kept by a write in place and a rename, new for a file saved over
//! atomically or copied), change events handed in by the test, a clock that only moves when
//! asked. Every run is the same run.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;

use crate::daemon::{Daemon, Event};
use crate::index::{FileId, Heard, visible};
use crate::log::{Line, hash};
use crate::trace::caught;
use crate::wire::{FromEngine, ToEngine};
use crate::world::{Found, Replaced, World, table_file};

#[derive(Default)]
pub(crate) struct Sim {
    files: RefCell<BTreeMap<PathBuf, String>>,
    /// Each file's system ID.
    ids: RefCell<BTreeMap<PathBuf, u64>>,
    /// The last system ID given.
    last_id: Cell<u64>,
    /// A disk that gives no file IDs: what a move is followed by is the events' IDs, or text.
    pub(crate) blind: bool,
    /// Files the disk will not give, and why (iCloud's, kept only in the cloud).
    unreadable: RefCell<BTreeMap<PathBuf, String>>,
    watched: Vec<(PathBuf, bool)>,
    /// What each script returns when run, by name: its lines, or why it failed.
    scripts: RefCell<BTreeMap<String, Result<Vec<String>, String>>>,
    /// Every script started, by name, in order.
    started: RefCell<Vec<String>>,
    /// The clock, in milliseconds since the Unix epoch: `NOW` unless a test moves it.
    clock: Cell<u64>,
    /// The random bits handed out, counted: every run the same ids.
    drawn: Cell<u64>,
    /// What the daemon says back to itself (a run ended), read by `pump`.
    inbox: Option<std::sync::mpsc::Receiver<Event>>,
}

/// The simulated clock's start: 2026-11-06T21:20:00Z.
const NOW: u64 = 1_794_000_000_000;

/// The simulated home folder.
pub(crate) const HOME: &str = "/h";

impl Sim {
    /// A system ID never given before.
    fn fresh(&self) -> u64 {
        self.last_id.set(self.last_id.get() + 1);
        self.last_id.get()
    }

    /// `text` written at `path` in place, as an editor that writes into the file does: its
    /// system ID kept, a new one for a file new there.
    pub(crate) fn put(&self, path: &str, text: &str) {
        let p = PathBuf::from(path);
        if !self.ids.borrow().contains_key(&p) {
            let id = self.fresh();
            self.ids.borrow_mut().insert(p.clone(), id);
        }
        self.files.borrow_mut().insert(p, text.to_owned());
    }

    /// `text` saved over `path` atomically, as most editors save: a new file renamed over
    /// it, another system ID.
    fn save_over(&self, path: &str, text: &str) {
        let id = self.fresh();
        self.ids.borrow_mut().insert(PathBuf::from(path), id);
        self.files.borrow_mut().insert(PathBuf::from(path), text.to_owned());
    }

    pub(crate) fn get(&self, path: &str) -> String {
        self.files.borrow()[Path::new(path)].clone()
    }

    fn remove(&self, path: &str) -> String {
        self.ids.borrow_mut().remove(Path::new(path));
        self.files.borrow_mut().remove(Path::new(path)).expect("a file to remove")
    }

    fn id(&self, path: &str) -> u64 {
        self.ids.borrow()[Path::new(path)]
    }

    /// The log's lines as written.
    pub(crate) fn log(&self) -> Vec<String> {
        self.files.borrow().get(&crate::log::path()).map_or(Vec::new(), |t| t.lines().map(str::to_owned).collect())
    }

    /// The log's lines, read.
    fn lines(&self) -> Vec<Line> {
        self.log().iter().map(|l| serde_json::from_str(l).unwrap()).collect()
    }
}

impl World for Sim {
    fn read(&self, path: &Path) -> Found {
        if let Some(why) = self.unreadable.borrow().get(path) {
            return Found::Unreadable(why.clone());
        }
        self.files.borrow().get(path).map_or(Found::Missing, |t| Found::Text(t.clone()))
    }

    fn file_id(&self, path: &Path) -> Option<u64> {
        if self.blind {
            return None;
        }
        self.ids.borrow().get(path).copied()
    }

    fn files(&self, root: &Path) -> Vec<PathBuf> {
        let home = self.home();
        self.files.borrow().keys().filter(|p| p.starts_with(root) && table_file(p) && visible(&home, p)).cloned().collect()
    }

    fn folder_logs(&self) -> Vec<PathBuf> {
        let home = self.home();
        self.files
            .borrow()
            .keys()
            .filter(|p| p.ends_with(".cells/log.jsonl"))
            .filter(|p| p.starts_with(&home) && visible(&home, p.parent().and_then(Path::parent).expect("a folder's log")))
            .cloned()
            .collect()
    }

    fn entries(&self, dir: &Path) -> Vec<PathBuf> {
        self.files.borrow().keys().filter(|p| p.parent() == Some(dir)).cloned().collect()
    }

    fn replace(&self, path: &Path, against: &str, text: &str) -> Replaced {
        let now = self.files.borrow().get(path).cloned();
        match now {
            // the engine writes beside and renames over: another system ID
            Some(now) if hash(&now) == against => {
                self.save_over(path.to_str().unwrap(), text);
                Replaced::Done
            }
            _ => Replaced::Moved,
        }
    }

    fn save(&self, path: &Path, text: &str) -> Result<(), String> {
        self.save_over(path.to_str().unwrap(), text);
        Ok(())
    }

    fn append(&self, path: &Path, line: &str) -> Result<(), String> {
        self.files.borrow_mut().entry(path.to_path_buf()).or_default().push_str(&format!("{line}\n"));
        Ok(())
    }

    fn keep(&self, path: &Path, text: &str) -> Result<(), String> {
        self.files.borrow_mut().entry(path.to_path_buf()).or_insert_with(|| text.to_owned());
        Ok(())
    }

    fn trash(&mut self, path: &Path, name: &str) -> Result<PathBuf, String> {
        let trash = self.home().join(".Trash");
        let to = (1..)
            .map(|n| trash.join(if n == 1 { name.to_owned() } else { format!("{name} {n}") }))
            .find(|t| !self.files.borrow().keys().any(|p| p.starts_with(t)))
            .unwrap();
        let moved: Vec<PathBuf> = self.files.borrow().keys().filter(|p| p.starts_with(path)).cloned().collect();
        if moved.is_empty() {
            return Err(format!("{}: no such folder", path.display()));
        }
        for p in moved {
            let there = to.join(p.strip_prefix(path).unwrap());
            let text = self.files.borrow_mut().remove(&p).unwrap();
            self.files.borrow_mut().insert(there.clone(), text);
            let id = self.ids.borrow_mut().remove(&p);
            if let Some(id) = id {
                self.ids.borrow_mut().insert(there, id);
            }
        }
        Ok(to)
    }

    fn watch(&mut self, dir: &Path, whole: bool) -> Result<(), String> {
        self.watched.push((dir.to_path_buf(), whole));
        Ok(())
    }

    fn run_script(&self, name: &str, _: &str, _: &str, _: String, done: Box<dyn FnOnce(Result<Vec<String>, String>) + Send>) {
        self.started.borrow_mut().push(name.to_owned());
        done(self.scripts.borrow().get(name).cloned().unwrap_or_else(|| Err(format!("no script {name} in the simulation"))));
    }

    fn home(&self) -> PathBuf {
        PathBuf::from(HOME)
    }

    fn now(&self) -> u64 {
        match self.clock.get() {
            0 => NOW,
            t => t,
        }
    }

    fn bits(&self) -> u64 {
        self.drawn.set(self.drawn.get() + 1);
        self.drawn.get().wrapping_mul(0x9E37_79B9_7F4A_7C15)
    }
}

/// Everything the daemon said back to itself handed to it, as its loop would.
fn pump(d: &mut Daemon<Sim>) {
    while let Ok(e) = d.world().inbox.as_ref().expect("an inbox").try_recv() {
        d.answer(e);
    }
}

const Q4: &str = "/h/w/q4/money.cells";
const QUARTERS: &str = "/h/w/quarters.cells";
const ACCOUNTS: &str = "/books/accounts.cells";
const MONEY: &str = "---\ntype Kind = income|cost\ncost: EUR = CASE WHEN kind = 'cost' THEN amount ELSE 0 END\n---\nkind:Kind,amount:EUR,cost\nincome,4200.00,0.00=\ncost,180.00,180.00=\n";
const INDEX: &str = "---\nimport q4 from \"./q4/money.cells\"\n---\nquarter:text!,costs:EUR\nq4,180.00=SUM(q4.cost)\n";

/// The engine started over `sim`.
pub(crate) fn started(mut sim: Sim) -> Daemon<Sim> {
    let (back, inbox) = channel();
    sim.inbox = Some(inbox);
    Daemon::start(sim, back).unwrap()
}

/// The engine stopped and started again over the same disk.
fn restarted(d: Daemon<Sim>) -> Daemon<Sim> {
    started(d.into_world())
}

fn daemon() -> Daemon<Sim> {
    let sim = Sim::default();
    sim.put(Q4, MONEY);
    sim.put(QUARTERS, INDEX);
    started(sim)
}

fn heard(d: &mut Daemon<Sim>, paths: &[&str]) {
    let heard: Vec<Heard> = paths.iter().map(|p| Heard { path: p.into(), id: None }).collect();
    d.hear(&heard);
}

/// The identity of the file held at `path`.
fn id(d: &Daemon<Sim>, path: &str) -> FileId {
    d.index().at(Path::new(path)).unwrap_or_else(|| panic!("{path} held")).id.clone()
}

/// What each line of the log is, in order.
fn kinds(log: &[String]) -> Vec<String> {
    log.iter().map(|l| l.split("\"kind\":\"").nth(1).unwrap().split('"').next().unwrap().to_owned()).collect()
}

/// Each line about the file `id`: its kind and its path then.
fn of(d: &Daemon<Sim>, id: &FileId) -> Vec<(String, String)> {
    d.world()
        .lines()
        .iter()
        .filter(|l| l.file == *id)
        .map(|l| (kinds(&[serde_json::to_string(l).unwrap()])[0].clone(), l.entry.path().to_owned()))
        .collect()
}

/// `from` moved to `to` on the simulated disk, as the Finder moves it on one disk: its
/// system ID with it.
fn mv(d: &mut Daemon<Sim>, from: &str, to: &str) {
    let w = d.world();
    let id = w.id(from);
    let text = w.remove(from);
    w.files.borrow_mut().insert(to.into(), text);
    w.ids.borrow_mut().insert(to.into(), id);
}

/// `from` moved to `to` across disks (a copy, the original removed), or replaced by iCloud:
/// another system ID.
fn copied(d: &mut Daemon<Sim>, from: &str, to: &str) {
    let text = d.world().remove(from);
    d.world().save_over(to, &text);
}

fn ask(d: &mut Daemon<Sim>, client: &str, request: ToEngine) -> FromEngine {
    let (answer, answered) = channel();
    d.answer(Event::Ask { id: 1, client: client.into(), request, answer: Some(answer) });
    answered.try_recv().expect("answered")
}

fn history(d: &mut Daemon<Sim>, file: Option<&str>, under: Option<&str>) -> Vec<crate::history::Chain> {
    let asked = ToEngine::History { file: file.map(PathBuf::from), under: under.map(PathBuf::from), limit: 100 };
    match ask(d, "app", asked) {
        FromEngine::History { chains } => chains,
        other => panic!("{other:?}"),
    }
}

#[test]
fn every_cells_file_under_home_is_held_by_an_identity_kept_across_restarts() {
    let sim = Sim::default();
    sim.put(Q4, MONEY);
    sim.put(QUARTERS, INDEX);
    sim.put("/h/Music/prices.cells", "price:num\n1\n");
    sim.put("/h/w/loose.csv", "a,b\n1,2\n");
    sim.put("/h/.config/hidden.cells", "a:num\n1\n");
    sim.put("/h/Library/Preferences/app.cells", "a:num\n1\n");
    sim.put("/h/Library/Mobile Documents/com~apple~CloudDocs/cloud.cells", "a:num\n1\n");
    let d = started(sim);
    assert!(d.world().watched.contains(&(PathBuf::from(HOME), true)), "the home folder watched whole: {:?}", d.world().watched);
    let held: Vec<String> = d.index().held().map(|k| k.path.display().to_string()).collect();
    assert_eq!(
        held,
        ["/h/Library/Mobile Documents/com~apple~CloudDocs/cloud.cells", "/h/Music/prices.cells", Q4, QUARTERS],
        "every .cells file the walk takes in, iCloud Drive's too; no CSV nobody imports"
    );
    assert!(d.world().log().is_empty(), "starting writes nothing in the history");
    let q4 = id(&d, Q4);
    let d = restarted(d);
    assert_eq!(id(&d, Q4), q4, "the index kept: the same file, the same identity");
}

#[test]
fn a_history_or_an_index_the_disk_will_not_give_is_refused() {
    let sim = Sim::default();
    sim.put(Q4, MONEY);
    sim.unreadable.borrow_mut().insert(crate::log::path(), "Resource deadlock avoided (os error 11)".into());
    let (back, _inbox) = channel();
    let Err(why) = Daemon::start(sim, back) else { panic!("refused") };
    assert!(why.contains("history/log.jsonl") && why.contains("os error 11"), "{why}");
}

#[test]
fn a_save_stores_what_it_reaches_in_every_file() {
    let mut d = daemon();
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    heard(&mut d, &[Q4]);
    assert_eq!(d.world().get(Q4), MONEY.replace("cost,180.00,180.00=", "cost,200.00,200.00="));
    assert_eq!(d.world().get(QUARTERS), INDEX.replace("180.00=SUM", "200.00=SUM"));
    let log = d.world().log();
    assert_eq!(kinds(&log), ["external", "store", "store"]);
    let (q4, quarters) = (id(&d, Q4), id(&d, QUARTERS));
    assert!(
        log[0].starts_with(&format!("{{\"n\":1,\"prev\":\"\",\"at\":{NOW},\"file\":\"{q4}\",\"kind\":\"external\",\"path\":\"{Q4}\"")),
        "{}",
        log[0]
    );
    assert!(log[1].starts_with(&format!("{{\"n\":2,\"prev\":\"{}\",", hash(&log[0]))), "each line links the one before");
    assert!(
        log[0].contains("\"edited\":{\"cells\":[{\"row\":2,\"column\":\"amount\",\"was\":\"180.00\",\"now\":\"200.00\"}]}"),
        "{}",
        log[0]
    );
    assert!(
        log[2].contains(&format!(
            "\"file\":\"{quarters}\",\"kind\":\"store\",\"path\":\"{QUARTERS}\",\"cause\":{{\"n\":1}},\"via\":[\"{Q4}\"]"
        )),
        "{}",
        log[2]
    );
    assert!(
        log[2].contains("\"stored\":[{\"row\":1,\"key\":\"q4\",\"column\":\"costs\",\"was\":\"180.00\",\"now\":\"200.00\"}]"),
        "{}",
        log[2]
    );
}

#[test]
fn a_file_moved_is_followed_by_every_import_naming_it() {
    let mut d = daemon();
    let q4 = id(&d, Q4);
    let to = "/h/w/2026/q4.cells";
    mv(&mut d, Q4, to);
    heard(&mut d, &[Q4, to]);
    assert_eq!(d.world().get(QUARTERS), INDEX.replace("./q4/money.cells", "./2026/q4.cells"), "the index names it where it is");
    assert_eq!(id(&d, to), q4, "the same file");
    let log = d.world().log();
    assert_eq!(kinds(&log), ["relocated", "store"]);
    assert!(log[0].contains(&format!("\"file\":\"{q4}\",\"kind\":\"relocated\",\"path\":\"{to}\",\"from\":\"{Q4}\"")), "{}", log[0]);
    assert!(log[1].contains("\"imports\":[[\"./q4/money.cells\",\"./2026/q4.cells\"]]"), "{}", log[1]);
    // the moved file read where it is: a change to it reaches the index as before
    d.world().put(to, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    heard(&mut d, &[to]);
    assert_eq!(d.world().get(QUARTERS), INDEX.replace("./q4/money.cells", "./2026/q4.cells").replace("180.00=SUM", "200.00=SUM"));
}

/// Rule 1: the events name a file ID, the disk none; the file edited as it moved.
#[test]
fn rule_1_a_move_heard_by_its_file_id_is_followed_edited_as_it_moved() {
    let sim = Sim { blind: true, ..Sim::default() };
    sim.put(Q4, MONEY);
    sim.put(QUARTERS, INDEX);
    let mut d = started(sim);
    let q4 = id(&d, Q4);
    let to = "/h/Music/q4.cells";
    mv(&mut d, Q4, to);
    let edited = MONEY.replace("income,4200.00", "income,4300.00");
    d.world().put(to, &edited);
    let sys = d.world().id(to);
    let ((), said) = caught(|| d.hear(&[Heard { path: Q4.into(), id: Some(sys) }, Heard { path: to.into(), id: Some(sys) }]));
    assert!(said.iter().any(|l| l.contains("~/w/q4/money.cells → ~/Music/q4.cells by its file ID, heard")), "{said:#?}");
    assert_eq!(id(&d, to), q4);
    assert_eq!(d.world().get(QUARTERS), INDEX.replace("./q4/money.cells", "../Music/q4.cells"));
    assert_eq!(of(&d, &q4), [("relocated".to_owned(), to.to_owned()), ("external".to_owned(), to.to_owned())], "moved, then its edit");
}

/// Two files trading places in one moment (through a name no table has): each is followed
/// to the other's path, though each path was the other's file's when the moment began.
#[test]
fn two_files_trading_places_are_each_followed() {
    let mut d = daemon();
    let (q4, quarters) = (id(&d, Q4), id(&d, QUARTERS));
    let (a, b) = (d.world().id(Q4), d.world().id(QUARTERS));
    let tmp = "/h/w/trading";
    mv(&mut d, Q4, tmp);
    mv(&mut d, QUARTERS, Q4);
    mv(&mut d, tmp, QUARTERS);
    let at = |path: &str, id| Heard { path: path.into(), id: Some(id) };
    d.hear(&[at(Q4, a), at(tmp, a), at(QUARTERS, b), at(Q4, b), at(tmp, a), at(QUARTERS, a)]);
    assert_eq!(id(&d, QUARTERS), q4);
    assert_eq!(id(&d, Q4), quarters);
    assert_eq!(d.world().get(Q4), INDEX.replace("./q4/money.cells", "../quarters.cells"), "its import follows the file it reads");
}

/// Rule 2: an editor's atomic save gives the file another system ID; it is the same file.
#[test]
fn rule_2_an_atomic_save_keeps_the_file_s_identity() {
    let mut d = daemon();
    let q4 = id(&d, Q4);
    let was = d.world().id(Q4);
    d.world().save_over(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    assert_ne!(d.world().id(Q4), was);
    heard(&mut d, &[Q4]);
    assert_eq!(id(&d, Q4), q4, "the same file");
    assert_eq!(kinds(&d.world().log()), ["external", "store", "store"], "a save, not a file added");
    assert_eq!(d.index().get(&q4).sys, Some(d.world().id(Q4)), "its system ID now, the engine's own write's");
}

/// Rule 3: the disk's file IDs, no event naming one; the file edited as it moved.
#[test]
fn rule_3_a_move_known_by_the_file_id_held_is_followed_edited_as_it_moved() {
    let mut d = daemon();
    let q4 = id(&d, Q4);
    let to = "/h/Music/q4.cells";
    mv(&mut d, Q4, to);
    d.world().put(to, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    heard(&mut d, &[Q4, to]);
    assert_eq!(id(&d, to), q4);
    assert_eq!(
        d.world().get(QUARTERS),
        INDEX.replace("./q4/money.cells", "../Music/q4.cells").replace("180.00=SUM", "200.00=SUM"),
        "followed, and its edit reached the index"
    );
    assert_eq!(kinds(&d.world().log())[..2], ["relocated", "store"]);
}

/// Rule 4: copied across disks (another system ID), its text the same.
#[test]
fn rule_4_a_file_come_with_a_gone_file_s_text_is_it() {
    let mut d = daemon();
    let q4 = id(&d, Q4);
    let to = "/h/w/2026/q4.cells";
    copied(&mut d, Q4, to);
    let ((), said) = caught(|| heard(&mut d, &[Q4, to]));
    assert!(said.iter().any(|l| l.contains("by its text")), "{said:#?}");
    assert_eq!(id(&d, to), q4);
    assert_eq!(d.world().get(QUARTERS), INDEX.replace("./q4/money.cells", "./2026/q4.cells"));
}

/// Rule 5: another system ID and another text: no move, never guessed.
#[test]
fn rule_5_a_file_gone_and_another_come_with_other_text_is_no_move() {
    let mut d = daemon();
    let q4 = id(&d, Q4);
    let to = "/h/w/2026/q4.cells";
    d.world().remove(Q4);
    d.world().put(to, &MONEY.replace("income,4200.00", "income,4300.00"));
    heard(&mut d, &[Q4, to]);
    assert_eq!(d.world().get(QUARTERS), INDEX, "never guessed: the index still names where it was");
    assert_ne!(id(&d, to), q4, "another file");
    assert!(d.index().get(&q4).gone, "the one gone remembered");
    assert_eq!(kinds(&d.world().log()), ["external", "external"]);
}

#[test]
fn a_file_moved_keeps_its_history() {
    let mut d = daemon();
    let q4 = id(&d, Q4);
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    heard(&mut d, &[Q4]);
    let to = "/h/w/2026/q4.cells";
    mv(&mut d, Q4, to);
    heard(&mut d, &[Q4, to]);
    assert_eq!(
        of(&d, &q4),
        [("external".to_owned(), Q4.to_owned()), ("store".to_owned(), Q4.to_owned()), ("relocated".to_owned(), to.to_owned())],
        "its saves before the move are its own"
    );
    let chains = history(&mut d, Some(to), None);
    assert_eq!(chains.len(), 2, "the move and the save before it: {chains:#?}");
}

#[test]
fn a_file_gone_is_found_later_in_another_folder() {
    let mut d = daemon();
    let q4 = id(&d, Q4);
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    heard(&mut d, &[Q4]);
    // moved off the home folder (another disk): heard gone
    let away = d.world().remove(Q4);
    d.world().clock.set(NOW + 1_000);
    heard(&mut d, &[Q4]);
    assert_eq!(kinds(&d.world().log())[3..], ["external"], "first heard as gone");
    // brought back, into another folder
    let to = "/h/Music/q4.cells";
    d.world().clock.set(NOW + 2_000);
    d.world().put(to, &away);
    heard(&mut d, &[to]);
    assert_eq!(id(&d, to), q4, "the file gone, found");
    assert_eq!(
        d.world().get(QUARTERS),
        INDEX.replace("./q4/money.cells", "../Music/q4.cells").replace("180.00=SUM", "200.00=SUM"),
        "the index names it where it went"
    );
    let roots: Vec<(String, String)> = history(&mut d, Some(to), None)
        .iter()
        .map(|c| (kinds(&[serde_json::to_string(&c.root).unwrap()])[0].clone(), c.root.entry.path().to_owned()))
        .collect();
    assert_eq!(
        roots,
        [("relocated".to_owned(), to.to_owned()), ("external".to_owned(), Q4.to_owned()), ("external".to_owned(), Q4.to_owned())],
        "moved, gone, saved: one history across both folders"
    );
}

#[test]
fn a_file_gone_is_remembered_across_a_restart() {
    let mut d = daemon();
    let q4 = id(&d, Q4);
    let away = d.world().remove(Q4);
    heard(&mut d, &[Q4]);
    let d = restarted(d);
    d.world().put("/h/v/q4.cells", &away);
    let d = restarted(d);
    assert_eq!(id(&d, "/h/v/q4.cells"), q4, "found by the walk, by its text");
    assert_eq!(d.world().get(QUARTERS), INDEX.replace("./q4/money.cells", "../v/q4.cells"));
}

#[test]
fn a_file_moved_and_edited_while_the_engine_was_stopped_is_followed_at_its_start() {
    let mut d = daemon();
    let q4 = id(&d, Q4);
    let to = "/h/Music/q4.cells";
    mv(&mut d, Q4, to);
    d.world().put(to, &MONEY.replace("income,4200.00", "income,4300.00"));
    d.world().put(QUARTERS, &INDEX.replace("quarter:text!", "quarter: text!"));
    let d = restarted(d);
    assert_eq!(id(&d, to), q4, "by the file ID the index held");
    assert!(d.world().get(QUARTERS).contains("import q4 from \"../Music/q4.cells\""));
    let log = d.world().log();
    assert_eq!(kinds(&log), ["external", "relocated", "store", "external"], "the index's edit, the move, followed, its edit: {log:#?}");
}

#[test]
fn a_file_of_a_gone_file_s_text_and_another_name_among_two_is_no_move() {
    let mut d = daemon();
    copied(&mut d, Q4, "/h/v/a.cells");
    d.world().put("/h/v/b.cells", MONEY);
    heard(&mut d, &[Q4, "/h/v/a.cells", "/h/v/b.cells"]);
    assert_eq!(d.world().get(QUARTERS), INDEX, "two files of its text, neither of its name: never guessed");
}

#[test]
fn a_folder_moved_moves_every_file_in_it() {
    let mut d = daemon();
    let (q4, quarters) = (id(&d, Q4), id(&d, QUARTERS));
    mv(&mut d, Q4, "/h/Books/w/q4/money.cells");
    mv(&mut d, QUARTERS, "/h/Books/w/quarters.cells");
    // the watcher names the folders alone
    heard(&mut d, &["/h/w", "/h/Books/w"]);
    assert_eq!(id(&d, "/h/Books/w/q4/money.cells"), q4);
    assert_eq!(id(&d, "/h/Books/w/quarters.cells"), quarters);
    assert_eq!(d.world().get("/h/Books/w/quarters.cells"), INDEX, "moved together, naming each other as they did");
    assert_eq!(kinds(&d.world().log()), ["relocated", "relocated"]);
}

#[test]
fn history_is_one_across_folders_a_folder_s_its_lines_under_it() {
    let mut d = daemon();
    let other = "/h/Music/prices.cells";
    d.world().put(other, "price:num\n1\n");
    heard(&mut d, &[other]);
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    heard(&mut d, &[Q4]);
    assert_eq!(history(&mut d, None, None).len(), 2, "every chain");
    let under = history(&mut d, None, Some("/h/w"));
    assert_eq!(under.len(), 1, "only what was under ~/w: {under:#?}");
    assert_eq!(under[0].root.entry.path(), Q4);
    assert_eq!(history(&mut d, Some(QUARTERS), None).len(), 1, "a file's chains are those reaching it");
}

#[test]
fn a_change_is_said_as_a_span_and_what_it_stored_in_it() {
    let mut d = daemon();
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    let ((), said) = caught(|| heard(&mut d, &[Q4]));
    assert_eq!(said[..2], ["heard    paths=~/w/q4/money.cells", "change   ~/w/q4/money.cells what=changed"], "{said:#?}");
    assert!(said.contains(&"  edited   cells=row 2 amount 180.00 → 200.00".to_owned()), "{said:#?}");
    assert!(
        said.contains(&"  stored   ~/w/quarters.cells via=~/w/q4/money.cells cells=costs row 1 180.00 → 200.00".to_owned()),
        "{said:#?}"
    );
}

#[test]
fn its_own_writes_heard_again_change_nothing() {
    let mut d = daemon();
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    heard(&mut d, &[Q4]);
    let logged = d.world().log().len();
    heard(&mut d, &[Q4, QUARTERS]);
    assert_eq!(d.world().log().len(), logged, "{:#?}", d.world().log());
}

#[test]
fn an_auto_packed_chain_settles_in_one_pass_and_its_writes_heard_again_log_nothing() {
    // b packs a, c packs b, each `pack: "auto"`, each packed as `cells pack` leaves it
    let a = "key:text!,v:EUR\nk,1.00\n";
    let importer = |from: &str| {
        format!("---\nimport {from} from \"./{from}.cells\" with {{ pack: \"auto\" }}\n---\nkey:text!,v:EUR={from}[key].v\nk,1.00=\n")
    };
    let mut held = BTreeMap::new();
    held.insert("/h/w/a.cells".to_owned(), a.to_owned());
    for (p, from) in [("b", "a"), ("c", "b")] {
        let path = format!("/h/w/{p}.cells");
        let text = importer(from);
        let doc = cells::read_with(&path, text.as_str(), &mut cells::files::Files::new(&held, &path), 1).unwrap();
        held.insert(path, doc.packed(cells::Pack::All));
    }
    let sim = Sim::default();
    for (p, t) in &held {
        sim.put(p, t);
    }
    let mut d = started(sim);
    assert!(d.world().log().is_empty(), "current copies write nothing");
    d.world().put("/h/w/a.cells", "key:text!,v:EUR\nk,2.00\n");
    heard(&mut d, &["/h/w/a.cells"]);
    let log = d.world().log();
    assert_eq!(kinds(&log), ["external", "store", "store"], "{log:#?}");
    assert!(log[1].contains("\"path\":\"/h/w/b.cells\"") && log[2].contains("\"path\":\"/h/w/c.cells\""), "{log:#?}");
    assert!(d.world().get("/h/w/c.cells").contains("packed b = ```cells\nkey: text!,v: EUR\nk,2.00\n```"));
    // its own writes heard again, and the source heard again unchanged: nothing
    heard(&mut d, &["/h/w/a.cells", "/h/w/b.cells", "/h/w/c.cells"]);
    heard(&mut d, &["/h/w/a.cells", "/h/w/b.cells", "/h/w/c.cells"]);
    assert_eq!(d.world().log().len(), 3, "{:#?}", d.world().log());
}

#[test]
fn a_file_changed_since_it_was_read_is_never_written_over() {
    let mut d = daemon();
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    // someone saves quarters by hand before the daemon hears q4
    let typed = INDEX.replace("q4,", "fourth,");
    d.world().put(QUARTERS, &typed);
    heard(&mut d, &[Q4]);
    assert_eq!(d.world().get(QUARTERS), typed, "their save kept");
    assert!(d.world().log().last().unwrap().contains(&format!("\"kind\":\"moved\",\"path\":\"{QUARTERS}\"")));
    // then their save is heard, and what q4 reached is stored over it
    heard(&mut d, &[QUARTERS]);
    assert_eq!(d.world().get(QUARTERS), typed.replace("180.00=SUM", "200.00=SUM"));
}

#[test]
fn a_value_typed_over_its_formula_stays_for_a_person() {
    let mut d = daemon();
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,180.00,999.00="));
    heard(&mut d, &[Q4]);
    assert_eq!(d.world().get(Q4), MONEY.replace("cost,180.00,180.00=", "cost,180.00,999.00="));
    assert_eq!(d.world().log().len(), 1, "the edit logged, nothing stored");
}

#[test]
fn an_import_outside_home_is_held_and_watched_where_it_is() {
    let sim = Sim::default();
    sim.put(ACCOUNTS, "account:text!,budget:EUR\nrent,900.00\n");
    sim.put(
        "/h/w/plan.cells",
        "---\nimport accounts from \"../../books/accounts.cells\"\n---\nwhat:text!,budget:EUR\nrent,900.00=SUM(accounts.budget)\n",
    );
    let mut d = started(sim);
    assert!(d.world().watched.contains(&(PathBuf::from("/books"), false)), "{:?}", d.world().watched);
    assert!(d.index().at(Path::new(ACCOUNTS)).is_some(), "held: a file one held imports");
    d.world().put(ACCOUNTS, "account:text!,budget:EUR\nrent,950.00\n");
    heard(&mut d, &[ACCOUNTS]);
    assert!(d.world().get("/h/w/plan.cells").contains("rent,950.00=SUM"));
}

#[test]
fn what_is_not_a_table_or_is_hidden_or_a_csv_nobody_imports_is_not_held() {
    let mut d = daemon();
    d.world().put("/h/w/.git/x.cells", "a\n1\n");
    d.world().put("/h/w/notes.txt", "hi\n");
    d.world().put("/h/w/loose.csv", "a,b\n1,2\n");
    d.world().put("/h/.Trash/old.cells", "a\n1\n");
    heard(&mut d, &["/h/w/.git/x.cells", "/h/w/notes.txt", "/h/w/loose.csv", "/h/.Trash/old.cells"]);
    for p in ["/h/w/.git/x.cells", "/h/w/loose.csv", "/h/.Trash/old.cells"] {
        assert!(d.index().at(Path::new(p)).is_none(), "{p}");
    }
    assert!(d.world().log().is_empty());
}

#[test]
fn a_file_trashed_is_gone() {
    let mut d = daemon();
    let q4 = id(&d, Q4);
    mv(&mut d, Q4, "/h/.Trash/money.cells");
    heard(&mut d, &[Q4, "/h/.Trash/money.cells"]);
    assert!(d.index().get(&q4).gone, "never followed into the Trash");
    assert_eq!(d.world().get(QUARTERS), INDEX);
}

#[test]
fn a_file_added_then_removed_is_logged_and_read_by_its_readers() {
    let mut d = daemon();
    d.world().put("/h/w/q1/money.cells", MONEY);
    heard(&mut d, &["/h/w/q1/money.cells"]);
    assert!(d.world().log()[0].contains("\"kind\":\"external\",\"path\":\"/h/w/q1/money.cells\",\"was\":null"));
    d.world().remove(Q4);
    heard(&mut d, &[Q4]);
    assert!(d.world().log().last().unwrap().contains(&format!("\"kind\":\"external\",\"path\":\"{Q4}\",\"was\":\"")));
    assert!(d.world().log().last().unwrap().contains("\"now\":null"));
}

#[test]
fn a_file_a_client_names_outside_home_is_held() {
    let mut d = daemon();
    let file = "/tmp/books/money.cells";
    d.world().put(file, MONEY);
    let applied = ask(
        &mut d,
        "cli",
        ToEngine::Intent { id: INTENT.into(), file: file.into(), against: hash(MONEY), ops: vec![set_amount("180.00", "200.00")] },
    );
    assert!(matches!(applied, FromEngine::Applied { .. }), "{applied:?}");
    assert!(d.index().at(Path::new(file)).is_some());
    assert_eq!(kinds(&d.world().log()), ["edit"]);
}

#[test]
fn a_client_listening_is_told_every_file_whose_version_changed() {
    let mut d = daemon();
    let (tell, told) = channel();
    let ((), said) = caught(|| d.answer(Event::Listen { id: 1, client: "lsp".into(), tell }));
    assert_eq!(said, ["joined   client=lsp id=1"]);
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    heard(&mut d, &[Q4]);
    d.tell();
    assert_eq!(told.try_recv().unwrap(), [PathBuf::from(Q4), PathBuf::from(QUARTERS)], "its edit and the engine's writes, once each");
    heard(&mut d, &[Q4, QUARTERS]);
    d.tell();
    assert!(told.try_recv().is_err(), "the engine's own writes heard again change nothing");
    drop(told);
    d.world().put(Q4, MONEY);
    heard(&mut d, &[Q4]);
    d.tell();
}

#[test]
fn an_approval_is_kept_by_the_engine_alone_and_said() {
    let mut d = daemon();
    let asked = ToEngine::Allow { file: PathBuf::from(Q4), script: "rates".into(), version: "ab12".into() };
    let (answer, said) = caught(|| ask(&mut d, "lsp", asked));
    assert_eq!(answer, FromEngine::Allowed { file: PathBuf::from(Q4), script: "rates".into() });
    assert_eq!(said, ["allowed, as shown to its person ~/w/q4/money.cells script=rates by=lsp"]);
    let approval = crate::allowed::Approval {
        file: PathBuf::from(Q4),
        script: "rates".into(),
        version: "ab12".into(),
        by: "lsp".into(),
        at: d.world().now(),
    };
    let kept = d.world().files.borrow()[&crate::allowed::path()].clone();
    assert_eq!(kept, format!("{}\n", crate::allowed::line(&approval)));
}

const EDITED: &str = "cost,200.00,180.00=";

#[test]
fn a_save_is_its_editor_s_when_it_said_it_held_that_version() {
    let mut d = daemon();
    let typed = MONEY.replace("cost,180.00,180.00=", EDITED);
    d.answer(Event::Ask {
        id: 1,
        client: "Helix".into(),
        request: ToEngine::Editing { file: Q4.into(), version: hash(&typed) },
        answer: None,
    });
    d.world().put(Q4, &typed);
    heard(&mut d, &[Q4]);
    assert!(d.world().log()[0].contains("\"by\":\"Helix\""), "{}", d.world().log()[0]);
    d.world().put(Q4, MONEY);
    heard(&mut d, &[Q4]);
    assert!(
        d.world().log().iter().filter(|l| l.contains("\"kind\":\"external\"")).nth(1).unwrap().contains("\"by\":null"),
        "a version no client held is no one's"
    );
}

#[test]
fn a_chain_is_its_root_and_every_file_it_reached() {
    let sim = Sim::default();
    sim.put(Q4, MONEY);
    sim.put(QUARTERS, INDEX);
    sim.put(
        "/h/w/year.cells",
        "---\nimport quarters from \"./quarters.cells\"\n---\nyear:text!,costs:EUR\n2026,180.00=SUM(quarters.costs)\n",
    );
    let mut d = started(sim);
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", EDITED));
    heard(&mut d, &[Q4]);
    let chains = history(&mut d, None, Some("/h/w"));
    let [chain] = &chains[..] else { panic!("one chain: {chains:#?}") };
    assert_eq!(chain.root.entry.path(), Q4);
    let followed: Vec<(&str, Vec<String>, Option<usize>)> = chain
        .followed
        .iter()
        .map(|f| match &f.event.entry {
            crate::log::Logged::Store { path, via, .. } => (path.as_str(), via.clone(), f.under),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        followed,
        [(Q4, vec![Q4.to_owned()], None), (QUARTERS, vec![Q4.to_owned()], None), ("/h/w/year.cells", vec![QUARTERS.to_owned()], Some(1))],
        "the edit's own write and the quarters under the edit, the year under the quarters"
    );
    let raw = serde_json::to_string(&chains).unwrap();
    assert_eq!(serde_json::from_str::<Vec<crate::history::Chain>>(&raw).unwrap(), chains, "a chain crosses the wire whole");
    assert_eq!(history(&mut d, Some("/h/w/year.cells"), None).len(), 1, "a file's chains are those reaching it");
}

#[test]
fn a_cell_says_every_value_it_held_and_the_root_of_each() {
    let mut d = daemon();
    for (n, cost) in ["200.00", "250.00"].iter().enumerate() {
        d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", &format!("cost,{cost},{}=", if n == 0 { "180.00" } else { "200.00" })));
        heard(&mut d, &[Q4]);
    }
    let FromEngine::Values { values } = ask(&mut d, "app", ToEngine::Cell { file: QUARTERS.into(), row: 1, column: "costs".into() }) else {
        panic!()
    };
    assert_eq!(values.iter().map(|v| (v.was.as_str(), v.value.as_str())).collect::<Vec<_>>(), [("200.00", "250.00"), ("180.00", "200.00")]);
    let log = d.world().log();
    for v in values {
        assert!(log[v.root as usize - 1].contains(&format!("\"kind\":\"external\",\"path\":\"{Q4}\"")), "each value's root is the q4 edit");
    }
}

#[test]
fn a_cell_is_followed_through_its_column_renamed() {
    let notes = "/h/w/notes.cells";
    let sim = Sim::default();
    sim.put(notes, "note:text,amount:EUR\nrent,900.00\n");
    let mut d = started(sim);
    let first = hash(&d.world().get(notes));
    for text in ["note:text,amount:EUR\nrent,950.00\n", "note:text,sum:EUR\nrent,950.00\n", "note:text,sum:EUR\nrent,1000.00\n"] {
        d.world().put(notes, text);
        heard(&mut d, &[notes]);
    }
    let log = d.world().log();
    assert!(log.iter().any(|l| l.contains("\"columns_renamed\":[{\"from\":\"amount\",\"to\":\"sum\"}]")), "{log:#?}");
    let FromEngine::Values { values } = ask(&mut d, "app", ToEngine::Cell { file: notes.into(), row: 1, column: "sum".into() }) else {
        panic!()
    };
    assert_eq!(
        values.iter().map(|v| (v.was.as_str(), v.value.as_str())).collect::<Vec<_>>(),
        [("950.00", "1000.00"), ("900.00", "950.00")],
        "the value it held under its old name"
    );
    let FromEngine::Since { marks, .. } = ask(&mut d, "app", ToEngine::Since { file: notes.into(), version: first }) else { panic!() };
    let [m] = &marks[..] else { panic!("one cell, renamed between its changes: {marks:#?}") };
    assert_eq!((m.column.as_deref(), m.was.as_deref(), m.now.as_str()), (Some("sum"), Some("900.00"), "1000.00"));
}

#[test]
fn a_cell_s_values_follow_its_file_across_a_move() {
    let mut d = daemon();
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    heard(&mut d, &[Q4]);
    let to = "/h/Music/q4.cells";
    mv(&mut d, Q4, to);
    heard(&mut d, &[Q4, to]);
    let FromEngine::Values { values } = ask(&mut d, "app", ToEngine::Cell { file: to.into(), row: 2, column: "amount".into() }) else {
        panic!()
    };
    assert_eq!(
        values.iter().map(|v| (v.was.as_str(), v.value.as_str())).collect::<Vec<_>>(),
        [("180.00", "200.00")],
        "saved before it moved"
    );
}

#[test]
fn a_file_returned_to_a_version_is_a_change_of_its_own() {
    let mut d = daemon();
    let first = hash(MONEY);
    assert_eq!(d.world().get(crate::log::version(&first).to_str().unwrap()), MONEY, "every version seen is kept");
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", EDITED));
    heard(&mut d, &[Q4]);
    let FromEngine::Restored { .. } = ask(&mut d, "app", ToEngine::Restore { file: Q4.into(), version: first.clone() }) else { panic!() };
    assert_eq!(d.world().get(Q4), MONEY);
    assert_eq!(d.world().get(QUARTERS), INDEX, "what it reached follows it back");
    let log = d.world().log();
    let restore = log.iter().find(|l| l.contains("\"kind\":\"restore\"")).expect("logged");
    assert!(restore.contains("\"by\":\"app\"") && restore.contains(&format!("\"now\":\"{first}\"")), "{restore}");
    assert!(log.len() > 3, "history kept, never rewritten");
    let stored = hash(&INDEX.replace("180.00=SUM", "200.00=SUM"));
    let FromEngine::Version { text, .. } = ask(&mut d, "app", ToEngine::Version { file: QUARTERS.into(), version: stored }) else {
        panic!()
    };
    assert_eq!(text, INDEX.replace("180.00=SUM", "200.00=SUM"), "the version the engine stored, shown whole");
}

const TRIP: &str = "/h/w/trip.cells";

/// A file whose table converts with the rates a script fetches every day at 18:00 UTC, last
/// run on 2026-11-02 at 18:00: a run missed since.
fn trip(schedule: &str) -> String {
    format!(
        "---\nrates = run {schedule}\n```ts\nexport default async () => []\n```\n```cells\ndate:date!,rate:num>0,run:int?\n2026-11-03,0.9,1\n```\n```cells runs\nrun:int!,at:datetime,outcome:ok|failed,error:text?\n1,2026-11-02T18:00:00Z,ok,\n```\n---\ndate:date,usd:USD,eur:EUR=usd * rates[date].rate\n2026-11-03,10.00,9.00=\n"
    )
}

fn allow_all(d: &Daemon<Sim>, file: &str) {
    let doc = d.folder().doc(file).expect("read");
    let lines: Vec<String> = doc
        .scripts()
        .iter()
        .map(|s| {
            crate::allowed::line(&crate::allowed::Approval {
                file: file.into(),
                script: s.name.clone(),
                version: crate::allowed::version(s),
                by: "test".into(),
                at: 0,
            })
        })
        .collect();
    d.world().put(crate::allowed::path().to_str().unwrap(), &format!("{}\n", lines.join("\n")));
}

fn with_trip(schedule: &str) -> Daemon<Sim> {
    let sim = Sim::default();
    sim.put(TRIP, &trip(schedule));
    sim.scripts.borrow_mut().insert("rates".into(), Ok(vec![r#"{"date":"2026-11-03","rate":0.95}"#.into()]));
    started(sim)
}

#[test]
fn a_run_missed_runs_once_when_allowed_and_is_a_root_of_what_it_reached() {
    let mut d = with_trip("every day at 18:00 in UTC");
    let ((), said) = caught(|| _ = d.tick());
    assert!(d.world().started.borrow().is_empty(), "not allowed: not run");
    assert!(said.iter().any(|l| l.contains("due, and not run: not allowed")), "{said:#?}");
    let ((), again) = caught(|| _ = d.tick());
    assert!(!again.iter().any(|l| l.contains("not allowed")), "said once: {again:#?}");
    allow_all(&d, TRIP);
    d.tick();
    pump(&mut d);
    assert_eq!(*d.world().started.borrow(), ["rates"], "the run missed since 2026-11-03 18:00, once");
    let file = d.world().get(TRIP);
    assert!(file.contains("2026-11-03,0.95,2\n"), "{file}");
    assert!(file.contains("2,2026-11-06T21:20:00Z,ok,\n"), "its runs block: {file}");
    assert!(file.contains("2026-11-03,10.00,9.50=\n"), "what the rates reached, stored: {file}");
    let log = d.world().log();
    assert!(
        log[0].contains(&format!(
            "\"kind\":\"run\",\"path\":\"{TRIP}\",\"script\":\"rates\",\"run\":2,\"outcome\":\"ok\",\"added\":0,\"changed\":1,\"by\":\"its schedule\""
        )),
        "{}",
        log[0]
    );
    d.tick();
    pump(&mut d);
    assert_eq!(d.world().started.borrow().len(), 1, "ran, and next due tomorrow at 18:00");
    d.world().clock.set(NOW + 21 * 3_600_000);
    d.tick();
    pump(&mut d);
    assert_eq!(d.world().started.borrow().len(), 2, "2026-11-07 18:00 passed");
}

#[test]
fn a_run_that_fails_writes_only_its_failure() {
    let mut d = with_trip("every day at 18:00 in UTC");
    d.world().scripts.borrow_mut().insert("rates".into(), Err("the bank's API answered 503".into()));
    allow_all(&d, TRIP);
    d.tick();
    pump(&mut d);
    let file = d.world().get(TRIP);
    assert!(file.contains("2026-11-03,0.9,1\n") && file.contains("2026-11-03,10.00,9.00=\n"), "rows as they were: {file}");
    assert!(file.contains("2,2026-11-06T21:20:00Z,failed,the bank's API answered 503\n"), "{file}");
    assert!(d.world().log()[0].contains("\"outcome\":\"failed\",\"why\":\"the bank's API answered 503\""), "{}", d.world().log()[0]);
    d.tick();
    pump(&mut d);
    assert_eq!(d.world().started.borrow().len(), 1, "a failure is a run: next due tomorrow, never retried in a loop");
}

#[test]
fn a_client_runs_a_script_now_and_hears_when_it_landed() {
    let mut d = with_trip("every day at 18:00 in UTC");
    let run = ToEngine::Run { file: TRIP.into(), script: "rates".into() };
    let FromEngine::Refused { why } = ask(&mut d, "cli", run.clone()) else { panic!("an unallowed script is refused") };
    assert!(why.contains("cells allow"), "{why}");
    allow_all(&d, TRIP);
    let (answer, answered) = channel();
    d.answer(Event::Ask { id: 1, client: "cli".into(), request: run, answer: Some(answer) });
    pump(&mut d);
    let FromEngine::Ran { run, outcome, .. } = answered.try_recv().expect("answered once landed") else { panic!() };
    assert_eq!((run, outcome), (2, crate::log::Outcome::Ok { added: 0, changed: 1 }));
    assert!(d.world().log()[0].contains("\"by\":\"cli\""));
}

#[test]
fn a_script_runs_when_the_file_it_names_changes() {
    let sim = Sim::default();
    sim.put("/h/w/notes.cells", "note:text\nhello\n");
    sim.put(TRIP, &trip("when ./notes.cells changes"));
    sim.scripts.borrow_mut().insert("rates".into(), Ok(vec![r#"{"date":"2026-11-03","rate":0.95}"#.into()]));
    let mut d = started(sim);
    allow_all(&d, TRIP);
    d.tick();
    assert!(d.world().started.borrow().is_empty(), "nothing changed yet");
    d.world().put("/h/w/notes.cells", "note:text\nbye\n");
    heard(&mut d, &["/h/w/notes.cells"]);
    d.tick();
    pump(&mut d);
    assert_eq!(*d.world().started.borrow(), ["rates"]);
    assert!(d.world().log().iter().any(|l| l.contains("\"by\":\"a change to ~/w/notes.cells\"")), "{:#?}", d.world().log());
}

#[test]
fn what_changed_since_a_version_is_marked_with_the_chain_that_changed_it() {
    let mut d = daemon();
    let opened = hash(INDEX);
    let since = |d: &mut Daemon<Sim>, file: &str, version: &str| match ask(
        d,
        "app",
        ToEngine::Since { file: file.into(), version: version.into() },
    ) {
        FromEngine::Since { known, marks, chains } => (known, marks, chains),
        other => panic!("{other:?}"),
    };
    for cost in ["200.00", "250.00"] {
        let held = d.world().get(Q4);
        let line = held.lines().find(|l| l.starts_with("cost,")).unwrap().to_owned();
        d.world().put(Q4, &held.replace(&line, &format!("cost,{cost},{}", line.rsplit(',').next().unwrap())));
        heard(&mut d, &[Q4]);
    }
    let (known, marks, chains) = since(&mut d, QUARTERS, &opened);
    assert!(known);
    let [m] = &marks[..] else { panic!("one cell, changed twice: {marks:#?}") };
    assert_eq!((m.row, m.column.as_deref(), m.was.as_deref(), m.now.as_str()), (1, Some("costs"), Some("180.00"), "250.00"));
    assert_eq!(m.key.as_deref(), Some("q4"));
    let root = chains.iter().find(|c| c.root.n == m.root).expect("its chain");
    assert_eq!(root.root.entry.path(), Q4, "the save in q4 that reached it");
    let now = hash(&d.world().get(QUARTERS));
    assert!(since(&mut d, QUARTERS, &now).1.is_empty(), "nothing since the version it holds");
    assert!(!since(&mut d, QUARTERS, &hash("a version never logged")).0);

    // a row added above a cell changed since: the cell followed to where it is now
    let before = hash(&d.world().get(Q4));
    let held = d.world().get(Q4);
    d.world().put(Q4, &held.replace("kind:Kind,amount:EUR,cost\n", "kind:Kind,amount:EUR,cost\ncost,1.00,0.00=\n"));
    heard(&mut d, &[Q4]);
    let (_, marks, _) = since(&mut d, Q4, &before);
    assert!(marks.iter().any(|m| m.row == 1 && m.column.is_none()), "the row added: {marks:#?}");
    assert!(marks.iter().any(|m| m.row == 1 && m.column.is_none() && m.was.is_none()));
}

const INTENT: &str = "k7m2q9xa";

fn intent(d: &mut Daemon<Sim>, id: &str, against: &str, ops: Vec<crate::log::Op>) -> FromEngine {
    ask(d, "Cells Dev", ToEngine::Intent { id: id.into(), file: Q4.into(), against: against.into(), ops })
}

fn set_amount(was: &str, to: &str) -> crate::log::Op {
    crate::log::Op::Set {
        row: crate::log::Row::Line("cost,180.00,180.00=".into()),
        column: "amount".into(),
        was: Some(was.into()),
        text: to.into(),
    }
}

#[test]
fn an_intent_is_journalled_its_rows_given_ids_and_what_it_reached_follows_it() {
    let mut d = daemon();
    let FromEngine::Applied { version, assigned, .. } = intent(&mut d, INTENT, &hash(MONEY), vec![set_amount("180.00", "200.00")]) else {
        panic!("applied")
    };
    let q4 = d.world().get(Q4);
    assert_eq!(version, hash(&q4));
    assert_eq!(assigned.len(), 2, "a file with no key gets an id in every row at the engine's first write: {q4}");
    assert!(q4.contains("kind:Kind,amount:EUR,cost,id: id!\n"), "after the person's columns: {q4}");
    assert!(q4.contains(&format!("cost,200.00,200.00=,{}\n", assigned[1])), "the cell set, its computed cell stored: {q4}");
    assert_eq!(d.world().get(QUARTERS), INDEX.replace("180.00=SUM", "200.00=SUM"), "what it reached follows");
    let log = d.world().log();
    assert!(log[0].contains(&format!("\"kind\":\"edit\",\"path\":\"{Q4}\",\"intent\":\"k7m2q9xa\",\"by\":\"Cells Dev\"")), "{}", log[0]);
    assert!(
        log.iter().any(|l| l.contains(&format!("\"path\":\"{QUARTERS}\",\"cause\":{{\"n\":1}}"))),
        "the index's store follows the edit: {log:#?}"
    );
    // sent again, as a client that never heard the answer would: answered the same, nothing applied twice
    let again = intent(&mut d, INTENT, &hash(MONEY), vec![set_amount("180.00", "200.00")]);
    assert_eq!(again, FromEngine::Applied { file: Q4.into(), intent: INTENT.into(), version, assigned });
    assert_eq!(d.world().log().len(), log.len(), "applied once");
}

#[test]
fn an_intent_made_against_an_earlier_version_applies_only_where_it_saw_what_is_there() {
    let mut d = daemon();
    // someone saves the income row since the intent was made
    d.world().put(Q4, &MONEY.replace("income,4200.00,0.00=", "income,4300.00,0.00="));
    heard(&mut d, &[Q4]);
    let applied = intent(&mut d, INTENT, &hash(MONEY), vec![set_amount("180.00", "200.00")]);
    assert!(matches!(applied, FromEngine::Applied { .. }), "the cost row is untouched: {applied:?}");
    let q4 = d.world().get(Q4);
    assert!(q4.contains("income,4300.00,0.00=,") && q4.contains("cost,200.00,200.00=,"), "both kept: {q4}");
    // an intent whose cell was changed since is refused, both said, nothing written
    let before = d.world().get(Q4);
    let FromEngine::Refused { why } = intent(
        &mut d,
        "a1b2c3d4",
        &hash(MONEY),
        vec![crate::log::Op::Set {
            row: crate::log::Row::Line("income,4200.00,0.00=".into()),
            column: "amount".into(),
            was: Some("4200.00".into()),
            text: "4250.00".into(),
        }],
    ) else {
        panic!("refused")
    };
    assert!(why.contains("no row is that one"), "the row it named reads otherwise now: {why}");
    assert_eq!(d.world().get(Q4), before);
}

#[test]
fn a_whole_text_is_written_logged_by_its_version_and_laid_over_a_moved_file() {
    let mut d = daemon();
    let text = |t: &str| crate::log::Op::Text { text: t.into(), version: None };
    // the app's text, made against what the engine holds: the file, its rows given ids
    let mine = MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00=");
    let FromEngine::Applied { version: first, assigned, .. } = intent(&mut d, INTENT, &hash(MONEY), vec![text(&mine)]) else {
        panic!("applied")
    };
    let q4 = d.world().get(Q4);
    assert_eq!(first, hash(&q4));
    assert_eq!(assigned.len(), 2, "a .cells file without a key gets its ids: {q4}");
    assert!(q4.contains("cost,200.00,200.00=,"), "its computed cell stored: {q4}");
    let log = d.world().log();
    let edit = log.iter().find(|l| l.contains("\"kind\":\"edit\"")).expect("the edit logged");
    assert!(edit.contains(&format!("\"op\":\"text\",\"version\":\"{}\"", hash(&mine))), "by its version: {edit}");
    assert!(!edit.contains("\"text\":"), "never its text: {edit}");
    assert_eq!(d.world().get(crate::log::version(&hash(&mine)).to_str().unwrap()), mine, "its text kept, though the file never held it");
    // someone saves the income row; the app, not having heard, sends its next text made
    // against the version it holds: its row laid over theirs
    d.world().put(Q4, &q4.replace("income,4200.00,0.00=,", "income,4300.00,0.00=,"));
    heard(&mut d, &[Q4]);
    let next = q4.replace("cost,200.00,200.00=,", "cost,210.00,200.00=,");
    let applied = intent(&mut d, "a1b2c3d4", &first, vec![text(&next)]);
    assert!(matches!(applied, FromEngine::Applied { .. }), "{applied:?}");
    let q4 = d.world().get(Q4);
    assert!(q4.contains("income,4300.00,0.00=,") && q4.contains("cost,210.00,210.00=,"), "both kept: {q4}");
}

#[test]
fn a_version_on_disk_not_yet_heard_is_heard_before_an_intent_applies() {
    let mut d = daemon();
    // a save lands on disk; the engine has not heard it when the intent arrives
    d.world().put(Q4, &MONEY.replace("income,4200.00,0.00=", "income,4300.00,0.00="));
    let applied = intent(&mut d, INTENT, &hash(MONEY), vec![set_amount("180.00", "200.00")]);
    assert!(matches!(applied, FromEngine::Applied { .. }), "{applied:?}");
    let q4 = d.world().get(Q4);
    assert!(q4.contains("income,4300.00,0.00=,") && q4.contains("cost,200.00,200.00=,"), "nobody's write lost: {q4}");
    let log = d.world().log();
    assert_eq!(&kinds(&log)[..2], ["external", "edit"], "the save it found, then the edit over it: {log:#?}");
}

/// Books kept as plain files, each note holding what would read as a formula in a `.cells`
/// file: an `=` with arithmetic either side of it, `×`, `€`, `→`, a note that opens with `=`,
/// a quoted field holding the separator. Each with the row the test edits, its separator.
const PLAIN: [(&str, &str, char); 3] = [
    (
        "/h/w/money.csv",
        "date,amount,notes\n2026-04-30,50.18,\"prorrateado (17 de 30 días → 50,18 = 88,56 × 17/30), 88,56 €\"\n2026-05-31,88.56,tarifa plana = 88.56 € (80 + MEI)\n2026-06-30,11.12,=SUM(B2:B3)\n",
        ',',
    ),
    (
        "/h/w/gastos.csv",
        "fecha;importe;notas\r\n2026-04-30;50,18;\"tarifa; prorrateada → 50,18 = 88,56 × 17/30 €\"\r\n2026-05-31;88,56;total = 88,56 € (80 + MEI)\r\n2026-06-30;11,12;=SUMA(B2:B3)\r\n",
        ';',
    ),
    (
        "/h/w/money.tsv",
        "date\tamount\tnotes\n2026-04-30\t50.18\tprorrateado → 50,18 = 88,56 × 17/30, \"88,56 €\"\n2026-05-31\t88.56\ttarifa plana = 88.56 € (80 + MEI)\n2026-06-30\t11.12\t=SUM(B2:B3)\n",
        '\t',
    ),
];

#[test]
fn a_plain_file_is_read_plain_and_an_intent_changes_its_one_cell_alone() {
    let sim = Sim::default();
    for (path, text, _) in PLAIN {
        sim.put(path, text);
    }
    let mut d = started(sim);
    for (k, (path, text, sep)) in PLAIN.into_iter().enumerate() {
        let id = ["a1b2c3d4", "a1b2c3d5", "a1b2c3d6"][k];
        // set the amount of the second row, the file named by the client: held from then
        let line = text.lines().nth(2).unwrap().trim_end_matches('\r');
        let (was, now) = if sep == ';' { ("88,56", "90,00") } else { ("88.56", "90.00") };
        let header = text.lines().next().unwrap().trim_end_matches('\r');
        let cols: Vec<&str> = header.split(sep).collect();
        let set = crate::log::Op::Set {
            row: crate::log::Row::Line(line.into()),
            column: cols[1].into(),
            was: Some(was.into()),
            text: now.into(),
        };
        let file = PathBuf::from(path);
        let applied = ask(&mut d, "Cells Dev", ToEngine::Intent { id: id.into(), file, against: hash(text), ops: vec![set] });
        assert!(matches!(applied, FromEngine::Applied { ref assigned, .. } if assigned.is_empty()), "{path}: {applied:?}");
        let doc = d.folder().doc(path).unwrap_or_else(|| panic!("{path} is read"));
        assert_eq!(doc.dialect(), cells::Dialect::Plain(cells::Separator::of(sep as u8).unwrap()), "{path}");
        assert!(doc.problems().is_empty(), "{path}: {:?}", doc.problems().iter().map(|p| doc.describe(p)).collect::<Vec<_>>());
        let written = |c: &cells::Column| c.formula.is_none() && (0..doc.rows()).all(|r| c.origin(r) == cells::Origin::Written);
        assert!(doc.columns().iter().all(written), "{path}: a plain file has no formula");
        // every note read whole, as its text
        let notes: Vec<String> = (0..doc.rows()).map(|r| doc.text(r, 2).into_owned()).collect();
        assert!(notes[0].contains("= 88,56 × 17/30") && notes[0].contains('€') && notes[0].contains('→'), "{path}: {notes:?}");
        assert!(notes[1].contains("= 88") && notes[2].starts_with('='), "{path}: {notes:?}");
        let notes = doc.columns()[2].name.clone();
        let edited = line.replacen(&format!("{sep}{was}{sep}"), &format!("{sep}{now}{sep}"), 1);
        assert_eq!(d.world().get(path), text.replacen(line, &edited, 1), "{path}: one cell changed, no id, no '=");
        // a note set to text that holds `=` and the separator is written as its value, quoted
        let row = edited.clone();
        let note = crate::log::Op::Set { row: crate::log::Row::Line(row), column: notes, was: None, text: format!("a = b{sep} c") };
        let before = d.world().get(path);
        let id = ["b1b2c3d4", "b1b2c3d5", "b1b2c3d6"][k];
        let file = PathBuf::from(path);
        let applied = ask(&mut d, "Cells Dev", ToEngine::Intent { id: id.into(), file, against: hash(&before), ops: vec![note] });
        assert!(matches!(applied, FromEngine::Applied { .. }), "{path}: {applied:?}");
        let noted = edited.rsplit_once(sep).unwrap().0.to_owned() + &format!("{sep}\"a = b{sep} c\"");
        assert_eq!(d.world().get(path), before.replacen(&edited, &noted, 1), "{path}");
    }
}

#[test]
fn a_cells_file_importing_a_csv_reads_it_plain() {
    let sim = Sim::default();
    let (csv, text, _) = PLAIN[0];
    sim.put(csv, text);
    let book = "---\nimport money from \"./money.csv\"\n---\nwhat:text!,total:EUR\nq2,149.86=SUM(money.amount)\n";
    sim.put("/h/w/book.cells", book);
    let d = started(sim);
    assert!(d.index().at(Path::new(csv)).is_some(), "a CSV one held imports is held");
    let doc = d.folder().doc("/h/w/book.cells").unwrap();
    assert!(doc.problems().is_empty(), "{:?}", doc.problems().iter().map(|p| doc.describe(p)).collect::<Vec<_>>());
    let imported = doc.imported()[0].1.as_deref().expect("the csv is reached");
    assert!(matches!(imported.dialect(), cells::Dialect::Plain(_)));
    assert!(imported.text(0, 2).contains("= 88,56 × 17/30"));
    // the `.cells` file's own `=` is still a formula, computed over the csv's amounts
    assert_eq!(doc.columns()[1].origin(0), cells::Origin::Cell);
    assert_eq!(doc.text(0, 1), "149.86=SUM(money.amount)");
}

/// A folder's own log as it was written: each line its fields, numbered from 1 and linked
/// to the one before.
fn folder_log(lines: Vec<serde_json::Value>) -> String {
    let mut prev = String::new();
    let mut out = String::new();
    for (i, line) in lines.into_iter().enumerate() {
        let serde_json::Value::Object(fields) = line else { panic!("a line is an object") };
        let mut l = serde_json::Map::new();
        l.insert("n".into(), (i as u64 + 1).into());
        l.insert("prev".into(), prev.clone().into());
        l.extend(fields);
        let raw = serde_json::to_string(&l).unwrap();
        prev = hash(&raw);
        out.push_str(&raw);
        out.push('\n');
    }
    out
}

/// Two folders that kept their own history: `~/a` saved `x.cells` (what it reached stored
/// in `~/a/y.cells`, and in `~/b/t.cells` through an import), then `x.cells` moved into `~/b`,
/// heard gone in `~/a` and found in `~/b`.
#[test]
fn every_folder_s_history_moves_into_the_one_and_its_folder_to_the_trash() {
    use serde_json::json;
    let text = |what: &str, v: &str| format!("{what}:num\n{v}\n");
    let (x0, x1, y0, y1, t0, t1) = (text("x", "1"), text("x", "2"), text("y", "1"), text("y", "2"), text("t", "1"), text("t", "2"));
    let sim = Sim::default();
    sim.put("/h/a/y.cells", &y1);
    sim.put("/h/b/x.cells", &x1);
    sim.put("/h/b/t.cells", &t1);
    let a = folder_log(vec![
        json!({"at": 1000, "kind": "external", "path": "x.cells", "was": hash(&x0), "now": hash(&x1), "by": null}),
        json!({"at": 1000, "kind": "store", "path": "y.cells", "cause": {"n": 1}, "via": ["x.cells"], "against": hash(&y0), "now": hash(&y1), "stored": []}),
        json!({"at": 1500, "kind": "external", "path": "x.cells", "was": hash(&x1), "now": null, "by": null}),
    ]);
    let b = folder_log(vec![
        json!({"at": 1000, "kind": "store", "path": "t.cells", "cause": {"n": 1, "folder": "/h/a"}, "via": ["/h/a/x.cells"], "against": hash(&t0), "now": hash(&t1), "stored": []}),
        json!({"at": 2000, "kind": "relocated", "path": "x.cells", "from": "/h/a/x.cells", "now": hash(&x1)}),
    ]);
    sim.put("/h/a/.cells/log.jsonl", &a);
    sim.put("/h/b/.cells/log.jsonl", &b);
    for (folder, versions) in [("/h/a", [&x0, &x1, &y0, &y1]), ("/h/b", [&x1, &t0, &t1, &t1])] {
        for v in versions {
            sim.put(&format!("{folder}/.cells/versions/{}", hash(v)), v);
        }
    }
    sim.put("/h/a/.cells/serve.lock", "");
    let ((), said) = caught(|| {
        let d = started(sim);
        let lines = d.world().lines();
        let shape: Vec<(u64, String, Option<u64>)> =
            lines.iter().map(|l| (l.n, l.entry.path().to_owned(), l.entry.cause().map(|c| c.n))).collect();
        assert_eq!(
            shape,
            [
                (1, "/h/a/x.cells".to_owned(), None),
                (2, "/h/a/y.cells".to_owned(), Some(1)),
                (3, "/h/b/t.cells".to_owned(), Some(1)),
                (4, "/h/a/x.cells".to_owned(), None),
                (5, "/h/b/x.cells".to_owned(), None)
            ],
            "by time, a root before what follows it, every cause numbered anew"
        );
        assert!(matches!(&lines[4].entry, crate::log::Logged::Relocated { from, .. } if from == "/h/a/x.cells"));
        assert_eq!(lines[0].file, lines[4].file, "the file moved is one file across both folders");
        assert_eq!(lines[0].file, lines[3].file);
        assert_eq!(id(&d, "/h/b/x.cells"), lines[0].file, "the index holds it where it is");
        assert_eq!(d.world().get(crate::log::path().to_str().unwrap()), crate::log::written(&lines), "one chain, each line linked");
        for v in [&x0, &x1, &y0, &y1, &t0, &t1] {
            assert_eq!(&d.world().get(crate::log::version(&hash(v)).to_str().unwrap()), v, "every version copied");
        }
        for folder in ["/h/a", "/h/b"] {
            assert!(!d.world().files.borrow().keys().any(|p| p.starts_with(format!("{folder}/.cells"))), "{folder}/.cells moved");
        }
        assert_eq!(d.world().get("/h/.Trash/a .cells/log.jsonl"), a, "in the Trash whole, nothing deleted");
        assert_eq!(d.world().get("/h/.Trash/b .cells/log.jsonl"), b);
        assert!(d.world().files.borrow().contains_key(Path::new("/h/.Trash/a .cells/serve.lock")));
        let mut d = d;
        let chains = history(&mut d, Some("/h/b/x.cells"), None);
        assert_eq!(chains.len(), 3, "its history, from both folders: {chains:#?}");
        // started again: the history there, nothing looked for again
        let d = restarted(d);
        assert_eq!(d.world().lines().len(), 5);
    });
    assert!(said.iter().any(|l| l.contains("folders=2 lines=5 files=3")), "x, y and t: {said:#?}");
}

#[test]
fn a_home_with_no_folder_history_starts_one_empty() {
    let d = daemon();
    assert_eq!(d.world().files.borrow().get(&crate::log::path()).map(String::as_str), Some(""), "looked for once");
}

#[test]
fn an_intent_that_names_no_id_writes_one_or_breaks_a_rule_is_refused() {
    let mut d = daemon();
    let FromEngine::Refused { why } = intent(&mut d, "not-an-id", &hash(MONEY), vec![]) else { panic!() };
    assert!(why.contains("is no intent id"), "{why}");
    let add = crate::log::Op::Add { cells: vec![crate::log::Given { column: "id".into(), text: "k7m2q9xa".into() }] };
    let FromEngine::Refused { why } = intent(&mut d, INTENT, &hash(MONEY), vec![add]) else { panic!() };
    assert!(why.contains("the id column is the engine's"), "{why}");
    let word = set_amount("180.00", "lots");
    let FromEngine::Refused { why } = intent(&mut d, "a1b2c3d4", &hash(MONEY), vec![word]) else { panic!() };
    assert!(why.contains("would break a rule"), "{why}");
    assert_eq!(d.world().get(Q4), MONEY, "nothing written");
    assert!(d.world().log().is_empty(), "nothing logged");
}
