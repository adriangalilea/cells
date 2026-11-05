//! The Cells engine as WebAssembly. A page hands the file's bytes in, gets a handle, and
//! reads the columns where they lie in linear memory: offsets, bytes, values and validity
//! as typed-array views, strings decoded only when a cell is asked for. One description
//! of the columns (JSON, small) says where everything is; nothing is serialized per row.
//!
//! Every function that returns bytes returns their address and leaves their length for
//! `cells_len`; the bytes live until the next call on the same handle.

use std::cell::UnsafeCell;
use std::fmt::Write;

use cells::{Base, ColumnAt, Data, Dialect, Doc, Naming, Origin, Separator};

/// Results that outlive the call that made them, one set per handle.
pub struct Handle {
    doc: Doc,
    out: Vec<u8>,
    problems: Vec<u32>,
    /// Where each cell `cells_cells` wrote ends, in UTF-16 units.
    ends: Vec<u32>,
}

/// The module's one piece of state: the length of the last bytes returned, and the last
/// read's error. A wasm instance runs one call at a time.
struct Shared {
    len: usize,
    error: Vec<u8>,
}

struct Global(UnsafeCell<Shared>);

// SAFETY: wasm32-unknown-unknown without the atomics feature has one thread.
unsafe impl Sync for Global {}

static SHARED: Global = Global(UnsafeCell::new(Shared { len: 0, error: Vec::new() }));

fn shared() -> &'static mut Shared {
    // SAFETY: one thread, and no call holds this across another.
    unsafe { &mut *SHARED.0.get() }
}

fn give(bytes: &[u8]) -> *const u8 {
    shared().len = bytes.len();
    bytes.as_ptr()
}

/// The length of the bytes the last call returned.
#[unsafe(no_mangle)]
pub extern "C" fn cells_len() -> usize {
    shared().len
}

/// Room for `len` bytes the page fills (a file, a cell's text).
#[unsafe(no_mangle)]
pub extern "C" fn cells_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Gives back room from `cells_alloc` that no call took.
///
/// # Safety
/// `ptr` and `cap` are exactly what `cells_alloc(cap)` gave.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_free(ptr: *mut u8, cap: usize) {
    // SAFETY: the caller's promise.
    drop(unsafe { Vec::from_raw_parts(ptr, 0, cap.max(1)) });
}

/// Takes room from `cells_alloc(cap)` with its first `len` bytes written.
///
/// # Safety
/// `ptr` and `cap` are what `cells_alloc(cap)` gave, `len <= cap` of it written.
unsafe fn took(ptr: *mut u8, len: usize, cap: usize) -> Vec<u8> {
    assert!(len <= cap.max(1));
    // SAFETY: the caller's promise.
    unsafe { Vec::from_raw_parts(ptr, len, cap.max(1)) }
}

/// Reads the `len` bytes written at `ptr` (room from `cells_alloc(cap)`, which this call
/// takes). Null only for bytes that are not UTF-8 or past 4 GiB, `cells_error` saying why;
/// a broken grammar reads, its breaks in `cells_meta`.
///
/// # Safety
/// `ptr` and `cap` are what `cells_alloc(cap)` gave, `len` bytes of it written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_read(ptr: *mut u8, len: usize, cap: usize) -> *mut Handle {
    // SAFETY: the caller's promise.
    unsafe { cells_read_as(ptr, len, cap, 0) }
}

/// The dialect a call names: 0 for Cells, else a plain file's separator byte.
fn dialect(sep: u32) -> Dialect {
    if sep == 0 {
        return Dialect::Cells;
    }
    Dialect::Plain(Separator::of(sep as u8).unwrap_or_else(|| panic!("{sep} is no separator")))
}

fn handled(read: Result<Doc, cells::Error>) -> *mut Handle {
    match read {
        Ok(doc) => Box::into_raw(Box::new(Handle { doc, out: Vec::new(), problems: Vec::new(), ends: Vec::new() })),
        Err(e) => {
            failed(e);
            std::ptr::null_mut()
        }
    }
}

fn failed(e: cells::Error) {
    let mut s = String::from("{\"at\":");
    json_str(&mut s, &e.at.to_string());
    s.push_str(",\"why\":");
    json_str(&mut s, &e.why);
    s.push('}');
    shared().error = s.into_bytes();
}

/// `cells_read` in a dialect: `sep` 0 for Cells, else a plain file's separator byte.
///
/// # Safety
/// As `cells_read`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_read_as(ptr: *mut u8, len: usize, cap: usize, sep: u32) -> *mut Handle {
    // SAFETY: the caller's promise.
    let bytes = unsafe { took(ptr, len, cap) };
    handled(cells::read_as(bytes, dialect(sep), 1))
}

/// The separator the first lines of the `len` bytes at `ptr` agree on, as its byte; the
/// bytes stay the caller's.
///
/// # Safety
/// `ptr` points at `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_detect(ptr: *const u8, len: usize) -> u32 {
    // SAFETY: the caller's promise.
    let bytes = if len == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(ptr, len) } };
    cells::detect(bytes).byte() as u32
}

/// Strings a page wrote one after another, each a little-endian `u32` length and its bytes.
struct Strings<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Strings<'a> {
    fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.bytes[self.at..self.at + 4].try_into().unwrap());
        self.at += 4;
        v
    }

    fn str(&mut self) -> &'a str {
        let n = self.u32() as usize;
        let s = std::str::from_utf8(&self.bytes[self.at..self.at + n]).expect("the page writes UTF-8");
        self.at += n;
        s
    }

    fn list(&mut self) -> Vec<&'a str> {
        let n = self.u32();
        (0..n).map(|_| self.str()).collect()
    }
}

/// Reads the file as `cells_read` does, its imports followed through the files the page
/// holds: a list (room from `cells_alloc(files_cap)`, which this call takes) of the file's
/// own path, then each file's path and its text. An import of a file the list does not
/// hold reads as unreachable.
///
/// # Safety
/// `ptr`, `len`, `cap` as for `cells_read`; `files`, `files_len`, `files_cap` the same for
/// the list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_read_with(
    ptr: *mut u8,
    len: usize,
    cap: usize,
    files: *mut u8,
    files_len: usize,
    files_cap: usize,
) -> *mut Handle {
    // SAFETY: the caller's promise.
    let (bytes, list) = unsafe { (took(ptr, len, cap), took(files, files_len, files_cap)) };
    let list = Strings { bytes: &list, at: 0 }.list();
    let (at, rest) = list.split_first().expect("the list opens with the file's own path");
    let held: std::collections::BTreeMap<String, String> = rest.chunks(2).map(|p| (p[0].to_owned(), p[1].to_owned())).collect();
    handled(cells::read_with(bytes, &mut cells::files::Files::new(&held, at), 1))
}

/// The file with exactly the columns the page wrote (room from `cells_alloc(cap)`, `len`
/// bytes of it, which this call takes), as a new handle; null when it does not read, the
/// error at `cells_error`. Each column is a tag byte and its parts: 0 and a `u32` index
/// (kept), 1, an index and a name (renamed), 2, a header cell and a list of cells (new).
///
/// # Safety
/// `h` is a live handle; `ptr`, `len`, `cap` as for `cells_read`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_reshaped(h: *mut Handle, ptr: *mut u8, len: usize, cap: usize) -> *mut Handle {
    // SAFETY: the caller's promise.
    let (h, spec) = unsafe { (handle(h), took(ptr, len, cap)) };
    let mut s = Strings { bytes: &spec, at: 0 };
    let mut parts: Vec<(u8, usize, &str, Vec<&str>)> = Vec::new();
    while s.at < spec.len() {
        let tag = spec[s.at];
        s.at += 1;
        parts.push(match tag {
            0 => (0, s.u32() as usize, "", Vec::new()),
            1 => {
                let k = s.u32() as usize;
                (1, k, s.str(), Vec::new())
            }
            2 => {
                let header = s.str();
                (2, 0, header, s.list())
            }
            t => panic!("{t} is no column's tag"),
        });
    }
    let columns: Vec<ColumnAt> = parts
        .iter()
        .map(|(tag, k, text, cells)| match tag {
            0 => ColumnAt::Kept(*k),
            1 => ColumnAt::Renamed(*k, text),
            _ => ColumnAt::New(text, cells),
        })
        .collect();
    handled(h.doc.reshaped(&columns, 1))
}

/// The file written in a dialect (`sep` as for `cells_read_as`), its header cells the list
/// the page wrote (room from `cells_alloc(cap)`, which this call takes), or the file's own
/// when the list is empty.
///
/// # Safety
/// `h` is a live handle; `ptr`, `len`, `cap` as for `cells_read`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_written_as(h: *mut Handle, sep: u32, ptr: *mut u8, len: usize, cap: usize) -> *const u8 {
    // SAFETY: the caller's promise.
    let (h, spec) = unsafe { (handle(h), took(ptr, len, cap)) };
    let header = Strings { bytes: &spec, at: 0 }.list();
    h.out = h.doc.written_as(dialect(sep), if header.is_empty() { None } else { Some(&header) }).into_bytes();
    give(&h.out)
}

/// A header problem in words without its place, `has no name`.
///
/// # Safety
/// `h` is a live handle; `i` below the meta's `names`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_naming_why(h: *mut Handle, i: usize) -> *const u8 {
    // SAFETY: the caller's promise.
    let h = unsafe { handle(h) };
    h.out = h.doc.naming_why(&h.doc.naming()[i]).into_bytes();
    give(&h.out)
}

/// The last failed read's error, `{"at":"row 3","why":"..."}`.
#[unsafe(no_mangle)]
pub extern "C" fn cells_error() -> *const u8 {
    give(&shared().error)
}

/// # Safety
/// `h` is a live handle from `cells_read`.
unsafe fn handle<'a>(h: *mut Handle) -> &'a mut Handle {
    assert!(!h.is_null(), "a null handle");
    // SAFETY: the caller's promise.
    unsafe { &mut *h }
}

/// Lets a handle go.
///
/// # Safety
/// `h` is a live handle from `cells_read`, used no more.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_drop(h: *mut Handle) {
    // SAFETY: the caller's promise.
    drop(unsafe { Box::from_raw(h) });
}

fn json_str(s: &mut String, v: &str) {
    s.push('"');
    for c in v.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(s, "\\u{:04x}", c as u32);
            }
            c => s.push(c),
        }
    }
    s.push('"');
}

fn addr<T>(v: &[T]) -> usize {
    v.as_ptr() as usize
}

/// Where everything is: the rows, the dialect (`"cells"`, or a plain file's separator), the
/// named types, every column's type, formula, kind and buffers (its cells' origins too,
/// bytes 0 written, 1 its formula's, 2 their own, when any is computed), the header
/// problems, where the grammar breaks (`{at, why, span}`, the span in the text written
/// now), the problems as `u32` triples (row, column, kind: 0 type, 1 missing, 2 key,
/// 3 ragged, 4 formula, 5 reference). Read again after every edit: an edit may move a
/// column's buffers.
///
/// # Safety
/// `h` is a live handle from `cells_read`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_meta(h: *mut Handle) -> *const u8 {
    // SAFETY: the caller's promise.
    let h = unsafe { handle(h) };
    let doc = &h.doc;
    h.problems.clear();
    for p in doc.problems() {
        h.problems.extend([p.row, p.column, p.kind() as u32]);
    }
    let mut s = String::with_capacity(512);
    let _ = write!(s, "{{\"rows\":{},\"newline\":", doc.rows());
    json_str(&mut s, doc.newline());
    s.push_str(",\"dialect\":");
    match doc.dialect() {
        Dialect::Cells => json_str(&mut s, "cells"),
        Dialect::Plain(sep) => json_str(&mut s, &(sep.byte() as char).to_string()),
    }
    let _ = write!(s, ",\"quotesAll\":{},\"names\":[", doc.quotes_all());
    for (k, n) in doc.naming().iter().enumerate() {
        if k > 0 {
            s.push(',');
        }
        let kind = match n {
            Naming::Blank { .. } => "blank",
            Naming::Twice { .. } => "twice",
        };
        let _ = write!(s, "{{\"column\":{},\"kind\":\"{kind}\"}}", n.column());
    }
    s.push_str("],\"broken\":[");
    for (k, (e, span)) in doc.broken().iter().zip(doc.broken_spans()).enumerate() {
        if k > 0 {
            s.push(',');
        }
        s.push_str("{\"at\":");
        json_str(&mut s, &e.at.to_string());
        s.push_str(",\"why\":");
        json_str(&mut s, &e.why);
        if let Some(span) = span {
            let _ = write!(s, ",\"span\":[{},{}]", span.start, span.len);
        }
        s.push('}');
    }
    s.push(']');
    s.push_str(",\"prelude\":");
    json_str(&mut s, doc.prelude_text());
    s.push_str(",\"unread\":");
    json_str(&mut s, doc.unread());
    let _ = write!(
        s,
        ",\"trailing\":{},\"problems\":{},\"problemCount\":{},\"named\":{{",
        doc.trailing(),
        addr(&h.problems),
        doc.problems().len()
    );
    for (k, t) in doc.named().iter().enumerate() {
        if k > 0 {
            s.push(',');
        }
        json_str(&mut s, &t.name);
        s.push(':');
        json_str(&mut s, &t.ty.shorthand());
    }
    s.push_str("},\"columns\":[");
    for (k, c) in doc.columns().iter().enumerate() {
        if k > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        json_str(&mut s, &c.name);
        s.push_str(",\"type\":");
        json_str(&mut s, &c.ty.shorthand());
        if let Some(spelled) = &c.spelled {
            s.push_str(",\"spelled\":");
            json_str(&mut s, spelled);
        }
        s.push_str(",\"base\":");
        json_str(&mut s, c.ty.base.tag());
        let _ = write!(s, ",\"inferred\":{},\"key\":{},\"optional\":{}", c.ty.inferred, c.ty.key, c.ty.optional);
        if let Some(f) = &c.formula {
            s.push_str(",\"formula\":");
            json_str(&mut s, f);
        }
        if !c.origin.is_empty() {
            // `Origin` as bytes, the order its variants are declared in.
            let bytes: &Vec<Origin> = &c.origin;
            let _ = write!(s, ",\"origin\":{}", bytes.as_ptr() as usize);
        }
        match &c.ty.base {
            Base::Ref(targets) => {
                s.push_str(",\"targets\":[");
                for (j, t) in targets.iter().enumerate() {
                    if j > 0 {
                        s.push(',');
                    }
                    json_str(&mut s, &format!("{}.{}", t.table, t.column));
                }
                s.push(']');
            }
            Base::Money(cur) => {
                s.push_str(",\"currency\":");
                json_str(&mut s, cur.as_str());
            }
            Base::Choice(words) => {
                s.push_str(",\"choices\":[");
                for (j, w) in words.iter().enumerate() {
                    if j > 0 {
                        s.push(',');
                    }
                    json_str(&mut s, w);
                }
                s.push(']');
            }
            Base::Bool { yes, no } => {
                s.push_str(",\"words\":[");
                json_str(&mut s, yes);
                s.push(',');
                json_str(&mut s, no);
                s.push(']');
            }
            _ => {}
        }
        if let Some(f) = &c.ty.format {
            s.push_str(",\"format\":");
            json_str(&mut s, &f.spelled);
        }
        let _ = write!(s, ",\"nulls\":{},\"valid\":{}", c.nulls(), addr(&c.valid.bytes));
        match &c.data {
            Data::Text { offsets, bytes } => {
                let _ =
                    write!(s, ",\"kind\":\"text\",\"offsets\":{},\"bytes\":{},\"bytesLen\":{}", addr(offsets), addr(bytes), bytes.len());
            }
            Data::Int(v) => {
                let _ = write!(s, ",\"kind\":\"int\",\"values\":{}", addr(v));
            }
            Data::Decimal { values, scale } => {
                let _ = write!(s, ",\"kind\":\"decimal\",\"values\":{},\"scale\":{scale}", addr(values));
            }
            Data::Bool(b) => {
                let _ = write!(s, ",\"kind\":\"bool\",\"values\":{}", addr(&b.bytes));
            }
            Data::Date(v) => {
                let _ = write!(s, ",\"kind\":\"date\",\"values\":{}", addr(v));
            }
            Data::Time(v) => {
                let _ = write!(s, ",\"kind\":\"time\",\"values\":{}", addr(v));
            }
            Data::DateTime { values, unit } => {
                let _ = write!(s, ",\"kind\":\"datetime\",\"values\":{},\"unit\":{unit}", addr(values));
            }
            Data::Choice(v) => {
                let _ = write!(s, ",\"kind\":\"choice\",\"values\":{}", addr(v));
            }
        }
        s.push('}');
    }
    s.push_str("]}");
    h.out = s.into_bytes();
    give(&h.out)
}

/// The file's text: what was read, but for what was changed.
///
/// # Safety
/// `h` is a live handle from `cells_read`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_write(h: *mut Handle) -> *const u8 {
    // SAFETY: the caller's promise.
    let h = unsafe { handle(h) };
    h.out = h.doc.write().into_bytes();
    give(&h.out)
}

/// A stored cell's text as written, unquoted.
///
/// # Safety
/// `h` is a live handle from `cells_read`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_text(h: *mut Handle, row: usize, column: usize) -> *const u8 {
    // SAFETY: the caller's promise.
    let h = unsafe { handle(h) };
    h.out = h.doc.text(row, column).into_owned().into_bytes();
    give(&h.out)
}

/// Problem `i` in words: `row 3, cost: "8x.50" is not a number`.
///
/// # Safety
/// `h` is a live handle from `cells_read`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_describe(h: *mut Handle, i: usize) -> *const u8 {
    // SAFETY: the caller's promise.
    let h = unsafe { handle(h) };
    h.out = h.doc.describe(&h.doc.problems()[i]).into_bytes();
    give(&h.out)
}

/// Problem `i` in words without its place: `"8x.50" is not a number`.
///
/// # Safety
/// `h` is a live handle from `cells_read`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_why(h: *mut Handle, i: usize) -> *const u8 {
    // SAFETY: the caller's promise.
    let h = unsafe { handle(h) };
    h.out = h.doc.why(&h.doc.problems()[i]).into_bytes();
    give(&h.out)
}

/// Every stored cell's text, unquoted, row by row and in each row column by column, one
/// after another: one decode in the page, cut at `cells_cell_ends`, never a call per cell.
///
/// # Safety
/// `h` is a live handle from `cells_read`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_cells(h: *mut Handle) -> *const u8 {
    // SAFETY: the caller's promise.
    let h = unsafe { handle(h) };
    let (rows, stored) = (h.doc.rows(), h.doc.columns().len());
    let mut out = Vec::with_capacity(h.doc.source().len());
    let mut ends = Vec::with_capacity(rows * stored);
    let mut units = 0u32;
    for r in 0..rows {
        for t in h.doc.texts(r) {
            out.extend_from_slice(t.as_bytes());
            units += if t.is_ascii() { t.len() as u32 } else { t.chars().map(|c| c.len_utf16() as u32).sum() };
            ends.push(units);
        }
    }
    h.out = out;
    h.ends = ends;
    give(&h.out)
}

/// Where each cell the last `cells_cells` wrote ends, `u32`s in UTF-16 units of its text
/// decoded, rows times stored columns of them.
///
/// # Safety
/// `h` is a live handle from `cells_read`, `cells_cells` called on it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_cell_ends(h: *mut Handle) -> *const u32 {
    // SAFETY: the caller's promise.
    let h = unsafe { handle(h) };
    h.ends.as_ptr()
}

/// Takes the text a page wrote into room from `cells_alloc`.
///
/// # Safety
/// As `took`, the bytes UTF-8.
unsafe fn take(ptr: *mut u8, len: usize, cap: usize) -> String {
    // SAFETY: the caller's promise.
    String::from_utf8(unsafe { took(ptr, len, cap) }).expect("the page writes UTF-8")
}

/// One cell's text changed (room from `cells_alloc(cap)`, `len` bytes of it written, which
/// this call takes); 0 when it was, else 1 with the error at `cells_error` (text holding a
/// control character).
///
/// # Safety
/// `h` is a live handle; `ptr`, `len`, `cap` as for `cells_read`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_set_cell(h: *mut Handle, row: usize, column: usize, ptr: *mut u8, len: usize, cap: usize) -> u32 {
    // SAFETY: the caller's promise.
    let (h, text) = unsafe { (handle(h), take(ptr, len, cap)) };
    match h.doc.set_cell(row, column, &text) {
        Ok(()) => 0,
        Err(e) => {
            failed(e);
            1
        }
    }
}

/// One column's type rewritten in shorthand; 0 when it read, else 1 with the error at
/// `cells_error`.
///
/// # Safety
/// `h` is a live handle; `ptr`, `len`, `cap` as for `cells_read`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cells_set_type(h: *mut Handle, column: usize, ptr: *mut u8, len: usize, cap: usize) -> u32 {
    // SAFETY: the caller's promise.
    let (h, spelled) = unsafe { (handle(h), take(ptr, len, cap)) };
    match h.doc.set_type(column, &spelled) {
        Ok(()) => 0,
        Err(e) => {
            let mut s = String::from("{\"at\":");
            json_str(&mut s, &e.at.to_string());
            s.push_str(",\"why\":");
            json_str(&mut s, &e.why);
            s.push('}');
            shared().error = s.into_bytes();
            1
        }
    }
}
