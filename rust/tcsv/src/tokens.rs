//! A file's text as spans, each with what it is: the fences, the prelude's statements and
//! comments, every header cell's name and the parts of its type, and every cell by its
//! column. What a highlighter paints and a language server's semantic tokens carry. Read
//! from the text alone, never refused: a file that does not read still has its spans, a
//! part that has no meaning marked as such, so a broken file shows where it breaks.

use alloc::vec::Vec;

use crate::Base;
use crate::types::{self, BASES, EXPLICIT, NamedType, OPS, RESERVED, Type};

/// What a span of the text is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A prelude's `---`.
    Fence,
    /// `// …` to the end of a prelude line.
    Comment,
    /// `type`, opening a named type.
    Keyword,
    /// A column's name: a header cell's, or a computed column's in the prelude.
    Name,
    /// What a column holds: a base, a currency, a named type (where it is used and where
    /// it is defined), `one of`, `bool`.
    Type,
    /// `!`, `?`, `key`, `optional`.
    Mark,
    /// A bound: `>=`, `..`, `min`, `between`, `and`, and its numbers.
    Bound,
    /// A date or time's format, its parentheses included.
    Format,
    /// A choice's word, or a boolean pair's.
    Choice,
    /// `:` after a name, `|` and `/` in a type, `=` in a statement, a record's commas, a
    /// quoted header cell's quotes.
    Punct,
    /// A number in a computed column's expression.
    Number,
    /// A column named in a computed column's expression.
    Reference,
    /// `+ - * / ( )` in an expression.
    Operator,
    /// A cell of a text column (an untyped column's too).
    CellText,
    /// A cell of an `int`, `num`, `pct` or money column.
    CellNumber,
    CellBool,
    /// A cell of a `date`, `time` or `datetime` column.
    CellDate,
    CellChoice,
    /// A cell of a `url` or `email` column.
    CellLink,
    CellId,
    /// A part that has no meaning where it stands.
    Error,
}

/// A span of the source in bytes, `start..end`, and its column where it has one (a header
/// cell's parts, a cell, a record's comma before it), else `u32::MAX`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub start: u32,
    pub end: u32,
    pub kind: Kind,
    pub column: u32,
}

const NONE: u32 = u32::MAX;

struct Out {
    tokens: Vec<Token>,
}

impl Out {
    fn push(&mut self, start: usize, end: usize, kind: Kind, column: u32) {
        if end > start {
            self.tokens.push(Token { start: start as u32, end: end as u32, kind, column });
        }
    }
}

/// The text's spans, in order, none overlapping; what lies between them (spaces, line
/// breaks) is nothing.
pub fn tokens(source: &str) -> Vec<Token> {
    let src = source.as_bytes();
    let mut out = Out { tokens: Vec::new() };
    let mut at = if source.starts_with('\u{feff}') { 3 } else { 0 };
    let mut named: Vec<NamedType> = Vec::new();
    // The prelude: from a first line of exactly `---` to the next one.
    if line(src, at).1 == b"---" {
        let (next, _) = line(src, at);
        out.push(at, at + 3, Kind::Fence, NONE);
        at = next;
        while at < src.len() {
            let (next, l) = line(src, at);
            if l == b"---" {
                out.push(at, at + 3, Kind::Fence, NONE);
                at = next;
                break;
            }
            statement(source, at, at + l.len(), &mut named, &mut out);
            at = next;
        }
    }
    if at >= src.len() {
        return out.tokens;
    }
    // The header: each cell's name, and its type's parts.
    let mut columns: Vec<Option<Type>> = Vec::new();
    let (fields, next) = record(src, at);
    for (k, &(s, e)) in fields.iter().enumerate() {
        columns.push(header_cell(source, s, e, k as u32, &named, &mut out));
    }
    at = next;
    // The records: every cell by its column's kind.
    while at < src.len() {
        let (fields, next) = record(src, at);
        for (k, &(s, e)) in fields.iter().enumerate() {
            if k > 0 {
                out.push(s - 1, s, Kind::Punct, k as u32);
            }
            let kind = match columns.get(k) {
                Some(Some(t)) => cell_kind(&t.base),
                Some(None) => Kind::CellText,
                None => Kind::Error,
            };
            out.push(s, e, kind, k as u32);
        }
        at = next;
    }
    out.tokens
}

/// The line at `at`: where the next one starts, and its bytes without the line break.
fn line(src: &[u8], at: usize) -> (usize, &[u8]) {
    let end = src[at..].iter().position(|&c| c == b'\n').map_or(src.len(), |p| at + p);
    let next = if end < src.len() { end + 1 } else { end };
    let l = &src[at..end];
    (next, l.strip_suffix(b"\r").unwrap_or(l))
}

/// A record's fields from `at`, each span with its quotes, and where the next record
/// starts. Lenient: an unclosed quote runs to the end, text after a closing quote is the
/// field's.
fn record(src: &[u8], at: usize) -> (Vec<(usize, usize)>, usize) {
    let mut fields = Vec::new();
    let mut i = at;
    loop {
        let start = i;
        if i < src.len() && src[i] == b'"' {
            i += 1;
            while i < src.len() {
                if src[i] == b'"' {
                    if src.get(i + 1) == Some(&b'"') {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
        }
        while i < src.len() && !matches!(src[i], b',' | b'\n' | b'\r') {
            i += 1;
        }
        fields.push((start, i));
        if i < src.len() && src[i] == b',' {
            i += 1;
            continue;
        }
        if i < src.len() && src[i] == b'\r' {
            i += 1;
        }
        if i < src.len() && src[i] == b'\n' {
            i += 1;
        }
        return (fields, i);
    }
}

fn cell_kind(base: &Base) -> Kind {
    match base {
        Base::Text => Kind::CellText,
        Base::Int | Base::Num | Base::Pct | Base::Money(_) => Kind::CellNumber,
        Base::Bool { .. } => Kind::CellBool,
        Base::Date | Base::Time | Base::DateTime => Kind::CellDate,
        Base::Url | Base::Email => Kind::CellLink,
        Base::Id => Kind::CellId,
        Base::Choice(_) => Kind::CellChoice,
    }
}

/// A header cell `s..e`: its name, `:`, its type's parts. Its type when it reads.
fn header_cell(source: &str, s: usize, e: usize, k: u32, named: &[NamedType], out: &mut Out) -> Option<Type> {
    let raw = &source[s..e];
    let (from, to) = match raw.strip_prefix('"') {
        Some(inner) if inner.ends_with('"') && !inner[..inner.len() - 1].contains('"') => {
            out.push(s, s + 1, Kind::Punct, k);
            out.push(e - 1, e, Kind::Punct, k);
            (s + 1, e - 1)
        }
        Some(_) => {
            out.push(s, e, Kind::Name, k);
            return None;
        }
        None => (s, e),
    };
    let cell = &source[from..to];
    let Some(colon) = cell.find(':') else {
        out.push(from, to, Kind::Name, k);
        return None;
    };
    out.push(from, from + colon, Kind::Name, k);
    out.push(from + colon, from + colon + 1, Kind::Punct, k);
    let spelled = &cell[colon + 1..];
    type_parts(source, from + colon + 1, to, k, named, out);
    types::parse_type(spelled, named).ok()
}

/// One prelude line `s..e`: a named type, a computed column, a comment, or nothing.
fn statement(source: &str, s: usize, e: usize, named: &mut Vec<NamedType>, out: &mut Out) {
    let b = source.as_bytes();
    // The comment: `//` outside single quotes.
    let mut quoted = false;
    let mut said_end = e;
    for i in s..e {
        if b[i] == b'\'' {
            quoted = !quoted;
        } else if !quoted && b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            said_end = i;
            break;
        }
    }
    let blank = |c: u8| c == b' ' || c == b'\t';
    let mut i = s;
    while i < said_end && blank(b[i]) {
        i += 1;
    }
    let mut end = said_end;
    while end > i && blank(b[end - 1]) {
        end -= 1;
    }
    let ident = |from: usize| {
        let mut j = from;
        while j < end && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
            j += 1;
        }
        j
    };
    if i < end {
        let word = ident(i);
        let after = {
            let mut j = word;
            while j < end && blank(b[j]) {
                j += 1;
            }
            j
        };
        if &source[i..word] == "type" && after > word && b.get(after) != Some(&b'=') {
            // `type Name = <type>`
            out.push(i, word, Kind::Keyword, NONE);
            let name_end = ident(after);
            out.push(after, name_end, Kind::Type, NONE);
            let mut j = name_end;
            while j < end && blank(b[j]) {
                j += 1;
            }
            if j < end && b[j] == b'=' {
                out.push(j, j + 1, Kind::Punct, NONE);
                type_parts(source, j + 1, end, NONE, named, out);
                if let Ok(ty) = types::parse_type(&source[j + 1..end], named) {
                    named.push(NamedType { name: source[after..name_end].into(), ty });
                }
            } else {
                out.push(j, end, Kind::Error, NONE);
            }
        } else if word > i && b.get(after) == Some(&b'=') {
            // `name = expression`
            out.push(i, word, Kind::Name, NONE);
            out.push(after, after + 1, Kind::Punct, NONE);
            expression(source, after + 1, end, out);
        } else {
            out.push(i, end, Kind::Error, NONE);
        }
    }
    out.push(said_end, e, Kind::Comment, NONE);
}

/// A computed column's expression `s..e`.
fn expression(source: &str, s: usize, e: usize, out: &mut Out) {
    let b = source.as_bytes();
    let mut i = s;
    while i < e {
        let c = b[i];
        let start = i;
        if c == b' ' || c == b'\t' {
            i += 1;
        } else if c.is_ascii_digit() {
            while i < e && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            out.push(start, i, Kind::Number, NONE);
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < e && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(start, i, Kind::Reference, NONE);
        } else if matches!(c, b'+' | b'-' | b'*' | b'/' | b'(' | b')') {
            i += 1;
            out.push(start, i, Kind::Operator, NONE);
        } else {
            i += source[i..].chars().next().map_or(1, char::len_utf8);
            out.push(start, i, Kind::Error, NONE);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Lex {
    Word,
    Quoted,
    Op(&'static str),
    Format,
    Bad,
}

/// A type's text `s..e`, its parts by what they are, as `types::parse_type` reads them.
fn type_parts(source: &str, s: usize, e: usize, column: u32, named: &[NamedType], out: &mut Out) {
    let b = source.as_bytes();
    let mut lexed: Vec<(usize, usize, Lex)> = Vec::new();
    let mut i = s;
    while i < e {
        let c = b[i];
        let start = i;
        if c == b' ' || c == b'\t' {
            i += 1;
            continue;
        }
        if c == b'\'' {
            i += 1;
            while i < e {
                if b[i] == b'\'' {
                    if i + 1 < e && b[i + 1] == b'\'' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            lexed.push((start, i, Lex::Quoted));
            continue;
        }
        if c == b'(' {
            i = source[i..e].find(')').map_or(e, |p| i + p + 1);
            lexed.push((start, i, Lex::Format));
            continue;
        }
        if let Some(op) = OPS.iter().find(|o| b[i..e].starts_with(o.as_bytes())) {
            i += op.len();
            lexed.push((start, i, Lex::Op(op)));
            continue;
        }
        while i < e && b[i] != b' ' && b[i] != b'\t' && !RESERVED.contains(&b[i]) && !b[i..e].starts_with(b"..") {
            i += 1;
        }
        if i == start {
            i += source[i..].chars().next().map_or(1, char::len_utf8);
            lexed.push((start, i, Lex::Bad));
        } else {
            lexed.push((start, i, Lex::Word));
        }
    }
    let word = |k: usize| -> &str { &source[lexed[k].0..lexed[k].1] };
    let op_at = |k: Option<usize>, o: &str| k.and_then(|k| lexed.get(k)).is_some_and(|t| matches!(t.2, Lex::Op(x) if x == o));
    let mut listing = false;
    let mut pair = 0;
    for k in 0..lexed.len() {
        let (start, end, lex) = lexed[k];
        let prev = k.checked_sub(1);
        let next = Some(k + 1);
        let kind = match lex {
            Lex::Quoted => Kind::Choice,
            Lex::Format => Kind::Format,
            Lex::Bad => Kind::Error,
            Lex::Op("|" | "/") => Kind::Punct,
            Lex::Op("!" | "?") => Kind::Mark,
            Lex::Op(_) => Kind::Bound,
            Lex::Word => {
                let w = word(k);
                if matches!(w, "key" | "optional") {
                    listing = false;
                    Kind::Mark
                } else if matches!(w, "min" | "max" | "above" | "below" | "between" | "and") {
                    listing = false;
                    Kind::Bound
                } else if pair > 0 {
                    pair -= 1;
                    Kind::Choice
                } else if (listing && !EXPLICIT.contains(&w))
                    || op_at(prev, "|")
                    || op_at(next, "|")
                    || op_at(prev, "/")
                    || op_at(next, "/")
                {
                    Kind::Choice
                } else if crate::value::Decimal::parse(w.as_bytes()).is_ok() {
                    Kind::Bound
                } else if w == "one" && lexed.get(k + 1).is_some_and(|t| t.2 == Lex::Word && word(k + 1) == "of") {
                    Kind::Type
                } else if w == "of" && prev.is_some_and(|p| lexed[p].2 == Lex::Word && word(p) == "one") {
                    listing = true;
                    Kind::Type
                } else if w == "bool" {
                    let listed =
                        |j: usize| lexed.get(j).is_some_and(|t| t.2 == Lex::Quoted || (t.2 == Lex::Word && !EXPLICIT.contains(&word(j))));
                    if listed(k + 1) && listed(k + 2) {
                        pair = 2;
                    }
                    Kind::Type
                } else if BASES.iter().any(|(n, _)| *n == w) || types::currency(w).is_some() || named.iter().any(|n| n.name == w) {
                    Kind::Type
                } else {
                    Kind::Error
                }
            }
        };
        out.push(start, end, kind, column);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    fn shown(src: &str) -> Vec<(String, Kind, u32)> {
        tokens(src).iter().map(|t| (String::from(&src[t.start as usize..t.end as usize]), t.kind, t.column)).collect()
    }

    #[test]
    fn every_part_by_what_it_is() {
        let src = "---\ntype Cat = travel|stay // why\ntotal = cost * 2\n---\nitem:text!,cat:Cat,cost:EUR>=0,on:date(DD/MM/YYYY)\nfuel,travel,84.20,14/08/2026\n";
        let t = shown(src);
        let kinds = |k: Kind| t.iter().filter(|x| x.1 == k).map(|x| x.0.as_str()).collect::<Vec<_>>();
        assert_eq!(kinds(Kind::Fence), ["---", "---"]);
        assert_eq!(kinds(Kind::Keyword), ["type"]);
        assert_eq!(kinds(Kind::Comment), ["// why"]);
        assert_eq!(kinds(Kind::Type), ["Cat", "text", "Cat", "EUR", "date"]);
        assert_eq!(kinds(Kind::Choice), ["travel", "stay"]);
        assert_eq!(kinds(Kind::Name), ["total", "item", "cat", "cost", "on"]);
        assert_eq!(kinds(Kind::Reference), ["cost"]);
        assert_eq!(kinds(Kind::Number), ["2"]);
        assert_eq!(kinds(Kind::Mark), ["!"]);
        assert_eq!(kinds(Kind::Bound), [">=", "0"]);
        assert_eq!(kinds(Kind::Format), ["(DD/MM/YYYY)"]);
        assert_eq!(kinds(Kind::CellText), ["fuel"]);
        assert_eq!(kinds(Kind::CellChoice), ["travel"]);
        assert_eq!(kinds(Kind::CellNumber), ["84.20"]);
        assert_eq!(kinds(Kind::CellDate), ["14/08/2026"]);
        assert!(kinds(Kind::Error).is_empty(), "{t:?}");
        // Cells carry their column, for a rainbow.
        assert_eq!(t.iter().filter(|x| x.0 == "84.20").map(|x| x.2).collect::<Vec<_>>(), [2]);
    }

    #[test]
    fn a_broken_file_still_has_its_spans() {
        let t = shown("a:EURO,b:one of x 'y z' key\n\"q, r\",2\n");
        assert!(t.contains(&("EURO".into(), Kind::Error, 0)));
        assert!(t.contains(&("'y z'".into(), Kind::Choice, 1)));
        assert!(t.contains(&("x".into(), Kind::Choice, 1)));
        assert!(t.contains(&("key".into(), Kind::Mark, 1)));
        assert!(t.contains(&("\"q, r\"".into(), Kind::CellText, 0)));
    }
}
