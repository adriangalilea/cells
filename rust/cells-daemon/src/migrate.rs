//! The histories folders kept of their own (`<folder>/.cells/log.jsonl`, `versions/`)
//! moved into the engine's one, once, at the first start of an engine that keeps one: every
//! such log under home read (its chain verified, a break said and read past), each line's
//! file given an identity (a `relocated` line joins its two paths into one file, a file
//! removed and found again elsewhere included), every line merged into one chain by time
//! (each log's own order kept, a root before what follows it at the same moment), numbered
//! and its cause numbered anew, every version copied, the index written; then each `.cells`
//! folder moved to the Trash, recoverable, nothing deleted. A start that finds the one
//! history there never looks again.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::index::{FileId, Index, Known};
use crate::log::{self, Cause, Line, Logged, hash};
use crate::trace::ENGINE;
use crate::world::{Found, World};

/// A line as a folder's log wrote it: its paths from that folder (or whole, for a file of
/// another), its cause naming the other folder's log when the root was logged there.
#[derive(Deserialize)]
struct Old {
    n: u64,
    prev: String,
    at: u64,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// A line of a folder's log read: its entry with every path whole, and the root it follows
/// as that folder and its number there.
struct Read {
    at: u64,
    entry: Logged,
    cause: Option<(PathBuf, u64)>,
}

/// `p`, as a log in `root` wrote it, whole.
fn whole(root: &Path, p: &str) -> String {
    root.join(p).to_str().expect("a path the engine wrote is UTF-8").to_owned()
}

/// A folder's line read whole.
fn read(root: &Path, old: Old) -> Result<Read, String> {
    let mut m = old.rest;
    let cause = match m.get_mut("cause") {
        None => None,
        Some(Value::Object(c)) => {
            let folder = match c.remove("folder") {
                None => root.to_path_buf(),
                Some(Value::String(f)) => PathBuf::from(f),
                Some(other) => return Err(format!("line {}: a cause's folder is no path: {other}", old.n)),
            };
            let n = c.get("n").and_then(Value::as_u64).ok_or(format!("line {}: a cause names no line", old.n))?;
            Some((folder, n))
        }
        Some(other) => return Err(format!("line {}: a cause is no line: {other}", old.n)),
    };
    for k in ["path", "from"] {
        if let Some(Value::String(p)) = m.get_mut(k) {
            *p = whole(root, p);
        }
    }
    if let Some(Value::Array(via)) = m.get_mut("via") {
        for v in via {
            if let Value::String(p) = v {
                *p = whole(root, p);
            }
        }
    }
    let entry: Logged = serde_json::from_value(Value::Object(m)).map_err(|e| format!("line {}: {e}", old.n))?;
    Ok(Read { at: old.at, entry, cause })
}

/// What a migration did, as the trace says it.
pub struct Migrated {
    pub folders: usize,
    pub lines: usize,
    pub files: usize,
}

/// Every folder's history under home moved into the one, the one written (an empty log for
/// a home that kept none, so it is looked for once), the index of every file its lines name
/// with it. A folder's log the disk will not give, or a line that reads as no entry of a
/// log (a log of a kind no engine wrote), stops it before anything is written or moved:
/// the engine does not start, saying which, and the person's folders are as they were.
pub fn run(world: &mut impl World) -> Result<Migrated, String> {
    let logs = world.folder_logs();
    let mut read_logs: Vec<(PathBuf, Vec<Read>)> = Vec::new();
    for at in &logs {
        let root = at.parent().and_then(Path::parent).expect("a log in a folder's .cells").to_path_buf();
        let text = match world.read(at) {
            Found::Text(t) => t,
            Found::Missing => continue,
            Found::Unreadable(why) => return Err(format!("{}: {why}", at.display())),
        };
        let (lines, _, broken) = log::chain(&text, &at.display().to_string(), |l: &Old| &l.prev);
        if let Some(why) = broken {
            tracing::warn!(target: ENGINE, why, "a folder's history broken: read past the break, as it was read before");
        }
        let lines =
            lines.into_iter().map(|l| read(&root, l)).collect::<Result<Vec<_>, _>>().map_err(|e| format!("{}: {e}", at.display()))?;
        read_logs.push((root, lines));
    }
    // one chain by time, each log's order kept, a root before a line that follows one at
    // the same moment (a store in one folder of an edit logged in another)
    let mut heads = vec![0usize; read_logs.len()];
    let mut merged: Vec<(usize, usize)> = Vec::new();
    loop {
        let next = (0..read_logs.len()).filter(|&k| heads[k] < read_logs[k].1.len()).min_by_key(|&k| {
            let r = &read_logs[k].1[heads[k]];
            (r.at, r.cause.is_some(), k)
        });
        let Some(k) = next else { break };
        merged.push((k, heads[k]));
        heads[k] += 1;
    }
    let mut numbered: BTreeMap<(PathBuf, u64), u64> = BTreeMap::new();
    let mut present: BTreeMap<String, FileId> = BTreeMap::new();
    let mut gone: BTreeMap<String, FileId> = BTreeMap::new();
    let mut last: BTreeMap<FileId, (String, String)> = BTreeMap::new();
    let mut minted: BTreeSet<FileId> = BTreeSet::new();
    let mut lines: Vec<Line> = Vec::new();
    for (k, i) in merged {
        let (root, logged) = &read_logs[k];
        let r = &logged[i];
        let old_n = i as u64 + 1;
        let cause = match &r.cause {
            None => None,
            Some(c) => match numbered.get(c) {
                Some(n) => Some(*n),
                None => {
                    tracing::warn!(target: ENGINE, folder = %root.display(), line = old_n, root = c.1, "follows a line no log holds: left out, kept in its folder's history in the Trash");
                    continue;
                }
            },
        };
        let mut mint = || {
            let id = FileId::mint(|| world.bits(), |id| minted.contains(id));
            minted.insert(id.clone());
            id
        };
        let path = r.entry.path().to_owned();
        // the file a line is about: the one its path held, else the one gone from it, else
        // a file the log first names here; a move, the one its `from` held
        let id = match &r.entry {
            Logged::Relocated { from, .. } => present.remove(from).or_else(|| gone.remove(from)).unwrap_or_else(&mut mint),
            Logged::External { was: None, .. } => present.get(&path).cloned().unwrap_or_else(&mut mint),
            _ => present.get(&path).cloned().or_else(|| gone.remove(&path)).unwrap_or_else(&mut mint),
        };
        match &r.entry {
            Logged::External { now: None, was, .. } => {
                present.remove(&path);
                gone.insert(path.clone(), id.clone());
                if let Some(was) = was {
                    last.insert(id.clone(), (path.clone(), was.clone()));
                }
            }
            entry => {
                present.insert(path.clone(), id.clone());
                if let Some(now) = entry.now() {
                    last.insert(id.clone(), (path.clone(), now.to_owned()));
                }
            }
        }
        let n = lines.len() as u64 + 1;
        numbered.insert((root.clone(), old_n), n);
        let entry = match r.entry.clone() {
            Logged::Store { path, via, against, now, stored, imports, .. } => {
                Logged::Store { path, cause: Cause { n: cause.expect("a store follows a root") }, via, against, now, stored, imports }
            }
            Logged::Moved { path, against, .. } => {
                Logged::Moved { path, cause: Cause { n: cause.expect("a write not made follows a root") }, against }
            }
            root => root,
        };
        lines.push(Line { n, prev: String::new(), at: r.at, file: id, entry });
    }
    // the index holds each file's identity as its lines name it, where it is now and the
    // text it last held; one the disk no longer has is remembered gone
    let mut known = Index::default();
    for (id, (path, held)) in &last {
        let at = PathBuf::from(path);
        let here = present.get(path) == Some(id) && matches!(world.read(&at), Found::Text(_));
        known.insert(Known {
            id: id.clone(),
            path: at.clone(),
            sys: if here { world.file_id(&at) } else { None },
            hash: held.clone(),
            gone: !here,
        });
    }
    for (root, _) in &read_logs {
        for v in world.entries(&root.join(".cells").join("versions")) {
            let name = v.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
            if !(name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit())) {
                continue;
            }
            match world.read(&v) {
                Found::Text(text) if hash(&text) == name => world.keep(&log::version(&name), &text)?,
                _ => {
                    tracing::warn!(target: ENGINE, version = %v.display(), "not its version's text: left in its folder's history in the Trash")
                }
            }
        }
    }
    world.save(&log::path(), &log::written(&lines))?;
    known.save(world)?;
    for (root, _) in &read_logs {
        let name = format!("{} .cells", root.file_name().and_then(|n| n.to_str()).unwrap_or("disk"));
        match world.trash(&root.join(".cells"), &name) {
            Ok(to) => {
                tracing::info!(target: ENGINE, folder = %root.display(), trash = %to.display(), "its history moved into the engine's, its .cells to the Trash")
            }
            Err(why) => {
                tracing::warn!(target: ENGINE, folder = %root.display(), why, "its history moved into the engine's, its .cells left where it is")
            }
        }
    }
    Ok(Migrated { folders: read_logs.len(), lines: lines.len(), files: last.len() })
}
