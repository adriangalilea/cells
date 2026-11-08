//! The engine kept running (ENGINE.md, v0): `cells serve`, one per user.
//!
//! A change written to a file by anything (an editor's save, a script, git) is heard, and
//! every computed cell it reaches is stored, in the file itself and in every file that reads
//! it through its imports, so a folder's files stay one consistent set of books with nobody
//! opening the files that follow. Each version of each file is logged (`.cells/log.jsonl`)
//! and each step said (stderr). Clients (the language server, the command line, Cells.app)
//! reach it over its socket, ask for folders to be served, and hear every file that changes.
//!
//! Each part owns one thing:
//!
//! - `cells::folder`, in the core: what a change reaches and what to store, decided from
//!   the changes handed in alone, with no disk, clock or thread;
//! - `world`: the daemon's one way to the disk, the clock and change events, `Disk` on a
//!   Mac and a simulated world in the tests;
//! - `daemon`: the two joined, one thread taking every event in order (the disk's changes,
//!   the clients' requests), what was decided carried out, with `log` and `trace` saying
//!   what happened and why;
//! - `wire`, `server` and `client`: the socket, the conversation over it, and its two ends.

pub mod allowed;
pub mod client;
pub mod daemon;
pub mod history;
pub mod hosted;
pub mod log;
pub mod schedule;
pub mod server;
pub mod trace;
pub mod trust;
pub mod wire;
pub mod world;

use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

pub use client::Engine;
pub use daemon::{Daemon, Event};
pub use world::{Disk, World};

use server::{Held, Listening, Unbound};

/// The engine run over its socket until the process ends, serving `folders` first.
fn run(
    listening: Listening,
    events: std::sync::mpsc::Sender<Event>,
    heard: Receiver<Event>,
    folders: &[PathBuf],
) -> Result<Infallible, String> {
    let disk = events.clone();
    let world = Disk::new(move |path| {
        // fails only once the daemon is gone
        let _ = disk.send(Event::Disk(path));
    })?;
    let mut daemon = Daemon::new(world, events.clone());
    tracing::info!(target: trace::ENGINE, socket = %listening.socket.display(), pid = std::process::id(), admits = listening.admits, "listening");
    for folder in folders {
        daemon.serve(folder)?;
    }
    // the socket and its lock live as long as the daemon, which never returns
    let _listening = listening;
    daemon.run(heard)
}

/// The engine as Cells.app keeps it (`CellsEngine.app`, a launch agent): the lock waited
/// for when another process holds it, so launchd's keeping it alive never turns into
/// restarting it against an engine a client started first. Every record goes to `sink`,
/// the host's writer (`trace`).
pub fn hosted(sink: trace::Sink) -> Result<Infallible, String> {
    trace::to_host(sink);
    // launchd hands a job no terminal: what the host writes on stderr goes to
    // `engine.trace` beside the socket, where a client starting the engine sends it too
    let folder = wire::folder();
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let trace_path = wire::trace();
    let trace =
        std::fs::OpenOptions::new().create(true).append(true).open(&trace_path).map_err(|e| format!("{}: {e}", trace_path.display()))?;
    use std::os::fd::AsRawFd;
    // SAFETY: dup2 points the process's stdout and stderr at the trace's descriptor, which
    // stays open as long as `trace`, held below for the process's life.
    for fd in [1, 2] {
        assert!(unsafe { libc::dup2(trace.as_raw_fd(), fd) } == fd, "stdio onto {}", trace_path.display());
    }
    let _trace = trace;
    let (events, heard) = channel();
    match server::listen(events.clone(), Held::Wait) {
        Ok(listening) => run(listening, events, heard, &[]),
        Err(Unbound::Running) => unreachable!("a hosted engine waits for the lock"),
        Err(Unbound::Failed(why)) => Err(why),
    }
}

pub const USAGE: &str = "cells serve [--verbose] [<folder>...]";

/// `cells serve [--verbose] [<folder>...]`: the engine, serving the folders named and every
/// folder a client asks for, until the process ends. With an engine running already, the
/// folders are handed to it and this returns.
pub fn serve(args: &[String]) -> Result<Infallible, String> {
    let verbose = args.iter().any(|a| a == "--verbose");
    trace::to_stderr(verbose);
    let named: Vec<&String> = args.iter().filter(|a| *a != "--verbose").collect();
    if named.iter().any(|a| a.starts_with("--")) {
        return Err(USAGE.to_owned());
    }
    let mut folders: Vec<PathBuf> = Vec::new();
    for f in named {
        let folder = std::fs::canonicalize(f).map_err(|e| format!("{f}: {e}"))?;
        if !folder.is_dir() {
            return Err(format!("{f}: not a folder"));
        }
        folders.push(folder);
    }
    let (events, heard) = channel();
    match server::listen(events.clone(), Held::Leave) {
        Ok(listening) => run(listening, events, heard, &folders),
        Err(Unbound::Running) => {
            let (engine, _) = Engine::connect("cli")?;
            if folders.is_empty() {
                return Err(format!("the engine runs already (pid {}); name a folder to have it served", engine.pid));
            }
            for folder in &folders {
                let served = engine.serve(folder)?;
                println!("{}: served by the engine (pid {})", served.display(), engine.pid);
            }
            std::process::exit(0)
        }
        Err(Unbound::Failed(why)) => Err(why),
    }
}

#[cfg(test)]
mod tests;
