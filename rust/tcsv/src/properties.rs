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

const TYPES: [&str; 12] =
    ["text", "text?", "int?", "EUR?", "num>=0", "yes/no?", "a|b|'c d'", "date?", "datetime?", "time(HHhmm)?", "pct?", ""];

fn cell(rng: &mut Rng) -> String {
    let pool = [
        "",
        "",
        "a",
        "b",
        "c d",
        "12",
        "-3",
        "0042",
        "84.20",
        "1e3",
        "yes",
        "no",
        "true",
        "2026-08-14",
        "2026-02-30",
        "14h30",
        "12%",
        "x,y",
        "say \"hi\"",
        "line\nbreak",
        "crlf\r\nbreak",
        "café",
        "2026-08-14T14:30:00.25+02:00",
        "k7m2q9xa",
        "tab\there",
        "nul\u{0}",
        "\u{1b}[1m",
        "c1\u{85}",
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
            let c = rng.below(doc.columns().len() as u64) as usize;
            let t = cell(&mut rng);
            // Text holding a control character is refused, the doc untouched.
            assert_eq!(doc.set_cell(r, c, &t).is_err(), crate::column::control(t.as_bytes()).is_some());
        }
        let again = crate::read(doc.write()).unwrap_or_else(|e| panic!("{:?}: {e}", doc.write()));
        assert_eq!(again.rows(), doc.rows(), "{src:?} became {:?}", doc.write());
        for r in 0..doc.rows() {
            for c in 0..doc.columns().len() {
                assert_eq!(again.text(r, c), doc.text(r, c), "{:?}", doc.write());
                assert_eq!(again.value(r, c), doc.value(r, c), "row {r} column {c} of {:?}", doc.write());
            }
        }
        let ours: Vec<_> = doc.problems().iter().map(|p| (p.row, p.column, p.kind())).collect();
        let theirs: Vec<_> = again.problems().iter().map(|p| (p.row, p.column, p.kind())).collect();
        assert_eq!(ours, theirs, "{:?}", doc.write());
    }
}

#[test]
fn rows_rearranged_keep_their_cells() {
    use crate::Row;
    let mut rng = Rng(23);
    for _ in 0..2000 {
        let src = file(&mut rng);
        let doc = crate::read(src.clone()).unwrap();
        if doc.rows() == 0 {
            continue;
        }
        // Reversed, the first row dropped, a new row in the middle.
        let blank: Vec<&str> = (0..doc.columns().len()).map(|_| "").collect();
        let mut order: Vec<Row> = (1..doc.rows()).rev().map(Row::Kept).collect();
        order.insert(order.len() / 2, Row::New(&blank));
        let after = doc.rearranged(&order, 1).unwrap_or_else(|e| panic!("{src:?}: {e}"));
        assert_eq!(after.rows(), order.len(), "{src:?} became {:?}", after.write());
        for (r, o) in order.iter().enumerate() {
            for c in 0..doc.columns().len() {
                let want = match o {
                    Row::Kept(k) => doc.text(*k, c).into_owned(),
                    Row::New(_) => String::new(),
                };
                assert_eq!(after.text(r, c), want, "row {r} of {:?}", after.write());
            }
        }
    }
}

#[test]
fn a_tail_read_alone_is_the_whole_read() {
    let mut rng = Rng(11);
    for _ in 0..3000 {
        let src = file(&mut rng);
        let whole = crate::read(src.clone()).unwrap();
        if whole.rows() == 0 {
            continue;
        }
        let k = rng.below(whole.rows() as u64) as usize;
        let cut = whole.records()[k] as usize;
        let prefix = crate::read(&src.as_bytes()[..cut]).unwrap();
        let grown = prefix.extend(src.clone().into_bytes()).unwrap();
        assert_eq!(grown.write(), src);
        assert_eq!(grown.rows(), whole.rows());
        assert_eq!(grown.problems(), whole.problems(), "{src:?} cut at {cut}");
        for c in 0..whole.columns().len() {
            assert_eq!(grown.columns()[c].ty, whole.columns()[c].ty, "{src:?}");
            for r in 0..whole.rows() {
                assert_eq!(grown.value(r, c), whole.value(r, c), "row {r} column {c} of {src:?} cut at {cut}");
            }
        }
    }
}

/// A plain file as people hold them: any separator, a name blank or twice, quotes out of
/// place, a quote never closed, rows of any width, lone `\r` line ends, every field quoted.
fn plain(rng: &mut Rng) -> (String, crate::Dialect) {
    use crate::Separator;
    let sep = *rng.pick(&Separator::ALL);
    let s = sep.byte() as char;
    let cols = 1 + rng.below(5) as usize;
    let all = rng.below(5) == 0;
    let cr = rng.below(8) == 0;
    let field = |rng: &mut Rng, out: &mut String, text: &str| {
        let needs = text.contains([s, '"', '\n', '\r']);
        match rng.below(12) {
            _ if all || needs => {
                out.push('"');
                out.push_str(&text.replace('"', "\"\""));
                out.push('"');
            }
            0 => out.push_str("5\" pipe"),
            1 => out.push_str("\"ab\"c"),
            _ => out.push_str(text),
        }
    };
    let mut out = String::new();
    if rng.below(4) == 0 {
        out.push('\u{feff}');
    }
    for c in 0..cols {
        if c > 0 {
            out.push(s);
        }
        let name = match rng.below(8) {
            0 => String::new(),
            1 => String::from("c0"),
            _ => format!("c{c}"),
        };
        field(rng, &mut out, &name);
    }
    let rows = rng.below(30);
    for _ in 0..rows {
        out.push_str(if cr {
            "\r"
        } else if rng.below(3) == 0 {
            "\r\n"
        } else {
            "\n"
        });
        let n = match rng.below(6) {
            0 => 1 + rng.below(cols as u64 + 2) as usize,
            _ => cols,
        };
        for f in 0..n {
            if f > 0 {
                out.push(s);
            }
            let text = cell(rng);
            field(rng, &mut out, &text);
        }
    }
    match rng.below(6) {
        0 => out.push_str(&format!("{s}\"never closed")),
        1 | 2 => out.push_str(if cr { "\r" } else { "\n" }),
        _ => {}
    }
    (out, crate::Dialect::Plain(sep))
}

#[test]
fn a_plain_file_reads_and_is_written_back() {
    let mut rng = Rng(5);
    for _ in 0..3000 {
        let (src, dialect) = plain(&mut rng);
        let doc = crate::read_as(src.clone(), dialect, 1).unwrap_or_else(|e| panic!("{src:?}: {e}"));
        assert_eq!(doc.write(), src);
        for r in 0..doc.rows() {
            let ragged = doc.problems().iter().any(|p| p.row as usize == r && p.kind() == crate::Kind::Ragged);
            assert_eq!(ragged, doc.width(r) != doc.columns().len(), "row {r} of {src:?}");
        }
        // A plain file's columns are inferred: a cell is broken only by a control character.
        let only = |p: &crate::Problem| p.kind() == crate::Kind::Ragged || matches!(p.why, crate::Why::Control { .. });
        assert!(doc.problems().iter().all(only), "{src:?}");
    }
}

#[test]
fn a_plain_edit_reads_back_as_written() {
    let mut rng = Rng(17);
    for _ in 0..2000 {
        let (src, dialect) = plain(&mut rng);
        let mut doc = crate::read_as(src.clone(), dialect, 1).unwrap();
        if doc.rows() == 0 {
            continue;
        }
        for _ in 0..1 + rng.below(4) {
            let r = rng.below(doc.rows() as u64) as usize;
            let c = rng.below(doc.columns().len() as u64) as usize;
            let t = cell(&mut rng);
            // Text holding a control character is refused, the doc untouched.
            assert_eq!(doc.set_cell(r, c, &t).is_err(), crate::column::control(t.as_bytes()).is_some());
        }
        let again = crate::read_as(doc.write(), dialect, 1).unwrap();
        assert_eq!(again.rows(), doc.rows(), "{src:?} became {:?}", doc.write());
        for r in 0..doc.rows() {
            assert_eq!(again.texts(r), doc.texts(r), "{:?}", doc.write());
        }
        assert_eq!(again.problems(), doc.problems(), "{:?}", doc.write());
        for c in 0..doc.columns().len() {
            assert_eq!(again.columns()[c].ty, doc.columns()[c].ty, "{:?}", doc.write());
            for r in 0..doc.rows() {
                assert_eq!(again.value(r, c), doc.value(r, c), "row {r} column {c} of {:?}", doc.write());
            }
        }
    }
}

#[test]
fn columns_reshaped_keep_their_cells() {
    use crate::ColumnAt;
    let mut rng = Rng(29);
    for k in 0..3000 {
        let (src, dialect) = if k % 2 == 0 { (file(&mut rng), crate::Dialect::Tcsv) } else { plain(&mut rng) };
        let doc = crate::read_as(src.clone(), dialect, 1).unwrap();
        if doc.columns().is_empty() {
            continue;
        }
        // Reversed, the first renamed, a new one in the middle, the last taken out.
        let cells: Vec<String> = (0..doc.rows()).map(|r| format!("n{r}")).collect();
        let cells: Vec<&str> = cells.iter().map(String::as_str).collect();
        let mut order: Vec<ColumnAt> = (0..doc.columns().len()).rev().map(ColumnAt::Kept).collect();
        order[doc.columns().len() - 1] = ColumnAt::Renamed(0, "first");
        if doc.columns().len() > 1 {
            order.remove(0);
        }
        order.insert(order.len() / 2, ColumnAt::New("fresh", &cells));
        let after = doc.reshaped(&order, 1).unwrap_or_else(|e| panic!("{src:?}: {e}"));
        assert_eq!(after.rows(), doc.rows(), "{src:?} became {:?}", after.write());
        assert_eq!(after.columns().len(), order.len());
        for (c, o) in order.iter().enumerate() {
            let name = match o {
                ColumnAt::Kept(j) => doc.columns()[*j].name.as_str(),
                ColumnAt::Renamed(_, n) | ColumnAt::New(n, _) => n,
            };
            assert_eq!(after.columns()[c].name, name, "{src:?} became {:?}", after.write());
            for r in 0..doc.rows() {
                let want = match o {
                    ColumnAt::Kept(j) | ColumnAt::Renamed(j, _) => doc.text(r, *j).into_owned(),
                    ColumnAt::New(..) => format!("n{r}"),
                };
                assert_eq!(after.text(r, c), want, "row {r} of {src:?} became {:?}", after.write());
            }
        }
    }
}

#[test]
fn a_file_written_as_another_reads_back_its_cells() {
    use crate::{Dialect, Separator};
    let mut rng = Rng(31);
    for k in 0..3000 {
        let (src, dialect) = if k % 2 == 0 { (file(&mut rng), Dialect::Tcsv) } else { plain(&mut rng) };
        let doc = crate::read_as(src.clone(), dialect, 1).unwrap();
        let to = Dialect::Plain(*rng.pick(&Separator::ALL));
        let text = doc.written_as(to, None);
        let again = crate::read_as(text.clone(), to, 1).unwrap();
        assert_eq!(again.rows(), doc.rows(), "{src:?} as {to:?}: {text:?}");
        let names: Vec<&str> = doc.columns().iter().map(|c| c.name.as_str()).collect();
        let theirs: Vec<&str> = again.columns().iter().map(|c| c.name.as_str()).collect();
        assert_eq!(theirs, names, "{src:?} as {to:?}: {text:?}");
        for r in 0..doc.rows() {
            let texts = doc.texts(r);
            assert_eq!(&again.texts(r)[..doc.columns().len()], &texts[..], "{src:?} as {to:?}: {text:?}");
        }
        if dialect == Dialect::Tcsv {
            continue;
        }
        // A plain file as TCSV: its rows and cells, a ragged row's extra fields gone.
        let text = doc.written_as(Dialect::Tcsv, None);
        match crate::read(text.clone()) {
            Err(e) => assert!(!doc.naming().is_empty(), "{src:?} as TCSV: {text:?}: {e}"),
            Ok(again) => {
                assert_eq!(again.rows(), doc.rows(), "{src:?} as TCSV: {text:?}");
                for r in 0..doc.rows() {
                    assert_eq!(again.texts(r), doc.texts(r), "{src:?} as TCSV: {text:?}");
                }
            }
        }
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
    let parts = crate::read::body(src, first, &decoders_of(&doc), crate::scan::Records::TCSV, 4).unwrap();
    assert!(parts.len() > 1, "the body was not split");
    let seq = crate::read::sequential(src, first, &decoders_of(&doc), crate::scan::Records::TCSV).unwrap();
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

/// A plain file large enough to be split, its ragged rows' problems renumbered by part.
#[cfg(feature = "std")]
#[test]
fn a_plain_file_in_parallel_is_read_as_in_sequence() {
    use crate::{Dialect, Separator};
    let mut rng = Rng(43);
    let mut s = String::from("item;cost;note\n");
    for r in 0..120_000 {
        match rng.below(200) {
            0 => s.push_str(&format!("{r};1,5\n")),
            1 => s.push_str(&format!("{r};2;\"x; {r}\";extra\n")),
            _ => s.push_str(&format!("{r};{},{:02};\"a; {r}\"\n", rng.below(100), rng.below(100))),
        }
    }
    let dialect = Dialect::Plain(Separator::Semicolon);
    let one = crate::read_as(s.clone(), dialect, 1).unwrap();
    let many = crate::read_as(s.clone(), dialect, 8).unwrap();
    assert!(!one.problems().is_empty());
    assert_eq!(one.problems(), many.problems());
    assert_eq!(one.records(), many.records());
    for c in 0..one.columns().len() {
        assert_eq!(one.columns()[c].ty, many.columns()[c].ty);
        assert_eq!(one.columns()[c].valid.bytes, many.columns()[c].valid.bytes);
    }
    assert_eq!(crate::detect(s.as_bytes()), Separator::Semicolon);
}

#[cfg(feature = "std")]
fn decoders_of(doc: &crate::Doc) -> Vec<crate::read::Cells> {
    doc.columns()
        .iter()
        .map(|c| {
            let ty = if c.ty.inferred { None } else { Some(&c.ty) };
            crate::read::Cells {
                decoder: crate::column::Decoder::of(ty),
                optional: ty.is_none_or(|t| t.optional),
                computed: c.formula.is_some(),
            }
        })
        .collect()
}
