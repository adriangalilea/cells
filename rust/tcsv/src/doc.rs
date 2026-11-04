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
use crate::prelude::{self, Computed, Op, Prelude};
use crate::read::{self, Part, Reading};
use crate::scan::{self, Fields, Stop};
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

/// A column: stored in the rows, or computed by the prelude.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub name: String,
    /// The type as the header cell spells it after its `:`; nothing for a plain column
    /// and a computed one.
    pub spelled: Option<String>,
    pub ty: Type,
    /// A computed column's expression, as the prelude writes it.
    pub computed: Option<String>,
    pub data: Data,
    /// Set where the cell has a value; clear for a blank or broken cell.
    pub valid: Bits,
}

impl Column {
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

    /// A value as this column writes it: its pair's words, its format, ISO otherwise.
    pub fn form(&self, v: Value) -> String {
        match v {
            Value::Null => String::new(),
            Value::Text(t) | Value::Choice(t) => t.to_owned(),
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

/// A `.tcsv` file read.
#[derive(Clone, Debug)]
pub struct Doc {
    source: String,
    /// Where the header starts: everything before it (a byte order mark, the prelude) is
    /// kept as written.
    body: u32,
    /// Where each row's record starts, and where the last one ends.
    records: Vec<u32>,
    prelude: Prelude,
    columns: Vec<Column>,
    stored: usize,
    problems: Vec<Problem>,
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
    read_on(source.into(), 1)
}

/// `read` on up to `threads` threads, for a large input: the body is split at record
/// boundaries and its parts read at once. The result is `read`'s, byte for byte.
#[cfg(feature = "std")]
pub fn read_parallel(source: impl Into<Vec<u8>>, threads: usize) -> Result<Doc, Error> {
    read_on(source.into(), threads)
}

fn read_on(bytes: Vec<u8>, threads: usize) -> Result<Doc, Error> {
    let source = String::from_utf8(bytes)
        .map_err(|e| error(At::File, format!("is not UTF-8: byte {} begins no character", e.utf8_error().valid_up_to())))?;
    if source.len() >= u32::MAX as usize {
        return Err(error(At::File, "is larger than 4 GiB"));
    }
    let bom = if source.starts_with('\u{feff}') { 3 } else { 0 };
    let (body, prelude) = read_prelude(&source, bom)?;
    let src = source.as_bytes();
    if body == src.len() {
        let records = Vec::from([body as u32]);
        return Ok(Doc {
            source,
            body: body as u32,
            records,
            prelude,
            columns: Vec::new(),
            stored: 0,
            problems: Vec::new(),
            header: None,
            edits: BTreeMap::new(),
            candidates: Vec::new(),
        });
    }
    let (fields, first) = scan::record(src, body).map_err(|s| error(At::Header, stop_why(s, 0)))?;
    let mut scratch = Vec::new();
    let mut heads: Vec<(String, Option<String>, Option<Type>)> = Vec::with_capacity(fields.len());
    for (j, &(s, e)) in fields.iter().enumerate() {
        let at = At::Column(j as u32 + 1);
        let cell = scan::text(src, s, e, &mut scratch);
        // Unquoting cuts at ASCII quotes only, so a field of a `str` is one.
        let cell = core::str::from_utf8(cell).unwrap();
        let (name, spelled) = types::split_header(cell);
        if name.is_empty() {
            return Err(error(at, "a column has a name"));
        }
        if heads.iter().any(|(n, _, _)| n == name) {
            return Err(error(at, format!("two columns are named {name}")));
        }
        let ty = match spelled {
            Some(t) => Some(types::parse_type(t, &prelude.named).map_err(|why| error(at, why))?),
            None => None,
        };
        heads.push((name.to_owned(), spelled.map(str::to_owned), ty));
    }
    let decoders = decoders(heads.iter().map(|(_, _, t)| t.as_ref()));
    let parts = read::body(src, first, &decoders, threads).map_err(|(row, s)| error(At::Row(row), stop_why(s, heads.len())))?;
    let mut part = read::join(parts);
    part.records.push(src.len() as u32);
    let Part { records, columns: readings, mut problems, .. } = part;
    let candidates = readings.iter().map(|r| r.candidates).collect();
    let mut columns = Vec::with_capacity(heads.len() + prelude.computed.len());
    for (j, ((name, spelled, ty), r)) in heads.into_iter().zip(readings).enumerate() {
        let (ty, data, valid) = settle(ty, r, j as u32, threads, &mut problems);
        columns.push(Column { name, spelled, ty, computed: None, data, valid });
    }
    let stored = columns.len();
    let mut doc =
        Doc { source, body: body as u32, records, prelude, columns, stored, problems, header: None, edits: BTreeMap::new(), candidates };
    doc.compute()?;
    doc.problems.sort_by_key(|p| p.order());
    Ok(doc)
}

fn decoders<'a>(types: impl Iterator<Item = Option<&'a Type>>) -> Vec<(Decoder, bool)> {
    types.map(|t| (Decoder::of(t), t.is_none_or(|t| t.optional))).collect()
}

/// The prelude, if the file opens with one after its byte order mark, and where the
/// header starts.
fn read_prelude(source: &str, from: usize) -> Result<(usize, Prelude), Error> {
    fn line_of(l: &str) -> &str {
        l.strip_suffix('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).unwrap_or(l)
    }
    let rest = &source[from..];
    let mut lines = rest.split_inclusive('\n');
    let Some(first) = lines.next() else { return Ok((from, Prelude::default())) };
    if line_of(first) != "---" {
        return Ok((from, Prelude::default()));
    }
    if !first.ends_with('\n') {
        return Err(error(At::Prelude, "opens with --- and never closes"));
    }
    let mut at = from + first.len();
    let mut inside = Vec::new();
    for l in lines {
        at += l.len();
        if line_of(l) == "---" {
            let p = prelude::parse(&inside).map_err(|(n, why)| error(At::PreludeLine(n), why))?;
            return Ok((at, p));
        }
        inside.push(line_of(l));
    }
    Err(error(At::Prelude, "opens with --- and never closes"))
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
                // Inferred columns are never reported: values the base cannot hold together
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

impl Doc {
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Every column: the header's, then the prelude's computed ones.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// How many of `columns` are stored in the rows (the rest are computed).
    pub fn stored(&self) -> usize {
        self.stored
    }

    pub fn rows(&self) -> usize {
        self.records.len() - 1
    }

    /// Every problem, by row, then column.
    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }

    pub fn named(&self) -> &[NamedType] {
        &self.prelude.named
    }

    pub fn computed(&self) -> &[Computed] {
        &self.prelude.computed
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
        s.strip_suffix('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).unwrap_or(s)
    }

    /// A record's fields as written (quotes included), from text that read once already.
    fn raw_fields(line: &str) -> Vec<&str> {
        let (spans, _) = scan::record(line.as_bytes(), 0).unwrap();
        spans.iter().map(|&(s, e)| &line[s as usize..e as usize]).collect()
    }

    /// A stored cell's text as written, unquoted; blank for a field the row leaves out.
    pub fn text(&self, row: usize, column: usize) -> Cow<'_, str> {
        assert!(column < self.stored, "column {column} is not stored");
        let fields = Self::raw_fields(self.line(row));
        let Some(raw) = fields.get(column) else { return Cow::Borrowed("") };
        let Some(inner) = raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')) else { return Cow::Borrowed(raw) };
        if inner.contains('"') { Cow::Owned(inner.replace("\"\"", "\"")) } else { Cow::Borrowed(inner) }
    }

    /// A problem in words: `row 3, cost: "8x.50" is not a number`.
    pub fn describe(&self, p: &Problem) -> String {
        let c = &self.columns[p.column as usize];
        let text = self.text(p.row as usize, p.column as usize);
        let bound = |b: &Option<types::Bound>| b.map(|b| b.value.normal().to_string()).unwrap_or_default();
        let length = matches!(c.ty.base, Base::Text);
        let why = match p.why {
            Why::Missing => return format!("row {}, {}: is missing", p.row, c.name),
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
        format!("row {}, {}: \"{}\" {}", p.row, c.name, text, why)
    }

    /// The header's line break, the one new lines take; `\n` when it has none.
    pub fn newline(&self) -> &str {
        let header = &self.source[self.body as usize..self.records[0] as usize];
        if header.ends_with("\r\n") { "\r\n" } else { "\n" }
    }

    /// The text before the header, the byte order mark and the prelude, as written.
    pub fn prelude_text(&self) -> &str {
        &self.source[..self.body as usize]
    }

    /// Whether the file ends in a line break.
    pub fn trailing(&self) -> bool {
        self.source.ends_with('\n')
    }

    fn terminator(&self, row: usize) -> &str {
        let s = &self.source[self.records[row] as usize..self.records[row + 1] as usize];
        if s.ends_with("\r\n") {
            "\r\n"
        } else if s.ends_with('\n') {
            "\n"
        } else {
            ""
        }
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

    /// One cell's text changed; every other byte stays as read. The cell is written in
    /// its file form (quoted only where CSV needs it, the blank fields before it in a
    /// short row) and decoded again, with its problems, its key's duplicates and the
    /// computed columns.
    pub fn set_cell(&mut self, row: usize, column: usize, text: &str) {
        assert!(column < self.stored, "column {column} is not stored");
        assert!(row < self.rows(), "row {row} does not exist");
        let mut fields: Vec<Cow<'_, str>> = Self::raw_fields(self.line(row)).into_iter().map(Cow::Borrowed).collect();
        while fields.len() <= column {
            fields.push(Cow::Borrowed(""));
        }
        fields[column] = scan::quote(text);
        let mut joined = fields.join(",");
        if joined.is_empty() && self.terminator(row).is_empty() {
            // An empty last line with no break after it is no record: the blank is quoted.
            joined.push_str("\"\"");
        }
        self.edits.insert(row as u32, joined);
        self.decode_cell(row, column, text);
    }

    /// One cell set to a value, written in its column's form.
    pub fn set_value(&mut self, row: usize, column: usize, v: Value) {
        let text = self.columns[column].form(v);
        self.set_cell(row, column, &text);
    }

    fn decode_cell(&mut self, row: usize, column: usize, text: &str) {
        let col = &mut self.columns[column];
        let why = match col.ty.inferred {
            true => None,
            false => put(col, row, text),
        };
        let Some(why) = why else { return self.rebuild(column) };
        let at = self.problems.partition_point(|p| p.order() < (row as u32, column as u32));
        let end = at + self.problems[at..].iter().take_while(|p| p.order() == (row as u32, column as u32)).count();
        let fresh: Vec<Problem> = why.map(|why| Problem { row: row as u32, column: column as u32, why }).into_iter().collect();
        self.problems.splice(at..end, fresh);
        if self.columns[column].ty.key {
            self.rekey(column);
        }
        self.compute_values();
    }

    /// A stored column read again from its cells, as a fresh read would.
    fn rebuild(&mut self, column: usize) {
        let rows = self.rows();
        let mut raw = String::new();
        let mut spans = Vec::with_capacity(rows);
        for r in 0..rows {
            let fields = Self::raw_fields(self.line(r));
            let s = raw.len() as u32;
            raw.push_str(fields.get(column).copied().unwrap_or(""));
            spans.push((s, raw.len() as u32));
        }
        let c = &self.columns[column];
        let ty = if c.ty.inferred { None } else { Some(c.ty.clone()) };
        let decoders = decoders(core::iter::once(ty.as_ref()));
        let mut part = Part::new(raw.as_bytes(), 0, raw.len(), &decoders);
        for &(s, e) in &spans {
            part.field(s, e).unwrap();
            part.record(e);
        }
        part.finish();
        let mut problems = core::mem::take(&mut part.problems);
        let reading = part.columns.pop().unwrap();
        self.candidates[column] = reading.candidates;
        for p in &mut problems {
            p.column = column as u32;
        }
        let (ty, data, valid) = settle(ty, reading, column as u32, 1, &mut problems);
        let col = &mut self.columns[column];
        col.ty = ty;
        col.data = data;
        col.valid = valid;
        self.problems.retain(|p| p.column != column as u32);
        self.problems.extend(problems);
        self.problems.sort_by_key(|p| p.order());
        self.compute_values();
    }

    /// The file grown: when `bytes` is this file's bytes with records after them (a
    /// ledger appended to), only the new records are read and joined to the columns
    /// already held; anything else (an edit pending, a change before the end, a type the
    /// new rows would infer otherwise) is read whole. Either way the result is what `read`
    /// gives for `bytes`.
    pub fn extend(mut self, bytes: Vec<u8>) -> Result<Doc, Error> {
        let from = self.source.len();
        let appended = !self.changed()
            && self.stored > 0
            && bytes.len() > from
            && self.source.ends_with('\n')
            && bytes.starts_with(self.source.as_bytes());
        if !appended {
            return read(bytes);
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
        let stored = &self.columns[..self.stored];
        let decoders = decoders(stored.iter().map(|c| (!c.ty.inferred).then_some(&c.ty)));
        let tail = read::sequential(source.as_bytes(), from, &decoders)
            .map_err(|(row, s)| error(At::Row(old_rows + row), stop_why(s, self.stored)))?;
        let Part { records, columns: readings, problems, .. } = tail;
        let mut joined = Vec::with_capacity(self.stored);
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
                return read(source);
            }
            self.candidates[c] = candidates;
            if base == Base::Text {
                joined.push((Building::Text { offsets, bytes }, r.valid));
                continue;
            }
            match redecode(&base, &offsets, &bytes, &r.valid) {
                Some(b) => joined.push(b),
                None => return read(source),
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
        for c in 0..self.stored {
            if self.columns[c].ty.key {
                let col = &self.columns[c];
                crate::keys::check(&col.data, &col.valid, c as u32, 1, &mut self.problems);
            }
        }
        self.compute_values();
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

    /// One column's type rewritten, in the shorthand a writer writes (`""` makes it a
    /// plain column again). No cell is rewritten: a cell the new type breaks is reported.
    pub fn set_type(&mut self, column: usize, spelled: &str) -> Result<(), Error> {
        assert!(column < self.stored, "column {column} is not stored");
        let at = At::Column(column as u32 + 1);
        let ty =
            if spelled.is_empty() { None } else { Some(types::parse_type(spelled, &self.prelude.named).map_err(|why| error(at, why))?) };
        for c in &self.prelude.computed {
            let reads = c.program.iter().any(|op| matches!(op, Op::Column(k) if *k == column));
            if reads && ty.as_ref().is_some_and(|t| !t.base.is_number()) {
                return Err(error(at, format!("{} computes from it, so it stays a number", c.name)));
            }
        }
        let shorthand = ty.as_ref().map(Type::shorthand);
        let name = self.columns[column].name.clone();
        let cell = match &shorthand {
            Some(s) => format!("{name}:{s}"),
            None => name,
        };
        let line = match &self.header {
            Some(h) => h.clone(),
            None => self.source[self.body as usize..self.records[0] as usize].trim_end_matches(['\r', '\n']).to_owned(),
        };
        let mut fields: Vec<Cow<'_, str>> = Self::raw_fields(&line).into_iter().map(Cow::Borrowed).collect();
        fields[column] = scan::quote(&cell);
        let joined = fields.join(",");
        self.header = Some(joined);
        let col = &mut self.columns[column];
        col.spelled = shorthand;
        col.ty = ty.unwrap_or_else(|| Type::inferred(Base::Text));
        self.rebuild(column);
        Ok(())
    }

    /// Resolves the computed columns' names and evaluates them.
    fn compute(&mut self) -> Result<(), Error> {
        for k in 0..self.prelude.computed.len() {
            let c = &mut self.prelude.computed[k];
            let at = At::PreludeLine(c.line);
            if self.columns[..self.stored].iter().any(|col| col.name == c.name) {
                return Err(error(at, format!("a column is named {} already", c.name)));
            }
            for op in &mut c.program {
                if let Op::Name(n) = op {
                    let Some(j) = self.columns.iter().position(|col| col.name == *n) else {
                        return Err(error(at, format!("no column \"{n}\" above it")));
                    };
                    if !self.columns[j].ty.base.is_number() {
                        return Err(error(at, format!("\"{n}\" is {}, not a number", self.columns[j].ty.base.tag())));
                    }
                    *op = Op::Column(j);
                }
            }
            let c = &self.prelude.computed[k];
            let (data, valid) = evaluate(&self.columns, c, self.rows());
            let ty = Type { optional: true, ..Type::plain(Base::Num) };
            self.columns.push(Column { name: c.name.clone(), spelled: None, ty, computed: Some(c.expression.clone()), data, valid });
        }
        Ok(())
    }

    fn compute_values(&mut self) {
        for k in 0..self.prelude.computed.len() {
            let (data, valid) = evaluate(&self.columns, &self.prelude.computed[k], self.rows());
            let col = &mut self.columns[self.stored + k];
            col.data = data;
            col.valid = valid;
        }
    }
}

/// A numeric cell as an exact decimal.
fn exact(c: &Column, row: usize) -> Option<Exact> {
    if !c.valid.get(row) {
        return None;
    }
    Some(match &c.data {
        Data::Int(v) => Exact::from_int(v[row]),
        Data::Decimal { values, scale } => Exact::from_decimal(Decimal { mantissa: values[row], scale: *scale }),
        _ => unreachable!("a computed column reads numbers"),
    })
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

/// A computed column's values from the columns before it, stored as `Decimal128(38, s)`:
/// `s` the most digits after the point any value has, less what the largest value's
/// integer digits need; a value with more digits after the point is rounded half to even
/// to `s`, one with more than 38 before it is blank.
fn evaluate(columns: &[Column], c: &Computed, rows: usize) -> (Data, Bits) {
    let results: Vec<Option<Exact>> = (0..rows).map(|r| prelude::evaluate(c, |k| exact(&columns[k], r))).collect();
    let fits = |e: &Exact| e.int_digits() <= value::PRECISION;
    let int = results.iter().flatten().filter(|e| fits(e)).map(Exact::int_digits).max().unwrap_or(0);
    let frac = results.iter().flatten().filter(|e| fits(e)).map(|e| e.scale).max().unwrap_or(0);
    let scale = frac.min(value::PRECISION - int);
    let mut values = Vec::with_capacity(rows);
    let mut valid = BitsBuilder::with_capacity(rows);
    for r in &results {
        let v = r.as_ref().filter(|e| fits(e)).and_then(|e| e.to_i128(scale));
        values.push(v.unwrap_or(0));
        valid.push(v.is_some());
    }
    (Data::Decimal { values: Arc::new(values), scale: scale as u8 }, valid.finish())
}

/// Writes one cell's value into its column in place and says why it is a problem, if it
/// is one; nothing when the column has to be read again whole (a value with more digits
/// after the point than its column's scale or unit).
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
    Some(Column { name: String::new(), spelled: None, ty: col.ty.clone(), computed: None, data: one.finish(), valid: valid.finish() })
}

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
/// mark and prelude, its header's line break and its final line break, every field quoted
/// only where CSV needs it.
pub fn compose(like: &Doc, header: &[&str], rows: &[Vec<&str>]) -> String {
    let nl = like.newline();
    let mut out = String::from(like.prelude_text());
    let line = |out: &mut String, cells: &[&str]| {
        for (k, c) in cells.iter().enumerate() {
            if k > 0 {
                out.push(',');
            }
            out.push_str(&scan::quote(c));
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
