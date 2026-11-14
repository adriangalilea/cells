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
    /// `when ../money.cells changes`.
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
    /// As written after `run`: `every day at 18:00 in Europe/Madrid`.
    pub spelled: String,
}

// A prelude holds a handful of statements: a script's size costs nothing.
#[allow(clippy::large_enum_variant)]
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
    /// `name: type`, `name: type = formula`: a column of the file's table, declared, its
    /// header cell then its name alone. `spelled` is what follows its `:`, as a header
    /// cell spells it after its own (its type, then `=` and its formula), spaces around
    /// the `=` cut.
    Column { line: u32, name: String, spelled: String },
    /// `look amount: grouped, negative red`: how the values of a column, a named type or
    /// the row look (SPEC.md, How a value looks), each rule as written.
    Look { line: u32, target: crate::look::Target, rules: Vec<String> },
    /// ```` ```chart bars ````, or `actual = ```chart`: a chart, its kind the fence's label
    /// (none when it is to be read from its entries), its entries the fence's lines
    /// (SPEC.md, Charts). Named only when another chart reads it.
    Chart { line: u32, name: Option<String>, body: Fenced },
    /// `packed accounts = ```cells`, `packed type Kind = …`, `packed look Kind: …`: a copy
    /// of what an import above names, kept in the file so it reads where the import cannot
    /// be reached (SPEC.md, Packed imports). Never this file's own: an edit of it is refused.
    Packed { line: u32, name: String, copy: Copy },
}

/// What a packed statement copies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Copy {
    /// A table: its header and rows in a ```cells fence, every type written whole.
    Table(Fenced),
    /// A named type, written whole: what follows its `=`.
    Type(String),
    /// A named type's looks, each rule as written.
    Look(Vec<String>),
}

impl Statement {
    pub fn line(&self) -> u32 {
        match self {
            Statement::Type { line, .. }
            | Statement::Import { line, .. }
            | Statement::Table { line, .. }
            | Statement::Run { line, .. }
            | Statement::Column { line, .. }
            | Statement::Look { line, .. }
            | Statement::Chart { line, .. }
            | Statement::Packed { line, .. } => *line,
        }
    }
}

/// The words that open a statement: a declared column whose name opens with one and a
/// space is quoted, so it never reads as that statement.
const OPENING: [&str; 5] = ["type", "import", "export", "look", "packed"];

/// A column's declaration, `name: type` or `name: type = formula`: its name and what
/// follows its `:`; none for a statement whose first `:` is not before its first `=` (a
/// table, a script). A name holding a `:`, a `,` or a `"` is quoted, `""` a quote inside.
fn column(said: &str) -> Result<Option<(String, String)>, String> {
    let (name, rest) = if let Some(quoted) = said.strip_prefix('"') {
        let mut name = String::new();
        let mut chars = quoted.char_indices().peekable();
        let rest = loop {
            match chars.next() {
                None => return Err("a column's quoted name is never closed".to_owned()),
                Some((i, '"')) => {
                    if chars.peek().is_some_and(|&(_, c)| c == '"') {
                        chars.next();
                        name.push('"');
                        continue;
                    }
                    break &quoted[i + 1..];
                }
                Some((_, c)) => name.push(c),
            }
        };
        match rest.trim_start_matches(BLANK).strip_prefix(':') {
            Some(after) => (name, after),
            None => return Err(format!("a column's quoted name is followed by its type: \"{name}\": type")),
        }
    } else {
        let Some(colon) = said.find(':') else { return Ok(None) };
        if said.find('=').is_some_and(|eq| eq < colon) {
            return Ok(None);
        }
        let name = said[..colon].trim_matches(BLANK);
        if name.contains([',', '"']) {
            return Err(format!("a column whose name holds a `,` or a `\"` is quoted: \"{}\": …", name.replace('"', "\"\"")));
        }
        (name.to_owned(), &said[colon + 1..])
    };
    if name.is_empty() {
        return Err("a column's declaration names it: name: type".to_owned());
    }
    let (ty, formula) = crate::types::split_formula(rest.trim_matches(BLANK));
    let ty = ty.trim_matches(BLANK);
    if ty.is_empty() {
        return Err(format!("{name}: no type"));
    }
    Ok(Some(match formula {
        Some(f) => (name, format!("{ty}={}", f.trim_matches(BLANK))),
        None => (name, ty.to_owned()),
    }))
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

/// A prelude read: the statements that read, in order, each one that does not by its
/// line and why (a line holding a control character is one), and where the header starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub statements: Vec<Statement>,
    /// Each statement's lines, the last one's excluded: a fenced one's fences inside.
    pub spans: Vec<core::ops::Range<u32>>,
    pub broken: Vec<(u32, String)>,
    pub body: usize,
}

/// A prelude that opens at `from` (its first `---` line) read to its closing `---`. A
/// statement that does not read is left out and the rest read; lines are 1-based inside
/// the fences. `Err` is a prelude never closed: every line after its `---` is its own.
pub fn parse(source: &str, from: usize) -> Result<Parsed, String> {
    let mut lines = Vec::new();
    let mut at = from;
    for l in source[from..].split_inclusive('\n') {
        lines.push((at, l));
        at += l.len();
    }
    let never = || "opens with --- and never closes".to_owned();
    // The opening `---` and its break, checked by the caller.
    let mut k = 1;
    let mut read: Vec<(Statement, core::ops::Range<usize>)> = Vec::new();
    let mut broken = Vec::new();
    let body = loop {
        let Some(&(start, raw)) = lines.get(k) else { return Err(never()) };
        if content(raw) == "---" {
            break start + raw.len();
        }
        if !raw.ends_with('\n') {
            return Err(never());
        }
        let first = k;
        match statement(source, &lines, &mut k) {
            Ok(Some(s)) => read.push((s, first..k)),
            Ok(None) => {}
            Err(e) => broken.push(e),
        }
    };
    // A line holding a control character breaks the statement it is in; a ```cells
    // fence's lines are its table's, whose cells say so themselves.
    let mut tables: Vec<core::ops::Range<usize>> = Vec::new();
    for (s, _) in &read {
        let fences = match s {
            Statement::Table { body, .. } => [Some(body), None],
            Statement::Run { stored, runs, .. } => [stored.as_ref(), runs.as_ref()],
            Statement::Packed { copy: Copy::Table(body), .. } => [Some(body), None],
            _ => [None, None],
        };
        for f in fences.into_iter().flatten() {
            tables.push(f.line as usize + 1..lines.iter().position(|l| l.0 == f.end).unwrap());
        }
    }
    let mut controlled = Vec::new();
    for (j, &(_, l)) in lines.iter().enumerate().take(k).skip(1) {
        if tables.iter().any(|t| t.contains(&j)) {
            continue;
        }
        if let Some(point) = crate::column::control(content(l).as_bytes()) {
            broken.push((j as u32, format!("holds U+{point:04X}, a control character")));
            controlled.push(j);
        }
    }
    read.retain(|(_, lines)| !controlled.iter().any(|j| lines.contains(j)));
    broken.sort_by_key(|b| b.0);
    // A script's blank lines between its fences are its own, those after its last are not.
    let spans = read
        .iter()
        .map(|(_, r)| {
            let end =
                (r.start + 1..r.end).rev().find(|&j| !content(lines[j].1).trim_matches(BLANK).is_empty()).map_or(r.start + 1, |j| j + 1);
            r.start as u32..end as u32
        })
        .collect();
    Ok(Parsed { statements: read.into_iter().map(|(s, _)| s).collect(), spans, broken, body })
}

/// A string as an import writes it: double quoted, `\"` and `\\` inside.
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `type Name = <type>`, as a writer writes it.
pub fn type_line(name: &str, spelled: &str, exported: bool) -> String {
    format!("{}type {name} = {spelled}", if exported { "export " } else { "" })
}

/// A script's first line, `rates = run every day at 18:00 in Europe/Madrid`, as a writer
/// writes it: its fences follow it.
pub fn run_line(name: &str, schedule: &str, exported: bool) -> String {
    format!("{}{name} = run {schedule}", if exported { "export " } else { "" })
}

/// An import as a writer writes it: `import accounts from "accounts.cells"`, `import { A, b }
/// from "…" with { sha256: "…" }`.
pub fn import_line(default: Option<&str>, names: &[String], from: &str, with: &[(String, String)]) -> String {
    let head = match default {
        Some(d) => d.to_owned(),
        None => format!("{{ {} }}", names.join(", ")),
    };
    let mut line = format!("import {head} from {}", quoted(from));
    if !with.is_empty() {
        let attrs: Vec<String> = with.iter().map(|(k, v)| format!("{k}: {}", quoted(v))).collect();
        line.push_str(&format!(" with {{ {} }}", attrs.join(", ")));
    }
    line
}

/// A packed copy of a table as a writer writes it: `packed accounts = ```cells`, its text
/// (lines ending in `\n`), its closing fence, a fence of more backticks than any line of it
/// opens with. Lines joined by `\n`, no break after the last.
pub fn packed_table_lines(name: &str, text: &str) -> String {
    let longest = text.lines().map(|l| l.bytes().take_while(|&b| b == b'`').count()).max().unwrap_or(0);
    let ticks = "`".repeat(longest.max(2) + 1);
    format!("packed {name} = {ticks}cells\n{text}{ticks}")
}

/// A packed copy of a named type as a writer writes it: `packed type Kind = income|cost`.
pub fn packed_type_line(name: &str, spelled: &str) -> String {
    format!("packed type {name} = {spelled}")
}

/// A packed copy of a named type's looks: `packed look Money: grouped, negative red`.
pub fn packed_look_line(name: &str, rules: &[String]) -> String {
    format!("packed look {name}: {}", rules.join(", "))
}

/// A column's declaration as a writer writes it, from what follows its `:` as a header cell
/// spells it (`EUR=cost * qty`): `total: EUR = cost * qty`.
pub fn column_line(name: &str, spelled: &str) -> String {
    format!("{}{}", declared_name(name), typed(spelled))
}

/// A typed header cell as a writer writes it, from its name and what follows its `:`
/// (`EUR=cost * qty`): `total: EUR = cost * qty`, the form a declaration has, so a column
/// is written one way wherever it is said. The field's quoting is the table's to add.
pub fn header_cell(name: &str, spelled: &str) -> String {
    format!("{name}{}", typed(spelled))
}

/// What follows a column's name, spaced as a writer spaces it: `: EUR`, `: EUR = cost * qty`.
/// The type keeps its shorthand whole and the formula its own spacing; the spaces stand
/// between the three parts, which read the same with or without them (SPEC.md, A header
/// cell).
fn typed(spelled: &str) -> String {
    match crate::types::split_formula(spelled) {
        (ty, Some(f)) => format!(": {} = {}", ty.trim_matches(BLANK), f.trim_matches(BLANK)),
        (ty, None) => format!(": {}", ty.trim_matches(BLANK)),
    }
}

/// A look as a writer writes it: `look amount: grouped, negative red`; a column named `row`
/// quoted, since `row` alone names the row.
pub fn look_line(target: &crate::look::Target, rules: &[String]) -> String {
    let named = match target {
        crate::look::Target::Row => "row".into(),
        crate::look::Target::Name(n) if n == "row" => "\"row\"".into(),
        crate::look::Target::Name(n) => declared_name(n),
    };
    format!("look {named}: {}", rules.join(", "))
}

/// A column's name as its declaration writes it: quoted, `""` a quote inside, when it
/// holds a `:`, a `,` or a `"`, starts or ends with a space or a tab, or opens with a
/// statement's word and a space (`"look ahead"`).
pub fn declared_name(name: &str) -> alloc::borrow::Cow<'_, str> {
    let edges = name.starts_with(BLANK) || name.ends_with(BLANK);
    let opens = OPENING.iter().any(|w| name.strip_prefix(w).is_some_and(|r| r.starts_with(BLANK)));
    if edges || opens || name.contains([':', ',', '"']) {
        alloc::borrow::Cow::Owned(format!("\"{}\"", name.replace('"', "\"\"")))
    } else {
        alloc::borrow::Cow::Borrowed(name)
    }
}

/// A fence open at line `k`: its content to its closing line, and the line after it. One
/// never closed runs to the prelude's `---`, where reading goes on.
fn fence(source: &str, lines: &[(usize, &str)], k: usize, ticks: usize, lang: &str, label: &str) -> Result<(Fenced, usize), Broken> {
    let start = lines.get(k + 1).map_or(source.len(), |l| l.0);
    let mut j = k + 1;
    while j < lines.len() {
        let c = content(lines[j].1);
        if c.trim_end_matches(BLANK).len() == ticks && c.bytes().all(|b| b == b'`' || b == b' ' || b == b'\t') && c.starts_with('`') {
            let body = Fenced { lang: lang.to_owned(), label: label.to_owned(), line: k as u32, start, end: lines[j].0 };
            return Ok((body, j + 1));
        }
        j += 1;
    }
    Err(Broken { line: k as u32, why: format!("a fence of {ticks} backticks is never closed"), resume: closing(lines, k + 1) })
}

/// The prelude's closing `---` from line `k` on, or past the last line.
fn closing(lines: &[(usize, &str)], k: usize) -> usize {
    (k..lines.len()).find(|&j| content(lines[j].1) == "---").unwrap_or(lines.len())
}

/// A statement that does not read, and the line reading goes on from when it is not the
/// one after the statement's.
struct Broken {
    line: u32,
    why: String,
    resume: usize,
}

/// The statement at line `*k`, `k` moved past it: nothing for a blank line or a comment.
fn statement(source: &str, lines: &[(usize, &str)], k: &mut usize) -> Result<Option<Statement>, (u32, String)> {
    let n = *k as u32;
    let said = uncomment(content(lines[*k].1)).trim_matches(BLANK);
    *k += 1;
    if said.is_empty() {
        return Ok(None);
    }
    let fail = |why: String| Err((n, why));
    let fenced = |at: usize, ticks: usize, lang: &str, label: &str, k: &mut usize| match fence(source, lines, at, ticks, lang, label) {
        Ok((body, next)) => {
            *k = next;
            Ok(body)
        }
        Err(b) => {
            *k = b.resume;
            Err((b.line, b.why))
        }
    };
    // a chart of its own: its fence alone, unnamed
    if let Some((ticks, lang, label)) = opening(said) {
        let body = fenced(n as usize, ticks, lang, label, k)?;
        if lang != "chart" {
            return fail(format!("a fence alone is a chart, ```chart; ```{lang} is named: name = ```{lang}"));
        }
        return Ok(Some(Statement::Chart { line: n, name: None, body }));
    }
    let (exported, said) = match said.strip_prefix("export").filter(|r| r.starts_with(BLANK)) {
        Some(r) => (true, r.trim_start_matches(BLANK)),
        None => (false, said),
    };
    if let Some(rest) = said.strip_prefix("packed").filter(|r| r.starts_with(BLANK)) {
        if exported {
            return fail("a packed copy is of an import, the file's own: export names a type or a table".to_owned());
        }
        let rest = rest.trim_start_matches(BLANK);
        if let Some(t) = rest.strip_prefix("type").filter(|r| r.starts_with(BLANK)) {
            let t = t.trim_start_matches(BLANK);
            let (name, after) = ident(t).unwrap_or(("", t));
            let Some(spelled) = after.trim_start_matches(BLANK).strip_prefix('=').filter(|_| !name.is_empty()) else {
                return fail("a packed type is `packed type Name = <type>`".to_owned());
            };
            let copy = Copy::Type(spelled.trim_matches(BLANK).to_owned());
            return Ok(Some(Statement::Packed { line: n, name: name.to_owned(), copy }));
        }
        if let Some(l) = rest.strip_prefix("look").filter(|r| r.starts_with(BLANK)) {
            let (target, rules) = crate::look::statement(l.trim_start_matches(BLANK)).map_err(|why| (n, why))?;
            let crate::look::Target::Name(name) = target else {
                return fail("a packed look is a named type's: packed look Name: rule, rule".to_owned());
            };
            return Ok(Some(Statement::Packed { line: n, name, copy: Copy::Look(rules) }));
        }
        let fence = ident(rest).and_then(|(name, after)| {
            let value = after.trim_start_matches(BLANK).strip_prefix('=')?.trim_matches(BLANK);
            opening(value).map(|o| (name, o))
        });
        let Some((name, (ticks, lang, label))) = fence else {
            return fail("a packed copy is `packed name = ```cells`, `packed type Name = …` or `packed look Name: …`".to_owned());
        };
        let body = fenced(n as usize, ticks, lang, label, k)?;
        if lang != "cells" || !label.is_empty() {
            return fail(format!("a packed table is a ```cells fence, not ```{lang}"));
        }
        return Ok(Some(Statement::Packed { line: n, name: name.to_owned(), copy: Copy::Table(body) }));
    }
    if let Some(rest) = said.strip_prefix("type").filter(|r| r.starts_with(BLANK) && !r.trim_start_matches(BLANK).starts_with('=')) {
        let rest = rest.trim_start_matches(BLANK);
        let (name, after) = ident(rest).unwrap_or(("", rest));
        let Some(spelled) = after.trim_start_matches(BLANK).strip_prefix('=') else {
            return fail("a named type is `type Name = <type>`".to_owned());
        };
        return Ok(Some(Statement::Type { line: n, name: name.to_owned(), spelled: spelled.trim_matches(BLANK).to_owned(), exported }));
    }
    if let Some(rest) = said.strip_prefix("import").filter(|r| r.starts_with(BLANK) || r.starts_with('{')) {
        if exported {
            return fail("an import is the file's own; export what it names instead".to_owned());
        }
        return import(n, rest.trim_start_matches(BLANK)).map(Some);
    }
    // a statement by its first word, as type and import are: a column named `look …` is
    // declared quoted
    if let Some(rest) = said.strip_prefix("look").filter(|r| r.starts_with(BLANK)) {
        if exported {
            return fail("a look goes with the type it names: export the type".to_owned());
        }
        let (target, rules) = crate::look::statement(rest.trim_start_matches(BLANK)).map_err(|why| (n, why))?;
        return Ok(Some(Statement::Look { line: n, target, rules }));
    }
    match column(said) {
        Ok(Some(_)) if exported => return fail("export names a type or a table".to_owned()),
        Ok(Some((name, spelled))) => return Ok(Some(Statement::Column { line: n, name, spelled })),
        Ok(None) => {}
        Err(why) => return fail(why),
    }
    let Some((name, after)) = ident(said) else {
        return fail("is no statement: a type, an import, a column (name: type), or name = a table".to_owned());
    };
    let Some(value) = after.trim_start_matches(BLANK).strip_prefix('=') else {
        return fail("is no statement: a type, an import, a column (name: type), or name = a table".to_owned());
    };
    let value = value.trim_matches(BLANK);
    if let Some((ticks, lang, label)) = opening(value) {
        // The fence is read first, so a table that does not read is left out whole.
        let body = fenced(n as usize, ticks, lang, label, k)?;
        if lang == "chart" {
            if exported {
                return fail("a chart is the file's own: export names a type or a table".to_owned());
            }
            return Ok(Some(Statement::Chart { line: n, name: Some(name.to_owned()), body }));
        }
        if lang != "cells" || !label.is_empty() {
            return fail(format!("a table written inline is a ```cells fence, not ```{lang}"));
        }
        return Ok(Some(Statement::Table { line: n, name: name.to_owned(), exported, body }));
    }
    if let Some(rest) = value.strip_prefix("run").filter(|r| r.is_empty() || r.starts_with(BLANK)) {
        // The script's fence, then what the engine stored: blank lines may stand between.
        // Past the script's own fence only a ```cells fence is the script's; any other
        // (a chart) is the next statement.
        let mut blocks = Vec::new();
        loop {
            while lines.get(*k).is_some_and(|l| content(l.1).trim_matches(BLANK).is_empty()) {
                *k += 1;
            }
            let Some(&(_, l)) = lines.get(*k) else { break };
            let Some((ticks, lang, label)) = opening(content(l).trim_matches(BLANK)) else { break };
            if !blocks.is_empty() && lang != "cells" {
                break;
            }
            blocks.push(fenced(*k, ticks, lang, label, k)?);
        }
        let schedule = schedule(rest.trim_matches(BLANK)).map_err(|why| (n, why))?;
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
        return Ok(Some(Statement::Run { line: n, name: name.to_owned(), exported, schedule, code, stored, runs }));
    }
    if exported {
        return fail("export names a type or a table".to_owned());
    }
    fail(format!("{name} = … names a table (```cells) or a script (run): a column is declared with its type, {name}: type = formula"))
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
        let Some((n, rest)) = ident(s) else { return fail("import takes a name: import accounts from \"../accounts.cells\"") };
        default = Some(n.to_owned());
        s = rest.trim_start_matches(BLANK);
    }
    let Some(rest) = s.strip_prefix("from").filter(|r| r.starts_with(BLANK)) else {
        return fail("an import says where from: from \"../accounts.cells\"");
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
            if !matches!(key, "sha256" | "rev" | "pack") {
                return fail("an import takes sha256, rev or pack");
            }
            if key == "pack" && value != "auto" {
                return fail("pack is \"auto\": the copy refreshed on every write (without it, cells pack writes copies)");
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
        return Err("run says when: every hour, every day at 18:00 in Europe/Madrid, when ../money.cells changes".to_owned());
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
                    "\"{}\" is no schedule: every 15m, every day at 18:00, cron 0 9 * * 1-5, when ../x.cells changes",
                    part.trim_matches(BLANK)
                ));
            }
        };
        triggers.push(t);
    }
    if zone.is_none() && triggers.iter().any(Trigger::clock) {
        return Err("a time of day is in a zone: in Europe/Madrid".to_owned());
    }
    Ok(Schedule { triggers, zone, spelled: s.to_owned() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements() {
        let src = "---\ntype Kind = a|b // the kinds\nimport accounts from \"../accounts.cells\"\nimport { Rate } from \"https://x.example/r.cells\" with { sha256: \"ab\" }\n\nfees = ```cells\nkind:text!\n---\n```\nfx = run every day at 18:00 in Europe/Madrid\n```ts\nexport default () => []\n```\n```cells runs\nrun:int!\n```\n---\nh\n";
        let Parsed { statements: s, spans, broken, body } = parse(src, 0).unwrap();
        assert!(broken.is_empty());
        assert_eq!(spans, [1..2, 2..3, 3..4, 5..9, 9..16]);
        assert_eq!(&src[body..], "h\n");
        assert_eq!(s.len(), 5);
        assert!(matches!(&s[1], Statement::Import { default: Some(d), .. } if d == "accounts"));
        assert!(matches!(&s[2], Statement::Import { names, with, .. } if names[0] == "Rate" && with[0].1 == "ab"));
        let Statement::Table { body, .. } = &s[3] else { panic!() };
        assert_eq!(&src[body.start..body.end], "kind:text!\n---\n");
        let Statement::Run { schedule: when, stored, runs, .. } = &s[4] else { panic!() };
        assert_eq!(when.triggers, [Trigger::Daily(18 * 60)]);
        assert!(stored.is_none() && runs.is_some());
        let lines = |src: &str| parse(src, 0).unwrap().broken.iter().map(|b| b.0).collect::<Vec<_>>();
        assert_eq!(lines("---\nnonsense\ntype A = a|b\n---\n"), [1]);
        assert_eq!(parse("---\nnonsense\ntype A = a|b\n---\n", 0).unwrap().statements.len(), 1);
        assert_eq!(lines("---\n\ntotal = cost * qty\n---\n"), [2]);
        // A fence never closed runs to the `---`, which still closes the prelude.
        let open = parse("---\nx = ```cells\na\n---\nh\n", 0).unwrap();
        assert_eq!((open.broken[0].0, &"---\nx = ```cells\na\n---\nh\n"[open.body..]), (1, "h\n"));
        assert!(parse("---\ntype A = a|b\n", 0).is_err());
        assert!(schedule("every day at 18:00").is_err());
        assert_eq!(schedule("every 15m, when ../m.cells changes").unwrap().triggers.len(), 2);
    }

    #[test]
    fn written() {
        // Each line a writer writes reads back as the statement it says.
        let lines = [
            type_line("Money", "EUR>=0", true),
            import_line(Some("accounts"), &[], "../a \"b\".cells", &[]),
            import_line(
                None,
                &["Rate".to_owned(), "fx".to_owned()],
                "https://x.example/r.cells",
                &[("sha256".to_owned(), "ab".to_owned())],
            ),
            column_line("total", "EUR=cost * qty"),
            column_line("a:b", "int"),
        ];
        assert_eq!(run_line("fx", "every hour", true), "export fx = run every hour");
        assert_eq!(lines[0], "export type Money = EUR>=0");
        assert_eq!(lines[3], "total: EUR = cost * qty");
        assert_eq!(lines[4], "\"a:b\": int");
        let src = format!("---\n{}\n---\nh\n", lines.join("\n"));
        let p = parse(&src, 0).unwrap();
        assert!(p.broken.is_empty(), "{:?}", p.broken);
        assert!(
            matches!(&p.statements[0], Statement::Type { name, spelled, exported: true, .. } if name == "Money" && spelled == "EUR>=0")
        );
        assert!(matches!(&p.statements[1], Statement::Import { from, .. } if from == "../a \"b\".cells"));
        assert!(matches!(&p.statements[2], Statement::Import { names, with, .. } if names.len() == 2 && with[0].1 == "ab"));
        assert!(matches!(&p.statements[3], Statement::Column { name, spelled, .. } if name == "total" && spelled == "EUR=cost * qty"));
        assert!(matches!(&p.statements[4], Statement::Column { name, .. } if name == "a:b"));
    }

    #[test]
    fn looks() {
        use crate::look::Target;
        let lines = [
            look_line(&Target::Name("amount".into()), &["grouped".into(), "negative red".into()]),
            look_line(&Target::Row, &["gray when paid".into()]),
            look_line(&Target::Name("row".into()), &["grouped".into()]),
            column_line("look ahead", "text"),
        ];
        assert_eq!(lines[0], "look amount: grouped, negative red");
        assert_eq!(lines[2], "look \"row\": grouped");
        assert_eq!(lines[3], "\"look ahead\": text");
        let src = format!("---\n{}\n---\nh\n", lines.join("\n"));
        let p = parse(&src, 0).unwrap();
        assert!(p.broken.is_empty(), "{:?}", p.broken);
        assert!(matches!(&p.statements[0], Statement::Look { target: Target::Name(n), rules, .. } if n == "amount" && rules.len() == 2));
        assert!(matches!(&p.statements[1], Statement::Look { target: Target::Row, .. }));
        assert!(matches!(&p.statements[2], Statement::Look { target: Target::Name(n), .. } if n == "row"));
        assert!(matches!(&p.statements[3], Statement::Column { name, .. } if name == "look ahead"));
        // `look ahead: text` unquoted is a look of the column `ahead` whose rule is no rule
        // the doc knows: the statement reads, its rule breaks where the doc settles it
        assert!(matches!(&parse("---\nlook ahead: text\n---\nh\n", 0).unwrap().statements[0], Statement::Look { .. }));
        assert_eq!(parse("---\nexport look x: grouped\n---\nh\n", 0).unwrap().broken.len(), 1);
    }
}
