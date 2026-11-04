//! Records: RFC 4180 fields found 64 bytes at a time. Each block's quotes and separators
//! (`,` `\n` `\r`) become two bitmasks (NEON, SSE2, wasm SIMD, or a portable loop); the
//! quoted stretches are the prefix XOR of the quotes, and every separator outside them
//! ends a field. That reading is exact only while every quote stands where RFC 4180 puts
//! it, opening a field or closing one, and every `\r` ends a line with `\n` after it; a
//! block that breaks either is irregular, and the input is read again byte by byte
//! (`scalar`), which says which record breaks and how.

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

/// The quotes and the separators of 64 bytes, bit `k` for byte `k`.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn masks(b: &[u8; 64]) -> (u64, u64) {
    use core::arch::aarch64::*;
    // SAFETY: NEON is part of every aarch64 target; the loads read the 64 bytes of `b`.
    unsafe {
        let p = b.as_ptr();
        let v = [vld1q_u8(p), vld1q_u8(p.add(16)), vld1q_u8(p.add(32)), vld1q_u8(p.add(48))];
        let (q, c, lf, cr) = (vdupq_n_u8(b'"'), vdupq_n_u8(b','), vdupq_n_u8(b'\n'), vdupq_n_u8(b'\r'));
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
fn masks(b: &[u8; 64]) -> (u64, u64) {
    use core::arch::x86_64::*;
    // SAFETY: SSE2 is part of every x86_64 target; the loads read the 64 bytes of `b`.
    unsafe {
        let p = b.as_ptr() as *const __m128i;
        let (q, c, lf, cr) = (_mm_set1_epi8(b'"' as i8), _mm_set1_epi8(b',' as i8), _mm_set1_epi8(b'\n' as i8), _mm_set1_epi8(b'\r' as i8));
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
fn masks(b: &[u8; 64]) -> (u64, u64) {
    use core::arch::wasm32::*;
    let (q, c, lf, cr) = (u8x16_splat(b'"'), u8x16_splat(b','), u8x16_splat(b'\n'), u8x16_splat(b'\r'));
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
fn masks(b: &[u8; 64]) -> (u64, u64) {
    let (mut qm, mut sm) = (0u64, 0u64);
    for (k, &c) in b.iter().enumerate() {
        qm |= ((c == b'"') as u64) << k;
        sm |= ((c == b',' || c == b'\n' || c == b'\r') as u64) << k;
    }
    (qm, sm)
}

/// Reads the records of `src[from..to]`, `from` a record's start. `to` is a record's start
/// too, or the input's end, where a record without a line break ends.
pub(crate) fn fast<F: Fields>(src: &[u8], from: usize, to: usize, out: &mut F) -> Result<(), Stop> {
    debug_assert!(to <= src.len() && to <= u32::MAX as usize);
    let mut inside = 0u64; // all ones while the block before ended inside quotes
    let mut prev = 1u64; // the byte before is a separator, a quote, or the start
    let mut closer = 0u64; // the block before ended on a closing quote
    let mut field = from;
    let mut skip = usize::MAX; // the `\n` of a `\r\n` already counted
    let mut pos = from;
    let mut pad = [b','; 64];
    while pos < to {
        let n = (to - pos).min(64);
        let block: &[u8; 64] = if n == 64 {
            src[pos..pos + 64].try_into().unwrap()
        } else {
            pad.fill(b',');
            pad[..n].copy_from_slice(&src[pos..to]);
            &pad
        };
        let (q, s) = masks(block);
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
            match src[p] {
                b',' => {
                    out.field(field as u32, p as u32)?;
                    field = p + 1;
                }
                b'\n' => {
                    if p == skip {
                        continue;
                    }
                    out.field(field as u32, p as u32)?;
                    out.record(p as u32 + 1);
                    field = p + 1;
                }
                _ => {
                    if p + 1 >= to || src[p + 1] != b'\n' {
                        return Err(Stop::Irregular);
                    }
                    out.field(field as u32, p as u32)?;
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
        out.field(field as u32, to as u32)?;
        out.record(to as u32);
    }
    Ok(())
}

/// Reads the records of `src[from..to]` byte by byte: a field opening with `"` is quoted
/// to its closing quote, `""` a quote inside; any other quote, text after a closing quote,
/// a quote never closed and a lone `\r` stop it.
pub(crate) fn scalar<F: Fields>(src: &[u8], from: usize, to: usize, out: &mut F) -> Result<(), Stop> {
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
            if i < to && !matches!(src[i], b',' | b'\n' | b'\r') {
                return Err(Stop::AfterQuote);
            }
        } else {
            while i < to && !matches!(src[i], b',' | b'\n' | b'\r') {
                if src[i] == b'"' {
                    return Err(Stop::Quote);
                }
                i += 1;
            }
        }
        if i < to && src[i] == b'\r' && (i + 1 >= to || src[i + 1] != b'\n') {
            return Err(Stop::Return);
        }
        out.field(start as u32, i as u32)?;
        if i >= to {
            out.record(to as u32);
            return Ok(());
        }
        if src[i] == b',' {
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
        if i >= to {
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
}

/// The record starting at `from`: its fields' raw spans and where the next starts.
pub(crate) fn record(src: &[u8], from: usize) -> Result<(alloc::vec::Vec<(u32, u32)>, usize), Stop> {
    // The record ends at the first line break outside quotes; reading to it is enough.
    let mut quoted = false;
    let mut end = src.len();
    let mut i = from;
    while i < src.len() {
        let c = src[i];
        if c == b'"' {
            quoted = !quoted;
        } else if !quoted && (c == b'\n' || c == b'\r') {
            end = if c == b'\r' && src.get(i + 1) == Some(&b'\n') { i + 2 } else { i + 1 };
            break;
        }
        i += 1;
    }
    let mut one = One { fields: alloc::vec::Vec::new(), next: None };
    match scalar(src, from, end, &mut one) {
        // A stray quote can make the parity above end too early: read on to the input's end.
        Err(Stop::Unclosed) if end < src.len() => {
            let mut one = One { fields: alloc::vec::Vec::new(), next: None };
            scalar(src, from, src.len(), &mut one)?;
            Ok((one.fields, one.next.unwrap_or(src.len() as u32) as usize))
        }
        Err(e) => Err(e),
        Ok(()) => Ok((one.fields, one.next.unwrap_or(end as u32) as usize)),
    }
}

/// A field's text: its raw span unquoted, `""` read as `"`.
#[inline]
pub(crate) fn text<'a>(src: &'a [u8], start: u32, end: u32, scratch: &'a mut alloc::vec::Vec<u8>) -> &'a [u8] {
    let raw = &src[start as usize..end as usize];
    if raw.first() != Some(&b'"') {
        return raw;
    }
    let inner = &raw[1..raw.len() - 1];
    if !inner.contains(&b'"') {
        return inner;
    }
    scratch.clear();
    let mut k = 0;
    while k < inner.len() {
        scratch.push(inner[k]);
        k += if inner[k] == b'"' { 2 } else { 1 };
    }
    scratch
}

/// A cell's text in the file's form: quoted only when it holds `,`, `"` or a line break.
pub fn quote(text: &str) -> alloc::borrow::Cow<'_, str> {
    if !text.bytes().any(|c| matches!(c, b',' | b'"' | b'\n' | b'\r')) {
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
        let b = src.as_bytes();
        let mut f = Collect::default();
        let fr = fast(b, 0, b.len(), &mut f).map(|_| f);
        let mut s = Collect::default();
        let sr = scalar(b, 0, b.len(), &mut s).map(|_| s);
        (fr, sr)
    }

    #[test]
    fn fast_equals_scalar() {
        let long = "x".repeat(70);
        let cases = ["a,b\n1,2\n", "a,b\r\n1,2", "a,\"b,\"\"c\"\"\nd\"\n1,2\n", "\"\",,\n", "a\r\nb\r\n", "a,b,\n", "\n\n", long.as_str()];
        for c in cases {
            let (f, s) = both(c);
            let (f, s) = (f.unwrap(), s.unwrap());
            assert_eq!(f.records, s.records, "{c:?}");
            assert_eq!(f.nexts, s.nexts, "{c:?}");
        }
        // Long input across many blocks, quotes crossing block edges.
        let mut big = alloc::string::String::new();
        for k in 0..500 {
            big.push_str(&alloc::format!("{k},\"q,{k}\r\n\"\"x\"\"\",{}\n", "y".repeat(k % 70)));
        }
        let (f, s) = both(&big);
        assert_eq!(f.unwrap().records, s.unwrap().records);
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
