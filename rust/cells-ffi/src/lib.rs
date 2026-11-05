//! The Cells engine's C ABI. A caller hands the file's bytes in and gets a `CellsDoc`; the
//! columns leave through the Arrow C Data Interface (`cells_export`), their buffers shared,
//! never copied; everything else is a handful of calls. `cells.h` is generated from this
//! file by cbindgen.
//!
//! Text the core returns is a `CellsStr`: UTF-8, not NUL-terminated, borrowed from the doc
//! and valid until the next call on it.

use std::ffi::c_char;
use std::ptr;

use cells::arrow::{ArrowArray, ArrowSchema};
use cells::{At, ColumnAt, Dialect, Doc, Separator};

/// A file read, with the text its last call returned.
pub struct CellsDoc {
    doc: Doc,
    out: String,
}

/// UTF-8 bytes, borrowed: valid until the next call on the doc that returned them.
#[repr(C)]
pub struct CellsStr {
    pub ptr: *const c_char,
    pub len: usize,
}

impl CellsStr {
    fn of(s: &str) -> CellsStr {
        CellsStr { ptr: s.as_ptr() as *const c_char, len: s.len() }
    }
}

/// Where a file's grammar breaks, in the suite's words.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellsAt {
    /// Nothing broke.
    CellsAtNone = 0,
    /// The header line.
    CellsAtHeader = 1,
    /// A header cell, `number` 1-based.
    CellsAtColumn = 2,
    /// A prelude line, `number` 1-based inside the fences.
    CellsAtPreludeLine = 3,
    /// The prelude as a whole.
    CellsAtPrelude = 4,
    /// A data row, `number` from 0.
    CellsAtRow = 5,
    /// The file as a whole: not UTF-8, or larger than 4 GiB.
    CellsAtFile = 6,
}

/// Why a call failed: the place, and the reason in words (owned by the caller once set;
/// `cells_failure_free` lets it go).
#[repr(C)]
pub struct CellsFailure {
    pub at: CellsAt,
    pub number: u32,
    pub why: *mut c_char,
    pub why_len: usize,
}

fn fail(out: *mut CellsFailure, e: cells::Error) {
    if out.is_null() {
        return;
    }
    let (at, number) = match e.at {
        At::Header => (CellsAt::CellsAtHeader, 0),
        At::Column(n) => (CellsAt::CellsAtColumn, n),
        At::PreludeLine(n) => (CellsAt::CellsAtPreludeLine, n),
        At::Prelude => (CellsAt::CellsAtPrelude, 0),
        At::Row(n) => (CellsAt::CellsAtRow, n),
        At::File => (CellsAt::CellsAtFile, 0),
    };
    let why = e.why.into_bytes().into_boxed_slice();
    let why_len = why.len();
    // SAFETY: the caller hands a writable CellsFailure.
    unsafe {
        *out = CellsFailure { at, number, why: Box::into_raw(why) as *mut c_char, why_len };
    }
}

/// Lets a failure's words go.
///
/// # Safety
/// `f` points at a failure a call set, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_failure_free(f: *mut CellsFailure) {
    // SAFETY: the caller's promise; `why` came from a boxed slice of `why_len` bytes.
    unsafe {
        let f = &mut *f;
        if !f.why.is_null() {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(f.why as *mut u8, f.why_len)));
        }
        f.why = ptr::null_mut();
        f.why_len = 0;
        f.at = CellsAt::CellsAtNone;
    }
}

/// Reads `len` bytes (copied: the caller keeps its buffer) on up to `threads` threads (1
/// starts none). Null when the file does not read, `failure` then set.
///
/// # Safety
/// `bytes` points at `len` readable bytes; `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_read(bytes: *const u8, len: usize, threads: usize, failure: *mut CellsFailure) -> *mut CellsDoc {
    // SAFETY: the caller's promise.
    unsafe { cells_read_as(bytes, len, 0, threads, failure) }
}

/// The dialect a call names: 0 for Cells, else a plain file's separator (`,` `;` `\t` `|`).
fn dialect(sep: u8) -> Dialect {
    if sep == 0 {
        return Dialect::Cells;
    }
    Dialect::Plain(Separator::of(sep).unwrap_or_else(|| panic!("{sep} is no separator")))
}

fn doc_or_fail(read: Result<Doc, cells::Error>, failure: *mut CellsFailure) -> *mut CellsDoc {
    match read {
        Ok(doc) => Box::into_raw(Box::new(CellsDoc { doc, out: String::new() })),
        Err(e) => {
            fail(failure, e);
            ptr::null_mut()
        }
    }
}

/// Reads the file as `cells_read` does, its imports followed through the files the caller
/// holds: `count` paths, each with its text, by the path an import names read from `at`,
/// the file's own path (`books/home/money.cell` importing `../accounts.cell` finds
/// `books/accounts.cell`). An import of a file not held reads as unreachable.
///
/// # Safety
/// As `cells_read`; `at` is UTF-8; `paths` and `texts` point at `count` strings of UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_read_with(
    bytes: *const u8,
    len: usize,
    at: CellsStr,
    paths: *const CellsStr,
    texts: *const CellsStr,
    count: usize,
    threads: usize,
    failure: *mut CellsFailure,
) -> *mut CellsDoc {
    // SAFETY: the caller's promise.
    let (src, at) = unsafe { (owned(bytes, len), text_of(at.ptr, at.len)) };
    let held: std::collections::BTreeMap<String, String> = (0..count)
        .map(|k| {
            // SAFETY: the caller's promise: `count` strings at each.
            unsafe {
                let (p, t) = (&*paths.add(k), &*texts.add(k));
                (text_of(p.ptr, p.len).to_owned(), text_of(t.ptr, t.len).to_owned())
            }
        })
        .collect();
    doc_or_fail(cells::read_with(src, &mut cells::files::Files::new(&held, at), threads.max(1)), failure)
}

/// Reads the file at `path`, its bytes given, its imports followed from the disk: a path
/// read from the importing file's folder as it is now. A URL, or a file at a revision,
/// reads as unreachable: the command line fetches and reads git, an app does neither.
///
/// # Safety
/// As `cells_read`; `path` is UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_read_at(
    bytes: *const u8,
    len: usize,
    path: CellsStr,
    threads: usize,
    failure: *mut CellsFailure,
) -> *mut CellsDoc {
    // SAFETY: the caller's promise.
    let (src, path) = unsafe { (owned(bytes, len), text_of(path.ptr, path.len)) };
    doc_or_fail(cells::read_with(src, &mut Disk::new(std::path::Path::new(path)), threads.max(1)), failure)
}

/// The imports of the file at `at`, read from its folder.
struct Disk {
    at: std::path::PathBuf,
    /// The files importing this one, so a cycle is found.
    stack: Vec<std::path::PathBuf>,
}

impl Disk {
    fn new(at: &std::path::Path) -> Disk {
        Disk { at: std::fs::canonicalize(at).unwrap_or_else(|_| at.to_path_buf()), stack: Vec::new() }
    }
}

impl cells::Imports for Disk {
    fn import(&mut self, i: &cells::Import) -> Result<Option<Doc>, String> {
        if cells::files::is_url(&i.from) || i.with.iter().any(|(k, _)| k == "rev") {
            return Ok(None);
        }
        let folder = self.at.parent().unwrap_or(std::path::Path::new("."));
        let path = folder.join(&i.from);
        let Ok(path) = std::fs::canonicalize(&path) else { return Ok(None) };
        if path == self.at || self.stack.contains(&path) {
            return Err(format!("{} imports itself", path.display()));
        }
        let Ok(bytes) = std::fs::read(&path) else { return Ok(None) };
        let mut stack = self.stack.clone();
        stack.push(self.at.clone());
        let mut inner = Disk { at: path, stack };
        cells::read_with(bytes, &mut inner, 1).map(Some).map_err(|e| format!("{e}"))
    }
}

/// `len` bytes copied out of the caller's buffer.
///
/// # Safety
/// `bytes` points at `len` readable bytes, or `len` is 0.
unsafe fn owned(bytes: *const u8, len: usize) -> Vec<u8> {
    // SAFETY: the caller's promise.
    if len == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec() }
}

/// `cells_read` in a dialect: `sep` 0 for Cells, else a plain file's separator byte (`,`
/// `;` `\t` `|`), read leniently: a plain file is refused only for bytes that are not
/// UTF-8, or past 4 GiB.
///
/// # Safety
/// As `cells_read`; `sep` is 0 or a separator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_read_as(bytes: *const u8, len: usize, sep: u8, threads: usize, failure: *mut CellsFailure) -> *mut CellsDoc {
    // SAFETY: the caller's promise.
    let src = if len == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec() };
    doc_or_fail(cells::read_as(src, dialect(sep), threads.max(1)), failure)
}

/// The separator byte the first lines of `len` bytes agree on (`cells::detect`).
///
/// # Safety
/// `bytes` points at `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_detect(bytes: *const u8, len: usize) -> u8 {
    // SAFETY: the caller's promise.
    let src = if len == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(bytes, len) } };
    cells::detect(src).byte()
}

/// How the doc was read: 0 for Cells, else a plain file's separator byte.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_dialect(d: *mut CellsDoc) -> u8 {
    // SAFETY: the caller's promise.
    match unsafe { doc(d) }.doc.dialect() {
        Dialect::Cells => 0,
        Dialect::Plain(s) => s.byte(),
    }
}

/// Whether a plain file quotes every field, so an edited cell and a new row are quoted too.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_quotes_all(d: *mut CellsDoc) -> bool {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.quotes_all()
}

/// A plain file's header cell that names its column badly: blank, or the name of the
/// column `first` too.
#[repr(C)]
pub struct CellsNaming {
    pub column: u32,
    pub blank: bool,
    pub first: u32,
}

/// How many header cells name their columns badly.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_naming_count(d: *mut CellsDoc) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.naming().len()
}

/// Header problem `i`.
///
/// # Safety
/// `d` is a live doc; `i < cells_naming_count(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_naming(d: *mut CellsDoc, i: usize) -> CellsNaming {
    // SAFETY: the caller's promise.
    match unsafe { doc(d) }.doc.naming()[i] {
        cells::Naming::Blank { column } => CellsNaming { column, blank: true, first: 0 },
        cells::Naming::Twice { column, first } => CellsNaming { column, blank: false, first },
    }
}

/// Header problem `i` in words without its place: `has no name`, `"a" names column 1 too`.
///
/// # Safety
/// `d` is a live doc; `i < cells_naming_count(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_naming_why(d: *mut CellsDoc, i: usize) -> CellsStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.naming_why(&d.doc.naming()[i]);
    CellsStr::of(&d.out)
}

/// How many fields a row's record holds now (a plain file's ragged row, a short row).
///
/// # Safety
/// `d` is a live doc; `row < cells_rows(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_width(d: *mut CellsDoc, row: usize) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.width(row)
}

/// A column of a file reshaped: `kept`, a stored column it has (renamed to `name` when
/// `renamed`), or when `kept` is negative a new one, its header cell `name` and `count`
/// cells, one a row, or none for blanks.
#[repr(C)]
pub struct CellsColumnAt {
    pub kept: i64,
    pub renamed: bool,
    pub name: CellsStr,
    pub cells: *const CellsStr,
    pub count: usize,
}

/// The file with exactly `columns`, in their order, read again on up to `threads` threads
/// (a column inserted, removed, moved, renamed): a new doc, this one untouched. Null when
/// it does not read, `failure` then set.
///
/// # Safety
/// `d` is a live doc; `columns` points at `count` columns, at least one, each kept one
/// below `cells_stored(d)` and every string UTF-8; `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_reshaped(
    d: *mut CellsDoc,
    columns: *const CellsColumnAt,
    count: usize,
    threads: usize,
    failure: *mut CellsFailure,
) -> *mut CellsDoc {
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

/// The file written in a dialect (`sep` as for `cells_read_as`): a plain file's header the
/// columns' names and every computed cell its value; a `.cell` file's header the `count`
/// cells of `header` when `count` is not 0 (one a column), its prelude kept.
///
/// # Safety
/// `d` is a live doc; `sep` is 0 or a separator; `header` points at `count` strings of
/// UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_written_as(d: *mut CellsDoc, sep: u8, header: *const CellsStr, count: usize) -> CellsStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    // SAFETY: the caller's promise.
    let cells = unsafe { strs(header, count) };
    d.out = d.doc.written_as(dialect(sep), if count == 0 { None } else { Some(&cells) });
    CellsStr::of(&d.out)
}

/// Lets a doc go. The arrays exported from it live on until released.
///
/// # Safety
/// `d` came from `cells_read`, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_free(d: *mut CellsDoc) {
    // SAFETY: the caller's promise.
    drop(unsafe { Box::from_raw(d) });
}

/// # Safety
/// `d` is a live doc.
unsafe fn doc<'a>(d: *mut CellsDoc) -> &'a mut CellsDoc {
    assert!(!d.is_null(), "a null doc");
    // SAFETY: the caller's promise.
    unsafe { &mut *d }
}

/// The rows.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_rows(d: *mut CellsDoc) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.rows()
}

/// Every column, in the header's order.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_columns(d: *mut CellsDoc) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.columns().len()
}

/// Where a cell's value comes from.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellsOrigin {
    /// Its text: written, or in a column with a formula, the row's override.
    CellsOriginWritten = 0,
    /// Its column's formula.
    CellsOriginFormula = 1,
    /// Its own formula, `` `=…` ``.
    CellsOriginCell = 2,
}

/// Where the value of `column` at `row` comes from.
///
/// # Safety
/// `d` is a live doc; `row` below `cells_rows`, `column` below `cells_columns`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_origin(d: *mut CellsDoc, row: usize, column: usize) -> CellsOrigin {
    // SAFETY: the caller's promise.
    match unsafe { doc(d) }.doc.columns()[column].origin(row) {
        cells::Origin::Written => CellsOrigin::CellsOriginWritten,
        cells::Origin::Formula => CellsOrigin::CellsOriginFormula,
        cells::Origin::Cell => CellsOrigin::CellsOriginCell,
    }
}

/// A column's name.
///
/// # Safety
/// `d` is a live doc; `column < cells_columns(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_column_name(d: *mut CellsDoc, column: usize) -> CellsStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.columns()[column].name.clone();
    CellsStr::of(&d.out)
}

/// A column's type in canonical shorthand: `EUR>=0`, `text!`, `yes/no?`.
///
/// # Safety
/// `d` is a live doc; `column < cells_columns(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_column_type(d: *mut CellsDoc, column: usize) -> CellsStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.columns()[column].ty.shorthand();
    CellsStr::of(&d.out)
}

/// What a problem is: the suite's kinds.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellsKind {
    CellsKindType = 0,
    CellsKindMissing = 1,
    CellsKindKey = 2,
    /// A plain file's row of another width than its header.
    CellsKindRagged = 3,
    /// A cell's own formula that does not compute.
    CellsKindFormula = 4,
    /// A reference to no row, or to rows of two tables.
    CellsKindReference = 5,
}

/// A cell that breaks its type, by place.
#[repr(C)]
pub struct CellsProblem {
    pub row: u32,
    pub column: u32,
    pub kind: CellsKind,
}

/// How many problems, by row then column.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_problem_count(d: *mut CellsDoc) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.problems().len()
}

/// Problem `i`.
///
/// # Safety
/// `d` is a live doc; `i < cells_problem_count(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_problem(d: *mut CellsDoc, i: usize) -> CellsProblem {
    // SAFETY: the caller's promise.
    let p = unsafe { doc(d) }.doc.problems()[i];
    let kind = match p.kind() {
        cells::Kind::Type => CellsKind::CellsKindType,
        cells::Kind::Missing => CellsKind::CellsKindMissing,
        cells::Kind::Key => CellsKind::CellsKindKey,
        cells::Kind::Ragged => CellsKind::CellsKindRagged,
        cells::Kind::Formula => CellsKind::CellsKindFormula,
        cells::Kind::Reference => CellsKind::CellsKindReference,
    };
    CellsProblem { row: p.row, column: p.column, kind }
}

/// Problem `i` in words: `row 3, cost: "8x.50" is not a number`.
///
/// # Safety
/// `d` is a live doc; `i < cells_problem_count(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_describe(d: *mut CellsDoc, i: usize) -> CellsStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.describe(&d.doc.problems()[i]);
    CellsStr::of(&d.out)
}

/// A stored cell's text as written, unquoted.
///
/// # Safety
/// `d` is a live doc; `row < cells_rows(d)`, `column < cells_stored(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_text(d: *mut CellsDoc, row: usize, column: usize) -> CellsStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.text(row, column).into_owned();
    CellsStr::of(&d.out)
}

/// The file's text: what was read, but for what was changed.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_write(d: *mut CellsDoc) -> CellsStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.write();
    CellsStr::of(&d.out)
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
/// `d` is a live doc; `row < cells_rows(d)`, `column < cells_stored(d)`; `text` points at
/// `len` bytes of UTF-8; `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_set_cell(
    d: *mut CellsDoc,
    row: usize,
    column: usize,
    text: *const c_char,
    len: usize,
    failure: *mut CellsFailure,
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
/// `d` is a live doc; `column < cells_stored(d)`; `spelled` points at `len` bytes of UTF-8;
/// `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_set_type(
    d: *mut CellsDoc,
    column: usize,
    spelled: *const c_char,
    len: usize,
    failure: *mut CellsFailure,
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
/// `release`), before or after `cells_free`. Each field carries its Cells type under the
/// metadata key `cells.type`.
///
/// # Safety
/// `d` is a live doc; `schema` and `array` are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_export(d: *mut CellsDoc, schema: *mut ArrowSchema, array: *mut ArrowArray) {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    let (s, a) = cells::arrow::export(&d.doc);
    // SAFETY: the caller's promise; the structs are moved, as the interface allows.
    unsafe {
        schema.write(s);
        array.write(a);
    }
}

/// One column alone, as `cells_export`.
///
/// # Safety
/// `d` is a live doc; `column < cells_columns(d)`; `schema` and `array` are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_export_column(d: *mut CellsDoc, column: usize, schema: *mut ArrowSchema, array: *mut ArrowArray) {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    let (s, a) = cells::arrow::export_column(&d.doc, column);
    // SAFETY: as `cells_export`.
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
pub unsafe extern "C" fn cells_clone(d: *mut CellsDoc) -> *mut CellsDoc {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    Box::into_raw(Box::new(CellsDoc { doc: d.doc.clone(), out: String::new() }))
}

/// A new file in a dialect (`sep` as for `cells_read_as`) written from cells: its header's,
/// then each row's (`rows`, every one new), each quoted only where the dialect needs it,
/// lines broken by `\r\n` or `\n`, ending in one when `trailing`. Null when it does not
/// read, `failure` then set.
///
/// # Safety
/// `header` points at `columns` strings of UTF-8, `rows` at `count` new rows; `sep` is 0
/// or a separator; `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_new(
    header: *const CellsStr,
    columns: usize,
    rows: *const CellsRow,
    count: usize,
    sep: u8,
    crlf: bool,
    trailing: bool,
    failure: *mut CellsFailure,
) -> *mut CellsDoc {
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
    doc_or_fail(cells::read_as(text, dialect, 1), failure)
}

/// # Safety
/// `ptr` points at `count` strings of UTF-8.
unsafe fn strs<'a>(ptr: *const CellsStr, count: usize) -> Vec<&'a str> {
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
pub enum CellsBase {
    CellsBaseText = 0,
    CellsBaseInt = 1,
    CellsBaseNum = 2,
    CellsBasePct = 3,
    CellsBaseBool = 4,
    CellsBaseDate = 5,
    CellsBaseTime = 6,
    CellsBaseDateTime = 7,
    CellsBaseUrl = 8,
    CellsBaseEmail = 9,
    CellsBaseId = 10,
    CellsBaseMoney = 11,
    CellsBaseChoice = 12,
    /// A key of another table; its tables through `cells_type_part` Word.
    CellsBaseRef = 13,
}

/// Which type a call reads: a column's, or the prelude's named type at an index.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellsOf {
    CellsOfColumn = 0,
    CellsOfNamed = 1,
}

/// A type's shape; its words through `cells_type_part`.
#[repr(C)]
pub struct CellsType {
    pub base: CellsBase,
    pub key: bool,
    pub optional: bool,
    /// Read from the values: the header said nothing.
    pub inferred: bool,
    /// A column with a formula (`cells_type_part` Formula is it).
    pub formula: bool,
    /// The type uses a named one (`cells_type_part` Named is its name).
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
pub enum CellsPart {
    /// The column's name, or the named type's.
    CellsPartName = 0,
    /// The canonical shorthand: what a writer writes.
    CellsPartShorthand = 1,
    /// As the header cell spells it after its `:` (empty for a plain column).
    CellsPartSpelled = 2,
    /// The column's formula, as its header cell writes it after the type's `=`.
    CellsPartFormula = 3,
    CellsPartCurrency = 4,
    CellsPartFormat = 5,
    /// The bounds, as decimals.
    CellsPartMin = 6,
    CellsPartMax = 7,
    /// Word `index`: a choice's, or a boolean's (0 true, 1 false).
    CellsPartWord = 8,
    /// The named type the type uses.
    CellsPartNamed = 9,
}

/// How many named types the prelude declares.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_named_count(d: *mut CellsDoc) -> usize {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.named().len()
}

fn type_of(d: &CellsDoc, of: CellsOf, index: usize) -> (&str, &cells::Type, Option<&cells::Column>) {
    match of {
        CellsOf::CellsOfColumn => {
            let c = &d.doc.columns()[index];
            (&c.name, &c.ty, Some(c))
        }
        CellsOf::CellsOfNamed => {
            let n = &d.doc.named()[index];
            (&n.name, &n.ty, None)
        }
    }
}

/// A type's shape.
///
/// # Safety
/// `d` is a live doc; `index` is below `cells_columns` or `cells_named_count`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_type(d: *mut CellsDoc, of: CellsOf, index: usize) -> CellsType {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    let (_, t, column) = type_of(d, of, index);
    use cells::Base as B;
    let (base, words) = match &t.base {
        B::Text => (CellsBase::CellsBaseText, 0),
        B::Int => (CellsBase::CellsBaseInt, 0),
        B::Num => (CellsBase::CellsBaseNum, 0),
        B::Pct => (CellsBase::CellsBasePct, 0),
        B::Bool { .. } => (CellsBase::CellsBaseBool, 2),
        B::Date => (CellsBase::CellsBaseDate, 0),
        B::Time => (CellsBase::CellsBaseTime, 0),
        B::DateTime => (CellsBase::CellsBaseDateTime, 0),
        B::Url => (CellsBase::CellsBaseUrl, 0),
        B::Email => (CellsBase::CellsBaseEmail, 0),
        B::Id => (CellsBase::CellsBaseId, 0),
        B::Money(_) => (CellsBase::CellsBaseMoney, 0),
        B::Choice(w) => (CellsBase::CellsBaseChoice, w.len()),
        B::Ref(targets) => (CellsBase::CellsBaseRef, targets.len()),
    };
    CellsType {
        base,
        key: t.key,
        optional: t.optional,
        inferred: t.inferred,
        formula: column.is_some_and(|c| c.formula.is_some()),
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
/// `d` is a live doc; `index` is below `cells_columns` or `cells_named_count`; `word` below
/// the type's `words` for `CellsPartWord`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_type_part(d: *mut CellsDoc, of: CellsOf, index: usize, part: CellsPart, word: usize) -> CellsStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    let (name, t, column) = type_of(d, of, index);
    let text = match part {
        CellsPart::CellsPartName => name.to_owned(),
        CellsPart::CellsPartShorthand => t.shorthand(),
        CellsPart::CellsPartSpelled => column.and_then(|c| c.spelled.clone()).unwrap_or_default(),
        CellsPart::CellsPartFormula => column.and_then(|c| c.formula.clone()).unwrap_or_default(),
        CellsPart::CellsPartCurrency => match &t.base {
            cells::Base::Money(c) => c.as_str().to_owned(),
            _ => String::new(),
        },
        CellsPart::CellsPartFormat => t.format.as_ref().map(|f| f.spelled.clone()).unwrap_or_default(),
        CellsPart::CellsPartMin => t.min.map(|b| b.value.normal().to_string()).unwrap_or_default(),
        CellsPart::CellsPartMax => t.max.map(|b| b.value.normal().to_string()).unwrap_or_default(),
        CellsPart::CellsPartWord => match &t.base {
            cells::Base::Choice(w) => w[word].clone(),
            cells::Base::Ref(targets) => format!("{}.{}", targets[word].table, targets[word].column),
            cells::Base::Bool { yes, no } => {
                if word == 0 {
                    yes.clone()
                } else {
                    no.clone()
                }
            }
            other => panic!("a {} has no words", other.tag()),
        },
        CellsPart::CellsPartNamed => t.named.as_ref().map(|n| n.name.clone()).unwrap_or_default(),
    };
    d.out = text;
    CellsStr::of(&d.out)
}

/// Problem `i` without its place: `"8x.50" is not a number`, `is missing`.
///
/// # Safety
/// `d` is a live doc; `i < cells_problem_count(d)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_problem_why(d: *mut CellsDoc, i: usize) -> CellsStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.why(&d.doc.problems()[i]);
    CellsStr::of(&d.out)
}

/// One cell set to a value given in its canonical spelling (a decimal with a point, an
/// integer, `true`/`false`, `YYYY-MM-DD`, `HH:mm:ss`, an ISO instant, any text), written in
/// the column's form: its pair's words, its format. `null` blanks the cell. False, as for
/// `cells_set_cell`, when the text holds a control character.
///
/// # Safety
/// `d` is a live doc; `row < cells_rows(d)`, `column < cells_stored(d)`; `text` points at
/// `len` bytes of UTF-8, the canonical spelling of a value of the column's base; `failure`
/// is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_set_value(
    d: *mut CellsDoc,
    row: usize,
    column: usize,
    null: bool,
    text: *const c_char,
    len: usize,
    failure: *mut CellsFailure,
) -> bool {
    // SAFETY: the caller's promise.
    let (d, t) = unsafe { (doc(d), text_of(text, len)) };
    let b = t.as_bytes();
    let wrong = || -> ! { panic!("{t:?} is no value of column {column}") };
    use cells::{Base as B, Value as V};
    let v = if null {
        V::Null
    } else {
        match &d.doc.columns()[column].ty.base {
            B::Bool { .. } => match t {
                "true" => V::Bool(true),
                "false" => V::Bool(false),
                _ => wrong(),
            },
            B::Int => V::Int(cells::value::parse_int(b).unwrap_or_else(|| wrong())),
            B::Num | B::Pct | B::Money(_) => V::Decimal(cells::Decimal::parse(b).unwrap_or_else(|_| wrong())),
            B::Date => V::Date(cells::value::parse_date(b).unwrap_or_else(|| wrong())),
            B::Time => V::Time(cells::value::parse_time(b).unwrap_or_else(|| wrong())),
            B::DateTime => V::DateTime(cells::value::parse_datetime(b).unwrap_or_else(|| wrong())),
            B::Choice(_) => V::Choice(t),
            B::Text | B::Url | B::Email | B::Id | B::Ref(_) => V::Text(t),
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
pub struct CellsRow {
    pub kept: i64,
    pub cells: *const CellsStr,
    pub count: usize,
}

/// The file with exactly `rows`, in their order, read again on up to `threads` threads (a
/// row moved, added, taken out): a new doc, this one untouched. Null when it does not
/// read, `failure` then set.
///
/// # Safety
/// `d` is a live doc with a header; `rows` points at `count` rows, each kept one below
/// `cells_rows(d)` and each new one's cells UTF-8; `failure` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_rearranged(
    d: *mut CellsDoc,
    rows: *const CellsRow,
    count: usize,
    threads: usize,
    failure: *mut CellsFailure,
) -> *mut CellsDoc {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    let given = if count == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(rows, count) } };
    // SAFETY: each new row's cells are the caller's UTF-8.
    let cells: Vec<Vec<&str>> = given.iter().map(|r| if r.kept < 0 { unsafe { strs(r.cells, r.count) } } else { Vec::new() }).collect();
    let order: Vec<cells::Row> =
        given.iter().zip(&cells).map(|(r, c)| if r.kept < 0 { cells::Row::New(c) } else { cells::Row::Kept(r.kept as usize) }).collect();
    doc_or_fail(d.doc.rearranged(&order, threads.max(1)), failure)
}

/// The text before the header: a byte order mark and the prelude, as written.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_prelude_text(d: *mut CellsDoc) -> CellsStr {
    // SAFETY: the caller's promise.
    let d = unsafe { doc(d) };
    d.out = d.doc.prelude_text().to_owned();
    CellsStr::of(&d.out)
}

/// Whether the header's line break is `\r\n`.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_crlf(d: *mut CellsDoc) -> bool {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.newline() == "\r\n"
}

/// Whether the file ends in a line break.
///
/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_trailing(d: *mut CellsDoc) -> bool {
    // SAFETY: the caller's promise.
    unsafe { doc(d) }.doc.trailing()
}

/// What the file's text is now: its length and a hash of its bytes.
#[repr(C)]
pub struct CellsVersion {
    pub len: u64,
    pub hash: u64,
}

/// # Safety
/// `d` is a live doc.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_version(d: *mut CellsDoc) -> CellsVersion {
    // SAFETY: the caller's promise.
    let v = unsafe { doc(d) }.doc.version();
    CellsVersion { len: v.len, hash: v.hash }
}

/// What a span of a file's text is (`cells::tokens::Kind`, in its order).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellsTokenKind {
    CellsTokenFence = 0,
    CellsTokenComment = 1,
    CellsTokenKeyword = 2,
    CellsTokenName = 3,
    CellsTokenType = 4,
    CellsTokenMark = 5,
    CellsTokenBound = 6,
    CellsTokenFormat = 7,
    CellsTokenChoice = 8,
    CellsTokenPunct = 9,
    CellsTokenNumber = 10,
    CellsTokenReference = 11,
    CellsTokenOperator = 12,
    CellsTokenCellText = 13,
    CellsTokenCellNumber = 14,
    CellsTokenCellBool = 15,
    CellsTokenCellDate = 16,
    CellsTokenCellChoice = 17,
    CellsTokenCellLink = 18,
    CellsTokenCellId = 19,
    CellsTokenError = 20,
    CellsTokenString = 21,
    CellsTokenCode = 22,
}

/// A span of the text in bytes, what it is, and its column (`UINT32_MAX` for none).
#[repr(C)]
pub struct CellsToken {
    pub start: u32,
    pub end: u32,
    pub kind: CellsTokenKind,
    pub column: u32,
}

/// A text's spans, owned by the caller until `cells_tokens_free`.
#[repr(C)]
pub struct CellsTokens {
    pub tokens: *mut CellsToken,
    pub count: usize,
}

/// The spans of `len` bytes of text, in order: what a highlighter paints. Never refused: a
/// file that does not read has its spans too, what has no meaning marked an error.
///
/// # Safety
/// `bytes` points at `len` bytes of UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_tokens(bytes: *const c_char, len: usize) -> CellsTokens {
    // SAFETY: the caller's promise.
    let text = unsafe { text_of(bytes, len) };
    use cells::tokens::Kind as K;
    let spans: Box<[CellsToken]> = cells::tokens::tokens(text)
        .into_iter()
        .map(|t| CellsToken {
            start: t.start,
            end: t.end,
            column: t.column,
            kind: match t.kind {
                K::Fence => CellsTokenKind::CellsTokenFence,
                K::Comment => CellsTokenKind::CellsTokenComment,
                K::Keyword => CellsTokenKind::CellsTokenKeyword,
                K::Name => CellsTokenKind::CellsTokenName,
                K::Type => CellsTokenKind::CellsTokenType,
                K::Mark => CellsTokenKind::CellsTokenMark,
                K::Bound => CellsTokenKind::CellsTokenBound,
                K::Format => CellsTokenKind::CellsTokenFormat,
                K::Choice => CellsTokenKind::CellsTokenChoice,
                K::Punct => CellsTokenKind::CellsTokenPunct,
                K::Number => CellsTokenKind::CellsTokenNumber,
                K::Reference => CellsTokenKind::CellsTokenReference,
                K::Operator => CellsTokenKind::CellsTokenOperator,
                K::CellText => CellsTokenKind::CellsTokenCellText,
                K::CellNumber => CellsTokenKind::CellsTokenCellNumber,
                K::CellBool => CellsTokenKind::CellsTokenCellBool,
                K::CellDate => CellsTokenKind::CellsTokenCellDate,
                K::CellChoice => CellsTokenKind::CellsTokenCellChoice,
                K::CellLink => CellsTokenKind::CellsTokenCellLink,
                K::CellId => CellsTokenKind::CellsTokenCellId,
                K::Error => CellsTokenKind::CellsTokenError,
                K::String => CellsTokenKind::CellsTokenString,
                K::Code => CellsTokenKind::CellsTokenCode,
            },
        })
        .collect();
    let count = spans.len();
    CellsTokens { tokens: Box::into_raw(spans) as *mut CellsToken, count }
}

/// Lets a text's spans go.
///
/// # Safety
/// `t` came from `cells_tokens`, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_tokens_free(t: CellsTokens) {
    // SAFETY: the caller's promise; the spans are a boxed slice of `count`.
    drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(t.tokens, t.count)) });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn through_the_abi() {
        let src = b"item:text!,cost:EUR\nfuel,84.20\nmaps,8x\n";
        let mut f = CellsFailure { at: CellsAt::CellsAtNone, number: 0, why: ptr::null_mut(), why_len: 0 };
        // SAFETY: valid pointers throughout; each struct released once.
        unsafe {
            let d = cells_read(src.as_ptr(), src.len(), 1, &mut f);
            assert!(!d.is_null());
            assert_eq!(cells_rows(d), 2);
            assert_eq!(cells_problem_count(d), 1);
            let p = cells_problem(d, 0);
            assert_eq!((p.row, p.column, p.kind), (1, 1, CellsKind::CellsKindType));
            let t = cells_column_type(d, 1);
            assert_eq!(std::slice::from_raw_parts(t.ptr as *const u8, t.len), b"EUR");
            assert!(cells_set_cell(d, 1, 1, c"12.00".as_ptr(), 5, ptr::null_mut()));
            assert_eq!(cells_problem_count(d), 0);
            assert!(!cells_set_cell(d, 1, 0, c"a\x07".as_ptr(), 2, &mut f));
            assert_eq!((f.at, f.number), (CellsAt::CellsAtRow, 1));
            cells_failure_free(&mut f);
            let mut s = ArrowSchema::empty();
            let mut a = ArrowArray::empty();
            cells_export(d, &mut s, &mut a);
            cells_free(d);
            // The arrays outlive the doc.
            let cost = &**a.children.add(1);
            assert_eq!(*(*cost.buffers.add(1) as *const i128).add(1), 1200);
            (s.release.unwrap())(&mut s);
            (a.release.unwrap())(&mut a);

            let bad = b"item:EURO\n";
            assert!(cells_read(bad.as_ptr(), bad.len(), 1, &mut f).is_null());
            assert_eq!((f.at, f.number), (CellsAt::CellsAtColumn, 1));
            cells_failure_free(&mut f);
        }
    }

    fn text(s: CellsStr) -> String {
        // SAFETY: the doc that lent it is alive.
        String::from_utf8(unsafe { std::slice::from_raw_parts(s.ptr as *const u8, s.len) }.to_vec()).unwrap()
    }

    fn lent(s: &str) -> CellsStr {
        CellsStr::of(s)
    }

    #[test]
    fn imports_held_and_on_disk() {
        let accounts = "account:text!\nrent\nbank\n";
        let money = "---\nimport accounts from \"../accounts.cell\"\n---\namount:EUR,to:accounts.account\n950,rent\n9,rnet\n";
        let mut f = CellsFailure { at: CellsAt::CellsAtNone, number: 0, why: ptr::null_mut(), why_len: 0 };
        // SAFETY: valid pointers throughout.
        unsafe {
            let paths = [lent("books/accounts.cell"), lent("books/home/money.cell")];
            let texts = [lent(accounts), lent(money)];
            let d =
                cells_read_with(money.as_ptr(), money.len(), lent("books/home/money.cell"), paths.as_ptr(), texts.as_ptr(), 2, 1, &mut f);
            assert!(!d.is_null());
            assert_eq!(cells_problem_count(d), 1);
            assert_eq!(cells_problem(d, 0).kind, CellsKind::CellsKindReference);
            cells_free(d);

            let dir = std::env::temp_dir().join(format!("cells-ffi-{}", std::process::id()));
            std::fs::create_dir_all(dir.join("home")).unwrap();
            std::fs::write(dir.join("accounts.cell"), accounts).unwrap();
            let at = dir.join("home/money.cell");
            let at = at.to_str().unwrap();
            let d = cells_read_at(money.as_ptr(), money.len(), lent(at), 1, &mut f);
            assert!(!d.is_null());
            assert_eq!(cells_problem_count(d), 1);
            assert_eq!(text(cells_describe(d, 0)), "row 1, to: \"rnet\" is no key of accounts");
            cells_free(d);
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn a_plain_file_through_the_abi() {
        let src = b"a;a;c\n1;2\n3;4;5;6\n";
        let mut f = CellsFailure { at: CellsAt::CellsAtNone, number: 0, why: ptr::null_mut(), why_len: 0 };
        // SAFETY: valid pointers throughout.
        unsafe {
            assert_eq!(cells_detect(src.as_ptr(), src.len()), b';');
            let d = cells_read_as(src.as_ptr(), src.len(), b';', 1, &mut f);
            assert!(!d.is_null());
            assert_eq!(cells_dialect(d), b';');
            assert_eq!(cells_naming_count(d), 1);
            let n = cells_naming(d, 0);
            assert_eq!((n.column, n.blank, n.first), (1, false, 0));
            assert_eq!(text(cells_naming_why(d, 0)), "\"a\" names column 1 too");
            assert_eq!(cells_problem_count(d), 2);
            assert_eq!(cells_problem(d, 1).kind, CellsKind::CellsKindRagged);
            assert_eq!((cells_width(d, 0), cells_width(d, 1)), (2, 4));
            // c moved first, b renamed, a new column last.
            let cells = [lent("x"), lent("y")];
            let order = [
                CellsColumnAt { kept: 2, renamed: false, name: lent(""), cells: ptr::null(), count: 0 },
                CellsColumnAt { kept: 1, renamed: true, name: lent("b"), cells: ptr::null(), count: 0 },
                CellsColumnAt { kept: -1, renamed: false, name: lent("new"), cells: cells.as_ptr(), count: 2 },
            ];
            let r = cells_reshaped(d, order.as_ptr(), order.len(), 1, &mut f);
            assert!(!r.is_null());
            assert_eq!(text(cells_write(r)), "c;b;new\n;2;x\n5;4;y;6\n");
            let head = [lent("c:int"), lent("b:int"), lent("new")];
            assert_eq!(text(cells_written_as(r, 0, head.as_ptr(), 3)), "c:int,b:int,new\n,2,x\n5,4,y\n");
            cells_free(r);
            cells_free(d);

            let src = b"cost:EUR,qty:int,total:EUR=cost * qty\n2.50,4,\n";
            let d = cells_read(src.as_ptr(), src.len(), 1, &mut f);
            assert_eq!(cells_origin(d, 0, 2), CellsOrigin::CellsOriginFormula);
            assert!(cells_type(d, CellsOf::CellsOfColumn, 2).formula);
            assert_eq!(text(cells_type_part(d, CellsOf::CellsOfColumn, 2, CellsPart::CellsPartFormula, 0)), "cost * qty");
            let spelled = "EUR=cost * 2";
            assert!(cells_set_type(d, 2, spelled.as_ptr() as *const c_char, spelled.len(), &mut f));
            assert_eq!(text(cells_write(d)), "cost:EUR,qty:int,total:EUR=cost * 2\n2.50,4,\n");
            let bad = "EUR=cost *";
            assert!(!cells_set_type(d, 2, bad.as_ptr() as *const c_char, bad.len(), &mut f));
            assert_eq!((f.at, f.number), (CellsAt::CellsAtColumn, 3));
            cells_failure_free(&mut f);
            cells_free(d);
        }
    }
}
