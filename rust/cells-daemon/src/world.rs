//! The daemon's one way to the world (ENGINE.md, Proving it): files read, walked and
//! replaced, lines appended to the log, folders watched and trashed, and the clock. `Disk`
//! is a Mac's; a simulation implements the same trait, so the daemon's every decision runs
//! the same against either.

use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::index::{Heard, icloud, visible};
use crate::reach::Protected;

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
    /// The system's ID of the file at `path` (its inode, which FSEvents names a file by),
    /// when one is there and the disk gives one.
    fn file_id(&self, path: &Path) -> Option<u64>;
    /// Every file a table is read from (`.cells`, `.csv`, `.tsv`) under `root` that the
    /// engine takes in (`index::visible`, by the home folder's rule; iCloud Drive's folder
    /// inside `~/Library` walked too), sorted; nothing under any of `beyond` (the protected
    /// folders not reached, `reach`), which the walk never enters.
    fn files(&self, root: &Path, beyond: &[PathBuf]) -> Vec<PathBuf>;
    /// Every log a folder kept its own history in before the engine kept one
    /// (`<folder>/.cells/log.jsonl`), of the home folder and every folder the walk takes
    /// in, never under `beyond`, sorted: what `migrate` reads once.
    fn folder_logs(&self, beyond: &[PathBuf]) -> Vec<PathBuf>;
    /// The protected folder `path` is in (`reach`), told from its words and the mount table
    /// alone, so nothing guarded is touched to tell; none for any other place.
    fn protected(&self, path: &Path) -> Option<Protected>;
    /// Every file directly in `dir`, sorted; none when there is no such folder.
    fn entries(&self, dir: &Path) -> Vec<PathBuf>;
    /// `text` written over `path` whole or not at all, and only over the version whose
    /// SHA-256 is `against`.
    fn replace(&self, path: &Path, against: &str, text: &str) -> Replaced;
    /// `text` written at `path` whole or not at all, whatever is there: the engine's own
    /// files (the index, the history written whole once).
    fn save(&self, path: &Path, text: &str) -> Result<(), String>;
    /// `line` appended to the file at `path`, and flushed to the disk.
    fn append(&self, path: &Path, line: &str) -> Result<(), String>;
    /// `text` written at `path` unless a file is there already: what is kept there is
    /// named by its own hash, so one there holds the same text.
    fn keep(&self, path: &Path, text: &str) -> Result<(), String>;
    /// `path` (a file or a folder) moved into the home folder's Trash as `name`, numbered
    /// when the Trash holds one of that name (`name 2`), so it is recoverable from the
    /// Finder and nothing is deleted: where it went.
    fn trash(&mut self, path: &Path, name: &str) -> Result<PathBuf, String>;
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
    /// The user's home folder, canonical: every `.cells` file under it is held.
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

/// Every file and every folder the walk takes in under `dir` (`index::visible` by `home`'s
/// rule), symbolic links never followed: a link is another name for a file held where it is.
fn walk(home: &Path, dir: &Path, beyond: &[PathBuf], dirs: &mut Vec<PathBuf>, files: &mut Vec<PathBuf>) {
    if beyond.iter().any(|b| dir.starts_with(b)) {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    dirs.push(dir.to_path_buf());
    for e in entries.flatten() {
        let p = e.path();
        if !visible(home, &p) || beyond.iter().any(|b| p.starts_with(b)) {
            continue;
        }
        match e.file_type() {
            Ok(t) if t.is_dir() => walk(home, &p, beyond, dirs, files),
            Ok(t) if t.is_file() && table_file(&p) => files.push(p),
            _ => {}
        }
    }
}

/// The walk of `root`, and of iCloud Drive's folder when `root` holds it (inside
/// `~/Library`, which the walk leaves out), never into `beyond`.
fn walked(home: &Path, root: &Path, beyond: &[PathBuf]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let (mut dirs, mut files) = (Vec::new(), Vec::new());
    walk(home, root, beyond, &mut dirs, &mut files);
    let cloud = icloud(home);
    if cloud.starts_with(root) && cloud != root {
        walk(home, &cloud, beyond, &mut dirs, &mut files);
    }
    dirs.sort();
    files.sort();
    (dirs, files)
}

/// The protected volume `path` is on (`reach::volume`), from the kernel's mount table
/// (`getmntinfo`), which touches no volume: the mount whose folder is the longest that holds
/// `path`.
#[cfg(target_os = "macos")]
fn mounted(path: &Path) -> Option<Protected> {
    let mut table: *mut libc::statfs = std::ptr::null_mut();
    // SAFETY: getmntinfo points `table` at the kernel's mount table, owned by libc, valid
    // until the next call on this thread; read here and copied out at once
    let count = unsafe { libc::getmntinfo(&mut table, libc::MNT_NOWAIT) };
    if count <= 0 || table.is_null() {
        return None;
    }
    // SAFETY: `count` entries from `table`, as getmntinfo says
    let mounts = unsafe { std::slice::from_raw_parts(table, count as usize) };
    mounts
        .iter()
        .map(|m| {
            // SAFETY: f_mntonname is a NUL-terminated C string within its array
            let at = unsafe { std::ffi::CStr::from_ptr(m.f_mntonname.as_ptr()) }.to_string_lossy().into_owned();
            (at, m.f_flags & libc::MNT_LOCAL as u32 != 0)
        })
        .filter(|(at, _)| path.starts_with(at))
        .max_by_key(|(at, _)| at.len())
        .and_then(|(at, local)| crate::reach::volume(&at, local))
}

#[cfg(not(target_os = "macos"))]
fn mounted(_: &Path) -> Option<Protected> {
    None
}

/// `text` written beside `path` and renamed over it, so it is never half of one.
fn written(path: &Path, text: &str) -> Result<(), String> {
    let dir = path.parent().expect("a file is in a folder");
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let beside = dir.join(format!(".{}.{}", path.file_name().expect("a file has a name").to_string_lossy(), std::process::id()));
    let done = (|| -> std::io::Result<()> {
        let mut f = File::create(&beside)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        fs::rename(&beside, path)
    })();
    done.map_err(|e| {
        let _ = fs::remove_file(&beside);
        format!("{}: {e}", path.display())
    })
}

/// Who hears the home folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ears {
    /// The engine, through `notify`: `cells engine` run from a shell, or on a system with no
    /// host app.
    Own,
    /// The host app (`CellsEngine.app`'s FSEvents streams, `hosted`): the home folder
    /// without `~/Library`, and iCloud Drive's folder in it (`host_hears`). Only a folder
    /// outside those is watched through `notify`.
    Host,
}

/// Whether the host app hears `dir` (`Ears::Host`): `Engine/Hearing.swift` streams the home
/// folder without `~/Library`, and `~/Library/Mobile Documents`.
pub fn host_hears(home: &Path, dir: &Path) -> bool {
    dir.starts_with(home) && (!dir.starts_with(home.join("Library")) || dir.starts_with(icloud(home)))
}

pub struct Disk {
    watcher: RecommendedWatcher,
    ears: Ears,
}

impl Disk {
    /// The disk, every path it says changed under a folder watched handed to `heard`, on the
    /// watcher's own thread, a folder the host hears for it (`ears`) left to the host.
    /// `notify` names no file ID (its FSEvents backend asks for no extended data and says a
    /// rename's two sides apart), so a move heard through it is followed by the IDs the
    /// index holds and by text (`index::follow`, rules 2 to 5): the fallback where no host
    /// hands the engine FSEvents' own events. A file iCloud keeps only
    /// in the cloud (dataless, as `~/Documents` holds them under Optimize Mac Storage) is
    /// fetched when read: macOS fetches one for an app by default but refuses a background
    /// agent (EDEADLK, "Resource deadlock avoided") unless the process asks, which this does
    /// for the whole process.
    pub fn new(heard: impl Fn(Heard) + Send + 'static, ears: Ears) -> Result<Disk, String> {
        #[cfg(target_os = "macos")]
        {
            // <sys/resource.h>; not in the libc crate
            const IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES: libc::c_int = 3;
            const IOPOL_SCOPE_PROCESS: libc::c_int = 0;
            const IOPOL_MATERIALIZE_DATALESS_FILES_ON: libc::c_int = 2;
            unsafe extern "C" {
                fn setiopolicy_np(kind: libc::c_int, scope: libc::c_int, policy: libc::c_int) -> libc::c_int;
            }
            // SAFETY: a plain syscall wrapper over three integers
            let set = unsafe {
                setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_ON)
            };
            if set != 0 {
                return Err(format!("iCloud's files cannot be fetched when read: {}", std::io::Error::last_os_error()));
            }
        }
        let watcher = notify::recommended_watcher(move |e: notify::Result<notify::Event>| match e {
            Ok(e) if !e.kind.is_access() => e.paths.into_iter().for_each(|path| heard(Heard { path, id: None })),
            Ok(_) => {}
            Err(e) => tracing::error!(target: crate::trace::ENGINE, why = %e, "the disk's change events failed"),
        })
        .map_err(|e| format!("no file watcher: {e}"))?;
        Ok(Disk { watcher, ears })
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

    fn file_id(&self, path: &Path) -> Option<u64> {
        use std::os::unix::fs::MetadataExt;
        fs::metadata(path).ok().map(|m| m.ino())
    }

    fn files(&self, root: &Path, beyond: &[PathBuf]) -> Vec<PathBuf> {
        walked(&self.home(), root, beyond).1
    }

    fn folder_logs(&self, beyond: &[PathBuf]) -> Vec<PathBuf> {
        let home = self.home();
        let (dirs, _) = walked(&home, &home, beyond);
        dirs.into_iter().map(|d| d.join(".cells").join("log.jsonl")).filter(|l| l.is_file()).collect()
    }

    fn protected(&self, path: &Path) -> Option<Protected> {
        crate::reach::in_home(&self.home(), path).or_else(|| mounted(path))
    }

    fn entries(&self, dir: &Path) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
        let mut out: Vec<PathBuf> = entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_file())).map(|e| e.path()).collect();
        out.sort();
        out
    }

    fn save(&self, path: &Path, text: &str) -> Result<(), String> {
        written(path, text)
    }

    /// A rename into `~/.Trash`, as the Finder's Move to Trash makes on the home volume: the
    /// engine is a library and asks no app (NSFileManager's `trashItem`) to do it. A folder
    /// on another volume is not moved, and the rename's failure says so.
    fn trash(&mut self, path: &Path, name: &str) -> Result<PathBuf, String> {
        let trash = self.home().join(".Trash");
        fs::create_dir_all(&trash).map_err(|e| format!("{}: {e}", trash.display()))?;
        let to = (1..).map(|n| trash.join(if n == 1 { name.to_owned() } else { format!("{name} {n}") })).find(|p| !p.exists());
        let to = to.expect("a name free in the Trash");
        fs::rename(path, &to).map_err(|e| format!("{} into the Trash: {e}", path.display()))?;
        Ok(to)
    }

    fn replace(&self, path: &Path, against: &str, text: &str) -> Replaced {
        let Ok(now) = fs::read(path) else { return Replaced::Moved };
        if cells::sha256::hex(&now) != against {
            return Replaced::Moved;
        }
        let dir = path.parent().expect("a file is in a folder");
        let name = path.file_name().expect("a file has a name").to_string_lossy();
        let beside = dir.join(format!(".{name}.cells-engine-{}", std::process::id()));
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
        written(path, text)
    }

    fn watch(&mut self, dir: &Path, whole: bool) -> Result<(), String> {
        if self.ears == Ears::Host && host_hears(&self.home(), dir) {
            return Ok(());
        }
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
