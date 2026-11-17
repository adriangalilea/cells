//! The engine's history, `history/log.jsonl` in its own folder (`folder`; ENGINE.md, The
//! log and History): one chain for every file it holds, one line per version of a file, so
//! every version of every file is explained, by what made it and what it set off. Each line
//! holds its number, the SHA-256 of the line before it (so a line edited or removed breaks
//! every later link), the file it is about by its identity (`index::FileId`) and its path
//! at that moment, whole. A file's history is every line of its identity, across every move
//! and folder. Every version a line names is kept whole in `history/versions/<sha256>`, so
//! any version can be shown and returned to. Nothing of it is ever written inside the
//! person's folders: a file sent carries no history.
//!
//! A line is a root or follows one:
//!
//! - `external`, a root: a version the engine did not write (an editor's save, a git
//!   checkout), the hash before (`null` for a file new to the engine) and now (`null` once
//!   removed), who wrote it when a client said it held that version (`by`), and what it
//!   changed beside the version before (`edited`).
//! - `restore`, a root: a file returned to a version by a client's request (`by`).
//! - `run`, a root: a script's run, what started it and what came of it.
//! - `edit`, a root: a client's intent (`intent` its id), its ops and the ids it gave, so
//!   it is applied once however often it is sent.
//! - `store`: the engine wrote the file, storing what a root reached: the root (`cause`),
//!   the files of its chain this one reads directly (`via`), the version written over
//!   (`against`) and written (`now`), and each cell stored, before and after.
//! - `moved`: a write the engine did not make, its file changed since the version it was
//!   made from; the change that moved it is a root of its own, read next.

use std::path::PathBuf;

use cells::folder::{Cell, Edited};
use serde::{Deserialize, Serialize};

use crate::index::FileId;
use crate::world::{Found, World};

/// The SHA-256 of `text`, as every hash in the log is written.
pub fn hash(text: &str) -> String {
    cells::sha256::hex(text.as_bytes())
}

/// The history's folder, in the engine's own (`wire::folder`): the log, the versions it
/// names, and the index of the files they are about.
pub fn folder() -> PathBuf {
    crate::wire::folder().join("history")
}

/// The log's one file.
pub fn path() -> PathBuf {
    folder().join("log.jsonl")
}

/// Where the version whose SHA-256 is `version` is kept.
pub fn version(version: &str) -> PathBuf {
    assert!(version.len() == 64 && version.bytes().all(|b| b.is_ascii_hexdigit()), "{version:?} is no SHA-256");
    folder().join("versions").join(version)
}

/// A line of the log: its place in the chain, its time (milliseconds since the Unix
/// epoch), the file it is about, and the entry, every path in it whole, as it was then.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub n: u64,
    pub prev: String,
    pub at: u64,
    pub file: FileId,
    #[serde(flatten)]
    pub entry: Logged,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Logged {
    External {
        path: String,
        was: Option<String>,
        now: Option<String>,
        by: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        edited: Option<Edit>,
    },
    Restore {
        path: String,
        was: String,
        now: String,
        by: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        edited: Option<Edit>,
    },
    /// A script's run, a root: its rows landed in its table (or only its failure in its
    /// runs block), who started it (`by`: its schedule, a file changing, a client).
    Run {
        path: String,
        script: String,
        run: u64,
        #[serde(flatten)]
        outcome: Outcome,
        by: String,
        was: String,
        now: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        edited: Option<Edit>,
    },
    /// A client's intent, a root (ENGINE.md, Writing): its id (the client's, what makes it
    /// idempotent), who sent it, its ops, the ids the engine gave rows it wrote, the version
    /// it was written over and the one written.
    Edit {
        path: String,
        intent: String,
        by: String,
        ops: Vec<Op>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        assigned: Vec<String>,
        was: String,
        now: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        edited: Option<Edit>,
    },
    /// A file moved, a root: the file held at `from` is the one at `path` (by anyone: the
    /// Finder, an app, a shell; how it was known is `index::follow`'s), holding the version
    /// it held (`now`). Every import naming it follows it (a `store` each).
    Relocated {
        path: String,
        from: String,
        now: String,
    },
    Store {
        path: String,
        cause: Cause,
        via: Vec<String>,
        against: String,
        now: String,
        stored: Vec<Changed>,
        /// Each import's path written anew following a file moved: as written before, as
        /// written now.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        imports: Vec<(String, String)>,
    },
    Moved {
        path: String,
        cause: Cause,
        against: String,
    },
}

impl Logged {
    pub fn path(&self) -> &str {
        match self {
            Logged::External { path, .. }
            | Logged::Restore { path, .. }
            | Logged::Run { path, .. }
            | Logged::Edit { path, .. }
            | Logged::Relocated { path, .. }
            | Logged::Store { path, .. }
            | Logged::Moved { path, .. } => path,
        }
    }

    /// The root this entry follows; nothing for a root.
    pub fn cause(&self) -> Option<&Cause> {
        match self {
            Logged::Store { cause, .. } | Logged::Moved { cause, .. } => Some(cause),
            Logged::External { .. } | Logged::Restore { .. } | Logged::Run { .. } | Logged::Edit { .. } | Logged::Relocated { .. } => None,
        }
    }

    /// The version the file holds after this entry, when it holds one.
    pub fn now(&self) -> Option<&str> {
        match self {
            Logged::External { now, .. } => now.as_deref(),
            Logged::Restore { now, .. }
            | Logged::Run { now, .. }
            | Logged::Edit { now, .. }
            | Logged::Relocated { now, .. }
            | Logged::Store { now, .. } => Some(now),
            Logged::Moved { .. } => None,
        }
    }

    /// What the root's own version changed in its file, when both versions read.
    pub fn edited(&self) -> Option<&Edit> {
        match self {
            Logged::External { edited, .. } | Logged::Restore { edited, .. } | Logged::Run { edited, .. } | Logged::Edit { edited, .. } => {
                edited.as_ref()
            }
            Logged::Relocated { .. } | Logged::Store { .. } | Logged::Moved { .. } => None,
        }
    }
}

/// A row as an intent names it (`cells::RowAt`): by its key, by its id, or, in a file
/// that has given its rows neither yet, by its line exactly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Row {
    Key(String),
    Id(String),
    Line(String),
}

impl From<&Row> for cells::RowAt {
    fn from(r: &Row) -> cells::RowAt {
        match r {
            Row::Key(k) => cells::RowAt::Key(k.clone()),
            Row::Id(i) => cells::RowAt::Id(i.clone()),
            Row::Line(l) => cells::RowAt::Line(l.clone()),
        }
    }
}

/// A column given its text, in a row an intent adds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Given {
    pub column: String,
    pub text: String,
}

/// An op of an intent (`cells::intent::Op`), as a client sends it and the log keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Op {
    Set {
        row: Row,
        column: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        was: Option<String>,
        text: String,
    },
    Add {
        cells: Vec<Given>,
    },
    Delete {
        row: Row,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        was: Option<String>,
    },
    /// The file whole, as the client holds it: sent with its `text`; kept in the log by the
    /// SHA-256 of that text (`version`, its text whole in the versions kept), never twice.
    Text {
        #[serde(skip_serializing_if = "String::is_empty", default)]
        text: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        version: Option<String>,
    },
}

impl Op {
    /// The op as the log keeps it: a whole text by its version.
    pub fn logged(&self) -> Op {
        match self {
            Op::Text { text, .. } => Op::Text { text: String::new(), version: Some(crate::log::hash(text)) },
            other => other.clone(),
        }
    }
}

impl From<&Op> for cells::intent::Op {
    fn from(op: &Op) -> cells::intent::Op {
        match op {
            Op::Set { row, column, was, text } => {
                cells::intent::Op::Set { row: row.into(), column: column.clone(), was: was.clone(), text: text.clone() }
            }
            Op::Add { cells } => cells::intent::Op::Add { cells: cells.iter().map(|g| (g.column.clone(), g.text.clone())).collect() },
            Op::Delete { row, was } => cells::intent::Op::Delete { row: row.into(), was: was.clone() },
            Op::Text { text, .. } => cells::intent::Op::Text { text: text.clone() },
        }
    }
}

/// What came of a run: the rows its table gained and had replaced, or why it failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "lowercase")]
pub enum Outcome {
    Ok { added: usize, changed: usize },
    Failed { why: String },
}

/// The root an entry follows: its number in the log.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cause {
    pub n: u64,
}

/// A cell changed (`cells::folder::Cell`), as the log writes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changed {
    pub row: usize,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub key: Option<String>,
    pub column: String,
    pub was: String,
    pub now: String,
}

impl From<&Cell> for Changed {
    fn from(c: &Cell) -> Changed {
        Changed { row: c.row, key: c.key.clone(), column: c.column.clone(), was: c.was.clone(), now: c.now.clone() }
    }
}

/// What a version changed (`cells::folder::Edited`), as the log writes it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edit {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub cells: Vec<Changed>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rows_added: Vec<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rows_removed: Vec<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub columns_added: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub columns_removed: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub columns_renamed: Vec<Renamed>,
}

/// A column a version holds under another name: its name before and its name now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Renamed {
    pub from: String,
    pub to: String,
}

impl From<&Edited> for Edit {
    fn from(e: &Edited) -> Edit {
        Edit {
            cells: e.cells.iter().map(Changed::from).collect(),
            rows_added: e.rows_added.clone(),
            rows_removed: e.rows_removed.clone(),
            columns_added: e.columns_added.clone(),
            columns_removed: e.columns_removed.clone(),
            columns_renamed: e.columns_renamed.iter().map(|(from, to)| Renamed { from: from.clone(), to: to.clone() }).collect(),
        }
    }
}

/// A chain of lines read from `text` (the file `name`): each line that reads as a `T`, the
/// SHA-256 of the last line (what the next continues from), and why it cannot be trusted
/// past a line when a link is broken (`prev_of` a line is not the hash of the one before)
/// or a line is no entry. Read on either way: what follows a break is said, never hidden.
pub fn chain<T: for<'de> Deserialize<'de>>(text: &str, name: &str, prev_of: impl Fn(&T) -> &str) -> (Vec<T>, String, Option<String>) {
    let mut lines = Vec::new();
    let mut prev = String::new();
    let mut broken = None;
    for (i, raw) in text.lines().enumerate() {
        match serde_json::from_str::<T>(raw) {
            Ok(line) => {
                if broken.is_none() && prev_of(&line) != prev {
                    broken = Some(format!("{name}: line {} does not follow the line before it", i + 1));
                }
                lines.push(line);
            }
            Err(e) => {
                if broken.is_none() {
                    broken = Some(format!("{name}: line {} is no entry ({e})", i + 1));
                }
            }
        }
        prev = hash(raw);
    }
    (lines, prev, broken)
}

/// The log, every line of it held, read whole when the engine starts.
pub struct Log {
    lines: Vec<Line>,
    /// The SHA-256 of the last line as written: what the next line continues from.
    prev: String,
}

impl Log {
    /// The log read whole, and why it cannot be trusted past a line (`chain`): it is
    /// continued either way. A log the disk will not give is refused: a history continued
    /// from nothing would fork it.
    pub fn open(world: &impl World) -> Result<(Log, Option<String>), String> {
        let at = path();
        let text = match world.read(&at) {
            Found::Text(t) => t,
            Found::Missing => String::new(),
            Found::Unreadable(why) => return Err(format!("the history ({}) cannot be read: {why}", at.display())),
        };
        let (lines, prev, broken) = chain(&text, &at.display().to_string(), |l: &Line| &l.prev);
        Ok((Log { lines, prev }, broken))
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// Every line about the file `id`, oldest first: its history, under every path it had.
    pub fn of<'a>(&'a self, id: &'a FileId) -> impl Iterator<Item = &'a Line> {
        self.lines.iter().filter(move |l| l.file == *id)
    }

    /// `text` kept as the version it is, so a line naming it can show it whole.
    pub fn keep(world: &impl World, text: &str) -> Result<(), String> {
        world.keep(&version(&hash(text)), text)
    }

    /// `entry`, about the file `file`, appended at `at`: its number.
    pub fn append(&mut self, world: &impl World, at: u64, file: &FileId, entry: Logged) -> Result<u64, String> {
        let n = self.lines.last().map_or(1, |l| l.n + 1);
        let line = Line { n, prev: self.prev.clone(), at, file: file.clone(), entry };
        let raw = serde_json::to_string(&line).expect("a log line serializes");
        world.append(&path(), &raw)?;
        self.prev = hash(&raw);
        self.lines.push(line);
        Ok(n)
    }
}

/// `lines` as the log writes them, each linked to the one before: the log's whole text.
/// Their numbers are theirs, in order from 1.
pub fn written(lines: &[Line]) -> String {
    let mut prev = String::new();
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        assert_eq!(line.n, i as u64 + 1, "a log numbered from 1, in order");
        let raw = serde_json::to_string(&Line { prev: prev.clone(), ..line.clone() }).expect("a log line serializes");
        prev = hash(&raw);
        out.push_str(&raw);
        out.push('\n');
    }
    out
}
