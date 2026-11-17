//! The files the engine holds, each by an identity of its own (`FileId`, eight base-32
//! characters, minted once, as a row id is), with its path, the system's file ID and the
//! SHA-256 of the text last held. What is held: every `.cells` file under the home folder,
//! every file one of them imports (a CSV included) and every file a client names; a CSV
//! nobody imports is not. A file keeps its identity however it moves, by one rule
//! (`Index::follow`), so its history is every line of its identity, wherever it went. Kept
//! as `index.json` beside the history (`log::folder`), so a restart reads no history to
//! know its files.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::world::{Found, World};

/// A file's identity: eight characters of Crockford's base 32 in lower case, the engine's
/// own, never the file's path or the system's ID, so a file moved, saved over by an editor
/// or copied across disks is still the file it was.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FileId(String);

impl FileId {
    /// An identity from random `bits`, drawn again while it is one `taken` says is known.
    pub fn mint(mut bits: impl FnMut() -> u64, taken: impl Fn(&FileId) -> bool) -> FileId {
        loop {
            let id = FileId(String::from_utf8(cells::value::id_from(bits()).to_vec()).expect("base 32 is ASCII"));
            if !taken(&id) {
                return id;
            }
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for FileId {
    type Error = String;
    fn try_from(s: String) -> Result<FileId, String> {
        if !cells::value::is_id(s.as_bytes()) {
            return Err(format!("{s:?} is no file id: eight characters of 0-9 and a-z but i, l, o, u"));
        }
        Ok(FileId(s))
    }
}

impl From<FileId> for String {
    fn from(id: FileId) -> String {
        id.0
    }
}

impl std::fmt::Display for FileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A file the engine holds, or held and saw go.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Known {
    pub id: FileId,
    /// Where it is; for a file gone, where it last was.
    pub path: PathBuf,
    /// The system's ID of the file at `path` when last seen (the inode, which FSEvents names
    /// a file by): what follows a rename. Unknown where the disk gives none.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sys: Option<u64>,
    /// The SHA-256 of the text last held: what finds it again once its ID is lost (a copy
    /// across disks, iCloud's replacement of a file).
    pub hash: String,
    /// When it was seen gone (milliseconds since the epoch), found nowhere since: remembered
    /// for `FORGOTTEN_AFTER`, so a file of its text come anywhere in that time, across restarts,
    /// is it. Its history outlives it in the log, under its identity.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gone: Option<u64>,
}

/// How long a file gone is remembered: a Trash's Put Back, a download of it again, iCloud
/// giving it back. Past that, a file of its text is a new file, and the index stays the size
/// of what the person keeps.
pub const FORGOTTEN_AFTER: u64 = 90 * 24 * 60 * 60 * 1000;

/// A path the disk said changed, and the system's ID of the file its event was about when
/// the watcher says it (FSEvents' extended data; `notify` says none).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heard {
    pub path: PathBuf,
    pub id: Option<u64>,
}

/// A path looked at: the IDs its events named, and the file there now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Looked {
    pub path: PathBuf,
    pub heard: Vec<u64>,
    pub now: Option<There>,
}

/// A file at a path: the SHA-256 of its text, and its system ID when the disk gives one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct There {
    pub hash: String,
    pub sys: Option<u64>,
}

/// How a move was known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum By {
    /// Rule 1: the system's ID an event at the path it left named is the file's where it came.
    Heard,
    /// Rule 3: the system's ID the file had, held since it was last seen, is the file's
    /// where it came.
    Held,
    /// Rule 4: its text, exactly the text it last held, alone of its text or of its name.
    Text,
}

impl By {
    pub fn said(self) -> &'static str {
        match self {
            By::Heard => "its file ID, heard",
            By::Held => "its file ID",
            By::Text => "its text",
        }
    }
}

/// What became of a file looked at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fate {
    /// The file held at `path`, there still: its text the same or another, its system ID
    /// the same or another (rule 2: an editor's atomic save replaces the file).
    Stayed { id: FileId, path: PathBuf },
    /// The file held at `from`, or last there when it was seen go before this look, is the
    /// one at `to`.
    Moved { id: FileId, from: PathBuf, to: PathBuf, by: By },
    /// The file held at `path` is nowhere looked at: remembered gone (rule 5).
    Gone { id: FileId, path: PathBuf },
    /// A file at `path` that is none held (rule 5): held from now on when it is one the
    /// engine holds.
    New { path: PathBuf },
}

/// Whether the walk and the watch take in `path`: under the home folder (`home`), never in a
/// hidden folder or itself hidden (`.git`, `~/.Trash`, an editor's temporary file), never
/// in `~/Library` (the system's and every app's own files) but for `~/Library/Mobile
/// Documents`, where iCloud Drive keeps the person's files; a path outside home, by its own
/// components alone. The one rule: `World::files` walks by it and the engine hears by it.
pub fn visible(home: &Path, path: &Path) -> bool {
    let within = path.strip_prefix(home).unwrap_or(path);
    let library = path.starts_with(home.join("Library")) && !path.starts_with(icloud(home));
    !within.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) && !library
}

/// Where iCloud Drive keeps the person's files, inside `~/Library`.
pub fn icloud(home: &Path) -> PathBuf {
    home.join("Library/Mobile Documents")
}

/// Every file the engine holds and every one it saw go, by identity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Index {
    files: Vec<Known>,
    /// Changed since it was last kept on disk.
    dirty: bool,
}

/// Where the index is kept.
pub fn path() -> PathBuf {
    crate::log::folder().join("index.json")
}

impl Index {
    /// The index kept on disk, or none for an engine that has kept none yet. One the disk
    /// will not give, or that is no index, is refused: files known again from nothing would
    /// be new files, and every history would fork.
    pub fn open(world: &impl World) -> Result<Index, String> {
        let at = path();
        let text = match world.read(&at) {
            Found::Text(t) => t,
            Found::Missing => return Ok(Index::default()),
            Found::Unreadable(why) => return Err(format!("{}: {why}", at.display())),
        };
        let files: Vec<Known> = serde_json::from_str(&text).map_err(|e| format!("{}: no index: {e}", at.display()))?;
        let index = Index { files, dirty: false };
        index.holds_together();
        Ok(index)
    }

    /// Kept on disk when it changed since.
    pub fn save(&mut self, world: &impl World) -> Result<(), String> {
        if !std::mem::take(&mut self.dirty) {
            return Ok(());
        }
        self.holds_together();
        let lines: Vec<String> = self.files.iter().map(|k| serde_json::to_string(k).expect("a file known serializes")).collect();
        world.save(&path(), &format!("[\n{}\n]\n", lines.join(",\n")))
    }

    /// Every identity once, and every path held by one file at most.
    fn holds_together(&self) {
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for k in &self.files {
            assert!(ids.insert(&k.id), "{} known twice", k.id);
            assert!(k.gone.is_some() || paths.insert(&k.path), "{} held by two files", k.path.display());
        }
    }

    /// The file held at `path`, not gone.
    pub fn at(&self, path: &Path) -> Option<&Known> {
        self.files.iter().find(|k| k.gone.is_none() && k.path == path)
    }

    pub fn get(&self, id: &FileId) -> &Known {
        self.files.iter().find(|k| k.id == *id).unwrap_or_else(|| panic!("{id} is no file known"))
    }

    /// Every file held, not gone.
    pub fn held(&self) -> impl Iterator<Item = &Known> {
        self.files.iter().filter(|k| k.gone.is_none())
    }

    /// Every file known, gone ones included.
    pub fn known(&self) -> &[Known] {
        &self.files
    }

    /// A file held from now on at `path`: its identity, minted from `bits` (drawn again
    /// while it names one known).
    pub fn add(&mut self, path: &Path, sys: Option<u64>, hash: &str, mut bits: impl FnMut() -> u64) -> FileId {
        assert!(self.at(path).is_none(), "{} added, held already", path.display());
        let id = FileId::mint(&mut bits, |id| self.files.iter().any(|k| k.id == *id));
        self.files.push(Known { id: id.clone(), path: path.to_path_buf(), sys, hash: hash.to_owned(), gone: None });
        self.dirty = true;
        id
    }

    /// A file known, with its identity: `id` taken as it is (a history read in, whose lines
    /// name it already).
    pub fn insert(&mut self, known: Known) {
        assert!(!self.files.iter().any(|k| k.id == known.id), "{} known twice", known.id);
        assert!(known.gone.is_some() || self.at(&known.path).is_none(), "{} held by two files", known.path.display());
        self.files.push(known);
        self.dirty = true;
    }

    /// `id` seen at `path`, holding the text whose SHA-256 is `hash`, with the system ID
    /// `sys`: where it stayed, moved or came back. Mid-moment a path may be held twice (two
    /// files trading places, one recorded before the other): the index holds together again by
    /// the time it is saved, where that is checked.
    pub fn seen(&mut self, id: &FileId, path: &Path, sys: Option<u64>, hash: &str) {
        let k = self.files.iter_mut().find(|k| k.id == *id).unwrap_or_else(|| panic!("{id} is no file known"));
        let now = Known { id: id.clone(), path: path.to_path_buf(), sys, hash: hash.to_owned(), gone: None };
        if *k != now {
            *k = now;
            self.dirty = true;
        }
    }

    /// `id` seen gone at `now` (milliseconds since the epoch).
    pub fn gone(&mut self, id: &FileId, now: u64) {
        let k = self.files.iter_mut().find(|k| k.id == *id).unwrap_or_else(|| panic!("{id} is no file known"));
        assert!(k.gone.is_none(), "{id} gone twice");
        k.gone = Some(now);
        self.dirty = true;
    }

    /// The files gone longer than `FORGOTTEN_AFTER` at `now` (milliseconds since the epoch)
    /// forgotten: called before every `follow`, so none of them is found again.
    pub fn forget(&mut self, now: u64) {
        let before = self.files.len();
        self.files.retain(|k| k.gone.is_none_or(|at| now.saturating_sub(at) < FORGOTTEN_AFTER));
        self.dirty |= self.files.len() != before;
    }

    /// What became of each file `looked` reaches, by one rule, in order:
    ///
    /// 1. The system's ID an event at a path held named, leaving it, is the ID of the file
    ///    at another path looked at: that file, moved.
    /// 2. A path held still holds a file, whatever its ID now (an editor's atomic save
    ///    replaces the file): the same file.
    /// 3. The system's ID a file had when last seen is the ID of the file at another path
    ///    looked at: that file, moved.
    /// 4. A file held whose path holds none now, or one remembered gone, and a file come with
    ///    exactly the text it last held: the one come of that text, else the one of its name
    ///    among several; and of the gone files of that text, it alone, else it alone of that
    ///    name: moved. Never guessed beyond that.
    /// 5. Otherwise a file come is new, and a file held whose path holds none is gone,
    ///    remembered by its identity, so rule 4 finds it later, anywhere, across restarts.
    ///
    /// Rules 1 and 3 come before 2, so a file moved and another put at its path in one
    /// moment is followed, and the one put there is new.
    pub fn follow(&self, looked: &[Looked]) -> Vec<Fate> {
        let paths: BTreeSet<&Path> = looked.iter().map(|l| l.path.as_path()).collect();
        assert_eq!(paths.len(), looked.len(), "a path looked at twice in one look");
        // the system IDs of the file at a path now: the disk's word, else its events'
        let ids_now = |l: &Looked| -> Vec<u64> {
            match &l.now {
                Some(t) => t.sys.map_or_else(|| l.heard.clone(), |s| vec![s]),
                None => Vec::new(),
            }
        };
        let held: Vec<(usize, &Known)> = looked.iter().enumerate().filter_map(|(i, l)| self.at(&l.path).map(|k| (i, k))).collect();
        // a path whose file is another than the one held there, or none held: one a file
        // held elsewhere may have come to
        let foreign = |i: usize| -> bool {
            let now = ids_now(&looked[i]);
            looked[i].now.is_some()
                && match self.at(&looked[i].path) {
                    None => true,
                    Some(k) => !now.is_empty() && k.sys.is_some_and(|s| !now.contains(&s)),
                }
        };
        let mut fates: Vec<Fate> = Vec::new();
        // the paths a file held elsewhere came to, by its system ID (rules 1 and 3)
        let mut claimed: BTreeSet<usize> = BTreeSet::new();
        let mut moved: BTreeSet<&FileId> = BTreeSet::new();
        for &(q, k) in &held {
            let now = ids_now(&looked[q]);
            let leaving: Vec<(u64, By)> = looked[q]
                .heard
                .iter()
                .map(|&s| (s, By::Heard))
                .chain(k.sys.map(|s| (s, By::Held)))
                .filter(|(s, _)| !now.contains(s))
                .collect();
            if leaving.is_empty() {
                continue;
            }
            let came: Vec<(usize, By)> = (0..looked.len())
                .filter(|&p| p != q && foreign(p) && !claimed.contains(&p))
                .filter_map(|p| {
                    let there = ids_now(&looked[p]);
                    let by = leaving.iter().filter(|(s, _)| there.contains(s)).map(|(_, b)| *b).min_by_key(|b| *b != By::Held)?;
                    Some((p, by))
                })
                .collect();
            // two files of one ID at once are hard links: not a move to tell by it
            if let [(p, by)] = came[..] {
                claimed.insert(p);
                moved.insert(&k.id);
                fates.push(Fate::Moved { id: k.id.clone(), from: k.path.clone(), to: looked[p].path.clone(), by });
            }
        }
        let mut gone: Vec<&Known> = Vec::new();
        let mut stayed: BTreeSet<usize> = BTreeSet::new();
        for &(q, k) in &held {
            if moved.contains(&k.id) {
                continue;
            }
            if looked[q].now.is_some() && !claimed.contains(&q) {
                stayed.insert(q);
                fates.push(Fate::Stayed { id: k.id.clone(), path: k.path.clone() });
            } else {
                gone.push(k);
            }
        }
        let remembered: Vec<&Known> = self.files.iter().filter(|k| k.gone.is_some()).collect();
        let away: Vec<&Known> = gone.iter().copied().chain(remembered).collect();
        let come: Vec<usize> =
            (0..looked.len()).filter(|i| looked[*i].now.is_some() && !stayed.contains(i) && !claimed.contains(i)).collect();
        let hash = |i: usize| looked[i].now.as_ref().map(|t| t.hash.as_str()).expect("a file come is there");
        let name = |p: &Path| p.file_name().map(|n| n.to_owned());
        // the one of `of` its text names, else the one of them of `named`'s name
        fn one<T: Copy>(of: &[T], named: impl Fn(T) -> bool) -> Option<T> {
            match of {
                [one] => Some(*one),
                _ => match of.iter().copied().filter(|t| named(*t)).collect::<Vec<_>>()[..] {
                    [one] => Some(one),
                    _ => None,
                },
            }
        }
        let mut found: BTreeSet<usize> = BTreeSet::new();
        let mut found_gone: BTreeSet<&FileId> = BTreeSet::new();
        for k in &away {
            let same: Vec<usize> = come.iter().copied().filter(|&c| hash(c) == k.hash).collect();
            let Some(c) = one(&same, |c| name(&looked[c].path) == name(&k.path)) else { continue };
            let rivals: Vec<&Known> = away.iter().copied().filter(|g| g.hash == hash(c)).collect();
            let Some(g) = one(&rivals, |g| name(&g.path) == name(&looked[c].path)) else { continue };
            if g.id != k.id || found.contains(&c) {
                continue;
            }
            found.insert(c);
            found_gone.insert(&k.id);
            fates.push(Fate::Moved { id: k.id.clone(), from: k.path.clone(), to: looked[c].path.clone(), by: By::Text });
        }
        for k in gone {
            if !found_gone.contains(&k.id) {
                fates.push(Fate::Gone { id: k.id.clone(), path: k.path.clone() });
            }
        }
        for c in come {
            if !found.contains(&c) {
                fates.push(Fate::New { path: looked[c].path.clone() });
            }
        }
        fates
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(index: &mut Index, path: &str, sys: Option<u64>, hash: &str) -> FileId {
        let mut n = index.files.len() as u64;
        index.add(Path::new(path), sys, hash, || {
            n += 1;
            n.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        })
    }

    fn looked(path: &str, heard: &[u64], now: Option<(&str, Option<u64>)>) -> Looked {
        Looked { path: path.into(), heard: heard.to_vec(), now: now.map(|(h, sys)| There { hash: h.into(), sys }) }
    }

    #[test]
    fn an_id_minted_is_a_row_id_s_shape_and_unique() {
        let mut index = Index::default();
        let a = index.add(Path::new("/h/a.cells"), None, "x", || 7);
        let b = index.add(Path::new("/h/b.cells"), None, "x", {
            let mut n = 6;
            move || {
                n += 1;
                n
            }
        });
        assert!(cells::value::is_id(a.as_str().as_bytes()));
        assert_ne!(a, b, "drawn again while it names one known");
        assert!(FileId::try_from("not-an-id".to_owned()).is_err());
    }

    #[test]
    fn rule_1_an_id_heard_leaving_is_the_file_where_it_came() {
        let mut index = Index::default();
        let id = known(&mut index, "/h/a.cells", None, "x");
        let fates = index.follow(&[looked("/h/a.cells", &[9], None), looked("/h/b/a.cells", &[9], Some(("y", None)))]);
        assert_eq!(fates, [Fate::Moved { id, from: "/h/a.cells".into(), to: "/h/b/a.cells".into(), by: By::Heard }]);
    }

    #[test]
    fn rule_2_a_path_saved_over_with_another_id_is_the_same_file() {
        let mut index = Index::default();
        let id = known(&mut index, "/h/a.cells", Some(1), "x");
        let fates = index.follow(&[looked("/h/a.cells", &[], Some(("y", Some(2))))]);
        assert_eq!(fates, [Fate::Stayed { id, path: "/h/a.cells".into() }]);
    }

    /// As FSEvents says an atomic save: the new file's ID renamed onto the path, then the old
    /// one's at the same path with no partner.
    #[test]
    fn rule_2_as_fsevents_says_an_atomic_save() {
        let mut index = Index::default();
        let id = known(&mut index, "/h/a.cells", Some(1), "x");
        let fates = index.follow(&[looked("/h/a.cells", &[2, 1], Some(("y", Some(2))))]);
        assert_eq!(fates, [Fate::Stayed { id, path: "/h/a.cells".into() }]);
    }

    #[test]
    fn rule_3_an_id_held_is_the_file_where_it_came_its_text_changed() {
        let mut index = Index::default();
        let id = known(&mut index, "/h/a.cells", Some(1), "x");
        let fates = index.follow(&[looked("/h/a.cells", &[], None), looked("/h/z.cells", &[], Some(("y", Some(1))))]);
        assert_eq!(fates, [Fate::Moved { id, from: "/h/a.cells".into(), to: "/h/z.cells".into(), by: By::Held }]);
    }

    #[test]
    fn rules_1_and_3_come_before_2_a_file_put_where_one_left_is_new() {
        let mut index = Index::default();
        let id = known(&mut index, "/h/a.cells", Some(1), "x");
        let fates = index.follow(&[looked("/h/a.cells", &[], Some(("z", Some(2)))), looked("/h/b.cells", &[], Some(("x", Some(1))))]);
        assert_eq!(
            fates,
            [Fate::Moved { id, from: "/h/a.cells".into(), to: "/h/b.cells".into(), by: By::Held }, Fate::New { path: "/h/a.cells".into() }]
        );
    }

    #[test]
    fn rule_4_a_file_come_with_a_gone_file_s_text_is_it() {
        let mut index = Index::default();
        let id = known(&mut index, "/h/a.cells", Some(1), "x");
        let fates = index.follow(&[looked("/h/a.cells", &[], None), looked("/h/b.cells", &[], Some(("x", Some(5))))]);
        assert_eq!(fates, [Fate::Moved { id: id.clone(), from: "/h/a.cells".into(), to: "/h/b.cells".into(), by: By::Text }]);
        // two come of its text, one of its name: that one
        let fates = index.follow(&[
            looked("/h/a.cells", &[], None),
            looked("/h/b.cells", &[], Some(("x", None))),
            looked("/h/c/a.cells", &[], Some(("x", None))),
        ]);
        assert!(fates.contains(&Fate::Moved { id, from: "/h/a.cells".into(), to: "/h/c/a.cells".into(), by: By::Text }));
        assert!(fates.contains(&Fate::New { path: "/h/b.cells".into() }));
    }

    #[test]
    fn rule_5_otherwise_new_and_gone_and_the_gone_one_found_later() {
        let mut index = Index::default();
        let id = known(&mut index, "/h/a.cells", Some(1), "x");
        let fates = index.follow(&[looked("/h/a.cells", &[], None), looked("/h/b.cells", &[], Some(("y", Some(2))))]);
        assert_eq!(fates, [Fate::Gone { id: id.clone(), path: "/h/a.cells".into() }, Fate::New { path: "/h/b.cells".into() }]);
        index.gone(&id, 0);
        // later, anywhere: a file of the text it last held
        let fates = index.follow(&[looked("/h/far/away.cells", &[], Some(("x", Some(3))))]);
        assert_eq!(fates, [Fate::Moved { id, from: "/h/a.cells".into(), to: "/h/far/away.cells".into(), by: By::Text }]);
        // two of its text and neither of its name: never guessed
        let fates = index.follow(&[looked("/h/p.cells", &[], Some(("x", None))), looked("/h/q.cells", &[], Some(("x", None)))]);
        assert_eq!(fates, [Fate::New { path: "/h/p.cells".into() }, Fate::New { path: "/h/q.cells".into() }]);
    }

    #[test]
    fn what_the_walk_and_the_watch_take_in() {
        let home = Path::new("/h");
        assert!(visible(home, Path::new("/h/Music/a.cells")));
        assert!(!visible(home, Path::new("/h/.Trash/a.cells")));
        assert!(!visible(home, Path::new("/h/w/.git/a.cells")));
        assert!(!visible(home, Path::new("/h/w/.a.cells.tmp")));
        assert!(!visible(home, Path::new("/h/Library/Preferences/a.cells")));
        assert!(visible(home, Path::new("/h/Library/Mobile Documents/com~apple~CloudDocs/a.cells")));
        assert!(visible(home, Path::new("/books/a.cells")));
    }
}
