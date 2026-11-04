//! Properties over generated files: a read file written back is the same bytes, an edit
//! reads back as written, and the parallel reading equals the sequential one.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::column::Value;

/// xorshift64*, seeded: the same files every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len() as u64) as usize]
    }
}

const TYPES: [&str; 12] = ["text", "text?", "int?", "EUR?", "num>=0", "yes/no?", "a|b|'c d'", "date?", "datetime?", "time(HHhmm)?", "pct?", ""];

fn cell(rng: &mut Rng) -> String {
    let pool = [
        "", "", "a", "b", "c d", "12", "-3", "0042", "84.20", "1e3", "yes", "no", "true", "2026-08-14", "2026-02-30", "14h30", "12%", "x,y", "say \"hi\"",
        "line\nbreak", "crlf\r\nbreak", "café", "2026-08-14T14:30:00.25+02:00", "k7m2q9xa",
    ];
    String::from(*rng.pick(&pool))
}

fn file(rng: &mut Rng) -> String {
    let cols = 1 + rng.below(5) as usize;
    let mut s = String::new();
    if rng.below(4) == 0 {
        s.push('\u{feff}');
    }
    if rng.below(3) == 0 {
        s.push_str("---\n// a note\ntype K = a|b\n---\n");
    }
    for c in 0..cols {
        if c > 0 {
            s.push(',');
        }
        let t = rng.pick(&TYPES);
        if t.is_empty() { s.push_str(&format!("c{c}")) } else { s.push_str(&format!("c{c}:{t}")) }
    }
    let rows = rng.below(30);
    for _ in 0..rows {
        s.push_str(if rng.below(3) == 0 { "\r\n" } else { "\n" });
        let n = 1 + rng.below(cols as u64) as usize;
        for f in 0..n {
            if f > 0 {
                s.push(',');
            }
            let text = cell(rng);
            if rng.below(5) == 0 && !text.contains(['"', ',', '\n', '\r']) {
                s.push('"');
                s.push_str(&text);
                s.push('"');
            } else {
                s.push_str(&crate::quote(&text));
            }
        }
    }
    if rng.below(2) == 0 {
        s.push('\n');
    }
    s
}

#[test]
fn written_back_is_the_same_bytes() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..3000 {
        let src = file(&mut rng);
        let doc = crate::read(src.clone()).unwrap_or_else(|e| panic!("{src:?}: {e}"));
        assert_eq!(doc.write(), src);
    }
}

#[test]
fn an_edit_reads_back_as_written() {
    let mut rng = Rng(7);
    for _ in 0..2000 {
        let src = file(&mut rng);
        let mut doc = crate::read(src.clone()).unwrap();
        if doc.rows() == 0 {
            continue;
        }
        let edits = 1 + rng.below(4);
        for _ in 0..edits {
            let r = rng.below(doc.rows() as u64) as usize;
            let c = rng.below(doc.stored() as u64) as usize;
            doc.set_cell(r, c, &cell(&mut rng));
        }
        let again = crate::read(doc.write()).unwrap_or_else(|e| panic!("{:?}: {e}", doc.write()));
        assert_eq!(again.rows(), doc.rows(), "{src:?} became {:?}", doc.write());
        for r in 0..doc.rows() {
            for c in 0..doc.stored() {
                assert_eq!(again.text(r, c), doc.text(r, c), "{:?}", doc.write());
                if !doc.columns()[c].ty.inferred {
                    assert_eq!(again.value(r, c), doc.value(r, c), "row {r} column {c} of {:?}", doc.write());
                }
            }
        }
        let ours: Vec<_> = doc.problems().iter().map(|p| (p.row, p.column, p.kind())).collect();
        let theirs: Vec<_> = again.problems().iter().map(|p| (p.row, p.column, p.kind())).collect();
        assert_eq!(ours, theirs, "{:?}", doc.write());
    }
}

/// A body large enough to be split, whose parts disagree on scales and units.
#[cfg(feature = "std")]
#[test]
fn parallel_equals_sequential() {
    let mut rng = Rng(42);
    let mut s = String::from("id:int!,cost:EUR?,at:datetime?,kind:a|b,note:text?,n\n");
    let rows = 120_000;
    for r in 0..rows {
        let cost = match rng.below(1000) {
            0 => String::from("1.123456789"),
            1 => String::from("bad"),
            _ => format!("{}.{:02}", rng.below(10_000), rng.below(100)),
        };
        let at = match rng.below(500) {
            0 => String::from("2026-08-14T14:30:00.123456789Z"),
            1 => String::from("2026-08-14T14:30:00.5Z"),
            _ => String::from("2026-08-14T14:30:00Z"),
        };
        let id = if rng.below(10_000) == 0 { 5 } else { r };
        s.push_str(&format!("{id},{cost},{at},{},\"x, {r}\",{}\n", if r % 3 == 0 { "a" } else { "b" }, rng.below(100)));
    }
    let src = s.as_bytes();
    let doc = crate::read(s.clone()).unwrap();
    assert!(doc.rows() == rows as usize);
    let first = src.iter().position(|&c| c == b'\n').unwrap() + 1;
    let parts = crate::read::body(src, first, &decoders_of(&doc)).unwrap();
    assert!(parts.len() > 1, "the body was not split");
    let seq = crate::read::sequential(src, first, &decoders_of(&doc)).unwrap();
    let joined = crate::read::join(parts);
    assert_eq!(joined.records, seq.records);
    let mut a = joined.problems.clone();
    let mut b = seq.problems.clone();
    a.sort_by_key(|p| p.order());
    b.sort_by_key(|p| p.order());
    assert_eq!(a, b);
    for (x, y) in joined.columns.into_iter().zip(seq.columns) {
        assert_eq!(x.valid.bytes, y.valid.bytes);
        assert_eq!(x.building.finish(), y.building.finish());
    }
    // Spot values the reading kept.
    assert!(matches!(doc.value(0, 1), Value::Decimal(_)));
}

#[cfg(feature = "std")]
fn decoders_of(doc: &crate::Doc) -> Vec<(crate::column::Decoder, bool)> {
    doc.columns()[..doc.stored()]
        .iter()
        .map(|c| {
            let ty = if c.ty.inferred { None } else { Some(&c.ty) };
            (crate::column::Decoder::of(ty), ty.is_none_or(|t| t.optional))
        })
        .collect()
}
