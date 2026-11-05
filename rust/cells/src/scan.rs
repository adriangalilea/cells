//! Records: RFC 4180 fields found 64 bytes at a time. Each block's quotes and separators
//! (the field separator, `\n`, `\r`) become two bitmasks (NEON, SSE2, wasm SIMD, or a
//! portable loop); the quoted stretches are the prefix XOR of the quotes, and every
//! separator outside them ends a field. That reading is exact only while every quote
//! stands where RFC 4180 puts it, opening a field or closing one, and every `\r` ends a
//! line with `\n` after it; a block that breaks either is irregular, and the input is read
//! again byte by byte (`scalar`), which says which record breaks and how, or, for a file
//! that is not Cells (`Records::lenient`), reads it as CSV readers do.

/// How records are separated and how strictly they are read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Records {
    /// The byte between fields: `,` `;` `\t` `|`.
    pub sep: u8,
    /// A plain file's reading: a quote out of RFC 4180's places is text, a quoted field
    /// never closed runs to the end, a lone `\r` ends a line, a record may be any width.
    pub lenient: bool,
}

impl Records {
    pub const CELLS: Records = Records { sep: b',', lenient: false };
}

/// Where a reading stopped short.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stop {
    /// A quote out of RFC 4180's places, or a lone `\r`: read again with `scalar`.
    Irregular,
    /// A quoted field reaches the end of the input.
    Unclosed,
    /// Text after a quoted field's closing quote.
    AfterQuote,
    /// A `"` inside a field that did not open with one.
    Quote,
    /// A `\r` outside quotes with no `\n` after it.
    Return,
    /// A record with more fields than the header.
    TooMany,
}

/// What the scanners hand their reader: each field's raw span (quotes included) and
/// each record's end, as the offset where the next record starts.
pub(crate) trait Fields {
    fn field(&mut self, start: u32, end: u32) -> Result<(), Stop>;
    fn record(&mut self, next: u32);
    /// Whether the record being read has a field already.
    fn open(&self) -> bool;
    /// Whether to read no further.
    fn done(&self) -> bool {
        false
    }
}

#[inline(always)]
fn prefix_xor(mut x: u64) -> u64 {
    x ^= x << 1;
    x ^= x << 2;
    x ^= x << 4;
    x ^= x << 8;
    x ^= x << 16;
    x ^= x << 32;
    x
}

/// The quotes and the separators of 64 bytes (`sep` between fields), bit `k` for byte `k`.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn masks(b: &[u8; 64], sep: u8) -> (u64, u64) {
    use core::arch::aarch64::*;
    // SAFETY: NEON is part of every aarch64 target; the loads read the 64 bytes of `b`.
    unsafe {
        let p = b.as_ptr();
        let v = [vld1q_u8(p), vld1q_u8(p.add(16)), vld1q_u8(p.add(32)), vld1q_u8(p.add(48))];
        let (q, c, lf, cr) = (vdupq_n_u8(b'"'), vdupq_n_u8(sep), vdupq_n_u8(b'\n'), vdupq_n_u8(b'\r'));
        let qm = v.map(|x| vceqq_u8(x, q));
        let sm = v.map(|x| vorrq_u8(vorrq_u8(vceqq_u8(x, c), vceqq_u8(x, lf)), vceqq_u8(x, cr)));
        (movemask(qm), movemask(sm))
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn movemask(m: [core::arch::aarch64::uint8x16_t; 4]) -> u64 {
    use core::arch::aarch64::*;
    const BITS: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];
    // SAFETY: NEON on aarch64; BITS is 16 bytes.
    unsafe {
        let bits = vld1q_u8(BITS.as_ptr());
        let s0 = vpaddq_u8(vandq_u8(m[0], bits), vandq_u8(m[1], bits));
        let s1 = vpaddq_u8(vandq_u8(m[2], bits), vandq_u8(m[3], bits));
        let s = vpaddq_u8(s0, s1);
        let s = vpaddq_u8(s, s);
        vgetq_lane_u64(vreinterpretq_u64_u8(s), 0)
    }
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn masks(b: &[u8; 64], sep: u8) -> (u64, u64) {
    use core::arch::x86_64::*;
    // SAFETY: SSE2 is part of every x86_64 target; the loads read the 64 bytes of `b`.
    unsafe {
        let p = b.as_ptr() as *const __m128i;
        let (q, c, lf, cr) = (_mm_set1_epi8(b'"' as i8), _mm_set1_epi8(sep as i8), _mm_set1_epi8(b'\n' as i8), _mm_set1_epi8(b'\r' as i8));
        let (mut qm, mut sm) = (0u64, 0u64);
        for k in 0..4 {
            let v = _mm_loadu_si128(p.add(k));
            let qk = _mm_movemask_epi8(_mm_cmpeq_epi8(v, q)) as u16 as u64;
            let s = _mm_or_si128(_mm_or_si128(_mm_cmpeq_epi8(v, c), _mm_cmpeq_epi8(v, lf)), _mm_cmpeq_epi8(v, cr));
            let sk = _mm_movemask_epi8(s) as u16 as u64;
            qm |= qk << (16 * k);
            sm |= sk << (16 * k);
        }
        (qm, sm)
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline(always)]
fn masks(b: &[u8; 64], sep: u8) -> (u64, u64) {
    use core::arch::wasm32::*;
    let (q, c, lf, cr) = (u8x16_splat(b'"'), u8x16_splat(sep), u8x16_splat(b'\n'), u8x16_splat(b'\r'));
    let (mut qm, mut sm) = (0u64, 0u64);
    for k in 0..4 {
        // SAFETY: reads 16 of the 64 bytes of `b`; v128_load takes any alignment.
        let v = unsafe { v128_load(b.as_ptr().add(16 * k) as *const v128) };
        let qk = u8x16_bitmask(u8x16_eq(v, q)) as u64;
        let s = v128_or(v128_or(u8x16_eq(v, c), u8x16_eq(v, lf)), u8x16_eq(v, cr));
        qm |= qk << (16 * k);
        sm |= (u8x16_bitmask(s) as u64) << (16 * k);
    }
    (qm, sm)
}

#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64", all(target_arch = "wasm32", target_feature = "simd128"))))]
#[inline(always)]
fn masks(b: &[u8; 64], sep: u8) -> (u64, u64) {
    let (mut qm, mut sm) = (0u64, 0u64);
    for (k, &c) in b.iter().enumerate() {
        qm |= ((c == b'"') as u64) << k;
        sm |= ((c == sep || c == b'\n' || c == b'\r') as u64) << k;
    }
    (qm, sm)
}

/// Whether 64 bytes may hold a control character: a byte below 0x20 but tab, `\n` and
/// `\r`, DEL, or 0xC2, which begins a C1 and also `¡`, `£` and a no-break space, so a block
/// it flags is read exactly (`any_control`).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn suspects(b: &[u8; 64]) -> bool {
    use core::arch::aarch64::*;
    // 0xFF at every byte below 0x20 but tab, `\n` and `\r`.
    const C0: [u8; 32] = {
        let mut t = [0xFF; 32];
        t[b'\t' as usize] = 0;
        t[b'\n' as usize] = 0;
        t[b'\r' as usize] = 0;
        t
    };
    // SAFETY: NEON is part of every aarch64 target; the loads read the 64 bytes of `b` and
    // the 32 of `C0`.
    unsafe {
        let table = uint8x16x2_t(vld1q_u8(C0.as_ptr()), vld1q_u8(C0.as_ptr().add(16)));
        let (del, c2) = (vdupq_n_u8(0x7F), vdupq_n_u8(0xC2));
        let p = b.as_ptr();
        let mut any = vdupq_n_u8(0);
        for k in 0..4 {
            let v = vld1q_u8(p.add(16 * k));
            // A lookup past the table's 32 bytes is 0.
            any = vorrq_u8(any, vorrq_u8(vqtbl2q_u8(table, v), vorrq_u8(vceqq_u8(v, del), vceqq_u8(v, c2))));
        }
        vmaxvq_u8(any) != 0
    }
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn suspects(b: &[u8; 64]) -> bool {
    use core::arch::x86_64::*;
    // SAFETY: SSE2 is part of every x86_64 target; the loads read the 64 bytes of `b`.
    unsafe {
        let p = b.as_ptr() as *const __m128i;
        let (top, tab, lf, cr) = (_mm_set1_epi8(0x1F), _mm_set1_epi8(b'\t' as i8), _mm_set1_epi8(b'\n' as i8), _mm_set1_epi8(b'\r' as i8));
        let (del, c2) = (_mm_set1_epi8(0x7F), _mm_set1_epi8(0xC2u8 as i8));
        let mut any = _mm_setzero_si128();
        for k in 0..4 {
            let v = _mm_loadu_si128(p.add(k));
            // Unsigned `v <= 0x1F`: the minimum of the two is `v`.
            let low = _mm_cmpeq_epi8(_mm_min_epu8(v, top), v);
            let line = _mm_or_si128(_mm_or_si128(_mm_cmpeq_epi8(v, tab), _mm_cmpeq_epi8(v, lf)), _mm_cmpeq_epi8(v, cr));
            let rest = _mm_or_si128(_mm_cmpeq_epi8(v, del), _mm_cmpeq_epi8(v, c2));
            any = _mm_or_si128(any, _mm_or_si128(_mm_andnot_si128(line, low), rest));
        }
        _mm_movemask_epi8(any) != 0
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline(always)]
fn suspects(b: &[u8; 64]) -> bool {
    use core::arch::wasm32::*;
    let (top, tab, lf, cr) = (u8x16_splat(0x20), u8x16_splat(b'\t'), u8x16_splat(b'\n'), u8x16_splat(b'\r'));
    let (del, c2) = (u8x16_splat(0x7F), u8x16_splat(0xC2));
    let mut any = u8x16_splat(0);
    for k in 0..4 {
        // SAFETY: reads 16 of the 64 bytes of `b`; v128_load takes any alignment.
        let v = unsafe { v128_load(b.as_ptr().add(16 * k) as *const v128) };
        let line = v128_or(v128_or(u8x16_eq(v, tab), u8x16_eq(v, lf)), u8x16_eq(v, cr));
        let rest = v128_or(u8x16_eq(v, del), u8x16_eq(v, c2));
        any = v128_or(any, v128_or(v128_andnot(u8x16_lt(v, top), line), rest));
    }
    v128_any_true(any)
}

#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64", all(target_arch = "wasm32", target_feature = "simd128"))))]
#[inline(always)]
fn suspects(b: &[u8; 64]) -> bool {
    b.iter().any(|&c| (c < 0x20 && !matches!(c, b'\t' | b'\n' | b'\r')) || c == 0x7F || c == 0xC2)
}

/// Where each control character of `src[from..to]` starts, in order, into `out`: 64 bytes
/// a step flagged by `suspects`, a flagged block read exactly. What lets a batch's cells
/// skip `column::control` one by one.
pub(crate) fn controls(src: &[u8], from: usize, to: usize, out: &mut alloc::vec::Vec<u32>) {
    let exact = |a: usize, b: usize, out: &mut alloc::vec::Vec<u32>| {
        // The byte after the stretch too, the second of a C1 begun at its end.
        let mut k = a;
        while let Some((at, _)) = crate::column::control_at(&src[k..to.min(b + 1)]) {
            if k + at >= b {
                break;
            }
            out.push((k + at) as u32);
            k += at + 1;
        }
    };
    let mut i = from;
    while i + 64 <= to {
        if suspects(src[i..i + 64].try_into().unwrap()) {
            exact(i, i + 64, out);
        }
        i += 64;
    }
    exact(i, to, out);
}

/// Reads the records of `src[from..to]`, `from` a record's start, fields separated by
/// `sep`. `to` is a record's start too, or the input's end, where a record without a line
/// break ends.
pub(crate) fn fast<F: Fields>(src: &[u8], from: usize, to: usize, sep: u8, out: &mut F) -> Result<(), Stop> {
    debug_assert!(to <= src.len() && to <= u32::MAX as usize);
    let mut inside = 0u64; // all ones while the block before ended inside quotes
    let mut prev = 1u64; // the byte before is a separator, a quote, or the start
    let mut closer = 0u64; // the block before ended on a closing quote
    let mut field = from;
    let mut skip = usize::MAX; // the `\n` of a `\r\n` already counted
    let mut pos = from;
    let mut pad = [sep; 64];
    while pos < to {
        let n = (to - pos).min(64);
        let block: &[u8; 64] = if n == 64 {
            src[pos..pos + 64].try_into().unwrap()
        } else {
            pad.fill(sep);
            pad[..n].copy_from_slice(&src[pos..to]);
            &pad
        };
        let (q, s) = masks(block, sep);
        let x = prefix_xor(q) ^ inside;
        inside = ((x as i64) >> 63) as u64;
        let openers = q & x;
        let closers = q & !x;
        let sq = s | q;
        let after = (sq << 1) | prev;
        prev = sq >> 63;
        let next = (closers << 1) | closer;
        closer = closers >> 63;
        if (openers & !after) | (next & !sq) != 0 {
            return Err(Stop::Irregular);
        }
        let valid = if n == 64 { !0 } else { (1u64 << n) - 1 };
        let mut st = s & !x & valid;
        while st != 0 {
            let p = pos + st.trailing_zeros() as usize;
            st &= st - 1;
            let c = src[p];
            if c == b'\n' && p == skip {
                continue;
            }
            let (a, b) = span(src, field, p, q, pos);
            out.field(a, b)?;
            match c {
                b'\n' => {
                    out.record(p as u32 + 1);
                    field = p + 1;
                }
                c if c == sep => field = p + 1,
                _ => {
                    if p + 1 >= to || src[p + 1] != b'\n' {
                        return Err(Stop::Irregular);
                    }
                    skip = p + 1;
                    out.record(p as u32 + 2);
                    field = p + 2;
                }
            }
        }
        pos += 64;
    }
    if inside != 0 {
        // A quote left open: `scalar` says where.
        return Err(Stop::Irregular);
    }
    if field < to || out.open() {
        let (a, b) = span(src, field, to, 0, usize::MAX);
        out.field(a, b)?;
        out.record(to as u32);
    }
    Ok(())
}

/// A field's span as `fast` hands it on: a quoted field with no `""` inside as its text
/// alone, quotes cut, so `text` returns it as it is; any other field raw. `q` is the quotes
/// of the block at `pos`, which says it at once for a field that starts in that block.
#[inline(always)]
fn span(src: &[u8], field: usize, end: usize, q: u64, pos: usize) -> (u32, u32) {
    if field >= end || src[field] != b'"' {
        return (field as u32, end as u32);
    }
    let clean = if field >= pos {
        // Quotes strictly inside: bits field+1 ..= end-2 of the block.
        let inner = end - field - 2;
        let bits = q >> (field - pos + 1);
        let bits = if inner >= 64 { bits } else { bits & ((1u64 << inner) - 1) };
        bits == 0 && end - pos <= 64
    } else {
        !src[field + 1..end - 1].contains(&b'"')
    };
    if clean { (field as u32 + 1, end as u32 - 1) } else { (field as u32, end as u32) }
}

/// Reads the records of `src[from..to]` byte by byte: a field opening with `"` is quoted
/// to its closing quote, `""` a quote inside; any other quote, text after a closing quote,
/// a quote never closed and a lone `\r` stop it. Read leniently, none of them stops it: a
/// field runs on to its separator, a quote never closed to the end, and a lone `\r` ends
/// its line.
pub(crate) fn scalar<F: Fields>(src: &[u8], from: usize, to: usize, how: Records, out: &mut F) -> Result<(), Stop> {
    let Records { sep, lenient } = how;
    let ends = |c: u8| c == sep || c == b'\n' || c == b'\r';
    let mut i = from;
    if i >= to {
        return Ok(());
    }
    loop {
        let start = i;
        if src[i] == b'"' {
            i += 1;
            loop {
                if i >= to {
                    if lenient {
                        break;
                    }
                    return Err(Stop::Unclosed);
                }
                if src[i] == b'"' {
                    if i + 1 < to && src[i + 1] == b'"' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            if i < to && !ends(src[i]) {
                if !lenient {
                    return Err(Stop::AfterQuote);
                }
                while i < to && !ends(src[i]) {
                    i += 1;
                }
            }
        } else {
            while i < to && !ends(src[i]) {
                if src[i] == b'"' && !lenient {
                    return Err(Stop::Quote);
                }
                i += 1;
            }
        }
        if i < to && src[i] == b'\r' && (i + 1 >= to || src[i + 1] != b'\n') && !lenient {
            return Err(Stop::Return);
        }
        out.field(start as u32, i as u32)?;
        if i >= to {
            out.record(to as u32);
            return Ok(());
        }
        if src[i] == sep {
            i += 1;
            if i >= to {
                out.field(to as u32, to as u32)?;
                out.record(to as u32);
                return Ok(());
            }
            continue;
        }
        i += if src[i] == b'\r' && i + 1 < to && src[i + 1] == b'\n' { 2 } else { 1 };
        out.record(i as u32);
        if i >= to || out.done() {
            return Ok(());
        }
    }
}

/// One record's fields, read from `from`.
pub(crate) struct One {
    pub fields: alloc::vec::Vec<(u32, u32)>,
    pub next: Option<u32>,
}

impl Fields for One {
    fn field(&mut self, start: u32, end: u32) -> Result<(), Stop> {
        if self.next.is_none() {
            self.fields.push((start, end));
        }
        Ok(())
    }

    fn record(&mut self, next: u32) {
        if self.next.is_none() {
            self.next = Some(next);
        }
    }

    fn open(&self) -> bool {
        self.next.is_none() && !self.fields.is_empty()
    }

    fn done(&self) -> bool {
        self.next.is_some()
    }
}

/// The record starting at `from`: its fields' raw spans and where the next starts.
pub(crate) fn record(src: &[u8], from: usize, how: Records) -> Result<(alloc::vec::Vec<(u32, u32)>, usize), Stop> {
    if from == src.len() {
        // An empty record: one blank field.
        return Ok((alloc::vec![(from as u32, from as u32)], from));
    }
    let mut one = One { fields: alloc::vec::Vec::new(), next: None };
    scalar(src, from, src.len(), how, &mut one)?;
    Ok((one.fields, one.next.unwrap_or(src.len() as u32) as usize))
}

/// A field's text: its span unquoted, `""` read as `"`; a span `fast` cut already is its
/// text as it is. A field opening with `"` that a lenient reading took as written keeps
/// what follows its closing quote, and one never closed is everything after its quote.
#[inline(always)]
pub(crate) fn text<'a>(src: &'a [u8], start: u32, end: u32, scratch: &'a mut alloc::vec::Vec<u8>) -> &'a [u8] {
    let raw = &src[start as usize..end as usize];
    if raw.first() != Some(&b'"') {
        return raw;
    }
    let inner = &raw[1..];
    if let Some((&b'"', body)) = inner.split_last()
        && !body.contains(&b'"')
    {
        return body;
    }
    scratch.clear();
    let mut k = 0;
    while k < inner.len() {
        if inner[k] == b'"' {
            if inner.get(k + 1) == Some(&b'"') {
                scratch.push(b'"');
                k += 2;
                continue;
            }
            scratch.extend_from_slice(&inner[k + 1..]);
            break;
        }
        scratch.push(inner[k]);
        k += 1;
    }
    scratch
}

/// Whether a field as written ends where it does with anything written after it: false
/// only for a field a lenient reading took open to the end of its file (a quote never
/// closed), which would swallow what follows it.
pub(crate) fn closed(raw: &str) -> bool {
    let Some(inner) = raw.strip_prefix('"') else { return true };
    let b = inner.as_bytes();
    let mut k = 0;
    while k < b.len() {
        if b[k] == b'"' {
            if b.get(k + 1) != Some(&b'"') {
                return true;
            }
            k += 2;
            continue;
        }
        k += 1;
    }
    false
}

/// A field as written, unquoted, as `text` reads it.
pub(crate) fn unquoted(raw: &str) -> alloc::borrow::Cow<'_, str> {
    use alloc::borrow::Cow;
    let Some(inner) = raw.strip_prefix('"') else { return Cow::Borrowed(raw) };
    if let Some(body) = inner.strip_suffix('"')
        && !body.contains('"')
    {
        return Cow::Borrowed(body);
    }
    let mut scratch = alloc::vec::Vec::new();
    let t = text(raw.as_bytes(), 0, raw.len() as u32, &mut scratch).to_vec();
    // Cut at ASCII quotes only, so the text of a `str` is one.
    Cow::Owned(alloc::string::String::from_utf8(t).unwrap())
}

/// A cell's text in the file's form: quoted only when it holds `,`, `"` or a line break.
pub fn quote(text: &str) -> alloc::borrow::Cow<'_, str> {
    quote_in(text, b',', false)
}

/// A cell's text in a file whose fields are separated by `sep`: quoted when it holds
/// `sep`, `"` or a line break, or always when `all` (a file that quotes every field).
pub(crate) fn quote_in(text: &str, sep: u8, all: bool) -> alloc::borrow::Cow<'_, str> {
    if !all && !text.bytes().any(|c| c == sep || matches!(c, b'"' | b'\n' | b'\r')) {
        return alloc::borrow::Cow::Borrowed(text);
    }
    let mut s = alloc::string::String::with_capacity(text.len() + 2);
    s.push('"');
    for ch in text.chars() {
        if ch == '"' {
            s.push('"');
        }
        s.push(ch);
    }
    s.push('"');
    alloc::borrow::Cow::Owned(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[derive(Default)]
    struct Collect {
        records: Vec<Vec<(u32, u32)>>,
        current: Vec<(u32, u32)>,
        nexts: Vec<u32>,
    }

    impl Fields for Collect {
        fn field(&mut self, start: u32, end: u32) -> Result<(), Stop> {
            self.current.push((start, end));
            Ok(())
        }
        fn record(&mut self, next: u32) {
            self.records.push(core::mem::take(&mut self.current));
            self.nexts.push(next);
        }
        fn open(&self) -> bool {
            !self.current.is_empty()
        }
    }

    fn both(src: &str) -> (Result<Collect, Stop>, Result<Collect, Stop>) {
        both_in(src, Records::CELLS)
    }

    fn both_in(src: &str, how: Records) -> (Result<Collect, Stop>, Result<Collect, Stop>) {
        let b = src.as_bytes();
        let mut f = Collect::default();
        let fr = fast(b, 0, b.len(), how.sep, &mut f).map(|_| f);
        let mut s = Collect::default();
        let sr = scalar(b, 0, b.len(), how, &mut s).map(|_| s);
        (fr, sr)
    }

    #[test]
    fn other_separators() {
        for sep in *b";\t|" {
            let s = sep as char;
            let src = alloc::format!("a{s}\"b{s}c\"{s}d,e\n1{s}\"\"\"q\"\"\"{s}{}\n", "z".repeat(80));
            let how = Records { sep, lenient: false };
            let (f, sc) = both_in(&src, how);
            let (f, sc) = (f.unwrap(), sc.unwrap());
            assert_eq!(texts(&src, &f), texts(&src, &sc));
            let want = alloc::vec![b"a".to_vec(), alloc::format!("b{s}c").into_bytes(), b"d,e".to_vec()];
            assert_eq!(texts(&src, &f)[0], want);
            assert_eq!(texts(&src, &f)[1][1], b"\"q\"");
        }
    }

    #[test]
    fn lenient_reads_what_strict_refuses() {
        let how = Records { sep: b',', lenient: true };
        let read = |src: &str| {
            let mut s = Collect::default();
            scalar(src.as_bytes(), 0, src.len(), how, &mut s).unwrap();
            texts(src, &s)
                .into_iter()
                .map(|r| r.into_iter().map(|t| alloc::string::String::from_utf8(t).unwrap()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(read("a,5\" pipe,c\n"), [["a", "5\" pipe", "c"]]);
        assert_eq!(read("\"ab\"c,d\n"), [["abc", "d"]]);
        assert_eq!(read("a,\"open\nx,y"), [["a", "open\nx,y"]]);
        assert_eq!(read("a,b\rc,d\r"), [["a", "b"], ["c", "d"]]);
        assert_eq!(read("a,\""), [["a", ""]]);
        assert_eq!(unquoted("\"ab\"c"), "abc");
        assert_eq!(unquoted("\"a\"\"b\""), "a\"b");
        assert_eq!(unquoted("plain"), "plain");
    }

    /// The records' texts: `fast` hands a clean quoted field on unquoted, `scalar` raw.
    fn texts(src: &str, c: &Collect) -> Vec<Vec<Vec<u8>>> {
        let mut scratch = Vec::new();
        c.records.iter().map(|r| r.iter().map(|&(s, e)| text(src.as_bytes(), s, e, &mut scratch).to_vec()).collect()).collect()
    }

    #[test]
    fn fast_equals_scalar() {
        let long = "x".repeat(70);
        let quoted = alloc::format!("a,\"{}\",b\n\"{}\"", "z".repeat(70), "w".repeat(130));
        let cases = [
            "a,b\n1,2\n",
            "a,b\r\n1,2",
            "a,\"b,\"\"c\"\"\nd\"\n1,2\n",
            "\"\",,\n",
            "\"q\"",
            "a\r\nb\r\n",
            "a,b,\n",
            "\n\n",
            long.as_str(),
            quoted.as_str(),
        ];
        for c in cases {
            let (f, s) = both(c);
            let (f, s) = (f.unwrap(), s.unwrap());
            assert_eq!(texts(c, &f), texts(c, &s), "{c:?}");
            assert_eq!(f.nexts, s.nexts, "{c:?}");
        }
        // Long input across many blocks, quotes crossing block edges.
        let mut big = alloc::string::String::new();
        for k in 0..500 {
            big.push_str(&alloc::format!("{k},\"q,{k}\r\n\"\"x\"\"\",\"{}\",{}\n", "v".repeat(k % 90), "y".repeat(k % 70)));
        }
        let (f, s) = both(&big);
        assert_eq!(texts(&big, &f.unwrap()), texts(&big, &s.unwrap()));
    }

    #[test]
    fn controls_found_anywhere() {
        let found = |s: &str| {
            let mut out = Vec::new();
            controls(s.as_bytes(), 0, s.len(), &mut out);
            out
        };
        let filler = "a,b\t1\r\n£ ¡x".repeat(40);
        assert_eq!(found(&filler), []);
        assert_eq!(found(&"\u{a0}\u{a3}\u{bf}".repeat(30)), []);
        for ch in ['\u{0}', '\u{7}', '\u{1b}', '\u{7f}', '\u{80}', '\u{85}', '\u{9f}'] {
            for at in [0, 62, 63, 64, 127, filler.len()] {
                let at = (0..=at).rev().find(|&k| filler.is_char_boundary(k)).unwrap();
                let mut s = alloc::string::String::from(&filler[..at]);
                s.push(ch);
                s.push_str(&filler[at..]);
                s.push(ch);
                assert_eq!(found(&s), [at as u32, (s.len() - ch.len_utf8()) as u32], "{ch:?} at {at}");
            }
        }
    }

    #[test]
    fn irregular_quotes() {
        let (f, s) = both("a,5\" pipe,c\n");
        assert_eq!(f.err(), Some(Stop::Irregular));
        assert_eq!(s.err(), Some(Stop::Quote));
        let (f, s) = both("a,b\rc\n");
        assert_eq!(f.err(), Some(Stop::Irregular));
        assert_eq!(s.err(), Some(Stop::Return));
        let (f, s) = both("\"ab\"c,d\n");
        assert_eq!(f.err(), Some(Stop::Irregular));
        assert_eq!(s.err(), Some(Stop::AfterQuote));
        let (f, s) = both("a,\"open\n");
        assert_eq!(f.err(), Some(Stop::Irregular));
        assert_eq!(s.err(), Some(Stop::Unclosed));
    }
}
