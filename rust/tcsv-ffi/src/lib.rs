//! The tcsv core's C ABI. A caller hands the file's bytes in and gets a `TcsvDoc`; the
//! columns leave through the Arrow C Data Interface (`tcsv_export`), their buffers shared,
//! never copied; everything else is a handful of calls. `tcsv.h` is generated from this
//! file by cbindgen.
//!
//! Text the core returns is a `TcsvStr`: UTF-8, not NUL-terminated, borrowed from the doc
//! and valid until the next call on it.

use std::ffi::c_char;
use std::ptr;

use tcsv::arrow::{ArrowArray, ArrowSchema};
use tcsv::{At, ColumnAt, Dialect, Doc, Separator};

/// A file read, with the text its last call returned.
pub struct TcsvDoc {
    doc: Doc,
    out: String,
}

/// UTF-8 bytes, borrowed: valid until the next call on the doc that returned them.
#[repr(C)]
pub struct TcsvStr {
    pub ptr: *const c_char,
    pub len: usize,
}

impl TcsvStr {
    fn of(s: &str) -> TcsvStr {
        TcsvStr { ptr: s.as_ptr() as *const c_char, len: s.len() }
    }
}

/// Where a file's grammar breaks, in the suite's words.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TcsvAt {
    /// Nothing broke.
    TcsvAtNone = 0,
    /// The header line.
    TcsvAtHeader = 1,
    /// A header cell, `number` 1-based.
    TcsvAtColumn = 2,
    /// A prelude line, `number` 1-based inside the fences.
    TcsvAtPreludeLine = 3,
    /// The prelude as a whole.
    TcsvAtPrelude = 4,
    /// A data row, `number` from 0.
    TcsvAtRow = 5,
    /// The file as a whole: not UTF-8, or larger than 4 GiB.
    TcsvAtFile = 6,
}

/// Why a call failed: the place, and the reason in words (owned by the caller once set;
/// `tcsv_failure_free` lets it go).
#[repr(C)]
pub struct TcsvFailure {
    pub at: TcsvAt,
    pub number: u32,
    pub why: *mut c_char,
    pub why_len: usize,
}

fn fail(out: *mut TcsvFailure, e: tcsv::Error) {
    if out.is_null() {
        return;
    }
    let (at, number) = match e.at {
        At::Header => (TcsvAt::TcsvAtHeader, 0),
        At::Column(n) => (TcsvAt::TcsvAtColumn, n),
        At::PreludeLine(n) => (TcsvAt::TcsvAtPreludeLine, n),
        At::Prelude => (TcsvAt::TcsvAtPrelude, 0),
        At::Row(n) => (TcsvAt::TcsvAtRow, n),
        At::File => (TcsvAt::TcsvAtFile, 0),
    };
    let why = e.why.into_bytes().into_boxed_slice();
    let why_len = why.len();
    // SAFETY: the caller hands a writable TcsvFailure.
    unsafe {
        *out = TcsvFailure { at, number, why: Box::into_raw(why) as *mut c_char, why_len };
    }
}

/// Lets a failure's words go.
///
/// # Safety
/// `f` points at a failure a call set, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_failure_free(f: *mut TcsvFailure) {
    // SAFETY: the caller's promise; `why` came from a boxed slice of `why_len` bytes.
    unsafe {
        let f = &mut *f;
        if !f.why.is_null() {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(f.why as *mut u8, f.why_len)));
        }
        f.why = ptr::null_mut();
        f.why_len = 0;
        f.at = TcsvAt::TcsvAtNone;
    }
}

/// Reads `len` bytes (copied: the caller keeps its buffer) on up to `threads` threads (1
/// starts none). Null when the file does not read, `failure` then set.
///
/// # Safety
/// `bytes` points at `len` readable bytes; `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_read(bytes: *const u8, len: usize, threads: usize, failure: *mut TcsvFailure) -> *mut TcsvDoc {
    // SAFETY: the caller's promise.
    unsafe { tcsv_read_as(bytes, len, 0, threads, failure) }
}

/// The dialect a call names: 0 for TCSV, else a plain file's separator (`,` `;` `\t` `|`).
fn dialect(sep: u8) -> Dialect {
    if sep == 0 {
        return Dialect::Tcsv;
    }
    Dialect::Plain(Separator::of(sep).unwrap_or_else(|| panic!("{sep} is no separator")))
}

fn doc_or_fail(read: Result<Doc, tcsv::Error>, failure: *mut TcsvFailure) -> *mut TcsvDoc {
    match read {
        Ok(doc) => Box::into_raw(Box::new(TcsvDoc { doc, out: String::new() })),
        Err(e) => {
            fail(failure, e);
            ptr::null_mut()
        }
    }
}

/// `tcsv_read` in a dialect: `sep` 0 for TCSV, else a plain file's separator byte (`,`
/// `;` `\t` `|`), read leniently: a plain file is refused only for bytes that are not
/// UTF-8, or past 4 GiB.
///
/// # Safety
/// As `tcsv_read`; `sep` is 0 or a separator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_read_as(bytes: *const u8, len: usize, sep: u8, threads: usize, failure: *mut TcsvFailure) -> *mut TcsvDoc {
    // SAFETY: the caller's promise.
    let src = if len == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec() };
    doc_or_fail(tcsv::read_as(src, dialect(sep), threads.max(1)), failure)
}

/// The separator byte the first lines of `len` bytes agree on (`tcsv::detect`).
///
/// # Safety
/// `bytes` points at `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_detect(bytes: *const u8, len: usize) -> u8 {
    // SAFETY: the caller's promise.
    let src = if len == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(bytes, len) } };
    tcsv::detect(src).byte()
}

/// How the doc was read: 0 for TCSV, else a plain file's separator byte.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_dialect(d: *mut TcsvDoc) -> u8 {
    // SAFETY: the caller's promise.
    match unsafe { doc(d) }.doc.dialect() {
        Dialect::Tcsv => 0,
        Dialect::Plain(s) => s.byte(),
    }
}

/// Whether a plain file quotes every field, so an edited cell and a new row are quoted too.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_quotes_all(d: *mut TcsvDoc) -> bool {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.quotes_all()
}

/// A plain file's header cell that names its column badly: blank, or the name of the
/// column `first` too.
#[repr(C)]
pub struct TcsvNaming {
    pub column: u32,
    pub blank: bool,
    pub first: u32,
}

/// How many header cells name their columns badly.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_naming_count(d: *mut TcsvDoc) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.naming().len()
}

/// Header problem `i`.
///
/// # Safety
/// `d` is a live doc; `i < tcsv_naming_count(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_naming(d: *mut TcsvDoc, i: usize) -> TcsvNaming {
    // SAFETY: the caller's promise.
    match unsafe { doc(d) }.doc.naming()[i] {
        tcsv::Naming::Blank { column } => TcsvNaming { column, blank: true, first: 0 },
        tcsv::Naming::Twice { column, first } => TcsvNaming { column, blank: false, first },
    }
}

/// Header problem `i` in words without its place: `has no name`, `"a" names column 1 too`.
///
/// # Safety
/// `d` is a live doc; `i < tcsv_naming_count(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_naming_why(d: *mut TcsvDoc, i: usize) -> TcsvStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.naming_why(&d.doc.naming()[i]);
    TcsvStr::of(&d.out)
}

/// How many fields a row's record holds now (a plain file's ragged row, a short row).
///
/// # Safety
/// `d` is a live doc; `row < tcsv_rows(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_width(d: *mut TcsvDoc, row: usize) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.width(row)
}

/// A column of a file reshaped: `kept`, a stored column it has (renamed to `name` when
/// `renamed`), or when `kept` is negative a new one, its header cell `name` and `count`
/// cells, one a row, or none for blanks.
#[repr(C)]
pub struct TcsvColumnAt {
    pub kept: i64,
    pub renamed: bool,
    pub name: TcsvStr,
    pub cells: *const TcsvStr,
    pub count: usize,
}

/// The file with exactly `columns`, in their order, read again on up to `threads` threads
/// (a column inserted, removed, moved, renamed): a new doc, this one untouched. Null when
/// it does not read, `failure` then set.
///
/// # Safety
/// `d` is a live doc; `columns` points at `count` columns, at least one, each kept one
/// below `tcsv_stored(d)` and every string UTF-8; `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_reshaped(
    d: *mut TcsvDoc,
    columns: *const TcsvColumnAt,
    count: usize,
    threads: usize,
    failure: *mut TcsvFailure,
) -> *mut TcsvDoc {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    let given = if count == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(columns, count) } };
    // SAFETY: every string is the caller's UTF-8.
    let parts: Vec<(&str, Vec<&str>)> =
        given.iter().map(|c| unsafe { (text_of(c.name.ptr, c.name.len), strs(c.cells, c.count)) }).collect();
    let order: Vec<ColumnAt> = given
        .iter()
        .zip(&parts)
        .map(|(c, (name, cells))| match (c.kept < 0, c.renamed) {
            (true, _) => ColumnAt::New(name, cells),
            (false, true) => ColumnAt::Renamed(c.kept as usize, name),
            (false, false) => ColumnAt::Kept(c.kept as usize),
        })
        .collect();
    doc_or_fail(d.doc.reshaped(&order, threads.max(1)), failure)
}

/// A change of a computed column: computed column `index` given `name` and `expression`,
/// a new one, or `index` taken out.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TcsvComputing {
    TcsvComputingSet = 0,
    TcsvComputingNew = 1,
    TcsvComputingRemoved = 2,
}

/// The file with one computed column changed, its prelude line alone rewritten, read again
/// as a new doc on up to `threads` threads; this one untouched. Null when the prelude no
/// longer reads (an expression that does not parse, a name taken, a column it reads
/// missing or no number), `failure` then set at its prelude line.
///
/// # Safety
/// `d` is a live `.tcsv` doc; `index` below its computed columns for Set and Removed;
/// `name` and `expression` UTF-8 (unread for Removed); `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_recomputed(
    d: *mut TcsvDoc,
    change: TcsvComputing,
    index: usize,
    name: TcsvStr,
    expression: TcsvStr,
    threads: usize,
    failure: *mut TcsvFailure,
) -> *mut TcsvDoc {
    // SAFETY: the caller's promise.
    let (d, name, expression) = unsafe { (doc(d), text_of(name.ptr, name.len), text_of(expression.ptr, expression.len)) };
    let change = match change {
        TcsvComputing::TcsvComputingSet => tcsv::Computing::Set(index, name, expression),
        TcsvComputing::TcsvComputingNew => tcsv::Computing::New(name, expression),
        TcsvComputing::TcsvComputingRemoved => tcsv::Computing::Removed(index),
    };
    doc_or_fail(d.doc.recomputed(change, threads.max(1)), failure)
}

/// The file written in a dialect (`sep` as for `tcsv_read_as`): a plain file's header the
/// columns' names, computed ones with their values; a `.tcsv` file's header the `count`
/// cells of `header` when `count` is not 0 (one a stored column), its prelude kept.
///
/// # Safety
/// `d` is a live doc; `sep` is 0 or a separator; `header` points at `count` strings of
/// UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_written_as(d: *mut TcsvDoc, sep: u8, header: *const TcsvStr, count: usize) -> TcsvStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    // SAFETY: the caller's promise.
    let cells = unsafe { strs(header, count) };
    d.out = d.doc.written_as(dialect(sep), if count == 0 { None } else { Some(&cells) });
    TcsvStr::of(&d.out)
}

/// Lets a doc go. The arrays exported from it live on until released.
///
/// # Safety
/// `d` came from `tcsv_read`, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_free(d: *mut TcsvDoc) {
    // SAFETY: the caller's promise.
    drop(unsafe { Box::from_raw(d) });
}

/// # Safety
/// `d` is a live doc.
unsafe fn doc<'a>(d: *mut TcsvDoc) -> &'a mut TcsvDoc {
    assert!(!d.is_null(), "a null doc");
    // SAFETY: the caller's promise.
    unsafe { &mut *d }
}

/// The rows.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_rows(d: *mut TcsvDoc) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.rows()
}

/// Every column: the header's, then the computed ones.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_columns(d: *mut TcsvDoc) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.columns().len()
}

/// How many columns are stored in the rows; the rest are computed.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_stored(d: *mut TcsvDoc) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.stored()
}

/// A column's name.
///
/// # Safety
/// `d` is a live doc; `column < tcsv_columns(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_column_name(d: *mut TcsvDoc, column: usize) -> TcsvStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.columns()[column].name.clone();
    TcsvStr::of(&d.out)
}

/// A column's type in canonical shorthand: `EUR>=0`, `text!`, `yes/no?`.
///
/// # Safety
/// `d` is a live doc; `column < tcsv_columns(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_column_type(d: *mut TcsvDoc, column: usize) -> TcsvStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.columns()[column].ty.shorthand();
    TcsvStr::of(&d.out)
}

/// What a problem is: the suite's kinds.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TcsvKind {
    TcsvKindType = 0,
    TcsvKindMissing = 1,
    TcsvKindKey = 2,
    /// A plain file's row of another width than its header.
    TcsvKindRagged = 3,
}

/// A cell that breaks its type, by place.
#[repr(C)]
pub struct TcsvProblem {
    pub row: u32,
    pub column: u32,
    pub kind: TcsvKind,
}

/// How many problems, by row then column.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_problem_count(d: *mut TcsvDoc) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.problems().len()
}

/// Problem `i`.
///
/// # Safety
/// `d` is a live doc; `i < tcsv_problem_count(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_problem(d: *mut TcsvDoc, i: usize) -> TcsvProblem {
    // SAFETY: the caller's promise.
    let p = unsafe { doc(d) }.doc.problems()[i];
    let kind = match p.kind() {
        tcsv::Kind::Type => TcsvKind::TcsvKindType,
        tcsv::Kind::Missing => TcsvKind::TcsvKindMissing,
        tcsv::Kind::Key => TcsvKind::TcsvKindKey,
        tcsv::Kind::Ragged => TcsvKind::TcsvKindRagged,
    };
    TcsvProblem { row: p.row, column: p.column, kind }
}

/// Problem `i` in words: `row 3, cost: "8x.50" is not a number`.
///
/// # Safety
/// `d` is a live doc; `i < tcsv_problem_count(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_describe(d: *mut TcsvDoc, i: usize) -> TcsvStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.describe(&d.doc.problems()[i]);
    TcsvStr::of(&d.out)
}

/// A stored cell's text as written, unquoted.
///
/// # Safety
/// `d` is a live doc; `row < tcsv_rows(d)`, `column < tcsv_stored(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_text(d: *mut TcsvDoc, row: usize, column: usize) -> TcsvStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.text(row, column).into_owned();
    TcsvStr::of(&d.out)
}

/// The file's text: what was read, but for what was changed.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_write(d: *mut TcsvDoc) -> TcsvStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.write();
    TcsvStr::of(&d.out)
}

/// # Safety
/// `ptr` points at `len` bytes of UTF-8.
unsafe fn text_of<'a>(ptr: *const c_char, len: usize) -> &'a str {
    if len == 0 {
        return "";
    }
    // SAFETY: the caller's promise.
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(ptr as *const u8, len) }).expect("the caller hands UTF-8")
}

/// One cell's text changed; every other byte stays as read. False when the text holds a
/// control character, the doc untouched and `failure` set.
///
/// # Safety
/// `d` is a live doc; `row < tcsv_rows(d)`, `column < tcsv_stored(d)`; `text` points at
/// `len` bytes of UTF-8; `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_set_cell(
    d: *mut TcsvDoc,
    row: usize,
    column: usize,
    text: *const c_char,
    len: usize,
    failure: *mut TcsvFailure,
) -> bool {
    // SAFETY: the caller's promise.
    let (d, t) = unsafe { (doc(d), text_of(text, len)) };
    match d.doc.set_cell(row, column, t) {
        Ok(()) => true,
        Err(e) => {
            fail(failure, e);
            false
        }
    }
}

/// One column's type rewritten in shorthand (`""` makes it plain). False when the type
/// does not read, `failure` then set.
///
/// # Safety
/// `d` is a live doc; `column < tcsv_stored(d)`; `spelled` points at `len` bytes of UTF-8;
/// `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_set_type(
    d: *mut TcsvDoc,
    column: usize,
    spelled: *const c_char,
    len: usize,
    failure: *mut TcsvFailure,
) -> bool {
    // SAFETY: the caller's promise.
    let (d, t) = unsafe { (doc(d), text_of(spelled, len)) };
    match d.doc.set_type(column, t) {
        Ok(()) => true,
        Err(e) => {
            fail(failure, e);
            false
        }
    }
}

/// The columns as one Arrow struct array, written into the caller's two structs. The
/// buffers are the doc's own, shared: the caller releases each struct once (its
/// `release`), before or after `tcsv_free`. Each field carries its TCSV type under the
/// metadata key `tcsv.type`.
///
/// # Safety
/// `d` is a live doc; `schema` and `array` are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_export(d: *mut TcsvDoc, schema: *mut ArrowSchema, array: *mut ArrowArray) {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    let (s, a) = tcsv::arrow::export(&d.doc);
    // SAFETY: the caller's promise; the structs are moved, as the interface allows.
    unsafe {
        schema.write(s);
        array.write(a);
    }
}

/// One column alone, as `tcsv_export`.
///
/// # Safety
/// `d` is a live doc; `column < tcsv_columns(d)`; `schema` and `array` are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_export_column(d: *mut TcsvDoc, column: usize, schema: *mut ArrowSchema, array: *mut ArrowArray) {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    let (s, a) = tcsv::arrow::export_column(&d.doc, column);
    // SAFETY: as `tcsv_export`.
    unsafe {
        schema.write(s);
        array.write(a);
    }
}

/// A copy of a doc to change on its own: the columns' buffers shared until one is written.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_clone(d: *mut TcsvDoc) -> *mut TcsvDoc {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    Box::into_raw(Box::new(TcsvDoc { doc: d.doc.clone(), out: String::new() }))
}

/// A new file in a dialect (`sep` as for `tcsv_read_as`) written from cells: its header's,
/// then each row's (`rows`, every one new), each quoted only where the dialect needs it,
/// lines broken by `\r\n` or `\n`, ending in one when `trailing`. Null when it does not
/// read, `failure` then set.
///
/// # Safety
/// `header` points at `columns` strings of UTF-8, `rows` at `count` new rows; `sep` is 0
/// or a separator; `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_new(
    header: *const TcsvStr,
    columns: usize,
    rows: *const TcsvRow,
    count: usize,
    sep: u8,
    crlf: bool,
    trailing: bool,
    failure: *mut TcsvFailure,
) -> *mut TcsvDoc {
    let nl = if crlf { "\r\n" } else { "\n" };
    let dialect = dialect(sep);
    let between = (dialect.separator().byte() as char).to_string();
    let line = |text: &mut String, cells: &[&str]| {
        for (k, c) in cells.iter().enumerate() {
            if k > 0 {
                text.push_str(&between);
            }
            text.push_str(&dialect.quote(c));
        }
    };
    let mut text = String::new();
    // SAFETY: the caller's promise.
    line(&mut text, &unsafe { strs(header, columns) });
    let given = if count == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(rows, count) } };
    for r in given {
        assert!(r.kept < 0, "a new file's rows are all new");
        text.push_str(nl);
        // SAFETY: the caller's promise.
        line(&mut text, &unsafe { strs(r.cells, r.count) });
    }
    if trailing {
        text.push_str(nl);
    } else if !given.is_empty() && text.ends_with(nl) {
        text.push_str("\"\"");
    }
    doc_or_fail(tcsv::read_as(text, dialect, 1), failure)
}

/// # Safety
/// `ptr` points at `count` strings of UTF-8.
unsafe fn strs<'a>(ptr: *const TcsvStr, count: usize) -> Vec<&'a str> {
    if count == 0 {
        return Vec::new();
    }
    // SAFETY: the caller's promise.
    unsafe { std::slice::from_raw_parts(ptr, count) }
        .iter()
        // SAFETY: each is the caller's UTF-8.
        .map(|s| unsafe { text_of(s.ptr, s.len) })
        .collect()
}

/// What a column holds.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TcsvBase {
    TcsvBaseText = 0,
    TcsvBaseInt = 1,
    TcsvBaseNum = 2,
    TcsvBasePct = 3,
    TcsvBaseBool = 4,
    TcsvBaseDate = 5,
    TcsvBaseTime = 6,
    TcsvBaseDateTime = 7,
    TcsvBaseUrl = 8,
    TcsvBaseEmail = 9,
    TcsvBaseId = 10,
    TcsvBaseMoney = 11,
    TcsvBaseChoice = 12,
}

/// Which type a call reads: a column's, or the prelude's named type at an index.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TcsvOf {
    TcsvOfColumn = 0,
    TcsvOfNamed = 1,
}

/// A type's shape; its words through `tcsv_type_part`.
#[repr(C)]
pub struct TcsvType {
    pub base: TcsvBase,
    pub key: bool,
    pub optional: bool,
    /// Read from the values: the header said nothing.
    pub inferred: bool,
    /// A computed column's.
    pub computed: bool,
    /// The type uses a named one (`tcsv_type_part` Named is its name).
    pub named: bool,
    pub has_format: bool,
    pub has_min: bool,
    pub min_inclusive: bool,
    pub has_max: bool,
    pub max_inclusive: bool,
    /// A choice's words, or a boolean's two (true, then false).
    pub words: usize,
}

/// A type's words, read one at a time.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TcsvPart {
    /// The column's name, or the named type's.
    TcsvPartName = 0,
    /// The canonical shorthand: what a writer writes.
    TcsvPartShorthand = 1,
    /// As the header cell spells it after its `:` (empty for a plain column).
    TcsvPartSpelled = 2,
    /// A computed column's expression, as the prelude writes it.
    TcsvPartExpression = 3,
    TcsvPartCurrency = 4,
    TcsvPartFormat = 5,
    /// The bounds, as decimals.
    TcsvPartMin = 6,
    TcsvPartMax = 7,
    /// Word `index`: a choice's, or a boolean's (0 true, 1 false).
    TcsvPartWord = 8,
    /// The named type the type uses.
    TcsvPartNamed = 9,
}

/// How many named types the prelude declares.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_named_count(d: *mut TcsvDoc) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.named().len()
}

fn type_of(d: &TcsvDoc, of: TcsvOf, index: usize) -> (&str, &tcsv::Type, Option<&tcsv::Column>) {
    match of {
        TcsvOf::TcsvOfColumn => {
            let c = &d.doc.columns()[index];
            (&c.name, &c.ty, Some(c))
        }
        TcsvOf::TcsvOfNamed => {
            let n = &d.doc.named()[index];
            (&n.name, &n.ty, None)
        }
    }
}

/// A type's shape.
///
/// # Safety
/// `d` is a live doc; `index` is below `tcsv_columns` or `tcsv_named_count`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_type(d: *mut TcsvDoc, of: TcsvOf, index: usize) -> TcsvType {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    let (_, t, column) = type_of(d, of, index);
    use tcsv::Base as B;
    let (base, words) = match &t.base {
        B::Text => (TcsvBase::TcsvBaseText, 0),
        B::Int => (TcsvBase::TcsvBaseInt, 0),
        B::Num => (TcsvBase::TcsvBaseNum, 0),
        B::Pct => (TcsvBase::TcsvBasePct, 0),
        B::Bool { .. } => (TcsvBase::TcsvBaseBool, 2),
        B::Date => (TcsvBase::TcsvBaseDate, 0),
        B::Time => (TcsvBase::TcsvBaseTime, 0),
        B::DateTime => (TcsvBase::TcsvBaseDateTime, 0),
        B::Url => (TcsvBase::TcsvBaseUrl, 0),
        B::Email => (TcsvBase::TcsvBaseEmail, 0),
        B::Id => (TcsvBase::TcsvBaseId, 0),
        B::Money(_) => (TcsvBase::TcsvBaseMoney, 0),
        B::Choice(w) => (TcsvBase::TcsvBaseChoice, w.len()),
    };
    TcsvType {
        base,
        key: t.key,
        optional: t.optional,
        inferred: t.inferred,
        computed: column.is_some_and(|c| c.computed.is_some()),
        named: t.named.is_some(),
        has_format: t.format.is_some(),
        has_min: t.min.is_some(),
        min_inclusive: t.min.is_some_and(|b| b.inclusive),
        has_max: t.max.is_some(),
        max_inclusive: t.max.is_some_and(|b| b.inclusive),
        words,
    }
}

/// One of a type's words; empty where the type has none.
///
/// # Safety
/// `d` is a live doc; `index` is below `tcsv_columns` or `tcsv_named_count`; `word` below
/// the type's `words` for `TcsvPartWord`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_type_part(d: *mut TcsvDoc, of: TcsvOf, index: usize, part: TcsvPart, word: usize) -> TcsvStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    let (name, t, column) = type_of(d, of, index);
    let text = match part {
        TcsvPart::TcsvPartName => name.to_owned(),
        TcsvPart::TcsvPartShorthand => t.shorthand(),
        TcsvPart::TcsvPartSpelled => column.and_then(|c| c.spelled.clone()).unwrap_or_default(),
        TcsvPart::TcsvPartExpression => column.and_then(|c| c.computed.clone()).unwrap_or_default(),
        TcsvPart::TcsvPartCurrency => match &t.base {
            tcsv::Base::Money(c) => c.as_str().to_owned(),
            _ => String::new(),
        },
        TcsvPart::TcsvPartFormat => t.format.as_ref().map(|f| f.spelled.clone()).unwrap_or_default(),
        TcsvPart::TcsvPartMin => t.min.map(|b| b.value.normal().to_string()).unwrap_or_default(),
        TcsvPart::TcsvPartMax => t.max.map(|b| b.value.normal().to_string()).unwrap_or_default(),
        TcsvPart::TcsvPartWord => match &t.base {
            tcsv::Base::Choice(w) => w[word].clone(),
            tcsv::Base::Bool { yes, no } => {
                if word == 0 {
                    yes.clone()
                } else {
                    no.clone()
                }
            }
            other => panic!("a {} has no words", other.tag()),
        },
        TcsvPart::TcsvPartNamed => t.named.as_ref().map(|n| n.name.clone()).unwrap_or_default(),
    };
    d.out = text;
    TcsvStr::of(&d.out)
}

/// Problem `i` without its place: `"8x.50" is not a number`, `is missing`.
///
/// # Safety
/// `d` is a live doc; `i < tcsv_problem_count(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_problem_why(d: *mut TcsvDoc, i: usize) -> TcsvStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.why(&d.doc.problems()[i]);
    TcsvStr::of(&d.out)
}

/// One cell set to a value given in its canonical spelling (a decimal with a point, an
/// integer, `true`/`false`, `YYYY-MM-DD`, `HH:mm:ss`, an ISO instant, any text), written in
/// the column's form: its pair's words, its format. `null` blanks the cell. False, as for
/// `tcsv_set_cell`, when the text holds a control character.
///
/// # Safety
/// `d` is a live doc; `row < tcsv_rows(d)`, `column < tcsv_stored(d)`; `text` points at
/// `len` bytes of UTF-8, the canonical spelling of a value of the column's base; `failure`
/// is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_set_value(
    d: *mut TcsvDoc,
    row: usize,
    column: usize,
    null: bool,
    text: *const c_char,
    len: usize,
    failure: *mut TcsvFailure,
) -> bool {
    // SAFETY: the caller's promise.
    let (d, t) = unsafe { (doc(d), text_of(text, len)) };
    let b = t.as_bytes();
    let wrong = || -> ! { panic!("{t:?} is no value of column {column}") };
    use tcsv::{Base as B, Value as V};
    let v = if null {
        V::Null
    } else {
        match &d.doc.columns()[column].ty.base {
            B::Bool { .. } => match t {
                "true" => V::Bool(true),
                "false" => V::Bool(false),
                _ => wrong(),
            },
            B::Int => V::Int(tcsv::value::parse_int(b).unwrap_or_else(|| wrong())),
            B::Num | B::Pct | B::Money(_) => V::Decimal(tcsv::Decimal::parse(b).unwrap_or_else(|_| wrong())),
            B::Date => V::Date(tcsv::value::parse_date(b).unwrap_or_else(|| wrong())),
            B::Time => V::Time(tcsv::value::parse_time(b).unwrap_or_else(|| wrong())),
            B::DateTime => V::DateTime(tcsv::value::parse_datetime(b).unwrap_or_else(|| wrong())),
            B::Choice(_) => V::Choice(t),
            B::Text | B::Url | B::Email | B::Id => V::Text(t),
        }
    };
    let text = d.doc.columns()[column].form(v);
    match d.doc.set_cell(row, column, &text) {
        Ok(()) => true,
        Err(e) => {
            fail(failure, e);
            false
        }
    }
}

/// A row of a file rearranged: `kept`, a row it has, or when `kept` is negative a new row
/// of `count` cells.
#[repr(C)]
pub struct TcsvRow {
    pub kept: i64,
    pub cells: *const TcsvStr,
    pub count: usize,
}

/// The file with exactly `rows`, in their order, read again on up to `threads` threads (a
/// row moved, added, taken out): a new doc, this one untouched. Null when it does not
/// read, `failure` then set.
///
/// # Safety
/// `d` is a live doc with a header; `rows` points at `count` rows, each kept one below
/// `tcsv_rows(d)` and each new one's cells UTF-8; `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_rearranged(
    d: *mut TcsvDoc,
    rows: *const TcsvRow,
    count: usize,
    threads: usize,
    failure: *mut TcsvFailure,
) -> *mut TcsvDoc {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    let given = if count == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(rows, count) } };
    // SAFETY: each new row's cells are the caller's UTF-8.
    let cells: Vec<Vec<&str>> = given.iter().map(|r| if r.kept < 0 { unsafe { strs(r.cells, r.count) } } else { Vec::new() }).collect();
    let order: Vec<tcsv::Row> =
        given.iter().zip(&cells).map(|(r, c)| if r.kept < 0 { tcsv::Row::New(c) } else { tcsv::Row::Kept(r.kept as usize) }).collect();
    doc_or_fail(d.doc.rearranged(&order, threads.max(1)), failure)
}

/// The text before the header: a byte order mark and the prelude, as written.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_prelude_text(d: *mut TcsvDoc) -> TcsvStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.prelude_text().to_owned();
    TcsvStr::of(&d.out)
}

/// Whether the header's line break is `\r\n`.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_crlf(d: *mut TcsvDoc) -> bool {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.newline() == "\r\n"
}

/// Whether the file ends in a line break.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_trailing(d: *mut TcsvDoc) -> bool {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.trailing()
}

/// What the file's text is now: its length and a hash of its bytes.
#[repr(C)]
pub struct TcsvVersion {
    pub len: u64,
    pub hash: u64,
}

/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_version(d: *mut TcsvDoc) -> TcsvVersion {
    // SAFETY: the caller's promise.
    let v = unsafe { doc(d) }.doc.version();
    TcsvVersion { len: v.len, hash: v.hash }
}

/// What a span of a file's text is (`tcsv::tokens::Kind`, in its order).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TcsvTokenKind {
    TcsvTokenFence = 0,
    TcsvTokenComment = 1,
    TcsvTokenKeyword = 2,
    TcsvTokenName = 3,
    TcsvTokenType = 4,
    TcsvTokenMark = 5,
    TcsvTokenBound = 6,
    TcsvTokenFormat = 7,
    TcsvTokenChoice = 8,
    TcsvTokenPunct = 9,
    TcsvTokenNumber = 10,
    TcsvTokenReference = 11,
    TcsvTokenOperator = 12,
    TcsvTokenCellText = 13,
    TcsvTokenCellNumber = 14,
    TcsvTokenCellBool = 15,
    TcsvTokenCellDate = 16,
    TcsvTokenCellChoice = 17,
    TcsvTokenCellLink = 18,
    TcsvTokenCellId = 19,
    TcsvTokenError = 20,
}

/// A span of the text in bytes, what it is, and its column (`UINT32_MAX` for none).
#[repr(C)]
pub struct TcsvToken {
    pub start: u32,
    pub end: u32,
    pub kind: TcsvTokenKind,
    pub column: u32,
}

/// A text's spans, owned by the caller until `tcsv_tokens_free`.
#[repr(C)]
pub struct TcsvTokens {
    pub tokens: *mut TcsvToken,
    pub count: usize,
}

/// The spans of `len` bytes of text, in order: what a highlighter paints. Never refused: a
/// file that does not read has its spans too, what has no meaning marked an error.
///
/// # Safety
/// `bytes` points at `len` bytes of UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_tokens(bytes: *const c_char, len: usize) -> TcsvTokens {
    // SAFETY: the caller's promise.
    let text = unsafe { text_of(bytes, len) };
    use tcsv::tokens::Kind as K;
    let spans: Box<[TcsvToken]> = tcsv::tokens::tokens(text)
        .into_iter()
        .map(|t| TcsvToken {
            start: t.start,
            end: t.end,
            column: t.column,
            kind: match t.kind {
                K::Fence => TcsvTokenKind::TcsvTokenFence,
                K::Comment => TcsvTokenKind::TcsvTokenComment,
                K::Keyword => TcsvTokenKind::TcsvTokenKeyword,
                K::Name => TcsvTokenKind::TcsvTokenName,
                K::Type => TcsvTokenKind::TcsvTokenType,
                K::Mark => TcsvTokenKind::TcsvTokenMark,
                K::Bound => TcsvTokenKind::TcsvTokenBound,
                K::Format => TcsvTokenKind::TcsvTokenFormat,
                K::Choice => TcsvTokenKind::TcsvTokenChoice,
                K::Punct => TcsvTokenKind::TcsvTokenPunct,
                K::Number => TcsvTokenKind::TcsvTokenNumber,
                K::Reference => TcsvTokenKind::TcsvTokenReference,
                K::Operator => TcsvTokenKind::TcsvTokenOperator,
                K::CellText => TcsvTokenKind::TcsvTokenCellText,
                K::CellNumber => TcsvTokenKind::TcsvTokenCellNumber,
                K::CellBool => TcsvTokenKind::TcsvTokenCellBool,
                K::CellDate => TcsvTokenKind::TcsvTokenCellDate,
                K::CellChoice => TcsvTokenKind::TcsvTokenCellChoice,
                K::CellLink => TcsvTokenKind::TcsvTokenCellLink,
                K::CellId => TcsvTokenKind::TcsvTokenCellId,
                K::Error => TcsvTokenKind::TcsvTokenError,
            },
        })
        .collect();
    let count = spans.len();
    TcsvTokens { tokens: Box::into_raw(spans) as *mut TcsvToken, count }
}

/// Lets a text's spans go.
///
/// # Safety
/// `t` came from `tcsv_tokens`, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_tokens_free(t: TcsvTokens) {
    // SAFETY: the caller's promise; the spans are a boxed slice of `count`.
    drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(t.tokens, t.count)) });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn through_the_abi() {
        let src = b"item:text!,cost:EUR\nfuel,84.20\nmaps,8x\n";
        let mut f = TcsvFailure { at: TcsvAt::TcsvAtNone, number: 0, why: ptr::null_mut(), why_len: 0 };
        // SAFETY: valid pointers throughout; each struct released once.
        unsafe {
            let d = tcsv_read(src.as_ptr(), src.len(), 1, &mut f);
            assert!(!d.is_null());
            assert_eq!(tcsv_rows(d), 2);
            assert_eq!(tcsv_problem_count(d), 1);
            let p = tcsv_problem(d, 0);
            assert_eq!((p.row, p.column, p.kind), (1, 1, TcsvKind::TcsvKindType));
            let t = tcsv_column_type(d, 1);
            assert_eq!(std::slice::from_raw_parts(t.ptr as *const u8, t.len), b"EUR");
            assert!(tcsv_set_cell(d, 1, 1, c"12.00".as_ptr(), 5, ptr::null_mut()));
            assert_eq!(tcsv_problem_count(d), 0);
            assert!(!tcsv_set_cell(d, 1, 0, c"a\x07".as_ptr(), 2, &mut f));
            assert_eq!((f.at, f.number), (TcsvAt::TcsvAtRow, 1));
            tcsv_failure_free(&mut f);
            let mut s = ArrowSchema::empty();
            let mut a = ArrowArray::empty();
            tcsv_export(d, &mut s, &mut a);
            tcsv_free(d);
            // The arrays outlive the doc.
            let cost = &**a.children.add(1);
            assert_eq!(*(*cost.buffers.add(1) as *const i128).add(1), 1200);
            (s.release.unwrap())(&mut s);
            (a.release.unwrap())(&mut a);

            let bad = b"item:EURO\n";
            assert!(tcsv_read(bad.as_ptr(), bad.len(), 1, &mut f).is_null());
            assert_eq!((f.at, f.number), (TcsvAt::TcsvAtColumn, 1));
            tcsv_failure_free(&mut f);
        }
    }

    fn text(s: TcsvStr) -> String {
        // SAFETY: the doc that lent it is alive.
        String::from_utf8(unsafe { std::slice::from_raw_parts(s.ptr as *const u8, s.len) }.to_vec()).unwrap()
    }

    fn lent(s: &str) -> TcsvStr {
        TcsvStr::of(s)
    }

    #[test]
    fn a_plain_file_through_the_abi() {
        let src = b"a;a;c\n1;2\n3;4;5;6\n";
        let mut f = TcsvFailure { at: TcsvAt::TcsvAtNone, number: 0, why: ptr::null_mut(), why_len: 0 };
        // SAFETY: valid pointers throughout.
        unsafe {
            assert_eq!(tcsv_detect(src.as_ptr(), src.len()), b';');
            let d = tcsv_read_as(src.as_ptr(), src.len(), b';', 1, &mut f);
            assert!(!d.is_null());
            assert_eq!(tcsv_dialect(d), b';');
            assert_eq!(tcsv_naming_count(d), 1);
            let n = tcsv_naming(d, 0);
            assert_eq!((n.column, n.blank, n.first), (1, false, 0));
            assert_eq!(text(tcsv_naming_why(d, 0)), "\"a\" names column 1 too");
            assert_eq!(tcsv_problem_count(d), 2);
            assert_eq!(tcsv_problem(d, 1).kind, TcsvKind::TcsvKindRagged);
            assert_eq!((tcsv_width(d, 0), tcsv_width(d, 1)), (2, 4));
            // c moved first, b renamed, a new column last.
            let cells = [lent("x"), lent("y")];
            let order = [
                TcsvColumnAt { kept: 2, renamed: false, name: lent(""), cells: ptr::null(), count: 0 },
                TcsvColumnAt { kept: 1, renamed: true, name: lent("b"), cells: ptr::null(), count: 0 },
                TcsvColumnAt { kept: -1, renamed: false, name: lent("new"), cells: cells.as_ptr(), count: 2 },
            ];
            let r = tcsv_reshaped(d, order.as_ptr(), order.len(), 1, &mut f);
            assert!(!r.is_null());
            assert_eq!(text(tcsv_write(r)), "c;b;new\n;2;x\n5;4;y;6\n");
            let head = [lent("c:int"), lent("b:int"), lent("new")];
            assert_eq!(text(tcsv_written_as(r, 0, head.as_ptr(), 3)), "c:int,b:int,new\n,2,x\n5,4,y\n");
            tcsv_free(r);
            tcsv_free(d);

            let src = b"---\ntotal = cost * qty  // never stored\n---\ncost:EUR,qty:int\n2.50,4\n";
            let d = tcsv_read(src.as_ptr(), src.len(), 1, &mut f);
            let set = tcsv_recomputed(d, TcsvComputing::TcsvComputingSet, 0, lent("sum"), lent("cost * 2"), 1, &mut f);
            assert_eq!(text(tcsv_write(set)), "---\nsum = cost * 2  // never stored\n---\ncost:EUR,qty:int\n2.50,4\n");
            let bad = tcsv_recomputed(d, TcsvComputing::TcsvComputingNew, 0, lent("x"), lent("cost *"), 1, &mut f);
            assert!(bad.is_null());
            assert_eq!((f.at, f.number), (TcsvAt::TcsvAtPreludeLine, 2));
            tcsv_failure_free(&mut f);
            tcsv_free(set);
            tcsv_free(d);
        }
    }
}
