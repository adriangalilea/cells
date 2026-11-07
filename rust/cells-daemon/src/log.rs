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

fn json(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn nullable(s: Option<&str>) -> String {
    s.map_or("null".to_owned(), json)
}

impl Log {
    /// The log of the folder at `root`, continued from its last line.
    pub fn open(root: &Path, world: &impl World) -> Log {
        let path = root.join(".cells").join("log.jsonl");
        let (next, prev) = match world.last_line(&path) {
            None => (1, String::new()),
            Some(line) => {
                let n = line
                    .strip_prefix("{\"n\":")
                    .and_then(|rest| rest.split(',').next())
                    .and_then(|n| n.parse::<u64>().ok())
                    .unwrap_or_else(|| panic!("{}: its last line has no number: {line}", path.display()));
                (n + 1, hash(&line))
            }
        };
        Log { root: root.to_path_buf(), path, next, prev }
    }

    fn shown(&self, path: &Path) -> String {
        json(&path.strip_prefix(&self.root).unwrap_or(path).to_string_lossy())
    }

    /// `entry` appended at `at` (milliseconds since the Unix epoch).
    pub fn append(&mut self, world: &impl World, at: u64, entry: Entry) -> Result<(), String> {
        let body = match entry {
            Entry::Store { path, because, against, now, stored } => {
                let cells: Vec<String> = stored
                    .iter()
                    .map(|s| {
                        format!("{{\"row\":{},\"column\":{},\"was\":{},\"now\":{}}}", s.row, json(&s.column), json(&s.was), json(&s.now))
                    })
                    .collect();
                format!(
                    "\"kind\":\"store\",\"path\":{},\"because\":{},\"against\":{},\"now\":{},\"stored\":[{}]",
                    self.shown(path),
                    self.shown(because),
                    json(against),
                    json(now),
                    cells.join(",")
                )
            }
            Entry::External { path, was, now } => {
                format!("\"kind\":\"external\",\"path\":{},\"was\":{},\"now\":{}", self.shown(path), nullable(was), nullable(now))
            }
            Entry::Moved { path, against } => format!("\"kind\":\"moved\",\"path\":{},\"against\":{}", self.shown(path), json(against)),
        };
        let line = format!("{{\"n\":{},\"prev\":{},\"at\":{at},{body}}}", self.next, json(&self.prev));
        world.append(&self.path, &line)?;
        self.next += 1;
        self.prev = hash(&line);
        Ok(())
    }
}
