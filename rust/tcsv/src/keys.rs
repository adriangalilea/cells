//! The key check: every row whose key value another row holds too is a problem, the first
//! row included, each naming another row with that key. Values compare as stored, so `7`
//! and `007` in an `int`, `1.5` and `1.50` at a column's scale, and two spellings of one
//! instant are one key; blank and broken cells take no part.

use alloc::vec::Vec;

use crate::column::{Bits, Data, Problem, Why};

/// A value's bytes as stored: equal values, equal bytes, within one column.
fn bytes_of<T: Copy>(v: &T) -> &[u8] {
    // SAFETY: `T` is a plain number (`i16`, `i32`, `i64`, `i128`); every byte is initialized.
    unsafe { core::slice::from_raw_parts(v as *const T as *const u8, core::mem::size_of::<T>()) }
}

/// Every duplicate key of a column.
pub(crate) fn check(data: &Data, valid: &Bits, column: u32, out: &mut Vec<Problem>) {
    let n = valid.len;
    let ok = |i: usize| valid.get(i);
    match data {
        Data::Text { offsets, bytes } => {
            duplicates(n, |i| ok(i).then(|| &bytes[offsets[i] as usize..offsets[i + 1] as usize]), column, out)
        }
        Data::Int(v) => duplicates(n, |i| ok(i).then(|| bytes_of(&v[i])), column, out),
        Data::Decimal { values, .. } => duplicates(n, |i| ok(i).then(|| bytes_of(&values[i])), column, out),
        Data::Date(v) | Data::Time(v) => duplicates(n, |i| ok(i).then(|| bytes_of(&v[i])), column, out),
        Data::DateTime { values, .. } => duplicates(n, |i| ok(i).then(|| bytes_of(&values[i])), column, out),
        Data::Choice(v) => duplicates(n, |i| ok(i).then(|| bytes_of(&v[i])), column, out),
        Data::Bool(b) => {
            const WORDS: [[u8; 1]; 2] = [[0], [1]];
            duplicates(n, |i| ok(i).then(|| &WORDS[b.get(i) as usize][..]), column, out)
        }
    }
}

/// A 64-bit hash of a key's bytes, eight at a time.
#[inline]
fn hash(s: &[u8]) -> u64 {
    const K: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut h = (s.len() as u64).wrapping_mul(K);
    let mut chunks = s.chunks_exact(8);
    for c in &mut chunks {
        let w = u64::from_le_bytes(c.try_into().unwrap());
        h = (h ^ w).wrapping_mul(K).rotate_left(29);
    }
    let rest = chunks.remainder();
    if !rest.is_empty() {
        let mut b = [0u8; 8];
        b[..rest.len()].copy_from_slice(rest);
        h = (h ^ u64::from_le_bytes(b)).wrapping_mul(K).rotate_left(29);
    }
    h ^= h >> 32;
    h = h.wrapping_mul(K);
    h ^ (h >> 29)
}

/// Rows `0..n`, each key's bytes by `key(row)`, nothing for a cell in no comparison.
fn duplicates<'a>(n: usize, key: impl Fn(usize) -> Option<&'a [u8]>, column: u32, out: &mut Vec<Problem>) {
    let cap = (n * 2).next_power_of_two().max(16);
    let mask = cap - 1;
    // A slot: the hash's high half, then the row plus one; zero is empty.
    let mut slots: Vec<u64> = alloc::vec![0; cap];
    let mut reported: Vec<u64> = alloc::vec![0; n.div_ceil(64)];
    let start = out.len();
    for row in 0..n {
        let Some(t) = key(row) else { continue };
        let h = hash(t);
        let tag = h & 0xFFFF_FFFF_0000_0000;
        let mut i = (h as usize) & mask;
        loop {
            let s = slots[i];
            if s == 0 {
                slots[i] = tag | (row as u64 + 1);
                break;
            }
            let other = (s & 0xFFFF_FFFF) as usize - 1;
            if s & 0xFFFF_FFFF_0000_0000 == tag && key(other) == Some(t) {
                if reported[other / 64] >> (other % 64) & 1 == 0 {
                    reported[other / 64] |= 1 << (other % 64);
                    out.push(Problem { row: other as u32, column, why: Why::Duplicate { other: row as u32 } });
                }
                out.push(Problem { row: row as u32, column, why: Why::Duplicate { other: other as u32 } });
                break;
            }
            i = (i + 1) & mask;
        }
    }
    out[start..].sort_unstable_by_key(|p| p.row);
}
