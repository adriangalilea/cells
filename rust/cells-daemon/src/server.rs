//! The engine's socket: where its clients reach it (`wire`). It is made so a client never
//! meets a half-made one: the engine's folder is private to its user, a lock says which
//! process is the engine, and the socket is bound under a name of its own, made private,
//! listening, then renamed into place, so the socket's name answers the moment it exists.
//!
//! Who may connect is `trust`'s: the kernel's word for the process at the other end, held
//! to the engine's own signature, never the caller's say.

use std::fs::{self, File, Permissions, TryLockError};
use std::io::BufReader;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::daemon::Event;
use crate::trust::Trust;
use crate::wire::{self, FromEngine, ToEngine, VERSION};

/// The engine's socket, listening. Its lock is held while it lives: the one engine of this
/// user is the process holding it.
pub struct Listening {
    _lock: File,
    pub socket: PathBuf,
    /// Who it admits, as the trace says it.
    pub admits: String,
}

/// Why the socket could not be made.
pub enum Unbound {
    /// Another process holds the engine's lock: the engine runs already.
    Running,
    Failed(String),
}

/// What a process does when another holds the engine's lock.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// Returns `Unbound::Running`: `cells serve` typed by a person, or started by a client,
    /// hands its folders to the engine running.
    Leave,
    /// Waits until it is free, then is the engine: Cells.app's own engine, kept by
    /// launchd, outwaiting one a client started before it, never restarting against it.
    Wait,
}

/// The socket made and listening, each client's requests handed to the daemon as events.
pub fn listen(events: Sender<Event>, held: Held) -> Result<Listening, Unbound> {
    let folder = wire::folder();
    let failed = |what: &str, e: std::io::Error| Unbound::Failed(format!("{what}: {e}"));
    fs::create_dir_all(&folder).map_err(|e| failed(&folder.display().to_string(), e))?;
    fs::set_permissions(&folder, Permissions::from_mode(0o700)).map_err(|e| failed(&folder.display().to_string(), e))?;
    let lock_path = folder.join("engine.lock");
    let lock = File::create(&lock_path).map_err(|e| failed(&lock_path.display().to_string(), e))?;
    match (lock.try_lock(), held) {
        (Ok(()), _) => {}
        (Err(TryLockError::WouldBlock), Held::Leave) => return Err(Unbound::Running),
        (Err(TryLockError::WouldBlock), Held::Wait) => lock.lock().map_err(|e| failed(&lock_path.display().to_string(), e))?,
        (Err(TryLockError::Error(e)), _) => return Err(failed(&lock_path.display().to_string(), e)),
    }
    let socket = wire::socket();
    let staged = folder.join(format!("engine.sock.{}", std::process::id()));
    let _ = fs::remove_file(&staged);
    let listener = UnixListener::bind(&staged).map_err(|e| failed(&staged.display().to_string(), e))?;
    fs::set_permissions(&staged, Permissions::from_mode(0o600)).map_err(|e| failed(&staged.display().to_string(), e))?;
    // holding the lock, any socket at the name is a dead engine's
    fs::rename(&staged, &socket).map_err(|e| failed(&socket.display().to_string(), e))?;
    let trust = Trust::own();
    let admits = trust.said();
    thread::Builder::new()
        .name("cells-accept".into())
        .spawn(move || accept(listener, events, trust))
        .map_err(|e| failed("the accepting thread", e))?;
    Ok(Listening { _lock: lock, socket, admits })
}

fn accept(listener: UnixListener, events: Sender<Event>, trust: Trust) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cells serve: a connection failed: {e}");
                continue;
            }
        };
        if let Err(why) = trust.admits(&stream) {
            eprintln!("cells serve: a connection refused, closed: {why}");
            // the refused program is told why (it is this user's: nothing the person
            // cannot read already), and its greeting is never read
            let mut told = stream;
            let _ = wire::send(
                &mut told,
                &FromEngine::Refused { why: format!("this engine admits Cells and its signed `cells` command only: {why}") },
            );
            continue;
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let events = events.clone();
        let spawned = thread::Builder::new().name(format!("cells-client-{id}")).spawn(move || {
            if let Err(e) = converse(id, stream, &events) {
                eprintln!("cells serve: client #{id}: {e}");
            }
            // the daemon outlives every client
            let _ = events.send(Event::Leave { id });
        });
        if let Err(e) = spawned {
            eprintln!("cells serve: no thread for client #{id}: {e}");
        }
    }
}

/// One client: its greeting answered, the files that change told to it as they change,
/// each request handed to the daemon and its answer written back, until it closes.
fn converse(id: u64, stream: UnixStream, events: &Sender<Event>) -> Result<(), String> {
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let writer = Arc::new(Mutex::new(stream));
    let say = |m: &FromEngine| wire::send(&mut *writer.lock().unwrap(), m).map_err(|e| e.to_string());
    let client = match wire::receive(&mut reader).map_err(|e| e.to_string())? {
        Some(ToEngine::Hello { version, client }) if version == VERSION => client,
        Some(ToEngine::Hello { version, .. }) => {
            say(&FromEngine::Refused { why: format!("this engine speaks version {VERSION}, the client {version}") })?;
            return Ok(());
        }
        Some(other) => return Err(format!("its first line is no greeting: {other:?}")),
        None => return Ok(()),
    };
    say(&FromEngine::Hello { version: VERSION, pid: std::process::id() })?;
    let (tell, told) = channel::<Vec<PathBuf>>();
    events.send(Event::Listen { id, client: client.clone(), tell }).map_err(|_| "the daemon is gone")?;
    let teller = Arc::clone(&writer);
    thread::Builder::new()
        .name(format!("cells-tell-{id}"))
        .spawn(move || {
            for paths in told {
                if wire::send(&mut *teller.lock().unwrap(), &FromEngine::Changed { paths }).is_err() {
                    return;
                }
            }
        })
        .map_err(|e| e.to_string())?;
    while let Some(request) = wire::receive(&mut reader).map_err(|e| e.to_string())? {
        if matches!(request, ToEngine::Hello { .. }) {
            return Err("a second greeting".into());
        }
        let request = match canonical(request) {
            Ok(r) => r,
            Err(why) => {
                say(&FromEngine::Refused { why })?;
                continue;
            }
        };
        if matches!(request, ToEngine::Editing { .. }) {
            events.send(Event::Ask { id, client: client.clone(), request, answer: None }).map_err(|_| "the daemon is gone")?;
            continue;
        }
        let (answer, answered) = channel();
        events.send(Event::Ask { id, client: client.clone(), request, answer: Some(answer) }).map_err(|_| "the daemon is gone")?;
        say(&answered.recv().map_err(|_| "the daemon is gone")?)?;
    }
    Ok(())
}

/// A request's paths as the engine keys files: canonical, as the disk resolves them (a
/// file not there yet, by its folder's), so `/tmp` and `/private/tmp` are one file.
fn canonical(request: ToEngine) -> Result<ToEngine, String> {
    fn path(p: PathBuf) -> Result<PathBuf, String> {
        match fs::canonicalize(&p) {
            Ok(c) => Ok(c),
            Err(_) => {
                let (dir, name) =
                    (p.parent().ok_or(format!("{}: no folder", p.display()))?, p.file_name().ok_or(format!("{}: no file", p.display()))?);
                Ok(fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?.join(name))
            }
        }
    }
    Ok(match request {
        ToEngine::Hello { .. } => request,
        ToEngine::Serve { folder } => {
            let folder = fs::canonicalize(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
            if !folder.is_dir() {
                return Err(format!("{}: not a folder", folder.display()));
            }
            ToEngine::Serve { folder }
        }
        ToEngine::Allow { file, script, version } => ToEngine::Allow { file: path(file)?, script, version },
        ToEngine::Editing { file, version } => ToEngine::Editing { file: path(file)?, version },
        ToEngine::History { folder, file, limit } => ToEngine::History { folder: path(folder)?, file: file.map(path).transpose()?, limit },
        ToEngine::Cell { file, row, column } => ToEngine::Cell { file: path(file)?, row, column },
        ToEngine::Version { file, version } => ToEngine::Version { file: path(file)?, version },
        ToEngine::Restore { file, version } => ToEngine::Restore { file: path(file)?, version },
    })
}
