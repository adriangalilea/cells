//! A spreadsheet's formula written in names (SPEC.md, A formula in a cell: a spreadsheet's
//! references are input, never stored). A client takes `=C2*D2` or `=SUM($E$2:E2)` as a
//! person types it and writes it as the file holds it: a column by the header's name for
//! its letter, this row bare, another row by where it is from this one, the first data row
//! (a spreadsheet's 2) `FIRST`, a fixed row by its key, a whole column by its name. Pure: it
//! reads only what it is given.

use alloc::format;
use alloc::string::{String, ToString};

/// What a rewrite reads of the file: its columns' names in the header's order, how many
/// rows it has, and each row's key when it has one.
pub struct Sheet<'a> {
    pub columns: &'a [&'a str],
    pub rows: usize,
    pub key: &'a dyn Fn(usize) -> Option<String>,
}

/// One end of a reference: its column (A is 0), its row as a spreadsheet numbers it, and
/// whether each is fixed (`$`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ref {
    column: usize,
    fixed_column: bool,
    row: Option<(usize, bool)>,
}

/// `src` (a formula as a spreadsheet writes it, its leading `=` or not) on the data row
/// `row` (from 0, a spreadsheet's `row + 2`), its references written in names; nothing
/// when it holds none, or one this file has no column for, or one names cannot say (a
/// range across columns).
pub fn from_sheet(src: &str, row: usize, sheet: &Sheet) -> Option<String> {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len() + 16);
    let mut i = 0;
    let mut changed = false;
    while i < b.len() {
        let c = b[i];
        // Text and quoted names hold no reference.
        if c == b'\'' || c == b'"' {
            let end = b[i + 1..].iter().position(|&d| d == c).map_or(b.len(), |k| i + 2 + k);
            out.push_str(&src[i..end]);
            i = end;
            continue;
        }
        let starts = (c == b'$' || c.is_ascii_uppercase()) && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_'));
        if starts && let Some((a, after)) = reference(b, i) {
            // A range: two references joined by `:`.
            let (text, next) = match (b.get(after), reference(b, after + 1)) {
                (Some(b':'), Some((z, end))) => (range(a, z, row, sheet)?, end),
                _ => (single(a, row, sheet)?, after),
            };
            out.push_str(&text);
            changed = true;
            i = next;
            continue;
        }
        let ch = src[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    changed.then_some(out)
}

/// A reference at `i`: `$?LETTERS` then `$?DIGITS` or nothing (a whole column), and where
/// it ends; nothing when the word there is no reference (a function's name, a name).
fn reference(b: &[u8], mut i: usize) -> Option<(Ref, usize)> {
    let fixed_column = b.get(i) == Some(&b'$');
    i += fixed_column as usize;
    let from = i;
    while i < b.len() && b[i].is_ascii_uppercase() {
        i += 1;
    }
    if i == from || i - from > 3 {
        return None;
    }
    let column = b[from..i].iter().fold(0usize, |n, &c| n * 26 + (c - b'A' + 1) as usize) - 1;
    let fixed_row = b.get(i) == Some(&b'$');
    let digits = i + fixed_row as usize;
    let mut end = digits;
    while end < b.len() && b[end].is_ascii_digit() {
        end += 1;
    }
    // A letter or `_` right after is a longer word, never a reference; `(` a function.
    if b.get(end).is_some_and(|&c| c.is_ascii_alphanumeric() || c == b'_' || c == b'(') {
        return None;
    }
    if end == digits {
        // Letters alone are a whole column only as a range's end: `E:E`.
        return (!fixed_row && !fixed_column).then_some((Ref { column, fixed_column, row: None }, i));
    }
    let n: usize = core::str::from_utf8(&b[digits..end]).ok()?.parse().ok()?;
    Some((Ref { column, fixed_column, row: Some((n, fixed_row)) }, end))
}

/// A column's name as a formula writes it: bare when it is a word of letters, digits and
/// `_` in lower case, quoted otherwise.
fn named(name: &str) -> String {
    let word = !name.is_empty()
        && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        && !name.as_bytes()[0].is_ascii_digit()
        && name.bytes().any(|c| c.is_ascii_lowercase());
    if word { name.to_string() } else { format!("\"{}\"", name.replace('"', "\"\"")) }
}

/// Where a spreadsheet's row `n` is from the data row `row`, in the file's words: `FIRST`
/// for a fixed 2, `THIS`, or a row by where it is (`-1`, `+2`).
fn place(n: usize, fixed: bool, row: usize) -> Option<String> {
    let data = n.checked_sub(2)?;
    if fixed && data == 0 {
        return Some("FIRST".into());
    }
    let by = data as i64 - row as i64;
    Some(match by {
        0 => "THIS".into(),
        n if n < 0 => format!("{n}"),
        n => format!("+{n}"),
    })
}

fn single(r: Ref, row: usize, sheet: &Sheet) -> Option<String> {
    let name = named(sheet.columns.get(r.column)?);
    let (n, fixed) = r.row?;
    let data = n.checked_sub(2)?;
    if data >= sheet.rows {
        return None;
    }
    // A fixed row other than the first is that row, by its key when it has one.
    if fixed
        && data != 0
        && let Some(key) = (sheet.key)(data)
    {
        return Some(format!("TABLE['{}'].{name}", key.replace('\'', "''")));
    }
    Some(match place(n, fixed, row)?.as_str() {
        "THIS" => name,
        p => format!("{name}[{p}]"),
    })
}

fn range(a: Ref, z: Ref, row: usize, sheet: &Sheet) -> Option<String> {
    if a.column != z.column {
        return None;
    }
    let name = named(sheet.columns.get(a.column)?);
    match (a.row, z.row) {
        // `E:E`: the whole column.
        (None, None) => Some(name),
        (Some((n, fa)), Some((m, fz))) => {
            let last = sheet.rows + 1;
            let end = |n: usize, fixed: bool| if fixed && n == last { Some("LAST".into()) } else { place(n, fixed, row) };
            Some(format!("{name}[{}:{}]", end(n, fa)?, end(m, fz)?))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_formula_is_written_in_names() {
        let columns = ["item", "on", "cost", "qty", "total", "running"];
        let keys = |r: usize| Some(["fuel", "groceries", "coffee"][r].to_string());
        let sheet = Sheet { columns: &columns, rows: 3, key: &keys };
        let w = |src: &str, row: usize| from_sheet(src, row, &sheet);
        assert_eq!(w("=C2*D2", 0).as_deref(), Some("=cost*qty"));
        assert_eq!(w("=C3*D3", 1).as_deref(), Some("=cost*qty"));
        assert_eq!(w("=SUM($E$2:E2)", 0).as_deref(), Some("=SUM(total[FIRST:THIS])"));
        assert_eq!(w("=SUM($E$2:E4)", 2).as_deref(), Some("=SUM(total[FIRST:THIS])"));
        assert_eq!(w("=F2+E3", 1).as_deref(), Some("=running[-1]+total"));
        assert_eq!(w("=E4", 1).as_deref(), Some("=total[+1]"));
        assert_eq!(w("=SUM(E:E)", 0).as_deref(), Some("=SUM(total)"));
        assert_eq!(w("=C$3*2", 0).as_deref(), Some("=TABLE['groceries'].cost*2"));
        assert_eq!(w("=SUM(E$2:E$4)", 1).as_deref(), Some("=SUM(total[FIRST:LAST])"));
        // Nothing to write: no reference, a column this file lacks, a range across columns.
        assert_eq!(w("=cost * qty", 0), None);
        assert_eq!(w("=Z2", 0), None);
        assert_eq!(w("=SUM(C2:D2)", 0), None);
        // Text holds none, and a function's name is none.
        assert_eq!(w("='C2' || C2", 0).as_deref(), Some("='C2' || cost"));
        assert_eq!(w("=ROUND(C2, 2)", 0).as_deref(), Some("=ROUND(cost, 2)"));
    }
}
