//! The daemon's loop: what the world says changed handed to the folder (`cells::folder`),
//! and what the folder decides carried out. Every decision of what to store is the
//! folder's, and of which file is which the index's (`index::follow`); here is only the
//! world's side of it: reading what was heard, telling a change from the engine's own
//! write coming back, writing only over the version a write was made from, logging every
//! version with what made it and the chain it set off, and saying each step.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use cells::files::is_url;
use cells::folder::{Change, Edited, Folder, Write, key_of};

use cells::Trigger;

use crate::allowed::{Allowed, Approval};
use crate::history::{self, Asked};
use crate::index::{By, Fate, FileId, Heard, Index, Looked, There, visible};
use crate::log::{self, Cause, Changed, Edit, Log, Logged, Op, Outcome, hash};
use crate::trace::ENGINE;
use crate::wire::{FromEngine, ToEngine};
use crate::world::{Found, Replaced, World, table_file};

/// What reaches the daemon, in the order it came: the disk's changes and its clients'
/// requests, through one channel, so the daemon is one thread and never shares its state.
pub enum Event {
    /// A path the disk says changed, with the file ID its event named when the watcher
    /// names one.
    Disk(Heard),
    /// A client's request, its paths canonical (`server`), answered when it wants one.
    Ask {
        id: u64,
        client: String,
        request: ToEngine,
        answer: Option<Sender<FromEngine>>,
    },
    /// A client told, from now on, every file whose version changed.
    Listen {
        id: u64,
        client: String,
        tell: Sender<Vec<PathBuf>>,
    },
    Leave {
        id: u64,
    },
    /// A script's run ended (`World::run_script`): what it returned, to land in its file.
    Ran(Running, Result<Vec<String>, String>),
}

/// A run under way: whose, at which version (the one allowed), started when and by what,
/// and the clients waiting for it to land.
pub struct Running {
    file: PathBuf,
    script: String,
    version: String,
    at: u64,
    by: String,
    waiting: Vec<Sender<FromEngine>>,
}

/// How long the disk must be quiet before what was heard is read: a save is several events
/// (a temporary file, a rename, metadata), and a writer caught mid-write is read once it is
/// done. Measured in the time a person waits for a total to follow, never noticed under a
/// tenth of a second.
const STILL: Duration = Duration::from_millis(30);

/// The longest a busy disk delays what was heard: a stream of writes is read in batches.
const PATIENCE: Duration = Duration::from_millis(500);

pub struct Daemon<W: World> {
    world: W,
    folder: Folder,
    /// Every file held, by identity, and every one seen go.
    index: Index,
    /// The one history.
    log: Log,
    /// The home folder, canonical: every `.cells` file under it is held.
    home: PathBuf,
    /// Each directory watched, and whether everything under it is.
    watched: Vec<(PathBuf, bool)>,
    /// Each client told what changes, and its name.
    listeners: BTreeMap<u64, (String, Sender<Vec<PathBuf>>)>,
    /// The files whose version changed since the clients were last told.
    changed: BTreeSet<PathBuf>,
    /// The version of a file each client said it holds (`ToEngine::Editing`): when that
    /// version lands, that client wrote it.
    holding: BTreeMap<PathBuf, Vec<Holding>>,
    /// Where a run that ends says so: the daemon's own channel.
    back: Sender<Event>,
    /// When the engine first saw each script, by file and name: a script never run is due
    /// from then (`schedule`).
    seen: BTreeMap<(PathBuf, String), u64>,
    /// The scripts running now, and the clients waiting on each.
    running: BTreeMap<(PathBuf, String), Vec<Sender<FromEngine>>>,
    /// Runs that ended over a file changed meanwhile: landed once that change is heard.
    landing: Vec<(Running, Result<Vec<String>, String>)>,
    /// What was said once and not again until it changes: a script due and not allowed at
    /// a version, a schedule that cannot be kept.
    told: BTreeSet<String>,
    /// The files a root changed since the scripts were last looked at: what starts a
    /// script that runs `when <file> changes`.
    touched: BTreeSet<PathBuf>,
}

struct Holding {
    id: u64,
    client: String,
    version: String,
}

/// A change that is a root of history, logged once the folder said what it edited.
struct Root {
    file: FileId,
    path: PathBuf,
    was: Option<String>,
    now: Option<String>,
    kind: RootKind,
}

enum RootKind {
    External {
        by: Option<String>,
    },
    /// The file at `path` is the one held at `from`, holding the text it held.
    Relocated {
        from: PathBuf,
    },
    Restore {
        by: String,
    },
    Run {
        script: String,
        run: u64,
        outcome: Outcome,
        by: String,
    },
    Edit {
        intent: String,
        by: String,
        ops: Vec<Op>,
        assigned: Vec<String>,
    },
}

/// The longest the engine waits before looking at its scripts again: a Mac asleep stops
/// the clock a wait is measured on, so a run due while it slept is found this long after
/// it wakes at most.
const GLANCE: Duration = Duration::from_secs(30);

/// What a version edited, as the trace says it: `row 2 amount 180.00 → 200.00, 1 row added`.
fn said(e: &Edited) -> String {
    let mut parts: Vec<String> = e.cells.iter().map(|c| format!("row {} {} {} → {}", c.row, c.column, c.was, c.now)).collect();
    let mut count = |n: usize, what: &str, verb: &str| {
        if n > 0 {
            parts.push(format!("{n} {what}{} {verb}", if n == 1 { "" } else { "s" }));
        }
    };
    count(e.rows_added.len(), "row", "added");
    count(e.rows_removed.len(), "row", "removed");
    count(e.columns_added.len(), "column", "added");
    count(e.columns_removed.len(), "column", "removed");
    parts.extend(e.columns_renamed.iter().map(|(from, to)| format!("column {from} renamed {to}")));
    if parts.is_empty() { "nothing a table holds".into() } else { parts.join(", ") }
}

/// What the engine says (`trace`): `said!` always, `detail!` when asked for more, `warned!`
/// for what did not go as it should.
macro_rules! said {
    ($($t:tt)*) => { tracing::info!(target: ENGINE, $($t)*) };
}
macro_rules! detail {
    ($($t:tt)*) => { tracing::debug!(target: ENGINE, $($t)*) };
}
macro_rules! warned {
    ($($t:tt)*) => { tracing::warn!(target: ENGINE, $($t)*) };
}

/// A path with its `.` and `..` taken out by their names: how a schedule's `when
/// ../money.cells changes` names a file the engine holds by its canonical path.
fn normal(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Milliseconds since the Unix epoch as a moment `schedule` reckons with.
fn timestamp(ms: u64) -> jiff::Timestamp {
    jiff::Timestamp::from_millisecond(ms as i64).expect("a moment the clock can say")
}

/// The folder's key for a path: the path, absolute, as text; the log writes a path so too.
fn key(path: &Path) -> String {
    path.to_str().unwrap_or_else(|| panic!("{} is not UTF-8: the engine keys files by their path as text", path.display())).to_owned()
}

/// Each file looked at as the disk has it now: its text and system ID, by path.
type Texts = BTreeMap<PathBuf, (String, Option<u64>)>;

impl<W: World> Daemon<W> {
    /// The daemon over `world`, a run that ends saying so on `back`, the channel `run`
    /// reads: its history and index read, the home folder watched whole, and every file
    /// under it walked and followed against what the index last knew (a file moved,
    /// changed or removed while the engine was not running is logged as such), each read
    /// as the start of what the engine holds. A history or an index the disk will not give
    /// is refused, saying why: continued from nothing, every history would fork.
    pub fn start(world: W, back: Sender<Event>) -> Result<Daemon<W>, String> {
        let home = world.home();
        let span = tracing::info_span!(target: ENGINE, "start", home = %home.display());
        let _starting = span.enter();
        let (log, broken) = Log::open(&world)?;
        if let Some(why) = broken {
            warned!(why, "history broken: kept and continued, what it says past there cannot be trusted");
        }
        let index = Index::open(&world)?;
        let mut d = Daemon {
            world,
            folder: Folder::default(),
            index,
            log,
            home: home.clone(),
            watched: Vec::new(),
            listeners: BTreeMap::new(),
            changed: BTreeSet::new(),
            holding: BTreeMap::new(),
            back,
            seen: BTreeMap::new(),
            running: BTreeMap::new(),
            landing: Vec::new(),
            told: BTreeSet::new(),
            touched: BTreeSet::new(),
        };
        d.watch(&home, true)?;
        d.walk();
        d.persist();
        said!(files = d.index.held().count(), lines = d.log.lines().len(), "holding");
        Ok(d)
    }

    /// Every table file under home and every file the index holds, looked at once and
    /// followed (`index::follow`), each read as the start of what the folder holds.
    fn walk(&mut self) {
        let mut paths: BTreeMap<PathBuf, Vec<u64>> = self.world.files(&self.home).into_iter().map(|p| (p, Vec::new())).collect();
        paths.extend(self.index.held().map(|k| (k.path.clone(), Vec::new())));
        let (looked, texts) = self.look(paths);
        let fates = self.index.follow(&looked);
        // every identity settled before any file is read, so a file read through an import
        // is the one the index says it is
        let mut seen: Vec<(PathBuf, Option<Root>)> = Vec::new();
        let mut moved: Vec<(FileId, PathBuf, PathBuf, By, String)> = Vec::new();
        for fate in &fates {
            match fate {
                Fate::Stayed { id, path } => {
                    let (text, sys) = &texts[path];
                    let was = self.index.get(id).hash.clone();
                    let now = hash(text);
                    self.index.seen(id, path, *sys, &now);
                    let changed = (was != now).then(|| Root {
                        file: id.clone(),
                        path: path.clone(),
                        was: Some(was),
                        now: Some(now),
                        kind: RootKind::External { by: None },
                    });
                    seen.push((path.clone(), changed));
                }
                Fate::Moved { id, from, to, by, .. } => {
                    let (text, sys) = &texts[to];
                    let was = self.index.get(id).hash.clone();
                    self.index.seen(id, to, *sys, &hash(text));
                    moved.push((id.clone(), from.clone(), to.clone(), *by, was));
                }
                Fate::Gone { id, path } => {
                    let was = self.index.get(id).hash.clone();
                    self.index.gone(id);
                    let span = tracing::info_span!(target: ENGINE, "change", path = %self.shown(path), what = "removed");
                    let _changing = span.enter();
                    let root =
                        Root { file: id.clone(), path: path.clone(), was: Some(was), now: None, kind: RootKind::External { by: None } };
                    self.log_root(root, None);
                }
                Fate::New { path } => {
                    if self.wanted(path) {
                        let (text, sys) = &texts[path];
                        let world = &self.world;
                        self.index.add(path, *sys, &hash(text), || world.bits());
                        seen.push((path.clone(), None));
                    }
                }
            }
        }
        for (path, changed) in seen {
            let text = texts[&path].0.clone();
            if let Some(root) = changed {
                let span = tracing::info_span!(target: ENGINE, "change", path = %self.shown(&path), what = "changed");
                let _changing = span.enter();
                self.log_root(root, None);
            }
            self.keep(&text);
            if self.folder.text(&key(&path)).is_none() {
                self.hand(Change::Seen { path: key(&path), text }, None);
            }
        }
        // after every other file is read, so each import naming where a file was follows it
        for (id, from, to, by, was) in moved {
            let text = texts[&to].0.clone();
            self.keep(&text);
            self.arrived(&id, &from, &to, &text, by, &was);
        }
    }

    /// Each path as the disk has it now, with the IDs its events named: what `follow`
    /// looks at, and each file's text. A file the disk will not give is said and left as
    /// the engine last knew it.
    fn look(&self, paths: BTreeMap<PathBuf, Vec<u64>>) -> (Vec<Looked>, Texts) {
        let mut looked = Vec::new();
        let mut texts = Texts::new();
        for (path, heard) in paths {
            let now = match self.world.read(&path) {
                Found::Text(text) => {
                    let sys = self.world.file_id(&path);
                    let there = There { hash: hash(&text), sys };
                    texts.insert(path.clone(), (text, sys));
                    Some(there)
                }
                Found::Missing => None,
                Found::Unreadable(why) => {
                    warned!(path = %self.shown(&path), why, "unread");
                    continue;
                }
            };
            looked.push(Looked { path, heard, now });
        }
        (looked, texts)
    }

    /// Whether a file come new is one the engine holds: a `.cells` file the walk takes in
    /// under home, or a file one held imports (a CSV, a file outside home).
    fn wanted(&self, path: &Path) -> bool {
        let k = key(path);
        let cells = path.extension().is_some_and(|e| e == "cells") && path.starts_with(&self.home) && visible(&self.home, path);
        cells || self.folder.paths().any(|p| self.folder.reads(p).contains(&k))
    }

    /// Served until the process ends: each event as it comes, what the disk says changed
    /// read once the disk is still, and the clients told what changed after each; between
    /// them, each script run when it is due.
    pub fn run(&mut self, events: Receiver<Event>) -> ! {
        loop {
            let wait = self.tick();
            let first = match events.recv_timeout(wait) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => panic!("the daemon's senders are gone"),
            };
            let mut heard = Vec::new();
            let mut asked = Vec::new();
            match first {
                Event::Disk(h) => heard.push(h),
                other => asked.push(other),
            }
            if !heard.is_empty() {
                let began = Instant::now();
                loop {
                    match events.recv_timeout(STILL.min(PATIENCE.saturating_sub(began.elapsed()))) {
                        Ok(Event::Disk(h)) => heard.push(h),
                        Ok(other) => asked.push(other),
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => panic!("the daemon's senders are gone"),
                    }
                }
                self.hear(&heard);
            }
            for event in asked {
                self.answer(event);
            }
            self.tell();
        }
    }

    pub(crate) fn answer(&mut self, event: Event) {
        match event {
            Event::Disk(_) => unreachable!("the disk's events are heard, never answered"),
            Event::Ran(running, returned) => self.land(running, returned),
            Event::Ask { id, client, request: ToEngine::Run { file, script }, answer } => {
                let answer = answer.expect("a run is answered");
                if let Err(why) = self.named(&file).and_then(|_| self.start_run(&file, &script, &client, Some(answer.clone()))) {
                    warned!(client, id, why, "refused");
                    let _ = answer.send(FromEngine::Refused { why });
                }
            }
            Event::Ask { id, client, request, answer } => {
                let said = match self.asked(id, &client, request) {
                    Ok(said) => said,
                    Err(why) => {
                        warned!(client, id, why, "refused");
                        Some(FromEngine::Refused { why })
                    }
                };
                if let (Some(answer), Some(said)) = (answer, said) {
                    // the client may be gone already
                    let _ = answer.send(said);
                }
            }
            Event::Listen { id, client, tell } => {
                said!(client, id, "joined");
                self.listeners.insert(id, (client, tell));
            }
            Event::Leave { id } => {
                for held in self.holding.values_mut() {
                    held.retain(|h| h.id != id);
                }
                self.holding.retain(|_, held| !held.is_empty());
                if let Some((client, _)) = self.listeners.remove(&id) {
                    said!(client, id, "left");
                }
            }
        }
        self.persist();
    }

    /// The identity of the file at `file`, a client named: held from now on when it was not
    /// (read, the start of its history), so any file a client opens is followed as one under
    /// home is.
    fn named(&mut self, file: &Path) -> Result<FileId, String> {
        if let Some(seen) = self.hold(file)? {
            self.hand(seen, None);
        }
        Ok(self.index.at(file).expect("a file held").id.clone())
    }

    /// The file at `path` held, when it is not: its identity minted, its text kept, and
    /// the change to hand the folder (`Seen`); nothing when it is held already. A file not
    /// there, or one the disk will not give, is not held.
    fn hold(&mut self, path: &Path) -> Result<Option<Change>, String> {
        let k = key(path);
        if self.folder.text(&k).is_some() {
            return Ok(None);
        }
        let text = match self.world.read(path) {
            Found::Text(text) => text,
            Found::Missing => return Err(format!("{}: no such file", self.shown(path))),
            Found::Unreadable(why) => return Err(format!("{}: {why}", self.shown(path))),
        };
        let (sys, now) = (self.world.file_id(path), hash(&text));
        match self.index.at(path).map(|k| k.id.clone()) {
            Some(id) => self.index.seen(&id, path, sys, &now),
            None => {
                let world = &self.world;
                let id = self.index.add(path, sys, &now, || world.bits());
                detail!(path = %self.shown(path), file = %id, "held");
            }
        }
        self.keep(&text);
        Ok(Some(Change::Seen { path: k, text }))
    }

    /// A client's request carried out: its answer, nothing for one never answered.
    fn asked(&mut self, id: u64, client: &str, request: ToEngine) -> Result<Option<FromEngine>, String> {
        Ok(Some(match request {
            ToEngine::Hello { .. } => unreachable!("a greeting is the server's"),
            ToEngine::Run { .. } => unreachable!("a run is answered when it lands"),
            ToEngine::Allow { file, script, version } => {
                let approval =
                    Approval { file: file.clone(), script: script.clone(), version, by: client.to_owned(), at: self.world.now() };
                self.world.append(&crate::allowed::path(), &crate::allowed::line(&approval))?;
                said!(path = %self.shown(&file), script, by = client, "allowed, as shown to its person");
                FromEngine::Allowed { file, script }
            }
            ToEngine::Editing { file, version } => {
                if let Err(why) = self.named(&file) {
                    warned!(client, id, why, "editing a file the engine cannot hold");
                }
                let held = self.holding.entry(file).or_default();
                held.retain(|h| h.id != id);
                held.push(Holding { id, client: client.to_owned(), version });
                return Ok(None);
            }
            ToEngine::History { file, under, limit } => {
                // a file gone is still the one last at its path
                let id = match &file {
                    Some(f) => Some(self.named(f).or_else(|why| self.last_at(f).ok_or(why))?),
                    None => None,
                };
                let asked = Asked { file: id.as_ref(), under: under.as_deref() };
                FromEngine::History { chains: history::chains(&self.log, &asked, limit) }
            }
            ToEngine::Cell { file, row, column } => {
                let id = self.named(&file)?;
                let doc = self.folder.doc(&key(&file)).ok_or(format!("{}: does not read", self.shown(&file)))?;
                if row == 0 || row > doc.rows() {
                    return Err(format!("{}: no row {row}, it has {}", self.shown(&file), doc.rows()));
                }
                let key = key_of(doc, row - 1);
                FromEngine::Values { values: history::cell(&self.log, &id, row, key.as_deref(), &column) }
            }
            ToEngine::Since { file, version } => {
                let id = self.named(&file)?;
                let current = hash(self.folder.text(&key(&file)).expect("a file held has its text"));
                match history::since(&self.log, &id, &version, &current) {
                    None => FromEngine::Since { known: false, marks: Vec::new(), chains: Vec::new() },
                    Some(marks) => {
                        let roots: BTreeSet<u64> = marks.iter().map(|m| m.root).collect();
                        let chains = history::chains(&self.log, &Asked { file: Some(&id), under: None }, usize::MAX)
                            .into_iter()
                            .filter(|c| roots.contains(&c.root.n))
                            .collect();
                        FromEngine::Since { known: true, marks, chains }
                    }
                }
            }
            ToEngine::Version { file, version } => match self.world.read(&log::version(&version)) {
                Found::Text(text) => FromEngine::Version { file, version, text },
                _ => return Err(format!("{}: no version {version} kept", self.shown(&file))),
            },
            ToEngine::Restore { file, version } => {
                let fid = self.named(&file)?;
                let text = match self.world.read(&log::version(&version)) {
                    Found::Text(text) => text,
                    _ => return Err(format!("{}: no version {version} kept", self.shown(&file))),
                };
                let k = key(&file);
                let was = hash(self.folder.text(&k).expect("a file held has its text"));
                if was == version {
                    return Err(format!("{}: it is that version already", self.shown(&file)));
                }
                match self.world.replace(&file, &was, &text) {
                    Replaced::Done => {}
                    Replaced::Moved => {
                        return Err(format!("{}: changed since the engine read it; its change is read next, ask again", self.shown(&file)));
                    }
                    Replaced::Failed(why) => return Err(format!("{}: {why}", self.shown(&file))),
                }
                let span = tracing::info_span!(target: ENGINE, "restore", path = %self.shown(&file), to = &version[..12], by = client);
                let _restoring = span.enter();
                self.wrote(&fid, &file, &text);
                self.changed.insert(file.clone());
                self.touched.insert(file.clone());
                let root = Root {
                    file: fid,
                    path: file.clone(),
                    was: Some(was.clone()),
                    now: Some(version),
                    kind: RootKind::Restore { by: client.to_owned() },
                };
                self.hand(Change::Changed { path: k, text }, Some(root));
                FromEngine::Restored { file, was }
            }
            ToEngine::Intent { id, file, against, ops } => self.intent(client, &id, &file, &against, ops)?,
        }))
    }

    /// The file last at `path`, held or seen go.
    fn last_at(&self, path: &Path) -> Option<FileId> {
        self.index.known().iter().rev().find(|k| k.path == path).map(|k| k.id.clone())
    }

    /// A client's intent carried out (ENGINE.md, Writing): applied once, by its id, however
    /// often it is sent; journalled as a root of history before what it reaches is stored;
    /// written only over the version the engine holds, and over a version written since
    /// (one the engine had not heard yet: heard first) only where it says what it saw.
    fn intent(&mut self, client: &str, id: &str, file: &Path, against: &str, ops: Vec<Op>) -> Result<FromEngine, String> {
        if !cells::value::is_id(id.as_bytes()) {
            return Err(format!("{id:?} is no intent id: eight characters of 0-9 and a-z but i, l, o, u"));
        }
        let fid = self.named(file)?;
        let done = self.log.of(&fid).find_map(|l| match &l.entry {
            Logged::Edit { intent, now, assigned, .. } if intent == id => Some((now.clone(), assigned.clone())),
            _ => None,
        });
        if let Some((version, assigned)) = done {
            detail!(path = %self.shown(file), intent = id, by = client, "applied already: answered again");
            return Ok(FromEngine::Applied { file: file.to_path_buf(), intent: id.to_owned(), version, assigned });
        }
        let k = key(file);
        // a whole text kept as the version it is, so the client's next text, made against it,
        // can be compared with it whether or not the file ever held it (another writer's
        // change laid in, its rows given ids)
        for op in &ops {
            if let Op::Text { text, .. } = op {
                self.keep(text);
            }
        }
        let ops_core: Vec<cells::intent::Op> = ops.iter().map(Into::into).collect();
        // the version it was made against, kept whole: what a whole text is compared with
        let named = against.len() == 64 && against.bytes().all(|b| b.is_ascii_hexdigit());
        let base = match named.then(|| self.world.read(&log::version(against))) {
            Some(Found::Text(text)) => Some(text),
            _ => None,
        };
        let ops: Vec<Op> = ops.iter().map(Op::logged).collect();
        // twice at most: a version on disk the engine had not heard is heard, then applied over
        for _ in 0..2 {
            let text = self.folder.text(&k).ok_or(format!("{}: not read by the engine", self.shown(file)))?.to_owned();
            let was = hash(&text);
            let (folder, world) = (&self.folder, &self.world);
            let mut read = |t: &str| folder.read_as(&k, t);
            let applied = cells::intent::apply(&text, &ops_core, was != against, base.as_deref(), || world.bits(), &mut read)
                .map_err(|why| format!("{}: {why}", self.shown(file)))?;
            if applied.text == text {
                return Ok(FromEngine::Applied { file: file.to_path_buf(), intent: id.to_owned(), version: was, assigned: Vec::new() });
            }
            match self.world.replace(file, &was, &applied.text) {
                Replaced::Done => {}
                Replaced::Moved => {
                    self.hear(&[Heard { path: file.to_path_buf(), id: None }]);
                    continue;
                }
                Replaced::Failed(why) => return Err(format!("{}: {why}", self.shown(file))),
            }
            let now = hash(&applied.text);
            let span = tracing::info_span!(target: ENGINE, "edit", path = %self.shown(file), intent = id, by = client);
            let _editing = span.enter();
            self.wrote(&fid, file, &applied.text);
            self.changed.insert(file.to_path_buf());
            self.touched.insert(file.to_path_buf());
            let root = Root {
                file: fid.clone(),
                path: file.to_path_buf(),
                was: Some(was),
                now: Some(now.clone()),
                kind: RootKind::Edit { intent: id.to_owned(), by: client.to_owned(), ops, assigned: applied.assigned.clone() },
            };
            self.hand(Change::Changed { path: k, text: applied.text }, Some(root));
            return Ok(FromEngine::Applied { file: file.to_path_buf(), intent: id.to_owned(), version: now, assigned: applied.assigned });
        }
        Err(format!("{}: written by another program twice while the engine applied this; send it again", self.shown(file)))
    }

    /// Every script of every file held looked at: each due run started when its person
    /// allowed it as written, a run that ended over a changed file landed again. How long
    /// until the next is due, `GLANCE` at most.
    pub fn tick(&mut self) -> Duration {
        let pending = std::mem::take(&mut self.landing);
        for (running, returned) in pending {
            self.land(running, returned);
        }
        let now = self.world.now();
        let allowed = match self.world.read(&crate::allowed::path()) {
            Found::Text(t) => Allowed::parse(&t),
            _ => Allowed::default(),
        };
        let touched = std::mem::take(&mut self.touched);
        let mut soonest = now + GLANCE.as_millis() as u64;
        let held: Vec<PathBuf> = self.folder.paths().map(PathBuf::from).collect();
        for path in held {
            let Some(doc) = self.folder.doc(&key(&path)) else { continue };
            let text = self.folder.text(&key(&path)).expect("a doc's text").to_owned();
            let scripts = doc.scripts().to_vec();
            for script in scripts {
                let id = (path.clone(), script.name.clone());
                if self.running.contains_key(&id) {
                    continue;
                }
                let seen = *self.seen.entry(id).or_insert(now);
                let dir = path.parent().expect("a file is in a folder");
                let changed = script.schedule.triggers.iter().find_map(|t| match t {
                    Trigger::Changes(p) => {
                        let p = normal(&dir.join(p));
                        (p != path && touched.contains(&p)).then(|| format!("a change to {}", self.shown(&p)))
                    }
                    _ => None,
                });
                let last = cells::run::last_ran(&text, &script).map(|i| i.seconds as u64 * 1000 + i.nanos as u64 / 1_000_000);
                let next = match crate::schedule::next(&script.schedule, last.map(timestamp), timestamp(seen)) {
                    Ok(next) => next.map(|t| t.as_millisecond() as u64),
                    Err(why) => {
                        if self.told.insert(format!("{}#{}: {why}", path.display(), script.name)) {
                            warned!(path = %self.shown(&path), script = script.name, why, "its schedule is not kept");
                        }
                        None
                    }
                };
                let by = match (changed, next) {
                    (Some(change), _) => change,
                    (None, Some(n)) if n <= now => "its schedule".to_owned(),
                    (None, Some(n)) => {
                        soonest = soonest.min(n);
                        continue;
                    }
                    (None, None) => continue,
                };
                if !allowed.allows(&path, &script) {
                    let version = crate::allowed::version(&script);
                    if self.told.insert(format!("{}#{}@{version}", path.display(), script.name)) {
                        warned!(path = %self.shown(&path), script = script.name, "due, and not run: not allowed on this Mac as it is written");
                    }
                    continue;
                }
                if let Err(why) = self.start_run(&path, &script.name, &by, None) {
                    warned!(path = %self.shown(&path), script = script.name, why, "not run");
                }
            }
        }
        Duration::from_millis(soonest.saturating_sub(now)).min(GLANCE)
    }

    /// The script `name` of `file` (held) started, as its person allowed it, by `by` (its
    /// schedule, a change, a client), `waiting` told once it lands; joining a run under way.
    fn start_run(&mut self, file: &Path, name: &str, by: &str, waiting: Option<Sender<FromEngine>>) -> Result<(), String> {
        let id = (file.to_path_buf(), name.to_owned());
        if let Some(waiters) = self.running.get_mut(&id) {
            waiters.extend(waiting);
            return Ok(());
        }
        let doc = self.folder.doc(&key(file)).ok_or(format!("{}: does not read", self.shown(file)))?;
        let script = doc
            .scripts()
            .iter()
            .find(|s| s.name == name)
            .ok_or(format!("{} has no script {}", self.shown(file), cells::say::code(name)))?
            .clone();
        let allowed = match self.world.read(&crate::allowed::path()) {
            Found::Text(t) => Allowed::parse(&t),
            _ => Allowed::default(),
        };
        if !allowed.allows(file, &script) {
            return Err(format!(
                "{} is not allowed on this Mac as it is written: `cells allow {} {name}` shows its code and asks",
                cells::say::code(name),
                file.display()
            ));
        }
        let table = doc.tables().iter().find(|t| t.name == name).and_then(|t| t.doc.clone()).expect("a script's table, declared");
        let at = self.world.now();
        let context = cells::run::context(&table, cells::value::Instant::from(at as i64, 3));
        said!(path = %self.shown(file), script = name, by, "running");
        self.running.insert(id, waiting.into_iter().collect());
        let running = Running {
            file: file.to_path_buf(),
            script: name.to_owned(),
            version: crate::allowed::version(&script),
            at,
            by: by.to_owned(),
            waiting: Vec::new(),
        };
        let back = self.back.clone();
        // the daemon outlives every run
        self.world.run_script(
            name,
            &script.language,
            &script.code,
            context,
            Box::new(move |returned| _ = back.send(Event::Ran(running, returned))),
        );
        Ok(())
    }

    /// A run that ended landed in its file (`cells::run::landed`): its rows when they read,
    /// its runs block either way, written over the version the engine holds, a root of
    /// history with what it reached following; over a file changed meanwhile, landed once
    /// that change is heard. A script edited while it ran lands nothing: its run was of
    /// another version.
    fn land(&mut self, mut running: Running, returned: Result<Vec<String>, String>) {
        let id = (running.file.clone(), running.script.clone());
        running.waiting.extend(self.running.remove(&id).unwrap_or_default());
        let k = key(&running.file);
        let shown = self.shown(&running.file);
        let Some(text) = self.folder.text(&k).map(str::to_owned) else {
            warned!(path = shown, script = running.script, "ran, and its file is gone: nothing landed");
            return;
        };
        let doc = self.folder.doc(&k);
        let now_version = doc.and_then(|d| d.scripts().iter().find(|s| s.name == running.script)).map(crate::allowed::version);
        if now_version.as_deref() != Some(running.version.as_str()) {
            warned!(path = shown, script = running.script, "ran, and the script changed meanwhile: nothing landed");
            for w in running.waiting {
                let _ = w.send(FromEngine::Refused { why: format!("{} changed while it ran: nothing landed", running.script) });
            }
            return;
        }
        let at = cells::value::Instant::from(running.at as i64, 3);
        let folder = &self.folder;
        let mut read = |t: &str| folder.read_as(&k, t);
        let (written, ran) = match cells::run::landed(&text, &running.script, returned.clone(), at, &mut read) {
            Ok(landed) => landed,
            Err(why) => {
                tracing::error!(target: ENGINE, path = shown, script = running.script, why, "ran, and its file does not read: nothing landed");
                return;
            }
        };
        let was = hash(&text);
        match self.world.replace(&running.file, &was, &written) {
            Replaced::Done => {}
            Replaced::Moved => {
                // held as running, so it is not started again before it lands
                self.running.insert(id, Vec::new());
                detail!(path = shown, script = running.script, "ran over a file changed meanwhile: landed once its change is heard");
                self.landing.push((running, returned));
                return;
            }
            Replaced::Failed(why) => {
                tracing::error!(target: ENGINE, path = shown, script = running.script, why, "ran, and its file could not be written");
                return;
            }
        }
        let outcome = match &ran.outcome {
            Ok((added, changed)) => Outcome::Ok { added: *added, changed: *changed },
            Err(why) => Outcome::Failed { why: why.clone() },
        };
        let span = tracing::info_span!(target: ENGINE, "run", path = shown, script = running.script, run = ran.run, by = running.by);
        let _ran = span.enter();
        match &outcome {
            Outcome::Ok { added, changed } => said!(added, changed, "ran"),
            Outcome::Failed { why } => warned!(why, "failed"),
        }
        let fid = self.index.at(&running.file).expect("a file run into is held").id.clone();
        self.wrote(&fid, &running.file, &written);
        self.changed.insert(running.file.clone());
        self.touched.insert(running.file.clone());
        let root = Root {
            file: fid,
            path: running.file.clone(),
            was: Some(was),
            now: Some(hash(&written)),
            kind: RootKind::Run { script: running.script.clone(), run: ran.run, outcome: outcome.clone(), by: running.by.clone() },
        };
        self.hand(Change::Changed { path: k, text: written }, Some(root));
        for w in running.waiting {
            let _ = w.send(FromEngine::Ran {
                file: running.file.clone(),
                script: running.script.clone(),
                run: ran.run,
                outcome: outcome.clone(),
            });
        }
    }

    /// Every client told the files whose version changed since it was last told; a client
    /// gone is forgotten.
    pub(crate) fn tell(&mut self) {
        if self.changed.is_empty() {
            return;
        }
        let paths: Vec<PathBuf> = std::mem::take(&mut self.changed).into_iter().collect();
        self.listeners.retain(|_, (_, tell)| tell.send(paths.clone()).is_ok());
    }

    pub fn folder(&self) -> &Folder {
        &self.folder
    }

    pub fn index(&self) -> &Index {
        &self.index
    }

    pub fn world(&self) -> &W {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut W {
        &mut self.world
    }

    /// The world taken back whole, the daemon ended: what a test starts another from.
    pub fn into_world(self) -> W {
        self.world
    }

    /// What the disk said changed, followed (`index::follow`): each file gone handed to the
    /// folder as gone, the files moved in one moment as one move (every import naming one
    /// follows it), a file seen gone before and come again as found late, and each file
    /// whose text is not what the folder holds as changed (the engine's own write coming
    /// back, or a file touched, is the same text and changes nothing). A folder heard
    /// (moved, removed) is every file held under it and every one the walk finds in it
    /// now. Each change is a span, ending once every file it reached is stored.
    pub fn hear(&mut self, heard: &[Heard]) {
        let mut paths: BTreeMap<PathBuf, Vec<u64>> = BTreeMap::new();
        for h in heard {
            if table_file(&h.path) {
                if self.index.at(&h.path).is_some() || visible(&self.home, &h.path) {
                    paths.entry(h.path.clone()).or_default().extend(h.id);
                }
                continue;
            }
            // a folder, or what was one: the home folder itself and anything the walk leaves
            // out never walked again for it
            if h.path == self.home || !visible(&self.home, &h.path) {
                continue;
            }
            let under: Vec<PathBuf> = self
                .index
                .held()
                .filter(|k| k.path.starts_with(&h.path))
                .map(|k| k.path.clone())
                .chain(self.world.files(&h.path))
                .collect();
            for p in under {
                paths.entry(p).or_default();
            }
        }
        if paths.is_empty() {
            return;
        }
        let shown: Vec<String> = paths.keys().map(|p| self.shown(p)).collect();
        detail!(paths = shown.join(" "), "heard");
        let (looked, texts) = self.look(paths);
        let fates = self.index.follow(&looked);
        let mut handed = 0;
        for fate in &fates {
            if let Fate::Gone { id, path } = fate {
                self.removed(id, path);
                handed += 1;
            }
        }
        // moved in this moment, from where the folder holds it; else found where it went after
        // it was seen go
        let (moves, late): (Vec<_>, Vec<_>) = fates
            .iter()
            .filter_map(|f| match f {
                Fate::Moved { id, from, to, by } => Some((id.clone(), from.clone(), to.clone(), *by)),
                _ => None,
            })
            .partition(|(_, from, _, _)| self.folder.text(&key(from)).is_some());
        if !moves.is_empty() {
            handed += moves.len();
            self.relocate(&moves, &texts);
        }
        for (id, from, to, by) in late {
            let (text, sys) = texts[&to].clone();
            let was = self.index.get(&id).hash.clone();
            self.index.seen(&id, &to, sys, &hash(&text));
            self.keep(&text);
            self.arrived(&id, &from, &to, &text, by, &was);
            handed += 1;
        }
        for fate in &fates {
            match fate {
                Fate::Stayed { id, path } => {
                    let (text, sys) = texts[path].clone();
                    if self.changed_at(id, path, sys, text) {
                        handed += 1;
                    }
                }
                Fate::New { path } if self.wanted(path) => {
                    let (text, sys) = texts[path].clone();
                    let now = hash(&text);
                    let world = &self.world;
                    let id = self.index.add(path, sys, &now, || world.bits());
                    let by = self.written_by(path, &now);
                    let span = tracing::info_span!(target: ENGINE, "change", path = %self.shown(path), what = "added", by = by.as_deref());
                    let _changing = span.enter();
                    self.keep(&text);
                    self.changed.insert(path.clone());
                    self.touched.insert(path.clone());
                    let root = Root { file: id, path: path.clone(), was: None, now: Some(now), kind: RootKind::External { by } };
                    self.hand(Change::Changed { path: key(path), text }, Some(root));
                    handed += 1;
                }
                Fate::New { .. } | Fate::Gone { .. } | Fate::Moved { .. } => {}
            }
        }
        if handed == 0 {
            detail!("same as held: the engine's own writes, or files touched");
        }
        self.persist();
    }

    /// The file `id`, at `path`, holding `text` as the disk has it now, with the system ID
    /// `sys`: handed to the folder as changed when its text is not what the folder holds.
    /// Whether it was.
    fn changed_at(&mut self, id: &FileId, path: &Path, sys: Option<u64>, text: String) -> bool {
        let k = key(path);
        let now = hash(&text);
        self.index.seen(id, path, sys, &now);
        let Some(held) = self.folder.text(&k) else {
            // held, and never read: the disk would not give it when the engine started
            self.keep(&text);
            self.hand(Change::Seen { path: k, text }, None);
            return true;
        };
        if held == text {
            return false;
        }
        let was = hash(held);
        let by = self.written_by(path, &now);
        let span = tracing::info_span!(target: ENGINE, "change", path = %self.shown(path), what = "changed", by = by.as_deref());
        let _changing = span.enter();
        self.keep(&text);
        self.changed.insert(path.to_path_buf());
        self.touched.insert(path.to_path_buf());
        let root = Root { file: id.clone(), path: path.to_path_buf(), was: Some(was), now: Some(now), kind: RootKind::External { by } };
        self.hand(Change::Changed { path: k, text }, Some(root));
        true
    }

    /// The file `id`, held at `path`, gone: remembered by the index, so a file of its text
    /// come anywhere later is it; every file reading it read again.
    fn removed(&mut self, id: &FileId, path: &Path) {
        let k = key(path);
        let was = self.index.get(id).hash.clone();
        self.index.gone(id);
        let span = tracing::info_span!(target: ENGINE, "change", path = %self.shown(path), what = "removed");
        let _changing = span.enter();
        self.changed.insert(path.to_path_buf());
        self.touched.insert(path.to_path_buf());
        let root = Root { file: id.clone(), path: path.to_path_buf(), was: Some(was), now: None, kind: RootKind::External { by: None } };
        self.hand(Change::Gone { path: k }, Some(root));
    }

    /// The file `id`, gone from `from` before the folder holds it at `to` (seen go earlier,
    /// or the engine starting): seen at `to` holding `text`, followed by every import still
    /// naming where it was, logged as relocated; logged changed too when `text` is not the
    /// version it held (`was`), moved and edited while the engine was not running.
    fn arrived(&mut self, id: &FileId, from: &Path, to: &Path, text: &str, by: By, was: &str) {
        let span = tracing::info_span!(target: ENGINE, "change", what = "moved", moves = format!("{} → {}", self.shown(from), self.shown(to)), by = by.said());
        let _changing = span.enter();
        self.changed.insert(to.to_path_buf());
        self.touched.insert(to.to_path_buf());
        self.hand(Change::Seen { path: key(to), text: text.to_owned() }, None);
        let root = Root {
            file: id.clone(),
            path: to.to_path_buf(),
            was: Some(was.to_owned()),
            now: Some(was.to_owned()),
            kind: RootKind::Relocated { from: from.to_path_buf() },
        };
        self.hand(Change::Moved { moves: vec![(key(from), key(to))] }, Some(root));
        let now = hash(text);
        if now != was {
            let root = Root {
                file: id.clone(),
                path: to.to_path_buf(),
                was: Some(was.to_owned()),
                now: Some(now),
                kind: RootKind::External { by: None },
            };
            self.log_root(root, None);
        }
    }

    /// The files `moves` names (each its identity, where it was, where it is, how it was
    /// known), moved in one moment: handed to the folder as one move, so every import naming
    /// one follows it, each logged as a root of its own; then each whose text changed with
    /// its move (an edit and a move at once, followed by its file ID) as changed.
    fn relocate(&mut self, moves: &[(FileId, PathBuf, PathBuf, By)], texts: &Texts) {
        let pairs: Vec<String> =
            moves.iter().map(|(_, from, to, by)| format!("{} → {} by {}", self.shown(from), self.shown(to), by.said())).collect();
        let span = tracing::info_span!(target: ENGINE, "change", what = "moved", moves = pairs.join(", "));
        let _changing = span.enter();
        let mut roots: Vec<Root> = Vec::new();
        let mut edited: Vec<(FileId, PathBuf, String)> = Vec::new();
        for (id, from, to, _) in moves {
            for p in [from, to] {
                self.changed.insert(p.clone());
                self.touched.insert(p.clone());
            }
            let held = hash(self.folder.text(&key(from)).expect("a file moved was held"));
            let (text, sys) = texts[to].clone();
            self.index.seen(id, to, sys, &hash(&text));
            if hash(&text) != held {
                edited.push((id.clone(), to.clone(), text));
            }
            roots.push(Root {
                file: id.clone(),
                path: to.clone(),
                was: Some(held.clone()),
                now: Some(held),
                kind: RootKind::Relocated { from: from.clone() },
            });
        }
        // every move a root; the last the one the folder's writes follow
        let last = roots.pop().expect("a move");
        for root in roots {
            self.log_root(root, None);
        }
        let moves = moves.iter().map(|(_, from, to, _)| (key(from), key(to))).collect();
        self.hand(Change::Moved { moves }, Some(last));
        for (id, to, text) in edited {
            let sys = texts[&to].1;
            self.changed_at(&id, &to, sys, text);
        }
    }

    /// The client that said it held `version` of `path`, when one did: it wrote it.
    fn written_by(&mut self, path: &Path, version: &str) -> Option<String> {
        let held = self.holding.get_mut(path)?;
        let at = held.iter().position(|h| h.version == version)?;
        let h = held.remove(at);
        if held.is_empty() {
            self.holding.remove(path);
        }
        Some(h.client)
    }

    /// `change` handed to the folder, and everything it leads to, until the folder settles:
    /// each file it needs read and handed in, each write made and its text handed back.
    /// A `root` is logged once the folder said what it edited, and every write it leads to
    /// is logged as following it.
    fn hand(&mut self, change: Change, root: Option<Root>) {
        let mut queue = VecDeque::from([change]);
        // A file written twice for one change means the folder never settles: a write
        // stores what a second would, so this is a bug, and it stops here.
        let mut written: BTreeSet<String> = BTreeSet::new();
        let mut root = root;
        let mut cause: Option<u64> = None;
        while let Some(change) = queue.pop_front() {
            let started = Instant::now();
            let out = self.folder.apply(change);
            let took = started.elapsed();
            if let Some(r) = root.take() {
                let edit = out.edited.as_ref().map(Edit::from);
                if let Some(e) = &out.edited {
                    said!(cells = said(e), "edited");
                }
                cause = self.log_root(r, edit);
            }
            for r in &out.read {
                let path = self.shown(Path::new(&r.path));
                match &r.refused {
                    Some(why) => warned!(path, why, "refused by the reader"),
                    None => detail!(path, problems = r.problems, stale = r.stale, "read"),
                }
            }
            if !out.read.is_empty() {
                detail!(files = out.read.len(), ms = took.as_secs_f64() * 1000.0, "settled");
            }
            for need in out.needs {
                queue.extend(self.needed(&need));
            }
            for w in out.writes {
                assert!(written.insert(w.path.clone()), "{} written twice for one change: the folder does not settle", w.path);
                queue.extend(self.write(w, cause));
            }
        }
    }

    /// `root` logged: its number, nothing when it could not be written, which is said.
    fn log_root(&mut self, root: Root, edited: Option<Edit>) -> Option<u64> {
        let path = key(&root.path);
        let entry = match root.kind {
            RootKind::External { by } => Logged::External { path, was: root.was, now: root.now, by, edited },
            RootKind::Relocated { from } => {
                Logged::Relocated { path, from: key(&from), now: root.now.expect("a file moved holds its text") }
            }
            RootKind::Restore { by } => Logged::Restore {
                path,
                was: root.was.expect("a restored file was held"),
                now: root.now.expect("a restored file holds the version returned to"),
                by,
                edited,
            },
            RootKind::Run { script, run, outcome, by } => Logged::Run {
                path,
                script,
                run,
                outcome,
                by,
                was: root.was.expect("a file run into was held"),
                now: root.now.expect("a run writes a version"),
                edited,
            },
            RootKind::Edit { intent, by, ops, assigned } => Logged::Edit {
                path,
                intent,
                by,
                ops,
                assigned,
                was: root.was.expect("a file edited was held"),
                now: root.now.expect("an edit writes a version"),
                edited,
            },
        };
        self.log(&root.file, entry)
    }

    /// A file a read named and the folder does not hold: held, watched where it is when
    /// that is outside home (home is watched whole), and handed in when it can be read.
    fn needed(&mut self, need: &str) -> Option<Change> {
        if is_url(need) {
            detail!(url = need, "skipped: a URL import is fetched by `cells check` and `cells pin`, never by the engine");
            return None;
        }
        let path = PathBuf::from(need);
        if let Some(dir) = path.parent().filter(|d| d.is_absolute())
            && let Err(why) = self.watch(dir, false)
        {
            warned!(why, "unwatched");
        }
        match self.hold(&path) {
            Ok(seen) => seen,
            Err(why) => {
                detail!(why, "imported, not held");
                None
            }
        }
    }

    /// `w` written, only over the version it was made from; its text handed back to the
    /// folder once written.
    fn write(&mut self, w: Write, cause: Option<u64>) -> Option<Change> {
        let path = PathBuf::from(&w.path);
        let id = self.index.at(&path).unwrap_or_else(|| panic!("{} written, never held", w.path)).id.clone();
        // nothing when the root's line could not be written, which was said
        let cause = cause.map(|n| Cause { n });
        match self.world.replace(&path, &w.against, &w.text) {
            Replaced::Done => {
                let now = hash(&w.text);
                self.wrote(&id, &path, &w.text);
                if let Some(cause) = cause {
                    let entry = Logged::Store {
                        path: w.path.clone(),
                        cause,
                        via: w.via.clone(),
                        against: w.against.clone(),
                        now,
                        stored: w.stored.iter().map(Changed::from).collect(),
                        imports: w.imports.clone(),
                    };
                    self.log(&id, entry);
                }
                let cells: Vec<String> = w.stored.iter().map(|s| format!("{} row {} {} → {}", s.column, s.row, s.was, s.now)).collect();
                let via =
                    (w.via != [w.path.clone()]).then(|| w.via.iter().map(|v| self.shown(Path::new(v))).collect::<Vec<_>>().join(", "));
                if w.imports.is_empty() {
                    said!(path = %self.shown(&path), via, cells = cells.join(", "), "stored");
                } else {
                    let imports: Vec<String> = w.imports.iter().map(|(a, b)| format!("{a} → {b}")).collect();
                    said!(path = %self.shown(&path), via, imports = imports.join(", "), "followed: its imports name the files moved");
                }
                self.changed.insert(path);
                Some(Change::Written { path: w.path, text: w.text })
            }
            Replaced::Moved => {
                if let Some(cause) = cause {
                    self.log(&id, Logged::Moved { path: w.path.clone(), cause, against: w.against.clone() });
                }
                said!(path = %self.shown(&path), "moved: changed since it was read, not written; its change is read next");
                None
            }
            Replaced::Failed(why) => {
                tracing::error!(target: ENGINE, path = %self.shown(&path), why, "not written");
                None
            }
        }
    }

    fn watch(&mut self, dir: &Path, whole: bool) -> Result<(), String> {
        if self.watched.iter().any(|(d, all)| d == dir || (*all && dir.starts_with(d))) {
            return Ok(());
        }
        self.world.watch(dir, whole)?;
        self.watched.push((dir.to_path_buf(), whole));
        Ok(())
    }

    /// A path as the trace says it: from the home folder (`~/…`), else whole.
    fn shown(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(within) => format!("~/{}", within.display()),
            Err(_) => path.display().to_string(),
        }
    }

    /// `entry`, about the file `file`, appended to the log: its number, nothing when it
    /// could not be written, which is said.
    fn log(&mut self, file: &FileId, entry: Logged) -> Option<u64> {
        let at = self.world.now();
        match self.log.append(&self.world, at, file, entry) {
            Ok(n) => Some(n),
            Err(why) => {
                tracing::error!(target: ENGINE, why, "not logged");
                None
            }
        }
    }

    /// `text` kept whole as the version it is (`history/versions`), so the log's every
    /// version can be shown and returned to.
    fn keep(&self, text: &str) {
        if let Err(why) = Log::keep(&self.world, text) {
            tracing::error!(target: ENGINE, why, "version not kept");
        }
    }

    /// The engine's own write of `text` over the file `id` at `path` made: the version kept,
    /// and the index told the file's new system ID (a write is a rename over the file).
    fn wrote(&mut self, id: &FileId, path: &Path, text: &str) {
        self.keep(text);
        let sys = self.world.file_id(path);
        self.index.seen(id, path, sys, &hash(text));
    }

    /// The index kept on disk when it changed.
    fn persist(&mut self) {
        if let Err(why) = self.index.save(&self.world) {
            tracing::error!(target: ENGINE, why, "index not kept");
        }
    }
}
