//! A folder of `.cells` files as the editor has it: every file under its root, a file the
//! editor holds open read as it holds it, each read with its imports followed, and where
//! the tables a file names live.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cells::files::is_url;
use cells::{Doc, Statement};
use tower_lsp::lsp_types::Url;

use cells_disk::Disk;

#[derive(Clone)]
pub struct Folder {
    /// Where the editor opened the folder; nothing for a file opened alone.
    pub root: Option<PathBuf>,
    /// The files the editor holds open, by canonical path.
    pub open: Arc<BTreeMap<PathBuf, String>>,
    /// Each directory the editor spells another way than its canonical path (`/tmp` is
    /// `/private/tmp` on a Mac), canonical to spelled: an editor drops whatever names a
    /// file by a path it did not open, diagnostics included.
    pub spelled: Arc<BTreeMap<PathBuf, PathBuf>>,
}

/// A file read: its text as the editor has it, and the doc.
pub struct File {
    pub path: PathBuf,
    pub text: String,
    pub doc: Doc,
}

/// Where a table a file names lives: a file's own rows, or a table its prelude writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Home {
    Main(PathBuf),
    Inline(PathBuf, String),
}

/// A table read where it lives: its doc, and where its text starts in its file.
pub struct Held {
    pub file: File,
    /// The inline table, when it is one; the file's doc otherwise.
    pub inline: Option<Arc<Doc>>,
    pub base: u32,
}

impl Held {
    pub fn doc(&self) -> &Doc {
        self.inline.as_deref().unwrap_or(&self.file.doc)
    }
}

/// A table held is its doc, as the engine's completions reach it.
impl std::ops::Deref for Held {
    type Target = Doc;
    fn deref(&self) -> &Doc {
        self.doc()
    }
}

/// A path as the folder keys it: canonical when it exists.
pub fn canonical(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// A directory as the editor spells it, keyed by its canonical path; nothing when the two
/// are the same.
pub fn spelling(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let c = canonical(dir);
    (c != dir).then(|| (c, dir.to_path_buf()))
}

impl Folder {
    /// The URI naming `p` (canonical) as the editor spells it, under the deepest
    /// directory it spells its own way.
    pub fn uri(&self, p: &Path) -> Url {
        let spelled = self.spelled.iter().rev().find_map(|(c, s)| p.strip_prefix(c).ok().map(|rest| s.join(rest)));
        Url::from_file_path(spelled.as_deref().unwrap_or(p)).expect("the folder's paths are absolute")
    }

    pub fn text(&self, path: &Path) -> Option<String> {
        match self.open.get(path) {
            Some(t) => Some(t.clone()),
            None => fs::read_to_string(path).ok(),
        }
    }

    /// The file at `path`, read with its imports followed; nothing when it cannot be read
    /// as text.
    pub fn read(&self, path: &Path) -> Option<File> {
        self.read_as(path, self.text(path)?)
    }

    /// `text` read as the file at `path` (its imports followed from there): the file as
    /// it was saved, beside the text the editor holds now.
    pub fn read_as(&self, path: &Path, text: String) -> Option<File> {
        let doc = cells::read_with(text.clone().into_bytes(), &mut Disk::over(path, Arc::clone(&self.open)), 1).ok()?;
        Some(File { path: path.to_path_buf(), text, doc })
    }

    /// Every `.cells` file under the root (hidden folders left out, as `cells check` leaves
    /// them), and every file the editor holds open.
    pub fn files(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = self.open.keys().cloned().collect();
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = fs::read_dir(dir) else { return };
            for e in entries.flatten() {
                let p = e.path();
                let hidden = p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
                if p.is_dir() {
                    if !hidden {
                        walk(&p, out);
                    }
                } else if p.extension().is_some_and(|e| e == "cells") {
                    out.push(canonical(&p));
                }
            }
        }
        if let Some(root) = &self.root {
            walk(root, &mut out);
        }
        out.sort();
        out.dedup();
        out
    }

    /// The table `name` read where it lives, as `file` names it: `this`, the file's own.
    pub fn held(&self, file: &File, name: &str) -> Option<Held> {
        if name == cells::expr::THIS {
            let own = File { path: file.path.clone(), text: file.text.clone(), doc: file.doc.clone() };
            return Some(Held { file: own, inline: None, base: 0 });
        }
        match home(file, name)? {
            Home::Main(path) => Some(Held { file: self.read(&path)?, inline: None, base: 0 }),
            Home::Inline(path, table) => {
                let f = match path == file.path {
                    true => File { path, text: file.text.clone(), doc: file.doc.clone() },
                    false => self.read(&path)?,
                };
                let (inline, base) = inline(&f, &table)?;
                Some(Held { file: f, inline: Some(inline), base })
            }
        }
    }
}

/// Where the table `name` that `file`'s prelude names lives.
pub fn home(file: &File, name: &str) -> Option<Home> {
    let t = file.doc.tables().iter().find(|t| t.name == name)?;
    if t.own() {
        return Some(Home::Inline(file.path.clone(), name.to_owned()));
    }
    file.doc.statements().iter().find_map(|s| match s {
        Statement::Import { line, default, names, from, .. } if *line == t.line && !is_url(from) => {
            let path = canonical(&file.path.parent().unwrap_or(Path::new(".")).join(from));
            if default.as_deref() == Some(name) {
                Some(Home::Main(path))
            } else {
                names.iter().any(|n| n == name).then(|| Home::Inline(path, name.to_owned()))
            }
        }
        _ => None,
    })
}

/// A table `file`'s prelude writes, `name`: its doc and where its text starts.
pub fn inline(file: &File, name: &str) -> Option<(Arc<Doc>, u32)> {
    let doc = file.doc.tables().iter().find(|t| t.own() && t.name == name)?.doc.clone()?;
    let base = file.doc.statements().iter().find_map(|s| match s {
        Statement::Table { name: n, body, .. } if n == name => Some(body.start),
        Statement::Run { name: n, stored: Some(body), .. } if n == name => Some(body.start),
        _ => None,
    })?;
    Some((doc, base as u32))
}

/// The table `name` of `file` is the main table of `target`.
pub fn names_main(file: &File, name: &str, target: &Path) -> bool {
    home(file, name) == Some(Home::Main(target.to_path_buf()))
}
