//! History (ENGINE.md, History): the log read as a person asks it. A chain is a root (an
//! outside edit, a person's restore) and every entry that follows it; a cell's history is
//! every value it held, each beside the entry that wrote it and that entry's root. A file's
//! history is every line of its identity, wherever it was; a folder's, every line whose
//! path was under it when it was logged. Every surface asks the engine, so every surface
//! folds and follows a chain the same way.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::index::FileId;
use crate::log::{Line, Log, Logged, Renamed};

/// A root and what followed it, in the order it was logged, drawn as a tree: each entry
/// under the one it followed. An entry is known by its number (`Line::n`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chain {
    pub root: Line,
    pub followed: Vec<Followed>,
}

/// An entry of a chain and the one it is drawn under: the entry before it, in `followed`,
/// about the first file it read through (`via`), nothing for the root. The root's file
/// is the root's, its own write included, so what an edit reached hangs from the edit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Followed {
    #[serde(flatten)]
    pub event: Line,
    pub under: Option<usize>,
}

/// A value a cell held: the entry that wrote it (`n`), and that entry's root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    pub value: String,
    pub was: String,
    pub at: u64,
    pub n: u64,
    pub root: u64,
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
    pub entry: u64,
    pub root: u64,
}

/// What a history is asked of: one file's lines, every line whose path was under a folder,
/// both, or neither (every chain).
pub struct Asked<'a> {
    pub file: Option<&'a FileId>,
    pub under: Option<&'a Path>,
}

impl Asked<'_> {
    fn wants(&self, line: &Line) -> bool {
        self.file.is_none_or(|f| line.file == *f) && self.under.is_none_or(|u| Path::new(line.entry.path()).starts_with(u))
    }
}

/// The root of the chain `line` is in: its number.
fn root_of(line: &Line) -> u64 {
    line.entry.cause().map_or(line.n, |c| c.n)
}

/// The log's chains, newest root first: only those with an entry `asked` wants, at most
/// `limit`.
pub fn chains(log: &Log, asked: &Asked, limit: usize) -> Vec<Chain> {
    let mut roots: BTreeMap<u64, (Line, Vec<Line>)> = BTreeMap::new();
    for line in log.lines() {
        match line.entry.cause() {
            None => {
                roots.insert(line.n, (line.clone(), Vec::new()));
            }
            // an entry whose root is gone (a log cut short by hand) has no chain to be drawn
            // in, and the log said it was broken when it was read
            Some(cause) => {
                if let Some((_, followed)) = roots.get_mut(&cause.n) {
                    followed.push(line.clone());
                }
            }
        }
    }
    let mut out: Vec<Chain> = roots
        .into_values()
        .filter(|(root, followed)| asked.wants(root) || followed.iter().any(|e| asked.wants(e)))
        .map(|(root, followed)| {
            let followed = tree(&root, followed);
            Chain { root, followed }
        })
        .collect();
    out.sort_by_key(|c| std::cmp::Reverse((c.root.at, c.root.n)));
    out.truncate(limit);
    out
}

/// Each entry of a chain with the one it is drawn under (`Followed::under`).
fn tree(root: &Line, followed: Vec<Line>) -> Vec<Followed> {
    let mut out: Vec<Followed> = Vec::with_capacity(followed.len());
    for event in followed {
        let first = match &event.entry {
            Logged::Store { via, .. } => via.first().cloned(),
            _ => None,
        };
        let under = first.filter(|f| f != root.entry.path()).and_then(|f| out.iter().rposition(|o| o.event.entry.path() == f));
        out.push(Followed { event, under });
    }
    out
}

/// Every cell of the file `id` the versions after `version` changed, as `Mark`s: the rows
/// each version added and removed followed down to the rows the file has now, a cell
/// changed twice marked once with what it held at `version`, a cell changed back not
/// marked. `version` is one the log names, as what an entry made or what one was made
/// over; nothing when it names none (a version never logged, older than the log), since
/// what came after it is not known. `current` is the version the file holds now: since it,
/// nothing.
pub fn since(log: &Log, id: &FileId, version: &str, current: &str) -> Option<Vec<Mark>> {
    if version == current {
        return Some(Vec::new());
    }
    let lines: Vec<&Line> = log.of(id).collect();
    fn over(l: &Line) -> Option<&str> {
        match &l.entry {
            Logged::External { was, .. } => was.as_deref(),
            Logged::Restore { was, .. } | Logged::Run { was, .. } | Logged::Edit { was, .. } => Some(was.as_str()),
            Logged::Store { against, .. } => Some(against.as_str()),
            // a file moved holds the text it held: made over the version it is
            Logged::Relocated { now, .. } => Some(now.as_str()),
            Logged::Moved { .. } => None,
        }
    }
    let start = lines
        .iter()
        .rposition(|l| l.entry.now() == Some(version))
        .map(|i| i + 1)
        .or_else(|| lines.iter().position(|l| over(l) == Some(version)))?;
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
            // a move changes no cell; a write not made, none either
            Logged::Relocated { .. } | Logged::Moved { .. } => continue,
        };
        let (entry, root) = (l.n, root_of(l));
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
            marks.push(Mark { row: a, key: None, column: None, was: None, now: String::new(), at: l.at, entry, root });
        }
        for c in cells {
            if marks.iter().any(|m| m.row == c.row && m.column.is_none()) {
                continue;
            }
            match marks.iter_mut().find(|m| m.row == c.row && m.column.as_deref() == Some(c.column.as_str())) {
                Some(m) => {
                    m.now = c.now.clone();
                    m.at = l.at;
                    m.entry = entry;
                    m.root = root;
                }
                None => marks.push(Mark {
                    row: c.row,
                    key: c.key.clone(),
                    column: Some(c.column.clone()),
                    was: Some(c.was.clone()),
                    now: c.now.clone(),
                    at: l.at,
                    entry,
                    root,
                }),
            }
        }
        marks.retain(|m| m.was.as_deref() != Some(m.now.as_str()));
    }
    marks.sort_by(|a, b| (a.row, &a.column).cmp(&(b.row, &b.column)));
    Some(marks)
}

/// Every value the cell at `row` (as the file numbers it now, from 1) of `column` of the
/// file `id` held, newest first, back to the entry that added its row. A row with a key is
/// followed by its key; one without, by its place, moved back over the rows each version
/// added and removed above it.
pub fn cell(log: &Log, id: &FileId, row: usize, key: Option<&str>, column: &str) -> Vec<Held> {
    let lines: Vec<&Line> = log.of(id).collect();
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
            // a move changes no cell; a write not made, none either
            Logged::Relocated { .. } | Logged::Moved { .. } => continue,
        };
        let same =
            |c: &&crate::log::Changed| c.column == column && (key.is_some() && c.key.as_deref() == key || key.is_none() && c.row == row);
        if let Some(c) = cells.iter().find(same) {
            out.push(Held { value: c.now.clone(), was: c.was.clone(), at: l.at, n: l.n, root: root_of(l) });
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
