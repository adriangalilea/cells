//! The daemon's loop: what the world says changed handed to the folder (`cells::folder`),
//! and what the folder decides carried out. Every decision of what to store is the
//! folder's; here is only the world's side of it: reading what was heard, telling a change
//! from the engine's own write coming back, writing only over the version a write was made
//! from, logging every version, and saying each step.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use cells::files::is_url;
use cells::folder::{Change, Folder, Write};

use crate::log::{Entry, Log, hash};
use crate::trace::Trace;
use crate::world::{Found, Replaced, World, table_file};

/// What reaches the daemon, in the order it came: the disk's changes and its clients'
/// requests, through one channel, so the daemon is one thread and never shares its state.
pub enum Event {
    /// A path the disk says changed.
    Disk(PathBuf),
    /// A folder to serve: answered with the folder served (the one asked, or the folder
    /// served already holding it), or why not.
    Serve {
        folder: PathBuf,
        answer: Sender<Result<PathBuf, String>>,
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
    /// Each folder served, canonical, and its log.
    served: Vec<(PathBuf, Log)>,
    trace: Trace,
    /// Each directory watched, and whether everything under it is.
    watched: Vec<(PathBuf, bool)>,
    /// Each client told what changes, and its name.
    listeners: BTreeMap<u64, (String, Sender<Vec<PathBuf>>)>,
    /// The files whose version changed since the clients were last told.
    changed: BTreeSet<PathBuf>,
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
        if let Some((inner, _)) = self.served.iter().find(|(r, _)| r.starts_with(root)) {
            return Err(format!("{} holds {}, served already: serve the folders inside it one by one", root.display(), inner.display()));
        }
        self.world.lock(root)?;
        self.watch(root, true)?;
        self.served.push((root.to_path_buf(), Log::open(root, &self.world)));
        let held = self.folder.paths().count();
        let found = self.world.list(root);
        for path in &found {
            match self.world.read(path) {
                Found::Text(text) => self.hand(Change::Seen { path: key(path), text }),
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
            Event::Serve { folder, answer } => {
                let served = self.serve(&folder);
                if let Err(why) = &served {
                    self.say("refused", why);
                }
                // the client may be gone already
                let _ = answer.send(served);
            }
            Event::Listen { id, client, tell } => {
                self.say("joined", &format!("{client} (#{id})"));
                self.listeners.insert(id, (client, tell));
            }
            Event::Leave { id } => {
                if let Some((client, _)) = self.listeners.remove(&id) {
                    self.say("left", &format!("{client} (#{id})"));
                }
            }
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
                    self.log(path, Entry::External { path, was: was.as_deref(), now: Some(&now) });
                    self.say(if was.is_some() { "changed" } else { "added" }, &self.shown(path));
                    self.changed.insert(path.to_path_buf());
                    self.hand(Change::Changed { path: k, text });
                    handed += 1;
                }
                Found::Missing if was.is_some() => {
                    self.log(path, Entry::External { path, was: was.as_deref(), now: None });
                    self.say("removed", &self.shown(path));
                    self.changed.insert(path.to_path_buf());
                    self.hand(Change::Gone { path: k });
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

    /// `change` handed to the folder, and everything it leads to, until the folder settles:
    /// each file it needs read and handed in, each write made and its text handed back.
    fn hand(&mut self, change: Change) {
        let mut queue = VecDeque::from([change]);
        // A file written twice for one change means the folder never settles: a write
        // stores what a second would, so this is a bug, and it stops here.
        let mut written: BTreeSet<String> = BTreeSet::new();
        while let Some(change) = queue.pop_front() {
            let started = Instant::now();
            let out = self.folder.apply(change);
            let took = started.elapsed();
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
                queue.extend(self.write(w));
            }
        }
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
    fn write(&mut self, w: Write) -> Option<Change> {
        let path = PathBuf::from(&w.path);
        if self.root_of(&path).is_none() {
            self.say("skipped", &format!("{}: outside every folder served, never written", self.shown(&path)));
            return None;
        }
        match self.world.replace(&path, &w.against, &w.text) {
            Replaced::Done => {
                let now = hash(&w.text);
                let because = PathBuf::from(&w.because);
                self.log(&path, Entry::Store { path: &path, because: &because, against: &w.against, now: &now, stored: &w.stored });
                let cells: Vec<String> = w.stored.iter().map(|s| format!("{} row {} {} → {}", s.column, s.row, s.was, s.now)).collect();
                let why = if w.because == w.path { String::new() } else { format!(" for {}", self.shown(&because)) };
                self.say("stored", &format!("{}{why}: {}", self.shown(&path), cells.join(", ")));
                self.changed.insert(path);
                Some(Change::Written { path: w.path, text: w.text })
            }
            Replaced::Moved => {
                self.log(&path, Entry::Moved { path: &path, against: &w.against });
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
        self.served.iter().map(|(r, _)| r.as_path()).find(|r| path.starts_with(r))
    }

    /// A path as the trace says it: from its folder served, else whole.
    fn shown(&self, path: &Path) -> String {
        match self.root_of(path) {
            Some(root) => path.strip_prefix(root).expect("under its root").display().to_string(),
            None => path.display().to_string(),
        }
    }

    fn log(&mut self, path: &Path, entry: Entry) {
        let at = self.world.now();
        let Some((_, log)) = self.served.iter_mut().find(|(r, _)| path.starts_with(r)) else { return };
        if let Err(e) = log.append(&self.world, at, entry) {
            self.trace.said(at, "unlogged", &e);
        }
    }

    fn say(&self, verb: &str, what: &str) {
        self.trace.said(self.world.now(), verb, what);
    }

    fn detail(&self, verb: &str, what: &str) {
        self.trace.traced(self.world.now(), verb, what);
    }
}
