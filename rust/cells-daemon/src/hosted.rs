//! The engine as a host app runs it: three C functions, declared in `include/cells_engine.h`.
//! Cells.app's `CellsEngine.app` is a Swift `main` that watches its own image (swift-utils'
//! `AppInstallations`, which every process of an app links), hears the person's files
//! (FSEvents, `Engine/Hearing.swift`) and calls `cells_engine_serve`, so Swift owns the app's
//! life on the Mac and the platform's word on what happened to a file, and Rust owns the
//! engine, its folder and its log included. What the engine says goes to the host's
//! `record`, one JSON object a call (`trace`), so the host writes it as its own log does.
//!
//! What the host hears reaches the daemon through `cells_engine_heard`, a batch at a time,
//! from the moment the host starts hearing: batches before the daemon runs wait in its
//! channel, and are heard once it has walked home at its start.

use std::ffi::{CStr, c_char};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{LazyLock, Mutex};

use serde::Deserialize;

use crate::daemon::Event;
use crate::index::{Heard, Index};
use crate::world::table_file;

/// The daemon's one channel in a hosted engine: the host's batches sent from its own queue
/// from before the daemon runs, which takes the receiver when it starts (`hosted`).
static CHANNEL: LazyLock<Channel> = LazyLock::new(|| {
    let (events, heard) = channel();
    Channel { events, heard: Mutex::new(Some(heard)) }
});

struct Channel {
    events: Sender<Event>,
    /// Until the daemon takes it.
    heard: Mutex<Option<Receiver<Event>>>,
}

/// The daemon's channel: its sender, and its receiver, which only the one daemon a process
/// runs takes.
pub(crate) fn channel_taken() -> (Sender<Event>, Receiver<Event>) {
    let heard = CHANNEL.heard.lock().expect("the channel's lock").take().expect("one engine a process");
    (CHANNEL.events.clone(), heard)
}

/// One batch the host heard, as `Hearing.swift` writes it:
/// `{"events":[{"n":412,"path":"/Users/a/x.cells","file":8812,"flags":["renamed","file"]}]}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub events: Vec<Said>,
}

/// One event: its number (FSEvents' event ID, rising), its path, the system's ID of the
/// file it is about (absent on the stream's own events), and its flags in words.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Said {
    pub n: u64,
    pub path: PathBuf,
    #[serde(default)]
    pub file: Option<u64>,
    pub flags: Vec<String>,
}

/// Every word `Hearing.swift` says a flag by.
const WORDS: [&str; 19] = [
    "created",
    "removed",
    "renamed",
    "modified",
    "meta",
    "xattr",
    "cloned",
    "file",
    "dir",
    "symlink",
    "hardlink",
    "own",
    "scan",
    "dropped",
    "wrapped",
    "history-done",
    "root-changed",
    "mount",
    "unmount",
];

/// A batch the host heard as the daemon hears it: each table file's event with the file ID
/// it named; a folder's when it came, went or moved (what is in it is looked at), never
/// when only its own metadata changed; every folder whose events were lost (FSEvents asks
/// for a scan of it, dropped events, its root moved, its numbering wrapped, a volume came or
/// went) looked at whole. Other files' events say nothing to the engine. The engine's own
/// writes (`own`) are heard like any: an event coalesces another program's write to the
/// same path with it, and a file whose text is what the engine holds changes nothing.
/// Flags coalesce, so nothing is decided from their combination: what is there now is read.
pub fn decoded(json: &str) -> Result<Event, String> {
    let batch: Batch = serde_json::from_str(json).map_err(|e| format!("no batch of events: {e}"))?;
    let n = batch.events.last().ok_or("a batch of no events")?.n;
    let (mut heard, mut lost) = (Vec::new(), Vec::new());
    for said in batch.events {
        if let Some(word) = said.flags.iter().find(|w| !WORDS.contains(&w.as_str())) {
            return Err(format!("{}: no flag is said {word:?}", said.path.display()));
        }
        if !said.path.is_absolute() {
            return Err(format!("{}: an event's path is absolute", said.path.display()));
        }
        let has = |w: &str| said.flags.iter().any(|f| f == w);
        if ["scan", "dropped", "root-changed", "wrapped", "mount", "unmount"].into_iter().any(has) {
            lost.push(said.path);
        } else if has("file") && table_file(&said.path) {
            heard.push(Heard { path: said.path, id: said.file });
        } else if has("dir") && ["created", "removed", "renamed"].into_iter().any(has) {
            heard.push(Heard { path: said.path, id: None });
        }
    }
    Ok(Event::Host { heard, lost, n })
}

/// The engine, until the process ends; returns only when it cannot run, its reason handed
/// to `record` as an error, with the exit status a launchd job should end with. `reach` is
/// the protected folders the host's process may enter, as it read them at its launch
/// (`reach::Reach::parse`, a JSON list of words): every other is kept out of until a client
/// says otherwise.
///
/// # Safety
/// `reach` is a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_engine_serve(reach: *const c_char, record: extern "C" fn(*const c_char)) -> i32 {
    assert!(!reach.is_null(), "the host handed no reach");
    // SAFETY: the caller hands a NUL-terminated string, valid for the call
    let reach = unsafe { CStr::from_ptr(reach) }.to_str().expect("the host's reach is UTF-8");
    let reach = crate::Reach::parse(reach).unwrap_or_else(|why| panic!("the host's reach: {why}"));
    match crate::hosted(record, reach) {
        Ok(never) => match never {},
        Err(why) => {
            tracing::error!(target: crate::trace::ENGINE, why, "cannot run");
            2
        }
    }
}

/// The last event the engine kept as handled (`Index::heard`), where the host starts
/// hearing: 0 when it kept none, or keeps no index the engine can read (the engine's start
/// says why), and then the host hears from now on and the engine's walk at its start finds
/// what changed before.
#[unsafe(no_mangle)]
pub extern "C" fn cells_engine_heard_since() -> u64 {
    let Ok(text) = std::fs::read_to_string(crate::index::path()) else { return 0 };
    Index::parse(&text).map_or(0, |index| index.heard())
}

/// One batch the host heard (`Batch`, JSON, valid only during the call), handed to the
/// daemon in the order heard; waiting in its channel until it runs. A batch that is none
/// is a host and an engine that disagree: the process ends saying so.
///
/// # Safety
/// `json` is a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_engine_heard(json: *const c_char) {
    assert!(!json.is_null(), "the host handed no batch");
    // SAFETY: the caller hands a NUL-terminated string, valid for the call
    let json = unsafe { CStr::from_ptr(json) }.to_str().expect("the host's batch is UTF-8");
    let event = decoded(json).unwrap_or_else(|why| panic!("the host's batch: {why}: {json}"));
    // the receiver lives as long as the process once the daemon runs
    CHANNEL.events.send(event).expect("the daemon's channel is open");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard(event: &Event) -> (&[Heard], &[PathBuf], u64) {
        match event {
            Event::Host { heard, lost, n } => (heard, lost, *n),
            _ => panic!("a host's batch"),
        }
    }

    #[test]
    fn a_batch_is_what_the_daemon_hears() {
        let json = r#"{"events":[
            {"n":10,"path":"/h/a.cells","file":7,"flags":["renamed","file"]},
            {"n":11,"path":"/h/b/a.cells","file":7,"flags":["renamed","file","own"]},
            {"n":12,"path":"/h/notes.txt","file":8,"flags":["created","modified","file"]},
            {"n":13,"path":"/h/w","file":9,"flags":["meta","dir"]},
            {"n":14,"path":"/h/trip","file":10,"flags":["renamed","dir"]},
            {"n":15,"path":"/h/Music","flags":["scan","dropped"]},
            {"n":16,"path":"/h","flags":["history-done"]}
        ]}"#;
        let event = decoded(json).unwrap();
        let (h, lost, n) = heard(&event);
        assert_eq!(
            h,
            [
                Heard { path: "/h/a.cells".into(), id: Some(7) },
                Heard { path: "/h/b/a.cells".into(), id: Some(7) },
                Heard { path: "/h/trip".into(), id: None },
            ],
            "a table file's events with its ID, a folder moved; not a text file, not a folder's metadata"
        );
        assert_eq!(lost, [PathBuf::from("/h/Music")]);
        assert_eq!(n, 16, "the last event's number");
    }

    #[test]
    fn a_batch_that_is_none_is_refused() {
        let refused = |json: &str| match decoded(json) {
            Err(why) => why,
            Ok(_) => panic!("{json} taken"),
        };
        assert!(refused(r#"{"events":[]}"#).contains("no events"));
        assert!(refused(r#"{"events":[{"n":1,"path":"/h/a.cells","flags":["exploded"]}]}"#).contains("exploded"));
        assert!(refused(r#"{"events":[{"n":1,"path":"a.cells","flags":["file"]}]}"#).contains("absolute"));
        assert!(refused(r#"{"events":[{"n":1,"path":"/a.cells","flags":[],"inode":3}]}"#).contains("inode"));
    }
}
