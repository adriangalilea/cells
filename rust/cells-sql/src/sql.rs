//! `cells sql`: a folder of `.cells` files asked as one database, through DuckDB (SPEC.md,
//! The query language). Every file is a table named by its stem, and so is every table a
//! file writes in its prelude, by its name; tables of one name, in any number of files, are
//! one table, their columns joined by name. Each table has one more column, `file`, the
//! path of the file each row is read from.
//!
//! The columns reach DuckDB through a table function, `cells_scan`, that writes the
//! engine's columns into DuckDB's vectors a chunk at a time, only the columns a query reads,
//! as many times as the query reads the table. The query is DuckDB's SQL, whole; what it
//! returns is written as `cells show` writes a table, tab-separated, each cell its value.

use std::collections::BTreeSet;
use std::ffi::{CStr, CString, c_char, c_void};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::ptr;
use std::sync::Arc;

use cells::value::{self, Instant};
use cells::{Base, Data, Decimal, Dialect, Doc, Separator, Statement};
use libduckdb_sys as ffi;

use cells_disk::Disk;

/// The column every table is given: the path of the file a row is read from.
const FILE: &str = "file";

/// DuckDB's widest decimal, the engine's own: `Decimal128(38, scale)`.
const WIDTH: u8 = 38;

/// What a column is in DuckDB. The one place a Cells type becomes a SQL type:
///
/// | Cells | DuckDB |
/// |---|---|
/// | `text`, `url`, `email`, `id` | VARCHAR |
/// | `int` | BIGINT |
/// | a currency | DECIMAL(38, its scale): `EUR` 2, `EUR(6)` 6, `JPY` 0 |
/// | `num`, `pct` | DECIMAL(38, the most digits after the point a value of the table holds) |
/// | `bool` and its pairs (`yes/no`) | BOOLEAN |
/// | `date` | DATE |
/// | `time` | TIME |
/// | `datetime` | TIMESTAMP in UTC, TIMESTAMP_NS when a value holds nanoseconds |
/// | a choice | ENUM of its words, in their order |
/// | a reference | its key's type |
#[derive(Clone, Debug, PartialEq)]
enum Sql {
    Varchar,
    Bigint,
    Decimal(u8),
    Boolean,
    Date,
    Time,
    Timestamp { nanos: bool },
    Enum(Vec<String>),
}

impl Sql {
    /// The type in DuckDB's words, for a message.
    fn spelled(&self) -> String {
        match self {
            Sql::Varchar => "VARCHAR".into(),
            Sql::Bigint => "BIGINT".into(),
            Sql::Decimal(s) => format!("DECIMAL({WIDTH}, {s})"),
            Sql::Boolean => "BOOLEAN".into(),
            Sql::Date => "DATE".into(),
            Sql::Time => "TIME".into(),
            Sql::Timestamp { nanos: false } => "TIMESTAMP".into(),
            Sql::Timestamp { nanos: true } => "TIMESTAMP_NS".into(),
            Sql::Enum(words) => format!("ENUM({})", words.join(", ")),
        }
    }

    /// Two files' types of one column as one: a scale or a unit the wider of the two, any
    /// other difference none.
    fn joined(&self, other: &Sql) -> Option<Sql> {
        match (self, other) {
            (Sql::Decimal(a), Sql::Decimal(b)) => Some(Sql::Decimal(*a.max(b))),
            (Sql::Timestamp { nanos: a }, Sql::Timestamp { nanos: b }) => Some(Sql::Timestamp { nanos: *a || *b }),
            (a, b) if a == b => Some(a.clone()),
            _ => None,
        }
    }

    /// The type DuckDB is handed for it.
    fn logical(&self) -> ffi::duckdb_logical_type {
        // SAFETY: each constructor returns a type the caller destroys.
        unsafe {
            match self {
                Sql::Varchar => ffi::duckdb_create_logical_type(ffi::DUCKDB_TYPE_DUCKDB_TYPE_VARCHAR),
                Sql::Bigint => ffi::duckdb_create_logical_type(ffi::DUCKDB_TYPE_DUCKDB_TYPE_BIGINT),
                Sql::Decimal(s) => ffi::duckdb_create_decimal_type(WIDTH, *s),
                Sql::Boolean => ffi::duckdb_create_logical_type(ffi::DUCKDB_TYPE_DUCKDB_TYPE_BOOLEAN),
                Sql::Date => ffi::duckdb_create_logical_type(ffi::DUCKDB_TYPE_DUCKDB_TYPE_DATE),
                Sql::Time => ffi::duckdb_create_logical_type(ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIME),
                Sql::Timestamp { nanos: false } => ffi::duckdb_create_logical_type(ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP),
                Sql::Timestamp { nanos: true } => ffi::duckdb_create_logical_type(ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_NS),
                Sql::Enum(words) => {
                    let owned: Vec<CString> =
                        words.iter().map(|w| CString::new(w.as_str()).expect("a choice's word holds no NUL")).collect();
                    let mut names: Vec<*const c_char> = owned.iter().map(|w| w.as_ptr()).collect();
                    ffi::duckdb_create_enum_type(names.as_mut_ptr(), names.len() as u64)
                }
            }
        }
    }
}

/// A column's type in DuckDB, from its Cells type and, for a scale or a unit the type leaves
/// open, from the digits its values hold.
fn sql_of(doc: &Doc, k: usize) -> Result<Sql, String> {
    let c = &doc.columns()[k];
    let scale = || match c.data {
        Data::Decimal { scale, .. } => scale,
        _ => unreachable!("a number with a point is a decimal column"),
    };
    Ok(match &c.ty.base {
        Base::Text | Base::Url | Base::Email | Base::Id => Sql::Varchar,
        Base::Int => Sql::Bigint,
        Base::Money(_) => Sql::Decimal(c.ty.scale().unwrap_or_else(scale)),
        Base::Num | Base::Pct => Sql::Decimal(scale()),
        Base::Bool { .. } => Sql::Boolean,
        Base::Date => Sql::Date,
        Base::Time => Sql::Time,
        Base::DateTime => match c.data {
            Data::DateTime { unit, .. } => Sql::Timestamp { nanos: unit == 9 },
            _ => unreachable!("a datetime column holds instants"),
        },
        Base::Choice(words) => Sql::Enum(words.clone()),
        Base::Ref(targets) => {
            let mut keys = targets.iter().map(|t| {
                let table = doc.tables().iter().find(|x| x.name == t.table).expect("a reference names a table of the file");
                let Some(target) = table.doc.as_deref() else {
                    return Err(format!("{}: references {}, which could not be read", c.name, t.table));
                };
                let key = target.column(&t.column).expect("a reference names a column of its table");
                sql_of(target, key)
            });
            let first = keys.next().expect("a reference names a table")?;
            for other in keys {
                let other = other?;
                if other != first {
                    return Err(format!("{}: references keys of two types, {} and {}", c.name, first.spelled(), other.spelled()));
                }
            }
            match first {
                Sql::Varchar | Sql::Bigint | Sql::Decimal(_) | Sql::Date => first,
                other => {
                    return Err(format!(
                        "{}: references a key of type {}; cells sql reads a reference to text, a number or a date",
                        c.name,
                        other.spelled()
                    ));
                }
            }
        }
    })
}

/// Whether two files' columns of one name hold the same kind of value: the same base, a
/// currency the same currency, a choice the same words, a reference the same tables; a
/// boolean's words are only its spelling.
fn same_kind(a: &Base, b: &Base) -> bool {
    match (a, b) {
        (Base::Bool { .. }, Base::Bool { .. }) => true,
        _ => a == b,
    }
}

/// One file's part of a table: the doc and the path its rows are said to come from.
struct Part {
    file: String,
    doc: Arc<Doc>,
}

/// A column of a table: its type across every part, and where each part holds it.
struct Column {
    name: String,
    sql: Sql,
    /// Each part's column, none where a part has no column of this name.
    at: Vec<Option<usize>>,
}

/// A table as DuckDB reads it: every part of one name, their columns joined by name in the
/// order they first appear, `file` last.
struct Table {
    name: String,
    parts: Vec<Part>,
    columns: Vec<Column>,
}

impl Table {
    fn rows(&self) -> usize {
        self.parts.iter().map(|p| p.doc.rows()).sum()
    }
}

/// The parts of one name as a table, or why their columns cannot be one: a column of two
/// kinds, or one named `file`.
fn table(name: String, parts: Vec<Part>) -> Result<Table, String> {
    let mut columns: Vec<Column> = Vec::new();
    // The part each column's type was first read from, for a clash's message.
    let mut first: Vec<usize> = Vec::new();
    for (p, part) in parts.iter().enumerate() {
        for (k, c) in part.doc.columns().iter().enumerate() {
            if c.name == FILE {
                return Err(format!("{}: has a column named {FILE}, the name cells sql gives the path each row is read from", part.file));
            }
            let sql = sql_of(&part.doc, k).map_err(|e| format!("{} {e}", part.file))?;
            let Some(j) = columns.iter().position(|x| x.name == c.name) else {
                let mut at = vec![None; parts.len()];
                at[p] = Some(k);
                columns.push(Column { name: c.name.clone(), sql, at });
                first.push(p);
                continue;
            };
            let was = &parts[first[j]];
            let was_c = &was.doc.columns()[columns[j].at[first[j]].unwrap()];
            let joined = same_kind(&was_c.ty.base, &c.ty.base).then(|| columns[j].sql.joined(&sql)).flatten();
            let Some(joined) = joined else {
                return Err(format!(
                    "{name}.{}: {} in {} and {} in {}",
                    c.name,
                    was_c.ty.shorthand(),
                    was.file,
                    c.ty.shorthand(),
                    part.file
                ));
            };
            columns[j].sql = joined;
            columns[j].at[p] = Some(k);
        }
    }
    Ok(Table { name, parts, columns })
}

/// A path with its `.` and `..` taken out where they can be: `books/home/../accounts.cells`
/// is `books/accounts.cells`.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir if matches!(out.components().next_back(), Some(Component::Normal(_))) => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// What the paths hold, read: every `.cells` file under them and every file they import
/// from the disk, each once, in the order found, as tables; and how many problems they
/// have, in how many files.
fn read(paths: &[String]) -> Result<(Vec<Table>, usize, usize), String> {
    let mut queue = Vec::new();
    for p in paths {
        cells_disk::files(Path::new(p), &mut queue)?;
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut named: Vec<(String, Vec<Part>)> = Vec::new();
    let mut part = |name: String, p: Part| match named.iter_mut().find(|(n, _)| *n == name) {
        Some((_, parts)) => parts.push(p),
        None => named.push((name, vec![p])),
    };
    let (mut problems, mut files_with) = (0, 0);
    let mut at = 0;
    while at < queue.len() {
        let path = queue[at].clone();
        at += 1;
        let canonical = fs::canonicalize(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if !seen.insert(canonical) {
            continue;
        }
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let shown = path.display().to_string();
        let doc = cells::read_with(bytes, &mut Disk::new(&path), threads).map_err(|e| format!("{shown} {e}"))?;
        // Imports through git's history or from a URL are read for the file importing them,
        // never as tables: a table is a file on the disk as it is now.
        for s in doc.statements() {
            if let Statement::Import { from, with, .. } = s
                && !cells::files::is_url(from)
                && !with.iter().any(|(k, _)| k == "rev")
            {
                queue.push(lexical(&path.parent().unwrap_or(Path::new(".")).join(from)));
            }
        }
        let own: Vec<_> = doc.tables().iter().filter(|t| t.own()).collect();
        let found = doc.broken().len()
            + doc.problems().len()
            + own.iter().map(|t| t.doc.as_ref().expect("a table of the file's own is read").problems().len()).sum::<usize>();
        problems += found;
        files_with += usize::from(found > 0);
        let tables: Vec<(String, Part)> =
            own.iter().map(|t| (t.name.clone(), Part { file: shown.clone(), doc: Arc::clone(t.doc.as_ref().unwrap()) })).collect();
        let stem = path.file_stem().expect("a .cells file has a name").to_string_lossy().into_owned();
        part(stem, Part { file: shown, doc: Arc::new(doc) });
        for (name, p) in tables {
            part(name, p);
        }
    }
    let tables = named.into_iter().map(|(name, parts)| table(name, parts)).collect::<Result<_, _>>()?;
    Ok((tables, problems, files_with))
}

/// One scan of a table: the columns the query reads, by their index in the table's (the
/// one past them `file`), and the next row, a part and a row of it.
struct Scan {
    table: usize,
    columns: Vec<usize>,
    part: usize,
    row: usize,
}

unsafe extern "C" fn free<T>(p: *mut c_void) {
    // SAFETY: DuckDB hands back, once, a pointer `Box::into_raw` gave it.
    drop(unsafe { Box::from_raw(p as *mut T) });
}

/// `cells_scan(k)`'s columns: the `k`th table's, each its type, then `file`.
unsafe extern "C" fn bind(info: ffi::duckdb_bind_info) {
    // SAFETY: the extra info is the tables, alive for as long as the database; the one
    // parameter is the UBIGINT the function declares.
    unsafe {
        let tables = &*(ffi::duckdb_bind_get_extra_info(info) as *const Vec<Table>);
        let mut v = ffi::duckdb_bind_get_parameter(info, 0);
        let k = ffi::duckdb_get_uint64(v) as usize;
        ffi::duckdb_destroy_value(&mut v);
        let t = &tables[k];
        let add = |name: &str, sql: &Sql| {
            let name = CString::new(name).expect("a column's name holds no NUL");
            let mut ty = sql.logical();
            ffi::duckdb_bind_add_result_column(info, name.as_ptr(), ty);
            ffi::duckdb_destroy_logical_type(&mut ty);
        };
        for c in &t.columns {
            add(&c.name, &c.sql);
        }
        add(FILE, &Sql::Varchar);
        ffi::duckdb_bind_set_bind_data(info, Box::into_raw(Box::new(k)) as *mut c_void, Some(free::<usize>));
        ffi::duckdb_bind_set_cardinality(info, t.rows() as u64, true);
    }
}

/// A scan starts at the first row, on one thread, so its rows come in the files' order.
unsafe extern "C" fn init(info: ffi::duckdb_init_info) {
    // SAFETY: the bind data is the table's index `bind` set.
    unsafe {
        let table = *(ffi::duckdb_init_get_bind_data(info) as *const usize);
        let columns = (0..ffi::duckdb_init_get_column_count(info)).map(|i| ffi::duckdb_init_get_column_index(info, i) as usize).collect();
        let scan = Scan { table, columns, part: 0, row: 0 };
        ffi::duckdb_init_set_init_data(info, Box::into_raw(Box::new(scan)) as *mut c_void, Some(free::<Scan>));
        ffi::duckdb_init_set_max_threads(info, 1);
    }
}

/// The next chunk: up to DuckDB's vector size of rows, all of one part.
unsafe extern "C" fn scan(info: ffi::duckdb_function_info, out: ffi::duckdb_data_chunk) {
    // SAFETY: the extra info is the tables, the init data the scan `init` set, used by one
    // thread.
    unsafe {
        let tables = &*(ffi::duckdb_function_get_extra_info(info) as *const Vec<Table>);
        let s = &mut *(ffi::duckdb_function_get_init_data(info) as *mut Scan);
        let t = &tables[s.table];
        while s.part < t.parts.len() && s.row == t.parts[s.part].doc.rows() {
            s.part += 1;
            s.row = 0;
        }
        if s.part == t.parts.len() {
            ffi::duckdb_data_chunk_set_size(out, 0);
            return;
        }
        let part = &t.parts[s.part];
        let n = (part.doc.rows() - s.row).min(ffi::duckdb_vector_size() as usize);
        for (i, &k) in s.columns.iter().enumerate() {
            let v = ffi::duckdb_data_chunk_get_vector(out, i as u64);
            let filled = match t.columns.get(k) {
                None => {
                    assert_eq!(k, t.columns.len(), "DuckDB asks for a column of the table, or file");
                    for r in 0..n {
                        ffi::duckdb_vector_assign_string_element_len(
                            v,
                            r as u64,
                            part.file.as_ptr() as *const c_char,
                            part.file.len() as u64,
                        );
                    }
                    Ok(())
                }
                Some(c) => match c.at[s.part] {
                    Some(j) => fill(v, &c.sql, &part.doc.columns()[j], s.row, n),
                    None => {
                        ffi::duckdb_vector_ensure_validity_writable(v);
                        let validity = ffi::duckdb_vector_get_validity(v);
                        (0..n).for_each(|r| ffi::duckdb_validity_set_row_invalid(validity, r as u64));
                        Ok(())
                    }
                },
            };
            if let Err(e) = filled {
                let e = CString::new(format!("{} row {}: {e}", part.file, s.row + 1)).unwrap();
                ffi::duckdb_function_set_error(info, e.as_ptr());
                return;
            }
        }
        ffi::duckdb_data_chunk_set_size(out, n as u64);
        s.row += n;
    }
}

/// A value written at `row` of a vector's data.
unsafe fn put<T>(data: *mut c_void, row: usize, x: T) {
    // SAFETY: the caller's vector holds values of `T`, `row` within it.
    unsafe { (data as *mut T).add(row).write_unaligned(x) }
}

/// Rows `from..from + n` of a column written into a vector of its type: a value DuckDB
/// holds as the engine does copied, a scale or a unit raised, a reference's text read as its
/// key's type.
unsafe fn fill(v: ffi::duckdb_vector, sql: &Sql, c: &cells::Column, from: usize, n: usize) -> Result<(), String> {
    // SAFETY: the vector is a chunk's, of `sql`'s type and DuckDB's vector size, which `n`
    // never passes.
    unsafe {
        let data = ffi::duckdb_vector_get_data(v);
        let mut validity: *mut u64 = ptr::null_mut();
        let mut null = |r: usize| {
            if validity.is_null() {
                ffi::duckdb_vector_ensure_validity_writable(v);
                validity = ffi::duckdb_vector_get_validity(v);
            }
            ffi::duckdb_validity_set_row_invalid(validity, r as u64);
        };
        let rows = (0..n).map(|r| (r, from + r));
        match (sql, &c.data) {
            (Sql::Varchar, Data::Text { offsets, bytes }) => {
                for (r, at) in rows {
                    if !c.valid.get(at) {
                        null(r);
                        continue;
                    }
                    let s = &bytes[offsets[at] as usize..offsets[at + 1] as usize];
                    ffi::duckdb_vector_assign_string_element_len(v, r as u64, s.as_ptr() as *const c_char, s.len() as u64);
                }
            }
            (Sql::Bigint, Data::Int(values)) => rows.for_each(|(r, at)| if c.valid.get(at) { put(data, r, values[at]) } else { null(r) }),
            (Sql::Decimal(to), Data::Decimal { values, scale }) => {
                let by = value::pow10((to - scale) as u32);
                for (r, at) in rows {
                    if !c.valid.get(at) {
                        null(r);
                        continue;
                    }
                    let m = values[at].checked_mul(by).filter(|m| m.unsigned_abs() < value::pow10(WIDTH as u32) as u128);
                    put(
                        data,
                        r,
                        m.ok_or_else(|| {
                            format!(
                                "{}: {} has more than {WIDTH} digits at {to} after the point",
                                c.name,
                                Decimal { mantissa: values[at], scale: *scale }
                            )
                        })?,
                    );
                }
            }
            (Sql::Boolean, Data::Bool(bits)) => rows.for_each(|(r, at)| if c.valid.get(at) { put(data, r, bits.get(at)) } else { null(r) }),
            (Sql::Date, Data::Date(days)) => rows.for_each(|(r, at)| if c.valid.get(at) { put(data, r, days[at]) } else { null(r) }),
            (Sql::Time, Data::Time(secs)) => {
                rows.for_each(|(r, at)| if c.valid.get(at) { put(data, r, secs[at] as i64 * 1_000_000) } else { null(r) })
            }
            (Sql::Timestamp { nanos }, Data::DateTime { values, unit }) => {
                let by = 10i64.pow((if *nanos { 9 } else { 6 }) - *unit as u32);
                for (r, at) in rows {
                    if !c.valid.get(at) {
                        null(r);
                        continue;
                    }
                    let t = values[at].checked_mul(by);
                    put(
                        data,
                        r,
                        t.ok_or_else(|| {
                            format!(
                                "{}: {} is past what a TIMESTAMP holds",
                                c.name,
                                value::format_datetime(Instant::from(values[at], *unit))
                            )
                        })?,
                    );
                }
            }
            (Sql::Enum(_), Data::Choice(indices)) => {
                let mut ty = ffi::duckdb_vector_get_column_type(v);
                let wide = ffi::duckdb_enum_internal_type(ty) == ffi::DUCKDB_TYPE_DUCKDB_TYPE_USMALLINT;
                ffi::duckdb_destroy_logical_type(&mut ty);
                for (r, at) in rows {
                    match (c.valid.get(at), wide) {
                        (false, _) => null(r),
                        (true, false) => put(data, r, indices[at] as u8),
                        (true, true) => put(data, r, indices[at] as u16),
                    }
                }
            }
            // A reference to a key that is not text: its text read as the key's type. Text
            // that does not read names no key, a problem `cells check` reports, and is null.
            (key, Data::Text { offsets, bytes }) => {
                for (r, at) in rows {
                    let text = &bytes[offsets[at] as usize..offsets[at + 1] as usize];
                    let read = c.valid.get(at).then_some(()).and_then(|()| match key {
                        Sql::Bigint => value::parse_int(text).map(|x| put(data, r, x)),
                        Sql::Decimal(s) => Decimal::parse(text)
                            .ok()
                            .map(Decimal::normal)
                            .filter(|d| d.scale <= *s)
                            .and_then(|d| d.at_scale(*s))
                            .map(|x| put(data, r, x)),
                        Sql::Date => value::parse_date(text).map(|x| put(data, r, x)),
                        _ => unreachable!("a reference is to text, a number or a date"),
                    });
                    if read.is_none() {
                        null(r);
                    }
                }
            }
            (sql, _) => unreachable!("a column of {} holds its type's values", sql.spelled()),
        }
        Ok(())
    }
}

/// An in-memory database and its one connection, closed when dropped.
struct Duck {
    db: ffi::duckdb_database,
    con: ffi::duckdb_connection,
}

impl Drop for Duck {
    fn drop(&mut self) {
        // SAFETY: both opened by `open`, closed once.
        unsafe {
            ffi::duckdb_disconnect(&mut self.con);
            ffi::duckdb_close(&mut self.db);
        }
    }
}

/// A database that fetches nothing: no extension installed or loaded because a query
/// names something one holds.
fn open() -> Result<Duck, String> {
    // SAFETY: the C API's own sequence, every out pointer written before it is read.
    unsafe {
        let mut config: ffi::duckdb_config = ptr::null_mut();
        assert_eq!(ffi::duckdb_create_config(&mut config), ffi::duckdb_state_DuckDBSuccess);
        for (k, v) in [(c"autoinstall_known_extensions", c"false"), (c"autoload_known_extensions", c"false")] {
            assert_eq!(ffi::duckdb_set_config(config, k.as_ptr(), v.as_ptr()), ffi::duckdb_state_DuckDBSuccess, "DuckDB knows {k:?}");
        }
        let (mut db, mut error): (ffi::duckdb_database, *mut c_char) = (ptr::null_mut(), ptr::null_mut());
        let opened = ffi::duckdb_open_ext(ptr::null(), &mut db, config, &mut error);
        ffi::duckdb_destroy_config(&mut config);
        if opened != ffi::duckdb_state_DuckDBSuccess {
            let why = CStr::from_ptr(error).to_string_lossy().into_owned();
            ffi::duckdb_free(error as *mut c_void);
            return Err(format!("DuckDB does not open: {why}"));
        }
        let mut con: ffi::duckdb_connection = ptr::null_mut();
        assert_eq!(ffi::duckdb_connect(db, &mut con), ffi::duckdb_state_DuckDBSuccess, "an in-memory database connects");
        Ok(Duck { db, con })
    }
}

/// A query's result, destroyed when dropped.
struct Answer(ffi::duckdb_result);

impl Drop for Answer {
    fn drop(&mut self) {
        // SAFETY: a result `query` filled, destroyed once.
        unsafe { ffi::duckdb_destroy_result(&mut self.0) }
    }
}

impl Duck {
    fn query(&self, sql: &str) -> Result<Answer, String> {
        let text = CString::new(sql).map_err(|_| "the query holds a NUL".to_string())?;
        // SAFETY: a zeroed result is what `duckdb_query` fills; it is destroyed either way.
        unsafe {
            let mut answer = Answer(std::mem::zeroed());
            if ffi::duckdb_query(self.con, text.as_ptr(), &mut answer.0) != ffi::duckdb_state_DuckDBSuccess {
                return Err(CStr::from_ptr(ffi::duckdb_result_error(&mut answer.0)).to_string_lossy().into_owned());
            }
            Ok(answer)
        }
    }

    /// `cells_scan` registered over the tables, and a view for each, by its name.
    fn tables(&self, tables: &Vec<Table>) -> Result<(), String> {
        // SAFETY: the tables outlive the database (`sql` drops the database first); DuckDB
        // copies the function and the parameter's type.
        unsafe {
            let mut f = ffi::duckdb_create_table_function();
            ffi::duckdb_table_function_set_name(f, c"cells_scan".as_ptr());
            let mut k = ffi::duckdb_create_logical_type(ffi::DUCKDB_TYPE_DUCKDB_TYPE_UBIGINT);
            ffi::duckdb_table_function_add_parameter(f, k);
            ffi::duckdb_destroy_logical_type(&mut k);
            ffi::duckdb_table_function_set_extra_info(f, tables as *const Vec<Table> as *mut c_void, None);
            ffi::duckdb_table_function_set_bind(f, Some(bind));
            ffi::duckdb_table_function_set_init(f, Some(init));
            ffi::duckdb_table_function_set_function(f, Some(scan));
            ffi::duckdb_table_function_supports_projection_pushdown(f, true);
            assert_eq!(ffi::duckdb_register_table_function(self.con, f), ffi::duckdb_state_DuckDBSuccess, "cells_scan registers");
            ffi::duckdb_destroy_table_function(&mut f);
        }
        for (k, t) in tables.iter().enumerate() {
            let name = format!("\"{}\"", t.name.replace('"', "\"\""));
            self.query(&format!("CREATE VIEW {name} AS SELECT * FROM cells_scan({k})"))
                .map_err(|e| format!("the table {}: {e}", t.name))?;
        }
        Ok(())
    }
}

/// How a column of the answer is written: every type a Cells column holds, as `cells show`
/// writes it, and the numbers SQL computes beside them.
enum Shown {
    Bool,
    Int(ffi::DUCKDB_TYPE),
    Float,
    Double,
    Decimal {
        held: ffi::DUCKDB_TYPE,
        scale: u8,
    },
    Varchar,
    Date,
    Time,
    /// The digits after the point of seconds its values count: 0, 3, 6 or 9.
    Instant(u8),
    Enum {
        held: ffi::DUCKDB_TYPE,
        words: Vec<String>,
    },
}

/// What DuckDB calls a type no Cells column holds, for the message saying so.
fn named(id: ffi::DUCKDB_TYPE) -> &'static str {
    match id {
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_INTERVAL => "INTERVAL",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_BLOB => "BLOB",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_LIST => "a LIST",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_STRUCT => "a STRUCT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_MAP => "a MAP",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_UUID => "UUID",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_UNION => "a UNION",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_BIT => "BIT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIME_TZ => "TIME WITH TIME ZONE",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_ARRAY => "an ARRAY",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_BIGNUM => "BIGNUM",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIME_NS => "TIME_NS",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_GEOMETRY => "GEOMETRY",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_VARIANT => "VARIANT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_TZ_NS => "TIMESTAMP_TZ_NS",
        _ => "a type DuckDB did not name to the C API",
    }
}

/// How a column of the answer is written, or why it cannot be.
unsafe fn shown(answer: &mut Answer, k: u64) -> Result<Shown, String> {
    // SAFETY: `k` is a column of the answer; its type is destroyed here.
    unsafe {
        let mut ty = ffi::duckdb_column_logical_type(&mut answer.0, k);
        let id = ffi::duckdb_get_type_id(ty);
        let shown = match id {
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_BOOLEAN => Ok(Shown::Bool),
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_TINYINT..=ffi::DUCKDB_TYPE_DUCKDB_TYPE_UBIGINT
            | ffi::DUCKDB_TYPE_DUCKDB_TYPE_HUGEINT
            | ffi::DUCKDB_TYPE_DUCKDB_TYPE_UHUGEINT => Ok(Shown::Int(id)),
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_FLOAT => Ok(Shown::Float),
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_DOUBLE => Ok(Shown::Double),
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_DECIMAL => {
                Ok(Shown::Decimal { held: ffi::duckdb_decimal_internal_type(ty), scale: ffi::duckdb_decimal_scale(ty) })
            }
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_VARCHAR => Ok(Shown::Varchar),
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_DATE => Ok(Shown::Date),
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIME => Ok(Shown::Time),
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_S => Ok(Shown::Instant(0)),
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_MS => Ok(Shown::Instant(3)),
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP | ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_TZ => Ok(Shown::Instant(6)),
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_NS => Ok(Shown::Instant(9)),
            ffi::DUCKDB_TYPE_DUCKDB_TYPE_ENUM => {
                let words = (0..ffi::duckdb_enum_dictionary_size(ty) as u64)
                    .map(|i| {
                        let w = ffi::duckdb_enum_dictionary_value(ty, i);
                        let word = CStr::from_ptr(w).to_string_lossy().into_owned();
                        ffi::duckdb_free(w as *mut c_void);
                        word
                    })
                    .collect();
                Ok(Shown::Enum { held: ffi::duckdb_enum_internal_type(ty), words })
            }
            id => {
                let name = CStr::from_ptr(ffi::duckdb_column_name(&mut answer.0, k)).to_string_lossy();
                Err(format!("{name} is {}, which no Cells column holds: CAST it to VARCHAR", named(id)))
            }
        };
        ffi::duckdb_destroy_logical_type(&mut ty);
        shown
    }
}

/// The value at `row` of a vector as `cells show` writes a cell.
unsafe fn cell(s: &Shown, data: *const c_void, row: usize) -> String {
    // SAFETY: `data` is a vector of the type `s` was read from, `row` within its chunk.
    unsafe {
        let at = |size: usize| (data as *const u8).add(row * size);
        let int = |held: ffi::DUCKDB_TYPE| -> i128 {
            match held {
                ffi::DUCKDB_TYPE_DUCKDB_TYPE_TINYINT => *(at(1) as *const i8) as i128,
                ffi::DUCKDB_TYPE_DUCKDB_TYPE_SMALLINT => (at(2) as *const i16).read_unaligned() as i128,
                ffi::DUCKDB_TYPE_DUCKDB_TYPE_INTEGER => (at(4) as *const i32).read_unaligned() as i128,
                ffi::DUCKDB_TYPE_DUCKDB_TYPE_BIGINT => (at(8) as *const i64).read_unaligned() as i128,
                ffi::DUCKDB_TYPE_DUCKDB_TYPE_UTINYINT => *at(1) as i128,
                ffi::DUCKDB_TYPE_DUCKDB_TYPE_USMALLINT => (at(2) as *const u16).read_unaligned() as i128,
                ffi::DUCKDB_TYPE_DUCKDB_TYPE_UINTEGER => (at(4) as *const u32).read_unaligned() as i128,
                ffi::DUCKDB_TYPE_DUCKDB_TYPE_UBIGINT => (at(8) as *const u64).read_unaligned() as i128,
                ffi::DUCKDB_TYPE_DUCKDB_TYPE_HUGEINT => (at(16) as *const i128).read_unaligned(),
                _ => unreachable!("an integer DuckDB holds in at most 128 signed bits"),
            }
        };
        match s {
            Shown::Bool => (*at(1) != 0).to_string(),
            Shown::Int(ffi::DUCKDB_TYPE_DUCKDB_TYPE_UHUGEINT) => (at(16) as *const u128).read_unaligned().to_string(),
            Shown::Int(held) => int(*held).to_string(),
            Shown::Float => (at(4) as *const f32).read_unaligned().to_string(),
            Shown::Double => (at(8) as *const f64).read_unaligned().to_string(),
            Shown::Decimal { held, scale } => Decimal { mantissa: int(*held), scale: *scale }.to_string(),
            Shown::Varchar => {
                let mut s = (at(16) as *const ffi::duckdb_string_t).read_unaligned();
                let len = ffi::duckdb_string_t_length(s) as usize;
                let bytes = std::slice::from_raw_parts(ffi::duckdb_string_t_data(&mut s) as *const u8, len);
                String::from_utf8_lossy(bytes).into_owned()
            }
            Shown::Date => value::format_date((at(4) as *const i32).read_unaligned()),
            Shown::Time => {
                let micros = (at(8) as *const i64).read_unaligned();
                let mut t = value::format_time((micros / 1_000_000) as i32);
                if micros % 1_000_000 != 0 {
                    t.push_str(format!(".{:06}", micros % 1_000_000).trim_end_matches('0'));
                }
                t
            }
            Shown::Instant(unit) => value::format_datetime(Instant::from((at(8) as *const i64).read_unaligned(), *unit)),
            Shown::Enum { held, words } => words[int(*held) as usize].clone(),
        }
    }
}

/// The answer written to `out` as `cells show` writes a table: a header of the columns'
/// names, then a line a row, tab-separated, a cell quoted only where the tab, a quote or a
/// line break would break it, a null an empty cell.
fn write(mut answer: Answer, out: &mut impl std::io::Write) -> Result<(), String> {
    let tsv = Dialect::Plain(Separator::Tab);
    // SAFETY: columns and chunks of the answer, each chunk destroyed after it is written.
    unsafe {
        let width = ffi::duckdb_column_count(&mut answer.0);
        if width == 0 {
            return Ok(());
        }
        let names: Vec<String> = (0..width)
            .map(|k| tsv.quote(&CStr::from_ptr(ffi::duckdb_column_name(&mut answer.0, k)).to_string_lossy()).into_owned())
            .collect();
        let shown: Vec<Shown> = (0..width).map(|k| shown(&mut answer, k)).collect::<Result<_, _>>()?;
        let io = |e: std::io::Error| format!("stdout: {e}");
        writeln!(out, "{}", names.join("\t")).map_err(io)?;
        loop {
            let mut chunk = ffi::duckdb_fetch_chunk(answer.0);
            if chunk.is_null() {
                return Ok(());
            }
            let vectors: Vec<(*const c_void, *mut u64)> = (0..width)
                .map(|k| {
                    let v = ffi::duckdb_data_chunk_get_vector(chunk, k);
                    (ffi::duckdb_vector_get_data(v) as *const c_void, ffi::duckdb_vector_get_validity(v))
                })
                .collect();
            let mut line = String::new();
            for row in 0..ffi::duckdb_data_chunk_get_size(chunk) as usize {
                line.clear();
                for (k, (s, &(data, validity))) in shown.iter().zip(&vectors).enumerate() {
                    if k > 0 {
                        line.push('\t');
                    }
                    if validity.is_null() || ffi::duckdb_validity_row_is_valid(validity, row as u64) {
                        line.push_str(&tsv.quote(&cell(s, data, row)));
                    }
                }
                writeln!(out, "{line}").map_err(io)?;
            }
            ffi::duckdb_destroy_data_chunk(&mut chunk);
        }
    }
}

/// What a query over the paths found: its answer written to stdout, and how many problems
/// the files it read have, in how many files.
pub fn sql(paths: &[String], query: &str) -> Result<(usize, usize), String> {
    let (tables, problems, files) = read(paths)?;
    let duck = open()?;
    duck.tables(&tables)?;
    let answer = duck.query(query)?;
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    write(answer, &mut out)?;
    std::io::Write::flush(&mut out).map_err(|e| format!("stdout: {e}"))?;
    drop(duck);
    Ok((problems, files))
}
