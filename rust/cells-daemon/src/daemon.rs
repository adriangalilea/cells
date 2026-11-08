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

use cells::Trigger;

use crate::allowed::{Allowed, Approval};
use crate::history;
use crate::log::{Cause, Changed, Edit, Log, Logged, Outcome, hash};
use crate::trace::ENGINE;
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
    /// Each folder served, canonical, as its log's root.
    served: Vec<Log>,
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
    path: PathBuf,
    was: Option<String>,
    now: Option<String>,
    kind: RootKind,
}

enum RootKind {
    External { by: Option<String> },
    Restore { by: String },
    Run { script: String, run: u64, outcome: Outcome, by: String },
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

/// The folder's key for a path: the path, absolute, as text.
fn key(path: &Path) -> String {
    path.to_str().unwrap_or_else(|| panic!("{} is not UTF-8: the engine keys files by their path as text", path.display())).to_owned()
}

impl<W: World> Daemon<W> {
    /// The daemon over `world`, a run that ends saying so on `back`, the channel `run`
    /// reads.
    pub fn new(world: W, back: Sender<Event>) -> Daemon<W> {
        Daemon {
            world,
            folder: Folder::default(),
            served: Vec::new(),
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
        }
    }

    /// `root` (canonical) served from now on: locked, watched whole, every table file under
    /// it read as the start of its history, and the files they import from elsewhere read and
    /// watched too. A folder inside one served already is that one. A folder holding ones
    /// served already takes them in: their files stay held, each logging where its history
    /// began (the folder nearest it), so a book opened from a quarter's file first and from
    /// its index next is one book. The home folder or the disk's root whole is refused.
    pub fn serve(&mut self, root: &Path) -> Result<PathBuf, String> {
        if let Some(holder) = self.served.iter().map(|l| l.root.as_path()).find(|r| root.starts_with(r)) {
            return Ok(holder.to_path_buf());
        }
        if root == self.world.home() || root.parent().is_none() {
            return Err(format!("{} is too wide to serve whole: serve the folder your files are in", root.display()));
        }
        let span = tracing::info_span!(target: ENGINE, "serve", path = %root.display());
        let _serving = span.enter();
        self.world.lock(root)?;
        self.watch(root, true)?;
        let (log, broken) = Log::open(root, &self.world);
        if let Some(why) = broken {
            warned!(why, "log broken: kept and continued, what it says past there cannot be trusted");
        }
        self.served.push(log);
        let held = self.folder.paths().count();
        let found = self.world.list(root);
        for path in &found {
            // a file of a folder served already, taken in, is held as it was
            if self.folder.text(&key(path)).is_some() {
                continue;
            }
            match self.world.read(path) {
                Found::Text(text) => {
                    self.keep(path, &text);
                    self.hand(Change::Seen { path: key(path), text }, None);
                }
                Found::Missing => {}
                Found::Unreadable(why) => warned!(path = %self.shown(path), why, "unread"),
            }
        }
        let through = (self.folder.paths().count() - held).saturating_sub(found.len());
        said!(files = found.len(), through_imports = through, "serving");
        Ok(root.to_path_buf())
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
            Event::Ran(running, returned) => self.land(running, returned),
            Event::Ask { id, client, request: ToEngine::Run { file, script }, answer } => {
                let answer = answer.expect("a run is answered");
                if let Err(why) = self.start(&file, &script, &client, Some(answer.clone())) {
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
    }

    /// A client's request carried out: its answer, nothing for one never answered.
    fn asked(&mut self, id: u64, client: &str, request: ToEngine) -> Result<Option<FromEngine>, String> {
        Ok(Some(match request {
            ToEngine::Hello { .. } => unreachable!("a greeting is the server's"),
            ToEngine::Run { .. } => unreachable!("a run is answered when it lands"),
            ToEngine::Serve { folder } => FromEngine::Served { folder: self.serve(&folder)? },
            ToEngine::Allow { file, script, version } => {
                let approval =
                    Approval { file: file.clone(), script: script.clone(), version, by: client.to_owned(), at: self.world.now() };
                self.world.append(&crate::allowed::path(), &crate::allowed::line(&approval))?;
                said!(path = %self.shown(&file), script, by = client, "allowed, as shown to its person");
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
                let span = tracing::info_span!(target: ENGINE, "restore", path = %self.shown(&file), to = &version[..12], by = client);
                let _restoring = span.enter();
                self.changed.insert(file.clone());
                self.touched.insert(file.clone());
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

    /// Every script of every file served looked at: each due run started when its person
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
        let held: Vec<PathBuf> = self.folder.paths().map(PathBuf::from).filter(|p| self.root_of(p).is_some()).collect();
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
                if let Err(why) = self.start(&path, &script.name, &by, None) {
                    warned!(path = %self.shown(&path), script = script.name, why, "not run");
                }
            }
        }
        Duration::from_millis(soonest.saturating_sub(now)).min(GLANCE)
    }

    /// The script `name` of `file` started, as its person allowed it, by `by` (its schedule,
    /// a change, a client), `waiting` told once it lands; joining a run under way.
    fn start(&mut self, file: &Path, name: &str, by: &str, waiting: Option<Sender<FromEngine>>) -> Result<(), String> {
        let id = (file.to_path_buf(), name.to_owned());
        if let Some(waiters) = self.running.get_mut(&id) {
            waiters.extend(waiting);
            return Ok(());
        }
        let doc = self.folder.doc(&key(file)).ok_or(format!("{}: not read by the engine; is its folder served?", self.shown(file)))?;
        let script = doc.scripts().iter().find(|s| s.name == name).ok_or(format!("{} has no script {name}", self.shown(file)))?.clone();
        let allowed = match self.world.read(&crate::allowed::path()) {
            Found::Text(t) => Allowed::parse(&t),
            _ => Allowed::default(),
        };
        if !allowed.allows(file, &script) {
            return Err(format!(
                "{name} is not allowed on this Mac as it is written: `cells allow {} {name}` shows its code and asks",
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
        self.keep(&running.file, &written);
        self.changed.insert(running.file.clone());
        self.touched.insert(running.file.clone());
        let root = Root {
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

    /// The log of the folder served holding `file`.
    fn log_of(&self, file: &Path) -> Result<&Log, String> {
        self.log_for(file).ok_or(format!("{}: in no folder served", file.display()))
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

    /// What the disk said changed: each table file read, and handed in when its text is
    /// not what the folder holds (the engine's own write coming back, or a file touched,
    /// is the same text and changes nothing). Each change is a span, ending once every
    /// file it reached is stored.
    pub fn hear(&mut self, paths: &BTreeSet<PathBuf>) {
        let tracked: Vec<&PathBuf> = paths.iter().filter(|p| self.tracked(p)).collect();
        if tracked.is_empty() {
            return;
        }
        let heard: Vec<String> = tracked.iter().map(|p| self.shown(p)).collect();
        detail!(paths = heard.join(" "), "heard");
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
                    let what = if was.is_some() { "changed" } else { "added" };
                    let span = tracing::info_span!(target: ENGINE, "change", path = %self.shown(path), what, by = by.as_deref());
                    let _changing = span.enter();
                    self.keep(path, &text);
                    self.changed.insert(path.to_path_buf());
                    self.touched.insert(path.to_path_buf());
                    let root = Root { path: path.to_path_buf(), was, now: Some(now), kind: RootKind::External { by } };
                    self.hand(Change::Changed { path: k, text }, Some(root));
                    handed += 1;
                }
                Found::Missing if was.is_some() => {
                    let span = tracing::info_span!(target: ENGINE, "change", path = %self.shown(path), what = "removed");
                    let _changing = span.enter();
                    self.changed.insert(path.to_path_buf());
                    self.touched.insert(path.to_path_buf());
                    let root = Root { path: path.to_path_buf(), was, now: None, kind: RootKind::External { by: None } };
                    self.hand(Change::Gone { path: k }, Some(root));
                    handed += 1;
                }
                Found::Missing => {}
                Found::Unreadable(why) => warned!(path = %self.shown(path), why, "unread"),
            }
        }
        if handed == 0 {
            detail!("same as held: the engine's own writes, or files touched");
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
                    said!(cells = said(e), "edited");
                }
                // a root outside every folder served is logged where its first write is
                if self.root_of(&r.path).is_some() {
                    let at = r.path.clone();
                    let (r, edit) = root.take().expect("a root");
                    cause = self.log_root(&at, r, edit);
                }
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
                if let Some((r, edit)) = root.take() {
                    cause = self.log_root(Path::new(&w.path), r, edit);
                }
                queue.extend(self.write(w, cause.as_ref()));
            }
        }
    }

    /// `root` logged in the log of the folder holding `at`: its folder and number.
    fn log_root(&mut self, at: &Path, root: Root, edited: Option<Edit>) -> Option<(PathBuf, u64)> {
        let log = self.log_for(at)?;
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
        };
        let folder = log.root.clone();
        self.log(&folder, entry).map(|n| (folder, n))
    }

    /// A file a read named and the folder does not hold: watched where it is, and handed
    /// in when it can be read.
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
        match self.world.read(&path) {
            Found::Text(text) => Some(Change::Seen { path: need.to_owned(), text }),
            Found::Missing => {
                detail!(path = %self.shown(&path), "imported, not there");
                None
            }
            Found::Unreadable(why) => {
                warned!(path = %self.shown(&path), why, "unread");
                None
            }
        }
    }

    /// `w` written, only inside a folder served and only over the version it was made
    /// from; its text handed back to the folder once written.
    fn write(&mut self, w: Write, cause: Option<&(PathBuf, u64)>) -> Option<Change> {
        let path = PathBuf::from(&w.path);
        let Some(folder) = self.root_of(&path).map(Path::to_path_buf) else {
            said!(path = %self.shown(&path), "skipped: outside every folder served, never written");
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
                let via = (w.via != [w.path.clone()]).then(|| via.join(", "));
                said!(path = %self.shown(&path), via, cells = cells.join(", "), "stored");
                self.changed.insert(path);
                Some(Change::Written { path: w.path, text: w.text })
            }
            Replaced::Moved => {
                if let Some(cause) = cause {
                    self.log(&folder, Logged::Moved { path: shown(&path), cause, against: w.against.clone() });
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

    /// The log of the folder served nearest `path`.
    fn log_for(&self, path: &Path) -> Option<&Log> {
        let root = self.root_of(path)?;
        self.served.iter().find(|l| l.root == root)
    }

    /// The folder served nearest `path`: the one its history is logged in.
    fn root_of(&self, path: &Path) -> Option<&Path> {
        self.served.iter().map(|l| l.root.as_path()).filter(|r| path.starts_with(r)).max_by_key(|r| r.components().count())
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
            Err(why) => {
                tracing::error!(target: ENGINE, why, "not logged");
                None
            }
        }
    }

    /// `text`, a version of `path`, kept whole in its folder's `.cells/versions`, so the
    /// log's every version can be shown and returned to.
    fn keep(&self, path: &Path, text: &str) {
        let Some(log) = self.log_for(path) else { return };
        if let Err(why) = log.keep(&self.world, text) {
            tracing::error!(target: ENGINE, why, "version not kept");
        }
    }
}
