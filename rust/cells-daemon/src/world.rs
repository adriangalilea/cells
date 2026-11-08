//! The daemon's one way to the world (ENGINE.md, Proving it): files read, listed and
//! replaced, lines appended to a log, folders locked and watched, and the clock. `Disk` is
//! a Mac's; a simulation implements the same trait, so the daemon's every decision runs
//! the same against either.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

/// A file as it was read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    Text(String),
    Missing,
    /// It exists and could not be read as text: why.
    Unreadable(String),
}

/// What became of a replacement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Replaced {
    Done,
    /// The file is no longer the version the text was made from (someone wrote it since,
    /// or removed it): nothing was written, and the change that moved it is heard next.
    Moved,
    Failed(String),
}

pub trait World {
    fn read(&self, path: &Path) -> Found;
    /// Every file a table is read from (`.cells`, `.csv`, `.tsv`) under `root`, hidden
    /// folders left out (`.git`, the engine's own `.cells`), sorted.
    fn list(&self, root: &Path) -> Vec<PathBuf>;
    /// `text` written over `path` whole or not at all, and only over the version whose
    /// SHA-256 is `against`.
    fn replace(&self, path: &Path, against: &str, text: &str) -> Replaced;
    /// `line` appended to the file at `path`, and flushed to the disk.
    fn append(&self, path: &Path, line: &str) -> Result<(), String>;
    /// The last line of the file at `path`, nothing when it has none.
    fn last_line(&self, path: &Path) -> Option<String>;
    /// `root` held for this process alone while it runs: two daemons never write one folder.
    fn lock(&mut self, root: &Path) -> Result<(), String>;
    /// Changes under `dir` heard from now on, everything under it when `whole`: each path
    /// handed to what the world was made with (`Disk::new`).
    fn watch(&mut self, dir: &Path, whole: bool) -> Result<(), String>;
    /// The user's home folder, which is never served whole.
    fn home(&self) -> PathBuf;
    /// Milliseconds since the Unix epoch.
    fn now(&self) -> u64;
}

/// Whether a table is read from a file at `path`.
pub fn table_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "cells" || e == "csv" || e == "tsv")
}

pub struct Disk {
    watcher: RecommendedWatcher,
    /// Each folder's lock, held while the daemon runs.
    locks: Vec<File>,
}

impl Disk {
    /// The disk, every path it says changed under a folder watched handed to `heard`, on the
    /// watcher's own thread.
    pub fn new(heard: impl Fn(PathBuf) + Send + 'static) -> Result<Disk, String> {
        let watcher = notify::recommended_watcher(move |e: notify::Result<notify::Event>| match e {
            Ok(e) if !e.kind.is_access() => e.paths.into_iter().for_each(&heard),
            Ok(_) => {}
            Err(e) => eprintln!("cells serve: the disk's change events failed: {e}"),
        })
        .map_err(|e| format!("no file watcher: {e}"))?;
        Ok(Disk { watcher, locks: Vec::new() })
    }
}

impl World for Disk {
    fn read(&self, path: &Path) -> Found {
        match fs::read(path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => Found::Text(text),
                Err(_) => Found::Unreadable("its bytes are not UTF-8".into()),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Found::Missing,
            Err(e) => Found::Unreadable(e.to_string()),
        }
    }

    fn list(&self, root: &Path) -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = fs::read_dir(dir) else { return };
            for e in entries.flatten() {
                let p = e.path();
                let hidden = p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
                if hidden {
                    continue;
                }
                if p.is_dir() {
                    walk(&p, out);
                } else if table_file(&p) {
                    out.push(p);
                }
            }
        }
        let mut out = Vec::new();
        walk(root, &mut out);
        out.sort();
        out
    }

    fn replace(&self, path: &Path, against: &str, text: &str) -> Replaced {
        let Ok(now) = fs::read(path) else { return Replaced::Moved };
        if cells::sha256::hex(&now) != against {
            return Replaced::Moved;
        }
        let dir = path.parent().expect("a file is in a folder");
        let name = path.file_name().expect("a file has a name").to_string_lossy();
        let beside = dir.join(format!(".{name}.cells-serve-{}", std::process::id()));
        let written = (|| -> std::io::Result<()> {
            let mut f = File::create(&beside)?;
            f.write_all(text.as_bytes())?;
            f.set_permissions(fs::metadata(path)?.permissions())?;
            f.sync_all()?;
            // The check above and this rename are not one step: a write landing between them
            // is lost to the rename. Detected, never prevented (ENGINE.md, Never lose
            // anyone's write): the window is the rename's, and the log holds both hashes.
            fs::rename(&beside, path)?;
            File::open(dir)?.sync_all()
        })();
        match written {
            Ok(()) => Replaced::Done,
            Err(e) => {
                let _ = fs::remove_file(&beside);
                Replaced::Failed(e.to_string())
            }
        }
    }

    fn append(&self, path: &Path, line: &str) -> Result<(), String> {
        let dir = path.parent().expect("a log is in a folder");
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut f = OpenOptions::new().create(true).append(true).open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        f.write_all(format!("{line}\n").as_bytes()).and_then(|()| f.sync_data()).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn last_line(&self, path: &Path) -> Option<String> {
        let text = fs::read_to_string(path).ok()?;
        text.lines().last().map(str::to_owned)
    }

    fn lock(&mut self, root: &Path) -> Result<(), String> {
        let dir = root.join(".cells");
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join("serve.lock");
        let f = File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        match f.try_lock() {
            Ok(()) => {
                self.locks.push(f);
                Ok(())
            }
            Err(TryLockError::WouldBlock) => Err(format!("{} is served already: another `cells serve` holds it", root.display())),
            Err(TryLockError::Error(e)) => Err(format!("{}: {e}", path.display())),
        }
    }

    fn watch(&mut self, dir: &Path, whole: bool) -> Result<(), String> {
        let mode = if whole { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
        self.watcher.watch(dir, mode).map_err(|e| format!("{}: {e}", dir.display()))
    }

    fn home(&self) -> PathBuf {
        let home = PathBuf::from(std::env::var_os("HOME").expect("HOME is set"));
        fs::canonicalize(&home).unwrap_or(home)
    }

    fn now(&self) -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).expect("the clock is after 1970").as_millis() as u64
    }
}
