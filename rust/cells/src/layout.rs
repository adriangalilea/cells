//! A file's tables laid out as tables (SPEC.md, Formatting): every column starting where
//! it starts on every line of its table. One rule, read three ways: `formatted` writes it
//! into the text (`cells fmt`, a language server's formatting on save), `layout` says
//! what a view draws to show it without writing a byte (a language server's inlay hints,
//! ui's code blocks), and a formatted file lays out with nothing to draw. Each table on
//! its own: the file's, and each written inline in its prelude. A record across a line
//! break is left as written, and its table is measured without it.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::tokens;

/// A column's width rounds up to a stop of this many characters, so a value that grows
/// within its stop moves nothing.
pub const STOP: u32 = 4;

/// A value wider than this, in characters, widens no column: its line catches up with the
/// column starts as soon as it can, and one long value never moves the table.
pub const CAP: u32 = 32;

/// Spaces drawn before a field's value, at `at` (the value's first byte), to bring it to
/// its column's start. Never in the file: what a view draws for a file not formatted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pad {
    pub at: u32,
    pub width: u32,
}

/// The middle of a value wider than a view's fold, `start..end` in bytes, drawn folded
/// until a reader opens it: the value's start and end stay, and a header cell's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cut {
    pub start: u32,
    pub end: u32,
}

/// One record of a table: where it starts, where the next does, and its fields' values
/// (their spans, padding cut, a quoted one with its quotes).
struct Record {
    at: usize,
    next: usize,
    fields: Vec<(usize, usize)>,
    /// Whether it stands on one line; one across a line break is left as written.
    one_line: bool,
}

/// Every table of a file, as the byte ranges of its lines: the lines its parts with a
/// column stand on, run together.
fn tables(source: &str) -> Vec<(usize, usize)> {
    let starts: Vec<usize> = core::iter::once(0).chain(source.match_indices('\n').map(|(i, _)| i + 1)).collect();
    let line_of = |at: usize| starts.partition_point(|&s| s <= at) - 1;
    let mut tabled = alloc::vec![false; starts.len()];
    for t in tokens::tokens(source) {
        if t.column != u32::MAX {
            let (first, last) = (line_of(t.start as usize), line_of((t.end as usize).saturating_sub(1).max(t.start as usize)));
            tabled[first..=last].fill(true);
        }
    }
    // A column's declaration has its column too, and is the prelude's, never a table's line.
    for line in declared_lines(source) {
        tabled[line] = false;
    }
    let mut out = Vec::new();
    let mut l = 0;
    while l < starts.len() {
        if !tabled[l] {
            l += 1;
            continue;
        }
        let first = l;
        while l < starts.len() && tabled[l] {
            l += 1;
        }
        // The file's first line starts after its byte order mark.
        let from = if starts[first] == 0 && source.starts_with('\u{feff}') { 3 } else { starts[first] };
        out.push((from, starts.get(l).map_or(source.len(), |&s| s)));
    }
    out
}

/// The prelude and the lines of its column declarations (SPEC.md, Columns), counted
/// from the file's first line; none for a file with no prelude, or one never closed.
fn prelude(source: &str) -> Option<(crate::prelude::Parsed, usize)> {
    let bom = if source.starts_with('\u{feff}') { 3 } else { 0 };
    let first = source[bom..].split_inclusive('\n').next()?;
    if first.trim_end_matches(['\n', '\r']) != "---" {
        return None;
    }
    crate::prelude::parse(source, bom).ok().map(|p| (p, bom))
}

fn declared_lines(source: &str) -> Vec<usize> {
    let Some((parsed, _)) = prelude(source) else { return Vec::new() };
    parsed.statements.iter().filter(|s| matches!(s, crate::prelude::Statement::Column { .. })).map(|s| s.line() as usize).collect()
}

fn records(source: &str, mut at: usize, end: usize) -> Vec<Record> {
    let src = source.as_bytes();
    let mut out = Vec::new();
    while at < end {
        let (fields, next) = tokens::record(src, at);
        let one_line = !src[at..next.min(end)].trim_ascii_end().contains(&b'\n');
        out.push(Record { at, next: next.min(end), fields, one_line });
        if next <= at {
            break;
        }
        at = next;
    }
    out
}

/// A value's width as drawn, in characters: its own, or the fold's when it is wider.
fn width(value: &str, fold: Option<u32>) -> u32 {
    let n = value.chars().count() as u32;
    fold.map_or(n, |f| n.min(f))
}

/// Where each column of a table starts, in characters from its line's start: the one
/// before's start, plus that column's widest value no wider than the cap, its comma and
/// one space, rounded up to a stop. `header`, the header's cells as they are to be written
/// when that is not as read (`formatted` spaces them), measured in its place.
fn column_starts(source: &str, records: &[Record], fold: Option<u32>, header: Option<&[String]>) -> Vec<u32> {
    let columns = records.iter().filter(|r| r.one_line).map(|r| r.fields.len()).max().unwrap_or(0);
    let mut starts = Vec::with_capacity(columns);
    let mut at = 0;
    for k in 0..columns {
        starts.push(at);
        let widest = records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.one_line)
            .filter_map(|(n, r)| match header {
                Some(h) if n == 0 => h.get(k).map(|v| width(v, fold)),
                _ => r.fields.get(k).map(|&(s, e)| width(&source[s..e], fold)),
            })
            .filter(|&w| w <= CAP)
            .max()
            .unwrap_or(0);
        at += (widest + 2).div_ceil(STOP) * STOP;
    }
    starts
}

/// The line break that ends a record, its padding before it cut: `\r\n`, `\n`, or none at
/// the end of the file.
fn ending(source: &str, record: &Record) -> &'static str {
    let tail = &source[record.fields.last().map_or(record.at, |f| f.1)..record.next];
    if tail.ends_with("\r\n") {
        "\r\n"
    } else if tail.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

/// The file as the format's standard writes it: every table's fields with no padding but
/// what brings each value to its column's start (`align`), or none at all (`align` off).
/// The prelude, a record across a line break, and every byte outside a table are written
/// as read. Formatting twice is formatting once.
pub fn formatted(source: &str, align: bool) -> String {
    // What changes, in the file's order: each table's records, each declaration's line.
    let mut edits: Vec<(usize, usize, String)> = declarations(source, align);
    for (from, to) in tables(source) {
        edits.push((from, to, table(source, from, to, align)));
    }
    edits.sort_by_key(|e| e.0);
    let mut out = String::with_capacity(source.len() + source.len() / 4);
    let mut pos = 0;
    for (from, to, text) in edits {
        out.push_str(&source[pos..from]);
        out.push_str(&text);
        pos = to;
    }
    out.push_str(&source[pos..]);
    out
}

/// The prelude's column declarations (SPEC.md, Columns) as formatting writes them, each
/// line's place and its text: `name: type = formula`, aligned, every type at one column
/// and every formula's `=` at one, or with `align` off a single space around each. A
/// comment after a declaration is kept, one space before its `//`.
fn declarations(source: &str, align: bool) -> Vec<(usize, usize, String)> {
    let Some((parsed, bom)) = prelude(source) else { return Vec::new() };
    let lines: Vec<(usize, &str)> = {
        let mut at = bom;
        source[bom..parsed.body]
            .split_inclusive('\n')
            .map(|l| {
                let here = at;
                at += l.len();
                (here, l)
            })
            .collect()
    };
    // Each declaration: its line, its name as written, its type, its formula.
    let mut found: Vec<(usize, String, &str, Option<&str>)> = Vec::new();
    for s in &parsed.statements {
        if let crate::prelude::Statement::Column { line, name, spelled } = s {
            let (ty, formula) = crate::types::split_formula(spelled);
            found.push((*line as usize, crate::prelude::declared_name(name).into_owned(), ty, formula));
        }
    }
    let chars = |s: &str| s.chars().count();
    let name_w = found.iter().map(|d| chars(&d.1)).max().unwrap_or(0);
    let type_w = found.iter().filter(|d| d.3.is_some()).map(|d| chars(d.2)).max().unwrap_or(0);
    found
        .into_iter()
        .filter_map(|(line, name, ty, formula)| {
            let &(at, raw) = lines.get(line)?;
            let body = raw.trim_end_matches(['\n', '\r']);
            let end = &raw[body.len()..];
            let comment = &body[crate::prelude::uncomment(body).len()..];
            let mut text = format!("{name}:");
            let gap = |used: usize, to: usize| if align { to.saturating_sub(used).max(1) } else { 1 };
            text.extend(core::iter::repeat_n(' ', gap(chars(&name) + 1, name_w + 2)));
            text.push_str(ty);
            if let Some(f) = formula {
                text.extend(core::iter::repeat_n(' ', gap(chars(ty), type_w + 1)));
                text.push_str("= ");
                text.push_str(f);
            }
            if !comment.is_empty() {
                text.push(' ');
                text.push_str(comment);
            }
            text.push_str(end);
            (text != raw).then_some((at, at + raw.len(), text))
        })
        .collect()
}

/// A header cell as formatting writes it: a typed one spaced as a column is written
/// everywhere (`prelude::header_cell`: `total: EUR = cost * qty`), its quotes kept when it
/// needs them; a name alone as read. What it means never changes: the spaces stand
/// between its name, type and formula, which read the same with or without them.
fn header_cell(value: &str) -> String {
    let quoted = value.len() >= 2 && value.starts_with('"') && value.ends_with('"');
    let inner = if quoted { value[1..value.len() - 1].replace("\"\"", "\"") } else { value.to_owned() };
    let Some(colon) = inner.find(':') else { return value.to_owned() };
    let spelled = crate::prelude::header_cell(&inner[..colon], &inner[colon + 1..]);
    if quoted { format!("\"{}\"", spelled.replace('"', "\"\"")) } else { spelled }
}

/// One table's records as formatting writes them, its header's cells spaced
/// (`header_cell`).
fn table(source: &str, from: usize, to: usize, align: bool) -> String {
    let mut out = String::with_capacity(to - from + (to - from) / 4);
    {
        let records = records(source, from, to);
        let header: Option<Vec<String>> =
            records.first().filter(|r| r.one_line).map(|r| r.fields.iter().map(|&(s, e)| header_cell(&source[s..e])).collect());
        let starts = column_starts(source, &records, None, header.as_deref());
        for (n, r) in records.iter().enumerate() {
            if !r.one_line {
                out.push_str(&source[r.at..r.next]);
                continue;
            }
            let mut col = 0;
            let last = r.fields.len() - 1;
            for (k, &(s, e)) in r.fields.iter().enumerate() {
                let value = match &header {
                    Some(h) if n == 0 => h[k].as_str(),
                    _ => &source[s..e],
                };
                if k > 0 {
                    out.push(',');
                    col += 1;
                    // A blank last field is the line's end: no spaces left trailing.
                    if align && !(k == last && value.is_empty()) {
                        let spaces = starts[k].saturating_sub(col).max(1);
                        out.extend(core::iter::repeat_n(' ', spaces as usize));
                        col += spaces;
                    }
                }
                out.push_str(value);
                col += value.chars().count() as u32;
            }
            out.push_str(ending(source, r));
        }
    }
    out
}

/// What a view draws to show a file's tables laid out without writing a byte: the
/// spaces that bring each value to its column's start, where the file has fewer; with a
/// `fold`, each value wider than it drawn that wide, its middle a `Cut`. A file
/// `formatted` has no pads.
pub fn layout(source: &str, fold: Option<u32>) -> (Vec<Pad>, Vec<Cut>) {
    let (mut pads, mut cuts) = (Vec::new(), Vec::new());
    for (from, to) in tables(source) {
        let records = records(source, from, to);
        let starts = column_starts(source, &records, fold, None);
        for (n, r) in records.iter().enumerate() {
            if !r.one_line {
                continue;
            }
            let mut col = 0;
            let mut cursor = r.at;
            for (k, &(s, e)) in r.fields.iter().enumerate() {
                // What stands before the value: its comma and the padding the file has.
                col += source[cursor..s].chars().count() as u32;
                if k > 0 && starts[k] > col {
                    pads.push(Pad { at: s as u32, width: starts[k] - col });
                    col = starts[k];
                }
                let value = &source[s..e];
                let chars = value.chars().count() as u32;
                match fold {
                    Some(f) if chars > f && f >= 8 => {
                        // The start keeps a header cell's name and its `:`; the end keeps the
                        // rest of the fold, the part that tells values apart.
                        let name = if n == 0 { value.find(':').map_or(0, |c| value[..c].chars().count() as u32 + 1) } else { 0 };
                        let head = name.max(f / 3).min(f - 4);
                        let tail = f - 1 - head;
                        let byte = |i: u32| s + value.char_indices().nth(i as usize).map_or(value.len(), |(b, _)| b);
                        cuts.push(Cut { start: byte(head) as u32, end: byte(chars - tail) as u32 });
                        col += f;
                    }
                    _ => col += chars,
                }
                cursor = e;
            }
        }
    }
    (pads, cuts)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text as a view lays it out: every pad drawn as its spaces, every cut as `…`.
    fn drawn(src: &str, fold: Option<u32>) -> String {
        let (pads, cuts) = layout(src, fold);
        let mut marks: Vec<(usize, Option<usize>, u32)> = pads.iter().map(|p| (p.at as usize, None, p.width)).collect();
        marks.extend(cuts.iter().map(|c| (c.start as usize, Some(c.end as usize), 0)));
        marks.sort();
        let mut out = String::new();
        let mut at = 0;
        for (from, to, width) in marks {
            out.push_str(&src[at..from]);
            match to {
                Some(to) => {
                    out.push('…');
                    at = to;
                }
                None => {
                    out.extend(core::iter::repeat_n(' ', width as usize));
                    at = from;
                }
            }
        }
        out.push_str(&src[at..]);
        out
    }

    const BUDGET: &str = "item: text!,cost: EUR,total: EUR = cost * 2\nfuel,84.20,168.40=\nsplit dinner,3.10,6.20=\n";

    #[test]
    fn columns_start_together_in_stops() {
        // item: 12 + 2 = 14, to 16; cost: `cost: EUR` 9 + 2 = 11, to 12.
        assert_eq!(
            formatted(BUDGET, true),
            "item: text!,    cost: EUR,  total: EUR = cost * 2\nfuel,           84.20,      168.40=\nsplit dinner,   3.10,       6.20=\n"
        );
        assert_eq!(formatted(BUDGET, false), BUDGET);
    }

    /// A header cell spelled tight is written spaced, as a declaration is; what it means,
    /// its quotes and a plain name are kept.
    #[test]
    fn a_header_cell_is_spaced_as_a_declaration_is() {
        let tight = "item:text!,cost:EUR,total:EUR=cost * 2\nfuel,84.20,168.40=\nsplit dinner,3.10,6.20=\n";
        assert_eq!(formatted(tight, false), BUDGET);
        let quoted = "\"r:EUR=ROUND(cost, 1)\",note,cost:EUR\n1.0=,x,1.00\n";
        assert_eq!(formatted(quoted, false), "\"r: EUR = ROUND(cost, 1)\",note,cost: EUR\n1.0=,x,1.00\n");
    }

    #[test]
    fn formatting_twice_is_formatting_once_and_a_formatted_file_draws_nothing() {
        for align in [true, false] {
            let once = formatted(BUDGET, align);
            assert_eq!(formatted(&once, align), once);
        }
        let aligned = formatted(BUDGET, true);
        assert_eq!(layout(&aligned, None), (Vec::new(), Vec::new()));
        assert_eq!(drawn(BUDGET, None), aligned, "a view draws what fmt writes");
        // Padding of any width reads back to the same layout.
        assert_eq!(formatted(&formatted(BUDGET, true).replace(", ", ",  "), true), aligned);
        assert_eq!(formatted(&aligned, false), BUDGET);
    }

    #[test]
    fn a_value_past_the_cap_widens_no_column() {
        let long = "a note far longer than the cap of thirty two";
        let src = alloc::format!("item,note,qty\nfuel,{long},1\nmaps,short,2\n");
        let out = formatted(&src, true);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "item,   note,   qty");
        assert_eq!(lines[2], "maps,   short,  2");
        // The long line resumes one space after its comma.
        assert_eq!(lines[1], alloc::format!("fuel,   {long}, 1"));
    }

    #[test]
    fn each_table_on_its_own_and_the_rest_as_read() {
        let src = "\u{feff}---\nfees = ```cells\nkind: text!,pct: pct\ncard,1.4\n```\ntype K = a|b\n---\na,b\r\nlonger,1\r\n\"x\ny\",2\n";
        assert_eq!(
            formatted(src, true),
            "\u{feff}---\nfees = ```cells\nkind: text!,    pct: pct\ncard,           1.4\n```\ntype K = a|b\n---\na,      b\r\nlonger, 1\r\n\"x\ny\",2\n"
        );
    }

    #[test]
    fn declarations_line_up_their_types_and_their_equals() {
        let src = "---\ntype Money = EUR>=0\nitem:text!   // the key\ncost : Money\ntotal:Money=cost * qty\n\"price, net\": EUR?\n---\nitem,cost,total,\"price, net\"\nfuel,1,1=,\n";
        assert_eq!(
            formatted(src, true),
            "---\ntype Money = EUR>=0\nitem:         text! // the key\ncost:         Money\ntotal:        Money = cost * qty\n\"price, net\": EUR?\n---\nitem,   cost,   total,  \"price, net\"\nfuel,   1,      1=,\n"
        );
        assert_eq!(
            formatted(src, false),
            "---\ntype Money = EUR>=0\nitem: text! // the key\ncost: Money\ntotal: Money = cost * qty\n\"price, net\": EUR?\n---\nitem,cost,total,\"price, net\"\nfuel,1,1=,\n"
        );
        let once = formatted(src, true);
        assert_eq!(formatted(&once, true), once);
    }

    #[test]
    fn a_blank_last_field_leaves_no_spaces() {
        assert_eq!(formatted("a,b,c\nlonger,,\n", true), "a,      b,  c\nlonger, ,\n");
    }

    #[test]
    fn a_value_past_the_fold_draws_folded() {
        let src = "running:EUR=running[-1] + total,note\n1=,a very long note about the ferry\n";
        let shown = drawn(src, Some(16));
        // The header cell keeps its name; the note its start and its end.
        assert_eq!(shown, "running:…+ total,   note\n1=,                 a ver… the ferry\n");
    }
}
