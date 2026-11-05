//! The prelude between two `---` lines, as statements: named types, imports, tables
//! written inline and tables a script fills, with `//` comments. What a statement means
//! (a type parsed, an import followed, a table read) is the doc's to settle, in order;
//! this is its grammar, fences included, so a `---` inside a fence never closes it.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// A fenced block: its language and where its content lies in the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fenced {
    /// The word after the opening backticks: `cells`, `ts`.
    pub lang: String,
    /// What follows the language on the opening line: `runs` in ```` ```cells runs ````.
    pub label: String,
    /// The prelude line of the opening fence, 1-based inside the fences.
    pub line: u32,
    /// The content's bytes in the source: every line between the fences, breaks included.
    pub start: usize,
    pub end: usize,
}

/// What starts a script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// `every 15m`, `every hour`: minutes between runs.
    Every(u32),
    /// `every day at 18:00`.
    Daily(u16),
    /// `every weekday at 09:00`.
    Weekdays(u16),
    /// `every monday at 09:00`: the day, 1 for Monday to 7 for Sunday.
    Weekly(u8, u16),
    /// `every month on 1 at 08:00`.
    Monthly(u8, u16),
    /// `cron 0 9 * * 1-5`, its five fields.
    Cron(String),
    /// `when ../money.cell changes`.
    Changes(String),
}

impl Trigger {
    /// Whether it names a time of day, which needs a zone.
    pub fn clock(&self) -> bool {
        matches!(self, Trigger::Daily(_) | Trigger::Weekdays(_) | Trigger::Weekly(..) | Trigger::Monthly(..) | Trigger::Cron(_))
    }
}

/// `run …`: its triggers and the zone their times of day are in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schedule {
    pub triggers: Vec<Trigger>,
    pub zone: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Statement {
    /// `type Name = <type>`, `export type Name = <type>`.
    Type { line: u32, name: String, spelled: String, exported: bool },
    /// `import name from "path"`, `import { A, b } from "path" with { sha256: "…" }`.
    Import { line: u32, default: Option<String>, names: Vec<String>, from: String, with: Vec<(String, String)> },
    /// `name = ```cells`, its rows inline.
    Table { line: u32, name: String, exported: bool, body: Fenced },
    /// `name = run <schedule>`, its script's fence, and what the engine stored under it:
    /// the rows of its last run and the runs they came from.
    Run { line: u32, name: String, exported: bool, schedule: Schedule, code: Fenced, stored: Option<Fenced>, runs: Option<Fenced> },
}

impl Statement {
    pub fn line(&self) -> u32 {
        match self {
            Statement::Type { line, .. } | Statement::Import { line, .. } | Statement::Table { line, .. } | Statement::Run { line, .. } => {
                *line
            }
        }
    }
}

/// The line with its `//` comment cut, outside single and double quotes.
pub fn uncomment(line: &str) -> &str {
    let b = line.as_bytes();
    let mut quote = 0u8;
    for i in 0..b.len() {
        if quote != 0 {
            if b[i] == quote {
                quote = 0;
            }
        } else if b[i] == b'\'' || b[i] == b'"' {
            quote = b[i];
        } else if b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            return &line[..i];
        }
    }
    line
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

const BLANK: [char; 2] = [' ', '\t'];

/// A line's content, without its break.
fn content(l: &str) -> &str {
    l.strip_suffix('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).unwrap_or(l)
}

/// An identifier at the start of `s`, and the rest.
fn ident(s: &str) -> Option<(&str, &str)> {
    let b = s.as_bytes();
    if !b.first().is_some_and(|&c| is_ident_start(c)) {
        return None;
    }
    let end = b.iter().position(|&c| !is_ident(c)).unwrap_or(b.len());
    Some((&s[..end], &s[end..]))
}

/// A line opening a fence: its backticks, its language and what follows it.
fn opening(s: &str) -> Option<(usize, &str, &str)> {
    let ticks = s.bytes().take_while(|&c| c == b'`').count();
    if ticks < 3 {
        return None;
    }
    let rest = s[ticks..].trim_matches(BLANK);
    let (lang, label) = match rest.find(BLANK) {
        Some(at) => (&rest[..at], rest[at..].trim_matches(BLANK)),
        None => (rest, ""),
    };
    Some((ticks, lang, label))
}

/// A prelude that opens at `from` (its first `---` line) read to its closing `---`: its
/// statements, and where the header starts. An error names its line (1-based inside the
/// fences), or `0` for a prelude never closed, which is what a statement's error is when
/// no `---` follows it: the lines it reads are the file's.
pub fn parse(source: &str, from: usize) -> Result<(Vec<Statement>, usize), (u32, String)> {
    statements(source, from).map_err(|(n, why)| {
        let after = source[from..].split_inclusive('\n').skip(n as usize + 1).any(|l| content(l) == "---");
        if n == 0 || after { (n, why) } else { (0, "opens with --- and never closes".to_owned()) }
    })
}

fn statements(source: &str, from: usize) -> Result<(Vec<Statement>, usize), (u32, String)> {
    let mut lines = Vec::new();
    let mut at = from;
    for l in source[from..].split_inclusive('\n') {
        lines.push((at, l));
        at += l.len();
    }
    // The opening `---` and its break, checked by the caller.
    let mut k = 1;
    let mut out = Vec::new();
    let line_no = |k: usize| k as u32;
    // A fence open at line `k`: its content to its closing line, and the line after it.
    let fence = |k: usize, ticks: usize, lang: &str, label: &str| -> Result<(Fenced, usize), (u32, String)> {
        let start = lines.get(k + 1).map_or(source.len(), |l| l.0);
        let mut j = k + 1;
        while j < lines.len() {
            let c = content(lines[j].1);
            if c.trim_end_matches(BLANK).len() == ticks && c.bytes().all(|b| b == b'`' || b == b' ' || b == b'\t') && c.starts_with('`') {
                let body = Fenced { lang: lang.to_owned(), label: label.to_owned(), line: line_no(k), start, end: lines[j].0 };
                return Ok((body, j + 1));
            }
            j += 1;
        }
        Err((line_no(k), format!("a fence of {ticks} backticks is never closed")))
    };
    loop {
        let Some(&(_, raw)) = lines.get(k) else { return Err((0, "opens with --- and never closes".to_owned())) };
        let n = line_no(k);
        let text = content(raw);
        if text == "---" {
            let end = lines[k].0 + raw.len();
            return Ok((out, end));
        }
        if !raw.ends_with('\n') {
            return Err((0, "opens with --- and never closes".to_owned()));
        }
        let said = uncomment(text).trim_matches(BLANK);
        k += 1;
        if said.is_empty() {
            continue;
        }
        let fail = |why: String| Err((n, why));
        let (exported, said) = match said.strip_prefix("export").filter(|r| r.starts_with(BLANK)) {
            Some(r) => (true, r.trim_start_matches(BLANK)),
            None => (false, said),
        };
        if let Some(rest) = said.strip_prefix("type").filter(|r| r.starts_with(BLANK) && !r.trim_start_matches(BLANK).starts_with('=')) {
            let rest = rest.trim_start_matches(BLANK);
            let (name, after) = ident(rest).unwrap_or(("", rest));
            let Some(spelled) = after.trim_start_matches(BLANK).strip_prefix('=') else {
                return fail("a named type is `type Name = <type>`".to_owned());
            };
            out.push(Statement::Type { line: n, name: name.to_owned(), spelled: spelled.trim_matches(BLANK).to_owned(), exported });
            continue;
        }
        if let Some(rest) = said.strip_prefix("import").filter(|r| r.starts_with(BLANK) || r.starts_with('{')) {
            if exported {
                return fail("an import is the file's own; export what it names instead".to_owned());
            }
            out.push(import(n, rest.trim_start_matches(BLANK))?);
            continue;
        }
        let Some((name, after)) = ident(said) else {
            return fail("is no statement: a type, an import, or name = a table".to_owned());
        };
        let Some(value) = after.trim_start_matches(BLANK).strip_prefix('=') else {
            return fail("is no statement: a type, an import, or name = a table".to_owned());
        };
        let value = value.trim_matches(BLANK);
        if let Some((ticks, lang, label)) = opening(value) {
            if lang != "cells" || !label.is_empty() {
                return fail(format!("a table written inline is a ```cells fence, not ```{}", lang));
            }
            let (body, next) = fence(n as usize, ticks, lang, label)?;
            k = next;
            out.push(Statement::Table { line: n, name: name.to_owned(), exported, body });
            continue;
        }
        if let Some(rest) = value.strip_prefix("run").filter(|r| r.is_empty() || r.starts_with(BLANK)) {
            let schedule = schedule(rest.trim_matches(BLANK)).map_err(|why| (n, why))?;
            // The script's fence, then what the engine stored: blank lines may stand between.
            let mut blocks = Vec::new();
            loop {
                while lines.get(k).is_some_and(|l| content(l.1).trim_matches(BLANK).is_empty()) {
                    k += 1;
                }
                let Some(&(_, l)) = lines.get(k) else { break };
                let Some((ticks, lang, label)) = opening(content(l).trim_matches(BLANK)) else { break };
                let (body, next) = fence(k, ticks, lang, label)?;
                blocks.push(body);
                k = next;
            }
            let mut blocks = blocks.into_iter();
            let Some(code) = blocks.next() else {
                return fail("run is followed by its script's fence: ```ts".to_owned());
            };
            if code.lang.is_empty() {
                return Err((code.line, "a script's fence names its language: ```ts".to_owned()));
            }
            if code.lang != "ts" {
                return Err((code.line, format!("no runner for ```{}: v0 runs ```ts", code.lang)));
            }
            let mut stored = None;
            let mut runs = None;
            for b in blocks {
                match (b.lang.as_str(), b.label.as_str()) {
                    ("cells", "") if stored.is_none() && runs.is_none() => stored = Some(b),
                    ("cells", "runs") if runs.is_none() => runs = Some(b),
                    _ => {
                        return Err((b.line, "after a script come its rows (```cells) and its runs (```cells runs), once each".to_owned()));
                    }
                }
            }
            out.push(Statement::Run { line: n, name: name.to_owned(), exported, schedule, code, stored, runs });
            continue;
        }
        if exported {
            return fail("export names a type or a table".to_owned());
        }
        return fail(format!(
            "{name} = … names a table (```cells) or a script (run): a computed column is a header cell, {name}:type=formula"
        ));
    }
}

/// What follows `import`.
fn import(line: u32, s: &str) -> Result<Statement, (u32, String)> {
    let fail = |why: &str| Err((line, why.to_owned()));
    let mut s = s;
    let mut default = None;
    let mut names = Vec::new();
    if let Some(rest) = s.strip_prefix('{') {
        let Some(close) = rest.find('}') else { return fail("a { is not closed") };
        for part in rest[..close].split(',') {
            let part = part.trim_matches(BLANK);
            match ident(part) {
                Some((n, "")) => names.push(n.to_owned()),
                _ => return fail("an import names what it takes: import { Kind, rates } from \"…\""),
            }
        }
        if names.is_empty() {
            return fail("an import names what it takes: import { Kind, rates } from \"…\"");
        }
        s = rest[close + 1..].trim_start_matches(BLANK);
    } else {
        let Some((n, rest)) = ident(s) else { return fail("import takes a name: import accounts from \"../accounts.cell\"") };
        default = Some(n.to_owned());
        s = rest.trim_start_matches(BLANK);
    }
    let Some(rest) = s.strip_prefix("from").filter(|r| r.starts_with(BLANK)) else {
        return fail("an import says where from: from \"../accounts.cell\"");
    };
    let (from, rest) =
        string(rest.trim_start_matches(BLANK)).ok_or((line, "where an import comes from is a quoted path or URL".to_owned()))?;
    let rest = rest.trim_matches(BLANK);
    let mut with = Vec::new();
    if !rest.is_empty() {
        let Some(attrs) = rest.strip_prefix("with").map(|r| r.trim_start_matches(BLANK)) else {
            return fail("an import's attributes follow with: with { sha256: \"…\" }");
        };
        let Some(inner) = attrs.strip_prefix('{').and_then(|r| r.trim_end_matches(BLANK).strip_suffix('}')) else {
            return fail("an import's attributes are { key: \"value\" }");
        };
        let mut inner = inner.trim_matches(BLANK);
        while !inner.is_empty() {
            let Some((key, r)) = ident(inner) else { return fail("an attribute is key: \"value\"") };
            let Some(r) = r.trim_start_matches(BLANK).strip_prefix(':') else { return fail("an attribute is key: \"value\"") };
            let (value, r) = string(r.trim_start_matches(BLANK)).ok_or((line, "an attribute's value is quoted".to_owned()))?;
            if !matches!(key, "sha256" | "rev") {
                return fail("an import takes sha256 or rev");
            }
            if with.iter().any(|(k, _): &(String, String)| k == key) {
                return Err((line, format!("{key} is said twice")));
            }
            with.push((key.to_owned(), value));
            inner = r.trim_start_matches(BLANK);
            inner = inner.strip_prefix(',').unwrap_or(inner).trim_start_matches(BLANK);
        }
    }
    Ok(Statement::Import { line, default, names, from, with })
}

/// A `"double quoted"` string at the start of `s` (`\"` and `\\` inside), and the rest.
fn string(s: &str) -> Option<(String, &str)> {
    let rest = s.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &rest[i + 1..])),
            '\\' => out.push(chars.next()?.1),
            c => out.push(c),
        }
    }
    None
}

const DAYS: [&str; 7] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];

/// `HH:MM`, as minutes since midnight.
fn clock(s: &str) -> Result<u16, String> {
    let bad = || format!("a time of day is HH:MM, not \"{s}\"");
    let (h, m) = s.split_once(':').ok_or_else(bad)?;
    if h.len() != 2 || m.len() != 2 {
        return Err(bad());
    }
    let (h, m): (u16, u16) = (h.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?);
    if h > 23 || m > 59 {
        return Err(bad());
    }
    Ok(h * 60 + m)
}

/// What follows `run`: triggers joined by commas, then `in <zone>` for their times of day.
pub fn schedule(s: &str) -> Result<Schedule, String> {
    if s.is_empty() {
        return Err("run says when: every hour, every day at 18:00 in Europe/Madrid, when ../money.cell changes".to_owned());
    }
    let (body, zone) = match s.rfind(" in ") {
        Some(at) if !s[at + 4..].trim_matches(BLANK).contains(' ') => (&s[..at], Some(s[at + 4..].trim_matches(BLANK).to_owned())),
        _ => (s, None),
    };
    if let Some(z) = &zone
        && !(z.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'/' | b'_' | b'-' | b'+')) && z.contains('/') || z == "UTC")
    {
        return Err(format!("\"{z}\" is no time zone: Europe/Madrid, UTC"));
    }
    let mut triggers = Vec::new();
    for part in body.split(',') {
        let words: Vec<&str> = part.split(BLANK).filter(|w| !w.is_empty()).collect();
        let t = match words.as_slice() {
            ["every", "minute"] => Trigger::Every(1),
            ["every", "hour"] => Trigger::Every(60),
            ["every", n] if n.ends_with('m') || n.ends_with('h') => {
                let (digits, unit) = n.split_at(n.len() - 1);
                let v: u32 = digits.parse().ok().filter(|v| *v > 0).ok_or_else(|| format!("\"{n}\" is no interval: 15m, 6h"))?;
                Trigger::Every(if unit == "h" { v * 60 } else { v })
            }
            ["every", "day", "at", t] => Trigger::Daily(clock(t)?),
            ["every", "weekday", "at", t] => Trigger::Weekdays(clock(t)?),
            ["every", day, "at", t] if DAYS.contains(day) => {
                Trigger::Weekly(DAYS.iter().position(|d| d == day).unwrap() as u8 + 1, clock(t)?)
            }
            ["every", "month", "on", d, "at", t] => {
                let day: u8 = d.parse().ok().filter(|d| (1..=31).contains(d)).ok_or_else(|| format!("\"{d}\" is no day of a month"))?;
                Trigger::Monthly(day, clock(t)?)
            }
            ["cron", fields @ ..] if fields.len() == 5 => Trigger::Cron(fields.join(" ")),
            ["when", path, "changes"] => Trigger::Changes((*path).to_owned()),
            _ => {
                return Err(format!(
                    "\"{}\" is no schedule: every 15m, every day at 18:00, cron 0 9 * * 1-5, when ../x.cell changes",
                    part.trim_matches(BLANK)
                ));
            }
        };
        triggers.push(t);
    }
    if zone.is_none() && triggers.iter().any(Trigger::clock) {
        return Err("a time of day is in a zone: in Europe/Madrid".to_owned());
    }
    Ok(Schedule { triggers, zone })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements() {
        let src = "---\ntype Kind = a|b // the kinds\nimport accounts from \"../accounts.cell\"\nimport { Rate } from \"https://x.example/r.cell\" with { sha256: \"ab\" }\n\nfees = ```cells\nkind:text!\n---\n```\nfx = run every day at 18:00 in Europe/Madrid\n```ts\nexport default () => []\n```\n```cells runs\nrun:int!\n```\n---\nh\n";
        let (s, body) = parse(src, 0).unwrap();
        assert_eq!(&src[body..], "h\n");
        assert_eq!(s.len(), 5);
        assert!(matches!(&s[1], Statement::Import { default: Some(d), .. } if d == "accounts"));
        assert!(matches!(&s[2], Statement::Import { names, with, .. } if names[0] == "Rate" && with[0].1 == "ab"));
        let Statement::Table { body, .. } = &s[3] else { panic!() };
        assert_eq!(&src[body.start..body.end], "kind:text!\n---\n");
        let Statement::Run { schedule: when, stored, runs, .. } = &s[4] else { panic!() };
        assert_eq!(when.triggers, [Trigger::Daily(18 * 60)]);
        assert!(stored.is_none() && runs.is_some());
        assert_eq!(parse("---\nnonsense\n---\n", 0).unwrap_err().0, 1);
        assert_eq!(parse("---\n\ntotal = cost * qty\n---\n", 0).unwrap_err().0, 2);
        assert_eq!(parse("---\nx = ```cells\na\n---\n", 0).unwrap_err().0, 1);
        assert!(schedule("every day at 18:00").is_err());
        assert_eq!(schedule("every 15m, when ../m.cell changes").unwrap().triggers.len(), 2);
    }
}
