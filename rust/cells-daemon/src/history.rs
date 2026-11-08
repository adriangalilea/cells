//! History (ENGINE.md, History): the folders' logs read as a person asks them. A chain is
//! a root (an outside edit, a person's restore) and every entry that follows it, in any
//! folder served; a cell's history is every value it held, each beside the entry that
//! wrote it and that entry's root. Every surface asks the engine, so every surface folds
//! and follows a chain the same way.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::log::{Line, Log, Logged};

/// An entry of a folder's log, with the folder it is in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub folder: PathBuf,
    #[serde(flatten)]
    pub line: Line,
}

impl Event {
    /// The file the entry is about, whole.
    pub fn file(&self) -> PathBuf {
        self.folder.join(self.line.entry.path())
    }
}

/// A root and what followed it, in the order it was logged, drawn as a tree: each entry
/// under the one it followed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chain {
    pub root: Event,
    pub followed: Vec<Followed>,
}

/// An entry of a chain and the one it is drawn under: the entry before it, in `followed`,
/// about the first file it read through (`via`), nothing for the root. The root's file
/// is the root's, its own write included, so what an edit reached hangs from the edit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Followed {
    #[serde(flatten)]
    pub event: Event,
    pub under: Option<usize>,
}

/// A value a cell held: the entry that wrote it, and that entry's root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    pub value: String,
    pub was: String,
    pub at: u64,
    pub folder: PathBuf,
    pub n: u64,
    /// The root of the chain the entry is in: its folder and number.
    pub root: (PathBuf, u64),
}

/// The chains of every log, newest root first: only those with an entry about a file
/// `wanted` says yes to, at most `limit`.
pub fn chains<'a>(logs: impl Iterator<Item = &'a Log>, wanted: impl Fn(&Path) -> bool, limit: usize) -> Vec<Chain> {
    let mut roots: BTreeMap<(PathBuf, u64), (Event, Vec<Event>)> = BTreeMap::new();
    let mut followers: Vec<((PathBuf, u64), Event)> = Vec::new();
    for log in logs {
        for line in log.lines() {
            let event = Event { folder: log.root.clone(), line: line.clone() };
            match line.entry.cause() {
                None => {
                    roots.insert((log.root.clone(), line.n), (event, Vec::new()));
                }
                Some(cause) => followers.push(((cause.folder.clone().unwrap_or_else(|| log.root.clone()), cause.n), event)),
            }
        }
    }
    // an entry whose root is gone (a log cut short by hand) has no chain to be drawn in,
    // and the log said it was broken when it was read
    for (root, event) in followers {
        if let Some((_, followed)) = roots.get_mut(&root) {
            followed.push(event);
        }
    }
    let mut out: Vec<Chain> = roots
        .into_values()
        .filter(|(root, followed)| wanted(&root.file()) || followed.iter().any(|e| wanted(&e.file())))
        .map(|(root, mut followed)| {
            // logged in the order it happened, folders served apart by their clocks
            followed.sort_by_key(|e| e.line.at);
            let followed = tree(&root, followed);
            Chain { root, followed }
        })
        .collect();
    out.sort_by_key(|c| std::cmp::Reverse((c.root.line.at, c.root.line.n)));
    out.truncate(limit);
    out
}

/// Each entry of a chain with the one it is drawn under (`Followed::under`).
fn tree(root: &Event, followed: Vec<Event>) -> Vec<Followed> {
    let mut out: Vec<Followed> = Vec::with_capacity(followed.len());
    for event in followed {
        let first = match &event.line.entry {
            Logged::Store { via, .. } => via.first().map(|v| event.folder.join(v)),
            _ => None,
        };
        let under = first.filter(|f| *f != root.file()).and_then(|f| out.iter().rposition(|o| o.event.file() == f));
        out.push(Followed { event, under });
    }
    out
}

/// Every value the cell at `row` (as the file numbers it now, from 1) of `column` held,
/// newest first, back to the entry that added its row. A row with a key is followed by its
/// key; one without, by its place, moved back over the rows each version added and
/// removed above it.
pub fn cell(log: &Log, file: &Path, row: usize, key: Option<&str>, column: &str) -> Vec<Held> {
    let path = log.shown(file);
    let lines: Vec<&Line> = log.lines().iter().filter(|l| l.entry.path() == path).collect();
    let root_of = |l: &Line| match l.entry.cause() {
        Some(c) => (c.folder.clone().unwrap_or_else(|| log.root.clone()), c.n),
        None => (log.root.clone(), l.n),
    };
    let mut out = Vec::new();
    let mut row = row;
    for l in lines.into_iter().rev() {
        let (cells, added, removed): (&[_], &[usize], &[usize]) = match &l.entry {
            Logged::Store { stored, .. } => (stored, &[], &[]),
            Logged::External { edited: Some(e), .. } | Logged::Restore { edited: Some(e), .. } | Logged::Run { edited: Some(e), .. } => {
                (&e.cells, &e.rows_added, &e.rows_removed)
            }
            // a file new, removed, or a version either side of which did not read, has no
            // cells to follow through
            Logged::External { .. } | Logged::Restore { .. } | Logged::Run { .. } => break,
            Logged::Moved { .. } => continue,
        };
        let same =
            |c: &&crate::log::Changed| c.column == column && (key.is_some() && c.key.as_deref() == key || key.is_none() && c.row == row);
        if let Some(c) = cells.iter().find(same) {
            out.push(Held { value: c.now.clone(), was: c.was.clone(), at: l.at, folder: log.root.clone(), n: l.n, root: root_of(l) });
        }
        if key.is_none() {
            if added.contains(&row) {
                break;
            }
            row -= added.iter().filter(|&&a| a < row).count();
            for &r in removed {
                if r <= row {
                    row += 1;
                }
            }
        }
    }
    out
}
