//! Columns as their types want them, in Arrow's layout: whole numbers `i64`, numbers and
//! money exact `i128` at one scale per column (`Decimal128(38, scale)`), dates as days
//! (`Date32`), times as seconds (`Time32[s]`), instants in UTC at one unit per column
//! (`Timestamp[s|ms|us|ns, UTC]`), booleans as bits, choices as `i16` indices into the
//! type's words (a dictionary), text as `i32` offsets into one byte buffer, and a
//! validity bitmap beside every column. Buffers are shared (`Arc`), so an export hands
//! them out without a copy and an edit copies only what another holder still reads.

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::types::{Base, Bound, Format, Type};
use crate::value::{self, Decimal, Instant, NumberError};

/// Bits, least significant first, as Arrow's validity and boolean buffers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bits {
    pub bytes: Arc<Vec<u8>>,
    pub len: usize,
}

impl Bits {
    #[inline]
    pub fn get(&self, i: usize) -> bool {
        debug_assert!(i < self.len);
        self.bytes[i >> 3] >> (i & 7) & 1 == 1
    }

    pub fn set(&mut self, i: usize, v: bool) {
        assert!(i < self.len);
        let b = &mut Arc::make_mut(&mut self.bytes)[i >> 3];
        if v {
            *b |= 1 << (i & 7);
        } else {
            *b &= !(1 << (i & 7));
        }
    }

    /// How many are clear.
    pub fn zeros(&self) -> usize {
        let whole = self.len / 8;
        let mut ones: usize = self.bytes[..whole].iter().map(|b| b.count_ones() as usize).sum();
        if !self.len.is_multiple_of(8) {
            ones += (self.bytes[whole] & ((1u8 << (self.len % 8)) - 1)).count_ones() as usize;
        }
        self.len - ones
    }
}

/// Bits being written, one at a time or appended from another run.
#[derive(Clone, Debug, Default)]
pub struct BitsBuilder {
    pub bytes: Vec<u8>,
    pub len: usize,
}

impl BitsBuilder {
    pub fn with_capacity(n: usize) -> BitsBuilder {
        BitsBuilder { bytes: Vec::with_capacity(n.div_ceil(8)), len: 0 }
    }

    #[inline]
    pub fn push(&mut self, v: bool) {
        if self.len & 7 == 0 {
            self.bytes.push(0);
        }
        // Pushed above when the run reached a byte's edge.
        *self.bytes.last_mut().unwrap() |= (v as u8) << (self.len & 7);
        self.len += 1;
    }

    #[inline]
    pub fn get(&self, i: usize) -> bool {
        debug_assert!(i < self.len);
        self.bytes[i >> 3] >> (i & 7) & 1 == 1
    }

    pub fn clear(&mut self, i: usize) {
        debug_assert!(i < self.len);
        self.bytes[i >> 3] &= !(1 << (i & 7));
    }

    pub fn append(&mut self, other: &BitsBuilder) {
        if self.len & 7 == 0 {
            self.bytes.extend_from_slice(&other.bytes);
            self.len += other.len;
            return;
        }
        let shift = self.len & 7;
        for (k, &b) in other.bytes.iter().enumerate() {
            let left = other.len - k * 8;
            // The free high bits of the last byte take this byte's low bits.
            *self.bytes.last_mut().unwrap() |= b << shift;
            if left > 8 - shift {
                self.bytes.push(b >> (8 - shift));
            }
        }
        self.len += other.len;
        self.bytes.truncate(self.len.div_ceil(8));
        if !self.len.is_multiple_of(8) {
            let last = self.bytes.last_mut().unwrap();
            *last &= (1u8 << (self.len % 8)) - 1;
        }
    }

    pub fn finish(self) -> Bits {
        Bits { bytes: Arc::new(self.bytes), len: self.len }
    }
}

/// A column's values.
#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    /// `text`, `url`, `email`, `id`: `offsets[i]..offsets[i + 1]` of `bytes`, UTF-8.
    Text {
        offsets: Arc<Vec<i32>>,
        bytes: Arc<Vec<u8>>,
    },
    Int(Arc<Vec<i64>>),
    /// `num`, `pct`, money, a computed column: `values[i] / 10^scale`.
    Decimal {
        values: Arc<Vec<i128>>,
        scale: u8,
    },
    Bool(Bits),
    /// Days since 1970-01-01.
    Date(Arc<Vec<i32>>),
    /// Seconds since midnight.
    Time(Arc<Vec<i32>>),
    /// Units of `10^-unit` seconds since 1970-01-01T00:00:00Z; the unit is 0, 3, 6 or 9.
    DateTime {
        values: Arc<Vec<i64>>,
        unit: u8,
    },
    /// Indices into the choice's words.
    Choice(Arc<Vec<i16>>),
}

/// One cell's value, borrowed from its column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value<'a> {
    Null,
    Text(&'a str),
    Int(i64),
    Decimal(Decimal),
    Bool(bool),
    Date(i32),
    Time(i32),
    DateTime(Instant),
    Choice(&'a str),
}

impl Data {
    pub fn len(&self) -> usize {
        match self {
            Data::Text { offsets, .. } => offsets.len() - 1,
            Data::Int(v) => v.len(),
            Data::Decimal { values, .. } => values.len(),
            Data::Bool(b) => b.len,
            Data::Date(v) | Data::Time(v) => v.len(),
            Data::DateTime { values, .. } => values.len(),
            Data::Choice(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Why a cell is a problem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Why {
    /// Blank where the type has no `?`.
    Missing,
    NotInt,
    NotNumber,
    NotPct,
    NotBool,
    NotChoice,
    NotDate,
    NotTime,
    NotDateTime,
    NotUrl,
    NotEmail,
    NotId,
    /// A number past 38 digits at its column's scale, a whole number past `i64`, an
    /// instant past `i64` at its column's unit.
    OutOfRange,
    /// An amount with more digits after the point than its column's scale.
    PastScale,
    /// Under the type's lower bound (a value, or a text's length).
    Below,
    /// Over its upper bound.
    Above,
    /// A control character in the cell, by its code point (`control`), whatever the
    /// column's type: an inferred column's too.
    Control {
        point: u32,
    },
    /// The key of another row too.
    Duplicate {
        other: u32,
    },
    /// A plain file's record of another width than its header: `fields` fields. On the
    /// first cell it lacks, or the last cell of one too long.
    Ragged {
        fields: u32,
    },
    /// A formula's cell: while a file is read, one its column's formula computes
    /// (`Computed`) or one holding its own (`Formula`), both settled once every column
    /// is; after it, a cell formula that does not read, names what is not there or
    /// reads itself (`Formula`, its words the doc's).
    Computed,
    Formula,
    /// A reference whose key is in none of the tables it names (`Absent`), or in two
    /// (`Ambiguous`).
    Absent,
    Ambiguous,
    /// A computed cell that stores another value than its formula gives (SPEC.md,
    /// Reading): its value is what the formula gives, its stored text kept as written.
    Stale,
}

/// What a problem is: the suite's kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Type,
    Missing,
    Key,
    /// A plain file's record of another width than its header.
    Ragged,
    /// A cell's own formula that does not compute.
    Formula,
    /// A reference to no row, or to rows of two tables.
    Reference,
    /// A computed cell storing another value than its formula gives.
    Stale,
}

/// How much a problem weighs, the same in every reader (the command line, the language
/// server, an app): what is wrong, or what is not there yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// A value that is wrong: it breaks its type, repeats a key, names no row, or its
    /// formula does not compute. A grammar that breaks is one too.
    Error,
    /// Something absent or behind: a value not written, a stored value its formula has
    /// moved past, a plain row of another width.
    Warning,
}

impl Kind {
    pub fn severity(self) -> Severity {
        match self {
            Kind::Type | Kind::Key | Kind::Formula | Kind::Reference => Severity::Error,
            Kind::Missing | Kind::Stale | Kind::Ragged => Severity::Warning,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Type => "type",
            Kind::Missing => "missing",
            Kind::Key => "key",
            Kind::Ragged => "ragged",
            Kind::Formula => "formula",
            Kind::Reference => "reference",
            Kind::Stale => "stale",
        }
    }
}

impl Why {
    pub fn kind(self) -> Kind {
        match self {
            Why::Missing => Kind::Missing,
            Why::Duplicate { .. } => Kind::Key,
            Why::Ragged { .. } => Kind::Ragged,
            Why::Formula | Why::Computed => Kind::Formula,
            Why::Absent | Why::Ambiguous => Kind::Reference,
            Why::Stale => Kind::Stale,
            _ => Kind::Type,
        }
    }
}

/// A cell that breaks its type, by place: the row from 0 and the column's index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Problem {
    pub row: u32,
    pub column: u32,
    pub why: Why,
}

impl Problem {
    pub fn kind(&self) -> Kind {
        self.why.kind()
    }

    pub(crate) fn order(&self) -> (u32, u32) {
        (self.row, self.column)
    }
}

/// How a column decodes its cells, decided once from its type.
#[derive(Clone, Debug)]
pub(crate) enum Decoder {
    Text {
        min: Option<Bound>,
        max: Option<Bound>,
    },
    Url,
    Email,
    Id,
    Int {
        min: Option<Bound>,
        max: Option<Bound>,
    },
    Num {
        min: Option<Bound>,
        max: Option<Bound>,
        /// An amount's scale: the most digits after the point a written value holds.
        scale: Option<u8>,
    },
    Pct {
        min: Option<Bound>,
        max: Option<Bound>,
    },
    Bool {
        yes: Vec<u8>,
        no: Vec<u8>,
    },
    Choice(Vec<Vec<u8>>),
    Date(Option<Format>),
    Time(Option<Format>),
    DateTime,
    /// No type written: kept as text, the type read once every value is seen.
    Untyped,
}

impl Decoder {
    pub fn of(ty: Option<&Type>) -> Decoder {
        let Some(ty) = ty else { return Decoder::Untyped };
        let (min, max) = (ty.min, ty.max);
        match &ty.base {
            // A reference holds its key's text, checked against the tables once read.
            Base::Text | Base::Ref(_) => Decoder::Text { min, max },
            Base::Url => Decoder::Url,
            Base::Email => Decoder::Email,
            Base::Id => Decoder::Id,
            Base::Int => Decoder::Int { min, max },
            Base::Num => Decoder::Num { min, max, scale: None },
            Base::Money(_) => Decoder::Num { min, max, scale: ty.scale() },
            Base::Pct => Decoder::Pct { min, max },
            Base::Bool { yes, no } => Decoder::Bool { yes: yes.as_bytes().into(), no: no.as_bytes().into() },
            Base::Choice(words) => Decoder::Choice(words.iter().map(|w| w.as_bytes().into()).collect()),
            Base::Date => Decoder::Date(ty.format.clone()),
            Base::Time => Decoder::Time(ty.format.clone()),
            Base::DateTime => Decoder::DateTime,
        }
    }
}

/// Values being decoded.
#[derive(Clone, Debug)]
pub(crate) enum Building {
    Text { offsets: Vec<i32>, bytes: Vec<u8> },
    Int(Vec<i64>),
    Decimal { values: Vec<i128>, scale: u8 },
    Bool(BitsBuilder),
    Date(Vec<i32>),
    Time(Vec<i32>),
    DateTime { values: Vec<i64>, unit: u8 },
    Choice(Vec<i16>),
}

impl Building {
    pub fn new(d: &Decoder, rows: usize) -> Building {
        match d {
            Decoder::Text { .. } | Decoder::Url | Decoder::Email | Decoder::Id | Decoder::Untyped => {
                let mut offsets = Vec::with_capacity(rows + 1);
                offsets.push(0);
                Building::Text { offsets, bytes: Vec::new() }
            }
            Decoder::Int { .. } => Building::Int(Vec::with_capacity(rows)),
            Decoder::Num { .. } | Decoder::Pct { .. } => Building::Decimal { values: Vec::with_capacity(rows), scale: 0 },
            Decoder::Bool { .. } => Building::Bool(BitsBuilder::with_capacity(rows)),
            Decoder::Choice(_) => Building::Choice(Vec::with_capacity(rows)),
            Decoder::Date(_) => Building::Date(Vec::with_capacity(rows)),
            Decoder::Time(_) => Building::Time(Vec::with_capacity(rows)),
            Decoder::DateTime => Building::DateTime { values: Vec::with_capacity(rows), unit: 0 },
        }
    }

    /// Room for `rows` more values; a text column also for their text, at the length its
    /// `read` values so far run.
    pub fn reserve(&mut self, rows: usize, read: usize) {
        match self {
            Building::Text { offsets, bytes } => {
                offsets.reserve(rows);
                bytes.reserve(rows * bytes.len() / read.max(1));
            }
            Building::Int(v) => v.reserve(rows),
            Building::Decimal { values, .. } => values.reserve(rows),
            Building::Bool(b) => b.bytes.reserve(rows / 8 + 1),
            Building::Date(v) | Building::Time(v) => v.reserve(rows),
            Building::DateTime { values, .. } => values.reserve(rows),
            Building::Choice(v) => v.reserve(rows),
        }
    }

    pub fn finish(self) -> Data {
        match self {
            Building::Text { offsets, bytes } => Data::Text { offsets: Arc::new(offsets), bytes: Arc::new(bytes) },
            Building::Int(v) => Data::Int(Arc::new(v)),
            Building::Decimal { values, scale } => Data::Decimal { values: Arc::new(values), scale },
            Building::Bool(b) => Data::Bool(b.finish()),
            Building::Date(v) => Data::Date(Arc::new(v)),
            Building::Time(v) => Data::Time(Arc::new(v)),
            Building::DateTime { values, unit } => Data::DateTime { values: Arc::new(values), unit },
            Building::Choice(v) => Data::Choice(Arc::new(v)),
        }
    }
}

/// The placeholder under a null: the cell's validity bit says it holds nothing.
#[inline]
pub(crate) fn push_null(b: &mut Building) {
    match b {
        Building::Text { offsets, bytes } => offsets.push(bytes.len() as i32),
        Building::Int(v) => v.push(0),
        Building::Decimal { values, .. } => values.push(0),
        Building::Bool(bits) => bits.push(false),
        Building::Date(v) | Building::Time(v) => v.push(0),
        Building::DateTime { values, .. } => values.push(0),
        Building::Choice(v) => v.push(0),
    }
}

/// A date or time read by its format, as (days, seconds); each part the format lacks is
/// nothing (days) or zero (seconds).
pub(crate) fn by_format(s: &[u8], f: &Format) -> Option<(Option<i32>, i32)> {
    use crate::types::Piece;
    let (mut y, mut mo, mut d, mut h, mut mi, mut sec) = (None, None, None, 0, 0, 0);
    let mut i = 0;
    for p in &f.pieces {
        let width = match p {
            Piece::Year => 4,
            Piece::Literal(_) => 1,
            _ => 2,
        };
        let part = s.get(i..i + width)?;
        if let Piece::Literal(c) = p {
            if part[0] != *c {
                return None;
            }
        } else {
            let mut v = 0u32;
            for &c in part {
                let k = c.wrapping_sub(b'0');
                if k > 9 {
                    return None;
                }
                v = v * 10 + k as u32;
            }
            match p {
                Piece::Year => y = Some(v),
                Piece::Month => mo = Some(v),
                Piece::Day => d = Some(v),
                Piece::Hour => h = v,
                Piece::Minute => mi = v,
                _ => sec = v,
            }
        }
        i += width;
    }
    if i != s.len() {
        return None;
    }
    let days = match (y, mo, d) {
        (Some(y), Some(m), Some(d)) => Some(value::date_days(y, m, d)?),
        _ => None,
    };
    Some((days, value::time_seconds(h, mi, sec)?))
}

/// Where a decoded run's earlier values live, for a scale raised under them: the run's
/// validity, the problems, the column, and the row of the run's first value.
pub(crate) struct Run<'a> {
    pub valid: &'a mut BitsBuilder,
    pub problems: &'a mut Vec<Problem>,
    pub column: u32,
    pub first: u32,
}

/// Raises a run of scaled values (decimals at a scale, instants at a unit) by `10^by`; a
/// value that would pass `limit` becomes a null and a problem.
pub(crate) fn raise<T>(values: &mut [T], by: u32, limit: u128, run: &mut Run)
where
    T: Copy + TryInto<i128> + TryFrom<i128>,
{
    let k = value::pow10(by);
    let at = run.valid.len - values.len();
    for (i, v) in values.iter_mut().enumerate() {
        let x: i128 = (*v).try_into().ok().unwrap();
        let raised = x.checked_mul(k).filter(|r| r.unsigned_abs() < limit).and_then(|r| T::try_from(r).ok());
        match raised {
            Some(r) => *v = r,
            None => {
                *v = T::try_from(0).ok().unwrap();
                if run.valid.get(at + i) {
                    run.valid.clear(at + i);
                    run.problems.push(Problem { row: run.first + i as u32, column: run.column, why: Why::OutOfRange });
                }
            }
        }
    }
}

/// Past this, a decimal has more than 38 digits.
pub(crate) const DECIMAL_LIMIT: u128 = 100_000_000_000_000_000_000_000_000_000_000_000_000;
/// Past this, an instant is past `i64`.
pub(crate) const INSTANT_LIMIT: u128 = i64::MAX as u128 + 1;

/// Appends a cell's text to a text column. Cells are short, so up to 32 bytes go as two
/// overlapping word copies rather than a call to `memcpy`.
#[inline(always)]
pub(crate) fn push_text(offsets: &mut Vec<i32>, bytes: &mut Vec<u8>, t: &[u8]) {
    let n = t.len();
    let at = bytes.len();
    bytes.reserve(n);
    // SAFETY: `reserve` made room for `n` bytes after `at`; every read is inside `t`.
    unsafe {
        let src = t.as_ptr();
        let dst = bytes.as_mut_ptr().add(at);
        if (16..=32).contains(&n) {
            let a = (src as *const u128).read_unaligned();
            let b = (src.add(n - 16) as *const u128).read_unaligned();
            (dst as *mut u128).write_unaligned(a);
            (dst.add(n - 16) as *mut u128).write_unaligned(b);
        } else if (8..16).contains(&n) {
            let a = (src as *const u64).read_unaligned();
            let b = (src.add(n - 8) as *const u64).read_unaligned();
            (dst as *mut u64).write_unaligned(a);
            (dst.add(n - 8) as *mut u64).write_unaligned(b);
        } else if (4..8).contains(&n) {
            let a = (src as *const u32).read_unaligned();
            let b = (src.add(n - 4) as *const u32).read_unaligned();
            (dst as *mut u32).write_unaligned(a);
            (dst.add(n - 4) as *mut u32).write_unaligned(b);
        } else if n < 4 {
            for k in 0..n {
                *dst.add(k) = *src.add(k);
            }
        } else {
            core::ptr::copy_nonoverlapping(src, dst, n);
        }
        bytes.set_len(at + n);
    }
    offsets.push(bytes.len() as i32);
}

/// Whether a byte begins a control character, `next` the byte after it: a C0 byte but
/// tab, `\n` and `\r`, DEL, or the 0xC2 of a C1 (0xC2 0x80..=0x9F).
#[inline(always)]
fn begins_control(c: u8, next: u8) -> bool {
    ((c < 0x20) & (c != b'\t') & (c != b'\n') & (c != b'\r')) | (c == 0x7F) | ((c == 0xC2) & (next & 0xE0 == 0x80))
}

/// The code point of the first control character in a text (Unicode's Cc but tab, `\n`
/// and `\r`), which no cell, header cell or prelude line holds.
#[inline]
pub(crate) fn control(t: &[u8]) -> Option<u32> {
    control_at(t).map(|(_, point)| point)
}

/// `control`, with where the character starts in the text.
pub(crate) fn control_at(t: &[u8]) -> Option<(usize, u32)> {
    let k = (0..t.len()).find(|&k| begins_control(t[k], t.get(k + 1).copied().unwrap_or(0)))?;
    // A C1's code point is its second byte; C0 and DEL are their byte.
    Some((k, if t[k] == 0xC2 { t[k + 1] as u32 } else { t[k] as u32 }))
}

/// Decodes one non-blank cell's text into `b`, or says why it cannot be. A refused cell
/// pushes nothing; the caller pushes the null in its place. A number with more digits
/// after the point than its column's scale raises the scale over the run first.
#[inline(always)]
pub(crate) fn decode(d: &Decoder, b: &mut Building, text: &[u8], run: &mut Run) -> Result<(), Why> {
    match (d, b) {
        (Decoder::Text { min, max }, Building::Text { offsets, bytes }) => {
            if min.is_some() || max.is_some() {
                // Code points: every byte that does not continue a UTF-8 sequence.
                let n = text.iter().filter(|&&c| c & 0xC0 != 0x80).count() as i128;
                bounded(Decimal { mantissa: n, scale: 0 }, min, max)?;
            }
            push_text(offsets, bytes, text);
        }
        (Decoder::Url | Decoder::Email | Decoder::Id | Decoder::Untyped, Building::Text { offsets, bytes }) => {
            match d {
                Decoder::Url if !value::is_url(text) => return Err(Why::NotUrl),
                Decoder::Email if !value::is_email(text) => return Err(Why::NotEmail),
                Decoder::Id if !value::is_id(text) => return Err(Why::NotId),
                _ => {}
            }
            push_text(offsets, bytes, text);
        }
        (Decoder::Int { min, max }, Building::Int(v)) => match value::parse_int(text) {
            Some(n) => {
                if min.is_some() || max.is_some() {
                    bounded(Decimal { mantissa: n as i128, scale: 0 }, min, max)?;
                }
                v.push(n)
            }
            None if is_digits(text) => return Err(Why::OutOfRange),
            None => return Err(Why::NotInt),
        },
        (Decoder::Num { min, max, .. } | Decoder::Pct { min, max }, Building::Decimal { values, scale }) => {
            let pct = matches!(d, Decoder::Pct { .. });
            let bare = if pct { text.strip_suffix(b"%").unwrap_or(text) } else { text };
            let x = Decimal::parse(bare).map_err(|e| match e {
                NumberError::Syntax if pct => Why::NotPct,
                NumberError::Syntax => Why::NotNumber,
                NumberError::Range => Why::OutOfRange,
            })?;
            // An amount holds no more digits after the point than its column's scale: a
            // zero that changes nothing is no digit.
            if let Decoder::Num { scale: Some(most), .. } = d
                && x.normal().scale > *most
            {
                return Err(Why::PastScale);
            }
            bounded(x, min, max)?;
            if x.scale > *scale {
                raise(values, (x.scale - *scale) as u32, DECIMAL_LIMIT, run);
                *scale = x.scale;
            }
            values.push(x.at_scale(*scale).ok_or(Why::OutOfRange)?);
        }
        (Decoder::Bool { yes, no }, Building::Bool(bits)) => {
            if same(text, yes) {
                bits.push(true);
            } else if same(text, no) {
                bits.push(false);
            } else {
                return Err(Why::NotBool);
            }
        }
        (Decoder::Choice(words), Building::Choice(v)) => {
            let Some(k) = words.iter().position(|w| same(text, w)) else {
                return Err(Why::NotChoice);
            };
            v.push(k as i16);
        }
        (Decoder::Date(f), Building::Date(v)) => {
            let days = match f {
                None => value::parse_date(text),
                Some(f) => by_format(text, f).and_then(|(d, _)| d),
            };
            v.push(days.ok_or(Why::NotDate)?);
        }
        (Decoder::Time(f), Building::Time(v)) => {
            let secs = match f {
                None => value::parse_time(text),
                Some(f) => by_format(text, f).map(|(_, t)| t),
            };
            v.push(secs.ok_or(Why::NotTime)?);
        }
        (Decoder::DateTime, Building::DateTime { values, unit }) => {
            let t = value::parse_datetime(text).ok_or(Why::NotDateTime)?;
            let need = t.digits().div_ceil(3) * 3;
            if need > *unit {
                raise(values, (need - *unit) as u32, INSTANT_LIMIT, run);
                *unit = need;
            }
            values.push(t.at(*unit).ok_or(Why::OutOfRange)?);
        }
        _ => unreachable!("a decoder builds its own kind"),
    }
    Ok(())
}

/// Two short words equal: their lengths, then their bytes as two overlapping words.
#[inline(always)]
pub(crate) fn same(a: &[u8], b: &[u8]) -> bool {
    let n = a.len();
    if n != b.len() {
        return false;
    }
    // SAFETY: every read is inside `a` or `b`, both `n` bytes long.
    unsafe {
        let (p, q) = (a.as_ptr(), b.as_ptr());
        if (8..=16).contains(&n) {
            let x = (p as *const u64).read_unaligned() ^ (q as *const u64).read_unaligned();
            let y = (p.add(n - 8) as *const u64).read_unaligned() ^ (q.add(n - 8) as *const u64).read_unaligned();
            x | y == 0
        } else if (4..8).contains(&n) {
            let x = (p as *const u32).read_unaligned() ^ (q as *const u32).read_unaligned();
            let y = (p.add(n - 4) as *const u32).read_unaligned() ^ (q.add(n - 4) as *const u32).read_unaligned();
            x | y == 0
        } else if n < 4 {
            (0..n).all(|k| *p.add(k) == *q.add(k))
        } else {
            a == b
        }
    }
}

#[inline]
pub(crate) fn bounded(x: Decimal, min: &Option<Bound>, max: &Option<Bound>) -> Result<(), Why> {
    if let Some(b) = min {
        let o = x.cmp(&b.value);
        if o.is_lt() || (o.is_eq() && !b.inclusive) {
            return Err(Why::Below);
        }
    }
    if let Some(b) = max {
        let o = x.cmp(&b.value);
        if o.is_gt() || (o.is_eq() && !b.inclusive) {
            return Err(Why::Above);
        }
    }
    Ok(())
}

fn is_digits(s: &[u8]) -> bool {
    let d = s.strip_prefix(b"-").or_else(|| s.strip_prefix(b"+")).unwrap_or(s);
    !d.is_empty() && d.iter().all(u8::is_ascii_digit)
}

/// What every non-blank value of an untyped column could be, narrowed value by value.
pub(crate) mod infer {
    use crate::value;

    pub const INT: u8 = 1;
    pub const NUM: u8 = 2;
    pub const TRUE_FALSE: u8 = 4;
    pub const YES_NO: u8 = 8;
    pub const DATE: u8 = 16;
    pub const DATETIME: u8 = 32;
    pub const ALL: u8 = 63;

    #[inline]
    pub fn narrow(mut c: u8, s: &[u8]) -> u8 {
        if c & INT != 0 && value::parse_int(s).is_none() {
            c &= !INT;
        }
        if c & NUM != 0 && value::Decimal::parse(s).is_err() {
            c &= !NUM;
        }
        if c & TRUE_FALSE != 0 && s != b"true" && s != b"false" {
            c &= !TRUE_FALSE;
        }
        if c & YES_NO != 0 && s != b"yes" && s != b"no" {
            c &= !YES_NO;
        }
        if c & DATE != 0 && value::parse_date(s).is_none() {
            c &= !DATE;
        }
        if c & DATETIME != 0 && value::parse_datetime(s).is_none() {
            c &= !DATETIME;
        }
        c
    }
}
