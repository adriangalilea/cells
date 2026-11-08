//! The folder's log, `.cells/log.jsonl` (ENGINE.md, The log and History): one line per
//! version of a file, so every version of every file is explained, by what made it and
//! what it set off. Each line holds its number and the SHA-256 of the line before it, so a
//! line edited or removed breaks every later link. Every version a line names is kept whole
//! in `.cells/versions/<sha256>`, so any version can be shown and returned to.
//!
//! A line is a root or follows one:
//!
//! - `external`, a root: a version the engine did not write (an editor's save, a git
//!   checkout), the hash before (`null` for a file new to the folder) and now (`null` once
//!   removed), who wrote it when a client said it held that version (`by`), and what it
//!   changed beside the version before (`edited`).
//! - `restore`, a root: a file returned to a version by a client's request (`by`).
//! - `store`: the engine wrote the file, storing what a root reached: the root (`cause`),
//!   the files of its chain this one reads directly (`via`), the version written over
//!   (`against`) and written (`now`), and each cell stored, before and after.
//! - `moved`: a write the engine did not make, its file changed since the version it was
//!   made from; the change that moved it is a root of its own, read next.

use std::path::{Path, PathBuf};

use cells::folder::{Cell, Edited};
use serde::{Deserialize, Serialize};

use crate::world::{Found, World};

/// The SHA-256 of `text`, as every hash in the log is written.
pub fn hash(text: &str) -> String {
    cells::sha256::hex(text.as_bytes())
}

/// A line of the log: its place in the chain, its time (milliseconds since the Unix
/// epoch), and the entry, every path from the folder logged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub n: u64,
    pub prev: String,
    pub at: u64,
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
    Store {
        path: String,
        cause: Cause,
        via: Vec<String>,
        against: String,
        now: String,
        stored: Vec<Changed>,
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
            Logged::External { path, .. } | Logged::Restore { path, .. } | Logged::Store { path, .. } | Logged::Moved { path, .. } => path,
        }
    }

    /// The root this entry follows; nothing for a root.
    pub fn cause(&self) -> Option<&Cause> {
        match self {
            Logged::Store { cause, .. } | Logged::Moved { cause, .. } => Some(cause),
            Logged::External { .. } | Logged::Restore { .. } => None,
        }
    }

    /// The version the file holds after this entry, when it holds one.
    pub fn now(&self) -> Option<&str> {
        match self {
            Logged::External { now, .. } => now.as_deref(),
            Logged::Restore { now, .. } | Logged::Store { now, .. } => Some(now),
            Logged::Moved { .. } => None,
        }
    }
}

/// The root an entry follows: its number in its folder's log, and that folder when it is
/// another than the entry's (a file read through an import from a folder served apart).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cause {
    pub n: u64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub folder: Option<PathBuf>,
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
}

impl From<&Edited> for Edit {
    fn from(e: &Edited) -> Edit {
        Edit {
            cells: e.cells.iter().map(Changed::from).collect(),
            rows_added: e.rows_added.clone(),
            rows_removed: e.rows_removed.clone(),
            columns_added: e.columns_added.clone(),
            columns_removed: e.columns_removed.clone(),
        }
    }
}

/// A folder's log, every line of it held, read whole when the folder is served.
pub struct Log {
    /// The folder logged, its paths written from here.
    pub root: PathBuf,
    path: PathBuf,
    lines: Vec<Line>,
    /// The SHA-256 of the last line as written: what the next line continues from.
    prev: String,
}

impl Log {
    /// The log of the folder at `root`, read whole, and why it cannot be trusted past a
    /// line when a link is broken or a line is no entry: it is continued either way, and
    /// what follows a break is said, never hidden.
    pub fn open(root: &Path, world: &impl World) -> (Log, Option<String>) {
        let path = root.join(".cells").join("log.jsonl");
        let text = match world.read(&path) {
            Found::Text(t) => t,
            Found::Missing => String::new(),
            Found::Unreadable(why) => panic!("{}: {why}", path.display()),
        };
        let mut lines = Vec::new();
        let mut prev = String::new();
        let mut broken = None;
        for (i, raw) in text.lines().enumerate() {
            match serde_json::from_str::<Line>(raw) {
                Ok(line) => {
                    if broken.is_none() && line.prev != prev {
                        broken = Some(format!("{}: line {} does not follow the line before it", path.display(), i + 1));
                    }
                    lines.push(line);
                }
                Err(e) => {
                    if broken.is_none() {
                        broken = Some(format!("{}: line {} is no entry ({e})", path.display(), i + 1));
                    }
                }
            }
            prev = hash(raw);
        }
        (Log { root: root.to_path_buf(), path, lines, prev }, broken)
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// A path as the log writes it: from the folder logged.
    pub fn shown(&self, path: &Path) -> String {
        path.strip_prefix(&self.root).unwrap_or(path).to_string_lossy().into_owned()
    }

    /// Where the version whose SHA-256 is `version` is kept.
    pub fn version(&self, version: &str) -> PathBuf {
        assert!(version.len() == 64 && version.bytes().all(|b| b.is_ascii_hexdigit()), "{version:?} is no SHA-256");
        self.root.join(".cells").join("versions").join(version)
    }

    /// `text` kept as the version it is, so a line naming it can show it whole.
    pub fn keep(&self, world: &impl World, text: &str) -> Result<(), String> {
        world.keep(&self.version(&hash(text)), text)
    }

    /// `entry` appended at `at`: its number.
    pub fn append(&mut self, world: &impl World, at: u64, entry: Logged) -> Result<u64, String> {
        let n = self.lines.last().map_or(1, |l| l.n + 1);
        let line = Line { n, prev: self.prev.clone(), at, entry };
        let raw = serde_json::to_string(&line).expect("a log line serializes");
        world.append(&self.path, &raw)?;
        self.prev = hash(&raw);
        self.lines.push(line);
        Ok(n)
    }
}
