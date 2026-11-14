//! History (ENGINE.md, History): the folders' logs read as a person asks them. A chain is
//! a root (an outside edit, a person's restore) and every entry that follows it, in any
//! folder served; a cell's history is every value it held, each beside the entry that
//! wrote it and that entry's root. Every surface asks the engine, so every surface folds
//! and follows a chain the same way.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::log::{Line, Log, Logged, Renamed};

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

/// A cell the versions after one changed, its row as the file numbers it now: what it held
/// at that version and holds now, and the entry that last wrote it and that entry's root,
/// so a client marks it and says where it came from with the history's own words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub row: usize,
    pub key: Option<String>,
    /// Nothing for a row added since: the whole row is new.
    pub column: Option<String>,
    /// Nothing for a row added since.
    pub was: Option<String>,
    pub now: String,
    pub at: u64,
    pub entry: (PathBuf, u64),
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

/// Every cell of `file` the versions after `version` changed, as `Mark`s: the rows each
/// version added and removed followed down to the rows the file has now, a cell changed
/// twice marked once with what it held at `version`, a cell changed back not marked.
/// `version` is one the log names, as what an entry made or what one was made over; nothing
/// when it names none (a version never logged, older than the log), since what came after
/// it is not known. `current` is the version the file holds now: since it, nothing.
pub fn since(log: &Log, file: &Path, version: &str, current: &str) -> Option<Vec<Mark>> {
    if version == current {
        return Some(Vec::new());
    }
    let path = log.shown(file);
    let lines: Vec<&Line> = log.lines().iter().filter(|l| l.entry.path() == path).collect();
    fn over(l: &Line) -> Option<&str> {
        match &l.entry {
            Logged::External { was, .. } => was.as_deref(),
            Logged::Restore { was, .. } | Logged::Run { was, .. } | Logged::Edit { was, .. } => Some(was.as_str()),
            Logged::Store { against, .. } => Some(against.as_str()),
            Logged::Moved { .. } => None,
        }
    }
    let start = lines
        .iter()
        .rposition(|l| l.entry.now() == Some(version))
        .map(|i| i + 1)
        .or_else(|| lines.iter().position(|l| over(l) == Some(version)))?;
    let root_of = |l: &Line| match l.entry.cause() {
        Some(c) => (c.folder.clone().unwrap_or_else(|| log.root.clone()), c.n),
        None => (log.root.clone(), l.n),
    };
    let mut marks: Vec<Mark> = Vec::new();
    for l in &lines[start..] {
        let (cells, added, removed, renamed): (&[crate::log::Changed], &[usize], &[usize], &[Renamed]) = match &l.entry {
            Logged::Store { stored, .. } => (stored, &[], &[], &[]),
            Logged::External { edited: Some(e), .. }
            | Logged::Restore { edited: Some(e), .. }
            | Logged::Run { edited: Some(e), .. }
            | Logged::Edit { edited: Some(e), .. } => (&e.cells, &e.rows_added, &e.rows_removed, &e.columns_renamed),
            // a version either side of which did not read says no cells: what it changed is
            // not known, so nothing before it is either
            Logged::External { .. } | Logged::Restore { .. } | Logged::Run { .. } | Logged::Edit { .. } => {
                marks.clear();
                continue;
            }
            Logged::Moved { .. } => continue,
        };
        let (entry, root) = ((log.root.clone(), l.n), root_of(l));
        for r in renamed {
            marks.iter_mut().filter(|m| m.column.as_deref() == Some(r.from.as_str())).for_each(|m| m.column = Some(r.to.clone()));
        }
        // removed rows, numbered as before the version; then added ones, as after it
        let mut gone: Vec<usize> = removed.to_vec();
        gone.sort_unstable_by(|a, b| b.cmp(a));
        for r in gone {
            marks.retain(|m| m.row != r);
            marks.iter_mut().filter(|m| m.row > r).for_each(|m| m.row -= 1);
        }
        let mut new: Vec<usize> = added.to_vec();
        new.sort_unstable();
        for a in new {
            marks.iter_mut().filter(|m| m.row >= a).for_each(|m| m.row += 1);
            marks.push(Mark {
                row: a,
                key: None,
                column: None,
                was: None,
                now: String::new(),
                at: l.at,
                entry: entry.clone(),
                root: root.clone(),
            });
        }
        for c in cells {
            if marks.iter().any(|m| m.row == c.row && m.column.is_none()) {
                continue;
            }
            match marks.iter_mut().find(|m| m.row == c.row && m.column.as_deref() == Some(c.column.as_str())) {
                Some(m) => {
                    m.now = c.now.clone();
                    m.at = l.at;
                    m.entry = entry.clone();
                    m.root = root.clone();
                }
                None => marks.push(Mark {
                    row: c.row,
                    key: c.key.clone(),
                    column: Some(c.column.clone()),
                    was: Some(c.was.clone()),
                    now: c.now.clone(),
                    at: l.at,
                    entry: entry.clone(),
                    root: root.clone(),
                }),
            }
        }
        marks.retain(|m| m.was.as_deref() != Some(m.now.as_str()));
    }
    marks.sort_by(|a, b| (a.row, &a.column).cmp(&(b.row, &b.column)));
    Some(marks)
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
    let mut column = column.to_owned();
    for l in lines.into_iter().rev() {
        let (cells, added, removed, renamed): (&[_], &[usize], &[usize], &[Renamed]) = match &l.entry {
            Logged::Store { stored, .. } => (stored, &[], &[], &[]),
            Logged::External { edited: Some(e), .. }
            | Logged::Restore { edited: Some(e), .. }
            | Logged::Run { edited: Some(e), .. }
            | Logged::Edit { edited: Some(e), .. } => (&e.cells, &e.rows_added, &e.rows_removed, &e.columns_renamed),
            // a file new, removed, or a version either side of which did not read, has no
            // cells to follow through
            Logged::External { .. } | Logged::Restore { .. } | Logged::Run { .. } | Logged::Edit { .. } => break,
            Logged::Moved { .. } => continue,
        };
        let same =
            |c: &&crate::log::Changed| c.column == column && (key.is_some() && c.key.as_deref() == key || key.is_none() && c.row == row);
        if let Some(c) = cells.iter().find(same) {
            out.push(Held { value: c.now.clone(), was: c.was.clone(), at: l.at, folder: log.root.clone(), n: l.n, root: root_of(l) });
        }
        // before a version that renamed the column, it had its old name
        if let Some(r) = renamed.iter().find(|r| r.to == column) {
            column = r.from.clone();
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
