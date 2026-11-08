//! A folder of files as the engine holds it: each file's text and doc, the files each one
//! reads through its imports, and the version of each the engine last knew. A change goes
//! in; out come the files it read again, the writes it reaches and the files the folder
//! still has to be handed. Nothing here reaches a disk, a clock or a thread: a host (the
//! daemon, `cells serve`; an app; a simulation) reads and writes the world and hands each
//! change in, so what the folder decides replays exactly from the changes it was handed
//! (ENGINE.md, Proving it).
//!
//! A change reaches every file that reads it (SPEC.md, Writing). The changed file and each
//! file whose imports reach it are read again, in the order their reads need, and each is
//! written as a write after its version before leaves it (`Doc::stored_since`): a computed
//! cell whose formula now gives another value than it gave is stored, and a value typed
//! over a formula, which gives what it gave, is left stale for a person to resolve. A file
//! the folder had no version of (the first time it is seen) has nothing to compare, so
//! nothing is stored in it.
//!
//! ```
//! use cells::folder::{Change, Folder};
//! let mut folder = Folder::default();
//! folder.apply(Change::Seen { path: "/w/q4.cells".into(), text: "kind:text,amount:EUR\ncost,180.00\n".into() });
//! folder.apply(Change::Seen {
//!     path: "/w/quarters.cells".into(),
//!     text: "---\nimport q4 from \"./q4.cells\"\n---\nquarter:text!,costs:EUR\nq4,180.00=SUM(q4.amount)\n".into(),
//! });
//! let out = folder.apply(Change::Changed { path: "/w/q4.cells".into(), text: "kind:text,amount:EUR\ncost,200.00\n".into() });
//! assert_eq!(out.writes[0].path, "/w/quarters.cells");
//! assert_eq!(out.writes[0].text, "---\nimport q4 from \"./q4.cells\"\n---\nquarter:text!,costs:EUR\nq4,200.00=SUM(q4.amount)\n");
//! ```

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::column::Kind;
use crate::doc::{Doc, read_with};
use crate::files::Files;

/// What a host saw of a file, by the path the folder keys it with (absolute, as the host
/// resolves it; a URL for a fetched file).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// The file as it was found: when the host began watching, or when the folder asked
    /// for it (`Outcome::needs`). Its history starts here, so nothing is stored, in it or
    /// in a file that reads it.
    Seen { path: String, text: String },
    /// The file's text now, written by anyone but the engine (the same text as held
    /// changes nothing, so the engine's own write heard again is nothing).
    Changed { path: String, text: String },
    /// A write the folder decided (`Outcome::writes`), made: the file's version from now
    /// on, read again alone. It stores what the files reading it already read, so none of
    /// them is read again.
    Written { path: String, text: String },
    /// The file removed.
    Gone { path: String },
}

/// What a change led to, each list in the order it happened.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Each file read again.
    pub read: Vec<Read>,
    /// What the change reaches, a whole file each.
    pub writes: Vec<Write>,
    /// Files a read named that the folder does not hold: the host hands each in as
    /// `Seen` when it can read it, and watches where it is either way.
    pub needs: Vec<String>,
}

/// A file read again, and what its read found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    pub path: String,
    /// Why the reader refused it whole (bytes that are not UTF-8); nothing when it read.
    pub refused: Option<String>,
    pub problems: usize,
    /// Of the problems, the computed cells left stale: what only a person resolves.
    pub stale: usize,
}

/// A file to write: its whole text, made from the version whose SHA-256 is `against`, so
/// a host writes it only over that version, never over a change it has not handed in yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Write {
    pub path: String,
    pub against: String,
    pub text: String,
    /// The file whose change reached this one: itself when a write after a hand edit
    /// stores what the edit reached.
    pub because: String,
    pub stored: Vec<Stored>,
}

/// One computed cell a write stores: its row as a table numbers it (from 1), its column,
/// and what the file stored before its `=` and stores now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
    pub row: usize,
    pub column: String,
    pub was: String,
    pub now: String,
}

/// What the folder knows of a file beside its text: the doc it read, nothing when the
/// reader refused it, and every file its imports reached, held or not.
#[derive(Clone)]
struct Known {
    doc: Option<Doc>,
    reads: Vec<String>,
}

#[derive(Clone, Default)]
pub struct Folder {
    /// Every file's text, by path: what imports are followed through.
    texts: BTreeMap<String, String>,
    known: BTreeMap<String, Known>,
}

impl Folder {
    /// The text held of `path`.
    pub fn text(&self, path: &str) -> Option<&str> {
        self.texts.get(path).map(String::as_str)
    }

    /// The doc read of `path`: nothing when the folder does not hold it or the reader
    /// refused it.
    pub fn doc(&self, path: &str) -> Option<&Doc> {
        self.known.get(path)?.doc.as_ref()
    }

    /// Every file held, by path.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.texts.keys().map(String::as_str)
    }

    /// Every file `path`'s imports reach, imports of imports included.
    pub fn reads(&self, path: &str) -> &[String] {
        self.known.get(path).map_or(&[], |k| &k.reads)
    }

    pub fn apply(&mut self, change: Change) -> Outcome {
        let mut out = Outcome::default();
        match change {
            Change::Seen { path, text } => {
                self.texts.insert(path.clone(), text);
                for p in self.reached_by(&path) {
                    self.read(&p, None, &mut out);
                }
            }
            Change::Changed { path, text } => {
                if self.texts.get(&path) == Some(&text) {
                    return out;
                }
                let first = !self.texts.contains_key(&path);
                self.texts.insert(path.clone(), text);
                for p in self.reached_by(&path) {
                    // a file new to the folder has no version before to store against
                    let cause = (!(first && p == path)).then_some(path.as_str());
                    self.read(&p, cause, &mut out);
                }
            }
            Change::Written { path, text } => {
                assert!(self.texts.contains_key(&path), "{path} written, never held: the folder writes only files it read");
                self.texts.insert(path.clone(), text);
                self.read(&path, None, &mut out);
            }
            Change::Gone { path } => {
                let reached = self.reached_by(&path);
                self.texts.remove(&path);
                self.known.remove(&path);
                for p in reached.iter().filter(|p| **p != path) {
                    self.read(p, Some(&path), &mut out);
                }
            }
        }
        out
    }

    /// `path`, when held, and every file whose reads reach it, in the order their reads
    /// need: a file before every file that reads it. A file's reads hold the reads of
    /// every file it reads (and imports never cycle), so the fewer of these a file reads,
    /// the earlier it comes; ties by path, so the order never depends on a map's.
    fn reached_by(&self, path: &str) -> Vec<String> {
        let mut reached: Vec<String> =
            self.known.iter().filter(|(_, k)| k.reads.iter().any(|r| r == path)).map(|(p, _)| p.clone()).collect();
        if self.texts.contains_key(path) {
            reached.push(path.to_owned());
        }
        let depth = |p: &String| self.reads(p).iter().filter(|r| reached.contains(r)).count();
        let mut ordered: Vec<(usize, String)> = reached.iter().map(|p| (depth(p), p.clone())).collect();
        ordered.sort();
        ordered.into_iter().map(|(_, p)| p).collect()
    }

    /// `path` read again from the texts held, replacing the version the folder knew; with
    /// a `cause`, written as a write after that version leaves it.
    fn read(&mut self, path: &str, cause: Option<&str>, out: &mut Outcome) {
        let text = self.texts[path].clone();
        let mut files = Files::new(&self.texts, path);
        let read = read_with(text.as_bytes().to_vec(), &mut files, 1);
        let mut reads = files.reached;
        reads.sort();
        reads.dedup();
        for r in &reads {
            if !self.texts.contains_key(r) && !out.needs.contains(r) {
                out.needs.push(r.clone());
            }
        }
        let before = self.known.get(path).and_then(|k| k.doc.as_ref());
        match &read {
            Ok(doc) => {
                if let (Some(cause), Some(before)) = (cause, before) {
                    let mut after = doc.clone();
                    after.stored_since(before);
                    let written = after.write();
                    if written != text {
                        let against = crate::sha256::hex(text.as_bytes());
                        let stored = stored(doc, &after);
                        out.writes.push(Write { path: path.to_owned(), against, text: written, because: cause.to_owned(), stored });
                    }
                }
                let stale = doc.problems().iter().filter(|p| p.kind() == Kind::Stale).count();
                out.read.push(Read { path: path.to_owned(), refused: None, problems: doc.problems().len(), stale });
            }
            Err(e) => out.read.push(Read { path: path.to_owned(), refused: Some(e.why.clone()), problems: 0, stale: 0 }),
        }
        self.known.insert(path.to_owned(), Known { doc: read.ok(), reads });
    }
}

/// Every computed cell `after` stores another value in than `now` does.
fn stored(now: &Doc, after: &Doc) -> Vec<Stored> {
    let left = |cell: &str| crate::split_cell(cell).map_or(cell, |at| &cell[..at]).to_owned();
    let mut out = Vec::new();
    for (k, c) in now.columns().iter().enumerate() {
        // a column computes nothing when every cell is written
        if c.origin.is_empty() {
            continue;
        }
        for r in 0..now.rows() {
            let (was, is) = (now.text(r, k), after.text(r, k));
            if was != is {
                out.push(Stored { row: r + 1, column: c.name.clone(), was: left(&was), now: left(&is) });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const Q4: &str = "/w/q4/money.cells";
    const QUARTERS: &str = "/w/quarters.cells";
    const MONEY: &str = "---\ntype Kind = income|cost\ncost: EUR = CASE WHEN kind = 'cost' THEN amount ELSE 0 END\n---\nkind:Kind,amount:EUR,cost\nincome,4200.00,0.00=\ncost,180.00,180.00=\n";
    const INDEX: &str = "---\nimport q4 from \"./q4/money.cells\"\n---\nquarter:text!,costs:EUR\nq4,180.00=SUM(q4.cost)\n";

    fn seen(f: &mut Folder, path: &str, text: &str) -> Outcome {
        f.apply(Change::Seen { path: path.into(), text: text.into() })
    }

    fn changed(f: &mut Folder, path: &str, text: &str) -> Outcome {
        f.apply(Change::Changed { path: path.into(), text: text.into() })
    }

    fn folder() -> Folder {
        let mut f = Folder::default();
        seen(&mut f, Q4, MONEY);
        assert!(seen(&mut f, QUARTERS, INDEX).writes.is_empty(), "a file seen is never written");
        f
    }

    #[test]
    fn a_change_reaches_the_file_that_reads_it() {
        let mut f = folder();
        let edited = MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00=");
        let out = changed(&mut f, Q4, &edited);
        assert_eq!(out.read.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), [Q4, QUARTERS], "the file read first");
        let [own, index] = &out.writes[..] else { panic!("two writes: {:?}", out.writes) };
        assert_eq!((own.path.as_str(), own.because.as_str()), (Q4, Q4));
        assert_eq!(own.stored, [Stored { row: 2, column: "cost".into(), was: "180.00".into(), now: "200.00".into() }]);
        assert_eq!(index.path, QUARTERS);
        assert_eq!(index.because, Q4);
        assert_eq!(index.text, INDEX.replace("180.00=SUM", "200.00=SUM"));
        assert_eq!(index.against, crate::sha256::hex(INDEX.as_bytes()));
    }

    #[test]
    fn a_write_made_is_read_alone_and_heard_again_is_nothing() {
        let mut f = folder();
        let out = changed(&mut f, Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
        for w in &out.writes {
            let made = f.apply(Change::Written { path: w.path.clone(), text: w.text.clone() });
            assert_eq!(made.read.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), [w.path.as_str()], "read alone");
            assert_eq!((made.read[0].stale, made.writes.len()), (0, 0));
            assert!(changed(&mut f, &w.path, &w.text).read.is_empty(), "{} heard again", w.path);
        }
        // were the write heard as anyone's edit instead, it would store nothing more either
        let mut g = folder();
        let out = changed(&mut g, Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
        for w in &out.writes {
            assert!(changed(&mut g, &w.path, &w.text).writes.is_empty(), "{} written again", w.path);
        }
    }

    #[test]
    fn a_value_typed_over_its_formula_is_left_for_a_person() {
        let mut f = folder();
        let out = changed(&mut f, Q4, &MONEY.replace("cost,180.00,180.00=", "cost,180.00,999.00="));
        assert!(out.writes.is_empty(), "{:?}", out.writes);
        assert_eq!(out.read[0].stale, 1);
    }

    #[test]
    fn a_file_seen_late_is_a_baseline_for_its_readers() {
        let mut f = Folder::default();
        let out = seen(&mut f, QUARTERS, INDEX);
        assert_eq!(out.needs, [Q4]);
        let out = seen(&mut f, Q4, MONEY);
        assert_eq!(out.read.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), [Q4, QUARTERS]);
        assert!(out.writes.is_empty());
        let out = changed(&mut f, Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
        assert_eq!(out.writes.iter().map(|w| w.path.as_str()).collect::<Vec<_>>(), [Q4, QUARTERS]);
    }

    #[test]
    fn a_chain_is_written_in_the_order_its_reads_need() {
        let year = "/w/year.cells";
        let mut f = folder();
        seen(&mut f, year, "---\nimport quarters from \"./quarters.cells\"\n---\nyear:text!,costs:EUR\n2026,180.00=SUM(quarters.costs)\n");
        let out = changed(&mut f, Q4, &MONEY.replace("cost,180.00,180.00=", "cost,200.00,180.00="));
        assert_eq!(out.writes.iter().map(|w| w.path.as_str()).collect::<Vec<_>>(), [Q4, QUARTERS, year]);
        assert!(out.writes[2].text.contains("2026,200.00=SUM"));
    }

    #[test]
    fn a_file_gone_is_read_again_by_its_readers() {
        let mut f = folder();
        let out = f.apply(Change::Gone { path: Q4.into() });
        assert_eq!(out.read.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), [QUARTERS]);
        assert_eq!(out.needs, [Q4]);
        assert!(f.text(Q4).is_none());
    }
}
