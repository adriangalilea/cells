//! The Arrow C Data Interface (arrow.apache.org/docs/format/CDataInterface.html): a file's
//! columns handed to DuckDB, pyarrow, arrow-js or Swift as one struct array, each column a
//! child, its buffers the columns' own (shared, never copied). The structs are the
//! interface's stable C ABI, defined here.
//!
//! Each field carries its TCSV type in shorthand under the metadata key `tcsv.type`
//! (`EUR>=0`), so the currency and the bounds travel with the column.

use alloc::boxed::Box;
use alloc::ffi::CString;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ffi::{c_char, c_void};
use core::ptr;

use crate::column::Data;
use crate::doc::{Column, Doc};
use crate::types::Base;

pub const FLAG_NULLABLE: i64 = 2;

#[repr(C)]
#[derive(Debug)]
pub struct ArrowSchema {
    pub format: *const c_char,
    pub name: *const c_char,
    pub metadata: *const c_char,
    pub flags: i64,
    pub n_children: i64,
    pub children: *mut *mut ArrowSchema,
    pub dictionary: *mut ArrowSchema,
    pub release: Option<unsafe extern "C" fn(*mut ArrowSchema)>,
    pub private_data: *mut c_void,
}

#[repr(C)]
#[derive(Debug)]
pub struct ArrowArray {
    pub length: i64,
    pub null_count: i64,
    pub offset: i64,
    pub n_buffers: i64,
    pub n_children: i64,
    pub buffers: *mut *const c_void,
    pub children: *mut *mut ArrowArray,
    pub dictionary: *mut ArrowArray,
    pub release: Option<unsafe extern "C" fn(*mut ArrowArray)>,
    pub private_data: *mut c_void,
}

impl ArrowSchema {
    pub const fn empty() -> ArrowSchema {
        ArrowSchema {
            format: ptr::null(),
            name: ptr::null(),
            metadata: ptr::null(),
            flags: 0,
            n_children: 0,
            children: ptr::null_mut(),
            dictionary: ptr::null_mut(),
            release: None,
            private_data: ptr::null_mut(),
        }
    }
}

impl ArrowArray {
    pub const fn empty() -> ArrowArray {
        ArrowArray {
            length: 0,
            null_count: 0,
            offset: 0,
            n_buffers: 0,
            n_children: 0,
            buffers: ptr::null_mut(),
            children: ptr::null_mut(),
            dictionary: ptr::null_mut(),
            release: None,
            private_data: ptr::null_mut(),
        }
    }
}

/// What a schema owns: its strings, its children and their pointers.
struct SchemaOwn {
    _format: CString,
    _name: CString,
    _metadata: Vec<u8>,
    children: Vec<*mut ArrowSchema>,
    dictionary: *mut ArrowSchema,
}

/// What an array owns: the buffers it points into, kept alive by their `Arc`s.
struct ArrayOwn {
    _keep: Vec<Arc<dyn Send + Sync>>,
    buffers: Vec<*const c_void>,
    children: Vec<*mut ArrowArray>,
    dictionary: *mut ArrowArray,
}

unsafe extern "C" fn release_schema(s: *mut ArrowSchema) {
    // SAFETY: called by the consumer, once, on a schema `schema_of` built.
    unsafe {
        let own = Box::from_raw((*s).private_data as *mut SchemaOwn);
        for &c in &own.children {
            if let Some(r) = (*c).release {
                r(c);
            }
            drop(Box::from_raw(c));
        }
        if !own.dictionary.is_null() {
            if let Some(r) = (*own.dictionary).release {
                r(own.dictionary);
            }
            drop(Box::from_raw(own.dictionary));
        }
        (*s).release = None;
    }
}

unsafe extern "C" fn release_array(a: *mut ArrowArray) {
    // SAFETY: called by the consumer, once, on an array `array_of` built.
    unsafe {
        let own = Box::from_raw((*a).private_data as *mut ArrayOwn);
        for &c in &own.children {
            if let Some(r) = (*c).release {
                r(c);
            }
            drop(Box::from_raw(c));
        }
        if !own.dictionary.is_null() {
            if let Some(r) = (*own.dictionary).release {
                r(own.dictionary);
            }
            drop(Box::from_raw(own.dictionary));
        }
        (*a).release = None;
    }
}

fn cstring(s: &str) -> CString {
    // A NUL cannot pass a C string; a name holding one is cut there.
    let s = s.split('\0').next().unwrap();
    CString::new(s).unwrap()
}

fn metadata(pairs: &[(&str, &str)]) -> Vec<u8> {
    let mut m = Vec::new();
    m.extend_from_slice(&(pairs.len() as i32).to_le_bytes());
    for (k, v) in pairs {
        m.extend_from_slice(&(k.len() as i32).to_le_bytes());
        m.extend_from_slice(k.as_bytes());
        m.extend_from_slice(&(v.len() as i32).to_le_bytes());
        m.extend_from_slice(v.as_bytes());
    }
    m
}

fn schema(
    format: String,
    name: &str,
    meta: Option<Vec<u8>>,
    flags: i64,
    children: Vec<ArrowSchema>,
    dictionary: Option<ArrowSchema>,
) -> ArrowSchema {
    let format = CString::new(format).unwrap();
    let name = cstring(name);
    let meta = meta.unwrap_or_default();
    let children: Vec<*mut ArrowSchema> = children.into_iter().map(|c| Box::into_raw(Box::new(c))).collect();
    let dictionary = dictionary.map_or(ptr::null_mut(), |d| Box::into_raw(Box::new(d)));
    let own = Box::new(SchemaOwn { _format: format, _name: name, _metadata: meta, children, dictionary });
    ArrowSchema {
        format: own._format.as_ptr(),
        name: own._name.as_ptr(),
        metadata: if own._metadata.is_empty() { ptr::null() } else { own._metadata.as_ptr() as *const c_char },
        flags,
        n_children: own.children.len() as i64,
        children: if own.children.is_empty() { ptr::null_mut() } else { own.children.as_ptr() as *mut *mut ArrowSchema },
        dictionary: own.dictionary,
        release: Some(release_schema),
        private_data: Box::into_raw(own) as *mut c_void,
    }
}

fn array(
    length: usize,
    null_count: usize,
    keep: Vec<Arc<dyn Send + Sync>>,
    buffers: Vec<*const c_void>,
    children: Vec<ArrowArray>,
    dictionary: Option<ArrowArray>,
) -> ArrowArray {
    let children: Vec<*mut ArrowArray> = children.into_iter().map(|c| Box::into_raw(Box::new(c))).collect();
    let dictionary = dictionary.map_or(ptr::null_mut(), |d| Box::into_raw(Box::new(d)));
    let own = Box::new(ArrayOwn { _keep: keep, buffers, children, dictionary });
    ArrowArray {
        length: length as i64,
        null_count: null_count as i64,
        offset: 0,
        n_buffers: own.buffers.len() as i64,
        n_children: own.children.len() as i64,
        buffers: own.buffers.as_ptr() as *mut *const c_void,
        children: if own.children.is_empty() { ptr::null_mut() } else { own.children.as_ptr() as *mut *mut ArrowArray },
        dictionary: own.dictionary,
        release: Some(release_array),
        private_data: Box::into_raw(own) as *mut c_void,
    }
}

/// A column's Arrow format string.
pub fn format_of(c: &Column) -> String {
    match &c.data {
        Data::Text { .. } => "u".into(),
        Data::Int(_) => "l".into(),
        Data::Decimal { scale, .. } => format!("d:38,{scale}"),
        Data::Bool(_) => "b".into(),
        Data::Date(_) => "tdD".into(),
        Data::Time(_) => "tts".into(),
        Data::DateTime { unit, .. } => match unit {
            0 => "tss:UTC".into(),
            3 => "tsm:UTC".into(),
            6 => "tsu:UTC".into(),
            _ => "tsn:UTC".into(),
        },
        Data::Choice(_) => "s".into(),
    }
}

fn column_schema(c: &Column) -> ArrowSchema {
    let ty = match &c.computed {
        Some(e) => format!("{} = {e}", c.ty.shorthand()),
        None => c.ty.shorthand(),
    };
    let meta = metadata(&[("tcsv.type", &ty)]);
    let dictionary = matches!(c.data, Data::Choice(_)).then(|| schema("u".into(), "", None, 0, Vec::new(), None));
    schema(format_of(c), &c.name, Some(meta), FLAG_NULLABLE, Vec::new(), dictionary)
}

fn ptr_of<T>(v: &Arc<Vec<T>>) -> *const c_void {
    v.as_ptr() as *const c_void
}

fn column_array(c: &Column) -> ArrowArray {
    let n = c.len();
    let nulls = c.nulls();
    let valid_buf = Arc::clone(&c.valid.bytes);
    let validity = if nulls == 0 { ptr::null() } else { ptr_of(&valid_buf) };
    let mut keep: Vec<Arc<dyn Send + Sync>> = Vec::from([valid_buf as Arc<dyn Send + Sync>]);
    let (buffers, dictionary) = match &c.data {
        Data::Text { offsets, bytes } => {
            let b = [validity, ptr_of(offsets), ptr_of(bytes)];
            keep.push(offsets.clone());
            keep.push(bytes.clone());
            (b.to_vec(), None)
        }
        Data::Int(v) => (keep_one(&mut keep, v, validity), None),
        Data::Decimal { values, .. } => (keep_one(&mut keep, values, validity), None),
        Data::Bool(bits) => {
            keep.push(bits.bytes.clone());
            (Vec::from([validity, ptr_of(&bits.bytes)]), None)
        }
        Data::Date(v) | Data::Time(v) => (keep_one(&mut keep, v, validity), None),
        Data::DateTime { values, .. } => (keep_one(&mut keep, values, validity), None),
        Data::Choice(v) => {
            let Base::Choice(words) = &c.ty.base else { unreachable!("a choice column's type is a choice") };
            let mut offsets = Vec::with_capacity(words.len() + 1);
            let mut bytes = Vec::new();
            offsets.push(0i32);
            for w in words {
                bytes.extend_from_slice(w.as_bytes());
                offsets.push(bytes.len() as i32);
            }
            let (offsets, bytes) = (Arc::new(offsets), Arc::new(bytes));
            let dict_buffers = Vec::from([ptr::null(), ptr_of(&offsets), ptr_of(&bytes)]);
            let dict = array(words.len(), 0, Vec::from([offsets as Arc<dyn Send + Sync>, bytes]), dict_buffers, Vec::new(), None);
            (keep_one(&mut keep, v, validity), Some(dict))
        }
    };
    array(n, nulls, keep, buffers, Vec::new(), dictionary)
}

fn keep_one<T: Send + Sync + 'static>(
    keep: &mut Vec<Arc<dyn Send + Sync>>,
    v: &Arc<Vec<T>>,
    validity: *const c_void,
) -> Vec<*const c_void> {
    keep.push(v.clone());
    Vec::from([validity, ptr_of(v)])
}

/// The file's columns as one struct array (`+s`), the stored columns then the computed
/// ones. The caller owns both and calls each one's `release` once.
pub fn export(doc: &Doc) -> (ArrowSchema, ArrowArray) {
    let cols = doc.columns();
    let fields: Vec<ArrowSchema> = cols.iter().map(column_schema).collect();
    let arrays: Vec<ArrowArray> = cols.iter().map(column_array).collect();
    let s = schema("+s".into(), "", None, 0, fields, None);
    let a = array(doc.rows(), 0, Vec::new(), Vec::from([ptr::null()]), arrays, None);
    (s, a)
}

/// One column alone.
pub fn export_column(doc: &Doc, column: usize) -> (ArrowSchema, ArrowArray) {
    let c = &doc.columns()[column];
    (column_schema(c), column_array(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_and_releases() {
        let doc = crate::read("a:text,b:int,c:x|y,d:EUR?\nhi,1,x,1.50\n,2,y,\n".into()).unwrap();
        let (mut s, mut a) = export(&doc);
        assert_eq!(a.length, 2);
        assert_eq!(a.n_children, 4);
        // SAFETY: the structs `export` built, read and released once.
        unsafe {
            let c = &**a.children.add(3);
            assert_eq!(c.null_count, 1);
            let vals = *c.buffers.add(1) as *const i128;
            assert_eq!(*vals, 150);
            let f = &**s.children.add(3);
            assert_eq!(core::ffi::CStr::from_ptr(f.format).to_str().unwrap(), "d:38,2");
            let choice = &**a.children.add(2);
            assert!(!choice.dictionary.is_null());
            (s.release.unwrap())(&mut s);
            (a.release.unwrap())(&mut a);
        }
        assert!(s.release.is_none() && a.release.is_none());
    }
}
