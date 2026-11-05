//! Imports followed from the disk: a path from the importing file's folder, `with { rev }`
//! through git, a URL from the cache of what was fetched by its hash, else fetched.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use cells::files::{is_url, pinned};
use cells::{Doc, Import, Imports};

/// Where fetched files are kept, each by its hash: `$XDG_CACHE_HOME/cells`, else
/// `~/.cache/cells`.
pub fn cache() -> PathBuf {
    match env::var_os("XDG_CACHE_HOME") {
        Some(c) => PathBuf::from(c).join("cells"),
        None => PathBuf::from(env::var_os("HOME").expect("HOME is set")).join(".cache/cells"),
    }
}

/// A URL's bytes, by `curl`; nothing when it cannot be fetched.
pub fn fetch(url: &str) -> Option<Vec<u8>> {
    let out = Command::new("curl").args(["-fsSL", "--max-time", "30", url]).output().ok()?;
    out.status.success().then_some(out.stdout)
}

/// The imports of the file at `at`.
pub struct Disk {
    at: PathBuf,
    /// The files importing this one, so a cycle is found.
    stack: Vec<PathBuf>,
    /// A cycle found under this file: every import on the way to it is refused, since a
    /// file read inside one opens with its own break and would hide it.
    cycle: Option<String>,
}

impl Disk {
    pub fn new(at: &Path) -> Disk {
        Disk { at: at.to_path_buf(), stack: Vec::new(), cycle: None }
    }

    fn folder(&self) -> &Path {
        self.at.parent().unwrap_or(Path::new("."))
    }

    /// A file's bytes as git holds them at `rev`, from the repository the file is in.
    fn at_rev(&self, path: &Path, rev: &str) -> Result<Option<Vec<u8>>, String> {
        let dir = path.parent().unwrap_or(Path::new("."));
        let name = path.file_name().ok_or("names no file")?.to_string_lossy().into_owned();
        let out = Command::new("git").arg("-C").arg(dir).args(["show", &format!("{rev}:./{name}")]).output();
        match out {
            Ok(o) if o.status.success() => Ok(Some(o.stdout)),
            Ok(o) => Err(format!("git has no {name} at {rev}: {}", String::from_utf8_lossy(&o.stderr).trim())),
            Err(_) => Ok(None),
        }
    }
}

impl Imports for Disk {
    fn import(&mut self, i: &Import) -> Result<Option<Doc>, String> {
        let rev = i.with.iter().find(|(k, _)| k == "rev").map(|(_, v)| v.as_str());
        let (bytes, path) = if is_url(&i.from) {
            let pin = i.with.iter().find(|(k, _)| k == "sha256").map(|(_, v)| v.clone()).ok_or("is pinned by its hash")?;
            let kept = cache().join(&pin);
            let bytes = match fs::read(&kept) {
                Ok(b) => b,
                Err(_) => {
                    let Some(b) = fetch(&i.from) else { return Ok(None) };
                    pinned(&b, &i.with)?;
                    fs::create_dir_all(cache()).map_err(|e| e.to_string())?;
                    fs::write(&kept, &b).map_err(|e| e.to_string())?;
                    b
                }
            };
            (bytes, self.at.clone())
        } else {
            let path = self.folder().join(&i.from);
            let path = fs::canonicalize(&path).unwrap_or(path);
            let mut chain = self.stack.clone();
            chain.push(fs::canonicalize(&self.at).unwrap_or(self.at.clone()));
            if let Some(from) = chain.iter().position(|f| *f == path) {
                let through: Vec<String> = chain[from + 1..].iter().map(|p| p.display().to_string()).collect();
                let why = match through.is_empty() {
                    true => format!("{} imports itself", path.display()),
                    false => format!("{} imports itself, through {}", path.display(), through.join(", ")),
                };
                self.cycle = Some(why.clone());
                return Err(why);
            }
            let bytes = match rev {
                Some(r) => match self.at_rev(&path, r)? {
                    Some(b) => b,
                    None => return Ok(None),
                },
                None => match fs::read(&path) {
                    Ok(b) => b,
                    Err(_) => return Ok(None),
                },
            };
            (bytes, path)
        };
        pinned(&bytes, &i.with)?;
        let mut stack = self.stack.clone();
        stack.push(fs::canonicalize(&self.at).unwrap_or(self.at.clone()));
        let mut inner = Disk { at: path, stack, cycle: None };
        let doc = cells::read_with(bytes, &mut inner, 1).map_err(|e| e.to_string())?;
        if let Some(why) = inner.cycle {
            self.cycle = Some(why.clone());
            return Err(why);
        }
        Ok(Some(doc))
    }
}
