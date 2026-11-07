//! The daemon against a simulated world: files in memory, change events handed in by
//! the test, a clock that only moves when asked. Every run is the same run.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;

use crate::daemon::{Daemon, Event};
use crate::log::hash;
use crate::trace::Trace;
use crate::world::{Found, Replaced, World, table_file};

#[derive(Default)]
struct Sim {
    files: RefCell<BTreeMap<PathBuf, String>>,
    locked: Vec<PathBuf>,
    watched: Vec<(PathBuf, bool)>,
}

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

    fn last_line(&self, path: &Path) -> Option<String> {
        self.files.borrow().get(path)?.lines().last().map(str::to_owned)
    }

    fn lock(&mut self, root: &Path) -> Result<(), String> {
        self.locked.push(root.to_path_buf());
        Ok(())
    }

    fn watch(&mut self, dir: &Path, whole: bool) -> Result<(), String> {
        self.watched.push((dir.to_path_buf(), whole));
        Ok(())
    }

    fn home(&self) -> PathBuf {
        PathBuf::from("/home/someone")
    }

    fn now(&self) -> u64 {
        1_794_000_000_000
    }
}

const Q4: &str = "/w/q4/money.cells";
const QUARTERS: &str = "/w/quarters.cells";
const ACCOUNTS: &str = "/books/accounts.cells";
const MONEY: &str = "---\ntype Kind = income|cost\ncost: EUR = CASE WHEN kind = 'cost' THEN amount ELSE 0 END\n---\nkind:Kind,amount:EUR,cost\nincome,4200.00,0.00=\ncost,180.00,180.00=\n";
const INDEX: &str = "---\nimport q4 from \"./q4/money.cells\"\n---\nquarter:text!,costs:EUR\nq4,180.00=SUM(q4.cost)\n";

fn served(sim: Sim, root: &str) -> Daemon<Sim> {
    let mut d = Daemon::new(sim, Trace::kept());
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
    assert!(log[2].contains("\"path\":\"quarters.cells\",\"because\":\"q4/money.cells\""), "{}", log[2]);
    assert!(log[2].contains("\"stored\":[{\"row\":1,\"column\":\"costs\",\"was\":\"180.00\",\"now\":\"200.00\"}]"), "{}", log[2]);
    let said = d.trace().lines();
    assert!(said.iter().any(|l| l.contains("stored   quarters.cells for q4/money.cells: costs row 1 180.00 → 200.00")), "{said:#?}");
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
    assert!(d.world().log().last().unwrap().ends_with("\"now\":null}"));
}

#[test]
fn a_folder_inside_one_served_is_that_one_and_a_wide_one_is_refused() {
    let mut d = daemon();
    assert_eq!(d.serve(Path::new("/w/q4")).unwrap(), PathBuf::from("/w"));
    assert!(d.serve(Path::new("/")).unwrap_err().contains("too wide"));
    assert!(d.serve(Path::new("/home/someone")).unwrap_err().contains("too wide"));
    let mut e = served(Sim::default(), "/w/q4");
    assert!(e.serve(Path::new("/w")).unwrap_err().contains("holds /w/q4"));
}

#[test]
fn a_client_listening_is_told_every_file_whose_version_changed() {
    let mut d = daemon();
    let (tell, told) = channel();
    d.answer(Event::Listen { id: 1, client: "lsp".into(), tell });
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
    assert!(d.trace().lines().iter().any(|l| l.contains("joined   lsp (#1)")));
}
