//! What the language server answers, each a function of the folder as the editor has it
//! and a place in one file: its diagnostics, a hover, completions, a definition, a rename
//! followed through every file that names what is renamed.

use std::collections::HashMap;
use std::path::Path;

use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, Diagnostic, DiagnosticSeverity, Hover, HoverContents, Location, MarkupContent,
    MarkupKind, Range, TextEdit, Url, WorkspaceEdit,
};

use cells::expr::{self, Named};
use cells::{Base, Column, Doc, Origin, Place, Span, Statement, Value};

use super::folder::{File, Folder, Held, inline, names_main};
use super::text::Lines;

pub fn uri(p: &Path) -> Url {
    Url::from_file_path(p).expect("the folder's paths are absolute")
}

fn span(start: u32, len: u32) -> Span {
    Span { start, len }
}

fn shifted(s: Span, by: u32) -> Span {
    span(s.start + by, s.len)
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
    /// A header cell's name.
    Name(usize, Span),
    /// A header cell's type.
    Type(usize, Span),
    Formula(Formula<'a>),
    Cell(usize, usize, Span),
    Nothing,
}

/// A header cell's parts in the file: its name, its type, its formula.
struct HeaderCell<'a> {
    name: Span,
    ty: Option<Span>,
    formula: Option<(&'a str, u32)>,
}

fn header_cell<'a>(file: &'a File, column: usize) -> HeaderCell<'a> {
    let s = file.doc.header_span(column);
    let raw = &file.text[s.start as usize..(s.start + s.len) as usize];
    let (name, spelled) = cells::split_header(raw);
    let name = span(s.start, name.len() as u32);
    let Some(spelled) = spelled else { return HeaderCell { name, ty: None, formula: None } };
    let at = |part: &str| s.start + (part.as_ptr() as usize - raw.as_ptr() as usize) as u32;
    let (ty, formula) = cells::split_formula(spelled);
    HeaderCell { name, ty: Some(span(at(ty), ty.len() as u32)), formula: formula.map(|f| (f, at(f))) }
}

/// A cell's own formula, `` `=…` ``: its text and where it starts.
fn cell_formula(file: &File, row: usize, column: usize) -> Option<(&str, u32)> {
    let s = file.doc.cell_span(row, column);
    let raw = &file.text[s.start as usize..(s.start + s.len) as usize];
    (raw.len() >= 3 && raw.starts_with("`=") && raw.ends_with('`')).then(|| (&raw[2..raw.len() - 1], s.start + 2))
}

pub fn spot(file: &File, at: usize) -> Spot<'_> {
    let at = at as u32;
    match file.doc.place(at) {
        Place::Prelude => Spot::Prelude,
        Place::Header(c) => {
            let c = c as usize;
            let h = header_cell(file, c);
            match (h.ty, h.formula) {
                (_, Some((src, base))) if at >= base => Spot::Formula(Formula { src, base }),
                (Some(ty), _) if at >= ty.start => Spot::Type(c, ty),
                _ => Spot::Name(c, h.name),
            }
        }
        Place::Cell { row, column } => {
            let (row, column) = (row as usize, column as usize);
            match cell_formula(file, row, column) {
                Some((src, base)) if at >= base && at <= base + src.len() as u32 => Spot::Formula(Formula { src, base }),
                _ => Spot::Cell(row, column, file.doc.cell_span(row, column)),
            }
        }
        Place::Unread | Place::Outside => Spot::Nothing,
    }
}

/// Every formula of a file, and where its text starts: its columns', then its cells'.
fn formulas(file: &File) -> Vec<(&str, u32)> {
    let doc = &file.doc;
    let mut out: Vec<(&str, u32)> = (0..doc.columns().len()).filter_map(|c| header_cell(file, c).formula).collect();
    for (c, col) in doc.columns().iter().enumerate() {
        for r in 0..col.origin.len() {
            if col.origin(r) == Origin::Cell
                && let Some(f) = cell_formula(file, r, c)
            {
                out.push(f);
            }
        }
    }
    out
}

// ---- diagnostics ----

fn error(range: Range, message: String) -> Diagnostic {
    Diagnostic { range, severity: Some(DiagnosticSeverity::ERROR), source: Some("cells".into()), message, ..Diagnostic::default() }
}

/// Every break and problem of a file, its own tables' included, where it is.
pub fn diagnostics(file: &File) -> Vec<Diagnostic> {
    let lines = Lines::new(&file.text);
    let doc = &file.doc;
    let mut out: Vec<Diagnostic> = doc
        .broken()
        .iter()
        .zip(doc.broken_spans())
        .map(|(e, s)| error(s.map_or(Range::default(), |s| lines.range(s)), e.why.clone()))
        .collect();
    let problems = |doc: &Doc, base: u32, table: Option<&str>, out: &mut Vec<Diagnostic>| {
        for (p, s) in doc.problems().iter().zip(doc.spans()) {
            let column = &doc.columns()[p.column as usize].name;
            let place = match table {
                Some(t) => format!("{t}, row {}, {column}", p.row),
                None => column.clone(),
            };
            out.push(error(lines.range(shifted(s, base)), format!("{place}: {}", doc.why(p))));
        }
    };
    problems(doc, 0, None, &mut out);
    for t in doc.tables().iter().filter(|t| t.own) {
        if let Some((d, base)) = inline(file, &t.name) {
            problems(&d, base, Some(&t.name), &mut out);
        }
    }
    out
}

// ---- hover ----

/// A column in words: its header cell, what it holds, what computes it, what it names.
fn about(c: &Column) -> String {
    let mut out = format!("`{}:{}`", c.name, c.ty.shorthand());
    if let Some(f) = &c.formula {
        out.push_str(&format!(" `={f}`"));
    }
    let mut said = Vec::new();
    if c.ty.inferred {
        said.push("its type inferred from its values".to_owned());
    }
    if c.ty.key {
        said.push("the key: in every row, once".to_owned());
    }
    if let Base::Ref(targets) = &c.ty.base {
        let named: Vec<String> = targets.iter().map(|t| format!("`{}.{}`", t.table, t.column)).collect();
        said.push(format!("a key of {}", named.join(" or ")));
    }
    if !said.is_empty() {
        out.push_str(&format!("\n\n{}", said.join("; ")));
    }
    out
}

/// A cell's value as its column writes it, or `blank`.
fn shown(c: &Column, row: usize) -> String {
    match c.value(row) {
        Value::Null => "blank".to_owned(),
        v => c.form(v),
    }
}

/// A row in a line: its first columns' values.
fn summary(doc: &Doc, row: usize) -> String {
    let cells: Vec<String> = doc.columns().iter().take(6).map(|c| format!("{} {}", c.name, shown(c, row))).collect();
    cells.join(" · ")
}

/// The row of `held` a reference's text names, and its key column.
fn target_row(held: &Held, key: &str, text: &str) -> Option<(usize, usize)> {
    let doc = held.doc();
    let k = doc.column(key)?;
    (0..doc.rows()).find(|&r| doc.text(r, k) == text).map(|r| (r, k))
}

fn markdown(value: String, range: Option<Range>) -> Hover {
    Hover { contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value }), range }
}

pub fn hover(folder: &Folder, file: &File, at: usize) -> Option<Hover> {
    let lines = Lines::new(&file.text);
    let doc = &file.doc;
    match spot(file, at) {
        Spot::Name(c, s) | Spot::Type(c, s) => Some(markdown(about(&doc.columns()[c]), Some(lines.range(s)))),
        Spot::Cell(r, c, s) => {
            let col = &doc.columns()[c];
            let mut out = format!("{}\n\n", about(col));
            match col.origin(r) {
                Origin::Formula => out.push_str(&format!("computed: **{}**", shown(col, r))),
                _ => out.push_str(&format!("**{}**", shown(col, r))),
            }
            if let Some(p) = doc.problems().iter().find(|p| p.row as usize == r && p.column as usize == c) {
                out.push_str(&format!("\n\n{}", doc.why(p)));
            }
            if let Base::Ref(targets) = &col.ty.base {
                let text = doc.text(r, c);
                for t in targets {
                    if let Some(held) = folder.held(file, &t.table)
                        && let Some((row, _)) = target_row(&held, &t.column, &text)
                    {
                        out.push_str(&format!("\n\n{}: {}", t.table, summary(held.doc(), row)));
                    }
                }
            }
            Some(markdown(out, Some(lines.range(s))))
        }
        Spot::Formula(f) => {
            let at = at as u32 - f.base;
            let names = expr::names(f.src).ok()?;
            let (n, part) = names.iter().find_map(|n| n.parts.iter().position(|(_, (s, l))| at >= *s && at <= s + l).map(|k| (n, k)))?;
            let (name, (s, l)) = &n.parts[part];
            let range = Some(lines.range(span(f.base + s, *l)));
            let said = match follow(folder, file, n, part)? {
                Followed::Column(held, k) => {
                    let d = held.as_ref().map_or(doc, |h| h.doc());
                    about(&d.columns()[k])
                }
                Followed::Table(held) => {
                    let d = held.doc();
                    let columns: Vec<&str> = d.columns().iter().map(|c| c.name.as_str()).collect();
                    format!("table `{name}`: {} rows, {}", d.rows(), columns.join(", "))
                }
            };
            Some(markdown(said, range))
        }
        Spot::Prelude | Spot::Nothing => None,
    }
}

/// What a part of a name in a formula of `file` is.
enum Followed {
    /// A column: of `file` itself (nothing), or of a table.
    Column(Option<Held>, usize),
    Table(Held),
}

/// Part `k` of a name, followed: its first part a column of the file (or a table, `table`
/// or not), every part after it a column of the table the one before leads to.
fn follow(folder: &Folder, file: &File, n: &Named, k: usize) -> Option<Followed> {
    let first = &n.parts[0].0;
    let mut at: Followed = match (n.table, file.doc.column(first)) {
        (false, Some(c)) => Followed::Column(None, c),
        _ => Followed::Table(folder.held(file, first)?),
    };
    for (name, _) in n.parts.iter().take(k + 1).skip(1) {
        at = match at {
            Followed::Table(held) => {
                let c = held.doc().column(name)?;
                Followed::Column(Some(held), c)
            }
            Followed::Column(held, c) => {
                // A reference leads to its first table's row.
                let doc = held.as_ref().map_or(&file.doc, |h| h.doc());
                let Base::Ref(targets) = &doc.columns()[c].ty.base else { return None };
                let from = held.as_ref().map_or(file, |h| &h.file);
                let next = folder.held(from, &targets[0].table)?;
                let c = next.doc().column(name)?;
                Followed::Column(Some(next), c)
            }
        };
    }
    Some(at)
}

// ---- completion ----

fn item(label: String, kind: CompletionItemKind, detail: String, range: Range) -> CompletionItem {
    CompletionItem {
        label: label.clone(),
        kind: Some(kind),
        detail: Some(detail),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit { range, new_text: label })),
        ..CompletionItem::default()
    }
}

fn ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// A name as a formula writes it: as it is, or quoted.
fn written(name: &str) -> String {
    let plain = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && name.chars().all(ident);
    if plain { name.to_owned() } else { format!("\"{}\"", name.replace('"', "\"\"")) }
}

pub fn complete(folder: &Folder, file: &File, at: usize) -> Vec<CompletionItem> {
    let lines = Lines::new(&file.text);
    let doc = &file.doc;
    match spot(file, at) {
        Spot::Cell(_, c, s) => {
            let range = lines.range(s);
            let col = &doc.columns()[c];
            match &col.ty.base {
                Base::Ref(targets) => {
                    let mut out = Vec::new();
                    for t in targets {
                        let Some(held) = folder.held(file, &t.table) else { continue };
                        let d = held.doc();
                        let Some(k) = d.column(&t.column) else { continue };
                        for row in 0..d.rows() {
                            let key = d.text(row, k).into_owned();
                            if key.is_empty() {
                                continue;
                            }
                            let mut i = item(key, CompletionItemKind::VALUE, format!("{}: {}", t.table, summary(d, row)), range);
                            if let Some(CompletionTextEdit::Edit(e)) = &mut i.text_edit {
                                e.new_text = cells::quote(&e.new_text).into_owned();
                            }
                            out.push(i);
                        }
                    }
                    out
                }
                Base::Choice(words) => {
                    words.iter().map(|w| item(w.clone(), CompletionItemKind::ENUM_MEMBER, col.name.clone(), range)).collect()
                }
                Base::Bool { yes, no } => {
                    [yes, no].iter().map(|w| item((*w).clone(), CompletionItemKind::VALUE, col.name.clone(), range)).collect()
                }
                _ => Vec::new(),
            }
        }
        Spot::Type(_, s) => {
            // The word at the place, `.` included, so `accounts.a` is one.
            let text = &file.text[s.start as usize..(s.start + s.len) as usize];
            let here = at - s.start as usize;
            let from = text[..here].rfind(|c: char| !(ident(c) || c == '.')).map_or(0, |i| i + 1);
            let to = here + text[here..].find(|c: char| !(ident(c) || c == '.')).unwrap_or(text.len() - here);
            let range = lines.range(span(s.start + from as u32, (to - from) as u32));
            let mut out: Vec<CompletionItem> =
                doc.named().iter().map(|t| item(t.name.clone(), CompletionItemKind::CLASS, t.ty.shorthand(), range)).collect();
            for t in doc.tables() {
                let Some(d) = &t.doc else { continue };
                if let Some(key) = d.columns().iter().find(|c| c.ty.key) {
                    out.push(item(
                        format!("{}.{}", t.name, key.name),
                        CompletionItemKind::REFERENCE,
                        format!("a key of {}", t.name),
                        range,
                    ));
                }
            }
            for b in ["text", "int", "num", "pct", "bool", "date", "time", "datetime", "url", "email", "id"] {
                out.push(item(b.to_owned(), CompletionItemKind::KEYWORD, "a base".into(), range));
            }
            out
        }
        Spot::Formula(f) => formula_completions(folder, file, &f, at, &lines),
        Spot::Name(..) | Spot::Prelude | Spot::Nothing => Vec::new(),
    }
}

fn formula_completions(folder: &Folder, file: &File, f: &Formula, at: usize, lines: &Lines) -> Vec<CompletionItem> {
    let here = at - f.base as usize;
    let src = &f.src[..here];
    let from = src.rfind(|c: char| !ident(c)).map_or(0, |i| i + 1);
    let range = lines.range(span(f.base + from as u32, (here - from) as u32));
    let columns = |doc: &Doc, detail: &str| -> Vec<CompletionItem> {
        doc.columns()
            .iter()
            .map(|c| item(written(&c.name), CompletionItemKind::FIELD, format!("{detail}{}", c.ty.shorthand()), range))
            .collect()
    };
    let before = src[..from].trim_end_matches([' ', '\t']);
    if let Some(path) = before.strip_suffix('.') {
        // `table['k'].`: the table's columns.
        if let Some(open) = path.strip_suffix(']').and_then(|p| p.rfind('[').map(|i| &p[..i])) {
            let name = &open[open.rfind(|c: char| !ident(c)).map_or(0, |i| i + 1)..];
            return folder.held(file, name).map_or(Vec::new(), |h| columns(h.doc(), &format!("{name}: ")));
        }
        // `a.b.`: what the path leads to.
        let start = path.rfind(|c: char| !(ident(c) || c == '.')).map_or(0, |i| i + 1);
        let parts: Vec<(String, (u32, u32))> = path[start..].split('.').map(|p| (p.to_owned(), (0, 0))).collect();
        let n = Named { parts, table: false, key: None };
        return match follow(folder, file, &n, n.parts.len() - 1) {
            Some(Followed::Table(h)) => columns(h.doc(), ""),
            Some(Followed::Column(held, c)) => {
                let doc = held.as_ref().map_or(&file.doc, |h| h.doc());
                let Base::Ref(targets) = &doc.columns()[c].ty.base else { return Vec::new() };
                let from = held.as_ref().map_or(file, |h| &h.file);
                folder.held(from, &targets[0].table).map_or(Vec::new(), |h| columns(h.doc(), &format!("{}: ", targets[0].table)))
            }
            None => Vec::new(),
        };
    }
    let mut out = columns(&file.doc, "");
    for t in file.doc.tables() {
        out.push(item(written(&t.name), CompletionItemKind::MODULE, "a table".into(), range));
    }
    for name in expr::FUNCTIONS {
        out.push(item(name.to_owned(), CompletionItemKind::FUNCTION, "a function".into(), range));
    }
    out
}

// ---- definition ----

pub fn definition(folder: &Folder, file: &File, at: usize) -> Option<Location> {
    let doc = &file.doc;
    let located =
        |held: &Held, s: Span| Location { uri: uri(&held.file.path), range: Lines::new(&held.file.text).range(shifted(s, held.base)) };
    let header = |held: &Held, k: usize| located(held, held.doc().header_span(k));
    match spot(file, at) {
        Spot::Cell(r, c, _) => {
            let Base::Ref(targets) = &doc.columns()[c].ty.base else { return None };
            let text = doc.text(r, c);
            targets.iter().find_map(|t| {
                let held = folder.held(file, &t.table)?;
                let (row, k) = target_row(&held, &t.column, &text)?;
                Some(located(&held, held.doc().cell_span(row, k)))
            })
        }
        Spot::Type(c, s) => {
            if let Base::Ref(targets) = &doc.columns()[c].ty.base {
                let typed = &file.text[s.start as usize..(s.start + s.len) as usize];
                let here = at - s.start as usize;
                // The target the place is in, else the first.
                let t = targets
                    .iter()
                    .find(|t| {
                        let w = format!("{}.{}", t.table, t.column);
                        typed.match_indices(&w).any(|(i, _)| here >= i && here <= i + w.len())
                    })
                    .unwrap_or(&targets[0]);
                let held = folder.held(file, &t.table)?;
                return Some(header(&held, held.doc().column(&t.column)?));
            }
            // A named type, where the prelude names it.
            let name = doc.columns()[c].ty.named.as_ref()?.name.clone();
            let line = doc.statements().iter().find_map(|s| match s {
                Statement::Type { line, name: n, .. } if *n == name => Some(*line),
                _ => None,
            })?;
            Some(prelude_line(file, line))
        }
        Spot::Formula(f) => {
            let at = at as u32 - f.base;
            let names = expr::names(f.src).ok()?;
            let (n, k) = names.iter().find_map(|n| n.parts.iter().position(|(_, (s, l))| at >= *s && at <= s + l).map(|k| (n, k)))?;
            match follow(folder, file, n, k)? {
                Followed::Column(None, c) => {
                    Some(Location { uri: uri(&file.path), range: Lines::new(&file.text).range(doc.header_span(c)) })
                }
                Followed::Column(Some(held), c) => Some(header(&held, c)),
                Followed::Table(held) => match &held.inline {
                    Some(_) => Some(located(&held, span(0, 0))),
                    None => Some(Location { uri: uri(&held.file.path), range: Range::default() }),
                },
            }
        }
        Spot::Prelude => {
            // An import's line: the file it takes from.
            let line = Lines::new(&file.text).position(at).line;
            doc.statements().iter().find_map(|s| match s {
                Statement::Import { line: l, from, .. } if *l == line && !cells::files::is_url(from) => {
                    let path = super::folder::canonical(&file.path.parent()?.join(from));
                    Some(Location { uri: uri(&path), range: Range::default() })
                }
                _ => None,
            })
        }
        Spot::Name(..) | Spot::Nothing => None,
    }
}

/// Prelude line `n` (1-based inside the fences, the file's line `n` after its `---`).
fn prelude_line(file: &File, n: u32) -> Location {
    let start = tower_lsp::lsp_types::Position { line: n, character: 0 };
    Location { uri: uri(&file.path), range: Range { start, end: start } }
}

// ---- rename ----

/// What a rename at a place renames, and where its name is.
pub enum Renamed {
    Column(usize, Span),
    Key(usize, usize, Span),
}

pub fn renamed(file: &File, at: usize) -> Option<Renamed> {
    let doc = &file.doc;
    match spot(file, at) {
        Spot::Name(c, s) => Some(Renamed::Column(c, s)),
        Spot::Cell(r, c, s) if doc.columns()[c].ty.key => Some(Renamed::Key(r, c, s)),
        Spot::Formula(f) => {
            let at = at as u32 - f.base;
            let names = expr::names(f.src).ok()?;
            names.iter().find_map(|n| {
                let (name, (s, l)) = &n.parts[0];
                (!n.table && at >= *s && at <= s + l).then(|| doc.column(name).map(|c| Renamed::Column(c, span(f.base + s, *l))))?
            })
        }
        _ => None,
    }
}

/// A cell's field rewritten to hold `to`: its quotes, when it has them, taken in, and the
/// text quoted only where CSV needs it.
fn field(file: &File, s: Span, to: &str) -> (Span, String) {
    let b = file.text.as_bytes();
    let quoted = s.start > 0 && b[s.start as usize - 1] == b'"' && b.get((s.start + s.len) as usize) == Some(&b'"');
    let s = if quoted { span(s.start - 1, s.len + 2) } else { s };
    (s, cells::quote(to).into_owned())
}

/// Every edit a rename makes, by file: the name where it is defined, and every file of the
/// folder that names it, its formulas and references included.
pub fn rename(folder: &Folder, file: &File, at: usize, to: &str) -> Result<WorkspaceEdit, String> {
    let what = renamed(file, at).ok_or("nothing here is renamed: a column's name or a key")?;
    let mut edits: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    let mut edit = |f: &File, s: Span, text: String| {
        let range = Lines::new(&f.text).range(s);
        edits.entry(uri(&f.path)).or_default().push(TextEdit { range, new_text: text });
    };
    let others: Vec<File> = folder.files().iter().filter(|p| **p != file.path).filter_map(|p| folder.read(p)).collect();
    match what {
        Renamed::Column(c, _) => {
            // A name a formula writes bare, so no record needs quoting for it.
            if written(to) != to {
                return Err(format!("\"{to}\" is a name formulas would quote: a rename takes letters, digits and _"));
            }
            let old = file.doc.columns()[c].name.clone();
            if file.doc.column(to).is_some() {
                return Err(format!("a column is named {to} already"));
            }
            edit(file, header_cell(file, c).name, to.to_owned());
            for (src, base) in formulas(file) {
                for n in expr::names(src).into_iter().flatten() {
                    let (name, (s, l)) = &n.parts[0];
                    if !n.table && *name == old {
                        edit(file, span(base + s, *l), written(to));
                    }
                }
            }
            for f in &others {
                // A reference to the column, and a formula reading it through a table.
                for (k, col) in f.doc.columns().iter().enumerate() {
                    let Base::Ref(targets) = &col.ty.base else { continue };
                    for t in targets.iter().filter(|t| t.column == old && names_main(f, &t.table, &file.path)) {
                        let Some(ty) = header_cell(f, k).ty else { continue };
                        let typed = &f.text[ty.start as usize..(ty.start + ty.len) as usize];
                        let w = format!("{}.{}", t.table, old);
                        for (i, _) in typed.match_indices(&w) {
                            edit(f, span(ty.start + (i + t.table.len() + 1) as u32, old.len() as u32), to.to_owned());
                        }
                    }
                }
                for (src, base) in formulas(f) {
                    for n in expr::names(src).into_iter().flatten() {
                        let Some((name, (s, l))) = n.parts.get(1) else { continue };
                        if *name != old {
                            continue;
                        }
                        let first = &n.parts[0].0;
                        let leads = match (n.table, f.doc.column(first)) {
                            (false, Some(k)) => match &f.doc.columns()[k].ty.base {
                                Base::Ref(targets) => names_main(f, &targets[0].table, &file.path),
                                _ => false,
                            },
                            _ => names_main(f, first, &file.path),
                        };
                        if leads {
                            edit(f, span(base + s, *l), written(to));
                        }
                    }
                }
            }
        }
        Renamed::Key(r, c, s) => {
            if to.is_empty() || to.chars().any(char::is_control) {
                return Err("a key is never blank and holds no control character".into());
            }
            let old = file.doc.text(r, c).into_owned();
            let key = &file.doc.columns()[c].name;
            let (s, text) = field(file, s, to);
            edit(file, s, text);
            for f in &others {
                for (k, col) in f.doc.columns().iter().enumerate() {
                    let Base::Ref(targets) = &col.ty.base else { continue };
                    if !targets.iter().any(|t| t.column == *key && names_main(f, &t.table, &file.path)) {
                        continue;
                    }
                    for row in 0..f.doc.rows() {
                        if f.doc.text(row, k) == old {
                            let (s, text) = field(f, f.doc.cell_span(row, k), to);
                            edit(f, s, text);
                        }
                    }
                }
                for (src, base) in formulas(f) {
                    for n in expr::names(src).into_iter().flatten() {
                        if let (true, Some((k, (s, l)))) = (n.table, &n.key)
                            && *k == old
                            && names_main(f, &n.parts[0].0, &file.path)
                        {
                            edit(f, span(base + s, *l), format!("'{}'", to.replace('\'', "''")));
                        }
                    }
                }
            }
        }
    }
    Ok(WorkspaceEdit { changes: Some(edits), ..WorkspaceEdit::default() })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    use tower_lsp::lsp_types::{CompletionTextEdit, HoverContents};

    use super::super::folder::{Folder, canonical};
    use super::*;

    const ACCOUNTS: &str = "account:text!,kind:assets|expenses\nbank,assets\nrent,expenses\npower,expenses\n";
    const MONEY: &str = "---\nimport accounts from \"../accounts.cell\"\n---\nid:text!,amount:EUR,to:accounts.account,note:text?,double:EUR=amount * 2\na1,950.00,rent,,\na2,64.30,pwer,,\na3,8x,bank,`=accounts['rent'].kind`,\n";

    /// A folder of two files, `books/accounts.cell` and `books/home/money.cell`.
    fn books(name: &str) -> (Folder, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("cells-lsp-{name}-{}", std::process::id()));
        fs::create_dir_all(root.join("home")).unwrap();
        fs::write(root.join("accounts.cell"), ACCOUNTS).unwrap();
        fs::write(root.join("home/money.cell"), MONEY).unwrap();
        let root = canonical(&root);
        let folder = Folder { root: Some(root.clone()), open: Arc::new(BTreeMap::new()) };
        (folder, root.join("accounts.cell"), root.join("home/money.cell"))
    }

    /// The byte where `what` is written in `text`, `nth` from 0.
    fn at(text: &str, what: &str, nth: usize) -> usize {
        text.match_indices(what).nth(nth).unwrap_or_else(|| panic!("no {what:?} {nth}")).0
    }

    fn said(h: Hover) -> String {
        match h.contents {
            HoverContents::Markup(m) => m.value,
            other => panic!("markup, not {other:?}"),
        }
    }

    #[test]
    fn diagnostics_and_hover() {
        let (folder, _, money) = books("diagnose");
        let file = folder.read(&money).unwrap();
        let d = diagnostics(&file);
        let messages: Vec<&str> = d.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(messages, ["to: \"pwer\" is no key of accounts", "amount: \"8x\" is not a number"]);
        let lines = Lines::new(MONEY);
        assert_eq!(d[0].range, lines.range(span(at(MONEY, "pwer", 0) as u32, 4)));
        let rent = said(hover(&folder, &file, at(MONEY, "rent", 0)).unwrap());
        assert!(rent.contains("accounts: account rent · kind expenses"), "{rent}");
        let double = said(hover(&folder, &file, at(MONEY, "double", 0)).unwrap());
        assert!(double.starts_with("`double:EUR` `=amount * 2`"), "{double}");
        let kind = said(hover(&folder, &file, at(MONEY, ".kind", 0) + 2).unwrap());
        assert_eq!(kind, "`kind:assets|expenses`");
    }

    #[test]
    fn a_broken_header_cell_is_marked_and_the_rest_answers() {
        let (folder, _, money) = books("broken");
        let text = MONEY.replace("to:accounts.account", "to:accounts.a");
        let folder = Folder { open: Arc::new(BTreeMap::from([(money.clone(), text.clone())])), ..folder };
        let file = folder.read(&money).unwrap();
        let d = diagnostics(&file);
        assert_eq!(d[0].message, "to: accounts has no column \"a\" (its key is account)");
        assert_eq!(d[0].range, Lines::new(&text).range(span(at(&text, "accounts.a,", 0) as u32, 10)));
        assert!(hover(&folder, &file, at(&text, "950", 0)).is_some());
    }

    #[test]
    fn completion() {
        let (folder, _, money) = books("complete");
        let file = folder.read(&money).unwrap();
        let labels = |items: Vec<CompletionItem>| items.into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(labels(complete(&folder, &file, at(MONEY, "pwer", 0) + 2)), ["bank", "rent", "power"]);
        let formula = labels(complete(&folder, &file, at(MONEY, "amount * 2", 0) + 2));
        assert!(formula.starts_with(&["id".into(), "amount".into(), "to".into()]) && formula.contains(&"accounts".into()), "{formula:?}");
        assert!(formula.contains(&"sum".into()));
        let after = at(MONEY, "['rent'].", 0) + "['rent'].".len();
        assert_eq!(labels(complete(&folder, &file, after)), ["account", "kind"]);
        let typed = complete(&folder, &file, at(MONEY, "accounts.account", 0) + 3);
        let key = typed.iter().find(|i| i.label == "accounts.account").expect("the table's key");
        let Some(CompletionTextEdit::Edit(e)) = &key.text_edit else { panic!("an edit") };
        assert_eq!(e.range, Lines::new(MONEY).range(span(at(MONEY, "accounts.account", 0) as u32, 16)));
    }

    #[test]
    fn definition_follows_references_and_names() {
        let (folder, accounts, money) = books("define");
        let file = folder.read(&money).unwrap();
        let rent = definition(&folder, &file, at(MONEY, "rent", 0)).unwrap();
        assert_eq!(rent.uri, uri(&accounts));
        assert_eq!(rent.range, Lines::new(ACCOUNTS).range(span(at(ACCOUNTS, "rent", 0) as u32, 4)));
        let typed = definition(&folder, &file, at(MONEY, "accounts.account", 0) + 10).unwrap();
        assert_eq!(typed.range, Lines::new(ACCOUNTS).range(span(0, "account:text!".len() as u32)));
        let amount = definition(&folder, &file, at(MONEY, "amount * 2", 0)).unwrap();
        assert_eq!((amount.uri, amount.range.start.line), (uri(&money), 3));
        let import = definition(&folder, &file, at(MONEY, "import", 0)).unwrap();
        assert_eq!(import.uri, uri(&accounts));
    }

    #[test]
    fn a_key_renamed_follows_every_reference() {
        let (folder, accounts, money) = books("rename-key");
        let file = folder.read(&accounts).unwrap();
        let e = rename(&folder, &file, at(ACCOUNTS, "rent", 0), "lease").unwrap();
        let changes = e.changes.unwrap();
        let texts = |u: &Path| changes[&uri(u)].iter().map(|t| t.new_text.as_str()).collect::<Vec<_>>();
        assert_eq!(texts(&accounts), ["lease"]);
        // The reference, and the formula's key.
        assert_eq!(texts(&money), ["lease", "'lease'"]);
    }

    #[test]
    fn a_column_renamed_follows_every_formula() {
        let (folder, accounts, money) = books("rename-column");
        let file = folder.read(&accounts).unwrap();
        let e = rename(&folder, &file, at(ACCOUNTS, "kind", 0), "group").unwrap();
        let changes = e.changes.unwrap();
        assert_eq!(changes[&uri(&money)].len(), 1, "the formula reading accounts['rent'].kind");
        let file = folder.read(&money).unwrap();
        let e = rename(&folder, &file, at(MONEY, "amount", 0), "sum_paid").unwrap();
        let edits = &e.changes.unwrap()[&uri(&money)];
        assert_eq!(edits.len(), 2, "the header cell and the formula reading it");
        assert!(rename(&folder, &file, at(MONEY, "amount", 0), "two words").is_err());
    }
}
