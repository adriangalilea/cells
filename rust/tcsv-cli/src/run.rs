//! `tcsv run`: each script of a file run once, with Bun, and what it returned written in
//! the file under it (SPEC.md, A table from a script). The script exports a function given
//! the rows it last returned; the process between is the contract: the context as JSON on
//! stdin, a row a line on stdout, its words on stderr.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::disk::Disk;
use crate::json::{self, Scalar};
use tcsv::{Doc, Script, Value};

/// What starts the script and hands it and its rows over: the context on stdin, each row
/// returned a JSON line on stdout, and what the script prints on stderr.
const RUNNER: &str = r#"import script from "./script.ts"
console.log = console.error
const context = JSON.parse(await Bun.stdin.text())
const rows = await script(context)
if (!Array.isArray(rows)) throw new Error("the script returns no list of rows")
for (const row of rows) process.stdout.write(JSON.stringify(row) + "\n")
"#;

/// The engine's column of a script's table: the run that last wrote the row.
const RUN: &str = "run";

/// Now, as an instant the runs table holds.
fn now() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).expect("a clock after 1970").as_secs() as i64;
    tcsv::value::format_datetime(tcsv::value::Instant::from(secs, 0))
}

/// A table's rows as text, by column, the engine's `run` left out: what the script is given.
fn context(table: &Doc) -> String {
    let columns: Vec<usize> = (0..table.columns().len()).filter(|&k| table.columns()[k].name != RUN).collect();
    let row = |r: usize| {
        let cells: Vec<String> =
            columns.iter().map(|&k| format!("{}:{}", json::quote(&table.columns()[k].name), json::quote(&table.text(r, k)))).collect();
        format!("{{{}}}", cells.join(","))
    };
    let rows: Vec<String> = (0..table.rows()).map(row).collect();
    let last = rows.last().cloned().unwrap_or_else(|| "null".into());
    format!("{{\"rows\":[{}],\"last\":{last},\"now\":{}}}", rows.join(","), json::quote(&now()))
}

/// The script run with Bun: the rows it wrote, or why it failed.
fn execute(script: &Script, context: &str) -> Result<Vec<String>, String> {
    let dir = std::env::temp_dir().join(format!("tcsv-run-{}-{}", std::process::id(), script.name));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join("script.ts"), &script.code).map_err(|e| e.to_string())?;
    fs::write(dir.join("runner.ts"), RUNNER).map_err(|e| e.to_string())?;
    let child = Command::new("bun")
        .arg("run")
        .arg(dir.join("runner.ts"))
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = child.map_err(|_| "bun is not installed: v0 runs ```ts with Bun".to_owned())?;
    child.stdin.take().unwrap().write_all(context.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&dir);
    let log = String::from_utf8_lossy(&out.stderr);
    for l in log.lines() {
        eprintln!("{}: {l}", script.name);
    }
    if !out.status.success() {
        let why = log.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("the script failed").trim();
        return Err(why.to_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).lines().filter(|l| !l.trim().is_empty()).map(str::to_owned).collect())
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
            .ok_or_else(|| format!("the script returns {name:?}, a column {} does not have", "its table"))?;
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

/// The file's text with the content of `fence` (or, when it has none, a new fence at
/// `at`) replaced by `content`.
fn replaced(text: &str, fence: Option<&tcsv::Fenced>, at: usize, open: &str, content: &str) -> String {
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
            columns.iter().zip(cells).map(|(c, t)| if c.name == RUN { run.clone() } else { tcsv::quote(t).into_owned() }).collect();
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
fn runs_text(runs: Option<&Doc>, rows: &[(Vec<String>, String)], run: u64, outcome: &str, error: &str, nl: &str) -> String {
    let mut all: Vec<Vec<String>> = match runs {
        Some(d) => (0..d.rows()).map(|r| d.texts(r).into_iter().map(|t| t.into_owned()).collect()).collect(),
        None => Vec::new(),
    };
    all.push(vec![run.to_string(), now(), outcome.to_owned(), error.to_owned()]);
    let last_failure = all.iter().rposition(|r| r.get(2).is_some_and(|o| o == "failed"));
    let n = all.len();
    let kept: Vec<String> = all
        .iter()
        .enumerate()
        .filter(|(i, r)| *i == n - 1 || Some(*i) == last_failure || rows.iter().any(|(_, used)| *used == r[0]))
        .map(|(_, r)| r.iter().map(|c| tcsv::quote(c).into_owned()).collect::<Vec<_>>().join(","))
        .collect();
    format!("run:int!,at:datetime,outcome:ok|failed,error:text?{nl}{}{nl}", kept.join(nl))
}

/// Each script of the file at `path` (or only `only`) run once, the file rewritten after
/// each: what each did, a line each, and whether every one succeeded.
pub fn run(path: &Path, only: Option<&str>) -> Result<(Vec<String>, bool), String> {
    let read =
        |text: &str| tcsv::read_with(text.as_bytes().to_vec(), &mut Disk::new(path), 1).map_err(|e| format!("{} {e}", path.display()));
    let mut text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let doc = read(&text)?;
    let names: Vec<String> = doc.scripts().iter().map(|s| s.name.clone()).filter(|n| only.is_none_or(|o| o == n)).collect();
    if names.is_empty() {
        return Err(match only {
            Some(o) => format!("{} has no script {o}", path.display()),
            None => format!("{} has no script", path.display()),
        });
    }
    let mut said = Vec::new();
    let mut all_ok = true;
    for name in names {
        let doc = read(&text)?;
        let script = doc.scripts().iter().find(|s| s.name == name).unwrap().clone();
        let nl = doc.newline().to_owned();
        let table = doc.tables().iter().find(|t| t.name == name).and_then(|t| t.doc.clone()).expect("a script's table, declared");
        let runs = script
            .runs
            .as_ref()
            .map(|f| tcsv::read(text.as_bytes()[f.start..f.end].to_vec()))
            .transpose()
            .map_err(|e| format!("{} {name}'s runs: {e}", path.display()))?;
        let run = runs.as_ref().map_or(0, |d| (0..d.rows()).filter_map(|r| d.text(r, 0).parse::<u64>().ok()).max().unwrap_or(0)) + 1;
        let outcome =
            execute(&script, &context(&table)).and_then(|lines| lines.iter().map(|l| cells(&table, l)).collect::<Result<Vec<_>, _>>());
        let (stored, outcome) = match outcome {
            Err(why) => (None, Err(why)),
            Ok(returned) => {
                let (rows, added, changed) = merged(&table, returned, run);
                let candidate = replaced(&text, script.stored.as_ref(), script.after, "```tcsv", &table_text(&table, &rows, &nl));
                // What a run returns lands only when it reads: its rows checked as typed rows are.
                match read(&candidate) {
                    Err(e) => (None, Err(e.to_string())),
                    Ok(d) => {
                        let t = d.tables().iter().find(|t| t.name == name).and_then(|t| t.doc.clone()).unwrap();
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
        let base = stored.map(|(_, t)| t).unwrap_or_else(|| text.clone());
        // The runs block's place in the text the rows were written into.
        let again = read(&base)?;
        let s = again.scripts().iter().find(|s| s.name == name).unwrap().clone();
        text = replaced(&base, s.runs.as_ref(), s.after, "```tcsv runs", &runs_text(runs.as_ref(), &rows, run, word, &error, &nl));
        let tmp = path.with_extension("tcsv.run");
        fs::write(&tmp, &text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))?;
        said.push(match outcome {
            Ok((added, changed)) => format!("{name}: run {run} ok, {added} added, {changed} changed"),
            Err(why) => {
                all_ok = false;
                format!("{name}: run {run} failed: {why}")
            }
        });
    }
    Ok((said, all_ok))
}
