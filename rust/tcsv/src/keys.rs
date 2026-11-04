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

/// Every duplicate key of a column, on up to `threads` threads.
pub(crate) fn check(data: &Data, valid: &Bits, column: u32, threads: usize, out: &mut Vec<Problem>) {
    let n = valid.len;
    let ok = |i: usize| valid.get(i);
    match data {
        Data::Text { offsets, bytes } => {
            duplicates(n, |i| ok(i).then(|| &bytes[offsets[i] as usize..offsets[i + 1] as usize]), column, threads, out)
        }
        Data::Int(v) => duplicates(n, |i| ok(i).then(|| bytes_of(&v[i])), column, threads, out),
        Data::Decimal { values, .. } => duplicates(n, |i| ok(i).then(|| bytes_of(&values[i])), column, threads, out),
        Data::Date(v) | Data::Time(v) => duplicates(n, |i| ok(i).then(|| bytes_of(&v[i])), column, threads, out),
        Data::DateTime { values, .. } => duplicates(n, |i| ok(i).then(|| bytes_of(&values[i])), column, threads, out),
        Data::Choice(v) => duplicates(n, |i| ok(i).then(|| bytes_of(&v[i])), column, threads, out),
        Data::Bool(b) => {
            const WORDS: [[u8; 1]; 2] = [[0], [1]];
            duplicates(n, |i| ok(i).then(|| &WORDS[b.get(i) as usize][..]), column, threads, out)
        }
    }
}

/// A 64-bit hash of a key's bytes, eight at a time.
#[inline]
pub(crate) fn hash(s: &[u8]) -> u64 {
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

/// Rows `0..n`, each key's bytes by `key(row)`, nothing for a cell in no comparison. On
/// threads the rows are split by their hash's top bits, so equal keys meet in one part and
/// each part's table is a fraction of the whole.
fn duplicates<'a, K>(n: usize, key: K, column: u32, threads: usize, out: &mut Vec<Problem>)
where
    K: Fn(usize) -> Option<&'a [u8]> + Sync,
{
    assert!(threads >= 1);
    let start = out.len();
    #[cfg(feature = "std")]
    if threads > 1 && n >= 1 << 16 {
        let parts = threads.next_power_of_two().min(64);
        let bits = parts.trailing_zeros();
        let chunk = n.div_ceil(threads);
        // Zero for a cell in no comparison; a real hash of zero is moved to one.
        let hashes: Vec<u64> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..n)
                .step_by(chunk)
                .map(|a| {
                    let key = &key;
                    s.spawn(move || (a..(a + chunk).min(n)).map(|r| key(r).map_or(0, |t| hash(t).max(1))).collect::<Vec<u64>>())
                })
                .collect();
            hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        let found: Vec<Vec<Problem>> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..parts as u64)
                .map(|p| {
                    let (key, hashes) = (&key, &hashes);
                    s.spawn(move || {
                        let mine = (0..n).filter(|&r| hashes[r] != 0 && hashes[r] >> (64 - bits) == p);
                        let count = mine.clone().count();
                        let mut found = Vec::new();
                        insert(mine, count, |r| hashes[r], key, column, &mut found);
                        found
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        out.extend(found.into_iter().flatten());
        out[start..].sort_unstable_by_key(|p| p.row);
        return;
    }
    insert((0..n).filter(|&r| key(r).is_some()), n, |r| hash(key(r).unwrap()), &key, column, out);
    out[start..].sort_unstable_by_key(|p| p.row);
}

/// Inserts `rows` (at most `most` of them) into one open-addressing table, reporting each
/// duplicate and, once, the row it repeats. A slot holds the hash's top 31 bits, a bit for
/// "reported", and the row plus one; zero is empty. The table is filled to 80% at most.
fn insert<'a>(
    rows: impl Iterator<Item = usize>,
    most: usize,
    hash_of: impl Fn(usize) -> u64,
    key: &impl Fn(usize) -> Option<&'a [u8]>,
    column: u32,
    out: &mut Vec<Problem>,
) {
    const TAG: u64 = 0xFFFF_FFFE_0000_0000;
    const REPORTED: u64 = 1 << 32;
    let cap = (most + most / 4 + 1).next_power_of_two().max(16);
    let mask = cap - 1;
    let mut slots: Vec<u64> = alloc::vec![0; cap];
    for row in rows {
        let h = hash_of(row);
        let tag = h & TAG;
        let mut i = (h as usize) & mask;
        loop {
            let s = slots[i];
            if s == 0 {
                slots[i] = tag | (row as u64 + 1);
                break;
            }
            let other = (s & 0xFFFF_FFFF) as usize - 1;
            if s & TAG == tag && key(other) == key(row) {
                if s & REPORTED == 0 {
                    slots[i] = s | REPORTED;
                    out.push(Problem { row: other as u32, column, why: Why::Duplicate { other: row as u32 } });
                }
                out.push(Problem { row: row as u32, column, why: Why::Duplicate { other: other as u32 } });
                break;
            }
            i = (i + 1) & mask;
        }
    }
}
