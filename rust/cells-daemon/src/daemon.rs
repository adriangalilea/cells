//! The daemon's loop: what the world says changed handed to the folder (`cells::folder`),
//! and what the folder decides carried out. Every decision of what to store is the
//! folder's; here is only the world's side of it: reading what was heard, telling a change
//! from the engine's own write coming back, writing only over the version a write was made
//! from, logging every version with what made it and the chain it set off, and saying
//! each step.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use cells::files::is_url;
use cells::folder::{Change, Edited, Folder, Write, key_of};

use crate::allowed::Approval;
use crate::history;
use crate::log::{Cause, Changed, Edit, Log, Logged, hash};
use crate::trace::Trace;
use crate::wire::{FromEngine, ToEngine};
use crate::world::{Found, Replaced, World, table_file};

/// What reaches the daemon, in the order it came: the disk's changes and its clients'
/// requests, through one channel, so the daemon is one thread and never shares its state.
pub enum Event {
    /// A path the disk says changed.
    Disk(PathBuf),
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
    /// Each folder served, canonical, as its log's root.
    served: Vec<Log>,
    trace: Trace,
    /// Each directory watched, and whether everything under it is.
    watched: Vec<(PathBuf, bool)>,
    /// Each client told what changes, and its name.
    listeners: BTreeMap<u64, (String, Sender<Vec<PathBuf>>)>,
    /// The files whose version changed since the clients were last told.
    changed: BTreeSet<PathBuf>,
    /// The version of a file each client said it holds (`ToEngine::Editing`): when that
    /// version lands, that client wrote it.
    holding: BTreeMap<PathBuf, Vec<Holding>>,
}

struct Holding {
    id: u64,
    client: String,
    version: String,
}

/// A change that is a root of history, logged once the folder said what it edited.
struct Root {
    path: PathBuf,
    was: Option<String>,
    now: Option<String>,
    kind: RootKind,
}

enum RootKind {
    External { by: Option<String> },
    Restore { by: String },
}

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
    if parts.is_empty() { "nothing a table holds".into() } else { parts.join(", ") }
}

/// The folder's key for a path: the path, absolute, as text.
fn key(path: &Path) -> String {
    path.to_str().unwrap_or_else(|| panic!("{} is not UTF-8: the engine keys files by their path as text", path.display())).to_owned()
}

impl<W: World> Daemon<W> {
    pub fn new(world: W, trace: Trace) -> Daemon<W> {
        Daemon {
            world,
            folder: Folder::default(),
            served: Vec::new(),
            trace,
            watched: Vec::new(),
            listeners: BTreeMap::new(),
            changed: BTreeSet::new(),
            holding: BTreeMap::new(),
        }
    }

    /// `root` (canonical) served from now on: locked, watched whole, every table file under
    /// it read as the start of its history, and the files they import from elsewhere read and
    /// watched too. A folder inside one served already is that one; a folder holding one,
    /// or the home folder or the disk's root whole, is refused.
    pub fn serve(&mut self, root: &Path) -> Result<PathBuf, String> {
        if let Some(holder) = self.root_of(root) {
            return Ok(holder.to_path_buf());
        }
        if root == self.world.home() || root.parent().is_none() {
            return Err(format!("{} is too wide to serve whole: serve the folder your files are in", root.display()));
        }
        if let Some(inner) = self.served.iter().find(|l| l.root.starts_with(root)) {
            return Err(format!(
                "{} holds {}, served already: serve the folders inside it one by one",
                root.display(),
                inner.root.display()
            ));
        }
        self.world.lock(root)?;
        self.watch(root, true)?;
        let (log, broken) = Log::open(root, &self.world);
        if let Some(why) = broken {
            self.say("broken", &format!("{why}: kept, and continued; what it says past there cannot be trusted"));
        }
        self.served.push(log);
        let held = self.folder.paths().count();
        let found = self.world.list(root);
        for path in &found {
            match self.world.read(path) {
                Found::Text(text) => {
                    self.keep(path, &text);
                    self.hand(Change::Seen { path: key(path), text }, None);
                }
                Found::Missing => {}
                Found::Unreadable(why) => self.say("unread", &format!("{}: {why}", self.shown(path))),
            }
        }
        let away = (self.folder.paths().count() - held).saturating_sub(found.len());
        let through = if away == 0 { String::new() } else { format!(", {away} more read through imports") };
        self.say("serving", &format!("{} ({} files{through})", root.display(), found.len()));
        Ok(root.to_path_buf())
    }

    /// Served until the process ends: each event as it comes, what the disk says changed
    /// read once the disk is still, and the clients told what changed after each.
    pub fn run(&mut self, events: Receiver<Event>) -> ! {
        loop {
            let first = events.recv().expect("the daemon's senders live as long as it does");
            let mut heard = BTreeSet::new();
            let mut asked = Vec::new();
            match first {
                Event::Disk(p) => {
                    heard.insert(p);
                }
                other => asked.push(other),
            }
            if !heard.is_empty() {
                let began = Instant::now();
                loop {
                    match events.recv_timeout(STILL.min(PATIENCE.saturating_sub(began.elapsed()))) {
                        Ok(Event::Disk(p)) => {
                            heard.insert(p);
                        }
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
            Event::Ask { id, client, request, answer } => {
                let said = match self.asked(id, &client, request) {
                    Ok(said) => said,
                    Err(why) => {
                        self.say("refused", &format!("{client} (#{id}): {why}"));
                        Some(FromEngine::Refused { why })
                    }
                };
                if let (Some(answer), Some(said)) = (answer, said) {
                    // the client may be gone already
                    let _ = answer.send(said);
                }
            }
            Event::Listen { id, client, tell } => {
                self.say("joined", &format!("{client} (#{id})"));
                self.listeners.insert(id, (client, tell));
            }
            Event::Leave { id } => {
                for held in self.holding.values_mut() {
                    held.retain(|h| h.id != id);
                }
                self.holding.retain(|_, held| !held.is_empty());
                if let Some((client, _)) = self.listeners.remove(&id) {
                    self.say("left", &format!("{client} (#{id})"));
                }
            }
        }
    }

    /// A client's request carried out: its answer, nothing for one never answered.
    fn asked(&mut self, id: u64, client: &str, request: ToEngine) -> Result<Option<FromEngine>, String> {
        Ok(Some(match request {
            ToEngine::Hello { .. } => unreachable!("a greeting is the server's"),
            ToEngine::Serve { folder } => FromEngine::Served { folder: self.serve(&folder)? },
            ToEngine::Allow { file, script, version } => {
                let approval =
                    Approval { file: file.clone(), script: script.clone(), version, by: client.to_owned(), at: self.world.now() };
                self.world.append(&crate::allowed::path(), &crate::allowed::line(&approval))?;
                self.say("allowed", &format!("{script} in {}, as shown to its person (by {client})", self.shown(&file)));
                FromEngine::Allowed { file, script }
            }
            ToEngine::Editing { file, version } => {
                let held = self.holding.entry(file).or_default();
                held.retain(|h| h.id != id);
                held.push(Holding { id, client: client.to_owned(), version });
                return Ok(None);
            }
            ToEngine::History { folder, file, limit } => {
                let chains = match &file {
                    Some(f) => history::chains(self.served.iter(), |p| p == f, limit),
                    None => history::chains(self.served.iter(), |p| p.starts_with(&folder), limit),
                };
                FromEngine::History { chains }
            }
            ToEngine::Cell { file, row, column } => {
                let log = self.log_of(&file)?;
                let doc = self.folder.doc(&key(&file)).ok_or(format!("{}: not read", self.shown(&file)))?;
                if row == 0 || row > doc.rows() {
                    return Err(format!("{}: no row {row}, it has {}", self.shown(&file), doc.rows()));
                }
                let key = key_of(doc, row - 1);
                FromEngine::Values { values: history::cell(log, &file, row, key.as_deref(), &column) }
            }
            ToEngine::Version { file, version } => {
                let log = self.log_of(&file)?;
                match self.world.read(&log.version(&version)) {
                    Found::Text(text) => FromEngine::Version { file, version, text },
                    _ => return Err(format!("{}: no version {version} kept", self.shown(&file))),
                }
            }
            ToEngine::Restore { file, version } => {
                let log = self.log_of(&file)?;
                let text = match self.world.read(&log.version(&version)) {
                    Found::Text(text) => text,
                    _ => return Err(format!("{}: no version {version} kept", self.shown(&file))),
                };
                let k = key(&file);
                let was = self.folder.text(&k).map(hash).ok_or(format!("{}: not held, so not returned to a version", self.shown(&file)))?;
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
                self.say("restored", &format!("{} to {} (by {client})", self.shown(&file), &version[..12]));
                self.changed.insert(file.clone());
                let root = Root {
                    path: file.clone(),
                    was: Some(was.clone()),
                    now: Some(version),
                    kind: RootKind::Restore { by: client.to_owned() },
                };
                self.hand(Change::Changed { path: k, text }, Some(root));
                FromEngine::Restored { file, was }
            }
        }))
    }

    /// The log of the folder served holding `file`.
    fn log_of(&self, file: &Path) -> Result<&Log, String> {
        self.served.iter().find(|l| file.starts_with(&l.root)).ok_or(format!("{}: in no folder served", file.display()))
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

    pub fn world(&self) -> &W {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut W {
        &mut self.world
    }

    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    /// What the disk said changed: each table file read, and handed in when its text is
    /// not what the folder holds (the engine's own write coming back, or a file touched,
    /// is the same text and changes nothing).
    pub fn hear(&mut self, paths: &BTreeSet<PathBuf>) {
        let tracked: Vec<&PathBuf> = paths.iter().filter(|p| self.tracked(p)).collect();
        if tracked.is_empty() {
            return;
        }
        let started = Instant::now();
        let heard: Vec<String> = tracked.iter().map(|p| self.shown(p)).collect();
        self.detail("heard", &heard.join(" "));
        let mut handed = 0;
        for path in tracked {
            let k = key(path);
            let was = self.folder.text(&k).map(hash);
            match self.world.read(path) {
                Found::Text(text) => {
                    if self.folder.text(&k) == Some(text.as_str()) {
                        continue;
                    }
                    let now = hash(&text);
                    let by = self.written_by(path, &now);
                    let whom = by.as_ref().map_or(String::new(), |b| format!(" by {b}"));
                    self.say(if was.is_some() { "changed" } else { "added" }, &format!("{}{whom}", self.shown(path)));
                    self.keep(path, &text);
                    self.changed.insert(path.to_path_buf());
                    let root = Root { path: path.to_path_buf(), was, now: Some(now), kind: RootKind::External { by } };
                    self.hand(Change::Changed { path: k, text }, Some(root));
                    handed += 1;
                }
                Found::Missing if was.is_some() => {
                    self.say("removed", &self.shown(path));
                    self.changed.insert(path.to_path_buf());
                    let root = Root { path: path.to_path_buf(), was, now: None, kind: RootKind::External { by: None } };
                    self.hand(Change::Gone { path: k }, Some(root));
                    handed += 1;
                }
                Found::Missing => {}
                Found::Unreadable(why) => self.say("unread", &format!("{}: {why}", self.shown(path))),
            }
        }
        let took = started.elapsed().as_secs_f64() * 1000.0;
        match handed {
            0 => self.detail("same", "as held: the engine's own writes, or files touched"),
            _ => self.detail("done", &format!("in {took:.1} ms, the disk's writes included")),
        }
    }

    /// Whether a path heard is a file the folder reads: a table file, never in a hidden
    /// folder or itself hidden (a temporary file an editor or this daemon writes beside it).
    fn tracked(&self, path: &Path) -> bool {
        let within = self.root_of(path).map_or(path, |root| path.strip_prefix(root).expect("under its root"));
        table_file(path) && !within.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
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
        let mut root = root.map(|r| (r, None::<Edit>));
        let mut cause: Option<(PathBuf, u64)> = None;
        let mut first = true;
        while let Some(change) = queue.pop_front() {
            let started = Instant::now();
            let out = self.folder.apply(change);
            let took = started.elapsed();
            if std::mem::take(&mut first)
                && let Some((r, edit)) = root.as_mut()
            {
                *edit = out.edited.as_ref().map(Edit::from);
                if let Some(e) = &out.edited {
                    self.detail("edited", &format!("{}: {}", self.shown(&r.path), said(e)));
                }
                // a root outside every folder served is logged where its first write is
                if self.root_of(&r.path).is_some() {
                    let at = r.path.clone();
                    let (r, edit) = root.take().expect("a root");
                    cause = self.log_root(&at, r, edit);
                }
            }
            for r in &out.read {
                let found = match &r.refused {
                    Some(why) => format!("refused: {why}"),
                    None => format!("{} problems, {} stale", r.problems, r.stale),
                };
                self.detail("read", &format!("{}: {found}", self.shown(Path::new(&r.path))));
            }
            if !out.read.is_empty() {
                let files = if out.read.len() == 1 { "file" } else { "files" };
                self.detail("settled", &format!("{} {files} in {:.1} ms", out.read.len(), took.as_secs_f64() * 1000.0));
            }
            for need in out.needs {
                queue.extend(self.needed(&need));
            }
            for w in out.writes {
                assert!(written.insert(w.path.clone()), "{} written twice for one change: the folder does not settle", w.path);
                if let Some((r, edit)) = root.take() {
                    cause = self.log_root(Path::new(&w.path), r, edit);
                }
                queue.extend(self.write(w, cause.as_ref()));
            }
        }
    }

    /// `root` logged in the log of the folder holding `at`: its folder and number.
    fn log_root(&mut self, at: &Path, root: Root, edited: Option<Edit>) -> Option<(PathBuf, u64)> {
        let log = self.served.iter().find(|l| at.starts_with(&l.root))?;
        let path = log.shown(&root.path);
        let entry = match root.kind {
            RootKind::External { by } => Logged::External { path, was: root.was, now: root.now, by, edited },
            RootKind::Restore { by } => Logged::Restore {
                path,
                was: root.was.expect("a restored file was held"),
                now: root.now.expect("a restored file holds the version returned to"),
                by,
                edited,
            },
        };
        let folder = log.root.clone();
        self.log(&folder, entry).map(|n| (folder, n))
    }

    /// A file a read named and the folder does not hold: watched where it is, and handed
    /// in when it can be read.
    fn needed(&mut self, need: &str) -> Option<Change> {
        if is_url(need) {
            self.detail("skipped", &format!("{need}: a URL import is fetched by `cells check` and `cells pin`, never by the daemon"));
            return None;
        }
        let path = PathBuf::from(need);
        if let Some(dir) = path.parent().filter(|d| d.is_absolute())
            && let Err(e) = self.watch(dir, false)
        {
            self.say("unwatched", &e);
        }
        match self.world.read(&path) {
            Found::Text(text) => Some(Change::Seen { path: need.to_owned(), text }),
            Found::Missing => {
                self.detail("missing", &format!("{}: imported, not there", self.shown(&path)));
                None
            }
            Found::Unreadable(why) => {
                self.say("unread", &format!("{}: {why}", self.shown(&path)));
                None
            }
        }
    }

    /// `w` written, only inside a folder served and only over the version it was made
    /// from; its text handed back to the folder once written.
    fn write(&mut self, w: Write, cause: Option<&(PathBuf, u64)>) -> Option<Change> {
        let path = PathBuf::from(&w.path);
        let Some(folder) = self.root_of(&path).map(Path::to_path_buf) else {
            self.say("skipped", &format!("{}: outside every folder served, never written", self.shown(&path)));
            return None;
        };
        // nothing when the root's line could not be written, which was said
        let cause = cause.map(|(root, n)| Cause { n: *n, folder: (*root != folder).then(|| root.clone()) });
        let shown = |p: &Path| p.strip_prefix(&folder).unwrap_or(p).to_string_lossy().into_owned();
        match self.world.replace(&path, &w.against, &w.text) {
            Replaced::Done => {
                let now = hash(&w.text);
                self.keep(&path, &w.text);
                let via: Vec<String> = w.via.iter().map(|v| shown(Path::new(v))).collect();
                if let Some(cause) = cause {
                    let entry = Logged::Store {
                        path: shown(&path),
                        cause,
                        via: via.clone(),
                        against: w.against.clone(),
                        now,
                        stored: w.stored.iter().map(Changed::from).collect(),
                    };
                    self.log(&folder, entry);
                }
                let cells: Vec<String> = w.stored.iter().map(|s| format!("{} row {} {} → {}", s.column, s.row, s.was, s.now)).collect();
                let why = if w.via == [w.path.clone()] { String::new() } else { format!(" for {}", via.join(", ")) };
                self.say("stored", &format!("{}{why}: {}", self.shown(&path), cells.join(", ")));
                self.changed.insert(path);
                Some(Change::Written { path: w.path, text: w.text })
            }
            Replaced::Moved => {
                if let Some(cause) = cause {
                    self.log(&folder, Logged::Moved { path: shown(&path), cause, against: w.against.clone() });
                }
                self.say("moved", &format!("{}: changed since it was read, not written; its change is read next", self.shown(&path)));
                None
            }
            Replaced::Failed(why) => {
                self.say("failed", &format!("{}: {why}", self.shown(&path)));
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

    fn root_of(&self, path: &Path) -> Option<&Path> {
        self.served.iter().map(|l| l.root.as_path()).find(|r| path.starts_with(r))
    }

    /// A path as the trace says it: from its folder served, else whole.
    fn shown(&self, path: &Path) -> String {
        match self.root_of(path) {
            Some(root) => path.strip_prefix(root).expect("under its root").display().to_string(),
            None => path.display().to_string(),
        }
    }

    /// `entry` appended to the log of the folder served at `folder`: its number, nothing
    /// when it could not be written, which is said.
    fn log(&mut self, folder: &Path, entry: Logged) -> Option<u64> {
        let at = self.world.now();
        let log = self.served.iter_mut().find(|l| l.root == folder).expect("a folder served");
        match log.append(&self.world, at, entry) {
            Ok(n) => Some(n),
            Err(e) => {
                self.trace.said(at, "unlogged", &e);
                None
            }
        }
    }

    /// `text`, a version of `path`, kept whole in its folder's `.cells/versions`, so the
    /// log's every version can be shown and returned to.
    fn keep(&self, path: &Path, text: &str) {
        let Some(log) = self.served.iter().find(|l| path.starts_with(&l.root)) else { return };
        if let Err(e) = log.keep(&self.world, text) {
            self.say("unkept", &e);
        }
    }

    fn say(&self, verb: &str, what: &str) {
        self.trace.said(self.world.now(), verb, what);
    }

    fn detail(&self, verb: &str, what: &str) {
        self.trace.traced(self.world.now(), verb, what);
    }
}
