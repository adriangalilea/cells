//! `cells-sql <file|folder>... <query>`: what `cells sql` runs. DuckDB is here alone, built
//! from its bundled sources, so `cells` (whose `cells lsp` every editor window starts) never
//! carries a database it does not use.
//!
//! Exit 0 when the query ran over files with no problem, 1 when it ran over a broken cell
//! (read as null, said on stderr), 2 when it cannot run (a path that does not exist, tables
//! whose columns clash, a query DuckDB refuses).

mod sql;

use std::process::ExitCode;

const USAGE: &str = "cells sql <file|folder>... <query>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let (paths, query) = args.split_at(args.len() - 1);
    match sql::sql(paths, &query[0]) {
        Ok((0, _)) => ExitCode::SUCCESS,
        Ok((problems, files)) => {
            let (n, s) = (if problems == 1 { "problem" } else { "problems" }, if files == 1 { "file" } else { "files" });
            eprintln!("cells sql: {problems} {n} in {files} {s} read, a broken cell read as null: cells check {}", paths.join(" "));
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("cells sql: {e}");
            ExitCode::from(2)
        }
    }
}
