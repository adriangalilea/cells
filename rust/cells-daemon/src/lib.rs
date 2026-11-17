//! The engine kept running (ENGINE.md, v0): `cells engine`, one per user.
//!
//! A change written to a file by anything (an editor's save, a script, git) is heard, and
//! every computed cell it reaches is stored, in the file itself and in every file that reads
//! it through its imports, so a person's files stay one consistent set of books with nobody
//! opening the files that follow, whichever folders they are in. The engine holds every
//! `.cells` file under the home folder, every file they import and every file a client
//! names, each by an identity that follows it as it moves (`index`). Each version of each
//! file is logged in one history of the engine's own (`log`) and each step said (stderr).
//! Clients (the language server, the command line, Cells.app) reach it over its socket and
//! hear every file that changes.
//!
//! Each part owns one thing:
//!
//! - `cells::folder`, in the core: what a change reaches and what to store, decided from
//!   the changes handed in alone, with no disk, clock or thread;
//! - `world`: the daemon's one way to the disk, the clock and change events, `Disk` on a
//!   Mac and a simulated world in the tests;
//! - `index`: which file is which, by identity, however it moved;
//! - `daemon`: the three joined, one thread taking every event in order (the disk's
//!   changes, the clients' requests), what was decided carried out, with `log` and `trace`
//!   saying what happened and why;
//! - `wire`, `server` and `client`: the socket, the conversation over it, and its two ends.

pub mod allowed;
pub mod client;
pub mod daemon;
pub mod history;
pub mod hosted;
pub mod index;
pub mod log;
pub mod migrate;
pub mod schedule;
pub mod server;
pub mod trace;
pub mod trust;
pub mod wire;
pub mod world;

use std::convert::Infallible;
use std::sync::mpsc::{Receiver, channel};

pub use client::Engine;
pub use daemon::{Daemon, Event};
pub use world::{Disk, World};

use server::{Held, Listening, Unbound};

/// The engine run over its socket until the process ends.
fn run(listening: Listening, events: std::sync::mpsc::Sender<Event>, heard: Receiver<Event>) -> Result<Infallible, String> {
    let disk = events.clone();
    let world = Disk::new(move |heard| {
        // fails only once the daemon is gone
        let _ = disk.send(Event::Disk(heard));
    })?;
    tracing::info!(target: trace::ENGINE, socket = %listening.socket.display(), pid = std::process::id(), admits = listening.admits, "listening");
    let mut daemon = Daemon::start(world, events.clone())?;
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
        Ok(listening) => run(listening, events, heard),
        Err(Unbound::Running) => unreachable!("a hosted engine waits for the lock"),
        Err(Unbound::Failed(why)) => Err(why),
    }
}

pub const USAGE: &str = "cells engine [--verbose]";

/// `cells engine [--verbose]`: the engine, until the process ends: every `.cells` file
/// under the home folder held, and every file they import and a client names. With an
/// engine running already, this says which and returns.
pub fn engine(args: &[String]) -> Result<Infallible, String> {
    let verbose = match args {
        [] => false,
        [v] if v == "--verbose" => true,
        _ => return Err(USAGE.to_owned()),
    };
    trace::to_stderr(verbose);
    let (events, heard) = channel();
    match server::listen(events.clone(), Held::Leave) {
        Ok(listening) => run(listening, events, heard),
        Err(Unbound::Running) => {
            let (engine, _) = Engine::connect("cli")?;
            Err(format!("the engine runs already (pid {})", engine.pid))
        }
        Err(Unbound::Failed(why)) => Err(why),
    }
}

#[cfg(test)]
mod tests;
