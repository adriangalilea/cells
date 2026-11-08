//! `cells history <file|folder> [<row> <column>]`: the engine's history in a terminal
//! (ENGINE.md, History). A folder's or a file's chains, newest first, each its root on one
//! line (what was done, by whom, when) and what it reached drawn under it as a tree; or a
//! cell's every value, each beside the root of the change that wrote it. The engine is
//! asked, as every surface asks it, so this draws what Cells.app draws.

use std::fs;
use std::path::Path;

use cells_daemon::Engine;
use cells_daemon::history::{Chain, Event};
use cells_daemon::log::{Changed, Edit, Logged, Outcome};
use cells_daemon::trace::when;

/// The chains a terminal shows at once.
const SHOWN: usize = 30;

/// What `cells history` prints, a line each.
pub fn history(path: &Path, cell: Option<(usize, &str)>) -> Result<Vec<String>, String> {
    let path = fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (folder, file) = if path.is_dir() {
        (path.clone(), None)
    } else {
        (path.parent().expect("a file is in a folder").to_path_buf(), Some(path.clone()))
    };
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let (engine, _) = Engine::connect_or_start("cells history", &exe)?;
    let served = engine.serve(&folder)?;
    match (cell, &file) {
        (Some((row, column)), Some(file)) => {
            let values = engine.cell(file, row, column)?;
            let chains = engine.history(&served, Some(file), usize::MAX)?;
            let mut out = vec![format!("{}, row {row}, {column}", shown(&served, file))];
            if values.is_empty() {
                out.push("  no value logged: it holds what it held when the engine first read it".into());
            }
            for v in values {
                let root = chains.iter().find(|c| c.root.folder == v.root.0 && c.root.line.n == v.root.1);
                let why = root.map_or(String::new(), |c| format!("   {}", headline(&served, &c.root)));
                out.push(format!("  {}  {} → {}{why}", when(v.at), v.was, v.value));
            }
            Ok(out)
        }
        (Some(_), None) => Err("a cell's history names its file".into()),
        (None, _) => {
            let chains = engine.history(&served, file.as_deref(), SHOWN)?;
            if chains.is_empty() {
                return Ok(vec![format!("{}: nothing has changed since the engine first read it", path.display())]);
            }
            Ok(chains.iter().flat_map(|c| drawn(&served, c)).collect())
        }
    }
}

fn shown(folder: &Path, file: &Path) -> String {
    file.strip_prefix(folder).unwrap_or(file).display().to_string()
}

/// A root as one line: the file, what was done to it and by whom.
fn headline(folder: &Path, root: &Event) -> String {
    let file = shown(folder, &root.file());
    match &root.line.entry {
        Logged::External { was: None, by, .. } => format!("{file} added{}", by_whom(by.as_deref())),
        Logged::External { now: None, .. } => format!("{file} removed"),
        Logged::External { by, .. } => format!("{file} saved{}", by_whom(by.as_deref())),
        Logged::Restore { now, by, .. } => format!("{file} returned to {} by {by}", &now[..12]),
        Logged::Run { script, run, outcome: Outcome::Ok { added, changed }, by, .. } => {
            format!("{file}: {script} run {run} by {by}, {added} added, {changed} changed")
        }
        Logged::Run { script, run, outcome: Outcome::Failed { why }, by, .. } => {
            format!("{file}: {script} run {run} by {by} failed: {why}")
        }
        Logged::Store { .. } | Logged::Moved { .. } => unreachable!("a root is an edit, a restore or a run"),
    }
}

fn by_whom(by: Option<&str>) -> String {
    by.map_or(" outside Cells".into(), |b| format!(" by {b}"))
}

/// A cell changed, as a person names it: by its row's key when the table has one.
fn cell(c: &Changed) -> String {
    let row = c.key.clone().unwrap_or_else(|| format!("row {}", c.row));
    format!("{row} {} {} → {}", c.column, c.was, c.now)
}

fn edit(e: &Edit) -> Vec<String> {
    let mut out: Vec<String> = e.cells.iter().map(cell).collect();
    let rows = |rows: &[usize], verb: &str| {
        (!rows.is_empty()).then(|| format!("{verb} row{} {}", if rows.len() == 1 { "" } else { "s" }, list(rows)))
    };
    out.extend(rows(&e.rows_added, "added"));
    out.extend(rows(&e.rows_removed, "removed"));
    out.extend((!e.columns_added.is_empty()).then(|| format!("added column {}", e.columns_added.join(", "))));
    out.extend((!e.columns_removed.is_empty()).then(|| format!("removed column {}", e.columns_removed.join(", "))));
    out
}

fn list(rows: &[usize]) -> String {
    rows.iter().map(usize::to_string).collect::<Vec<_>>().join(", ")
}

/// A chain: its root's line, the cells the root edited under it, and each entry that
/// followed under the one it followed.
fn drawn(folder: &Path, chain: &Chain) -> Vec<String> {
    let pad = " ".repeat(18);
    let mut out = vec![format!("{}  {}", when(chain.root.line.at), headline(folder, &chain.root))];
    if let Logged::External { edited: Some(e), .. } | Logged::Restore { edited: Some(e), .. } | Logged::Run { edited: Some(e), .. } =
        &chain.root.line.entry
    {
        out.extend(edit(e).into_iter().map(|l| format!("{pad}{l}")));
    }
    // each entry's children, in the order they followed
    let children =
        |under: Option<usize>| -> Vec<usize> { (0..chain.followed.len()).filter(|&i| chain.followed[i].under == under).collect() };
    let mut stack: Vec<(usize, String, bool)> =
        children(None).into_iter().rev().enumerate().map(|(k, i)| (i, String::new(), k == 0)).collect();
    while let Some((i, indent, last)) = stack.pop() {
        let e = &chain.followed[i].event;
        let file = shown(folder, &e.file());
        let what = match &e.line.entry {
            Logged::Store { stored, .. } => stored.iter().map(cell).collect::<Vec<_>>().join(", "),
            Logged::Moved { .. } => "not written: it changed since it was read".into(),
            _ => unreachable!("only a root's followers follow it"),
        };
        out.push(format!("{pad}{indent}{} {file}  {what}", if last { "└" } else { "├" }));
        let deeper = format!("{indent}{}", if last { "  " } else { "│ " });
        let kids = children(Some(i));
        stack.extend(kids.iter().rev().enumerate().map(|(k, &c)| (c, deeper.clone(), k == 0)));
    }
    out.push(String::new());
    out
}
