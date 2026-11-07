//! The daemon's loop: what the world says changed handed to the folder (`cells::folder`),
//! and what the folder decides carried out. Every decision of what to store is the
//! folder's; here is only the world's side of it: reading what was heard, telling a change
//! from the engine's own write coming back, writing only over the version a write was made
//! from, logging every version, and saying each step.

use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::Instant;

use cells::files::is_url;
use cells::folder::{Change, Folder, Write};

use crate::log::{Entry, Log, hash};
use crate::trace::Trace;
use crate::world::{Found, Replaced, World, table_file};

pub struct Daemon<W: World> {
    world: W,
    folder: Folder,
    /// Each folder served, canonical, and its log.
    served: Vec<(PathBuf, Log)>,
    trace: Trace,
    /// Each directory watched, and whether everything under it is.
    watched: Vec<(PathBuf, bool)>,
}

/// The folder's key for a path: the path, absolute, as text.
fn key(path: &Path) -> String {
    path.to_str().unwrap_or_else(|| panic!("{} is not UTF-8: the engine keys files by their path as text", path.display())).to_owned()
}

impl<W: World> Daemon<W> {
    /// The daemon over `roots` (canonical folders): each locked and watched whole, every
    /// table file under each read as the start of its history, and the files they import
    /// from elsewhere read and watched too.
    pub fn start(mut world: W, roots: &[PathBuf], trace: Trace) -> Result<Daemon<W>, String> {
        let mut served = Vec::new();
        for root in roots {
            world.lock(root)?;
            served.push((root.clone(), Log::open(root, &world)));
        }
        let mut daemon = Daemon { world, folder: Folder::default(), served, trace, watched: Vec::new() };
        for root in roots {
            daemon.watch(root, true)?;
        }
        let found: Vec<PathBuf> = roots.iter().flat_map(|r| daemon.world.list(r)).collect();
        for path in &found {
            match daemon.world.read(path) {
                Found::Text(text) => daemon.hand(Change::Seen { path: key(path), text }),
                Found::Missing => {}
                Found::Unreadable(why) => daemon.say("unread", &format!("{}: {why}", daemon.shown(path))),
            }
        }
        let held = daemon.folder.paths().count();
        let away = held - found.len();
        let names: Vec<String> = roots.iter().map(|r| r.display().to_string()).collect();
        let through = if away == 0 { String::new() } else { format!(", {away} more read through imports") };
        daemon.say("serving", &format!("{} ({} files{through})", names.join(", "), found.len()));
        Ok(daemon)
    }

    /// Served until the process ends: every change heard handed in as it comes.
    pub fn run(&mut self) -> ! {
        loop {
            let heard = self.world.heard();
            self.hear(&heard);
        }
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
                    self.hand(Change::Changed { path: k, text });
                    handed += 1;
                }
                Found::Missing if was.is_some() => {
                    self.log(path, Entry::External { path, was: was.as_deref(), now: None });
                    self.say("removed", &self.shown(path));
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
