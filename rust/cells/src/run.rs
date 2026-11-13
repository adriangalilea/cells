//! A script's run as the file records it (SPEC.md, A table from a script, and Running):
//! what the script is handed, and the file after it returned. No process, disk or clock:
//! the host runs the script (Bun, for `ts`), hands in what it returned and the time, and
//! writes the text this gives back, so every runner (the engine, a test) lands a run the
//! same way.
//!
//! The process between a runner and a script is the contract: the context as JSON on
//! stdin (`context`), a row a line on stdout, its words on stderr.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::json::{self, Scalar};
use crate::value::{Instant, format_datetime, parse_datetime};
use crate::{Doc, Fenced, Script, Value};

/// The engine's column of a script's table: the run that last wrote the row.
pub const RUN: &str = "run";

/// What a run did: its number, and the rows added and changed, or why it failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ran {
    pub run: u64,
    pub outcome: Result<(usize, usize), String>,
}

/// What a script is handed: its table's rows as text, by column, the engine's `run` left
/// out, the last of them, and `now`.
pub fn context(table: &Doc, now: Instant) -> String {
    let columns: Vec<usize> = (0..table.columns().len()).filter(|&k| table.columns()[k].name != RUN).collect();
    let row = |r: usize| {
        let cells: Vec<String> =
            columns.iter().map(|&k| format!("{}:{}", json::quote(&table.columns()[k].name), json::quote(&table.text(r, k)))).collect();
        format!("{{{}}}", cells.join(","))
    };
    let rows: Vec<String> = (0..table.rows()).map(row).collect();
    let last = rows.last().cloned().unwrap_or_else(|| "null".into());
    format!("{{\"rows\":[{}],\"last\":{last},\"now\":{}}}", rows.join(","), json::quote(&format_datetime(now)))
}

/// The runs block of `script` in `text`, read: nothing when it has not run.
fn runs(text: &str, script: &Script) -> Result<Option<Doc>, String> {
    script
        .runs
        .as_ref()
        .map(|f| crate::read(text.as_bytes()[f.start..f.end].to_vec()))
        .transpose()
        .map_err(|e| format!("{}'s runs: {e}", script.name))
}

/// When `script` last ran, whatever came of it: the latest `at` of its runs block.
pub fn last_ran(text: &str, script: &Script) -> Option<Instant> {
    let runs = runs(text, script).ok()??;
    let at = runs.columns().iter().position(|c| c.name == "at")?;
    (0..runs.rows()).filter_map(|r| parse_datetime(runs.text(r, at).as_bytes())).max()
}

/// A returned row as the table's cells, in its columns' order: text as it is, a number as
/// written, a boolean in the column's words, null blank. A column the table does not have
/// fails the run.
fn cells(table: &Doc, line: &str) -> Result<Vec<String>, String> {
    let row = json::row(line)?;
    let columns = table.columns();
    let mut out = vec![String::new(); columns.len()];
    for (name, v) in row {
        let k = columns
            .iter()
            .position(|c| c.name == name && c.name != RUN)
            .ok_or_else(|| format!("the script returns {name:?}, a column its table does not have"))?;
        let c = &columns[k];
        out[k] = match v {
            Scalar::Null => String::new(),
            Scalar::Str(s) => c.form(Value::Text(&s)),
            Scalar::Number(n) => n,
            Scalar::Bool(b) => c.form(Value::Bool(b)),
        };
    }
    Ok(out)
}

/// The text with the content of `fence` (or, when it has none, a new fence at `at`)
/// replaced by `content`.
fn replaced(text: &str, fence: Option<&Fenced>, at: usize, open: &str, content: &str) -> String {
    match fence {
        Some(f) => format!("{}{content}{}", &text[..f.start], &text[f.end..]),
        None => format!("{}{open}\n{content}```\n{}", &text[..at], &text[at..]),
    }
}

/// A table's rows as written, each with the run that last wrote it.
fn stored(table: &Doc) -> Vec<(Vec<String>, String)> {
    let run_at = table.columns().iter().position(|c| c.name == RUN);
    (0..table.rows())
        .map(|r| {
            let texts: Vec<String> = table.texts(r).into_iter().map(|t| t.into_owned()).collect();
            let run = run_at.map(|k| texts[k].clone()).unwrap_or_default();
            (texts, run)
        })
        .collect()
}

/// The rows a run leaves: with a key, the stored rows with each key returned replaced and
/// each new key added; without one, the rows returned. Each row with the run that last
/// wrote it; how many were added and changed.
fn merged(table: &Doc, returned: Vec<Vec<String>>, run: u64) -> (Vec<(Vec<String>, String)>, usize, usize) {
    let columns = table.columns();
    let run_at = columns.iter().position(|c| c.name == RUN);
    let stored = stored(table);
    let Some(key) = columns.iter().position(|c| c.ty.key) else {
        let n = returned.len();
        return (returned.into_iter().map(|c| (c, run.to_string())).collect(), n, 0);
    };
    let (mut rows, mut added, mut changed) = (stored, 0, 0);
    for cells in returned {
        let same = |row: &Vec<String>| row[key] == cells[key];
        match rows.iter().position(|(row, _)| same(row)) {
            Some(i) => {
                let differs = (0..columns.len()).any(|k| Some(k) != run_at && rows[i].0[k] != cells[k]);
                if differs {
                    rows[i] = (cells, run.to_string());
                    changed += 1;
                }
            }
            None => {
                rows.push((cells, run.to_string()));
                added += 1;
            }
        }
    }
    (rows, added, changed)
}

/// The text of a script's table: its header (with the engine's `run` column), then each
/// row, its run in that column.
fn table_text(table: &Doc, rows: &[(Vec<String>, String)], nl: &str) -> String {
    let columns = table.columns();
    let has_run = columns.iter().any(|c| c.name == RUN);
    let header = table.source()[table.body()..table.records()[0] as usize].trim_end_matches(['\r', '\n']).to_owned();
    // A row written by hand was written by no run: the column may be blank.
    let mut out = if has_run { header } else { format!("{header},{RUN}:int?") };
    out.push_str(nl);
    for (cells, run) in rows {
        let mut fields: Vec<String> =
            columns.iter().zip(cells).map(|(c, t)| if c.name == RUN { run.clone() } else { crate::quote(t).into_owned() }).collect();
        if !has_run {
            fields.push(run.clone());
        }
        out.push_str(&fields.join(","));
        out.push_str(nl);
    }
    out
}

/// The runs table after this one: the runs some row names, the last one and the last
/// failure.
fn runs_text(runs: Option<&Doc>, rows: &[(Vec<String>, String)], run: u64, at: Instant, outcome: &str, error: &str, nl: &str) -> String {
    let mut all: Vec<Vec<String>> = match runs {
        Some(d) => (0..d.rows()).map(|r| d.texts(r).into_iter().map(|t| t.into_owned()).collect()).collect(),
        None => Vec::new(),
    };
    all.push(vec![run.to_string(), format_datetime(at), outcome.to_owned(), error.to_owned()]);
    let last_failure = all.iter().rposition(|r| r.get(2).is_some_and(|o| o == "failed"));
    let n = all.len();
    let kept: Vec<String> = all
        .iter()
        .enumerate()
        .filter(|(i, r)| *i == n - 1 || Some(*i) == last_failure || rows.iter().any(|(_, used)| *used == r[0]))
        .map(|(_, r)| r.iter().map(|c| crate::quote(c).into_owned()).collect::<Vec<_>>().join(","))
        .collect();
    let header = [("run", "int!"), ("at", "datetime"), ("outcome", "ok|failed"), ("error", "text?")]
        .map(|(name, ty)| crate::prelude::header_cell(name, ty))
        .join(",");
    format!("{header}{nl}{}{nl}", kept.join(nl))
}

/// The number the next run of `name` in `text` takes: one past the last its runs block
/// holds.
pub fn next(text: &str, script: &Script) -> Result<u64, String> {
    let runs = runs(text, script)?;
    Ok(runs.as_ref().map_or(0, |d| (0..d.rows()).filter_map(|r| d.text(r, 0).parse::<u64>().ok()).max().unwrap_or(0)) + 1)
}

/// `text` after a run of the script `name`, at `at`, returned `returned` (a JSON row a
/// line) or failed: its rows landed when they read as typed rows, else nothing but its
/// failure; its runs block grown either way. `read` reads a version of the file as its
/// host does, imports followed.
pub fn landed(
    text: &str,
    name: &str,
    returned: Result<Vec<String>, String>,
    at: Instant,
    read: &mut dyn FnMut(&str) -> Result<Doc, String>,
) -> Result<(String, Ran), String> {
    let doc = read(text)?;
    let script = doc.scripts().iter().find(|s| s.name == name).ok_or_else(|| format!("no script {name}"))?.clone();
    let nl = doc.newline().to_owned();
    let table = doc.tables().iter().find(|t| t.name == name).and_then(|t| t.doc.clone()).expect("a script's table, declared");
    let runs = runs(text, &script)?;
    let run = next(text, &script)?;
    let outcome = returned.and_then(|lines| lines.iter().map(|l| cells(&table, l)).collect::<Result<Vec<_>, _>>());
    let (stored, outcome) = match outcome {
        Err(why) => (None, Err(why)),
        Ok(returned) => {
            let (rows, added, changed) = merged(&table, returned, run);
            let candidate = replaced(text, script.stored.as_ref(), script.after, "```cells", &table_text(&table, &rows, &nl));
            // what a run returns lands only when it reads: its rows checked as typed rows are
            match read(&candidate) {
                Err(e) => (None, Err(e)),
                Ok(d) => {
                    let t = d.tables().iter().find(|t| t.name == name).and_then(|t| t.doc.clone()).expect("the script's table");
                    match t.problems().first() {
                        Some(p) => (None, Err(format!("returns a row its types break: {}", t.describe(p)))),
                        None => (Some((rows, candidate)), Ok((added, changed))),
                    }
                }
            }
        }
    };
    let rows = stored.as_ref().map_or_else(|| self::stored(&table), |(r, _)| r.clone());
    let (word, error) = match &outcome {
        Ok(_) => ("ok", String::new()),
        Err(why) => ("failed", why.clone()),
    };
    let base = stored.map(|(_, t)| t).unwrap_or_else(|| text.to_owned());
    // the runs block's place in the text the rows were written into
    let again = read(&base)?;
    let s = again.scripts().iter().find(|s| s.name == name).expect("the script, still").clone();
    let text = replaced(&base, s.runs.as_ref(), s.after, "```cells runs", &runs_text(runs.as_ref(), &rows, run, at, word, &error, &nl));
    Ok((text, Ran { run, outcome }))
}
