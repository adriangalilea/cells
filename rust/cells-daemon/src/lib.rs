//! The engine kept running over folders of `.cells` files (ENGINE.md, v0): `cells serve`.
//!
//! A change written to a file by anything (an editor's save, a script, git) is heard, and
//! every computed cell it reaches is stored, in the file itself and in every file that reads
//! it through its imports, so a folder's files stay one consistent set of books with nobody
//! opening the files that follow. Each version of each file is logged (`.cells/log.jsonl`)
//! and each step said (stderr).
//!
//! Three parts, each owning one thing:
//!
//! - `cells::folder`, in the core: what a change reaches and what to store, decided from
//!   the changes handed in alone, with no disk, clock or thread;
//! - `world`: the daemon's one way to the disk, the clock and change events, `Disk` on a
//!   Mac and a simulated world in the tests;
//! - `daemon`: the two joined, what was heard handed in and what was decided carried out,
//!   with `log` and `trace` saying what happened and why.

pub mod daemon;
pub mod log;
pub mod trace;
pub mod world;

use std::convert::Infallible;
use std::path::PathBuf;

pub use daemon::Daemon;
pub use trace::Trace;
pub use world::{Disk, World};

pub const USAGE: &str = "cells serve [--verbose] <folder>...";

/// `cells serve [--verbose] <folder>...`: the folders served until the process ends.
pub fn serve(args: &[String]) -> Result<Infallible, String> {
    let verbose = args.iter().any(|a| a == "--verbose");
    let folders: Vec<&String> = args.iter().filter(|a| *a != "--verbose").collect();
    if folders.is_empty() || folders.iter().any(|a| a.starts_with("--")) {
        return Err(USAGE.to_owned());
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    for f in folders {
        let root = std::fs::canonicalize(f).map_err(|e| format!("{f}: {e}"))?;
        if !root.is_dir() {
            return Err(format!("{f}: not a folder"));
        }
        if roots.iter().any(|r| root.starts_with(r) || r.starts_with(&root)) {
            return Err(format!("{f}: inside another folder served, or holding one"));
        }
        roots.push(root);
    }
    let mut daemon = Daemon::start(Disk::new()?, &roots, Trace::new(verbose))?;
    daemon.run()
}

#[cfg(test)]
mod tests;
