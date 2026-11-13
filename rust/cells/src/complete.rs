//! What a place in a file's text is (a header cell's name or type, a formula, a cell, the
//! prelude), and what may be written there: the completions every client offers, the
//! language server and the apps alike, from the doc and the tables it names.
//!
//! ```
//! let text = "item:text!,cost:EUR>=0,total:EUR=co\nfuel,84.20,\n";
//! let doc = cells::read(text).unwrap();
//! let at = text.find("=co").unwrap() + 3;
//! // the tables it names, reached by the host: none here
//! let found = cells::complete::complete(&doc, text, at, |_: &[&str]| None::<std::sync::Arc<cells::Doc>>);
//! assert!(found.iter().any(|c| c.label == "cost" && c.from as usize == at - 2));
//! ```

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Deref;

use crate::expr::{self, Named};
use crate::look;
use crate::{Base, Column, Doc, Place, Span, Value};

fn span(start: u32, len: u32) -> Span {
    Span { start, len }
}

// ---- where a place is ----

/// A formula, and where its text is in the file.
pub struct Formula<'a> {
    pub src: &'a str,
    pub base: u32,
}

/// What a place in a file is, in the terms a feature needs.
pub enum Spot<'a> {
    Prelude,
    /// A header cell's name (a declared column's, its declaration's).
    Name(usize, Span),
    /// A header cell's type.
    Type(usize, Span),
    Formula(Formula<'a>),
    Cell(usize, usize, Span),
    Nothing,
}

/// A header cell's parts in the file: its name, its type, its formula.
pub struct HeaderCell<'a> {
    pub name: Span,
    pub ty: Option<Span>,
    pub formula: Option<(&'a str, u32)>,
}

/// A header cell's parts; a declared column's type and formula are its declaration's, in
/// the prelude (SPEC.md, Columns).
pub fn header_cell<'a>(doc: &Doc, text: &'a str, column: usize) -> HeaderCell<'a> {
    let s = doc.header_span(column);
    let raw = &text[s.start as usize..(s.start + s.len) as usize];
    let (name, spelled) = crate::split_header(raw);
    let name = span(s.start, name.len() as u32);
    if let Some((_, declared)) = doc.declaration_span(column) {
        let (ty, formula) = typed(text, declared);
        return HeaderCell { name, ty: Some(ty), formula };
    }
    let Some(spelled) = spelled else { return HeaderCell { name, ty: None, formula: None } };
    let at = |part: &str| s.start + (part.as_ptr() as usize - raw.as_ptr() as usize) as u32;
    let (ty, formula) = crate::split_formula(spelled);
    HeaderCell { name, ty: Some(span(at(ty), ty.len() as u32)), formula: formula.map(|f| (f, at(f))) }
}

/// A type and its formula at `s` in the text (what follows a header cell's or a
/// declaration's `:`): the type's span, and the formula's text and where it starts.
fn typed(text: &str, s: Span) -> (Span, Option<(&str, u32)>) {
    let raw = &text[s.start as usize..(s.start + s.len) as usize];
    let at = |part: &str| s.start + (part.as_ptr() as usize - raw.as_ptr() as usize) as u32;
    let (ty, formula) = crate::split_formula(raw);
    let ty = ty.trim_end_matches([' ', '\t']);
    let formula = formula.map(|f| f.trim_start_matches([' ', '\t']));
    (span(at(ty), ty.len() as u32), formula.map(|f| (f, at(f))))
}

/// A cell's own formula, right of its `=` (`32.25=96.75 / 3`): its text and where it
/// starts. Nothing for a cell its column's formula computes (`84.20=`) or a written one.
pub fn cell_formula<'a>(doc: &Doc, text: &'a str, row: usize, column: usize) -> Option<(&'a str, u32)> {
    let s = doc.cell_span(row, column);
    let raw = &text[s.start as usize..(s.start + s.len) as usize];
    let at = crate::split_cell(raw)? + 1;
    (!raw[at..].trim().is_empty()).then(|| (&raw[at..], s.start + at as u32))
}

/// What is at byte `at` of `text`, the text `doc` was read from.
pub fn spot<'a>(doc: &Doc, text: &'a str, at: usize) -> Spot<'a> {
    let at = at as u32;
    match doc.place(at) {
        Place::Prelude => {
            // A column's declaration is that column's: its name, its type, its formula.
            for c in 0..doc.columns().len() {
                let Some((name, declared)) = doc.declaration_span(c) else { continue };
                if at >= name.start && at <= name.start + name.len {
                    return Spot::Name(c, name);
                }
                if at >= declared.start && at <= declared.start + declared.len {
                    return match typed(text, declared) {
                        (_, Some((src, base))) if at >= base => Spot::Formula(Formula { src, base }),
                        (ty, _) => Spot::Type(c, ty),
                    };
                }
            }
            Spot::Prelude
        }
        Place::Header(c) => {
            let c = c as usize;
            let h = header_cell(doc, text, c);
            // A declared column's header cell is its name alone; its type is in the prelude.
            if doc.declaration_span(c).is_some() {
                return Spot::Name(c, h.name);
            }
            match (h.ty, h.formula) {
                (_, Some((src, base))) if at >= base => Spot::Formula(Formula { src, base }),
                (Some(ty), _) if at >= ty.start => Spot::Type(c, ty),
                _ => Spot::Name(c, h.name),
            }
        }
        Place::Cell { row, column } => {
            let (row, column) = (row as usize, column as usize);
            match cell_formula(doc, text, row, column) {
                Some((src, base)) if at >= base && at <= base + src.len() as u32 => Spot::Formula(Formula { src, base }),
                _ => Spot::Cell(row, column, doc.cell_span(row, column)),
            }
        }
        Place::Unread | Place::Outside => Spot::Nothing,
    }
}

// ---- a row said ----

/// A cell's value as a reader is told it: its form, or `blank`.
pub fn shown(c: &Column, row: usize) -> String {
    match c.value(row) {
        Value::Null => "blank".to_owned(),
        v => c.form(v),
    }
}

/// A row in a line: its first columns' values.
pub fn summary(doc: &Doc, row: usize) -> String {
    let cells: Vec<String> = doc.columns().iter().take(6).map(|c| format!("{} {}", c.name, shown(c, row))).collect();
    cells.join(" · ")
}

// ---- completion ----

/// What a completion is, as an editor marks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionKind {
    /// A row's key, a yes or a no.
    Value,
    /// One of a choice's words.
    EnumMember,
    /// A named type.
    Class,
    /// A table's key column, `accounts.account`.
    Reference,
    /// The language's or the prelude's word, a base type.
    Keyword,
    /// A column.
    Field,
    /// A table.
    Module,
    Function,
    /// A tint.
    Color,
}

/// What may be written at a place: `insert` in place of the bytes `from..to` of the text
/// (the word being typed), shown as `label` with its `detail`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub insert: String,
    pub from: u32,
    pub to: u32,
    /// What it is matched by, where that is not its label (`#3 card`).
    pub filter: Option<String>,
    /// Where it sorts, where that is not by its label.
    pub sort: Option<String>,
}

fn item(label: String, kind: CompletionKind, detail: String, from: u32, to: u32) -> Completion {
    Completion { insert: label.clone(), label, kind, detail, from, to, filter: None, sort: None }
}

pub(crate) fn ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// A name as a formula writes it: bare when it is a word with a lower-case letter, quoted
/// otherwise, so it is never the language's (a word in capitals).
pub fn written(name: &str) -> String {
    let plain = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(ident)
        && name.chars().any(|c| c.is_ascii_lowercase());
    if plain { name.to_owned() } else { format!("\"{}\"", name.replace('"', "\"\"")) }
}

/// A key as a formula writes it: text in quotes, a `'` inside doubled.
pub fn written_key(key: &str) -> String {
    format!("'{}'", key.replace('\'', "''"))
}

/// A table reached: the file's own doc, or one its host holds.
pub(crate) enum Got<'a, D> {
    Main(&'a Doc),
    Held(D),
}

impl<D: Deref<Target = Doc>> Deref for Got<'_, D> {
    type Target = Doc;
    fn deref(&self) -> &Doc {
        match self {
            Got::Main(d) => d,
            Got::Held(d) => d,
        }
    }
}

/// The tables a place reaches, through the host: `held(path)` is the table reached by the
/// names in `path`, the first a table `doc`'s prelude names and each after it one the
/// table before names (a reference followed through two files). `this` is `doc` itself.
pub(crate) struct Reach<'a, H> {
    pub(crate) doc: &'a Doc,
    pub(crate) held: H,
}

impl<'a, D: Deref<Target = Doc>, H: Fn(&[&str]) -> Option<D>> Reach<'a, H> {
    pub(crate) fn get(&self, path: &[&str]) -> Option<Got<'a, D>> {
        match path {
            [first] if *first == expr::THIS => Some(Got::Main(self.doc)),
            _ => (self.held)(path).map(Got::Held),
        }
    }
}

/// What a part of a name in a formula is, and the path of tables that reaches it.
pub(crate) enum Followed<'a, D> {
    /// A column: of the file itself (no path), or of a table.
    Column(Option<(Vec<String>, Got<'a, D>)>, usize),
    Table(Vec<String>, Got<'a, D>),
}

/// Part `k` of a name, followed: its first part a column of the file (or a table, `table`
/// or not), every part after it a column of the table the one before leads to.
pub(crate) fn follow<'a, D: Deref<Target = Doc>, H: Fn(&[&str]) -> Option<D>>(
    reach: &Reach<'a, H>,
    n: &Named,
    k: usize,
) -> Option<Followed<'a, D>> {
    let first = &n.parts[0].0;
    let mut at = match (n.table, reach.doc.column(first)) {
        (false, Some(c)) => Followed::Column(None, c),
        _ => Followed::Table(vec![first.clone()], reach.get(&[first.as_str()])?),
    };
    for (name, _) in n.parts.iter().take(k + 1).skip(1) {
        at = match at {
            Followed::Table(path, d) => {
                let c = d.column(name)?;
                Followed::Column(Some((path, d)), c)
            }
            Followed::Column(held, c) => {
                // A reference leads to its first table's row.
                let doc: &Doc = held.as_ref().map_or(reach.doc, |(_, d)| d);
                let Base::Ref(targets) = &doc.columns()[c].ty.base else { return None };
                let table = targets[0].table.clone();
                let mut path = held.map_or(Vec::new(), |(p, _)| p);
                path.push(table);
                let names: Vec<&str> = path.iter().map(String::as_str).collect();
                let next = reach.get(&names)?;
                let c = next.column(name)?;
                Followed::Column(Some((path, next)), c)
            }
        };
    }
    Some(at)
}

/// What may be written at byte `at` of `text`, the text `doc` was read from: a cell's
/// values (a reference's keys, a choice's words, a boolean's), a type's names and bases, a
/// formula's columns, tables, rows, functions and words, the prelude's statements. The
/// tables it names are the host's: `held` reaches them by name (see `Reach`), nothing for
/// one it cannot.
pub fn complete<D, H>(doc: &Doc, text: &str, at: usize, held: H) -> Vec<Completion>
where
    D: Deref<Target = Doc>,
    H: Fn(&[&str]) -> Option<D>,
{
    assert!(at <= text.len() && text.is_char_boundary(at), "completion at {at}, not a place in the text");
    let reach = Reach { doc, held };
    match spot(doc, text, at) {
        Spot::Cell(_, c, s) => {
            let (from, to) = (s.start, s.start + s.len);
            let col = &doc.columns()[c];
            match &col.ty.base {
                Base::Ref(targets) => {
                    let mut out = Vec::new();
                    for t in targets {
                        let Some(d) = reach.get(&[t.table.as_str()]) else { continue };
                        let Some(k) = d.column(&t.column) else { continue };
                        for row in 0..d.rows() {
                            let key = d.text(row, k).into_owned();
                            if key.is_empty() {
                                continue;
                            }
                            let mut i = item(key, CompletionKind::Value, format!("{}: {}", t.table, summary(&d, row)), from, to);
                            i.insert = crate::quote(&i.insert).into_owned();
                            out.push(i);
                        }
                    }
                    out
                }
                Base::Choice(words) => {
                    words.iter().map(|w| item(w.clone(), CompletionKind::EnumMember, col.name.clone(), from, to)).collect()
                }
                Base::Bool { yes, no } => {
                    [yes, no].iter().map(|w| item((*w).clone(), CompletionKind::Value, col.name.clone(), from, to)).collect()
                }
                _ => Vec::new(),
            }
        }
        Spot::Type(_, s) => {
            // The word at the place, `.` included, so `accounts.a` is one.
            let word = &text[s.start as usize..(s.start + s.len) as usize];
            let here = at - s.start as usize;
            let from = word[..here].rfind(|c: char| !(ident(c) || c == '.')).map_or(0, |i| i + 1);
            let to = here + word[here..].find(|c: char| !(ident(c) || c == '.')).unwrap_or(word.len() - here);
            let named = from == 0 || word[..from].ends_with([' ', '\t', '|', '<', '(']);
            let (from, to) = (s.start + from as u32, s.start + to as u32);
            if tinting(&text[..from as usize]) {
                tints(from, to)
            } else if named {
                types(doc, from, to)
            } else {
                // after a mark (`text?`, `id!`), a bound or a quote no name begins
                Vec::new()
            }
        }
        Spot::Formula(f) => formula(&reach, &f, at),
        Spot::Prelude => prelude(&reach, text, at),
        Spot::Name(..) | Spot::Nothing => Vec::new(),
    }
}

/// Whether what is typed next is a choice's tint: after its `#`, or its `tint`.
fn tinting(before: &str) -> bool {
    before.ends_with('#') || before.trim_end_matches([' ', '\t']).strip_suffix("tint").is_some_and(|b| b.ends_with([' ', '\t']))
}

/// The ten tints, each said: a choice's after its `#`, a look rule's.
fn tints(from: u32, to: u32) -> Vec<Completion> {
    look::TINTS.iter().map(|t| item((*t).to_owned(), CompletionKind::Color, "a tint, drawn in each theme's own".into(), from, to)).collect()
}

/// A type's names: the file's named types, its tables' keys, the bases.
fn types(doc: &Doc, from: u32, to: u32) -> Vec<Completion> {
    let mut out: Vec<Completion> =
        doc.named().iter().map(|t| item(t.name.clone(), CompletionKind::Class, t.ty.shorthand(), from, to)).collect();
    for t in doc.tables() {
        let Some(d) = &t.doc else { continue };
        if let Some(key) = d.columns().iter().find(|c| c.ty.key) {
            out.push(item(format!("{}.{}", t.name, key.name), CompletionKind::Reference, format!("a key of {}", t.name), from, to));
        }
    }
    for b in ["text", "int", "num", "pct", "bool", "date", "time", "datetime", "url", "email", "id"] {
        out.push(item(b.to_owned(), CompletionKind::Keyword, "a base".into(), from, to));
    }
    out
}

fn formula<D: Deref<Target = Doc>, H: Fn(&[&str]) -> Option<D>>(reach: &Reach<'_, H>, f: &Formula, at: usize) -> Vec<Completion> {
    let doc = reach.doc;
    let here = at - f.base as usize;
    let src = &f.src[..here];
    let start = src.rfind(|c: char| !ident(c)).map_or(0, |i| i + 1);
    let (from, to) = (f.base + start as u32, f.base + here as u32);
    let columns = |d: &Doc, detail: &str| -> Vec<Completion> {
        d.columns()
            .iter()
            .map(|c| item(written(&c.name), CompletionKind::Field, format!("{detail}{}", c.ty.shorthand()), from, to))
            .collect()
    };
    // After a column's `[`: the rows by where they are from this one.
    if let Some((name, open)) = opened(src)
        && name != "TABLE"
        && doc.column(name).is_some()
    {
        let from = f.base + open as u32;
        let places = [
            ("-1", "the row above"),
            ("+1", "the row below"),
            ("FIRST", "the first row"),
            ("LAST", "the last row"),
            ("FIRST:THIS", "the rows from the first to this one, an aggregate's"),
            ("THIS:LAST", "the rows from this one to the last, an aggregate's"),
        ];
        return places.iter().map(|(p, d)| item((*p).into(), CompletionKind::Keyword, (*d).into(), from, to)).collect();
    }
    // Inside `TABLE[`, `fees[`: the table's rows, each by its key, found by its number too.
    if let Some((name, open)) = opened(src) {
        let Some(d) = reach.get(&[name]) else { return Vec::new() };
        let Some(k) = d.columns().iter().position(|c| c.ty.key) else { return Vec::new() };
        let from = f.base + open as u32;
        return (0..d.rows())
            .filter_map(|r| {
                let key = d.text(r, k);
                (!key.is_empty()).then(|| Completion {
                    filter: Some(format!("#{} {key}", r + 1)),
                    sort: Some(format!("{r:010}")),
                    ..item(written_key(&key), CompletionKind::Value, format!("#{} {}", r + 1, summary(&d, r)), from, to)
                })
            })
            .collect();
    }
    let before = src[..start].trim_end_matches([' ', '\t']);
    if let Some(path) = before.strip_suffix('.') {
        // `table['k'].`: the table's columns.
        if let Some(open) = path.strip_suffix(']').and_then(|p| p.rfind('[').map(|i| &p[..i])) {
            let name = &open[open.rfind(|c: char| !ident(c)).map_or(0, |i| i + 1)..];
            return reach.get(&[name]).map_or(Vec::new(), |d| columns(&d, &format!("{name}: ")));
        }
        // `a.b.`: what the path leads to.
        let begin = path.rfind(|c: char| !(ident(c) || c == '.')).map_or(0, |i| i + 1);
        let parts: Vec<(String, (u32, u32))> = path[begin..].split('.').map(|p| (p.to_owned(), (0, 0))).collect();
        let n = Named { parts, table: false, key: None };
        return match follow(reach, &n, n.parts.len() - 1) {
            Some(Followed::Table(_, d)) => columns(&d, ""),
            Some(Followed::Column(held, c)) => {
                let d: &Doc = held.as_ref().map_or(doc, |(_, d)| d);
                let Base::Ref(targets) = &d.columns()[c].ty.base else { return Vec::new() };
                let mut path = held.as_ref().map_or(Vec::new(), |(p, _)| p.clone());
                path.push(targets[0].table.clone());
                let names: Vec<&str> = path.iter().map(String::as_str).collect();
                reach.get(&names).map_or(Vec::new(), |t| columns(&t, &format!("{}: ", targets[0].table)))
            }
            None => Vec::new(),
        };
    }
    let mut out = columns(doc, "");
    out.push(item("TABLE".to_owned(), CompletionKind::Module, "this file's own table".into(), from, to));
    for t in doc.tables() {
        out.push(item(written(&t.name), CompletionKind::Module, "a table".into(), from, to));
    }
    for name in expr::FUNCTIONS {
        let (call, gives) = expr::signature(name).expect("every function has its signature");
        out.push(item(name.to_owned(), CompletionKind::Function, format!("{call}: {gives}"), from, to));
    }
    for word in ["CASE", "WHEN", "THEN", "ELSE", "END", "AND", "OR", "NOT", "IS", "NULL", "TRUE", "FALSE", "OVER", "ORDER", "BY", "DESC"] {
        out.push(item(word.to_owned(), CompletionKind::Keyword, "the language's".into(), from, to));
    }
    out
}

/// The table a formula names before an `[` still open at its end, and where what
/// follows the `[` starts: `this[`, `fees['ca`.
pub fn opened(src: &str) -> Option<(&str, usize)> {
    let open = src.rfind('[')?;
    if src[open..].contains(']') {
        return None;
    }
    let before = src[..open].trim_end_matches([' ', '\t']);
    let name = &before[before.rfind(|c: char| !ident(c)).map_or(0, |i| i + 1)..];
    // The language's `TABLE` is the file's own table, as the engine names it.
    let name = if name == "TABLE" { expr::THIS } else { name };
    (!name.is_empty()).then_some((name, open + 1))
}

/// In the prelude, on a line not yet read as a column's declaration: what starts a
/// statement, a type after `type Name =` or a declaration's `:`, a script's schedule, a
/// declaration's formula. Nothing inside a fence (an inline table, a script's code: its
/// language's), nor where an import names its file (the host's paths).
fn prelude<D: Deref<Target = Doc>, H: Fn(&[&str]) -> Option<D>>(reach: &Reach<'_, H>, text: &str, at: usize) -> Vec<Completion> {
    let doc = reach.doc;
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    // Inside a fence: an odd number of fence lines between the prelude's `---` and here.
    let fences = text[..line_start].lines().filter(|l| l.trim_start().starts_with("```")).count();
    if fences % 2 == 1 {
        return Vec::new();
    }
    let line = &text[line_start..at];
    let start = line.rfind(|c: char| !ident(c)).map_or(0, |i| i + 1);
    let (from, to) = ((line_start + start) as u32, at as u32);
    let before = line[..start].trim();
    let word = |w: &str, detail: &str| item(w.to_owned(), CompletionKind::Keyword, detail.to_owned(), from, to);
    if before.is_empty() {
        // A statement's first word: the prelude's, and every column not declared yet.
        let mut out = vec![
            word("import", "a table from another file: import accounts from \"./accounts.cells\""),
            word("export", "what another file may import"),
            word("type", "a type with a name: type Money = EUR>=0"),
            word("look", "how values show: look amount: grouped, negative red"),
        ];
        for (c, col) in doc.columns().iter().enumerate() {
            if doc.declaration_span(c).is_none() {
                out.push(item(written(&col.name), CompletionKind::Field, "declare its type: name: type = formula".into(), from, to));
            }
        }
        return out;
    }
    if before == "export" {
        return vec![word("type", "a type another file may import")];
    }
    if before.starts_with("import") {
        return Vec::new();
    }
    if tinting(&text[..from as usize]) {
        return tints(from, to);
    }
    if before == "look" {
        // What it is for: a column, a named type, the row.
        let mut out: Vec<Completion> = doc
            .columns()
            .iter()
            .map(|c| item(crate::prelude::declared_name(&c.name).into_owned(), CompletionKind::Field, c.ty.shorthand(), from, to))
            .collect();
        out.extend(doc.named().iter().map(|t| item(t.name.clone(), CompletionKind::Class, t.ty.shorthand(), from, to)));
        out.push(word("row", "every row, tinted by a condition: look row: gray when paid"));
        return out;
    }
    if let Some((_, _, rules)) = look::spans(line) {
        let rule = &line[rules.last().expect("one rule at least").start..start];
        let said: Vec<&str> = rule.split_whitespace().collect();
        let rule_word = |w: &str| word(w, look::RULE_WORDS.iter().find(|(x, _)| *x == w).expect("a rule's word").1);
        return match said.as_slice() {
            [] => {
                let mut out: Vec<Completion> = look::RULE_WORDS.iter().filter(|(w, _)| *w != "when").map(|(w, _)| rule_word(w)).collect();
                out.extend(tints(from, to));
                out
            }
            ["negative"] => {
                let mut out = vec![word("in", "negative in parentheses: (950.00)")];
                out.extend(tints(from, to));
                out
            }
            ["negative", "in"] => vec![word("parentheses", "(950.00), as accounts write a negative")],
            ["positive"] | ["scale"] => tints(from, to),
            ["scale", t] if t.ends_with("..") => tints(from, to),
            [t] if look::Tint::parse(t).is_ok() => vec![rule_word("when")],
            [_, "when", ..] => {
                let begin = line_start + rules.last().expect("one rule").start + rule.find("when").expect("its when") + 4;
                let src = &text[begin..at];
                let skipped = src.len() - src.trim_start().len();
                formula(reach, &Formula { src: &src[skipped..], base: (begin + skipped) as u32 }, at)
            }
            _ => Vec::new(),
        };
    }
    let words: Vec<&str> = before.split_whitespace().collect();
    let scheduling = words.iter().position(|w| *w == "run").is_some_and(|r| r >= 2 && words[r - 1] == "=");
    // `rates = run e`, and after a comma, another trigger
    let trigger = scheduling && words.last().is_some_and(|w| *w == "run" || w.ends_with(','));
    match words.as_slice() {
        // `type Money = E`, `export type Money = E`
        ["type", _, "="] | ["export", "type", _, "="] => types(doc, from, to),
        // `rates = r`
        [_, "="] | ["export", _, "="] => vec![word("run", "a table a script fills: rates = run every day at 18:00")],
        _ if trigger => vec![
            word("every", "every hour, every day at 18:00, every monday at 09:00"),
            word("cron", "cron 0 9 * * 1-5"),
            word("when", "when ../money.cells changes"),
        ],
        [.., "every"] if scheduling => {
            ["15m", "hour", "day", "weekday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday", "month"]
                .iter()
                .map(|w| word(w, "how often it runs"))
                .collect()
        }
        _ if scheduling => Vec::new(),
        _ => {
            // `vat: E` or `vat: EUR = am`: a declaration the doc has not read yet.
            let Some(colon) = before.find(':') else { return Vec::new() };
            let after = &before[colon + 1..];
            if after.contains('=') {
                let base = (line_start + line.find(':').expect("the colon is on the line") + 1) as u32;
                let eq = text[base as usize..at].find('=').expect("the `=` is before the place") + 1;
                let src = &text[base as usize + eq..at];
                let skipped = src.len() - src.trim_start().len();
                let f = Formula { src: &src[skipped..], base: base + (eq + skipped) as u32 };
                formula(reach, &f, at)
            } else {
                types(doc, from, to)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;

    fn labels(text: &str, at: usize) -> Vec<String> {
        let doc = crate::read(text).unwrap();
        complete(&doc, text, at, |_: &[&str]| None::<Arc<Doc>>).into_iter().map(|c| c.label).collect()
    }

    /// A reference followed through an imported file into the file it imports: the
    /// tables a read holds are read with their own imports.
    #[cfg(feature = "std")]
    #[test]
    fn imports_of_imports() {
        use alloc::collections::BTreeMap;
        let mut files = BTreeMap::new();
        files.insert("kinds.cells".to_owned(), "kind:text!,side:text\nassets,debit\n".to_owned());
        files.insert(
            "accounts.cells".to_owned(),
            "---\nimport kinds from \"./kinds.cells\"\n---\naccount:text!,kind:kinds.kind\nbank,assets\n".to_owned(),
        );
        let text = "---\nimport accounts from \"./accounts.cells\"\n---\nto:accounts.account,s:text=to.kind.\nbank,x=\n";
        let doc = crate::read_with(text, &mut crate::files::Files::new(&files, "money.cells"), 1).unwrap();
        let held = |path: &[&str]| {
            let (first, rest) = path.split_first()?;
            let mut t = doc.tables().iter().find(|t| t.name == *first)?.doc.clone()?;
            for name in rest {
                let next = t.tables().iter().find(|x| x.name == *name)?.doc.clone()?;
                t = next;
            }
            Some(t)
        };
        let at = text.find("to.kind.").unwrap() + "to.kind.".len();
        let found: Vec<String> = complete(&doc, text, at, held).into_iter().map(|c| c.label).collect();
        assert_eq!(found, ["kind", "side"], "the columns of the table two imports away");
    }

    #[test]
    fn statements() {
        let text = "---\nty\n---\nitem:text!,cost:EUR\nfuel,1.00\n";
        let at = text.find("ty\n").unwrap() + 2;
        let found = labels(text, at);
        for w in ["import", "export", "type", "item", "cost"] {
            assert!(found.iter().any(|l| l == w), "{w} in {found:?}");
        }
        let doc = crate::read(text).unwrap();
        let first = &complete(&doc, text, at, |_: &[&str]| None::<Arc<Doc>>)[0];
        assert_eq!((first.from as usize, first.to as usize), (at - 2, at));
    }

    #[test]
    fn declared_not_offered() {
        let text = "---\ncost: EUR\n\n---\nitem:text!,cost\nfuel,1.00\n";
        let at = text.find("\n\n").unwrap() + 1;
        let found = labels(text, at);
        assert!(found.iter().any(|l| l == "item"));
        assert!(!found.iter().any(|l| l == "cost"), "{found:?}");
    }

    #[test]
    fn named_types() {
        let text = "---\ntype Money = EUR>=0\ntype Price = Mo\n---\na:text\nx\n";
        let at = text.find("Mo\n").unwrap() + 2;
        let found = labels(text, at);
        assert!(found.iter().any(|l| l == "Money"), "{found:?}");
        assert!(found.iter().any(|l| l == "num"));
        assert!(!found.iter().any(|l| l == "import"));
    }

    #[test]
    fn schedules() {
        let text = "---\nrates = run ev\n---\na:text\nx\n";
        let at = text.find("ev\n").unwrap() + 2;
        assert!(labels(text, at).iter().any(|l| l == "every"));
        let text = "---\nrates = r\n---\na:text\nx\n";
        let at = text.find("= r").unwrap() + 3;
        assert_eq!(labels(text, at), ["run"]);
        let text = "---\nrates = run every d\n---\na:text\nx\n";
        let at = text.find("y d").unwrap() + 3;
        assert!(labels(text, at).iter().any(|l| l == "day"));
    }

    #[test]
    fn a_declaration_typed() {
        let text = "---\nprice: E\n---\nprice,cost:EUR\n1,2.00\n";
        let at = text.find(": E").unwrap() + 3;
        assert!(labels(text, at).iter().any(|l| l == "text"));
        // a mark ends the name: nothing after `text?`, a name again after `|`
        let text = "---\nnote: text?\n---\nnote\nx\n";
        let at = text.find("text?").unwrap() + 5;
        assert_eq!(labels(text, at), Vec::<String>::new());
        let text = "---\nwho: people.id|t\n---\nwho\nx\n";
        let at = text.find("|t").unwrap() + 2;
        assert!(labels(text, at).iter().any(|l| l == "text"));
        let text = "---\ntotal: EUR = co\n---\ncost:EUR,total\n2.00,2.00\n";
        let at = text.find("= co").unwrap() + 4;
        let found = labels(text, at);
        assert!(found.iter().any(|l| l == "cost"), "{found:?}");
    }

    #[test]
    fn tints_and_looks() {
        let has = |text: &str, after: &str, want: &[&str]| {
            let at = text.find(after).unwrap() + after.len();
            let found = labels(text, at);
            for w in want {
                assert!(found.iter().any(|l| l == w), "{w} after {after:?} in {found:?}");
            }
            found
        };
        // a choice's tint, in a header cell and in the prelude, after # and after tint
        has("kind:income#gr\nincome\n", "#gr", &["green", "gray"]);
        has("---\ntype Kind = income#|cost\n---\nkind:Kind\nincome\n", "income#", &["red", "teal"]);
        has("---\nmood: one of yay tint b\n---\nmood\nyay\n", "tint b", &["blue", "brown"]);
        // a look: what it is for, then each rule's words
        let text = "---\ntype Money = EUR\nlook \nlook amount: neg\nlook amount: negative \nlook amount: red \nlook amount: red when am\nlook amount: scale red..g\n---\namount:Money,paid:yes/no\n1.00,yes\n";
        let found = has(text, "look ", &["amount", "Money", "row"]);
        assert!(!found.iter().any(|l| l == "grouped"), "{found:?}");
        has(text, ": neg", &["negative", "grouped", "scale", "as", "red"]);
        has(text, "negative ", &["in", "red"]);
        has(text, ": red ", &["when"]);
        has(text, "when am", &["amount", "paid"]);
        has(text, "red..g", &["green"]);
        has("---\n\n---\na:int\n1\n", "---\n", &["look"]);
    }

    #[test]
    fn not_in_fences_nor_imports() {
        let text = "---\nrates = run every day\n```ts\nexpo\n```\n---\na:text\nx\n";
        let at = text.find("expo").unwrap() + 4;
        assert!(labels(text, at).is_empty());
        let text = "---\nimport accounts from \"./a\n---\na:text\nx\n";
        let at = text.find("./a").unwrap() + 3;
        assert!(labels(text, at).is_empty());
    }
}
