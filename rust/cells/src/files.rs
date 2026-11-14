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
/// "../accounts.cells")` is `books/accounts.cells`. A path from `/` keeps its root.
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
    /// Every file this file's imports named, by its key in `files`, imports of imports
    /// included, held or not: what a change to would change this read, and what a host
    /// that holds less than the reader asked for has to read.
    pub reached: Vec<String>,
}

impl<'a> Files<'a> {
    pub fn new(files: &'a BTreeMap<String, String>, at: &str) -> Files<'a> {
        Files { files, at: at.to_owned(), stack: Vec::new(), cycle: None, reached: Vec::new() }
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
        self.reached.push(key.clone());
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
        let mut inner = Files { files: self.files, at: key, stack: chain, cycle: None, reached: Vec::new() };
        let doc = read_with(text.as_bytes().to_vec(), &mut inner, 1);
        self.reached.append(&mut inner.reached);
        let doc = doc.map_err(|e| format!("{e}"))?;
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
        assert_eq!(join("books/home", "../accounts.cells"), "books/accounts.cells");
        assert_eq!(join("books", "./home/money.cells"), "books/home/money.cells");
        assert_eq!(join("", "../x.cells"), "../x.cells");
        assert_eq!(join("/a/b", "../c"), "/a/c");
        assert_eq!(join("a", "/etc/x"), "/etc/x");
        assert_eq!(dir("books/home/money.cells"), "books/home");
    }

    #[test]
    fn follows_and_refuses() {
        let mut files = BTreeMap::new();
        files.insert("books/accounts.cells".to_owned(), "account:text!\nrent\nbank\n".to_owned());
        files.insert(
            "books/home/money.cells".to_owned(),
            "---\nimport accounts from \"../accounts.cells\"\n---\namount:EUR,to:accounts.account\n950,rent\n9,rnet\n".to_owned(),
        );
        let money = files["books/home/money.cells"].clone();
        let doc = read_with(money.clone(), &mut Files::new(&files, "books/home/money.cells"), 1).unwrap();
        assert_eq!(doc.problems().len(), 1);
        assert_eq!(doc.describe(&doc.problems()[0]), "row 2, to: \"rnet\" is no key of accounts");
        // Read alone, the reference is unchecked.
        assert!(crate::read(money).unwrap().problems().is_empty());
        // A formula reading the import computes where it is reached; alone, what the file
        // stores stands, never null and never stale, and a write leaves it as stored.
        files.insert("books/accounts.cells".to_owned(), "account:text!,limit:EUR\nrent,1000\n".to_owned());
        let spent = "---\nimport accounts from \"../accounts.cells\"\n---\nto:accounts.account,amount:EUR,left:EUR=accounts[to].limit - amount\nrent,950,40.00=\n";
        let reached = read_with(spent, &mut Files::new(&files, "books/home/money.cells"), 1).unwrap();
        assert_eq!(reached.value(0, 2), crate::Value::Decimal(crate::Decimal { mantissa: 5000, scale: 2 }));
        assert_eq!(reached.problems()[0].kind(), crate::Kind::Stale);
        let mut alone = crate::read(spent).unwrap();
        assert_eq!(alone.value(0, 2), crate::Value::Decimal(crate::Decimal { mantissa: 4000, scale: 2 }));
        let p = alone.problems()[0];
        assert_eq!((p.kind(), p.kind().severity()), (crate::Kind::Unreached, crate::Severity::Note));
        assert_eq!(alone.why(&p), "from ../accounts.cells, not reachable: the value the file stores stands");
        assert_eq!(alone.shown(), "to\tamount\tleft\nrent\t950\t40.00\n");
        alone.set_cell(0, 1, "900").unwrap();
        alone.store_unstored();
        assert_eq!(alone.write(), spent.replace("950", "900"));
        files.insert("loop.cells".to_owned(), "---\nimport me from \"./loop.cells\"\n---\na\n".to_owned());
        let looped = read_with(files["loop.cells"].clone(), &mut Files::new(&files, "loop.cells"), 1).unwrap();
        assert_eq!(looped.broken().iter().map(|e| e.at).collect::<Vec<_>>(), [crate::At::PreludeLine(1)]);
    }

    fn books() -> BTreeMap<String, String> {
        let mut files = BTreeMap::new();
        files.insert(
            "books/accounts.cells".to_owned(),
            "---\ntype Kind = assets|expenses\n---\naccount:text!,kind:Kind,limit:EUR,half:EUR=limit / 2\nrent,expenses,1000,500.00=\nbank,assets,0,0.00=\n".to_owned(),
        );
        files.insert(
            "books/rates.cells".to_owned(),
            "---\nexport type Money = EUR>=0\nlook Money: grouped\nexport type Kind = income#green|cost#red\nexport fees = ```cells\nkind:Kind!,pct:pct\ncost,1.5\n```\n---\na\n".to_owned(),
        );
        files
    }

    const SPENT: &str = "---\nimport accounts from \"../accounts.cells\"\nimport { Money, Kind, fees } from \"../rates.cells\"\n---\nto:accounts.account,kind:Kind,amount:Money,left:EUR=accounts[to].limit - amount,fee:pct=fees[kind].pct\nrent,cost,950,50.00=,1.5=\n";

    fn at<'a>(files: &'a BTreeMap<String, String>) -> Files<'a> {
        Files::new(files, "books/home/money.cells")
    }

    #[test]
    fn packed_copies_stand_in_for_what_is_not_reached() {
        use crate::{Pack, Packing};
        let files = books();
        let doc = read_with(SPENT, &mut at(&files), 1).unwrap();
        assert!(doc.copies().is_empty() && doc.problems().is_empty(), "{:?}", doc.problems());
        let packed = doc.packed(Pack::All);
        assert_eq!(
            packed,
            "---\nimport accounts from \"../accounts.cells\"\npacked accounts = ```cells\naccount: text!,kind: assets|expenses,limit: EUR,half: EUR\nrent,expenses,1000,500.00\nbank,assets,0,0.00\n```\nimport { Money, Kind, fees } from \"../rates.cells\"\npacked type Money = EUR>=0\npacked look Money: grouped\npacked type Kind = income#green|cost#red\npacked fees = ```cells\nkind: income#green|cost#red!,pct: pct\ncost,1.5\n```\n---\nto:accounts.account,kind:Kind,amount:Money,left:EUR=accounts[to].limit - amount,fee:pct=fees[kind].pct\nrent,cost,950,50.00=,1.5=\n"
        );
        // reached, every copy is what packing gives, and packing again changes nothing
        let again = read_with(packed.as_str(), &mut at(&files), 1).unwrap();
        assert!(again.broken().is_empty(), "{:?}", again.broken());
        assert!(again.copies().iter().all(|c| c.packing == Packing::Current), "{:?}", again.copies());
        assert_eq!(again.copies().len(), 5);
        assert_eq!(again.packed(Pack::All), packed);
        assert_eq!(again.packed(Pack::Remove), SPENT);
        // sent alone, the copies are read in place of their sources: types, references,
        // formulas, looks, and nothing unreached
        let alone = crate::read(packed.as_str()).unwrap();
        assert!(alone.broken().is_empty(), "{:?}", alone.broken());
        assert!(alone.problems().is_empty(), "{:?}", alone.problems());
        assert!(alone.copies().iter().all(|c| c.packing == Packing::Standing));
        assert_eq!(alone.columns()[2].ty.shorthand(), "Money");
        assert_eq!(alone.looks_of(2).len(), 1);
        assert_eq!(alone.tables()[0].held, crate::Held::Copy { from: "../accounts.cells".into(), line: 2 });
        let typo = packed.replace("\nrent,cost,950", "\nrnet,cost,950");
        assert_eq!(crate::read(typo).unwrap().problems()[0].kind(), crate::Kind::Reference);
        // alone, packing keeps every copy as it is: there is nothing to refresh it from
        assert_eq!(alone.packed(Pack::All), packed);
        // a source that changed leaves its copy stale, never read
        let mut moved = files.clone();
        moved.insert("books/accounts.cells".to_owned(), files["books/accounts.cells"].replace("1000", "1200"));
        let stale = read_with(packed.as_str(), &mut at(&moved), 1).unwrap();
        assert_eq!(stale.copies()[0].packing, Packing::Stale);
        assert_eq!(stale.problems()[0].kind(), crate::Kind::Stale, "the source is the truth: 1200 - 950");
        assert_eq!(stale.packed(Pack::All), packed.replace("rent,expenses,1000,500.00", "rent,expenses,1200,600.00"));
    }

    #[test]
    fn copies_never_nest_so_a_chain_packs_linearly() {
        use crate::Pack;
        // f0 a table; each f(k) imports f(k-1) and computes from it, and is packed in turn
        let mut files = BTreeMap::new();
        files.insert("f0.cells".to_owned(), "key:text!,v:int\nk,1\n".to_owned());
        for k in 1..=10 {
            let text = format!("---\nimport f from \"./f{}.cells\"\n---\nkey:text!,v:int=f[key].v + 1\nk,{}=\n", k - 1, k + 1);
            let path = format!("f{k}.cells");
            let packed = read_with(text.as_str(), &mut Files::new(&files, &path), 1).unwrap().packed(Pack::All);
            files.insert(path, packed);
        }
        for k in 1..=10 {
            let text = &files[&format!("f{k}.cells")];
            assert_eq!(text.matches("\npacked ").count(), 1, "f{k} holds one copy, its import's table alone: {text}");
            assert!(text.len() < 200, "f{k} is {} bytes: copies grew with the chain", text.len());
        }
        // f10 sent alone reads from its one copy
        let alone = crate::read(files["f10.cells"].as_str()).unwrap();
        assert!(alone.problems().is_empty() && alone.broken().is_empty(), "{:?}", alone.problems());
    }

    #[test]
    fn a_copy_is_data_never_an_import_site() {
        use crate::Pack;
        // a copy's rows that read as an import, a fence or a prelude are rows: nothing is
        // followed from inside it, so it cannot bring an import, or a cycle, back
        let mut files = BTreeMap::new();
        files.insert("c.cells".to_owned(), "note:text\nx\n".to_owned());
        let copy = "---\nimport b from \"./b.cells\"\npacked b = ```cells\nnote: text\n\"import c from \"\"./c.cells\"\"\"\n---\n```\n---\nnote:text\ny\n";
        let mut at = Files::new(&files, "c.cells");
        let doc = read_with(copy, &mut at, 1).unwrap();
        assert_eq!(at.reached, ["b.cells"], "the import line followed, the copy's rows never");
        assert!(at.cycle.is_none() && doc.broken().is_empty(), "{:?}", doc.broken());
        let rows = doc.tables()[0].doc.as_ref().unwrap();
        assert_eq!(rows.rows(), 2, "the copy's lines are its rows");
        // a cycle refuses the import, and packing writes no copy of what it refuses
        let mut files = BTreeMap::new();
        files.insert("a.cells".to_owned(), "---\nimport b from \"./b.cells\"\n---\nkey:text!\nx\n".to_owned());
        files.insert("b.cells".to_owned(), "---\nimport a from \"./a.cells\"\n---\nkey:text!\ny\n".to_owned());
        let a = read_with(files["a.cells"].as_str(), &mut Files::new(&files, "a.cells"), 1).unwrap();
        assert!(!a.broken().is_empty());
        assert_eq!(a.packed(Pack::All), files["a.cells"]);
    }

    #[test]
    fn a_url_is_packed_from_its_pinned_bytes_alone() {
        use crate::{Pack, Packing};
        let url = "https://x.example/rates.cells";
        let bytes = "key:text!,v:int\nk,1\n";
        let pin = crate::sha256::hex(bytes.as_bytes());
        let text = format!(
            "---\nimport rates from \"{url}\" with {{ sha256: \"{pin}\", pack: \"auto\" }}\n---\nkey:text!,v:int=rates[key].v\nk,1=\n"
        );
        let mut files = BTreeMap::new();
        files.insert(url.to_owned(), bytes.to_owned());
        let packed = read_with(text.as_str(), &mut Files::new(&files, "m.cells"), 1).unwrap().packed(Pack::Auto);
        assert!(packed.contains("packed rates = ```cells\nkey: text!,v: int\nk,1\n```"));
        // bytes that are not the pinned ones are refused: the copy stands and nothing is written
        files.insert(url.to_owned(), "key:text!,v:int\nk,2\n".to_owned());
        let moved = read_with(packed.as_str(), &mut Files::new(&files, "m.cells"), 1).unwrap();
        assert_eq!(moved.copies()[0].packing, Packing::Standing);
        assert_eq!(moved.packed(Pack::Auto), packed);
    }

    #[test]
    fn a_copy_is_of_an_import_and_auto_is_asked() {
        use crate::Pack;
        let files = books();
        let orphan = "---\npacked accounts = ```cells\naccount: text!\nrent\n```\n---\nto:text\nrent\n";
        let doc = crate::read(orphan).unwrap();
        assert_eq!(doc.broken().iter().map(|e| e.at).collect::<Vec<_>>(), [crate::At::PreludeLine(1)]);
        assert!(doc.broken()[0].why.contains("no import above it names accounts"), "{}", doc.broken()[0].why);
        assert!(crate::read("---\nexport packed type A = text\n---\na\n").unwrap().broken().len() == 1);
        assert!(crate::read("---\nimport x from \"./x.cells\" with { pack: \"always\" }\n---\na\n").unwrap().broken().len() == 1);
        // auto packs only the imports that ask, and always while their source is reached
        let auto = SPENT.replace("\"../accounts.cells\"", "\"../accounts.cells\" with { pack: \"auto\" }");
        let doc = read_with(auto.as_str(), &mut at(&files), 1).unwrap();
        let packed = doc.packed(Pack::Auto);
        assert!(packed.contains("packed accounts = ```cells"));
        assert!(!packed.contains("packed type Money"));
    }
}
