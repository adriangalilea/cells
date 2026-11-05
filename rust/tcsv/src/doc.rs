//! A `.tcsv` file read: its source kept whole, its columns decoded, its problems by place,
//! and the edits that `write` lays over the source so every unchanged byte stays as read.

use alloc::borrow::{Cow, ToOwned};
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;

use crate::column::{self, Bits, BitsBuilder, Building, Data, Decoder, Kind, Problem, Run, Value, Why, infer};
use crate::exact::Exact;
use crate::expr::{self, Ir, K, V};
use crate::format::{Dialect, Separator};
use crate::prelude::{self, Fenced, Schedule, Statement};
use crate::read::{self, Cells, Part, Reading, Said};
use crate::scan::{self, Fields, Records, Stop};
use crate::types::{self, Base, NamedType, Type};
use crate::value::{self, Decimal, Instant};

/// Where a file's grammar breaks, in the suite's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At {
    /// The header line.
    Header,
    /// A header cell, 1-based.
    Column(u32),
    /// A prelude line, 1-based inside the fences.
    PreludeLine(u32),
    /// The prelude as a whole.
    Prelude,
    /// A data row, from 0.
    Row(u32),
    /// The file as a whole (larger than 4 GiB).
    File,
}

impl fmt::Display for At {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            At::Header => f.write_str("the header"),
            At::Column(n) => write!(f, "column {n}"),
            At::PreludeLine(n) => write!(f, "prelude line {n}"),
            At::Prelude => f.write_str("the prelude"),
            At::Row(n) => write!(f, "row {n}"),
            At::File => f.write_str("the file"),
        }
    }
}

/// A file that does not read: its grammar is broken at `at`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub at: At,
    pub why: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.at, self.why)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

fn error(at: At, why: impl Into<String>) -> Error {
    Error { at, why: why.into() }
}

/// A character by its code point, as a message shows one that renders as nothing: `U+0000`.
fn code_point(point: u32) -> String {
    format!("U+{point:04X}")
}

/// Text for a cell a writer writes, refused at `at` when it holds a control character.
fn writable(text: &str, at: At) -> Result<(), Error> {
    match column::control(text.as_bytes()) {
        Some(point) => Err(error(at, format!("{} is a control character, which no cell holds", code_point(point)))),
        None => Ok(()),
    }
}

fn stop_why(s: Stop, header_fields: usize) -> String {
    match s {
        Stop::Unclosed => "a quoted field is never closed".to_owned(),
        Stop::AfterQuote => "text follows a quoted field's closing quote".to_owned(),
        Stop::Quote => "a \" inside a field that did not open with one".to_owned(),
        Stop::Return => "a \\r with no \\n after it".to_owned(),
        Stop::TooMany => format!("more fields than the header's {header_fields}"),
        Stop::Irregular => unreachable!("an irregular block is read again, never reported"),
    }
}

/// Where a cell's value comes from; a byte, 0 to 2 in this order, where a binding reads
/// them in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Origin {
    /// Its text: written, or in a column with a formula, the row's override.
    Written,
    /// Its column's formula, its field left blank.
    Formula,
    /// Its own formula, `` `=…` ``.
    Cell,
}

/// A column, its header cell's and its cells'.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub name: String,
    /// What the header cell spells after its `:`, its formula included; nothing for a
    /// plain column.
    pub spelled: Option<String>,
    pub ty: Type,
    /// The column's formula, as the header cell writes it after the type's `=`.
    pub formula: Option<String>,
    pub data: Data,
    /// Set where the cell has a value; clear for a blank or broken cell.
    pub valid: Bits,
    /// Each cell's origin, when any is computed; empty when every cell is written.
    pub origin: Arc<Vec<Origin>>,
}

impl Column {
    /// Where the value at `row` comes from.
    pub fn origin(&self, row: usize) -> Origin {
        self.origin.get(row).copied().unwrap_or(Origin::Written)
    }

    pub fn len(&self) -> usize {
        self.valid.len
    }

    pub fn is_empty(&self) -> bool {
        self.valid.len == 0
    }

    pub fn nulls(&self) -> usize {
        self.valid.zeros()
    }

    /// The value at `row`, null for a blank or broken cell.
    pub fn value(&self, row: usize) -> Value<'_> {
        if !self.valid.get(row) {
            return Value::Null;
        }
        match &self.data {
            Data::Text { offsets, bytes } => {
                let s = &bytes[offsets[row] as usize..offsets[row + 1] as usize];
                // Copied from the source, a `str`, at field boundaries.
                Value::Text(core::str::from_utf8(s).unwrap())
            }
            Data::Int(v) => Value::Int(v[row]),
            Data::Decimal { values, scale } => Value::Decimal(Decimal { mantissa: values[row], scale: *scale }),
            Data::Bool(b) => Value::Bool(b.get(row)),
            Data::Date(v) => Value::Date(v[row]),
            Data::Time(v) => Value::Time(v[row]),
            Data::DateTime { values, unit } => Value::DateTime(Instant::from(values[row], *unit)),
            Data::Choice(v) => match &self.ty.base {
                Base::Choice(words) => Value::Choice(&words[v[row] as usize]),
                _ => unreachable!("a choice column's type is a choice"),
            },
        }
    }

    /// A value as this column writes it: its pair's words, its format, ISO otherwise, and
    /// text that would read as a formula with its first backtick doubled.
    pub fn form(&self, v: Value) -> String {
        match v {
            Value::Null => String::new(),
            Value::Text(t) | Value::Choice(t) => escaped(t),
            Value::Int(n) => n.to_string(),
            Value::Decimal(d) => d.to_string(),
            Value::Bool(b) => match &self.ty.base {
                Base::Bool { yes, no } => {
                    if b {
                        yes.clone()
                    } else {
                        no.clone()
                    }
                }
                _ => b.to_string(),
            },
            Value::Date(d) => match &self.ty.format {
                Some(f) => f.write(Some(d), None),
                None => value::format_date(d),
            },
            Value::Time(t) => match &self.ty.format {
                Some(f) => f.write(None, Some(t)),
                None => value::format_time(t),
            },
            Value::DateTime(t) => value::format_datetime(t),
        }
    }
}

/// A row of a file being rearranged (`Doc::rearranged`): one it has, or a new one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row<'a> {
    Kept(usize),
    New(&'a [&'a str]),
}

/// A column of a file being reshaped (`Doc::reshaped`): a stored one it has, as it is or
/// renamed, or a new one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnAt<'a> {
    Kept(usize),
    /// A stored column under a new name, its type kept.
    Renamed(usize, &'a str),
    /// A new column: its header cell (`name`, or in a `.tcsv` file `name:type`) and its
    /// cells, one a row, or none for a column of blanks.
    New(&'a str, &'a [&'a str]),
}

/// Text as a cell writes it: as it is, or, when it would read as a formula (`` `=x` ``),
/// its first backtick doubled.
fn escaped(t: &str) -> String {
    match read::said(t.as_bytes()) {
        Said::Plain => t.to_owned(),
        _ => format!("`{t}"),
    }
}

/// A table the prelude names: written inline, filled by a script, or another file's.
#[derive(Clone, Debug)]
pub struct Table {
    pub name: String,
    pub exported: bool,
    /// The table read; nothing for an import that could not be reached, whose cells and
    /// formulas read as unchecked.
    pub doc: Option<Arc<Doc>>,
    /// Its prelude line.
    pub line: u32,
    /// Written in this file (inline, or by a script), not imported: its problems are this
    /// file's.
    pub own: bool,
}

/// A table a script fills (`name = run …`): its schedule, its code, and where the
/// engine's blocks under it lie in the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Script {
    pub name: String,
    pub line: u32,
    pub schedule: Schedule,
    pub language: String,
    /// The script's text.
    pub code: String,
    /// Where the next block would be written: the end of the code's fence or of the
    /// last block after it, the line break included.
    pub after: usize,
    pub stored: Option<Fenced>,
    pub runs: Option<Fenced>,
}

/// What an import is, for a host that follows it (`read_with`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    /// As written: a path relative to the importing file, or a URL.
    pub from: String,
    /// `with { sha256: "…", rev: "…" }`.
    pub with: Vec<(String, String)>,
}

/// How a reader follows imports: the file an import names read, or nothing when it cannot
/// be reached (the importing file still reads, what it takes from it unchecked), or why it
/// is refused (a hash that does not match, a cycle).
pub trait Imports {
    fn import(&mut self, import: &Import) -> Result<Option<Doc>, String>;
}

/// Imports never followed: a file read alone.
pub struct Alone;

impl Imports for Alone {
    fn import(&mut self, _: &Import) -> Result<Option<Doc>, String> {
        Ok(None)
    }
}

/// The imports a file followed when it was read, given again as they were.
struct Again<'a> {
    imported: &'a [(Import, Option<Arc<Doc>>)],
}

impl Imports for Again<'_> {
    fn import(&mut self, i: &Import) -> Result<Option<Doc>, String> {
        Ok(self.imported.iter().find(|(j, _)| j == i).and_then(|(_, d)| d.as_deref().cloned()))
    }
}

/// A plain file's header cell that names its column badly. TCSV refuses both (SPEC.md, A
/// header cell); a plain file reads with them, so the tool that can fix them shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Naming {
    /// The cell is blank: the column has no name.
    Blank { column: u32 },
    /// The column `first` has the name already.
    Twice { column: u32, first: u32 },
}

impl Naming {
    pub fn column(self) -> u32 {
        match self {
            Naming::Blank { column } | Naming::Twice { column, .. } => column,
        }
    }
}

/// A file read: a `.tcsv` file, or a plain one (`Dialect`).
#[derive(Clone, Debug)]
pub struct Doc {
    dialect: Dialect,
    /// A plain file whose header and first row quote every field: an edited cell and a new
    /// row are quoted so too.
    quotes_all: bool,
    /// A plain file's header cells that name their columns badly.
    naming: Vec<Naming>,
    source: String,
    /// Where the header starts: everything before it (a byte order mark, the prelude) is
    /// kept as written.
    body: u32,
    /// Where each row's record starts, and where the last one ends.
    records: Vec<u32>,
    statements: Vec<Statement>,
    named: Vec<NamedType>,
    tables: Vec<Table>,
    scripts: Vec<Script>,
    /// Every import and what following it gave, so the file read again after an edit reads
    /// them the same.
    imported: Vec<(Import, Option<Arc<Doc>>)>,
    columns: Vec<Column>,
    /// Each column's formula, resolved.
    formulas: Vec<Option<Ir>>,
    problems: Vec<Problem>,
    /// Why a cell's own formula does not compute, by place.
    unwell: BTreeMap<(u32, u32), String>,
    /// What a formula gave a cell whose type it breaks, by place, so its problem can say.
    gave: BTreeMap<(u32, u32), String>,
    /// The header line rewritten, without its line break.
    header: Option<String>,
    /// Rewritten rows' records, without their line breaks.
    edits: BTreeMap<u32, String>,
    /// What every stored column's values could be (`infer`), so a tail read later says
    /// whether an inferred type still holds.
    candidates: Vec<u8>,
}

/// Reads a `.tcsv` file. A broken cell is a problem and the file still reads; a broken
/// grammar (an unknown type, a row with more fields than the header, a malformed prelude)
/// is an `Error` naming where, and so are bytes that are not UTF-8. The bytes are kept,
/// never copied. One thread: the core starts none unless asked.
pub fn read(source: impl Into<Vec<u8>>) -> Result<Doc, Error> {
    read_on(source.into(), Dialect::Tcsv, 1)
}

/// `read` on up to `threads` threads, for a large input: the body is split at record
/// boundaries and its parts read at once. The result is `read`'s, byte for byte.
#[cfg(feature = "std")]
pub fn read_parallel(source: impl Into<Vec<u8>>, threads: usize) -> Result<Doc, Error> {
    read_on(source.into(), Dialect::Tcsv, threads)
}

/// A file read as `dialect` on up to `threads` threads (1 starts none). A plain file is
/// refused only for bytes that are not UTF-8, or past 4 GiB.
pub fn read_as(source: impl Into<Vec<u8>>, dialect: Dialect, threads: usize) -> Result<Doc, Error> {
    #[cfg(not(feature = "std"))]
    assert_eq!(threads, 1, "threads need the std feature");
    read_on(source.into(), dialect, threads)
}

/// `read` with its imports followed by `imports`, on up to `threads` threads (1 starts
/// none): what it takes from another file read there, its references checked and its
/// formulas computed against it. A file whose imports cannot be reached reads as `read`
/// reads it.
pub fn read_with(source: impl Into<Vec<u8>>, imports: &mut dyn Imports, threads: usize) -> Result<Doc, Error> {
    #[cfg(not(feature = "std"))]
    assert_eq!(threads, 1, "threads need the std feature");
    read_in(source.into(), Dialect::Tcsv, threads, imports, None)
}

fn read_on(bytes: Vec<u8>, dialect: Dialect, threads: usize) -> Result<Doc, Error> {
    read_in(bytes, dialect, threads, &mut Alone, None)
}

/// A table written in a prelude takes the types and the tables the prelude named above it.
type Above<'a> = (&'a [NamedType], &'a [Table]);

fn read_in(bytes: Vec<u8>, dialect: Dialect, threads: usize, imports: &mut dyn Imports, above: Option<Above>) -> Result<Doc, Error> {
    let source = String::from_utf8(bytes)
        .map_err(|e| error(At::File, format!("is not UTF-8: byte {} begins no character", e.utf8_error().valid_up_to())))?;
    if source.len() >= u32::MAX as usize {
        return Err(error(At::File, "is larger than 4 GiB"));
    }
    let bom = if source.starts_with('\u{feff}') { 3 } else { 0 };
    let plain = matches!(dialect, Dialect::Plain(_));
    let (mut named, mut tables) = match above {
        Some((n, t)) => (n.to_vec(), t.to_vec()),
        None => (Vec::new(), Vec::new()),
    };
    let (body, statements) = if plain || above.is_some() { (bom, Vec::new()) } else { read_prelude(&source, bom)? };
    let (scripts, imported) = settle_prelude(&source, &statements, imports, &mut named, &mut tables)?;
    let how = dialect.records();
    let src = source.as_bytes();
    let mut doc = Doc {
        dialect,
        quotes_all: false,
        naming: Vec::new(),
        source: String::new(),
        body: body as u32,
        records: Vec::from([body as u32]),
        statements,
        named,
        tables,
        scripts,
        imported,
        columns: Vec::new(),
        formulas: Vec::new(),
        problems: Vec::new(),
        unwell: BTreeMap::new(),
        gave: BTreeMap::new(),
        header: None,
        edits: BTreeMap::new(),
        candidates: Vec::new(),
    };
    if body == src.len() {
        doc.source = source;
        return Ok(doc);
    }
    let (fields, first) = scan::record(src, body, how).map_err(|s| error(At::Header, stop_why(s, 0)))?;
    let mut scratch = Vec::new();
    let names: Vec<String> = doc.tables.iter().map(|t| t.name.clone()).collect();
    let mut heads: Vec<Head> = Vec::with_capacity(fields.len());
    let mut naming = Vec::new();
    for (j, &(s, e)) in fields.iter().enumerate() {
        let at = At::Column(j as u32 + 1);
        let cell = scan::text(src, s, e, &mut scratch);
        // Unquoting cuts at ASCII quotes only, so a field of a `str` is one.
        let cell = core::str::from_utf8(cell).unwrap();
        if !plain && let Some(point) = column::control(cell.as_bytes()) {
            return Err(error(at, format!("holds {}, a control character", code_point(point))));
        }
        if plain {
            // A plain file's header cell is its column's name, whatever it holds.
            let column = j as u32;
            if cell.is_empty() {
                naming.push(Naming::Blank { column });
            } else if let Some(first) = heads.iter().position(|h| h.name == cell) {
                naming.push(Naming::Twice { column, first: first as u32 });
            }
            heads.push(Head { name: cell.to_owned(), spelled: None, ty: None, formula: None });
            continue;
        }
        let head = head(cell, &doc.named, &names).map_err(|why| error(at, why))?;
        if heads.iter().any(|h| h.name == head.name) {
            return Err(error(at, format!("two columns are named {}", head.name)));
        }
        heads.push(head);
    }
    let quoted = |fields: &[(u32, u32)]| fields.iter().all(|&(s, e)| s < e && src[s as usize] == b'"');
    doc.quotes_all = plain && quoted(&fields) && (first == src.len() || quoted(&scan::record(src, first, how).unwrap().0));
    doc.naming = naming;
    let decoders = decoders(heads.iter().map(|h| (h.ty.as_ref(), h.formula.is_some())));
    let parts = read::body(src, first, &decoders, how, threads).map_err(|(row, s)| error(At::Row(row), stop_why(s, heads.len())))?;
    let mut part = read::join(parts);
    part.records.push(src.len() as u32);
    let Part { records, columns: readings, mut problems, .. } = part;
    doc.candidates = readings.iter().map(|r| r.candidates).collect();
    let rows = records.len() - 1;
    let origins = origins(&mut problems, heads.len(), rows);
    for (j, ((h, r), origin)) in heads.into_iter().zip(readings).zip(origins).enumerate() {
        let (ty, data, valid) = settle(h.ty, r, j as u32, threads, &mut problems);
        doc.columns.push(Column { name: h.name, spelled: h.spelled, ty, formula: h.formula, data, valid, origin: Arc::new(origin) });
    }
    doc.records = records;
    doc.problems = problems;
    doc.source = source;
    doc.check_targets()?;
    doc.resolve_formulas()?;
    doc.recompute();
    Ok(doc)
}

/// A header cell read: its name, its type and its formula.
struct Head {
    name: String,
    spelled: Option<String>,
    ty: Option<Type>,
    formula: Option<String>,
}

/// A `.tcsv` header cell: `name`, `name:type`, `name:type=formula`.
fn head(cell: &str, named: &[NamedType], tables: &[String]) -> Result<Head, String> {
    let (name, spelled) = types::split_header(cell);
    if name.is_empty() {
        return Err("a column has a name".to_owned());
    }
    let Some(spelled) = spelled else { return Ok(Head { name: name.to_owned(), spelled: None, ty: None, formula: None }) };
    let (ty, formula) = types::split_formula(spelled);
    let formula = match formula.map(|f| f.trim_matches([' ', '\t'])) {
        Some("") => return Err("a formula follows the =".to_owned()),
        f => f.map(str::to_owned),
    };
    let ty = types::parse_type(ty, named, tables)?;
    Ok(Head { name: name.to_owned(), spelled: Some(spelled.to_owned()), ty: Some(ty), formula })
}

fn decoders<'a>(heads: impl Iterator<Item = (Option<&'a Type>, bool)>) -> Vec<Cells> {
    heads.map(|(t, computed)| Cells { decoder: Decoder::of(t), optional: t.is_none_or(|t| t.optional), computed }).collect()
}

/// Each column's cells' origins, from the marks the read left (`Why::Computed`,
/// `Why::Formula`), taken out of the problems: empty for a column every cell of which is
/// written.
fn origins(problems: &mut Vec<Problem>, columns: usize, rows: usize) -> Vec<Vec<Origin>> {
    let mut out: Vec<Vec<Origin>> = (0..columns).map(|_| Vec::new()).collect();
    problems.retain(|p| {
        let o = match p.why {
            Why::Computed => Origin::Formula,
            Why::Formula => Origin::Cell,
            _ => return true,
        };
        let col = &mut out[p.column as usize];
        if col.is_empty() {
            col.resize(rows, Origin::Written);
        }
        col[p.row as usize] = o;
        false
    });
    out
}

/// The prelude, if the file opens with one after its byte order mark: its statements, and
/// where the header starts.
fn read_prelude(source: &str, from: usize) -> Result<(usize, Vec<Statement>), Error> {
    let first = source[from..].split_inclusive('\n').next().unwrap_or("");
    if first.strip_suffix('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)) != Some("---") {
        if first == "---" {
            return Err(error(At::Prelude, "opens with --- and never closes"));
        }
        return Ok((from, Vec::new()));
    }
    let (statements, body) = prelude::parse(source, from).map_err(|(n, why)| match n {
        0 => error(At::Prelude, why),
        n => error(At::PreludeLine(n), why),
    })?;
    for (n, l) in source[from..body].split_inclusive('\n').enumerate().skip(1) {
        if let Some(point) = column::control(l.trim_end_matches(['\r', '\n']).as_bytes()) {
            return Err(error(At::PreludeLine(n as u32), format!("holds {}, a control character", code_point(point))));
        }
    }
    Ok((body, statements))
}

fn is_url(from: &str) -> bool {
    from.starts_with("https://") || from.starts_with("http://")
}

/// What a prelude settles to besides its types and tables: its scripts, and each import
/// with what following it gave.
type Settled = (Vec<Script>, Vec<(Import, Option<Arc<Doc>>)>);

/// The prelude's statements settled in order: each type parsed, each import followed,
/// each table read, every name once.
fn settle_prelude(
    source: &str,
    statements: &[Statement],
    imports: &mut dyn Imports,
    named: &mut Vec<NamedType>,
    tables: &mut Vec<Table>,
) -> Result<Settled, Error> {
    let mut scripts = Vec::new();
    let mut imported = Vec::new();
    let taken =
        |named: &[NamedType], tables: &[Table], name: &str| named.iter().any(|t| t.name == name) || tables.iter().any(|t| t.name == name);
    for s in statements {
        let at = At::PreludeLine(s.line());
        let fail = |why: String| error(at, why);
        match s {
            Statement::Type { name, spelled, .. } => {
                if !name.as_bytes().first().is_some_and(|c| c.is_ascii_uppercase()) {
                    return Err(fail(format!("a type's name starts with a capital letter, not \"{name}\"")));
                }
                if types::currency(name).is_some() {
                    return Err(fail(format!("{name} is a currency")));
                }
                if taken(named, tables, name) {
                    return Err(fail(format!("the type {name} is named twice")));
                }
                let names: Vec<String> = tables.iter().map(|t| t.name.clone()).collect();
                let ty = types::parse_type(spelled, named, &names).map_err(fail)?;
                named.push(NamedType { name: name.clone(), ty });
            }
            Statement::Import { default, names, from, with, .. } => {
                if is_url(from) && !with.iter().any(|(k, _)| k == "sha256") {
                    return Err(fail(format!("\"{from}\" is pinned by its hash: with {{ sha256: \"…\" }}, which tcsv pin writes")));
                }
                let import = Import { from: from.clone(), with: with.clone() };
                let doc = imports.import(&import).map_err(|why| fail(format!("\"{from}\": {why}")))?;
                let doc = doc.map(Arc::new);
                imported.push((import, doc.clone()));
                if let Some(name) = default {
                    if taken(named, tables, name) {
                        return Err(fail(format!("the name {name} is taken")));
                    }
                    tables.push(Table { name: name.clone(), exported: false, doc: doc.clone(), line: s.line(), own: false });
                }
                for name in names {
                    if taken(named, tables, name) {
                        return Err(fail(format!("the name {name} is taken")));
                    }
                    let Some(d) = &doc else {
                        // Unreachable: a capital names a type, read as unchecked text.
                        if name.as_bytes()[0].is_ascii_uppercase() {
                            named.push(NamedType { name: name.clone(), ty: Type::plain(Base::Text) });
                        } else {
                            tables.push(Table { name: name.clone(), exported: false, doc: None, line: s.line(), own: false });
                        }
                        continue;
                    };
                    if let Some(t) = d.exported_type(name) {
                        named.push(NamedType { name: name.clone(), ty: t.ty.clone() });
                    } else if let Some(t) = d.tables.iter().find(|t| t.exported && &t.name == name) {
                        tables.push(Table { name: name.clone(), exported: false, doc: t.doc.clone(), line: s.line(), own: false });
                    } else {
                        let all = d.exports();
                        let shown = if all.is_empty() { "nothing".to_owned() } else { all.join(", ") };
                        return Err(fail(format!("\"{from}\" exports no \"{name}\" (it exports: {shown})")));
                    }
                }
            }
            Statement::Table { name, exported, body, .. } => {
                if taken(named, tables, name) {
                    return Err(fail(format!("the name {name} is taken")));
                }
                let doc = inline(source, name, body, imports, named, tables)?;
                tables.push(Table { name: name.clone(), exported: *exported, doc: Some(Arc::new(doc)), line: s.line(), own: true });
            }
            Statement::Run { name, exported, schedule, code, stored, runs, .. } => {
                if taken(named, tables, name) {
                    return Err(fail(format!("the name {name} is taken")));
                }
                let Some(stored) = stored else {
                    return Err(fail(format!("{name}'s rows are declared under its script: a ```tcsv block, its header at least")));
                };
                let doc = inline(source, name, stored, imports, named, tables)?;
                if doc.columns.is_empty() {
                    return Err(error(At::PreludeLine(stored.line), format!("{name}'s ```tcsv block holds its header at least")));
                }
                if let Some(r) = runs {
                    inline(source, name, r, imports, named, tables)?;
                }
                let after = runs.as_ref().unwrap_or(stored).end;
                let after = after + source[after..].find('\n').map_or(source.len() - after, |n| n + 1);
                tables.push(Table { name: name.clone(), exported: *exported, doc: Some(Arc::new(doc)), line: s.line(), own: true });
                scripts.push(Script {
                    name: name.clone(),
                    line: s.line(),
                    schedule: schedule.clone(),
                    language: code.lang.clone(),
                    code: source[code.start..code.end].to_owned(),
                    after,
                    stored: Some(stored.clone()),
                    runs: runs.clone(),
                });
            }
        }
    }
    Ok((scripts, imported))
}

/// A table fenced in the prelude, read with what the prelude named above it. An error
/// inside it names the prelude line it is on.
fn inline(source: &str, name: &str, body: &Fenced, imports: &mut dyn Imports, named: &[NamedType], tables: &[Table]) -> Result<Doc, Error> {
    let text = &source[body.start..body.end];
    read_in(text.as_bytes().to_vec(), Dialect::Tcsv, 1, imports, Some((named, tables))).map_err(|e| {
        let line = |offset: usize| body.line + 1 + text[..offset.min(text.len())].matches('\n').count() as u32;
        let n = match e.at {
            At::Header | At::Column(_) | At::File | At::Prelude => body.line + 1,
            At::PreludeLine(n) => body.line + n,
            At::Row(r) => {
                let starts: Vec<usize> = core::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
                line(starts.get(r as usize + 1).copied().unwrap_or(text.len()))
            }
        };
        error(At::PreludeLine(n), format!("{name}: {}", e.why))
    })
}

/// A column's reading made final: an untyped column's type inferred from its values and
/// its cells decoded by it, and a key's duplicates reported.
fn settle(ty: Option<Type>, r: Reading, column: u32, threads: usize, problems: &mut Vec<Problem>) -> (Type, Data, Bits) {
    let Reading { building, valid, candidates, .. } = r;
    let (ty, building, valid) = match ty {
        Some(ty) => (ty, building, valid),
        None => {
            let Building::Text { offsets, bytes } = building else { unreachable!("an untyped column reads as text") };
            let blank = (0..valid.len).all(|i| !valid.get(i));
            let base = if blank { Base::Text } else { inferred_base(candidates) };
            match redecode(&base, &offsets, &bytes, &valid) {
                Some((b, v)) => (Type::inferred(base), b, v),
                // An inferred type is never reported: values the base cannot hold together
                // (38 digits at one scale) keep the column text.
                None => (Type::inferred(Base::Text), Building::Text { offsets, bytes }, valid),
            }
        }
    };
    let data = building.finish();
    let valid = valid.finish();
    if ty.key {
        crate::keys::check(&data, &valid, column, threads, problems);
    }
    (ty, data, valid)
}

fn inferred_base(c: u8) -> Base {
    if c & infer::INT != 0 {
        Base::Int
    } else if c & infer::NUM != 0 {
        Base::Num
    } else if c & infer::TRUE_FALSE != 0 {
        Base::Bool { yes: "true".into(), no: "false".into() }
    } else if c & infer::YES_NO != 0 {
        Base::Bool { yes: "yes".into(), no: "no".into() }
    } else if c & infer::DATE != 0 {
        Base::Date
    } else if c & infer::DATETIME != 0 {
        Base::DateTime
    } else {
        Base::Text
    }
}

/// An untyped column's texts decoded by the base inferred from them, or nothing when one
/// of them does not fit the column after all.
fn redecode(base: &Base, offsets: &[i32], bytes: &[u8], valid: &BitsBuilder) -> Option<(Building, BitsBuilder)> {
    let ty = Type::inferred(base.clone());
    let d = Decoder::of(Some(&ty));
    let n = offsets.len() - 1;
    let mut b = Building::new(&d, n);
    let mut v = BitsBuilder::with_capacity(n);
    let mut problems = Vec::new();
    for i in 0..n {
        let t = &bytes[offsets[i] as usize..offsets[i + 1] as usize];
        if !valid.get(i) {
            column::push_null(&mut b);
            v.push(false);
            continue;
        }
        let mut run = Run { valid: &mut v, problems: &mut problems, column: 0, first: 0 };
        column::decode(&d, &mut b, t, &mut run).ok()?;
        v.push(true);
    }
    problems.is_empty().then_some((b, v))
}

/// A line's ending: `\r\n`, `\n`, a plain file's lone `\r`, or none at the end of the file.
fn ending(s: &str) -> &str {
    if s.ends_with("\r\n") {
        "\r\n"
    } else if s.ends_with('\n') {
        "\n"
    } else if s.ends_with('\r') {
        "\r"
    } else {
        ""
    }
}

impl Doc {
    pub fn source(&self) -> &str {
        &self.source
    }

    /// How the file was read, and how it is written.
    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    /// Whether a plain file quotes every field (its header and first row do), so an edited
    /// cell and a new row are quoted so too.
    pub fn quotes_all(&self) -> bool {
        self.quotes_all
    }

    /// A plain file's header cells that name their columns badly, by column.
    pub fn naming(&self) -> &[Naming] {
        &self.naming
    }

    /// A naming problem in words without its place: `has no name`, `"a" names column 1 too`.
    pub fn naming_why(&self, n: &Naming) -> String {
        match *n {
            Naming::Blank { .. } => "has no name".to_owned(),
            Naming::Twice { column, first } => {
                format!("\"{}\" names column {} too", self.columns[column as usize].name, first + 1)
            }
        }
    }

    /// A naming problem in words: `the header, column 2: "a" names column 1 too`.
    pub fn describe_naming(&self, n: &Naming) -> String {
        format!("the header, column {}: {}", n.column() + 1, self.naming_why(n))
    }

    /// The text between two fields.
    fn sep(&self) -> &'static str {
        match self.dialect.separator() {
            Separator::Comma => ",",
            Separator::Semicolon => ";",
            Separator::Tab => "\t",
            Separator::Pipe => "|",
        }
    }

    /// A cell's text as this file writes it.
    fn quote<'a>(&self, text: &'a str) -> Cow<'a, str> {
        scan::quote_in(text, self.dialect.separator().byte(), self.quotes_all)
    }

    /// Every column, in the header's order.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    pub fn rows(&self) -> usize {
        self.records.len() - 1
    }

    /// Every problem, by row, then column.
    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }

    /// The types the prelude names, those it imports included, in order.
    pub fn named(&self) -> &[NamedType] {
        &self.named
    }

    /// The prelude's statements, as written.
    pub fn statements(&self) -> &[Statement] {
        &self.statements
    }

    /// The tables the prelude names: written inline, filled by scripts, imported.
    pub fn tables(&self) -> &[Table] {
        &self.tables
    }

    /// The scripts that fill tables, in the prelude's order.
    pub fn scripts(&self) -> &[Script] {
        &self.scripts
    }

    /// A type this file exports, by its name.
    pub fn exported_type(&self, name: &str) -> Option<&NamedType> {
        let exported = self.statements.iter().any(|s| matches!(s, Statement::Type { name: n, exported: true, .. } if n == name));
        exported.then(|| self.named.iter().find(|t| t.name == name)).flatten()
    }

    /// The names this file exports, types and tables, in the prelude's order.
    pub fn exports(&self) -> Vec<String> {
        self.statements
            .iter()
            .filter_map(|s| match s {
                Statement::Type { name, exported: true, .. }
                | Statement::Table { name, exported: true, .. }
                | Statement::Run { name, exported: true, .. } => Some(name.clone()),
                _ => None,
            })
            .collect()
    }

    /// Whether any cell is computed, by its column's formula or its own.
    pub fn computes(&self) -> bool {
        self.columns.iter().any(|c| !c.origin.is_empty())
    }

    pub fn column(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    pub fn value(&self, row: usize, column: usize) -> Value<'_> {
        self.columns[column].value(row)
    }

    /// Where each row's record starts in the source, and where the last one ends: the
    /// spans a cache keeps and a tail reading starts from.
    pub fn records(&self) -> &[u32] {
        &self.records
    }

    /// Where the header starts.
    pub fn body(&self) -> usize {
        self.body as usize
    }

    /// Whether anything was changed since the file was read.
    pub fn changed(&self) -> bool {
        self.header.is_some() || !self.edits.is_empty()
    }

    /// A row's record as the file has it now, without its line break.
    pub fn line(&self, row: usize) -> &str {
        if let Some(e) = self.edits.get(&(row as u32)) {
            return e;
        }
        let s = &self.source[self.records[row] as usize..self.records[row + 1] as usize];
        &s[..s.len() - ending(s).len()]
    }

    /// A record's fields as written (quotes included), from text that read once already.
    fn raw_fields<'a>(&self, line: &'a str) -> Vec<&'a str> {
        let (spans, _) = scan::record(line.as_bytes(), 0, self.dialect.records()).unwrap();
        spans.iter().map(|&(s, e)| &line[s as usize..e as usize]).collect()
    }

    /// A stored cell's text as written, unquoted; blank for a field the row leaves out.
    pub fn text(&self, row: usize, column: usize) -> Cow<'_, str> {
        assert!(column < self.columns.len(), "column {column} is not stored");
        let fields = self.raw_fields(self.line(row));
        fields.get(column).map_or(Cow::Borrowed(""), |raw| scan::unquoted(raw))
    }

    /// Every stored cell's text in a row, as `text` gives each, the record scanned once:
    /// what a host that keeps its own rows (a table editing the file) takes.
    pub fn texts(&self, row: usize) -> Vec<Cow<'_, str>> {
        let fields = self.raw_fields(self.line(row));
        (0..self.columns.len()).map(|k| fields.get(k).map_or(Cow::Borrowed(""), |raw| scan::unquoted(raw))).collect()
    }

    /// A field as written, to write again with more after it: as it is, but for a quote a
    /// plain file never closed, which is quoted from its text.
    fn sound<'a>(&self, raw: &'a str) -> Cow<'a, str> {
        if scan::closed(raw) { Cow::Borrowed(raw) } else { Cow::Owned(self.quote(&scan::unquoted(raw)).into_owned()) }
    }

    /// A row's record as `line` gives it, to write again with more after it (`sound`).
    fn sound_line(&self, row: usize) -> Cow<'_, str> {
        let line = self.line(row);
        let fields = self.raw_fields(line);
        if fields.last().is_none_or(|f| scan::closed(f)) {
            return Cow::Borrowed(line);
        }
        Cow::Owned(fields.iter().map(|f| self.sound(f)).collect::<Vec<_>>().join(self.sep()))
    }

    /// How many fields a row's record holds now: a short row fewer than the header, a
    /// plain file's ragged row more.
    pub fn width(&self, row: usize) -> usize {
        self.raw_fields(self.line(row)).len()
    }

    /// A problem in words: `row 3, cost: "8x.50" is not a number`.
    pub fn describe(&self, p: &Problem) -> String {
        format!("row {}, {}: {}", p.row, self.columns[p.column as usize].name, self.why(p))
    }

    /// A problem in words without its place, for a host that shows it on the cell:
    /// `"8x.50" is not a number`, `is missing`.
    pub fn why(&self, p: &Problem) -> String {
        let c = &self.columns[p.column as usize];
        let at = (p.row, p.column);
        let gave = self.gave.get(&at);
        let text = match gave {
            Some(g) => Cow::Borrowed(g.as_str()),
            None => self.text(p.row as usize, p.column as usize),
        };
        let bound = |b: &Option<types::Bound>| b.map(|b| b.value.normal().to_string()).unwrap_or_default();
        let length = matches!(c.ty.base, Base::Text);
        let tables = || match &c.ty.base {
            Base::Ref(t) => t.iter().map(|t| t.table.as_str()).collect::<Vec<_>>().join(" or "),
            _ => String::new(),
        };
        let why = match p.why {
            Why::Missing => return "is missing".to_owned(),
            Why::Formula | Why::Computed => {
                return format!("{}: {}", text, self.unwell.get(&at).map_or("the formula does not compute", String::as_str));
            }
            Why::Absent => format!("is no key of {}", tables()),
            Why::Ambiguous => format!("is a key of more than one of {}, so it names no one row", tables()),
            Why::Control { point } => {
                // The text up to the character, never the character itself, which shows nothing.
                let (at, _) = column::control_at(text.as_bytes()).unwrap();
                let what = format!("{}, a control character", code_point(point));
                return match at {
                    0 => format!("starts with {what}"),
                    _ => format!("holds {what}, after \"{}\"", &text[..at]),
                };
            }
            Why::Ragged { fields } => {
                let n = self.columns.len();
                return match (fields as usize) < n {
                    true => format!("the row has {fields} of the header's {n} fields"),
                    false => format!("the row has {fields} fields, the header {n}"),
                };
            }
            Why::NotInt => "is not a whole number".to_owned(),
            Why::NotNumber => "is not a number".to_owned(),
            Why::NotPct => "is not a percentage".to_owned(),
            Why::NotBool => match &c.ty.base {
                Base::Bool { yes, no } => format!("is not {yes} or {no}"),
                _ => "is not a boolean".to_owned(),
            },
            Why::NotChoice => match &c.ty.base {
                Base::Choice(w) => format!("is not one of {}", w.join(", ")),
                _ => "is not a choice".to_owned(),
            },
            Why::NotDate => match &c.ty.format {
                Some(f) => format!("is not a date ({})", f.spelled),
                None => "is not a date".to_owned(),
            },
            Why::NotTime => match &c.ty.format {
                Some(f) => format!("is not a time ({})", f.spelled),
                None => "is not a time".to_owned(),
            },
            Why::NotDateTime => "is not an instant (2026-08-14T14:30:00Z)".to_owned(),
            Why::NotUrl => "is not a URL".to_owned(),
            Why::NotEmail => "is not an email address".to_owned(),
            Why::NotId => "is not an id (eight of 0-9 a-z but i l o u)".to_owned(),
            Why::OutOfRange => match c.ty.base {
                Base::Int => "is past a whole number's range".to_owned(),
                Base::DateTime => "is past the range of this column's instants".to_owned(),
                _ => "has more than 38 digits at this column's scale".to_owned(),
            },
            Why::Below => format!(
                "{} {}{}",
                if length { "is shorter than" } else { "is below" },
                if c.ty.min.is_some_and(|b| !b.inclusive) { "or at " } else { "" },
                bound(&c.ty.min)
            ),
            Why::Above => format!(
                "{} {}{}",
                if length { "is longer than" } else { "is above" },
                if c.ty.max.is_some_and(|b| !b.inclusive) { "or at " } else { "" },
                bound(&c.ty.max)
            ),
            Why::Duplicate { other } => format!("is the key of row {other} too"),
        };
        match gave {
            Some(_) => format!("the formula gives \"{text}\", which {why}"),
            None => format!("\"{text}\" {why}"),
        }
    }

    /// The header's line break, the one new lines take; `\n` when it has none.
    pub fn newline(&self) -> &str {
        match ending(self.header_text()) {
            "" => "\n",
            nl => nl,
        }
    }

    /// The header's record as the file has it, its line break included.
    fn header_text(&self) -> &str {
        &self.source[self.body as usize..self.records[0] as usize]
    }

    /// The header's record as it is now, without its line break.
    fn header_line(&self) -> &str {
        match &self.header {
            Some(h) => h,
            None => {
                let h = self.header_text();
                &h[..h.len() - ending(h).len()]
            }
        }
    }

    /// The text before the header, the byte order mark and the prelude, as written.
    pub fn prelude_text(&self) -> &str {
        &self.source[..self.body as usize]
    }

    /// Whether the file ends in a line break.
    pub fn trailing(&self) -> bool {
        match self.dialect {
            Dialect::Tcsv => self.source.ends_with('\n'),
            Dialect::Plain(_) => self.source.ends_with(['\n', '\r']),
        }
    }

    fn terminator(&self, row: usize) -> &str {
        ending(&self.source[self.records[row] as usize..self.records[row + 1] as usize])
    }

    /// The file's text: what was read, but for what was changed.
    pub fn write(&self) -> String {
        if !self.changed() {
            return self.source.clone();
        }
        let mut out = String::with_capacity(self.source.len() + 64);
        out.push_str(&self.source[..self.body as usize]);
        let first = self.records[0] as usize;
        let header = &self.source[self.body as usize..first];
        match &self.header {
            Some(h) => {
                out.push_str(h);
                out.push_str(&header[header.trim_end_matches(['\r', '\n']).len()..]);
            }
            None => out.push_str(header),
        }
        let mut cursor = first;
        for (&row, text) in &self.edits {
            let row = row as usize;
            out.push_str(&self.source[cursor..self.records[row] as usize]);
            out.push_str(text);
            out.push_str(self.terminator(row));
            cursor = self.records[row + 1] as usize;
        }
        out.push_str(&self.source[cursor..]);
        out
    }

    /// `text`, this file changed, read again on up to `threads` threads, its imports the
    /// ones this file followed.
    fn reread(&self, text: String, threads: usize) -> Result<Doc, Error> {
        let mut again = Again { imported: &self.imported };
        read_in(text.into_bytes(), self.dialect, threads, &mut again, None)
    }

    /// The file with exactly `rows`, in their order, read again on up to `threads`
    /// threads: a kept row is its record as the file has it now, edits included; a new
    /// one is written from its cells, each quoted only where CSV needs it. The prelude and
    /// the header stay as they are, records are joined by the header's line break, and the
    /// file ends in one when it did. What moving, adding and taking out rows is. A new
    /// cell holding a control character is refused at its row.
    pub fn rearranged(&self, rows: &[Row], threads: usize) -> Result<Doc, Error> {
        assert!(!self.columns.is_empty(), "a file with no header has no rows to arrange");
        let nl = self.newline();
        let mut out = String::with_capacity(self.source.len() + 64);
        out.push_str(&self.source[..self.body as usize]);
        out.push_str(self.header_line());
        for (n, r) in rows.iter().enumerate() {
            out.push_str(nl);
            match r {
                Row::Kept(k) => out.push_str(&self.sound_line(*k)),
                Row::New(cells) => {
                    for (k, c) in cells.iter().enumerate() {
                        writable(c, At::Row(n as u32))?;
                        if k > 0 {
                            out.push_str(self.sep());
                        }
                        out.push_str(&self.quote(c));
                    }
                }
            }
        }
        self.close(&mut out, !rows.is_empty(), nl);
        self.reread(out, threads)
    }

    /// A file being written, its lines broken by `nl`, ends as this one does: in a line
    /// break when it did, and otherwise never in an empty last line, which is no record:
    /// its blank is quoted.
    fn close(&self, out: &mut String, rows: bool, nl: &str) {
        if self.trailing() {
            out.push_str(nl);
        } else if rows && out.ends_with(nl) {
            out.push_str("\"\"");
        }
    }

    /// The file with exactly `columns`, in their order, read again on up to `threads`
    /// threads: a kept column is its header cell and every row's field as written, a renamed
    /// one the same under its new name (its type kept), a new one its header cell and its
    /// cells, quoted only where the file needs it. A short row's missing fields are
    /// written blank, and a ragged row's fields past the header are kept after the
    /// columns. What inserting, removing, moving and renaming columns is. A `.tcsv` file
    /// whose columns no longer read (two of one name, a computed column reading one taken
    /// out) is refused, this one untouched, and so is a new cell holding a control
    /// character, at its row.
    pub fn reshaped(&self, columns: &[ColumnAt], threads: usize) -> Result<Doc, Error> {
        assert!(!columns.is_empty(), "a file keeps one column at least");
        let rows = self.rows();
        let blank = self.quote("");
        let heads = self.raw_fields(self.header_line());
        let mut out = String::with_capacity(self.source.len() + 64);
        out.push_str(&self.source[..self.body as usize]);
        for (k, c) in columns.iter().enumerate() {
            if k > 0 {
                out.push_str(self.sep());
            }
            match *c {
                ColumnAt::Kept(j) => {
                    assert!(j < self.columns.len(), "column {j} is not stored");
                    out.push_str(heads[j]);
                }
                ColumnAt::Renamed(j, name) => {
                    assert!(j < self.columns.len(), "column {j} is not stored");
                    let cell = match (&self.dialect, &self.columns[j].spelled) {
                        (Dialect::Tcsv, spelled) => {
                            assert!(!name.contains(':'), "a column's name holds no \":\" ({name})");
                            match spelled {
                                Some(t) => format!("{name}:{t}"),
                                None => name.to_owned(),
                            }
                        }
                        (Dialect::Plain(_), _) => name.to_owned(),
                    };
                    out.push_str(&self.quote(&cell));
                }
                ColumnAt::New(header, cells) => {
                    assert!(cells.is_empty() || cells.len() == rows, "a new column has a cell for each of the {rows} rows, or none");
                    out.push_str(&self.quote(header));
                }
            }
        }
        for r in 0..rows {
            out.push_str(self.newline());
            let fields = self.raw_fields(self.line(r));
            for (k, c) in columns.iter().enumerate() {
                if k > 0 {
                    out.push_str(self.sep());
                }
                match *c {
                    ColumnAt::Kept(j) | ColumnAt::Renamed(j, _) => out.push_str(&fields.get(j).map_or(blank.clone(), |f| self.sound(f))),
                    ColumnAt::New(_, cells) => {
                        let t = cells.get(r).copied().unwrap_or("");
                        writable(t, At::Row(r as u32))?;
                        out.push_str(&self.quote(t));
                    }
                }
            }
            for extra in fields.iter().skip(self.columns.len()) {
                out.push_str(self.sep());
                out.push_str(&self.sound(extra));
            }
        }
        self.close(&mut out, rows > 0, self.newline());
        self.reread(out, threads)
    }

    /// The file written as `to`, another dialect or this one with other header cells: a
    /// plain file's header the columns' names and every computed cell its value, so the
    /// numbers a person saw are in the file; a `.tcsv` file's header `header` when given
    /// (each column's cell with its type, a plain file converted, its text that would read
    /// as a formula escaped), else the header as it is, and the prelude kept. A record is
    /// kept as written when the separator is the same and nothing joins it; otherwise it
    /// is written from its cells, quoted where `to` needs it (a plain file that quotes
    /// every field still does). A `.tcsv` file holds no fields past its header's, so a
    /// ragged row loses them there. The byte order mark, the line breaks and the final
    /// one are this file's.
    pub fn written_as(&self, to: Dialect, header: Option<&[&str]>) -> String {
        if to == self.dialect && header.is_none() {
            return self.write();
        }
        let width = self.columns.len();
        if let Some(h) = header {
            assert_eq!(h.len(), width, "a header cell for each column");
        }
        let plain = matches!(to, Dialect::Plain(_));
        // A `.tcsv` file's lines end in `\n` or `\r\n`, never a lone `\r`.
        let nl = match self.newline() {
            "\r" if !plain => "\n",
            nl => nl,
        };
        let (sep, all) = (to.separator().byte(), plain && self.quotes_all);
        let quote = |t: &str| scan::quote_in(t, sep, all).into_owned();
        let sep_text = [sep];
        let sep_text = core::str::from_utf8(&sep_text).unwrap();
        let computes = plain && self.computes();
        let escapes = !plain && self.dialect != Dialect::Tcsv;
        let mut out = String::with_capacity(self.source.len() + 64);
        let bom = if self.source.starts_with('\u{feff}') { "\u{feff}" } else { "" };
        out.push_str(match (to, self.dialect) {
            (Dialect::Tcsv, Dialect::Tcsv) => self.prelude_text(),
            _ => bom,
        });
        let names: Vec<String> = match header {
            Some(h) => h.iter().map(|c| quote(c)).collect(),
            None if !plain && self.dialect == Dialect::Tcsv => Vec::from([self.header_line().to_owned()]),
            None => self.columns.iter().map(|c| quote(&c.name)).collect(),
        };
        out.push_str(&names.join(sep_text));
        let same = to.separator() == self.dialect.separator() && !computes;
        let strict = Records { sep, lenient: false };
        for r in 0..self.rows() {
            out.push_str(nl);
            let line = self.line(r);
            let fields = self.raw_fields(line);
            let regular = scan::record(line.as_bytes(), 0, strict).is_ok();
            if same && regular && (plain || fields.len() <= width) && !(escapes && line.contains('`')) {
                out.push_str(line);
                continue;
            }
            let mut cells: Vec<String> =
                fields.iter().map(|f| if escapes { quote(&escaped(&scan::unquoted(f))) } else { quote(&scan::unquoted(f)) }).collect();
            if !plain {
                cells.truncate(width);
            }
            if computes {
                cells.resize(cells.len().max(width), quote(""));
                for (k, c) in self.columns.iter().enumerate() {
                    if c.origin(r) != Origin::Written {
                        cells[k] = match c.value(r) {
                            Value::Null => quote(""),
                            Value::Decimal(d) => d.normal().to_string(),
                            Value::Text(t) | Value::Choice(t) => quote(t),
                            v => quote(&c.form(v)),
                        };
                    }
                }
            }
            out.push_str(&cells.join(sep_text));
        }
        self.close(&mut out, self.rows() > 0, nl);
        out
    }

    /// One cell's text changed; every other byte stays as read. The cell is written in
    /// its file form (quoted only where CSV needs it, the blank fields before it in a
    /// short row) and decoded again, with its problems, its key's duplicates and the
    /// computed columns. Text holding a control character is refused at the row, the doc
    /// untouched.
    pub fn set_cell(&mut self, row: usize, column: usize, text: &str) -> Result<(), Error> {
        assert!(column < self.columns.len(), "column {column} is not stored");
        assert!(row < self.rows(), "row {row} does not exist");
        writable(text, At::Row(row as u32))?;
        let was = self.text(row, column).into_owned();
        let blank = self.quote("");
        let mut fields: Vec<Cow<'_, str>> = self.raw_fields(self.line(row)).into_iter().map(|f| self.sound(f)).collect();
        while fields.len() <= column {
            fields.push(blank.clone());
        }
        fields[column] = self.quote(text);
        let width = fields.len();
        let mut joined = fields.join(self.sep());
        if joined.is_empty() && self.terminator(row).is_empty() {
            // An empty last line with no break after it is no record: the blank is quoted.
            joined.push_str("\"\"");
        }
        self.edits.insert(row as u32, joined);
        let tcsv = self.dialect == Dialect::Tcsv;
        let origin = match (tcsv && read::said(text.as_bytes()) == Said::Formula, text.is_empty() && self.columns[column].formula.is_some())
        {
            (true, _) => Origin::Cell,
            (false, true) => Origin::Formula,
            _ => Origin::Written,
        };
        if self.computes() || origin != Origin::Written {
            let col = &mut self.columns[column];
            if col.origin.is_empty() && origin != Origin::Written {
                col.origin = Arc::new(alloc::vec![Origin::Written; self.records.len() - 1]);
            }
            if !col.origin.is_empty() {
                Arc::make_mut(&mut col.origin)[row] = origin;
            }
            self.rebuild(column, &[]);
            self.recompute();
        } else {
            let text = if tcsv && read::said(text.as_bytes()) == Said::Escaped { &text[1..] } else { text };
            let was = if tcsv && read::said(was.as_bytes()) == Said::Escaped { &was[1..] } else { &was[..] };
            self.decode_cell(row, column, text, was);
        }
        if matches!(self.dialect, Dialect::Plain(_)) {
            // The row's width may have changed: a field written past a short row's end.
            self.problems.retain(|p| !(p.row == row as u32 && p.kind() == Kind::Ragged));
            if width != self.columns.len() {
                let p = read::ragged(row as u32, width, self.columns.len());
                let at = self.problems.partition_point(|q| q.order() < p.order());
                self.problems.insert(at, p);
            }
        }
        Ok(())
    }

    /// One cell set to a value, written in its column's form; refused as `set_cell`
    /// refuses its text.
    pub fn set_value(&mut self, row: usize, column: usize, v: Value) -> Result<(), Error> {
        let text = self.columns[column].form(v);
        self.set_cell(row, column, &text)
    }

    fn decode_cell(&mut self, row: usize, column: usize, text: &str, was: &str) {
        let col = &mut self.columns[column];
        if col.ty.inferred {
            // What the column's values could be is the meet of what each could be, so a
            // value that could be exactly what the one it replaces could be changes nothing
            // the column infers: it is decoded in place. Anything else, a cell broken by a
            // control character among it, is the column read again, as a fresh read would
            // infer it.
            let could = |t: &str| (!t.is_empty()).then(|| infer::narrow(infer::ALL, t.as_bytes()));
            if col.valid.get(row) && could(text) == could(was) && put(col, row, text) == Some(None) {
                return self.recompute();
            }
            self.rebuild(column, &[]);
            return self.recompute();
        }
        let why = put(col, row, text);
        let Some(why) = why else {
            self.rebuild(column, &[]);
            return self.recompute();
        };
        let at = self.problems.partition_point(|p| p.order() < (row as u32, column as u32));
        let end = at + self.problems[at..].iter().take_while(|p| p.order() == (row as u32, column as u32)).count();
        let fresh: Vec<Problem> = why.map(|why| Problem { row: row as u32, column: column as u32, why }).into_iter().collect();
        self.problems.splice(at..end, fresh);
        if self.columns[column].ty.key {
            self.rekey(column);
        }
        self.recompute();
    }

    /// A column read again from its cells, as a fresh read would, but for the cells
    /// `results` names (by row, in order), read as the text their formulas gave. A cell
    /// computed and given nothing here is null, and its only problem a type its result
    /// breaks.
    fn rebuild(&mut self, column: usize, results: &[(usize, String)]) {
        let rows = self.rows();
        let mut raw = String::new();
        let mut spans = Vec::with_capacity(rows);
        let mut given = results.iter().peekable();
        for r in 0..rows {
            let s = raw.len() as u32;
            match given.next_if(|(row, _)| *row == r) {
                Some((_, text)) => raw.push_str(&scan::quote(text)),
                None => {
                    let fields = self.raw_fields(self.line(r));
                    raw.push_str(fields.get(column).copied().unwrap_or(""));
                }
            }
            spans.push((s, raw.len() as u32));
        }
        let c = &self.columns[column];
        let ty = if c.ty.inferred { None } else { Some(c.ty.clone()) };
        let decoders = decoders(core::iter::once((ty.as_ref(), false)));
        let mut part = Part::new(raw.as_bytes(), 0, raw.len(), &decoders, self.dialect != Dialect::Tcsv);
        for &(s, e) in &spans {
            part.field(s, e).unwrap();
            part.record(e);
        }
        part.finish();
        let mut problems = core::mem::take(&mut part.problems);
        let reading = part.columns.pop().unwrap();
        self.candidates[column] = reading.candidates;
        let origin = Arc::clone(&self.columns[column].origin);
        problems.retain_mut(|p| {
            p.column = column as u32;
            let computed = origin.get(p.row as usize).is_some_and(|o| *o != Origin::Written);
            !matches!(p.why, Why::Formula | Why::Computed) && !(computed && p.why == Why::Missing)
        });
        let (ty, data, valid) = settle(ty, reading, column as u32, 1, &mut problems);
        let col = &mut self.columns[column];
        col.ty = ty;
        col.data = data;
        col.valid = valid;
        // A ragged row's problem is its record's, never its cell's.
        self.problems.retain(|p| p.column != column as u32 || p.kind() == Kind::Ragged);
        self.problems.extend(problems);
        self.problems.sort_by_key(|p| p.order());
    }

    /// The file grown: when `bytes` is this file's bytes with records after them (a
    /// ledger appended to), only the new records are read and joined to the columns
    /// already held; anything else (an edit pending, a change before the end, a type the
    /// new rows would infer otherwise) is read whole. Either way the result is what `read`
    /// gives for `bytes`.
    pub fn extend(mut self, bytes: Vec<u8>) -> Result<Doc, Error> {
        let from = self.source.len();
        // A formula or a reference reads more than its row, so a file holding one is read
        // whole.
        let alone = !self.computes() && self.formulas.iter().all(Option::is_none) && self.tables.is_empty();
        let appended = !self.changed()
            && alone
            && !self.columns.is_empty()
            && bytes.len() > from
            && self.source.ends_with('\n')
            && bytes.starts_with(self.source.as_bytes());
        if !appended {
            return self.reread(
                String::from_utf8(bytes)
                    .map_err(|e| error(At::File, format!("is not UTF-8: byte {} begins no character", e.utf8_error().valid_up_to())))?,
                1,
            );
        }
        if let Err(e) = core::str::from_utf8(&bytes[from..]) {
            return Err(error(At::File, format!("is not UTF-8: byte {} begins no character", from + e.valid_up_to())));
        }
        if bytes.len() >= u32::MAX as usize {
            return Err(error(At::File, "is larger than 4 GiB"));
        }
        // SAFETY: the first `from` bytes are this doc's source, a `str` ending at a line
        // break, and the rest was checked just above.
        let source = unsafe { String::from_utf8_unchecked(bytes) };
        let old_rows = self.rows() as u32;
        let decoders = decoders(self.columns.iter().map(|c| ((!c.ty.inferred).then_some(&c.ty), false)));
        let tail = read::sequential(source.as_bytes(), from, &decoders, self.dialect.records())
            .map_err(|(row, s)| error(At::Row(old_rows + row), stop_why(s, self.columns.len())))?;
        let Part { records, columns: readings, problems, .. } = tail;
        if problems.iter().any(|p| p.why == Why::Formula) {
            return self.reread(source, 1);
        }
        let mut joined = Vec::with_capacity(self.columns.len());
        for (c, r) in readings.into_iter().enumerate() {
            let col = &self.columns[c];
            if !col.ty.inferred {
                joined.push((r.building, r.valid));
                continue;
            }
            let Building::Text { offsets, bytes } = r.building else { unreachable!("an untyped column reads as text") };
            let had = col.nulls() < col.len();
            let has = (0..r.valid.len).any(|i| r.valid.get(i));
            let candidates = self.candidates[c] & r.candidates;
            let base = if had || has { inferred_base(candidates) } else { Base::Text };
            if base != col.ty.base {
                return self.reread(source, 1);
            }
            self.candidates[c] = candidates;
            if base == Base::Text {
                joined.push((Building::Text { offsets, bytes }, r.valid));
                continue;
            }
            match redecode(&base, &offsets, &bytes, &r.valid) {
                Some(b) => joined.push(b),
                None => return self.reread(source, 1),
            }
        }
        for (c, (b, v)) in joined.into_iter().enumerate() {
            let col = &mut self.columns[c];
            let data = core::mem::replace(&mut col.data, Data::Int(Arc::new(Vec::new())));
            let (data, valid) = join_data(data, col.valid.clone(), b, v, c as u32, old_rows, &mut self.problems);
            col.data = data;
            col.valid = valid;
        }
        self.problems.retain(|p| p.kind() != Kind::Key);
        self.problems.extend(problems.into_iter().map(|p| Problem { row: p.row + old_rows, ..p }));
        self.records.pop();
        self.records.extend_from_slice(&records);
        self.records.push(source.len() as u32);
        self.source = source;
        for c in 0..self.columns.len() {
            if self.columns[c].ty.key {
                let col = &self.columns[c];
                crate::keys::check(&col.data, &col.valid, c as u32, 1, &mut self.problems);
            }
        }
        self.problems.sort_by_key(|p| p.order());
        Ok(self)
    }

    fn rekey(&mut self, column: usize) {
        self.problems.retain(|p| !(p.column == column as u32 && p.kind() == Kind::Key));
        let c = &self.columns[column];
        let mut found = Vec::new();
        crate::keys::check(&c.data, &c.valid, column as u32, 1, &mut found);
        self.problems.extend(found);
        self.problems.sort_by_key(|p| p.order());
    }

    /// One column's header cell rewritten after its `:`: its type in the shorthand a
    /// writer writes, and its formula when `spelled` has one (`EUR=cost*qty`); `""` makes
    /// it a plain column again. No cell is rewritten: a cell the new type breaks is
    /// reported, and a formula fills the cells left blank. Refused, the doc untouched, when
    /// the header no longer reads (a type or a formula that does not, a formula another
    /// column's formula then cannot read): the place is the column refused.
    pub fn set_type(&mut self, column: usize, spelled: &str) -> Result<(), Error> {
        assert!(column < self.columns.len(), "column {column} is not stored");
        assert_eq!(self.dialect, Dialect::Tcsv, "a plain file holds no types: written as TCSV (`written_as`), it does");
        let at = At::Column(column as u32 + 1);
        if let Some(point) = column::control(spelled.as_bytes()) {
            return Err(error(at, format!("holds {}, a control character", code_point(point))));
        }
        let names: Vec<String> = self.tables.iter().map(|t| t.name.clone()).collect();
        let name = self.columns[column].name.clone();
        let cell = match spelled {
            "" => name,
            s => {
                let h = head(&format!("{name}:{s}"), &self.named, &names).map_err(|why| error(at, why))?;
                let ty = h.ty.as_ref().unwrap().shorthand();
                match h.formula {
                    Some(f) => format!("{name}:{ty}={f}"),
                    None => format!("{name}:{ty}"),
                }
            }
        };
        let line = self.header_line().to_owned();
        let mut fields: Vec<Cow<'_, str>> = self.raw_fields(&line).into_iter().map(Cow::Borrowed).collect();
        fields[column] = scan::quote(&cell);
        let header = fields.join(",");
        let mut text = String::with_capacity(self.source.len() + 32);
        text.push_str(self.prelude_text());
        text.push_str(&header);
        let rest = self.write();
        text.push_str(&rest[self.body as usize + self.header_line().len()..]);
        let doc = self.reread(text, 1).map_err(|e| match e.at {
            At::Column(_) => error(at, e.why),
            _ => e,
        })?;
        *self = doc;
        Ok(())
    }

    /// The column formulas resolved against the columns and the tables, and refused at
    /// their column when one does not read, names what is not there, gives what its
    /// column cannot hold, or reads itself through others.
    fn resolve_formulas(&mut self) -> Result<(), Error> {
        let scope = DocScope { columns: &self.columns, tables: &self.tables };
        let mut formulas = Vec::with_capacity(self.columns.len());
        let mut aggregates = 0;
        for (k, c) in self.columns.iter().enumerate() {
            let Some(f) = &c.formula else {
                formulas.push(None);
                continue;
            };
            let at = At::Column(k as u32 + 1);
            let e = expr::parse(f).map_err(|why| error(at, why))?;
            let (ir, kind) = expr::resolve(&e, &scope, &mut aggregates).map_err(|why| error(at, why))?;
            holds(&c.ty.base, kind).map_err(|why| error(at, why))?;
            formulas.push(Some(ir));
        }
        // A formula reading itself, through others or not, never finishes.
        let n = self.columns.len();
        let mut reads: Vec<Vec<usize>> = Vec::with_capacity(n);
        for f in &formulas {
            let mut out = Vec::new();
            if let Some(ir) = f {
                expr::reads(ir, None, &mut out);
            }
            reads.push(out);
        }
        let mut state = alloc::vec![0u8; n];
        fn visit(k: usize, reads: &[Vec<usize>], formulas: &[Option<Ir>], state: &mut [u8]) -> Option<usize> {
            if formulas[k].is_none() || state[k] == 2 {
                return None;
            }
            if state[k] == 1 {
                return Some(k);
            }
            state[k] = 1;
            for &j in &reads[k] {
                if let Some(c) = visit(j, reads, formulas, state) {
                    return Some(c);
                }
            }
            state[k] = 2;
            None
        }
        for k in 0..n {
            if let Some(c) = visit(k, &reads, &formulas, &mut state) {
                return Err(error(At::Column(c as u32 + 1), format!("{}'s formula reads itself", self.columns[c].name)));
            }
        }
        self.formulas = formulas;
        Ok(())
    }

    /// Every reference column's tables reachable, each naming its key: refused at the
    /// column when one names a column that is no key of its table.
    fn check_targets(&self) -> Result<(), Error> {
        for (k, c) in self.columns.iter().enumerate() {
            let Base::Ref(targets) = &c.ty.base else { continue };
            for t in targets {
                let table = self.tables.iter().find(|x| x.name == t.table).unwrap();
                let Some(doc) = &table.doc else { continue };
                let key = doc.columns.iter().find(|x| x.name == t.column);
                if !key.is_some_and(|x| x.ty.key) {
                    return Err(error(
                        At::Column(k as u32 + 1),
                        format!("{} is no key of {}: its key is written column!", t.column, t.table),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Every computed cell computed again, each column holding one decoded with what its
    /// cells gave, and every reference checked against its tables.
    fn recompute(&mut self) {
        self.problems.retain(|p| !matches!(p.why, Why::Formula | Why::Absent | Why::Ambiguous));
        self.unwell.clear();
        self.gave.clear();
        if self.computes() {
            self.compute();
        }
        self.check_references();
        self.problems.sort_by_key(|p| p.order());
    }

    fn compute(&mut self) {
        let rows = self.rows();
        // Each cell's own formula, resolved; why not, where it does not.
        let mut cells: BTreeMap<(u32, u32), Result<Ir, String>> = BTreeMap::new();
        let mut aggregates = self.formulas.iter().flatten().map(slots).max().unwrap_or(0);
        {
            let scope = DocScope { columns: &self.columns, tables: &self.tables };
            for (k, c) in self.columns.iter().enumerate() {
                for r in 0..c.origin.len() {
                    if c.origin[r] != Origin::Cell {
                        continue;
                    }
                    let text = self.text(r, k);
                    let src = &text[2..text.len() - 1];
                    let resolved = expr::parse(src).and_then(|e| expr::resolve(&e, &scope, &mut aggregates)).and_then(|(ir, kind)| {
                        holds(&c.ty.base, kind)?;
                        Ok(ir)
                    });
                    cells.insert((r as u32, k as u32), resolved);
                }
            }
        }
        let mut memo: Vec<Vec<Slot>> =
            self.columns.iter().map(|c| if c.origin.is_empty() { Vec::new() } else { alloc::vec![Slot::Pending; rows] }).collect();
        let mut eval = Eval {
            columns: &self.columns,
            tables: &self.tables,
            formulas: &self.formulas,
            cells: &cells,
            rows,
            memo: &mut memo,
            aggregates: alloc::vec![None; aggregates],
            indexes: (0..self.tables.len()).map(|_| None).collect(),
            cycles: Vec::new(),
        };
        let mut results: Vec<Vec<(usize, V)>> = Vec::with_capacity(self.columns.len());
        for (k, c) in self.columns.iter().enumerate() {
            let mut out = Vec::new();
            for r in 0..c.origin.len() {
                if c.origin[r] != Origin::Written {
                    let v = expr::Source::cell(&mut eval, None, r, k);
                    out.push((r, v));
                }
            }
            results.push(out);
        }
        let cycles = core::mem::take(&mut eval.cycles);
        for (k, out) in results.into_iter().enumerate() {
            if self.columns[k].origin.is_empty() {
                continue;
            }
            let given = render(&self.columns[k], &out);
            self.rebuild(k, &given);
            for (r, text) in given {
                if self.problems.iter().any(|p| p.row == r as u32 && p.column == k as u32) {
                    self.gave.insert((r as u32, k as u32), text);
                }
            }
        }
        for ((r, k), why) in cells.iter().filter_map(|(at, c)| c.as_ref().err().map(|w| (at, w))) {
            self.unwell.insert((*r, *k), why.clone());
        }
        for (r, k) in cycles {
            if self.columns[k].origin(r) == Origin::Cell {
                self.unwell.entry((r as u32, k as u32)).or_insert_with(|| "the formula reads itself".to_owned());
            }
        }
        for &(r, k) in self.unwell.keys() {
            self.problems.retain(|p| !(p.row == r && p.column == k));
            self.problems.push(Problem { row: r, column: k, why: Why::Formula });
        }
    }

    /// Every reference's key looked up in the tables its column names: a problem where it
    /// is in none, or in two. A column naming a table that could not be reached is
    /// unchecked.
    fn check_references(&mut self) {
        for (k, c) in self.columns.iter().enumerate() {
            let Base::Ref(targets) = &c.ty.base else { continue };
            let mut keys: Vec<(&Column, BTreeMap<String, ()>)> = Vec::with_capacity(targets.len());
            for t in targets {
                let Some(doc) = self.tables.iter().find(|x| x.name == t.table).and_then(|x| x.doc.as_deref()) else { break };
                let key = doc.columns.iter().find(|x| x.name == t.column).unwrap();
                let set = (0..key.len()).filter_map(|r| value_v(key, r).canonical()).map(|s| (s, ())).collect();
                keys.push((key, set));
            }
            if keys.len() != targets.len() {
                continue;
            }
            for r in 0..c.len() {
                let Value::Text(text) = c.value(r) else { continue };
                let found = keys
                    .iter()
                    .filter(|(key, set)| {
                        decode_one(key, text).and_then(|one| value_v(&one, 0).canonical()).is_some_and(|s| set.contains_key(&s))
                    })
                    .count();
                let why = match found {
                    0 => Why::Absent,
                    1 => continue,
                    _ => Why::Ambiguous,
                };
                self.problems.push(Problem { row: r as u32, column: k as u32, why });
            }
        }
    }
}

/// What a formula giving `kind` may fill a column of `base` with: a number a numeric
/// column, a boolean a boolean one; anything else is written as text and read by the
/// column's type.
fn holds(base: &Base, kind: K) -> Result<(), String> {
    let ok = match (base, kind) {
        (_, K::Null) => true,
        (b, k) if b.is_number() => k == K::Num,
        (Base::Bool { .. }, k) => k == K::Bool,
        (Base::Date, k) => matches!(k, K::Date | K::Text),
        _ => true,
    };
    if ok { Ok(()) } else { Err(format!("the formula gives {}, which a {} column does not hold", kind.word(), base.tag())) }
}

/// The slots an `Ir`'s aggregates take: one past the last.
fn slots(ir: &Ir) -> usize {
    match ir {
        Ir::Const(_) | Ir::Col(_) => 0,
        Ir::Agg(_, _, a, slot) => slots(a).max(slot + 1),
        Ir::Follow(a, _) | Ir::Neg(a) | Ir::Not(a) | Ir::IsNull(a, _) => slots(a),
        Ir::Bin(_, a, b) => slots(a).max(slots(b)),
        Ir::Case(arms, o) => arms.iter().map(|(w, t)| slots(w).max(slots(t))).max().unwrap_or(0).max(slots(o)),
        Ir::Call(_, args) => args.iter().map(slots).max().unwrap_or(0),
    }
}

/// The kind a column's values are to a formula.
fn kind_of(base: &Base) -> K {
    match base {
        b if b.is_number() => K::Num,
        Base::Bool { .. } => K::Bool,
        Base::Date => K::Date,
        _ => K::Text,
    }
}

/// A cell's value as a formula reads it.
fn value_v(c: &Column, row: usize) -> V {
    match c.value(row) {
        Value::Null => V::Null,
        Value::Text(t) | Value::Choice(t) => V::Text(t.to_owned()),
        Value::Int(n) => V::Num(Exact::from_int(n)),
        Value::Decimal(d) => V::Num(Exact::from_decimal(d)),
        Value::Bool(b) => V::Bool(b),
        Value::Date(d) => V::Date(d),
        Value::Time(t) => V::Text(value::format_time(t)),
        Value::DateTime(t) => V::Text(value::format_datetime(t)),
    }
}

/// What a column's formulas gave, as the text its cells are read from: an amount at its
/// currency's minor unit, half to even, any other number at the most digits after the
/// point any of them has (fewer when 38 digits would not hold the largest), a boolean in
/// the column's words, a date in its format, text that would read as a formula escaped.
fn render(c: &Column, results: &[(usize, V)]) -> Vec<(usize, String)> {
    let minor = match &c.ty.base {
        Base::Money(cur) => cur.minor_unit(),
        _ => None,
    };
    let numbers = results.iter().filter_map(|(_, v)| match v {
        V::Num(n) => {
            let mut n = n.clone();
            n.normalize();
            Some(n)
        }
        _ => None,
    });
    let (mut int, mut frac) = (0, 0);
    for n in numbers {
        if n.int_digits() <= value::PRECISION {
            int = int.max(n.int_digits());
            frac = frac.max(n.scale);
        }
    }
    let scale = frac.min(value::PRECISION - int);
    results
        .iter()
        .map(|(r, v)| {
            let text = match v {
                V::Null => String::new(),
                V::Num(n) if c.ty.base.is_number() => {
                    if let Some(d) = minor {
                        return (*r, n.rescale(d).to_string());
                    }
                    let mut n = n.round(scale);
                    if c.ty.base == Base::Int {
                        n.normalize();
                    }
                    n.to_string()
                }
                V::Bool(b) => match &c.ty.base {
                    Base::Bool { yes, no } => {
                        if *b {
                            yes.clone()
                        } else {
                            no.clone()
                        }
                    }
                    _ => b.to_string(),
                },
                V::Date(d) => match (&c.ty.base, &c.ty.format) {
                    (Base::Date, Some(f)) => f.write(Some(*d), None),
                    _ => value::format_date(*d),
                },
                V::Text(t) => escaped(t),
                other => other.canonical().unwrap_or_default(),
            };
            (*r, text)
        })
        .collect()
}

/// Names as a formula in this file reads them: its columns, and its tables' columns.
struct DocScope<'a> {
    columns: &'a [Column],
    tables: &'a [Table],
}

impl expr::Scope for DocScope<'_> {
    fn column(&self, table: expr::Table, name: &str) -> Option<(usize, K)> {
        let columns = match table {
            None => self.columns,
            Some(t) => match &self.tables[t].doc {
                // A table not reached: whatever it holds reads as null.
                None => return Some((0, K::Null)),
                Some(d) => &d.columns,
            },
        };
        columns.iter().position(|c| c.name == name).map(|k| (k, kind_of(&columns[k].ty.base)))
    }

    fn table(&self, name: &str) -> Option<usize> {
        self.tables.iter().position(|t| t.name == name)
    }

    fn targets(&self, table: expr::Table, column: usize) -> Vec<(usize, usize)> {
        let base = match table {
            None => &self.columns[column].ty.base,
            Some(t) => match &self.tables[t].doc {
                None => return Vec::new(),
                Some(d) => &d.columns[column].ty.base,
            },
        };
        let Base::Ref(targets) = base else { return Vec::new() };
        targets
            .iter()
            .filter_map(|t| {
                let k = self.table(&t.table)?;
                let key = self.tables[k].doc.as_ref().map_or(Some(0), |d| d.columns.iter().position(|c| c.name == t.column))?;
                Some((k, key))
            })
            .collect()
    }

    fn key(&self, table: usize) -> Option<usize> {
        match &self.tables[table].doc {
            None => Some(0),
            Some(d) => d.columns.iter().position(|c| c.ty.key),
        }
    }
}

#[derive(Clone)]
enum Slot {
    Pending,
    Busy,
    Done(V),
}

/// A file's formulas evaluated: a computed cell when first read, every other value as its
/// column holds it.
struct Eval<'a> {
    columns: &'a [Column],
    tables: &'a [Table],
    formulas: &'a [Option<Ir>],
    cells: &'a BTreeMap<(u32, u32), Result<Ir, String>>,
    rows: usize,
    memo: &'a mut [Vec<Slot>],
    aggregates: Vec<Option<V>>,
    /// Each table's key values, as `V::canonical` writes them, and the row of each.
    indexes: Vec<Option<BTreeMap<String, usize>>>,
    /// The computed cells found reading themselves.
    cycles: Vec<(usize, usize)>,
}

impl expr::Source for Eval<'_> {
    fn cell(&mut self, table: expr::Table, row: usize, column: usize) -> V {
        if let Some(t) = table {
            return match &self.tables[t].doc {
                Some(d) => value_v(&d.columns[column], row),
                None => V::Null,
            };
        }
        let c = &self.columns[column];
        let origin = c.origin(row);
        if origin == Origin::Written {
            return value_v(c, row);
        }
        match &self.memo[column][row] {
            Slot::Done(v) => return v.clone(),
            Slot::Busy => {
                self.cycles.push((row, column));
                return V::Null;
            }
            Slot::Pending => {}
        }
        self.memo[column][row] = Slot::Busy;
        let (formulas, cells) = (self.formulas, self.cells);
        let ir = match origin {
            Origin::Formula => formulas[column].as_ref(),
            _ => cells.get(&(row as u32, column as u32)).and_then(|r| r.as_ref().ok()),
        };
        let v = match ir {
            Some(ir) => expr::eval(ir, None, row, self),
            None => V::Null,
        };
        self.memo[column][row] = Slot::Done(v.clone());
        v
    }

    fn rows(&self, table: expr::Table) -> usize {
        match table {
            None => self.rows,
            Some(t) => self.tables[t].doc.as_ref().map_or(0, |d| d.rows()),
        }
    }

    fn find(&mut self, table: usize, key: usize, value: &V) -> Option<usize> {
        let doc = self.tables[table].doc.as_deref()?;
        let index = self.indexes[table].get_or_insert_with(|| {
            let mut m = BTreeMap::new();
            for r in (0..doc.rows()).rev() {
                if let Some(s) = value_v(&doc.columns[key], r).canonical() {
                    m.insert(s, r);
                }
            }
            m
        });
        index.get(&value.canonical()?).copied()
    }

    fn recall(&mut self, slot: usize) -> Option<V> {
        self.aggregates.get(slot).cloned().flatten()
    }

    fn remember(&mut self, slot: usize, v: V) {
        self.aggregates[slot] = Some(v);
    }
}

/// A column's values with a tail's after them, in place when nothing else holds the
/// buffers. A scale or unit that differs is raised to the larger, a value that then passes
/// its range becoming a problem (rows of the tail count from `base`).
fn join_data(
    data: Data,
    valid: Bits,
    tail: Building,
    tail_valid: BitsBuilder,
    c: u32,
    base: u32,
    problems: &mut Vec<Problem>,
) -> (Data, Bits) {
    use crate::column::{DECIMAL_LIMIT, INSTANT_LIMIT, raise};
    let mut v = BitsBuilder { bytes: Arc::unwrap_or_clone(valid.bytes), len: valid.len };
    let mut tv = tail_valid;
    let data = match (data, tail) {
        (Data::Text { offsets, bytes }, Building::Text { offsets: to, bytes: tb }) => {
            let (mut offsets, mut bytes) = (Arc::unwrap_or_clone(offsets), Arc::unwrap_or_clone(bytes));
            let shift = bytes.len() as i32;
            offsets.extend(to[1..].iter().map(|o| o + shift));
            bytes.extend_from_slice(&tb);
            Data::Text { offsets: Arc::new(offsets), bytes: Arc::new(bytes) }
        }
        (Data::Int(a), Building::Int(b)) => Data::Int(Arc::new(joined(a, &b))),
        (Data::Decimal { values, scale }, Building::Decimal { values: mut b, scale: bs }) => {
            let mut a = Arc::unwrap_or_clone(values);
            let s = scale.max(bs);
            raise(&mut a, (s - scale) as u32, DECIMAL_LIMIT, &mut Run { valid: &mut v, problems, column: c, first: 0 });
            raise(&mut b, (s - bs) as u32, DECIMAL_LIMIT, &mut Run { valid: &mut tv, problems, column: c, first: base });
            a.extend_from_slice(&b);
            Data::Decimal { values: Arc::new(a), scale: s }
        }
        (Data::DateTime { values, unit }, Building::DateTime { values: mut b, unit: bu }) => {
            let mut a = Arc::unwrap_or_clone(values);
            let u = unit.max(bu);
            raise(&mut a, (u - unit) as u32, INSTANT_LIMIT, &mut Run { valid: &mut v, problems, column: c, first: 0 });
            raise(&mut b, (u - bu) as u32, INSTANT_LIMIT, &mut Run { valid: &mut tv, problems, column: c, first: base });
            a.extend_from_slice(&b);
            Data::DateTime { values: Arc::new(a), unit: u }
        }
        (Data::Bool(a), Building::Bool(b)) => {
            let mut x = BitsBuilder { bytes: Arc::unwrap_or_clone(a.bytes), len: a.len };
            x.append(&b);
            Data::Bool(x.finish())
        }
        (Data::Date(a), Building::Date(b)) => Data::Date(Arc::new(joined(a, &b))),
        (Data::Time(a), Building::Time(b)) => Data::Time(Arc::new(joined(a, &b))),
        (Data::Choice(a), Building::Choice(b)) => Data::Choice(Arc::new(joined(a, &b))),
        _ => unreachable!("a tail read by its column's own decoder"),
    };
    v.append(&tv);
    (data, v.finish())
}

/// `a` then `b`, in `a`'s own buffer when nothing else holds it.
fn joined<T: Copy>(a: Arc<Vec<T>>, b: &[T]) -> Vec<T> {
    let mut a = Arc::unwrap_or_clone(a);
    a.extend_from_slice(b);
    a
}

/// One text read by a column's type, as a column of one cell: nothing when the text breaks
/// the type or is blank.
pub(crate) fn decode_one(col: &Column, text: &str) -> Option<Column> {
    let d = Decoder::of(Some(&col.ty));
    let mut one = Building::new(&d, 1);
    let mut valid = BitsBuilder::default();
    let mut problems = Vec::new();
    if text.is_empty() {
        return None;
    }
    let mut run = Run { valid: &mut valid, problems: &mut problems, column: 0, first: 0 };
    column::decode(&d, &mut one, text.as_bytes(), &mut run).ok()?;
    valid.push(true);
    Some(Column {
        name: String::new(),
        spelled: None,
        ty: col.ty.clone(),
        formula: None,
        data: one.finish(),
        valid: valid.finish(),
        origin: Arc::new(Vec::new()),
    })
}

/// Writes one cell's value into its column in place and says why it is a problem, if it
/// is one; nothing when the column has to be read again whole (a value with more digits
/// after the point than its column's scale or unit).
fn put(col: &mut Column, row: usize, text: &str) -> Option<Option<Why>> {
    let d = Decoder::of(Some(&col.ty));
    let mut one = Building::new(&d, 1);
    match (&mut one, &col.data) {
        (Building::Decimal { scale: s, .. }, Data::Decimal { scale, .. }) => *s = *scale,
        (Building::DateTime { unit: u, .. }, Data::DateTime { unit, .. }) => *u = *unit,
        _ => {}
    }
    let mut scratch = BitsBuilder::default();
    let mut raised = Vec::new();
    let why = match text.is_empty() {
        true => (!col.ty.optional).then_some(Why::Missing),
        false => {
            let mut run = Run { valid: &mut scratch, problems: &mut raised, column: 0, first: 0 };
            column::decode(&d, &mut one, text.as_bytes(), &mut run).err()
        }
    };
    match (&one, &col.data) {
        (Building::Decimal { scale: s, .. }, Data::Decimal { scale, .. }) if s > scale => return None,
        (Building::DateTime { unit: u, .. }, Data::DateTime { unit, .. }) if u > unit => return None,
        _ => {}
    }
    let ok = !text.is_empty() && why.is_none();
    if !ok {
        column::push_null(&mut one);
    }
    col.valid.set(row, ok);
    match (&mut col.data, one) {
        (Data::Text { offsets, bytes }, Building::Text { bytes: nb, .. }) => {
            let (a, b) = (offsets[row] as usize, offsets[row + 1] as usize);
            let delta = nb.len() as i32 - (b - a) as i32;
            Arc::make_mut(bytes).splice(a..b, nb);
            if delta != 0 {
                for o in &mut Arc::make_mut(offsets)[row + 1..] {
                    *o += delta;
                }
            }
        }
        (Data::Int(v), Building::Int(n)) => Arc::make_mut(v)[row] = n[0],
        (Data::Decimal { values, .. }, Building::Decimal { values: n, .. }) => Arc::make_mut(values)[row] = n[0],
        (Data::Bool(bits), Building::Bool(n)) => bits.set(row, n.get(0)),
        (Data::Date(v), Building::Date(n)) | (Data::Time(v), Building::Time(n)) => Arc::make_mut(v)[row] = n[0],
        (Data::DateTime { values, .. }, Building::DateTime { values: n, .. }) => Arc::make_mut(values)[row] = n[0],
        (Data::Choice(v), Building::Choice(n)) => Arc::make_mut(v)[row] = n[0],
        _ => unreachable!("a column decodes into its own kind"),
    }
    Some(why)
}

/// A file written from cells, for a host that keeps its own rows: `like`'s byte order
/// mark and prelude, its separator, its header's line break and its final line break,
/// every field quoted only where the file needs it (or always, in a plain file that
/// quotes every field).
pub fn compose(like: &Doc, header: &[&str], rows: &[Vec<&str>]) -> String {
    let nl = like.newline();
    let mut out = String::from(like.prelude_text());
    let line = |out: &mut String, cells: &[&str]| {
        for (k, c) in cells.iter().enumerate() {
            if k > 0 {
                out.push_str(like.sep());
            }
            out.push_str(&like.quote(c));
        }
    };
    line(&mut out, header);
    for r in rows {
        out.push_str(nl);
        line(&mut out, r);
    }
    if like.trailing() {
        out.push_str(nl);
    }
    out
}
