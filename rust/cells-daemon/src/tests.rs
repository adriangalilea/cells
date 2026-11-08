//! The daemon against a simulated world: files in memory, change events handed in by
//! the test, a clock that only moves when asked. Every run is the same run.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;

use crate::daemon::{Daemon, Event};
use crate::log::{Logged, hash};
use crate::trace::caught;
use crate::wire::{FromEngine, ToEngine};
use crate::world::{Found, Replaced, World, table_file};

#[derive(Default)]
struct Sim {
    files: RefCell<BTreeMap<PathBuf, String>>,
    locked: Vec<PathBuf>,
    watched: Vec<(PathBuf, bool)>,
    /// What each script returns when run, by name: its lines, or why it failed.
    scripts: RefCell<BTreeMap<String, Result<Vec<String>, String>>>,
    /// Every script started, by name, in order.
    started: RefCell<Vec<String>>,
    /// The clock, in milliseconds since the Unix epoch: `NOW` unless a test moves it.
    clock: std::cell::Cell<u64>,
    /// The random bits handed out, counted: every run the same ids.
    drawn: std::cell::Cell<u64>,
    /// What the daemon says back to itself (a run ended), read by `pump`.
    inbox: Option<std::sync::mpsc::Receiver<Event>>,
}

/// The simulated clock's start: 2026-11-06T21:20:00Z.
const NOW: u64 = 1_794_000_000_000;

impl Sim {
    fn put(&self, path: &str, text: &str) {
        self.files.borrow_mut().insert(PathBuf::from(path), text.to_owned());
    }

    fn get(&self, path: &str) -> String {
        self.files.borrow()[Path::new(path)].clone()
    }

    fn log(&self) -> Vec<String> {
        self.files.borrow().get(Path::new("/w/.cells/log.jsonl")).map_or(Vec::new(), |t| t.lines().map(str::to_owned).collect())
    }
}

impl World for Sim {
    fn read(&self, path: &Path) -> Found {
        self.files.borrow().get(path).map_or(Found::Missing, |t| Found::Text(t.clone()))
    }

    fn list(&self, root: &Path) -> Vec<PathBuf> {
        self.files.borrow().keys().filter(|p| p.starts_with(root) && table_file(p)).cloned().collect()
    }

    fn replace(&self, path: &Path, against: &str, text: &str) -> Replaced {
        let mut files = self.files.borrow_mut();
        match files.get(path) {
            Some(now) if hash(now) == against => {
                files.insert(path.to_path_buf(), text.to_owned());
                Replaced::Done
            }
            _ => Replaced::Moved,
        }
    }

    fn append(&self, path: &Path, line: &str) -> Result<(), String> {
        self.files.borrow_mut().entry(path.to_path_buf()).or_default().push_str(&format!("{line}\n"));
        Ok(())
    }

    fn keep(&self, path: &Path, text: &str) -> Result<(), String> {
        self.files.borrow_mut().entry(path.to_path_buf()).or_insert_with(|| text.to_owned());
        Ok(())
    }

    fn lock(&mut self, root: &Path) -> Result<(), String> {
        self.locked.push(root.to_path_buf());
        Ok(())
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
        PathBuf::from("/home/someone")
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

const Q4: &str = "/w/q4/money.cells";
const QUARTERS: &str = "/w/quarters.cells";
const ACCOUNTS: &str = "/books/accounts.cells";
const MONEY: &str = "---\ntype Kind = income|cost\ncost: EUR = CASE WHEN kind = 'cost' THEN amount ELSE 0 END\n---\nkind:Kind,amount:EUR,cost\nincome,4200.00,0.00=\ncost,180.00,180.00=\n";
const INDEX: &str = "---\nimport q4 from \"./q4/money.cells\"\n---\nquarter:text!,costs:EUR\nq4,180.00=SUM(q4.cost)\n";

fn served(sim: Sim, root: &str) -> Daemon<Sim> {
    let (back, inbox) = channel();
    let mut d = Daemon::new(sim, back);
    d.world_mut().inbox = Some(inbox);
    assert_eq!(d.serve(Path::new(root)).unwrap(), PathBuf::from(root));
    d
}

fn daemon() -> Daemon<Sim> {
    let sim = Sim::default();
    sim.put(Q4, MONEY);
    sim.put(QUARTERS, INDEX);
    served(sim, "/w")
}

fn heard(d: &mut Daemon<Sim>, paths: &[&str]) {
    d.hear(&paths.iter().map(PathBuf::from).collect());
}

#[test]
fn a_save_stores_what_it_reaches_in_every_file() {
    let mut d = daemon();
    assert!(d.world().log().is_empty(), "starting writes nothing");
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    heard(&mut d, &[Q4]);
    assert_eq!(d.world().get(Q4), MONEY.replace("cost,180.00,180.00=", "cost,200.00,200.00="));
    assert_eq!(d.world().get(QUARTERS), INDEX.replace("180.00=SUM", "200.00=SUM"));
    let log = d.world().log();
    let kinds: Vec<&str> = log.iter().map(|l| l.split("\"kind\":\"").nth(1).unwrap().split('"').next().unwrap()).collect();
    assert_eq!(kinds, ["external", "store", "store"]);
    assert!(log[0].starts_with("{\"n\":1,\"prev\":\"\","), "{}", log[0]);
    assert!(log[1].starts_with(&format!("{{\"n\":2,\"prev\":\"{}\",", hash(&log[0]))), "each line links the one before");
    assert!(
        log[0].contains("\"edited\":{\"cells\":[{\"row\":2,\"column\":\"amount\",\"was\":\"180.00\",\"now\":\"200.00\"}]}"),
        "{}",
        log[0]
    );
    assert!(log[2].contains("\"path\":\"quarters.cells\",\"cause\":{\"n\":1},\"via\":[\"q4/money.cells\"]"), "{}", log[2]);
    assert!(
        log[2].contains("\"stored\":[{\"row\":1,\"key\":\"q4\",\"column\":\"costs\",\"was\":\"180.00\",\"now\":\"200.00\"}]"),
        "{}",
        log[2]
    );
}

#[test]
fn a_change_is_said_as_a_span_and_what_it_stored_in_it() {
    let mut d = daemon();
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    let ((), said) = caught(|| heard(&mut d, &[Q4]));
    assert_eq!(said[..2], ["heard    paths=q4/money.cells", "change   q4/money.cells what=changed"], "{said:#?}");
    assert!(said.contains(&"  edited   cells=row 2 amount 180.00 → 200.00".to_owned()), "{said:#?}");
    assert!(said.contains(&"  stored   quarters.cells via=q4/money.cells cells=costs row 1 180.00 → 200.00".to_owned()), "{said:#?}");
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
fn a_file_changed_since_it_was_read_is_never_written_over() {
    let mut d = daemon();
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    // someone saves quarters by hand before the daemon hears q4
    let typed = INDEX.replace("q4,", "fourth,");
    d.world().put(QUARTERS, &typed);
    heard(&mut d, &[Q4]);
    assert_eq!(d.world().get(QUARTERS), typed, "their save kept");
    assert!(d.world().log().last().unwrap().contains("\"kind\":\"moved\",\"path\":\"quarters.cells\""));
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
fn an_import_outside_the_folder_is_read_and_watched_never_written() {
    let sim = Sim::default();
    sim.put(ACCOUNTS, "account:text!,budget:EUR\nrent,900.00\n");
    sim.put(
        "/w/plan.cells",
        "---\nimport accounts from \"../books/accounts.cells\"\n---\nwhat:text!,budget:EUR\nrent,900.00=SUM(accounts.budget)\n",
    );
    let mut d = served(sim, "/w");
    assert!(d.world().watched.contains(&(PathBuf::from("/books"), false)), "{:?}", d.world().watched);
    d.world().put(ACCOUNTS, "account:text!,budget:EUR\nrent,950.00\n");
    heard(&mut d, &[ACCOUNTS]);
    assert!(d.world().get("/w/plan.cells").contains("rent,950.00=SUM"));
}

#[test]
fn what_is_not_a_table_or_is_hidden_is_not_read() {
    let mut d = daemon();
    d.world().put("/w/.cells/log.jsonl", "{}\n");
    d.world().put("/w/.git/x.cells", "a\n1\n");
    d.world().put("/w/notes.txt", "hi\n");
    heard(&mut d, &["/w/.cells/log.jsonl", "/w/.git/x.cells", "/w/notes.txt"]);
    assert!(d.folder().text("/w/.git/x.cells").is_none());
}

#[test]
fn a_file_added_then_removed_is_logged_and_read_by_its_readers() {
    let mut d = daemon();
    d.world().put("/w/q1/money.cells", MONEY);
    heard(&mut d, &["/w/q1/money.cells"]);
    assert!(d.world().log()[0].contains("\"kind\":\"external\",\"path\":\"q1/money.cells\",\"was\":null"));
    d.world().files.borrow_mut().remove(Path::new(Q4));
    heard(&mut d, &[Q4]);
    assert!(d.world().log().last().unwrap().contains("\"kind\":\"external\",\"path\":\"q4/money.cells\",\"was\":\""));
    assert!(d.world().log().last().unwrap().contains("\"now\":null"));
}

#[test]
fn a_folder_inside_one_served_is_that_one_and_a_wide_one_is_refused() {
    let mut d = daemon();
    assert_eq!(d.serve(Path::new("/w/q4")).unwrap(), PathBuf::from("/w"));
    assert!(d.serve(Path::new("/")).unwrap_err().contains("too wide"));
    assert!(d.serve(Path::new("/home/someone")).unwrap_err().contains("too wide"));
}

#[test]
fn a_book_opened_from_a_quarter_first_is_one_book_once_its_index_is() {
    let sim = Sim::default();
    sim.put(Q4, MONEY);
    sim.put(QUARTERS, INDEX);
    let mut d = served(sim, "/w/q4");
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,190.00,180.00="));
    heard(&mut d, &[Q4]);
    assert_eq!(d.serve(Path::new("/w")).unwrap(), PathBuf::from("/w"), "the outer folder takes the inner in");
    assert_eq!(d.serve(Path::new("/w/q4")).unwrap(), PathBuf::from("/w/q4"), "the inner stays served, its history its own");
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
    heard(&mut d, &[Q4]);
    assert_eq!(d.world().get(QUARTERS), INDEX.replace("180.00=SUM", "200.00=SUM"), "the index follows its quarter");
    let inner = d.world().files.borrow().get(Path::new("/w/q4/.cells/log.jsonl")).cloned().unwrap();
    assert_eq!(inner.lines().filter(|l| l.contains("\"kind\":\"external\"")).count(), 2, "q4's saves logged where its history began");
    let FromEngine::History { chains } = ask(&mut d, "app", ToEngine::History { folder: "/w".into(), file: None, limit: 10 }) else {
        panic!()
    };
    let latest = &chains[0];
    assert_eq!(latest.root.folder, PathBuf::from("/w/q4"));
    assert!(
        latest.followed.iter().any(|f| f.event.folder == Path::new("/w") && f.event.line.entry.path() == "quarters.cells"),
        "one chain across both logs: {latest:#?}"
    );
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
    assert_eq!(said, ["allowed, as shown to its person q4/money.cells script=rates by=lsp"]);
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

fn ask(d: &mut Daemon<Sim>, client: &str, request: ToEngine) -> FromEngine {
    let (answer, answered) = channel();
    d.answer(Event::Ask { id: 1, client: client.into(), request, answer: Some(answer) });
    answered.try_recv().expect("answered")
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
        "/w/year.cells",
        "---\nimport quarters from \"./quarters.cells\"\n---\nyear:text!,costs:EUR\n2026,180.00=SUM(quarters.costs)\n",
    );
    let mut d = served(sim, "/w");
    d.world().put(Q4, &MONEY.replace("cost,180.00,180.00=", EDITED));
    heard(&mut d, &[Q4]);
    let FromEngine::History { chains } = ask(&mut d, "app", ToEngine::History { folder: "/w".into(), file: None, limit: 10 }) else {
        panic!()
    };
    let [chain] = &chains[..] else { panic!("one chain: {chains:#?}") };
    assert_eq!(chain.root.line.entry.path(), "q4/money.cells");
    let followed: Vec<(&str, Vec<String>, Option<usize>)> = chain
        .followed
        .iter()
        .map(|f| match &f.event.line.entry {
            Logged::Store { path, via, .. } => (path.as_str(), via.clone(), f.under),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        followed,
        [
            ("q4/money.cells", vec!["q4/money.cells".to_owned()], None),
            ("quarters.cells", vec!["q4/money.cells".to_owned()], None),
            ("year.cells", vec!["quarters.cells".to_owned()], Some(1))
        ],
        "the edit's own write and the quarters under the edit, the year under the quarters"
    );
    let raw = serde_json::to_string(&chains).unwrap();
    assert_eq!(serde_json::from_str::<Vec<crate::history::Chain>>(&raw).unwrap(), chains, "a chain crosses the wire whole");
    let FromEngine::History { chains } =
        ask(&mut d, "app", ToEngine::History { folder: "/w".into(), file: Some("/w/year.cells".into()), limit: 10 })
    else {
        panic!()
    };
    assert_eq!(chains.len(), 1, "a file's chains are those reaching it");
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
    let roots: Vec<u64> = values.iter().map(|v| v.root.1).collect();
    let log = d.world().log();
    for n in roots {
        assert!(log[n as usize - 1].contains("\"kind\":\"external\",\"path\":\"q4/money.cells\""), "each value's root is the q4 edit");
    }
}

#[test]
fn a_file_returned_to_a_version_is_a_change_of_its_own() {
    let mut d = daemon();
    let first = hash(MONEY);
    assert_eq!(d.world().get(&format!("/w/.cells/versions/{first}")), MONEY, "every version seen is kept");
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

const TRIP: &str = "/w/trip.cells";

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
    served(sim, "/w")
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
    assert!(log[0].contains("\"kind\":\"run\",\"path\":\"trip.cells\",\"script\":\"rates\",\"run\":2,\"outcome\":\"ok\",\"added\":0,\"changed\":1,\"by\":\"its schedule\""), "{}", log[0]);
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
    sim.put("/w/notes.cells", "note:text\nhello\n");
    sim.put(TRIP, &trip("when ./notes.cells changes"));
    sim.scripts.borrow_mut().insert("rates".into(), Ok(vec![r#"{"date":"2026-11-03","rate":0.95}"#.into()]));
    let mut d = served(sim, "/w");
    allow_all(&d, TRIP);
    d.tick();
    assert!(d.world().started.borrow().is_empty(), "nothing changed yet");
    d.world().put("/w/notes.cells", "note:text\nbye\n");
    heard(&mut d, &["/w/notes.cells"]);
    d.tick();
    pump(&mut d);
    assert_eq!(*d.world().started.borrow(), ["rates"]);
    assert!(d.world().log().iter().any(|l| l.contains("\"by\":\"a change to notes.cells\"")), "{:#?}", d.world().log());
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
    let root = chains.iter().find(|c| (c.root.folder.clone(), c.root.line.n) == m.root).expect("its chain");
    assert_eq!(root.root.line.entry.path(), "q4/money.cells", "the save in q4 that reached it");
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
    assert!(q4.contains("id:id!,kind:Kind,amount:EUR,cost\n"), "{q4}");
    assert!(q4.contains(&format!("{},cost,200.00,200.00=\n", assigned[1])), "the cell set, its computed cell stored: {q4}");
    assert_eq!(d.world().get(QUARTERS), INDEX.replace("180.00=SUM", "200.00=SUM"), "what it reached follows");
    let log = d.world().log();
    assert!(log[0].contains("\"kind\":\"edit\",\"path\":\"q4/money.cells\",\"intent\":\"k7m2q9xa\",\"by\":\"Cells Dev\""), "{}", log[0]);
    assert!(
        log.iter().any(|l| l.contains("\"path\":\"quarters.cells\",\"cause\":{\"n\":1}")),
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
    assert!(q4.contains(",income,4300.00,0.00=\n") && q4.contains(",cost,200.00,200.00=\n"), "both kept: {q4}");
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
fn a_version_on_disk_not_yet_heard_is_heard_before_an_intent_applies() {
    let mut d = daemon();
    // a save lands on disk; the engine has not heard it when the intent arrives
    d.world().put(Q4, &MONEY.replace("income,4200.00,0.00=", "income,4300.00,0.00="));
    let applied = intent(&mut d, INTENT, &hash(MONEY), vec![set_amount("180.00", "200.00")]);
    assert!(matches!(applied, FromEngine::Applied { .. }), "{applied:?}");
    let q4 = d.world().get(Q4);
    assert!(q4.contains(",income,4300.00,0.00=\n") && q4.contains(",cost,200.00,200.00=\n"), "nobody's write lost: {q4}");
    let log = d.world().log();
    let kinds: Vec<&str> = log.iter().map(|l| l.split("\"kind\":\"").nth(1).unwrap().split('"').next().unwrap()).collect();
    assert_eq!(&kinds[..2], ["external", "edit"], "the save it found, then the edit over it: {log:#?}");
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
