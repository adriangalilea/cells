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
    let mut d = Daemon::new(sim);
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
    let mut e = served(Sim::default(), "/w/q4");
    assert!(e.serve(Path::new("/w")).unwrap_err().contains("holds /w/q4"));
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
