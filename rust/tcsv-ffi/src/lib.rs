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
use tcsv::{At, Doc};

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
    let src = if len == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec() };
    match tcsv::read_parallel(src, threads.max(1)) {
        Ok(doc) => Box::into_raw(Box::new(TcsvDoc { doc, out: String::new() })),
        Err(e) => {
            fail(failure, e);
            ptr::null_mut()
        }
    }
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

/// What a problem is: the suite's three kinds.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TcsvKind {
    TcsvKindType = 0,
    TcsvKindMissing = 1,
    TcsvKindKey = 2,
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

/// One cell's text changed; every other byte stays as read.
///
/// # Safety
/// `d` is a live doc; `row < tcsv_rows(d)`, `column < tcsv_stored(d)`; `text` points at
/// `len` bytes of UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcsv_set_cell(d: *mut TcsvDoc, row: usize, column: usize, text: *const c_char, len: usize) {
    // SAFETY: the caller's promise.
    let (d, t) = unsafe { (doc(d), text_of(text, len)) };
    d.doc.set_cell(row, column, t);
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
            tcsv_set_cell(d, 1, 1, c"12.00".as_ptr(), 5);
            assert_eq!(tcsv_problem_count(d), 0);
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
}
