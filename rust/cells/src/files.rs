//! Imports followed through files a host holds by path: paths joined as a filesystem
//! joins them, a pinned file's bytes checked against its hash, a file that imports itself
//! refused. What a page or a test hands the reader; a command line reads the disk the same
//! way (`cells-cli`).

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::doc::{Doc, Import, Imports, read_with};

/// Whether an import names a file by its URL.
pub fn is_url(from: &str) -> bool {
    from.starts_with("https://") || from.starts_with("http://")
}

/// The folder a path is in: everything before its last `/`, or nothing.
pub fn dir(path: &str) -> &str {
    path.rfind('/').map_or("", |at| &path[..at])
}

/// `path` read from the folder `dir`, its `.` and `..` resolved: `join("books/home",
/// "../accounts.cell")` is `books/accounts.cell`. A path from `/` keeps its root.
pub fn join(dir: &str, path: &str) -> String {
    let rooted = path.starts_with('/') || (dir.starts_with('/') && !path.starts_with('/'));
    let mut parts: Vec<&str> = Vec::new();
    let joined = if path.starts_with('/') { [path, ""] } else { [dir, path] };
    for piece in joined.iter().flat_map(|p| p.split('/')) {
        match piece {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| *p != "..") {
                    parts.pop();
                } else if !rooted {
                    parts.push("..");
                }
            }
            p => parts.push(p),
        }
    }
    let body = parts.join("/");
    if rooted { format!("/{body}") } else { body }
}

/// The imports of the file at `at`, followed through `files`: a path read from the
/// importing file's folder, a URL by itself. A file `files` does not hold, or a revision
/// (`with { rev }`, which only a host with the file's history reads), is unreachable.
pub struct Files<'a> {
    files: &'a BTreeMap<String, String>,
    at: String,
    /// The files importing this one, so a cycle is found.
    stack: Vec<String>,
    /// A cycle found under this file: every import on the way to it is refused, since a
    /// file read inside one opens with its own break and would hide it.
    cycle: Option<String>,
}

impl<'a> Files<'a> {
    pub fn new(files: &'a BTreeMap<String, String>, at: &str) -> Files<'a> {
        Files { files, at: at.to_owned(), stack: Vec::new(), cycle: None }
    }
}

/// Bytes against a pin: nothing when they match, why not when they do not.
pub fn pinned(bytes: &[u8], with: &[(String, String)]) -> Result<(), String> {
    let Some((_, pin)) = with.iter().find(|(k, _)| k == "sha256") else { return Ok(()) };
    let now = crate::sha256::hex(bytes);
    if &now == pin { Ok(()) } else { Err(format!("changed: pinned sha256 {pin}, now {now} (cells pin moves it)")) }
}

impl Imports for Files<'_> {
    fn import(&mut self, i: &Import) -> Result<Option<Doc>, String> {
        if i.with.iter().any(|(k, _)| k == "rev") {
            return Ok(None);
        }
        let key = if is_url(&i.from) { i.from.clone() } else { join(dir(&self.at), &i.from) };
        let Some(text) = self.files.get(&key) else { return Ok(None) };
        pinned(text.as_bytes(), &i.with)?;
        let mut chain = self.stack.clone();
        chain.push(self.at.clone());
        if let Some(from) = chain.iter().position(|f| *f == key) {
            let through = &chain[from + 1..];
            let why = match through.is_empty() {
                true => format!("{key} imports itself"),
                false => format!("{key} imports itself, through {}", through.join(", ")),
            };
            self.cycle = Some(why.clone());
            return Err(why);
        }
        let mut inner = Files { files: self.files, at: key, stack: chain, cycle: None };
        let doc = read_with(text.as_bytes().to_vec(), &mut inner, 1).map_err(|e| format!("{e}"))?;
        if let Some(why) = inner.cycle {
            self.cycle = Some(why.clone());
            return Err(why);
        }
        Ok(Some(doc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        assert_eq!(join("books/home", "../accounts.cell"), "books/accounts.cell");
        assert_eq!(join("books", "./home/money.cell"), "books/home/money.cell");
        assert_eq!(join("", "../x.cell"), "../x.cell");
        assert_eq!(join("/a/b", "../c"), "/a/c");
        assert_eq!(join("a", "/etc/x"), "/etc/x");
        assert_eq!(dir("books/home/money.cell"), "books/home");
    }

    #[test]
    fn follows_and_refuses() {
        let mut files = BTreeMap::new();
        files.insert("books/accounts.cell".to_owned(), "account:text!\nrent\nbank\n".to_owned());
        files.insert(
            "books/home/money.cell".to_owned(),
            "---\nimport accounts from \"../accounts.cell\"\n---\namount:EUR,to:accounts.account\n950,rent\n9,rnet\n".to_owned(),
        );
        let money = files["books/home/money.cell"].clone();
        let doc = read_with(money.clone(), &mut Files::new(&files, "books/home/money.cell"), 1).unwrap();
        assert_eq!(doc.problems().len(), 1);
        assert_eq!(doc.describe(&doc.problems()[0]), "row 1, to: \"rnet\" is no key of accounts");
        // Read alone, the reference is unchecked.
        assert!(crate::read(money).unwrap().problems().is_empty());
        files.insert("loop.cell".to_owned(), "---\nimport me from \"./loop.cell\"\n---\na\n".to_owned());
        let looped = read_with(files["loop.cell"].clone(), &mut Files::new(&files, "loop.cell"), 1).unwrap();
        assert_eq!(looped.broken().iter().map(|e| e.at).collect::<Vec<_>>(), [crate::At::PreludeLine(1)]);
    }
}
