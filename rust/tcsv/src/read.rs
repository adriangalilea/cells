//! The body read into columns. Records are scanned into batches of field spans, and each
//! batch is decoded column by column, so one type's loop runs over a run of its cells. A
//! large input is split at record boundaries and its parts read on threads.

use alloc::vec::Vec;

use crate::column::{self, BitsBuilder, Building, DECIMAL_LIMIT, Decoder, INSTANT_LIMIT, Problem, Run, Why, infer};
use crate::scan::{self, Fields, Records, Stop};

/// Rows decoded together.
const BATCH: usize = 4096;

/// How a column's cells are read: its decoder, whether a blank is allowed, and whether
/// its formula computes a blank.
#[derive(Clone, Debug)]
pub(crate) struct Cells {
    pub decoder: Decoder,
    pub optional: bool,
    pub computed: bool,
}

/// One column while its cells are read.
pub(crate) struct Reading {
    pub decoder: Decoder,
    pub optional: bool,
    pub computed: bool,
    pub building: Building,
    pub valid: BitsBuilder,
    /// An untyped column's candidates (`infer`).
    pub candidates: u8,
}

/// What a cell's text is to a `.tcsv` file: a formula (`` `=cost*qty` ``), text written
/// with its first backtick doubled so it reads as no formula (`` ``=x` `` is `` `=x` ``),
/// or plain text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Said {
    Plain,
    Formula,
    Escaped,
}

#[inline]
pub(crate) fn said(t: &[u8]) -> Said {
    if t.first() != Some(&b'`') || t.last() != Some(&b'`') {
        return Said::Plain;
    }
    let ticks = t.iter().take_while(|&&c| c == b'`').count();
    match (ticks, t.get(ticks)) {
        (1, Some(b'=')) if t.len() >= 3 => Said::Formula,
        (2.., Some(b'=')) => Said::Escaped,
        _ => Said::Plain,
    }
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
    /// Where each control character of the batch being decoded starts in `src`.
    controls: Vec<u32>,
    /// Where the part starts and ends in `src`: the first batch's bytes against the
    /// part's say how many rows to make room for.
    from: usize,
    to: usize,
    /// A plain file's reading: a record of another width than the header is a problem,
    /// never a stop.
    lenient: bool,
}

impl<'a> Part<'a> {
    pub fn new(src: &'a [u8], from: usize, to: usize, decoders: &[Cells], lenient: bool) -> Part<'a> {
        let ncols = decoders.len();
        Part {
            src,
            ncols,
            lenient,
            cells: alloc::vec![(0, 0); ncols.max(1) * BATCH],
            batched: 0,
            field: 0,
            start: from as u32,
            records: Vec::with_capacity(BATCH + 1),
            columns: decoders
                .iter()
                .map(|c| Reading {
                    decoder: c.decoder.clone(),
                    optional: c.optional,
                    computed: c.computed,
                    building: Building::new(&c.decoder, BATCH),
                    valid: BitsBuilder::with_capacity(BATCH),
                    candidates: infer::ALL,
                })
                .collect(),
            problems: Vec::new(),
            rows: 0,
            scratch: Vec::new(),
            controls: Vec::new(),
            from,
            to,
        }
    }

    /// Decodes the batched rows.
    fn flush(&mut self) {
        let n = self.batched;
        let base = self.rows - n as u32;
        // The batch's records as one stretch of bytes, searched for control characters once:
        // a cell is looked into only when one starts inside it.
        let from = self.records[self.records.len() - n] as usize;
        self.controls.clear();
        scan::controls(self.src, from, self.start as usize, &mut self.controls);
        let formulas = !self.lenient;
        for (c, col) in self.columns.iter_mut().enumerate() {
            let cells = &self.cells[c * BATCH..c * BATCH + n];
            let (src, controls, problems, scratch) = (self.src, &self.controls[..], &mut self.problems, &mut self.scratch);
            match controls.is_empty() {
                true => decode_run::<false>(src, cells, controls, col, c as u32, base, formulas, problems, scratch),
                false => decode_run::<true>(src, cells, controls, col, c as u32, base, formulas, problems, scratch),
            }
        }
        if base == 0 && n == BATCH {
            // The first batch read `start - from` bytes: the rest of the part holds about
            // as many rows a byte, and each column about as much text a row.
            let read = (self.start as usize - self.from).max(1);
            let rest = (self.to - self.start as usize) * n / read;
            let rest = rest + rest / 16;
            self.records.reserve(rest);
            for col in &mut self.columns {
                col.valid.bytes.reserve(rest / 8 + 1);
                col.building.reserve(rest, n);
            }
        }
        self.batched = 0;
    }

    pub fn finish(&mut self) {
        if self.batched > 0 {
            self.flush();
        }
    }
}

/// A run of one column's cells, rows `base..`: `put(Some(text), run)` decodes a cell or
/// says why not, `put(None, run)` pushes the null under a blank or broken one. A cell
/// holding a control character is broken before its type is read: one whose span holds
/// a start in `controls` (sorted, as the cells are), looked for only in a run whose batch
/// has any (`CONTROLS`), so every other run's loop is as it would be without the rule.
/// A cell's formula, a formula column's blank, are marked (`Why::Formula`,
/// `Why::Computed`) where `formulas` (a `.tcsv` file's), its value null until computed.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn each<const CONTROLS: bool>(
    src: &[u8],
    cells: &[(u32, u32)],
    controls: &[u32],
    valid: &mut BitsBuilder,
    problems: &mut Vec<Problem>,
    c: u32,
    base: u32,
    missing: bool,
    computed: bool,
    formulas: bool,
    scratch: &mut Vec<u8>,
    mut put: impl FnMut(Option<&[u8]>, &mut Run) -> Result<(), Why>,
) {
    let mut next = 0;
    for (k, &(s, e)) in cells.iter().enumerate() {
        let row = base + k as u32;
        let mut text = scan::text(src, s, e, scratch);
        // A part's rows count from 0, so its values start at row 0.
        let mut run = Run { valid, problems, column: c, first: 0 };
        if text.is_empty() {
            let _ = put(None, &mut run);
            valid.push(false);
            if computed {
                problems.push(Problem { row, column: c, why: Why::Computed });
            } else if missing {
                problems.push(Problem { row, column: c, why: Why::Missing });
            }
            continue;
        }
        if formulas && text[0] == b'`' {
            match said(text) {
                Said::Formula => {
                    let _ = put(None, &mut run);
                    valid.push(false);
                    problems.push(Problem { row, column: c, why: Why::Formula });
                    continue;
                }
                Said::Escaped => text = &text[1..],
                Said::Plain => {}
            }
        }
        if CONTROLS {
            while next < controls.len() && controls[next] < s {
                next += 1;
            }
            if next < controls.len() && controls[next] < e {
                // The span may be quoted: the text says which character it is.
                let point = column::control(text).unwrap();
                let _ = put(None, &mut run);
                valid.push(false);
                problems.push(Problem { row, column: c, why: Why::Control { point } });
                continue;
            }
        }
        match put(Some(text), &mut run) {
            Ok(()) => valid.push(true),
            Err(why) => {
                let mut run = Run { valid, problems, column: c, first: 0 };
                let _ = put(None, &mut run);
                valid.push(false);
                problems.push(Problem { row, column: c, why });
            }
        }
    }
}

/// Decodes a run of one column's cells, rows `base..`, in a loop chosen once for the
/// column's kind; the kinds rarely seen share `column::decode`'s. `controls` is where the
/// control characters of the run's batch start, `CONTROLS` whether there are any.
#[allow(clippy::too_many_arguments)]
fn decode_run<const CONTROLS: bool>(
    src: &[u8],
    cells: &[(u32, u32)],
    controls: &[u32],
    col: &mut Reading,
    c: u32,
    base: u32,
    formulas: bool,
    problems: &mut Vec<Problem>,
    scratch: &mut Vec<u8>,
) {
    let Reading { decoder, optional, computed, building, valid, candidates } = col;
    let missing = !*optional && !matches!(decoder, Decoder::Untyped);
    let computed = *computed;
    match (&*decoder, &mut *building) {
        (Decoder::Untyped, Building::Text { offsets, bytes }) => {
            each::<CONTROLS>(src, cells, controls, valid, problems, c, base, missing, computed, formulas, scratch, |t, _| {
                match t {
                    None => offsets.push(bytes.len() as i32),
                    Some(t) => {
                        *candidates = infer::narrow(*candidates, t);
                        column::push_text(offsets, bytes, t);
                    }
                }
                Ok(())
            })
        }
        (Decoder::Text { min: None, max: None }, Building::Text { offsets, bytes }) => {
            each::<CONTROLS>(src, cells, controls, valid, problems, c, base, missing, computed, formulas, scratch, |t, _| {
                match t {
                    None => offsets.push(bytes.len() as i32),
                    Some(t) => column::push_text(offsets, bytes, t),
                }
                Ok(())
            })
        }
        (Decoder::Choice(words), Building::Choice(v)) => {
            each::<CONTROLS>(src, cells, controls, valid, problems, c, base, missing, computed, formulas, scratch, |t, _| {
                let Some(t) = t else {
                    v.push(0);
                    return Ok(());
                };
                let k = words.iter().position(|w| column::same(t, w)).ok_or(Why::NotChoice)?;
                v.push(k as i16);
                Ok(())
            })
        }
        (Decoder::Date(None), Building::Date(v)) => {
            each::<CONTROLS>(src, cells, controls, valid, problems, c, base, missing, computed, formulas, scratch, |t, _| {
                let Some(t) = t else {
                    v.push(0);
                    return Ok(());
                };
                v.push(crate::value::parse_date(t).ok_or(Why::NotDate)?);
                Ok(())
            })
        }
        (d, b) => {
            each::<CONTROLS>(src, cells, controls, valid, problems, c, base, missing, computed, formulas, scratch, |t, run| match t {
                None => {
                    column::push_null(b);
                    Ok(())
                }
                Some(t) => column::decode(d, b, t, run),
            })
        }
    }
}

impl Fields for Part<'_> {
    #[inline]
    fn field(&mut self, start: u32, end: u32) -> Result<(), Stop> {
        if self.field >= self.ncols {
            if !self.lenient {
                return Err(Stop::TooMany);
            }
            self.field += 1;
            return Ok(());
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
        if self.lenient && self.field != self.ncols {
            self.problems.push(ragged(self.rows, self.field, self.ncols));
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

/// A record of `fields` fields where the header has `ncols`, read leniently: its problem,
/// on the first cell it lacks, or on the last cell of a record too long.
pub(crate) fn ragged(row: u32, fields: usize, ncols: usize) -> Problem {
    Problem { row, column: fields.min(ncols - 1) as u32, why: Why::Ragged { fields: fields as u32 } }
}

/// The body's records read into parts, on up to `threads` threads: `Err` names the row
/// (from 0) and why. The parts joined are the sequential reading, whatever the threads.
pub(crate) fn body<'a>(src: &'a [u8], from: usize, decoders: &[Cells], how: Records, threads: usize) -> Result<Vec<Part<'a>>, (u32, Stop)> {
    #[cfg(feature = "std")]
    if let Some(parts) = parallel(src, from, decoders, how, threads) {
        return parts;
    }
    #[cfg(not(feature = "std"))]
    assert_eq!(threads, 1, "threads need the std feature");
    Ok(Vec::from([sequential(src, from, decoders, how)?]))
}

pub(crate) fn sequential<'a>(src: &'a [u8], from: usize, decoders: &[Cells], how: Records) -> Result<Part<'a>, (u32, Stop)> {
    let mut part = Part::new(src, from, src.len(), decoders, how.lenient);
    match scan::fast(src, from, src.len(), how.sep, &mut part) {
        Ok(()) => {}
        Err(Stop::Irregular) => {
            part = Part::new(src, from, src.len(), decoders, how.lenient);
            scan::scalar(src, from, src.len(), how, &mut part).map_err(|e| (part.rows, e))?;
        }
        Err(e) => return Err((part.rows, e)),
    }
    part.finish();
    Ok(part)
}

/// Splits at record boundaries and reads each part on its own thread. A boundary is the
/// first `\n` after a cut that the count of quotes before it puts outside quotes, exact
/// while every quote is RFC 4180's; a part that meets any other quote sends the whole
/// body back to `sequential`, which says where the file breaks. Nothing when the input is
/// too small to pay for threads.
#[cfg(feature = "std")]
fn parallel<'a>(
    src: &'a [u8],
    from: usize,
    decoders: &[Cells],
    how: Records,
    threads: usize,
) -> Option<Result<Vec<Part<'a>>, (u32, Stop)>> {
    // The least a thread reads: below it, starting the thread costs more than it saves.
    const PART: usize = 1 << 19;
    let len = src.len() - from;
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
                    let mut part = Part::new(src, a, b, decoders, how.lenient);
                    scan::fast(src, a, b, how.sep, &mut part).map_err(|e| (part.rows, e))?;
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

/// The parts joined into one, in order: records, columns and problems (rows renumbered),
/// each column joined on a thread of its own.
pub(crate) fn join(mut parts: Vec<Part<'_>>) -> Part<'_> {
    if parts.len() == 1 {
        return parts.pop().unwrap();
    }
    let mut bases = Vec::with_capacity(parts.len());
    let mut rows = 0u32;
    for p in &parts {
        bases.push(rows);
        rows += p.rows;
    }
    let ncols = parts[0].columns.len();
    let mut by_column: Vec<Vec<Reading>> = (0..ncols).map(|_| Vec::with_capacity(parts.len())).collect();
    for p in parts.iter_mut() {
        for (c, r) in p.columns.drain(..).enumerate() {
            by_column[c].push(r);
        }
    }
    let bases = &bases;
    #[cfg(feature = "std")]
    let joined: Vec<(Reading, Vec<Problem>)> = std::thread::scope(|s| {
        let hs: Vec<_> = by_column.into_iter().enumerate().map(|(c, rs)| s.spawn(move || join_column(rs, bases, c as u32))).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    #[cfg(not(feature = "std"))]
    let joined: Vec<(Reading, Vec<Problem>)> = by_column.into_iter().enumerate().map(|(c, rs)| join_column(rs, bases, c as u32)).collect();
    let mut records = Vec::with_capacity(rows as usize + 1);
    let mut problems = Vec::new();
    for (p, &base) in parts.iter().zip(bases) {
        records.extend_from_slice(&p.records);
        problems.extend(p.problems.iter().map(|q| Problem { row: q.row + base, ..*q }));
    }
    let mut first = parts.swap_remove(0);
    first.records = records;
    first.problems = problems;
    first.rows = rows;
    for (r, mut found) in joined {
        first.columns.push(r);
        first.problems.append(&mut found);
    }
    first
}

/// One column's readings from every part, joined, with the problems a raised scale finds.
fn join_column(readings: Vec<Reading>, bases: &[u32], c: u32) -> (Reading, Vec<Problem>) {
    let mut problems = Vec::new();
    let mut it = readings.into_iter();
    let mut a = it.next().unwrap();
    let rest: Vec<Reading> = it.collect();
    match &mut a.building {
        Building::Text { offsets, bytes } => {
            offsets.reserve(rest.iter().map(|r| r.valid.len).sum());
            bytes.reserve(rest.iter().map(|r| if let Building::Text { bytes, .. } = &r.building { bytes.len() } else { 0 }).sum());
        }
        Building::Int(v) => v.reserve(rest.iter().map(|r| r.valid.len).sum()),
        Building::Decimal { values, .. } => values.reserve(rest.iter().map(|r| r.valid.len).sum()),
        _ => {}
    }
    for (b, &base) in rest.into_iter().zip(&bases[1..]) {
        append(&mut a, b, c, base, &mut problems);
    }
    (a, problems)
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
