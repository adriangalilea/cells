//! `cells`, the command line. Every command, what it does and the exit codes are said once,
//! in `describe.rs`: the usage, `cells help` and the agent skill are read from there.

mod allow;
mod chart;
mod describe;
mod editors;
mod history;
mod lsp;
mod merge;
mod pack;
mod pin;
mod run;
mod skill;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

use cells_disk::{Disk, files};

/// What one file says: its lines of output, or the reason it could not be read. A file
/// whose only lines are notes (a stored value standing for an import not reached) says
/// them and fails nothing.
enum Said {
    Clean,
    Noted(Vec<String>),
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
    let mut fails = !lines.is_empty();
    // Its packed copies: one its source has moved past fails, one read in its source's
    // place is a note.
    for c in doc.copies() {
        let place = format!("{shown} prelude line {}: packed {}", c.line, c.name);
        match c.packing {
            cells::Packing::Current => {}
            cells::Packing::Stale => {
                fails = true;
                lines.push(format!("{place} is not what {} holds now: cells pack refreshes it", c.from));
            }
            cells::Packing::Standing => lines.push(format!("{place} is read in place of {}, not reachable", c.from)),
        }
    }
    let noted = |p: &cells::Problem| p.kind().severity() == cells::Severity::Note;
    for t in doc.tables().iter().filter(|t| t.own()) {
        let d = t.doc.as_ref().expect("a table of the file's own is read");
        fails |= d.problems().iter().any(|p| !noted(p));
        lines.extend(d.problems().iter().map(|p| format!("{shown} {} {}", t.name, d.describe(p))));
    }
    fails |= doc.problems().iter().any(|p| !noted(p));
    lines.extend(doc.problems().iter().map(|p| format!("{shown} {}", doc.describe(p))));
    Ok(match (lines.is_empty(), fails) {
        (true, _) => Said::Clean,
        (false, false) => Said::Noted(lines),
        (false, true) => Said::Found(lines),
    })
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
    let text = doc.packed(cells::Pack::Auto);
    if text.as_bytes() == bytes {
        return Ok(Said::Clean);
    }
    fs::write(path, text).map_err(|e| format!("{shown}: {e}"))?;
    Ok(Said::Found(vec![format!("{shown}: {stale} stale cells given their formula's value, every computed value stored")]))
}

/// The file as the format's standard lays it out (`cells::layout::formatted`): written
/// when it is not, or with `check` only said. A file the engine refuses (bytes that are
/// not UTF-8) is said, never written.
fn fmt(path: &Path, threads: usize, align: bool, check: bool) -> Result<Said, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let shown = path.display();
    if let Err(e) = cells::read_with(bytes.clone(), &mut Disk::new(path), threads) {
        return Ok(Said::Found(vec![format!("{shown} {e}")]));
    }
    let text = String::from_utf8(bytes).expect("a file the engine reads is UTF-8");
    let formatted = cells::layout::formatted(&text, align);
    if formatted == text {
        return Ok(Said::Clean);
    }
    if check {
        return Ok(Said::Found(vec![format!("{shown}: not formatted (cells fmt)")]));
    }
    fs::write(path, &formatted).map_err(|e| format!("{shown}: {e}"))?;
    Ok(Said::Found(vec![format!("{shown}: formatted")]))
}

/// What `cells fmt`'s arguments say: its alignment, whether it only checks, its paths.
fn fmt_args(args: &[String]) -> Option<(bool, bool, &[String])> {
    let (mut align, mut check, mut at) = (true, false, 0);
    while at < args.len() {
        match args[at].as_str() {
            "--check" => check = true,
            "--align" => {
                align = match args.get(at + 1).map(String::as_str) {
                    Some("on") => true,
                    Some("off") => false,
                    _ => return None,
                };
                at += 1;
            }
            _ => break,
        }
        at += 1;
    }
    (at < args.len()).then(|| (align, check, &args[at..]))
}

/// Runs `each` over the files: many files share the threads one each, a few files take
/// them all inside each file. Prints in path order.
fn run(paths: &[String], each: impl Fn(&Path, usize) -> Result<Said, String> + Sync) -> ExitCode {
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
            Ok(Said::Noted(lines)) => lines.iter().for_each(|l| println!("{l}")),
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
        Some("help" | "--help" | "-h") if args.len() == 1 => {
            println!("{}", describe::help());
            ExitCode::SUCCESS
        }
        Some("help") if args.len() == 2 && args[1] == "format" => {
            println!("{}", describe::format_guide());
            ExitCode::SUCCESS
        }
        Some("skill") => match args.get(1).map(String::as_str) {
            None => skill::status(),
            Some("show") if args.len() == 2 => {
                print!("{}", describe::skill());
                ExitCode::SUCCESS
            }
            Some("install") if args.len() == 2 => skill::install(),
            Some("remove") if args.len() == 2 => skill::remove(),
            _ => {
                eprintln!("{}", describe::usage());
                ExitCode::from(2)
            }
        },
        Some("check") if args.len() > 1 => run(&args[1..], check),
        Some("lsp") if args.len() == 1 => {
            lsp::serve();
            ExitCode::SUCCESS
        }
        // Served until the process ends; only a folder it cannot serve returns.
        Some("serve") => match cells_daemon::serve(&args[1..]) {
            Ok(never) => match never {},
            Err(e) => {
                eprintln!("cells serve: {e}");
                ExitCode::from(2)
            }
        },
        Some("fmt") => match fmt_args(&args[1..]) {
            // Formatting files exits 0, as a fix does; a check exits 1 on a file it would change.
            Some((align, false, paths)) => match run(paths, |p, t| fmt(p, t, align, false)) {
                code if code == ExitCode::from(2) => code,
                _ => ExitCode::SUCCESS,
            },
            Some((align, true, paths)) => run(paths, |p, t| fmt(p, t, align, true)),
            None => {
                eprintln!("{}", describe::usage());
                ExitCode::from(2)
            }
        },
        // git's textconv: the file with its padding taken out, so a diff shows values.
        Some("diff") if args.len() == 2 => match fs::read(&args[1]) {
            Ok(bytes) => {
                match String::from_utf8(bytes) {
                    Ok(text) => print!("{}", cells::layout::formatted(&text, false)),
                    Err(e) => {
                        use std::io::Write;
                        std::io::stdout().write_all(e.as_bytes()).expect("stdout");
                    }
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("cells: {}: {e}", args[1]);
                ExitCode::from(2)
            }
        },
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
        Some("pack") => match pack::args(&args[1..]) {
            Some((remove, out, file)) => match pack::pack(Path::new(file), remove, out.map(Path::new)) {
                Ok((said, whole)) => {
                    said.iter().for_each(|l| println!("{l}"));
                    if whole { ExitCode::SUCCESS } else { ExitCode::from(1) }
                }
                Err(e) => {
                    eprintln!("cells: {e}");
                    ExitCode::from(2)
                }
            },
            None => {
                eprintln!("{}", describe::usage());
                ExitCode::from(2)
            }
        },
        Some("editors") if args.len() == 2 && args[1] == "update" => {
            let (said, failed) = editors::update();
            said.iter().for_each(|l| println!("{l}"));
            if failed { ExitCode::from(2) } else { ExitCode::SUCCESS }
        }
        Some("editors") if args.len() == 3 && args[1] == "install" => match editors::install(&args[2]) {
            Ok(said) => {
                said.iter().for_each(|l| println!("{l}"));
                ExitCode::SUCCESS
            }
            Err(why) => {
                eprintln!("cells: {why}");
                ExitCode::from(2)
            }
        },
        Some("chart") if args.len() >= 2 => match chart::run(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(why) => {
                eprintln!("cells: {why}");
                ExitCode::from(2)
            }
        },
        Some("show") if args.len() == 2 => {
            let path = Path::new(&args[1]);
            let shown = fs::read(path)
                .map_err(|e| format!("{}: {e}", path.display()))
                .and_then(|b| cells::read_with(b, &mut Disk::new(path), 1).map_err(|e| format!("{} {e}", path.display())));
            match shown {
                Ok(doc) => {
                    print!("{}", doc.shown());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("cells: {e}");
                    ExitCode::from(2)
                }
            }
        }
        // DuckDB is `cells-sql`'s alone, so this binary (which every editor's language server
        // is) stays small: the query is handed to it, found beside this binary (the real one: on
        // the PATH `cells` is a link into Cells.app, whose Helpers hold both) or on the PATH
        Some("sql") => {
            let here = std::env::current_exe()
                .and_then(|e| e.canonicalize())
                .ok()
                .and_then(|e| e.parent().map(|d| d.join("cells-sql")))
                .filter(|p| p.is_file());
            let sql = here.unwrap_or_else(|| PathBuf::from("cells-sql"));
            match std::process::Command::new(&sql).args(&args[1..]).status() {
                Ok(status) => ExitCode::from(status.code().map_or(2, |c| c as u8)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    eprintln!(
                        "cells sql: cells-sql is not installed: it holds DuckDB, apart from cells (`cargo install --path rust/cells-sql`)"
                    );
                    ExitCode::from(2)
                }
                Err(e) => {
                    eprintln!("cells sql: {}: {e}", sql.display());
                    ExitCode::from(2)
                }
            }
        }
        Some("history") if args.len() == 2 || args.len() == 4 => {
            let cell = match args.get(2).map(|r| r.parse::<usize>()) {
                None => None,
                Some(Ok(row)) => Some((row, args[3].as_str())),
                Some(Err(_)) => {
                    eprintln!("cells history: a row is its number, from 1: {}", args[2]);
                    return ExitCode::from(2);
                }
            };
            match history::history(Path::new(&args[1]), cell) {
                Ok(said) => {
                    said.iter().for_each(|l| println!("{l}"));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("cells history: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Some("allow") if (2..=3).contains(&args.len()) => match allow::allow(Path::new(&args[1]), args.get(2).map(String::as_str)) {
            Ok(said) => {
                said.iter().for_each(|l| println!("{l}"));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("cells: {e}");
                ExitCode::from(2)
            }
        },
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
            eprintln!("{}", describe::usage());
            ExitCode::from(2)
        }
    }
}
