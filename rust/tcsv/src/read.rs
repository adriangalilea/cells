//! The body read into columns. Records are scanned into batches of field spans, and each
//! batch is decoded column by column, so one type's loop runs over a run of its cells. A
//! large input is split at record boundaries and its parts read on threads.

use alloc::vec::Vec;

use crate::column::{self, BitsBuilder, Building, DECIMAL_LIMIT, Decoder, INSTANT_LIMIT, Problem, Run, Why, infer};
use crate::scan::{self, Fields, Stop};

/// Rows decoded together.
const BATCH: usize = 4096;

/// One column while its cells are read.
pub(crate) struct Reading {
    pub decoder: Decoder,
    pub optional: bool,
    pub building: Building,
    pub valid: BitsBuilder,
    /// An untyped column's candidates (`infer`).
    pub candidates: u8,
}

/// A part of the body read: its records and columns and the problems in them.
pub(crate) struct Part<'a> {
    src: &'a [u8],
    ncols: usize,
    cells: Vec<(u32, u32)>,
    batched: usize,
    field: usize,
    start: u32,
    pub records: Vec<u32>,
    pub columns: Vec<Reading>,
    pub problems: Vec<Problem>,
    pub rows: u32,
    scratch: Vec<u8>,
}

impl<'a> Part<'a> {
    pub fn new(src: &'a [u8], from: usize, decoders: &[(Decoder, bool)], rows_hint: usize) -> Part<'a> {
        let ncols = decoders.len();
        Part {
            src,
            ncols,
            cells: alloc::vec![(0, 0); ncols.max(1) * BATCH],
            batched: 0,
            field: 0,
            start: from as u32,
            records: Vec::with_capacity(rows_hint + 1),
            columns: decoders
                .iter()
                .map(|(d, optional)| Reading {
                    decoder: d.clone(),
                    optional: *optional,
                    building: Building::new(d, rows_hint),
                    valid: BitsBuilder::with_capacity(rows_hint),
                    candidates: infer::ALL,
                })
                .collect(),
            problems: Vec::new(),
            rows: 0,
            scratch: Vec::new(),
        }
    }

    /// Decodes the batched rows.
    fn flush(&mut self) {
        let n = self.batched;
        let base = self.rows - n as u32;
        for (c, col) in self.columns.iter_mut().enumerate() {
            let cells = &self.cells[c * BATCH..c * BATCH + n];
            decode_run(self.src, cells, col, c as u32, base, &mut self.problems, &mut self.scratch);
        }
        self.batched = 0;
    }

    pub fn finish(&mut self) {
        if self.batched > 0 {
            self.flush();
        }
    }
}

/// Decodes a run of one column's cells, rows `base..`.
fn decode_run(src: &[u8], cells: &[(u32, u32)], col: &mut Reading, c: u32, base: u32, problems: &mut Vec<Problem>, scratch: &mut Vec<u8>) {
    let untyped = matches!(col.decoder, Decoder::Untyped);
    for (k, &(s, e)) in cells.iter().enumerate() {
        let row = base + k as u32;
        let text = scan::text(src, s, e, scratch);
        if text.is_empty() {
            column::push_null(&mut col.building);
            col.valid.push(false);
            if !col.optional && !untyped {
                problems.push(Problem { row, column: c, why: Why::Missing });
            }
            continue;
        }
        if untyped {
            col.candidates = infer::narrow(col.candidates, text);
        }
        // A part's rows count from 0, so its values start at row 0.
        let mut run = Run { valid: &mut col.valid, problems, column: c, first: 0 };
        match column::decode(&col.decoder, &mut col.building, text, &mut run) {
            Ok(()) => col.valid.push(true),
            Err(why) => {
                column::push_null(&mut col.building);
                col.valid.push(false);
                problems.push(Problem { row, column: c, why });
            }
        }
    }
}

impl Fields for Part<'_> {
    #[inline]
    fn field(&mut self, start: u32, end: u32) -> Result<(), Stop> {
        if self.field >= self.ncols {
            return Err(Stop::TooMany);
        }
        self.cells[self.field * BATCH + self.batched] = (start, end);
        self.field += 1;
        Ok(())
    }

    #[inline]
    fn record(&mut self, next: u32) {
        for f in self.field..self.ncols {
            self.cells[f * BATCH + self.batched] = (0, 0);
        }
        self.records.push(self.start);
        self.start = next;
        self.field = 0;
        self.batched += 1;
        self.rows += 1;
        if self.batched == BATCH {
            self.flush();
        }
    }

    fn open(&self) -> bool {
        self.field > 0
    }
}

/// The body's records read into parts: `Err` names the row (from 0) and why.
pub(crate) fn body<'a>(src: &'a [u8], from: usize, decoders: &[(Decoder, bool)]) -> Result<Vec<Part<'a>>, (u32, Stop)> {
    #[cfg(feature = "std")]
    if let Some(parts) = parallel(src, from, decoders) {
        return parts;
    }
    Ok(Vec::from([sequential(src, from, decoders)?]))
}

pub(crate) fn sequential<'a>(src: &'a [u8], from: usize, decoders: &[(Decoder, bool)]) -> Result<Part<'a>, (u32, Stop)> {
    let hint = rows_hint(src.len() - from);
    let mut part = Part::new(src, from, decoders, hint);
    match scan::fast(src, from, src.len(), &mut part) {
        Ok(()) => {}
        Err(Stop::Irregular) => {
            part = Part::new(src, from, decoders, hint);
            scan::scalar(src, from, src.len(), &mut part).map_err(|e| (part.rows, e))?;
        }
        Err(e) => return Err((part.rows, e)),
    }
    part.finish();
    Ok(part)
}

fn rows_hint(bytes: usize) -> usize {
    // Most budgets are tens of bytes a row; the hint only sizes the first allocation.
    (bytes / 48).min(1 << 24)
}

/// Splits at record boundaries and reads each part on its own thread. A boundary is the
/// first `\n` after a cut that the count of quotes before it puts outside quotes, exact
/// while every quote is RFC 4180's; a part that meets any other quote sends the whole
/// body back to `sequential`, which says where the file breaks. Nothing when the input is
/// too small to pay for threads.
#[cfg(feature = "std")]
fn parallel<'a>(src: &'a [u8], from: usize, decoders: &[(Decoder, bool)]) -> Option<Result<Vec<Part<'a>>, (u32, Stop)>> {
    const PART: usize = 1 << 21;
    let len = src.len() - from;
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    if len < 2 * PART || threads < 2 {
        return None;
    }
    let n = threads.min(len / PART);
    let cuts: Vec<usize> = (0..=n).map(|k| from + len * k / n).collect();
    let quotes: Vec<usize> = std::thread::scope(|s| {
        let hs: Vec<_> = cuts.windows(2).map(|w| s.spawn(move || src[w[0]..w[1]].iter().filter(|&&c| c == b'"').count())).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut bounds = Vec::from([from]);
    let mut before = 0usize;
    for k in 1..n {
        before += quotes[k - 1];
        let mut inside = before % 2 == 1;
        let mut at = None;
        for (i, &c) in src[cuts[k]..].iter().enumerate() {
            if c == b'"' {
                inside = !inside;
            } else if c == b'\n' && !inside {
                at = Some(cuts[k] + i + 1);
                break;
            }
        }
        if let Some(b) = at
            && b > *bounds.last().unwrap()
            && b < src.len()
        {
            bounds.push(b);
        }
    }
    bounds.push(src.len());
    let results: Vec<Result<Part<'a>, (u32, Stop)>> = std::thread::scope(|s| {
        let hs: Vec<_> = bounds
            .windows(2)
            .map(|w| {
                let (a, b) = (w[0], w[1]);
                s.spawn(move || {
                    let mut part = Part::new(src, a, decoders, rows_hint(b - a));
                    scan::fast(src, a, b, &mut part).map_err(|e| (part.rows, e))?;
                    part.finish();
                    Ok(part)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut parts = Vec::with_capacity(results.len());
    for r in results {
        match r {
            Ok(p) => parts.push(p),
            // Any other stop is reported by the sequential reading, the one that knows
            // every row before it.
            Err(_) => return None,
        }
    }
    Some(Ok(parts))
}

/// The parts joined into one, in order: records, columns and problems (rows renumbered).
pub(crate) fn join(mut parts: Vec<Part<'_>>) -> Part<'_> {
    let mut first = parts.remove(0);
    for p in parts {
        let base = first.rows;
        first.records.extend_from_slice(&p.records);
        first.problems.extend(p.problems.into_iter().map(|q| Problem { row: q.row + base, ..q }));
        for (c, (a, b)) in first.columns.iter_mut().zip(p.columns).enumerate() {
            append(a, b, c as u32, base, &mut first.problems);
        }
        first.rows += p.rows;
    }
    first
}

fn append(a: &mut Reading, b: Reading, c: u32, base: u32, problems: &mut Vec<Problem>) {
    a.candidates &= b.candidates;
    let mut b_valid = b.valid;
    match (&mut a.building, b.building) {
        (Building::Text { offsets, bytes }, Building::Text { offsets: bo, bytes: bb }) => {
            let shift = bytes.len() as i32;
            offsets.extend(bo[1..].iter().map(|o| o + shift));
            bytes.extend_from_slice(&bb);
        }
        (Building::Int(x), Building::Int(y)) => x.extend_from_slice(&y),
        (Building::Decimal { values, scale }, Building::Decimal { values: mut bv, scale: bs }) => {
            if bs > *scale {
                raise_part(values, &mut a.valid, (bs - *scale) as u32, DECIMAL_LIMIT, c, 0, problems);
                *scale = bs;
            } else if bs < *scale {
                raise_part(&mut bv, &mut b_valid, (*scale - bs) as u32, DECIMAL_LIMIT, c, base, problems);
            }
            values.extend_from_slice(&bv);
        }
        (Building::DateTime { values, unit }, Building::DateTime { values: mut bv, unit: bu }) => {
            if bu > *unit {
                raise_part(values, &mut a.valid, (bu - *unit) as u32, INSTANT_LIMIT, c, 0, problems);
                *unit = bu;
            } else if bu < *unit {
                raise_part(&mut bv, &mut b_valid, (*unit - bu) as u32, INSTANT_LIMIT, c, base, problems);
            }
            values.extend_from_slice(&bv);
        }
        (Building::Bool(x), Building::Bool(y)) => x.append(&y),
        (Building::Date(x), Building::Date(y)) | (Building::Time(x), Building::Time(y)) => x.extend_from_slice(&y),
        (Building::Choice(x), Building::Choice(y)) => x.extend_from_slice(&y),
        _ => unreachable!("parts read with the same decoders"),
    }
    a.valid.append(&b_valid);
}

fn raise_part<T>(values: &mut [T], valid: &mut BitsBuilder, by: u32, limit: u128, c: u32, first: u32, problems: &mut Vec<Problem>)
where
    T: Copy + TryInto<i128> + TryFrom<i128>,
{
    let mut run = Run { valid, problems, column: c, first };
    column::raise(values, by, limit, &mut run);
}
