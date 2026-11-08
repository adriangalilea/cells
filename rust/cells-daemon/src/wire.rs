//! What the engine and its clients say to each other: one JSON object per line over the
//! engine's socket, so a person can read a conversation with `nc -U`. A client greets with
//! its version and who it is, asks for folders to be served, and hears every file whose
//! version changed, whoever changed it, the engine's own writes included.

use std::io::{BufRead, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The protocol's version: a client and an engine of different versions part at the
/// greeting, the client saying so, never guessing at what the other means.
pub const VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "say", rename_all = "lowercase")]
pub enum ToEngine {
    /// The first line: the protocol's version and the client's name (`lsp`, `app`, `cli`),
    /// for the trace.
    Hello { version: u32, client: String },
    /// A folder served from now on, everything under it, as long as the engine runs.
    Serve { folder: PathBuf },
    /// A script allowed to run, as its person saw it (`allowed`): the file, its name and
    /// the version shown. Sent only after the person said yes, where they saw the code.
    Allow { file: PathBuf, script: String, version: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "say", rename_all = "lowercase")]
pub enum FromEngine {
    Hello {
        version: u32,
        pid: u32,
    },
    /// A folder served, or one it was already serving.
    Served {
        folder: PathBuf,
    },
    /// A script's approval kept.
    Allowed {
        file: PathBuf,
        script: String,
    },
    /// A request refused, and why.
    Refused {
        why: String,
    },
    /// Files whose version changed: written by anyone, the engine's own writes included,
    /// removed, or added.
    Changed {
        paths: Vec<PathBuf>,
    },
}

/// `message` written as one line, flushed.
pub fn send<T: Serialize>(to: &mut impl Write, message: &T) -> std::io::Result<()> {
    let mut line = serde_json::to_string(message).expect("a message serializes");
    line.push('\n');
    to.write_all(line.as_bytes())?;
    to.flush()
}

/// The next line read as a message: nothing once the other side has closed, an error
/// for a line that is not one.
pub fn receive<T: for<'de> Deserialize<'de>>(from: &mut impl BufRead) -> std::io::Result<Option<T>> {
    let mut line = String::new();
    if from.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    serde_json::from_str(&line)
        .map(Some)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e}: {}", line.trim_end())))
}

/// Where the engine answers: `Cells/engine.sock` in the user's application support folder
/// (`~/Library/Application Support` on a Mac, `$XDG_STATE_HOME` or `~/.local/state`
/// elsewhere), one engine per user.
pub fn socket() -> PathBuf {
    folder().join("engine.sock")
}

/// The engine's own folder, private to its user: its socket, its lock, its log.
pub fn folder() -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").expect("HOME is set"));
    if cfg!(target_os = "macos") {
        return home.join("Library/Application Support/Cells");
    }
    match std::env::var_os("XDG_STATE_HOME") {
        Some(state) => PathBuf::from(state).join("cells"),
        None => home.join(".local/state/cells"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_a_message() {
        let mut out = Vec::new();
        send(&mut out, &ToEngine::Serve { folder: "/w".into() }).unwrap();
        assert_eq!(String::from_utf8(out.clone()).unwrap(), "{\"say\":\"serve\",\"folder\":\"/w\"}\n");
        let back: ToEngine = receive(&mut out.as_slice()).unwrap().unwrap();
        assert_eq!(back, ToEngine::Serve { folder: "/w".into() });
        assert!(receive::<ToEngine>(&mut b"".as_slice()).unwrap().is_none());
        assert!(receive::<ToEngine>(&mut b"{\"say\":\"shout\"}\n".as_slice()).is_err());
    }
}
