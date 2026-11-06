//! `cells`, the command line.
//!
//!   cells check <file|folder>...       every break and problem, `path column 3: to: why`,
//!                                     `path row N, column: why`, its imports followed, its
//!                                     own tables checked too
//!   cells fmt --check <file|folder>... every file written back is the same bytes
//!   cells fix <file|folder>...         every stale computed cell given its formula's value,
//!                                     every one not stored yet stored, each file written
//!   cells merge <base> <ours> <theirs> git's merge driver: what was written merged, what is
//!                                     computed computed again (SPEC.md, Merging)
//!   cells pin <file>...                every import from a URL pinned by its hash now
//!   cells run <file> [script]          its scripts run once, their rows written under them
//!   cells show <file>                  its table as tab-separated text, computed cells
//!                                     their values
//!   cells lsp                          the language server, on stdin and stdout
//!
//! A folder means every `.cells` file under it, hidden folders (`.git`, the engine's `.cells`)
//! left out. Exit 0 when nothing is found (or every run succeeds), 1 when something is (or
//! a run fails), 2 when the command cannot run (a path that does not exist, a file that
//! cannot be read, a wrong argument).

mod disk;
mod json;
mod lsp;
mod merge;
mod pin;
mod run;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

use disk::Disk;

const USAGE: &str = "cells check <file|folder>...\ncells fmt --check <file|folder>...\ncells fix <file|folder>...\ncells merge <base> <ours> <theirs>\ncells pin <file>...\ncells run <file> [script]\ncells show <file>\ncells lsp";

/// Every `.cells` file a path names, sorted.
fn files(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let meta = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.is_file() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    entries.sort();
    for p in entries {
        let hidden = p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
        if p.is_dir() {
            if !hidden {
                files(&p, out)?;
            }
        } else if p.extension().is_some_and(|e| e == "cells") {
            out.push(p);
        }
    }
    Ok(())
}

/// What one file says: its lines of output, or the reason it could not be read.
enum Said {
    Clean,
    Found(Vec<String>),
}

fn check(path: &Path, threads: usize) -> Result<Said, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let shown = path.display();
    let doc = match cells::read_with(bytes, &mut Disk::new(path), threads) {
        Ok(doc) => doc,
        Err(e) => return Ok(Said::Found(vec![format!("{shown} {e}")])),
    };
    // Where its grammar breaks first, then its own tables, in the prelude's order, then its
    // rows.
    let mut lines: Vec<String> = doc.broken().iter().map(|e| format!("{shown} {e}")).collect();
    for t in doc.tables().iter().filter(|t| t.own) {
        let d = t.doc.as_ref().expect("a table of the file's own is read");
        lines.extend(d.problems().iter().map(|p| format!("{shown} {} {}", t.name, d.describe(p))));
    }
    lines.extend(doc.problems().iter().map(|p| format!("{shown} {}", doc.describe(p))));
    if lines.is_empty() { Ok(Said::Clean) } else { Ok(Said::Found(lines)) }
}

/// The file written with every stale cell given its formula's value and every computed
/// cell not stored yet stored: what it says is how many cells it stored.
fn fix(path: &Path, threads: usize) -> Result<Said, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let shown = path.display();
    let mut doc = cells::read_with(bytes.clone(), &mut Disk::new(path), threads).map_err(|e| format!("{shown} {e}"))?;
    let stale = doc.problems().iter().filter(|p| p.kind() == cells::Kind::Stale).count();
    doc.fix_stale();
    doc.store_unstored();
    let text = doc.write();
    if text.as_bytes() == bytes {
        return Ok(Said::Clean);
    }
    fs::write(path, text).map_err(|e| format!("{shown}: {e}"))?;
    Ok(Said::Found(vec![format!("{shown}: {stale} stale cells given their formula's value, every computed value stored")]))
}

fn round_trip(path: &Path, threads: usize) -> Result<Said, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let shown = path.display();
    match cells::read_with(bytes.clone(), &mut Disk::new(path), threads) {
        Err(e) => Ok(Said::Found(vec![format!("{shown} {e}")])),
        Ok(doc) if doc.write().as_bytes() != bytes => Ok(Said::Found(vec![format!("{shown}: written back, it is not the same bytes")])),
        Ok(_) => Ok(Said::Clean),
    }
}

/// Runs `each` over the files: many files share the threads one each, a few files take
/// them all inside each file. Prints in path order.
fn run(paths: &[String], each: fn(&Path, usize) -> Result<Said, String>) -> ExitCode {
    let mut all = Vec::new();
    for p in paths {
        if let Err(e) = files(Path::new(p), &mut all) {
            eprintln!("cells: {e}");
            return ExitCode::from(2);
        }
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let results: Vec<Result<Said, String>> = if all.len() >= threads {
        let next = AtomicUsize::new(0);
        let mut slots: Vec<Option<Result<Said, String>>> = (0..all.len()).map(|_| None).collect();
        let done: Vec<Vec<(usize, Result<Said, String>)>> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..threads)
                .map(|_| {
                    s.spawn(|| {
                        let mut mine = Vec::new();
                        loop {
                            let k = next.fetch_add(1, Ordering::Relaxed);
                            if k >= all.len() {
                                return mine;
                            }
                            mine.push((k, each(&all[k], 1)));
                        }
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for (k, r) in done.into_iter().flatten() {
            slots[k] = Some(r);
        }
        slots.into_iter().map(Option::unwrap).collect()
    } else {
        all.iter().map(|p| each(p, threads)).collect()
    };
    let mut found = false;
    for r in results {
        match r {
            Err(e) => {
                eprintln!("cells: {e}");
                return ExitCode::from(2);
            }
            Ok(Said::Clean) => {}
            Ok(Said::Found(lines)) => {
                found = true;
                for l in lines {
                    println!("{l}");
                }
            }
        }
    }
    if found { ExitCode::from(1) } else { ExitCode::SUCCESS }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") if args.len() > 1 => run(&args[1..], check),
        Some("lsp") if args.len() == 1 => {
            lsp::serve();
            ExitCode::SUCCESS
        }
        Some("fmt") if args.get(1).map(String::as_str) == Some("--check") && args.len() > 2 => run(&args[2..], round_trip),
        Some("fmt") => {
            eprintln!("cells: fmt only checks for now: cells fmt --check <file|folder>...");
            ExitCode::from(2)
        }
        // A fix that wrote files exits 0: it did what it was asked.
        Some("fix") if args.len() > 1 => match run(&args[1..], fix) {
            code if code == ExitCode::from(2) => code,
            _ => ExitCode::SUCCESS,
        },
        Some("merge") if args.len() == 4 => match merge::merge(Path::new(&args[1]), Path::new(&args[2]), Path::new(&args[3])) {
            Ok(merge::Merged::Clean) => ExitCode::SUCCESS,
            Ok(merge::Merged::Conflicts(n)) => {
                eprintln!("cells merge: {n} conflicts left marked in {}", args[2]);
                ExitCode::from(1)
            }
            Err(e) => {
                eprintln!("cells: {e}");
                ExitCode::from(2)
            }
        },
        Some("pin") if args.len() > 1 => {
            for p in &args[1..] {
                match pin::pin(Path::new(p)) {
                    Ok(said) => said.iter().for_each(|l| println!("{l}")),
                    Err(e) => {
                        eprintln!("cells: {e}");
                        return ExitCode::from(2);
                    }
                }
            }
            ExitCode::SUCCESS
        }
        Some("show") if args.len() == 2 => {
            let path = Path::new(&args[1]);
            let shown = fs::read(path)
                .map_err(|e| format!("{}: {e}", path.display()))
                .and_then(|b| cells::read_with(b, &mut Disk::new(path), 1).map_err(|e| format!("{} {e}", path.display())));
            match shown {
                Ok(doc) => {
                    print!("{}", doc.written_as(cells::Dialect::Plain(cells::Separator::Tab), None));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("cells: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Some("run") if (2..=3).contains(&args.len()) => match run::run(Path::new(&args[1]), args.get(2).map(String::as_str)) {
            Ok((said, ok)) => {
                said.iter().for_each(|l| println!("{l}"));
                if ok { ExitCode::SUCCESS } else { ExitCode::from(1) }
            }
            Err(e) => {
                eprintln!("cells: {e}");
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
