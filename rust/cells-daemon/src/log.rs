//! The folder's log, `.cells/log.jsonl` (ENGINE.md, The log): one line per version of a
//! file, so every version of every file is explained. Each line holds its number and the
//! SHA-256 of the line before it, so a line edited or removed breaks every later link.
//!
//! - `store`: the engine wrote the file, what its change reached stored: which file's
//!   change reached it, the version it was written over (`against`), the version written
//!   (`now`) and each cell it stored, before and after.
//! - `external`: a version the engine did not write (an editor's save, a git checkout):
//!   the hash before (`null` for a file new to the folder) and now (`null` once removed).
//! - `moved`: a write the engine did not make, its file changed since the version it was
//!   made from; the change that moved it is read next, and stores what it reaches.

use std::path::{Path, PathBuf};

use cells::folder::Stored;
use serde::{Deserialize, Serialize};

use crate::world::World;

pub struct Log {
    /// The folder logged, its paths written from here.
    root: PathBuf,
    path: PathBuf,
    /// The next line's number, and the SHA-256 of the line before it.
    next: u64,
    prev: String,
}

pub enum Entry<'a> {
    Store { path: &'a Path, because: &'a Path, against: &'a str, now: &'a str, stored: &'a [Stored] },
    External { path: &'a Path, was: Option<&'a str>, now: Option<&'a str> },
    Moved { path: &'a Path, against: &'a str },
}

/// The SHA-256 of `text`, as every hash in the log is written.
pub fn hash(text: &str) -> String {
    cells::sha256::hex(&cells::sha256::digest(text.as_bytes()))
}

/// A line of the log as written: its place in the chain, its time, and the entry, every
/// path from the folder logged.
#[derive(Serialize)]
struct Line {
    n: u64,
    prev: String,
    at: u64,
    #[serde(flatten)]
    entry: Logged,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Logged {
    Store { path: String, because: String, against: String, now: String, stored: Vec<Cell> },
    External { path: String, was: Option<String>, now: Option<String> },
    Moved { path: String, against: String },
}

#[derive(Serialize)]
struct Cell {
    row: usize,
    column: String,
    was: String,
    now: String,
}

/// Of a line, only its number: what the next line continues from.
#[derive(Deserialize)]
struct Numbered {
    n: u64,
}

impl Log {
    /// The log of the folder at `root`, continued from its last line.
    pub fn open(root: &Path, world: &impl World) -> Log {
        let path = root.join(".cells").join("log.jsonl");
        let (next, prev) = match world.last_line(&path) {
            None => (1, String::new()),
            Some(line) => {
                let Numbered { n } =
                    serde_json::from_str(&line).unwrap_or_else(|e| panic!("{}: its last line is no entry ({e}): {line}", path.display()));
                (n + 1, hash(&line))
            }
        };
        Log { root: root.to_path_buf(), path, next, prev }
    }

    fn shown(&self, path: &Path) -> String {
        path.strip_prefix(&self.root).unwrap_or(path).to_string_lossy().into_owned()
    }

    /// `entry` appended at `at` (milliseconds since the Unix epoch).
    pub fn append(&mut self, world: &impl World, at: u64, entry: Entry) -> Result<(), String> {
        let entry = match entry {
            Entry::Store { path, because, against, now, stored } => Logged::Store {
                path: self.shown(path),
                because: self.shown(because),
                against: against.to_owned(),
                now: now.to_owned(),
                stored: stored
                    .iter()
                    .map(|s| Cell { row: s.row, column: s.column.clone(), was: s.was.clone(), now: s.now.clone() })
                    .collect(),
            },
            Entry::External { path, was, now } => {
                Logged::External { path: self.shown(path), was: was.map(str::to_owned), now: now.map(str::to_owned) }
            }
            Entry::Moved { path, against } => Logged::Moved { path: self.shown(path), against: against.to_owned() },
        };
        let line = serde_json::to_string(&Line { n: self.next, prev: self.prev.clone(), at, entry }).expect("a log line serializes");
        world.append(&self.path, &line)?;
        self.next += 1;
        self.prev = hash(&line);
        Ok(())
    }
}
