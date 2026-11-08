//! What the language server answers, each a function of the folder as the editor has it
//! and a place in one file: its diagnostics, a hover, completions, a definition, a rename
//! followed through every file that names what is renamed.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CompletionItem, CompletionItemKind, CompletionTextEdit, Diagnostic, DiagnosticSeverity, Documentation,
    Hover, HoverContents, Location, MarkupContent, MarkupKind, Range, TextEdit, Url, WorkspaceEdit,
};

use cells::complete::{CompletionKind as Kind, HeaderCell, Spot, opened, written, written_key};
use cells::expr::{self, Named};
use cells::{Base, Doc, Origin, Span, Statement};

use cells_daemon::allowed::Allowed;
use tower_lsp::lsp_types::LSPAny;

use super::folder::{File, Folder, Held, inline, names_main};
use super::text::Lines;

fn span(start: u32, len: u32) -> Span {
    Span { start, len }
}

fn shifted(s: Span, by: u32) -> Span {
    span(s.start + by, s.len)
}

// ---- where a place is: the engine's (cells::complete) ----

pub fn spot(file: &File, at: usize) -> Spot<'_> {
    cells::complete::spot(&file.doc, &file.text, at)
}

fn header_cell(file: &File, column: usize) -> HeaderCell<'_> {
    cells::complete::header_cell(&file.doc, &file.text, column)
}

fn cell_formula(file: &File, row: usize, column: usize) -> Option<(&str, u32)> {
    cells::complete::cell_formula(&file.doc, &file.text, row, column)
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
pub fn diagnostics(file: &File, allowed: &Allowed) -> Vec<Diagnostic> {
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
                Some(t) => format!("{t}, row {}, {column}", p.row + 1),
                None => column.clone(),
            };
            let mut d = error(lines.range(shifted(s, base)), format!("{place}: {}", doc.why(p)));
            // what is not there yet warns; what is wrong is an error
            if p.kind().severity() == cells::Severity::Warning {
                d.severity = Some(DiagnosticSeverity::WARNING);
            }
            out.push(d);
        }
    };
    problems(doc, 0, None, &mut out);
    for t in doc.tables().iter().filter(|t| t.own) {
        if let Some((d, base)) = inline(file, &t.name) {
            problems(&d, base, Some(&t.name), &mut out);
        }
    }
    for s in doc.scripts().iter().filter(|s| !allowed.allows(&file.path, s)) {
        let mut d = error(
            script_line(file, s),
            format!("{} does not run: not allowed on this Mac as it is written. Read its code, then allow it from its fix.", s.name),
        );
        d.severity = Some(DiagnosticSeverity::WARNING);
        out.push(d);
    }
    out
}

/// A script's statement line, whole: where its warning and its fix stand.
fn script_line(file: &File, script: &cells::Script) -> Range {
    let n = script.line;
    let len = file.text.lines().nth(n as usize).map_or(0, |l| l.encode_utf16().count() as u32);
    Range {
        start: tower_lsp::lsp_types::Position { line: n, character: 0 },
        end: tower_lsp::lsp_types::Position { line: n, character: len },
    }
}

/// The command the fix of a script not allowed runs: the engine keeps the approval of the
/// version the editor shows (`cells_daemon::allowed`).
pub const ALLOW: &str = "cells.allow";

// ---- hover ----

/// The row of `held` a reference's text names, and its key column.
fn target_row(held: &Held, key: &str, text: &str) -> Option<(usize, usize)> {
    let doc = held.doc();
    let k = doc.column(key)?;
    (0..doc.rows()).find(|&r| doc.text(r, k) == text).map(|r| (r, k))
}

/// The tables a file names, by a path of names, through the folder as the editor has it:
/// what the engine's `complete` and `hover` reach them by.
fn reach<'a>(folder: &'a Folder, file: &'a File) -> impl Fn(&[&str]) -> Option<Held> + 'a {
    move |path: &[&str]| {
        let (first, rest) = path.split_first()?;
        let mut h = folder.held(file, first)?;
        for name in rest {
            h = folder.held(&h.file, name)?;
        }
        Some(h)
    }
}

/// What is said of `at`, the engine's (`cells::hover`), in Markdown.
pub fn hover(folder: &Folder, file: &File, at: usize) -> Option<Hover> {
    let lines = Lines::new(&file.text);
    let h = cells::hover::hover(&file.doc, &file.text, at, reach(folder, file))?;
    let range = Some(lines.range(span(h.from, h.to - h.from)));
    Some(Hover { contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: h.markdown() }), range })
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

/// What completes at `at`, the engine's (`cells::complete`), the tables a file names
/// reached through the folder as the editor has it.
pub fn complete(folder: &Folder, file: &File, at: usize) -> Vec<CompletionItem> {
    let lines = Lines::new(&file.text);
    cells::complete::complete(&file.doc, &file.text, at, reach(folder, file))
        .into_iter()
        .map(|c| {
            let range = lines.range(span(c.from, c.to - c.from));
            // An editor draws a colour's swatch from its documentation, a hex.
            let documentation = (c.kind == Kind::Color)
                .then(|| cells::Tint::parse(&c.label).ok())
                .flatten()
                .map(|t| Documentation::String(format!("#{:06x}", super::colour::swatch(t))));
            CompletionItem {
                label: c.label,
                documentation,
                kind: Some(match c.kind {
                    Kind::Value => CompletionItemKind::VALUE,
                    Kind::EnumMember => CompletionItemKind::ENUM_MEMBER,
                    Kind::Class => CompletionItemKind::CLASS,
                    Kind::Reference => CompletionItemKind::REFERENCE,
                    Kind::Keyword => CompletionItemKind::KEYWORD,
                    Kind::Field => CompletionItemKind::FIELD,
                    Kind::Module => CompletionItemKind::MODULE,
                    Kind::Function => CompletionItemKind::FUNCTION,
                    Kind::Color => CompletionItemKind::COLOR,
                }),
                detail: Some(c.detail),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit { range, new_text: c.insert })),
                filter_text: c.filter,
                sort_text: c.sort,
                ..CompletionItem::default()
            }
        })
        .collect()
}

// ---- #n ----

/// A `#n` written in a formula: `T[#n]`, the row numbered `n` (from 1) where it shows,
/// which a file never holds; each one in `file` whose table has that row, with its key.
pub struct Numbered {
    pub span: Span,
    pub row: usize,
    pub key: String,
}

pub fn numbered(folder: &Folder, file: &File) -> Vec<Numbered> {
    let mut out = Vec::new();
    for (src, base) in formulas(file) {
        for (i, _) in src.match_indices('#') {
            let digits = src[i + 1..].bytes().take_while(u8::is_ascii_digit).count();
            let Ok(n) = src[i + 1..i + 1 + digits].parse::<usize>() else { continue };
            // `[`, then the number, then `]`.
            let Some((name, open)) = opened(&src[..i]) else { continue };
            if !src[open..i].trim().is_empty() || !src[i + 1 + digits..].trim_start().starts_with(']') {
                continue;
            }
            let Some(held) = folder.held(file, name) else { continue };
            let d = held.doc();
            let Some(k) = d.columns().iter().position(|c| c.ty.key) else { continue };
            if n == 0 || n > d.rows() || d.text(n - 1, k).is_empty() {
                continue;
            }
            out.push(Numbered { span: span(base + i as u32, 1 + digits as u32), row: n, key: d.text(n - 1, k).into_owned() });
        }
    }
    out
}

/// The file as a write leaves it (SPEC.md, Writing), for a client formatting it as it
/// saves: every computed value the edits since `saved` reached stored beside its `=`,
/// every one not stored yet stored, a stale cell the edits did not reach left as it is;
/// then laid out as `cells fmt` lays it (SPEC.md, Formatting), its columns aligned or,
/// with `align` off, no padding. Nothing when it is already so.
pub fn formatted(file: &File, saved: Option<&File>, align: bool) -> Option<TextEdit> {
    let mut doc = file.doc.clone();
    match saved {
        Some(saved) => doc.stored_since(&saved.doc),
        None => doc.store_unstored(),
    }
    let text = cells::layout::formatted(&doc.write(), align);
    (text != file.text).then(|| whole(file, &Lines::new(&file.text), text))
}

/// A fix: its title, and its edits to the file.
fn quickfix(folder: &Folder, file: &File, title: String, edits: Vec<TextEdit>) -> CodeAction {
    CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        edit: Some(WorkspaceEdit { changes: Some(HashMap::from([(folder.uri(&file.path), edits)])), ..WorkspaceEdit::default() }),
        ..CodeAction::default()
    }
}

/// The whole file replaced by `text`: one edit, for a change a write makes beyond the
/// cell it starts from (a value stored below an override kept).
fn whole(file: &File, lines: &Lines, text: String) -> TextEdit {
    TextEdit { range: lines.range(span(0, file.text.len() as u32)), new_text: text }
}

/// What can be done at a range, on its lines:
/// - each `#n` written as its row's key;
/// - a stale cell given its formula's value, or its stored value kept as the row's
///   override (SPEC.md, Writing);
/// - a word of the language written in lower case written in capitals;
/// - a spreadsheet's formula (`=C2*D2`, `=SUM($E$2:E2)`) written in names.
pub fn actions(folder: &Folder, file: &File, range: Range, allowed: &Allowed) -> Vec<CodeAction> {
    let lines = Lines::new(&file.text);
    let on = |s: Span| {
        let r = lines.range(s);
        r.start.line <= range.end.line && r.end.line >= range.start.line
    };
    let mut out: Vec<CodeAction> = numbered(folder, file)
        .into_iter()
        .filter(|n| on(n.span))
        .map(|n| {
            let edit = TextEdit { range: lines.range(n.span), new_text: written_key(&n.key) };
            quickfix(folder, file, format!("#{} is the row {}: write its key", n.row, written_key(&n.key)), Vec::from([edit]))
        })
        .collect();
    // a script not allowed, allowed at the version shown: no edit, the engine keeps it
    for s in file.doc.scripts().iter().filter(|s| !allowed.allows(&file.path, s)) {
        let at = script_line(file, s);
        if at.start.line < range.start.line || at.start.line > range.end.line {
            continue;
        }
        let title = format!("Allow {} to run, as written", s.name);
        let arguments = Some(Vec::from([
            LSPAny::String(file.path.display().to_string()),
            LSPAny::String(s.name.clone()),
            LSPAny::String(cells_daemon::allowed::version(s)),
        ]));
        out.push(CodeAction {
            title: title.clone(),
            kind: Some(CodeActionKind::QUICKFIX),
            command: Some(tower_lsp::lsp_types::Command { title, command: ALLOW.to_owned(), arguments }),
            ..CodeAction::default()
        });
    }
    let doc = &file.doc;
    for (p, s) in doc.problems().iter().zip(doc.spans()) {
        if p.kind() != cells::Kind::Stale || !on(s) {
            continue;
        }
        let (r, c) = (p.row as usize, p.column as usize);
        let raw = doc.text(r, c).into_owned();
        let stored = raw[..cells::split_cell(&raw).unwrap_or(raw.len())].to_owned();
        let mut fixed = doc.clone();
        fixed.fix_stale_at(r, c);
        let gives = fixed.text(r, c);
        let gives = gives[..cells::split_cell(&gives).unwrap_or(gives.len())].to_owned();
        out.push(quickfix(folder, file, format!("Use the formula's value, {gives}"), Vec::from([whole(file, &lines, fixed.write())])));
        let mut kept = doc.clone();
        if kept.keep_stale(r, c).is_ok() {
            out.push(quickfix(
                folder,
                file,
                format!("Keep {stored} as this row's override"),
                Vec::from([whole(file, &lines, kept.write())]),
            ));
        }
    }
    // Every formula on the lines: words of the language in lower case, and a spreadsheet's
    // references, each written as the file holds them.
    let columns: Vec<&str> = doc.columns().iter().map(|c| c.name.as_str()).collect();
    let key = doc.columns().iter().position(|c| c.ty.key);
    let keys = |r: usize| key.map(|k| doc.text(r, k).into_owned()).filter(|t| !t.is_empty());
    let sheet = cells::sheet::Sheet { columns: &columns, rows: doc.rows(), key: &keys };
    let names = |w: &str| doc.column(w).is_some() || doc.tables().iter().any(|t| t.name == w);
    for (src, base, row) in formula_rows(file) {
        if !on(span(base, src.len() as u32)) {
            continue;
        }
        let low: Vec<TextEdit> = expr::low_words(src)
            .into_iter()
            .filter(|((s, l), _)| !names(&src[*s as usize..(s + l) as usize]))
            .map(|((s, l), up)| TextEdit { range: lines.range(span(base + s, l)), new_text: up })
            .collect();
        if !low.is_empty() {
            let words: Vec<&str> = low.iter().map(|e| e.new_text.as_str()).collect();
            out.push(quickfix(folder, file, format!("Write in capitals: {}", words.join(", ")), low));
        }
        if let Some(named) = cells::sheet::from_sheet(src, row, &sheet) {
            let edit = TextEdit { range: lines.range(span(base, src.len() as u32)), new_text: named.clone() };
            out.push(quickfix(folder, file, format!("Write the spreadsheet's references in names: {}", named.trim()), Vec::from([edit])));
        }
    }
    out
}

/// Every formula of a file with where it starts and the row it is computed on: a column's
/// on its first row (a spreadsheet's formula is typed on the first and filled down), a
/// cell's on its own.
fn formula_rows(file: &File) -> Vec<(&str, u32, usize)> {
    let doc = &file.doc;
    let mut out: Vec<(&str, u32, usize)> =
        (0..doc.columns().len()).filter_map(|c| header_cell(file, c).formula.map(|(f, b)| (f, b, 0))).collect();
    for (c, col) in doc.columns().iter().enumerate() {
        for r in 0..col.origin.len() {
            if col.origin(r) == Origin::Cell
                && let Some((f, b)) = cell_formula(file, r, c)
            {
                out.push((f, b, r));
            }
        }
    }
    out
}

// ---- definition ----

pub fn definition(folder: &Folder, file: &File, at: usize) -> Option<Location> {
    let doc = &file.doc;
    let located = |held: &Held, s: Span| Location {
        uri: folder.uri(&held.file.path),
        range: Lines::new(&held.file.text).range(shifted(s, held.base)),
    };
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
            Some(prelude_line(folder, file, line))
        }
        Spot::Formula(f) => {
            let at = at as u32 - f.base;
            let names = expr::names(f.src).ok()?;
            let (n, k) = names.iter().find_map(|n| n.parts.iter().position(|(_, (s, l))| at >= *s && at <= s + l).map(|k| (n, k)))?;
            match follow(folder, file, n, k)? {
                Followed::Column(None, c) => {
                    Some(Location { uri: folder.uri(&file.path), range: Lines::new(&file.text).range(doc.header_span(c)) })
                }
                Followed::Column(Some(held), c) => Some(header(&held, c)),
                Followed::Table(held) => match &held.inline {
                    Some(_) => Some(located(&held, span(0, 0))),
                    None => Some(Location { uri: folder.uri(&held.file.path), range: Range::default() }),
                },
            }
        }
        Spot::Prelude => {
            // An import's line: the file it takes from.
            let line = Lines::new(&file.text).position(at).line;
            doc.statements().iter().find_map(|s| match s {
                Statement::Import { line: l, from, .. } if *l == line && !cells::files::is_url(from) => {
                    let path = super::folder::canonical(&file.path.parent()?.join(from));
                    Some(Location { uri: folder.uri(&path), range: Range::default() })
                }
                _ => None,
            })
        }
        // A declared column's name: from its header cell to its declaration, and back.
        Spot::Name(c, s) => {
            let (declared, _) = doc.declaration_span(c)?;
            let to = if s == declared { doc.header_span(c) } else { declared };
            Some(Location { uri: folder.uri(&file.path), range: Lines::new(&file.text).range(to) })
        }
        Spot::Nothing => None,
    }
}

/// Prelude line `n` (1-based inside the fences, the file's line `n` after its `---`).
fn prelude_line(folder: &Folder, file: &File, n: u32) -> Location {
    let start = tower_lsp::lsp_types::Position { line: n, character: 0 };
    Location { uri: folder.uri(&file.path), range: Range { start, end: start } }
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
        edits.entry(folder.uri(&f.path)).or_default().push(TextEdit { range, new_text: text });
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
            // A declared column's declaration, renamed with it.
            if let Some((declared, _)) = file.doc.declaration_span(c) {
                edit(file, declared, to.to_owned());
            }
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
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use tower_lsp::lsp_types::{CompletionTextEdit, HoverContents};

    use super::super::folder::{Folder, canonical, spelling};
    use super::*;

    const ACCOUNTS: &str = "account:text!,kind:assets|expenses\nbank,assets\nrent,expenses\npower,expenses\n";
    const MONEY: &str = "---\nimport accounts from \"../accounts.cells\"\n---\nid:text!,amount:EUR,to:accounts.account,note:text?,double:EUR=amount * 2\na1,950.00,rent,,1900.00=\na2,64.30,pwer,,128.60=\na3,8x,bank,expenses=accounts['rent'].kind,=\n";

    /// A folder of two files, `books/accounts.cells` and `books/home/money.cells`.
    fn books(name: &str) -> (Folder, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("cells-lsp-{name}-{}", std::process::id()));
        fs::create_dir_all(root.join("home")).unwrap();
        fs::write(root.join("accounts.cells"), ACCOUNTS).unwrap();
        fs::write(root.join("home/money.cells"), MONEY).unwrap();
        let root = canonical(&root);
        let folder = Folder { root: Some(root.clone()), open: Arc::new(BTreeMap::new()), spelled: Arc::new(BTreeMap::new()) };
        (folder, root.join("accounts.cells"), root.join("home/money.cells"))
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
        let d = diagnostics(&file, &Allowed::default());
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
    fn a_declared_column_answers_from_its_declaration() {
        let (folder, _, money) = books("declared");
        let text = "---\nqty: int>=1\ntotal: EUR = cost * qty\n---\nitem:text!,cost:EUR,qty,total\nfuel,2.00,3,6.00=\n";
        let folder = Folder { open: Arc::new(BTreeMap::from([(money.clone(), text.to_owned())])), ..folder };
        let file = folder.read(&money).unwrap();
        assert!(diagnostics(&file, &Allowed::default()).is_empty(), "{:?}", diagnostics(&file, &Allowed::default()));
        let lines = Lines::new(text);
        // Hover on the declaration and on the header's bare name says the column.
        let declared = said(hover(&folder, &file, at(text, "total:", 0)).unwrap());
        assert!(declared.starts_with("`total:EUR` `=cost * qty`"), "{declared}");
        let bare = said(hover(&folder, &file, at(text, "total\n", 0)).unwrap());
        assert_eq!(declared, bare);
        // Go to definition: the header's name to its declaration, and back.
        let to = definition(&folder, &file, at(text, "total\n", 0)).unwrap();
        assert_eq!(to.range, lines.range(span(at(text, "total:", 0) as u32, 5)));
        let back = definition(&folder, &file, at(text, "total:", 0)).unwrap();
        assert_eq!(back.range, lines.range(span(at(text, "total\n", 0) as u32, 5)));
        // A formula's name in a declaration goes to the column it reads.
        let qty = definition(&folder, &file, at(text, "qty\n---", 0)).unwrap();
        assert_eq!(qty.range, lines.range(span(at(text, "qty,", 0) as u32, 3)));
        // A rename follows the column into its declaration and every formula naming it.
        let edit = rename(&folder, &file, at(text, "qty,", 0), "count").unwrap();
        let mut edits = edit.changes.unwrap().into_values().next().unwrap();
        edits.sort_by_key(|e| (e.range.start.line, e.range.start.character));
        let renamed: Vec<(u32, &str)> = edits.iter().map(|e| (e.range.start.line, e.new_text.as_str())).collect();
        assert_eq!(renamed, [(1, "count"), (2, "count"), (4, "count")]);
        // Completion in a declared formula offers the columns.
        let items = complete(&folder, &file, at(text, "cost * qty", 0));
        assert!(items.iter().any(|i| i.label == "cost"), "{:?}", items.iter().map(|i| &i.label).collect::<Vec<_>>());
    }

    #[test]
    fn a_broken_header_cell_is_marked_and_the_rest_answers() {
        let (folder, _, money) = books("broken");
        let text = MONEY.replace("to:accounts.account", "to:accounts.a");
        let folder = Folder { open: Arc::new(BTreeMap::from([(money.clone(), text.clone())])), ..folder };
        let file = folder.read(&money).unwrap();
        let d = diagnostics(&file, &Allowed::default());
        assert_eq!(d[0].message, "to: accounts has no column \"a\" (its key is account)");
        assert_eq!(d[0].range, Lines::new(&text).range(span(at(&text, "accounts.a,", 0) as u32, 10)));
        assert!(hover(&folder, &file, at(&text, "950", 0)).is_some());
        let to = said(hover(&folder, &file, at(&text, "to:", 0)).unwrap());
        assert_eq!(to, "`to:text?`\n\nto: accounts has no column \"a\" (its key is account)");
    }

    #[test]
    fn completion() {
        let (folder, _, money) = books("complete");
        let file = folder.read(&money).unwrap();
        let labels = |items: Vec<CompletionItem>| items.into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(labels(complete(&folder, &file, at(MONEY, "pwer", 0) + 2)), ["bank", "rent", "power"]);
        let formula = labels(complete(&folder, &file, at(MONEY, "amount * 2", 0) + 2));
        assert!(formula.starts_with(&["id".into(), "amount".into(), "to".into()]) && formula.contains(&"accounts".into()), "{formula:?}");
        assert!(formula.contains(&"SUM".into()) && formula.contains(&"TABLE".into()) && formula.contains(&"CASE".into()), "{formula:?}");
        // After a column's `[`: the rows by where they are.
        let text = MONEY.replace("amount * 2", "amount[");
        let folder2 = Folder { open: Arc::new(BTreeMap::from([(money.clone(), text.clone())])), ..folder.clone() };
        let f2 = folder2.read(&money).unwrap();
        let places = labels(complete(&folder2, &f2, at(&text, "amount[", 0) + "amount[".len()));
        assert!(places.contains(&"-1".into()) && places.contains(&"FIRST:THIS".into()), "{places:?}");
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
        assert_eq!(rent.uri, folder.uri(&accounts));
        assert_eq!(rent.range, Lines::new(ACCOUNTS).range(span(at(ACCOUNTS, "rent", 0) as u32, 4)));
        let typed = definition(&folder, &file, at(MONEY, "accounts.account", 0) + 10).unwrap();
        assert_eq!(typed.range, Lines::new(ACCOUNTS).range(span(0, "account:text!".len() as u32)));
        let amount = definition(&folder, &file, at(MONEY, "amount * 2", 0)).unwrap();
        assert_eq!((amount.uri, amount.range.start.line), (folder.uri(&money), 3));
        let import = definition(&folder, &file, at(MONEY, "import", 0)).unwrap();
        assert_eq!(import.uri, folder.uri(&accounts));
    }

    /// A folder the editor opened through a link (`/tmp` on a Mac) is answered in the
    /// link's spelling, the one file the editor knows by that name.
    #[test]
    fn a_uri_is_spelled_as_the_editor_opened_it() {
        let (folder, accounts, money) = books("spelled");
        let root = folder.root.clone().unwrap();
        let link = root.with_file_name(format!("{}-link", root.file_name().unwrap().to_str().unwrap()));
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(&root, &link).unwrap();
        let spelled = spelling(&link.join("home")).expect("a link is not its canonical path");
        let folder = Folder { spelled: Arc::new(BTreeMap::from([spelled])), ..folder };
        assert_eq!(folder.uri(&money), Url::from_file_path(link.join("home/money.cells")).unwrap());
        assert_eq!(folder.uri(&accounts), Url::from_file_path(&accounts).unwrap(), "outside the spelled directory");
        let file = folder.read(&money).unwrap();
        let amount = definition(&folder, &file, at(MONEY, "amount * 2", 0)).unwrap();
        assert_eq!(amount.uri, Url::from_file_path(link.join("home/money.cells")).unwrap());
        fs::remove_file(&link).unwrap();
    }

    #[test]
    fn a_row_by_its_number_is_written_as_its_key() {
        let (folder, _, money) = books("numbered");
        let text = MONEY.replace("amount * 2", "TABLE[#2].amount + TABLE[");
        let folder = Folder { open: Arc::new(BTreeMap::from([(money.clone(), text.clone())])), ..folder };
        let file = folder.read(&money).unwrap();
        // The file holds it, so the formula breaks, and the action writes the key.
        assert!(diagnostics(&file, &Allowed::default()).iter().any(|d| d.message.starts_with("double: #2 is a row by where it shows")));
        let header = Lines::new(&text).position(at(&text, "#2", 0)).line;
        let line = Range {
            start: tower_lsp::lsp_types::Position { line: header, character: 0 },
            end: tower_lsp::lsp_types::Position { line: header, character: 0 },
        };
        let a = actions(&folder, &file, line, &Allowed::default());
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].title, "#2 is the row 'a2': write its key");
        let edit = &a[0].edit.as_ref().unwrap().changes.as_ref().unwrap()[&folder.uri(&money)][0];
        assert_eq!((edit.range, edit.new_text.as_str()), (Lines::new(&text).range(span(at(&text, "#2", 0) as u32, 2)), "'a2'"));
        // Inside `TABLE[`: the rows, by key, found by number.
        let rows = complete(&folder, &file, at(&text, "TABLE[", 1) + "TABLE[".len());
        let labels: Vec<(&str, Option<&str>)> = rows.iter().map(|i| (i.label.as_str(), i.filter_text.as_deref())).collect();
        assert_eq!(labels, [("'a1'", Some("#1 a1")), ("'a2'", Some("#2 a2")), ("'a3'", Some("#3 a3"))]);
    }

    /// The text `edit`, one edit over the whole file, leaves.
    fn applied(edit: &TextEdit) -> &str {
        edit.new_text.as_str()
    }

    #[test]
    fn saving_stores_what_the_edits_reached() {
        let (folder, _, money) = books("format");
        let saved = folder.read(&money).unwrap();
        // An amount changed in the editor: its double is reached, and stored on save.
        let text = MONEY.replace("64.30,pwer,,128.60=", "70.00,pwer,,128.60=");
        let folder = Folder { open: Arc::new(BTreeMap::from([(money.clone(), text.clone())])), ..folder };
        let now = folder.read(&money).unwrap();
        let edit = formatted(&now, Some(&saved), false).expect("a value to store");
        assert!(applied(&edit).contains("70.00,pwer,,140.00="), "{}", applied(&edit));
        assert!(applied(&edit).contains("950.00,rent,,1900.00="), "the row not reached is as it was");
        // Aligned, the same values, each table's columns laid out as `cells fmt` lays them.
        let aligned = formatted(&now, Some(&saved), true).expect("a value to store, the table aligned");
        assert_eq!(applied(&aligned), cells::layout::formatted(applied(&edit), true));
        // A value typed over its `=` is left for a person: a stale cell, not overwritten.
        let typed = MONEY.replace("1900.00=", "1999.00=");
        let folder = Folder { open: Arc::new(BTreeMap::from([(money.clone(), typed.clone())])), ..folder };
        let now = folder.read(&money).unwrap();
        assert!(formatted(&now, Some(&saved), false).is_none(), "nothing reached it");
        // Its two fixes.
        let lines = Lines::new(&typed);
        let row = lines.position(at(&typed, "1999.00=", 0)).line;
        let on = Range {
            start: tower_lsp::lsp_types::Position { line: row, character: 0 },
            end: tower_lsp::lsp_types::Position { line: row, character: 0 },
        };
        let a = actions(&folder, &now, on, &Allowed::default());
        let titles: Vec<&str> = a.iter().map(|a| a.title.as_str()).collect();
        assert!(titles.contains(&"Use the formula's value, 1900.00"), "{titles:?}");
        assert!(titles.contains(&"Keep 1999.00 as this row's override"), "{titles:?}");
        let keep = a.iter().find(|a| a.title.starts_with("Keep")).unwrap();
        let edit = &keep.edit.as_ref().unwrap().changes.as_ref().unwrap()[&folder.uri(&money)][0];
        assert!(applied(edit).contains("950.00,rent,,1999.00\n"), "{}", applied(edit));
    }

    #[test]
    fn a_formula_is_written_as_the_file_holds_it() {
        let (folder, _, money) = books("sheet");
        let text = MONEY.replace("amount * 2", "sum(amount) + B2");
        let folder = Folder { open: Arc::new(BTreeMap::from([(money.clone(), text.clone())])), ..folder };
        let file = folder.read(&money).unwrap();
        let header = Lines::new(&text).position(at(&text, "sum(", 0)).line;
        let on = Range {
            start: tower_lsp::lsp_types::Position { line: header, character: 0 },
            end: tower_lsp::lsp_types::Position { line: header, character: 0 },
        };
        let a = actions(&folder, &file, on, &Allowed::default());
        let titles: Vec<&str> = a.iter().map(|a| a.title.as_str()).collect();
        assert!(titles.contains(&"Write in capitals: SUM"), "{titles:?}");
        assert!(titles.iter().any(|t| t.contains("in names: sum(amount) + amount")), "{titles:?}");
    }

    #[test]
    fn a_key_renamed_follows_every_reference() {
        let (folder, accounts, money) = books("rename-key");
        let file = folder.read(&accounts).unwrap();
        let e = rename(&folder, &file, at(ACCOUNTS, "rent", 0), "lease").unwrap();
        let changes = e.changes.unwrap();
        let texts = |u: &Path| changes[&folder.uri(u)].iter().map(|t| t.new_text.as_str()).collect::<Vec<_>>();
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
        assert_eq!(changes[&folder.uri(&money)].len(), 1, "the formula reading accounts['rent'].kind");
        let file = folder.read(&money).unwrap();
        let e = rename(&folder, &file, at(MONEY, "amount", 0), "sum_paid").unwrap();
        let edits = &e.changes.unwrap()[&folder.uri(&money)];
        assert_eq!(edits.len(), 2, "the header cell and the formula reading it");
        assert!(rename(&folder, &file, at(MONEY, "amount", 0), "two words").is_err());
    }

    #[test]
    fn a_script_not_allowed_warns_and_its_fix_allows_the_version_shown() {
        let (folder, _, money) = books("allow");
        let rates = money.with_file_name("rates.cells");
        fs::write(
            &rates,
            "---\nrates = run every hour\n```ts\nexport default async () => []\n```\n```cells\nday:date!,rate:num\n```\n---\na\n1\n",
        )
        .unwrap();
        let file = folder.read(&rates).unwrap();
        let warned: Vec<_> =
            diagnostics(&file, &Allowed::default()).into_iter().filter(|d| d.message.starts_with("rates does not run")).collect();
        assert_eq!(warned.len(), 1);
        assert_eq!((warned[0].range.start.line, warned[0].severity), (1, Some(DiagnosticSeverity::WARNING)));
        let fixes = actions(&folder, &file, warned[0].range, &Allowed::default());
        let fix = fixes.iter().find(|a| a.title == "Allow rates to run, as written").expect("its fix");
        let command = fix.command.as_ref().unwrap();
        assert_eq!(command.command, ALLOW);
        let version = cells_daemon::allowed::version(&file.doc.scripts()[0]);
        assert_eq!(command.arguments.as_ref().unwrap()[2], LSPAny::String(version));
        assert!(fix.edit.is_none(), "allowing edits no file");
    }
}
