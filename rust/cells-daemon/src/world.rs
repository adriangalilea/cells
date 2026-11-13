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
    /// Every file a table is read from (`.cells`, `.csv`, `.tsv`) under `root` when `whole`,
    /// else in it alone, hidden folders left out (`.git`, the engine's own `.cells`), sorted.
    fn list(&self, root: &Path, whole: bool) -> Vec<PathBuf>;
    /// `text` written over `path` whole or not at all, and only over the version whose
    /// SHA-256 is `against`.
    fn replace(&self, path: &Path, against: &str, text: &str) -> Replaced;
    /// `line` appended to the file at `path`, and flushed to the disk.
    fn append(&self, path: &Path, line: &str) -> Result<(), String>;
    /// `text` written at `path` unless a file is there already: what is kept there is
    /// named by its own hash, so one there holds the same text.
    fn keep(&self, path: &Path, text: &str) -> Result<(), String>;
    /// `root` held for this process alone while it runs: two daemons never write one folder.
    fn lock(&mut self, root: &Path) -> Result<(), String>;
    /// Changes under `dir` heard from now on, everything under it when `whole`: each path
    /// handed to what the world was made with (`Disk::new`).
    fn watch(&mut self, dir: &Path, whole: bool) -> Result<(), String>;
    /// `code`, a script in `language`, started with `context` on its stdin, and `done`
    /// handed each JSON line it wrote on stdout, or why it failed, once it ends: never on
    /// the caller's thread when it takes time (a script fetches), so the engine keeps
    /// answering meanwhile. What it says on stderr is said as the script's.
    fn run_script(
        &self,
        name: &str,
        language: &str,
        code: &str,
        context: String,
        done: Box<dyn FnOnce(Result<Vec<String>, String>) + Send>,
    );
    /// The user's home folder, which is never served whole.
    fn home(&self) -> PathBuf;
    /// Milliseconds since the Unix epoch.
    fn now(&self) -> u64;
    /// Random bits, 40 at least: what a new row id is made from, so the core assigning it
    /// stays deterministic.
    fn bits(&self) -> u64;
}

/// What starts a `ts` script and hands it and its rows over (`cells::run`, the contract):
/// the context on stdin, each row returned a JSON line on stdout, its words on stderr.
const RUNNER: &str = r#"import script from "./script.ts"
console.log = console.error
const context = JSON.parse(await Bun.stdin.text())
const rows = await script(context)
if (!Array.isArray(rows)) throw new Error("the script returns no list of rows")
for (const row of rows) process.stdout.write(JSON.stringify(row) + "\n")
"#;

/// Where Bun is: on the PATH, else where its installer and Homebrew put it, since a launch
/// agent's PATH is the system's alone.
fn bun_path() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let on_path = std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).map(|d| d.join("bun"));
    on_path.chain([home.join(".bun/bin/bun"), "/opt/homebrew/bin/bun".into(), "/usr/local/bin/bun".into()]).find(|p| p.is_file())
}

/// A `ts` script run with Bun, in a folder of its own: the lines it wrote, or why it failed.
fn bun(name: &str, code: &str, context: &str) -> Result<Vec<String>, String> {
    use std::process::{Command, Stdio};
    let bun = bun_path().ok_or("bun is not installed (looked on the PATH, in ~/.bun/bin and Homebrew's): v0 runs ```ts with Bun")?;
    let dir = std::env::temp_dir().join(format!("cells-run-{}-{name}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join("script.ts"), code).map_err(|e| e.to_string())?;
    fs::write(dir.join("runner.ts"), RUNNER).map_err(|e| e.to_string())?;
    let child = Command::new(bun)
        .arg("run")
        .arg(dir.join("runner.ts"))
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("bun: {e}"));
    let out = child.and_then(|mut child| {
        child.stdin.take().expect("piped").write_all(context.as_bytes()).map_err(|e| e.to_string())?;
        child.wait_with_output().map_err(|e| e.to_string())
    });
    let _ = fs::remove_dir_all(&dir);
    let out = out?;
    let words = String::from_utf8_lossy(&out.stderr);
    for line in words.lines() {
        tracing::info!(target: crate::trace::ENGINE, script = name, line, "the script said");
    }
    if !out.status.success() {
        let why = words.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("the script failed").trim();
        return Err(why.to_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).lines().filter(|l| !l.trim().is_empty()).map(str::to_owned).collect())
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
            Err(e) => tracing::error!(target: crate::trace::ENGINE, why = %e, "the disk's change events failed"),
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

    fn list(&self, root: &Path, whole: bool) -> Vec<PathBuf> {
        fn walk(dir: &Path, whole: bool, out: &mut Vec<PathBuf>) {
            let Ok(entries) = fs::read_dir(dir) else { return };
            for e in entries.flatten() {
                let p = e.path();
                let hidden = p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
                if hidden {
                    continue;
                }
                if p.is_dir() {
                    if whole {
                        walk(&p, whole, out);
                    }
                } else if table_file(&p) {
                    out.push(p);
                }
            }
        }
        let mut out = Vec::new();
        walk(root, whole, &mut out);
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

    fn keep(&self, path: &Path, text: &str) -> Result<(), String> {
        if path.exists() {
            return Ok(());
        }
        let dir = path.parent().expect("a version is in a folder");
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        // written beside and renamed, so a version kept is never half of one
        let beside = dir.join(format!(".{}.{}", path.file_name().expect("a version has a name").to_string_lossy(), std::process::id()));
        let written = (|| -> std::io::Result<()> {
            let mut f = File::create(&beside)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
            fs::rename(&beside, path)
        })();
        written.map_err(|e| {
            let _ = fs::remove_file(&beside);
            format!("{}: {e}", path.display())
        })
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

    fn run_script(
        &self,
        name: &str,
        language: &str,
        code: &str,
        context: String,
        done: Box<dyn FnOnce(Result<Vec<String>, String>) + Send>,
    ) {
        if language != "ts" {
            return done(Err(format!("no runner runs ```{language}: v0 runs ```ts, with Bun")));
        }
        let (script, code) = (name.to_owned(), code.to_owned());
        let spawned = std::thread::Builder::new().name(format!("cells-run-{name}")).spawn(move || done(bun(&script, &code, &context)));
        if let Err(e) = spawned {
            tracing::error!(target: crate::trace::ENGINE, script = name, why = %e, "no thread to run a script");
        }
    }

    fn home(&self) -> PathBuf {
        let home = PathBuf::from(std::env::var_os("HOME").expect("HOME is set"));
        fs::canonicalize(&home).unwrap_or(home)
    }

    fn now(&self) -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).expect("the clock is after 1970").as_millis() as u64
    }

    fn bits(&self) -> u64 {
        let mut b = [0u8; 8];
        use std::io::Read as _;
        File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).expect("the kernel's random bytes");
        u64::from_le_bytes(b)
    }
}
