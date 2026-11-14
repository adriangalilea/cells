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
    /// What a `Changed` file's new version changed in it, beside the version before:
    /// nothing for a file new to the folder, or one either version of failed to read.
    pub edited: Option<Edited>,
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
    /// The files of this change this one reads directly, each changed by it or written
    /// before it: what a chain draws as its parents. The changed file alone for its own
    /// write, storing what its edit reached.
    pub via: Vec<String>,
    /// Each computed cell stored, its values as the file stores them before its `=`.
    pub stored: Vec<Cell>,
}

/// One cell of a table, changed: its row as the table numbers it (from 1), the row's key
/// when the table has one (what names it across rows inserted above), its column, and
/// its text before and after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub row: usize,
    pub key: Option<String>,
    pub column: String,
    pub was: String,
    pub now: String,
}

/// What a version changed in a file beside the version before it. Rows are matched by
/// the table's key when it has one; without one, the lines the two versions share at
/// their start and their end are the same rows, and between them rows pair by place when
/// both versions hold as many, else are removed and added. Columns are matched as
/// `same_columns` says: by name, else renamed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edited {
    /// Each cell whose text changed, of a row and a column both versions hold, the column
    /// named as the new version names it.
    pub cells: Vec<Cell>,
    /// Rows the new version holds and the one before did not, numbered as the new one.
    pub rows_added: Vec<usize>,
    /// Rows the version before held and the new one does not, numbered as before.
    pub rows_removed: Vec<usize>,
    pub columns_added: Vec<String>,
    pub columns_removed: Vec<String>,
    /// Each column both versions hold under another name: its name before, its name now.
    pub columns_renamed: Vec<(String, String)>,
}

impl Edited {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
            && self.rows_added.is_empty()
            && self.rows_removed.is_empty()
            && self.columns_added.is_empty()
            && self.columns_removed.is_empty()
            && self.columns_renamed.is_empty()
    }
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

    /// `text` read as a version of `path`, its imports followed through the files held: a
    /// version not held yet (what a run would write) read as the folder would read it.
    pub fn read_as(&self, path: &str, text: &str) -> Result<Doc, String> {
        let mut files = Files::new(&self.texts, path);
        read_with(text.as_bytes().to_vec(), &mut files, 1).map_err(|e| alloc::format!("{e}"))
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
                let mut changed = alloc::vec![path.clone()];
                for p in self.reached_by(&path) {
                    if p == path {
                        // a file new to the folder has no version before to store against
                        let via = (!first).then(|| alloc::vec![path.clone()]);
                        self.read(&p, via, &mut out);
                    } else {
                        let via = self.via(&p, &changed);
                        if self.read(&p, Some(via), &mut out) {
                            changed.push(p);
                        }
                    }
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
                let mut changed = alloc::vec![path.clone()];
                for p in reached.into_iter().filter(|p| *p != path) {
                    let via = self.via(&p, &changed);
                    if self.read(&p, Some(via), &mut out) {
                        changed.push(p);
                    }
                }
            }
        }
        out
    }

    /// Of the files `changed` by one change so far, those `path` reads directly: every one
    /// it reads but through another of them.
    fn via(&self, path: &str, changed: &[String]) -> Vec<String> {
        let reads = self.reads(path);
        let read: Vec<&String> = changed.iter().filter(|c| reads.contains(c)).collect();
        read.iter().filter(|c| !read.iter().any(|d| d != *c && self.reads(d).contains(*c))).map(|c| (*c).clone()).collect()
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
    /// `via`, written as a write after that version leaves it, and when `via` is the file
    /// itself, what its new version edited said. Whether it is written.
    fn read(&mut self, path: &str, via: Option<Vec<String>>, out: &mut Outcome) -> bool {
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
        let mut wrote = false;
        match &read {
            Ok(doc) => {
                if let (Some(via), Some(before)) = (via, before) {
                    if via.len() == 1 && via[0] == path {
                        out.edited = Some(edited(before, doc));
                    }
                    let mut after = doc.clone();
                    after.stored_since(before);
                    let written = after.write();
                    if written != text {
                        let against = crate::sha256::hex(text.as_bytes());
                        let stored = stored(doc, &after);
                        out.writes.push(Write { path: path.to_owned(), against, text: written, via, stored });
                        wrote = true;
                    }
                }
                let stale = doc.problems().iter().filter(|p| p.kind() == Kind::Stale).count();
                out.read.push(Read { path: path.to_owned(), refused: None, problems: doc.problems().len(), stale });
            }
            Err(e) => out.read.push(Read { path: path.to_owned(), refused: Some(e.why.clone()), problems: 0, stale: 0 }),
        }
        self.known.insert(path.to_owned(), Known { doc: read.ok(), reads });
        wrote
    }
}

/// Whether the folder at `root` is served whole, every file under it, or shallow, its own
/// files alone: the home folder (`home`) and the disk's root are shallow, so a file kept
/// there is followed without crawling the disk. Paths as the host writes them, absolute
/// and canonical, no trailing `/`. The one rule: the engine serves by it, and a host
/// lists a folder's files (an app's sidebar) and finds the books a file belongs to by it.
pub fn served_whole(root: &str, home: &str) -> bool {
    assert!(root.starts_with('/'), "a folder served is an absolute path, not {root:?}");
    root != home && root != "/"
}

/// The text of the key cell of `doc`'s `row` (from 0), when its table has a key.
pub fn key_of(doc: &Doc, row: usize) -> Option<String> {
    let k = doc.columns().iter().position(|c| c.ty.key)?;
    Some(doc.text(row, k).into_owned())
}

/// Every computed cell `after` stores another value in than `now` does.
fn stored(now: &Doc, after: &Doc) -> Vec<Cell> {
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
                out.push(Cell { row: r + 1, key: key_of(now, r), column: c.name.clone(), was: left(&was), now: left(&is) });
            }
        }
    }
    out
}

/// The most rows two versions may differ by for their rows to be matched one by one; past
/// it, the rows between the ones they share at their ends are compared by place, or, their
/// counts differing, removed and added whole.
const EDIT_LIMIT: usize = 1024;

/// The items `a` and `b` share, in order, as pairs of their places: Myers' shortest edit
/// script, in time and memory of the order of the differences squared plus the items, so
/// a large file with few rows changed costs what its rows do. Nothing past `limit`
/// differences.
fn shortest_edit(a: &[String], b: &[String], limit: usize) -> Option<Vec<(usize, usize)>> {
    let (n, m) = (a.len() as isize, b.len() as isize);
    // each round's furthest reach on every diagonal it tried, the diagonal k at k + d
    let mut rounds: Vec<Vec<isize>> = Vec::new();
    let mut last: Vec<isize> = alloc::vec![0];
    for d in 0..=(n + m).min(limit as isize) {
        let mut reach = alloc::vec![0isize; (2 * d + 1) as usize];
        let at = |v: &Vec<isize>, dd: isize, k: isize| v[(k + dd) as usize];
        for k in (-d..=d).step_by(2) {
            let mut x = if d == 0 {
                0
            } else if k == -d || (k != d && at(&last, d - 1, k - 1) < at(&last, d - 1, k + 1)) {
                at(&last, d - 1, k + 1)
            } else {
                at(&last, d - 1, k - 1) + 1
            };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            reach[(k + d) as usize] = x;
            if x >= n && y >= m {
                rounds.push(reach);
                return Some(walk_back(&rounds, a, b, n, m));
            }
        }
        rounds.push(reach.clone());
        last = reach;
    }
    None
}

/// The pairs a shortest edit script keeps, from its rounds' reaches, in order.
fn walk_back(rounds: &[Vec<isize>], a: &[String], b: &[String], n: isize, m: isize) -> Vec<(usize, usize)> {
    let mut kept = Vec::new();
    let (mut x, mut y) = (n, m);
    for d in (1..rounds.len() as isize).rev() {
        let prev = &rounds[(d - 1) as usize];
        let at = |k: isize| prev[(k + d - 1) as usize];
        let k = x - y;
        let down = k == -d || (k != d && at(k - 1) < at(k + 1));
        let pk = if down { k + 1 } else { k - 1 };
        let px = at(pk);
        let py = px - pk;
        // the snake after the move: items both share
        while x > px.max(if down { px } else { px + 1 }) && y > py.max(if down { py + 1 } else { py }) {
            x -= 1;
            y -= 1;
            kept.push((x as usize, y as usize));
        }
        (x, y) = (px, py);
    }
    // the first round's snake, from the start
    while x > 0 && y > 0 {
        debug_assert_eq!(a[(x - 1) as usize], b[(y - 1) as usize]);
        x -= 1;
        y -= 1;
        kept.push((x as usize, y as usize));
    }
    kept.reverse();
    kept
}

/// The columns `before` and `after` hold as one, as pairs of their places, in `before`'s
/// order. A file has no column ids, so a column is known by what it says: the same name is
/// the same column; of the columns left on each side, one whose type and formula are the
/// same and whose cells read the same, row for row, is the same column renamed, wherever
/// it now stands. A column renamed and retyped, or renamed with a cell changed in the same
/// version, shares only its place, and is one column removed and another added. Each
/// column left before is paired with the first one left after that is it, in header order,
/// so blank columns alike are paired in the order they stand.
pub fn same_columns(before: &Doc, after: &Doc) -> Vec<(usize, usize)> {
    let (cb, ca) = (before.columns(), after.columns());
    let by_name = |k: usize| ca.iter().position(|c| c.name == cb[k].name);
    let left_after: Vec<usize> = (0..ca.len()).filter(|&j| !cb.iter().any(|c| c.name == ca[j].name)).collect();
    let same = |k: usize, j: usize| {
        cb[k].ty == ca[j].ty
            && cb[k].formula == ca[j].formula
            && before.rows() == after.rows()
            && (0..before.rows()).all(|r| before.text(r, k) == after.text(r, j))
    };
    let mut taken: Vec<usize> = Vec::new();
    let mut out = Vec::new();
    for k in 0..cb.len() {
        let j = by_name(k).or_else(|| left_after.iter().copied().find(|&j| !taken.contains(&j) && same(k, j)));
        if let Some(j) = j {
            taken.push(j);
            out.push((k, j));
        }
    }
    out
}

/// What `after` changed beside `before` (`Edited`): rows matched by the table's key, else by
/// what their cells read, the rows both share at their ends set aside first, so a large file
/// with a few rows changed costs what reading its rows does.
pub fn edited(before: &Doc, after: &Doc) -> Edited {
    let mut out = Edited::default();
    let name = |d: &Doc, k: usize| d.columns()[k].name.clone();
    let shared = same_columns(before, after);
    out.columns_removed = (0..before.columns().len()).filter(|&k| !shared.iter().any(|&(b, _)| b == k)).map(|k| name(before, k)).collect();
    out.columns_added = (0..after.columns().len()).filter(|&j| !shared.iter().any(|&(_, a)| a == j)).map(|j| name(after, j)).collect();
    out.columns_renamed = shared
        .iter()
        .filter(|&&(k, j)| before.columns()[k].name != after.columns()[j].name)
        .map(|&(k, j)| (name(before, k), name(after, j)))
        .collect();

    let keyed = |d: &Doc| d.columns().iter().position(|c| c.ty.key);
    let pairs: Vec<(usize, usize)> = match (keyed(before), keyed(after)) {
        (Some(kb), Some(ka)) if shared.contains(&(kb, ka)) => {
            let mut at = BTreeMap::new();
            for r in 0..before.rows() {
                at.insert(before.text(r, kb).into_owned(), r);
            }
            let mut pairs = Vec::new();
            for r in 0..after.rows() {
                match at.remove(after.text(r, ka).as_ref()) {
                    Some(b) => pairs.push((b, r)),
                    None => out.rows_added.push(r + 1),
                }
            }
            out.rows_removed = at.into_values().map(|b| b + 1).collect();
            out.rows_removed.sort();
            pairs
        }
        _ => {
            // a row is the same row when its cells read the same, padding no part of them
            let (cb, ca): (Vec<usize>, Vec<usize>) = shared.iter().copied().unzip();
            let id =
                |d: &Doc, r: usize, cols: &[usize]| -> String { cols.iter().map(|&k| d.text(r, k)).collect::<Vec<_>>().join("\u{1f}") };
            let same = |b: usize, a: usize| before.line(b) == after.line(a) || id(before, b, &cb) == id(after, a, &ca);
            let (nb, na) = (before.rows(), after.rows());
            let mut head = 0;
            while head < nb.min(na) && same(head, head) {
                head += 1;
            }
            let mut tail = 0;
            while tail < (nb - head).min(na - head) && same(nb - 1 - tail, na - 1 - tail) {
                tail += 1;
            }
            let olds: Vec<String> = (head..nb - tail).map(|r| id(before, r, &cb)).collect();
            let news: Vec<String> = (head..na - tail).map(|r| id(after, r, &ca)).collect();
            // between the rows both share, in order, each run of rows that differ: a row
            // paired, in order, with the first row of the other side holding a cell it holds
            // (a row edited), what is left over removed or added
            let kept = shortest_edit(&olds, &news, EDIT_LIMIT).unwrap_or_default();
            let shares = |b: usize, a: usize| shared.iter().any(|&(kb, ka)| before.text(b, kb) == after.text(a, ka));
            let mut pairs = Vec::new();
            let (mut i, mut j) = (0, 0);
            for (x, y) in kept.into_iter().chain([(olds.len(), news.len())]) {
                let mut next = head + j;
                for b in (head + i)..(head + x) {
                    match (next..head + y).find(|&a| shares(b, a)) {
                        Some(a) => {
                            out.rows_added.extend((next + 1)..=a);
                            pairs.push((b, a));
                            next = a + 1;
                        }
                        None => out.rows_removed.push(b + 1),
                    }
                }
                out.rows_added.extend((next + 1)..=(head + y));
                (i, j) = (x + 1, y + 1);
            }
            out.rows_added.sort_unstable();
            pairs
        }
    };
    for (b, a) in pairs {
        for &(kb, ka) in &shared {
            let (was, now) = (before.text(b, kb), after.text(a, ka));
            if was != now {
                out.cells.push(Cell {
                    row: a + 1,
                    key: key_of(after, a),
                    column: name(after, ka),
                    was: was.into_owned(),
                    now: now.into_owned(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

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
        assert_eq!((own.path.as_str(), &own.via[..]), (Q4, &[Q4.to_owned()][..]));
        assert_eq!(own.stored, [Cell { row: 2, key: None, column: "cost".into(), was: "180.00".into(), now: "200.00".into() }]);
        assert_eq!(index.path, QUARTERS);
        assert_eq!(index.via, [Q4]);
        assert_eq!(index.stored[0].key.as_deref(), Some("q4"), "a keyed table names its row");
        let edited = out.edited.expect("the edit said");
        assert_eq!(edited.cells, [Cell { row: 2, key: None, column: "amount".into(), was: "180.00".into(), now: "200.00".into() }]);
        assert!(edited.rows_added.is_empty() && edited.rows_removed.is_empty());
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
        assert_eq!(out.writes[2].via, [QUARTERS], "the year follows the quarters, never the q4 it reaches through them");
    }

    #[test]
    fn the_shortest_edit_keeps_the_most_in_order() {
        let s = |t: &str| t.chars().map(String::from).collect::<Vec<_>>();
        // Myers' own example: ABCABBA to CBABAC keeps four
        let (a, b) = (s("ABCABBA"), s("CBABAC"));
        let kept = shortest_edit(&a, &b, EDIT_LIMIT).unwrap();
        assert_eq!(kept.len(), 4);
        assert!(kept.windows(2).all(|w| w[0].0 < w[1].0 && w[0].1 < w[1].1), "in order: {kept:?}");
        assert!(kept.iter().all(|&(x, y)| a[x] == b[y]));
        assert_eq!(shortest_edit(&a, &a, EDIT_LIMIT).unwrap().len(), 7, "the same: all kept");
        assert!(shortest_edit(&s("ABCD"), &s("WXYZ"), 3).is_none(), "past the limit");
        assert!(shortest_edit(&s(""), &s("AB"), EDIT_LIMIT).unwrap().is_empty());
    }

    #[test]
    fn rows_edited_and_added_in_one_save_are_told_apart() {
        let table = |rows: &[&str]| alloc::format!("what:text,amount:EUR\n{}\n", rows.join("\n"));
        let before = read_with(
            table(&["rent,900.00", "power,60.00", "water,30.00", "phone,20.00"]).into_bytes(),
            &mut Files::new(&BTreeMap::new(), "/x"),
            1,
        )
        .unwrap();
        let after = read_with(
            table(&["rent,900.00", "power,65.00", "water,30.00", "phone,20.00", "fuel,40.00"]).into_bytes(),
            &mut Files::new(&BTreeMap::new(), "/x"),
            1,
        )
        .unwrap();
        let e = edited(&before, &after);
        assert_eq!(e.cells, [Cell { row: 2, key: None, column: "amount".into(), was: "60.00".into(), now: "65.00".into() }]);
        assert_eq!((&e.rows_added[..], &e.rows_removed[..]), (&[5][..], &[][..]), "the appended row alone");
        let moved = read_with(
            table(&["power,60.00", "rent,900.00", "water,31.00", "phone,20.00"]).into_bytes(),
            &mut Files::new(&BTreeMap::new(), "/x"),
            1,
        )
        .unwrap();
        let e = edited(&before, &moved);
        assert_eq!(e.cells.iter().map(|c| (c.row, c.now.as_str())).collect::<Vec<_>>(), [(3, "31.00")], "{e:?}");
        assert_eq!((e.rows_added.len(), e.rows_removed.len()), (1, 1), "a row moved: removed where it was, added where it is");
    }

    fn doc(text: &str) -> Doc {
        read_with(text.as_bytes().to_vec(), &mut Files::new(&BTreeMap::new(), "/x"), 1).unwrap()
    }

    fn columns(e: &Edited) -> (Vec<(&str, &str)>, Vec<&str>, Vec<&str>) {
        (
            e.columns_renamed.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect(),
            e.columns_added.iter().map(String::as_str).collect(),
            e.columns_removed.iter().map(String::as_str).collect(),
        )
    }

    #[test]
    fn a_column_renamed_is_one_change() {
        let before = doc("what:text,something:text,amount:EUR\nrent,a,900.00\npower,b,60.00\n");
        let e = edited(&before, &doc("what:text,somethingff:text,amount:EUR\nrent,a,900.00\npower,b,60.00\n"));
        assert_eq!(columns(&e), (vec![("something", "somethingff")], vec![], vec![]));
        assert!(e.cells.is_empty() && e.rows_added.is_empty() && e.rows_removed.is_empty(), "its cells are the same: {e:?}");

        let e = edited(&before, &doc("somethingff:text,what:text,amount:EUR\na,rent,900.00\nb,power,60.00\n"));
        assert_eq!(columns(&e), (vec![("something", "somethingff")], vec![], vec![]), "moved and renamed, still the column");
        assert!(e.cells.is_empty(), "{e:?}");

        let e = edited(&before, &doc("what:text,somethingff:int,amount:EUR\nrent,1,900.00\npower,2,60.00\n"));
        assert_eq!(columns(&e), (vec![], vec!["somethingff"], vec!["something"]), "renamed and retyped is another column");

        let e = edited(&before, &doc("what:text,somethingff:text,amount:EUR\nrent,a,900.00\npower,c,60.00\n"));
        assert_eq!(columns(&e), (vec![], vec!["somethingff"], vec!["something"]), "renamed with a cell changed");

        let e = edited(&before, &doc("what:text,note:text,amount:EUR\nrent,x,900.00\npower,y,60.00\n"));
        assert_eq!(columns(&e), (vec![], vec!["note"], vec!["something"]), "one column removed, another added in its place");
        assert!(e.cells.is_empty(), "no cell of a column removed is a cell changed: {e:?}");

        let keyed = doc("what:text!,amount:EUR\nrent,900.00\npower,60.00\n");
        let e = edited(&keyed, &doc("item:text!,amount:EUR\nrent,900.00\npower,65.00\n"));
        assert_eq!(e.columns_renamed, [("what".into(), "item".into())]);
        assert_eq!(
            e.cells,
            [Cell { row: 2, key: Some("power".into()), column: "amount".into(), was: "60.00".into(), now: "65.00".into() }],
            "a key renamed still names its rows, another column's cells changed beside it"
        );
    }

    #[test]
    fn an_edit_says_its_rows_and_columns() {
        let mut f = folder();
        let added =
            changed(&mut f, Q4, &MONEY.replace("income,4200.00,0.00=\n", "income,4200.00,0.00=\ncost,5.00,0.00=\n")).edited.unwrap();
        assert_eq!(
            (&added.rows_added[..], &added.rows_removed[..], added.cells.len()),
            (&[2][..], &[][..], 0),
            "a row inserted, the one under it the same"
        );
        let keyed = |t: &str| alloc::format!("---\nimport q4 from \"./q4/money.cells\"\n---\nquarter:text!,costs:EUR\n{t}");
        let before = keyed("q3,1.00=SUM(q4.cost)\nq4,180.00=SUM(q4.cost)\n");
        changed(&mut f, QUARTERS, &before);
        let moved = changed(&mut f, QUARTERS, &keyed("q4,180.00=SUM(q4.cost)\nq3,2.00=SUM(q4.cost)\n")).edited.unwrap();
        assert_eq!(
            moved.cells,
            [Cell {
                row: 2,
                key: Some("q3".into()),
                column: "costs".into(),
                was: "1.00=SUM(q4.cost)".into(),
                now: "2.00=SUM(q4.cost)".into()
            }],
            "rows matched by key"
        );
        let wider =
            changed(&mut f, QUARTERS, &keyed("q4,180.00=SUM(q4.cost)\nq3,2.00=SUM(q4.cost)\n").replace("costs:EUR\n", "costs:EUR,note\n"))
                .edited
                .unwrap();
        assert_eq!(wider.columns_added, ["note"]);
        let mut g = folder();
        let aligned = MONEY.replace(
            "kind:Kind,amount:EUR,cost\nincome,4200.00,0.00=\ncost,180.00,180.00=\n",
            "kind:Kind,  amount:EUR,  cost\nincome,    4200.00,     0.00=\ncost,      180.00,      180.00=\n",
        );
        let realigned = changed(&mut g, Q4, &aligned);
        assert!(realigned.edited.as_ref().is_some_and(Edited::is_empty), "padding is no part of a value: {:?}", realigned.edited);
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
