//! A file's text as spans, each with what it is: the fences, the prelude's statements and
//! comments, its tables written inline and its scripts, every header cell's name, the
//! parts of its type and its formula, and every cell by its column. What a highlighter
//! paints and a language server's semantic tokens carry. Read from the text alone, never
//! refused: a file that does not read still has its spans, a part that has no meaning
//! marked as such, so a broken file shows where it breaks.

use alloc::string::String;
use alloc::vec::Vec;

use crate::Base;
use crate::types::{self, BASES, EXPLICIT, NamedType, OPS, RESERVED, Type};

/// What a span of the text is. A new kind goes last, and into `ALL` and the test that
/// names the last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kind {
    /// A prelude's `---`, and a fence's backticks with its language.
    Fence,
    /// `// …` to the end of a prelude line.
    Comment,
    /// `type`, `export`, `import`, `from`, `with`, `run` and a schedule's words; a
    /// formula's `and`, `case`, `null`, and its functions.
    Keyword,
    /// A column's name, and a table's where the prelude names it.
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
    /// `:` after a name, `|` and `/` in a type, `=` before a formula or in a statement, a
    /// record's commas, a quoted header cell's quotes, a cell formula's backticks.
    Punct,
    /// A number in a formula or a schedule.
    Number,
    /// A column or a table a formula names, a schedule's zone or path.
    Reference,
    /// `+ - * / || = < ( ) [ ] . ,` in a formula.
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
    /// A quoted string: where an import comes from, an attribute's value, a formula's text.
    String,
    /// A script's code, inside its fence.
    Code,
    /// A table a formula names, one the prelude imports or writes (`rates` in
    /// `rates['2026-08-14'].eur`).
    Table,
    /// A tint: a choice's after its `#` or `tint`, a look rule's (`negative red`).
    Tint,
}

impl Kind {
    /// Every kind, in declaration order: a kind's number across a boundary (wasm) is its
    /// place here, and its name the one `name` gives.
    pub const ALL: [Kind; 25] = [
        Kind::Fence,
        Kind::Comment,
        Kind::Keyword,
        Kind::Name,
        Kind::Type,
        Kind::Mark,
        Kind::Bound,
        Kind::Format,
        Kind::Choice,
        Kind::Punct,
        Kind::Number,
        Kind::Reference,
        Kind::Operator,
        Kind::CellText,
        Kind::CellNumber,
        Kind::CellBool,
        Kind::CellDate,
        Kind::CellChoice,
        Kind::CellLink,
        Kind::CellId,
        Kind::Error,
        Kind::String,
        Kind::Code,
        Kind::Table,
        Kind::Tint,
    ];

    /// Its name in lower camel case (`cellNumber`), as a page or a language server's legend
    /// calls it.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Fence => "fence",
            Kind::Comment => "comment",
            Kind::Keyword => "keyword",
            Kind::Name => "name",
            Kind::Type => "type",
            Kind::Mark => "mark",
            Kind::Bound => "bound",
            Kind::Format => "format",
            Kind::Choice => "choice",
            Kind::Punct => "punct",
            Kind::Number => "number",
            Kind::Reference => "reference",
            Kind::Operator => "operator",
            Kind::CellText => "cellText",
            Kind::CellNumber => "cellNumber",
            Kind::CellBool => "cellBool",
            Kind::CellDate => "cellDate",
            Kind::CellChoice => "cellChoice",
            Kind::CellLink => "cellLink",
            Kind::CellId => "cellId",
            Kind::Error => "error",
            Kind::String => "string",
            Kind::Code => "code",
            Kind::Table => "table",
            Kind::Tint => "tint",
        }
    }
}

/// A span of the source in bytes, `start..end`; its column where it has one (a header
/// cell's parts, a cell, a record's comma before it), else `u32::MAX`; and for a name a
/// formula reads, the column of its own table it names (`cost`, `cost[-1]`,
/// `TABLE['k7m2q9xa'].cost`), else `u32::MAX` (another table's, `TABLE` itself, a row's
/// word in capitals, a name no column has).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub start: u32,
    pub end: u32,
    pub kind: Kind,
    pub column: u32,
    pub reads: u32,
}

const NONE: u32 = u32::MAX;

struct Out {
    tokens: Vec<Token>,
    /// Each column the prelude declares: its name, and its tokens' range, given their
    /// column once the header says it.
    declared: Vec<(String, core::ops::Range<usize>)>,
}

impl Out {
    fn push(&mut self, start: usize, end: usize, kind: Kind, column: u32) {
        if end > start {
            self.tokens.push(Token { start: start as u32, end: end as u32, kind, column, reads: NONE });
        }
    }
}

/// A name as a formula and a header spell it alike: a quoted one without its quotes, its
/// `""` one `"`.
fn unquoted(spelled: &str) -> String {
    match spelled.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        Some(inner) => inner.replace("\"\"", "\""),
        None => String::from(spelled),
    }
}

/// Each name of a formula among `tokens` (a table's, or a column's declaration) given
/// what it names, its table's columns being `names`: standing alone, a column of this
/// table (`reads`) or else one of
/// the prelude's `tables` (`Table`); after `TABLE`'s `.` (its row between brackets or
/// none), a column of this table; after any other `.`, another table's column.
fn resolve(source: &str, tokens: &mut [Token], tables: &[String], names: &[(String, u32)]) {
    let text = |t: &Token| &source[t.start as usize..t.end as usize];
    for i in 0..tokens.len() {
        if tokens[i].kind != Kind::Reference {
            continue;
        }
        let own = match i.checked_sub(1).map(|p| (p, &tokens[p])) {
            Some((p, dot)) if dot.kind == Kind::Operator && text(dot) == "." => {
                // What the `.` follows, back over a row `[…]` to the `[` that opens it.
                let mut j = p;
                if j > 0 && text(&tokens[j - 1]) == "]" {
                    let mut depth = 0usize;
                    while j > 0 {
                        j -= 1;
                        match (tokens[j].kind, text(&tokens[j])) {
                            (Kind::Operator, "]") => depth += 1,
                            (Kind::Operator, "[") => depth -= 1,
                            _ => {}
                        }
                        if depth == 0 {
                            break;
                        }
                    }
                }
                j.checked_sub(1).is_some_and(|h| tokens[h].kind == Kind::Keyword && text(&tokens[h]) == "TABLE")
            }
            _ => true,
        };
        if own {
            let name = unquoted(text(&tokens[i]));
            if let Some(&(_, column)) = names.iter().find(|(n, _)| *n == name) {
                tokens[i].reads = column;
            } else if tables.contains(&name) {
                tokens[i].kind = Kind::Table;
            }
        }
    }
}

/// The text's spans, in order, none overlapping; what lies between them (spaces, line
/// breaks) is nothing.
pub fn tokens(source: &str) -> Vec<Token> {
    let src = source.as_bytes();
    let mut out = Out { tokens: Vec::new(), declared: Vec::new() };
    let mut at = if source.starts_with('\u{feff}') { 3 } else { 0 };
    let mut named: Vec<NamedType> = Vec::new();
    let mut tables: Vec<String> = Vec::new();
    // The prelude: from a first line of exactly `---` to the next one outside a fence.
    if line(src, at).1 == b"---" {
        let (next, _) = line(src, at);
        out.push(at, at + 3, Kind::Fence, NONE);
        at = next;
        // A fence open: its backticks, its language, where its content starts.
        let mut open: Option<(usize, bool, usize)> = None;
        while at < src.len() {
            let (next, l) = line(src, at);
            let end = at + l.len();
            if let Some((ticks, table, from)) = open {
                let closing = l.len() >= ticks
                    && l.iter().all(|&c| c == b'`' || c == b' ' || c == b'\t')
                    && l.iter().filter(|&&c| c == b'`').count() == ticks;
                if closing {
                    if table {
                        // A table written inline: its own header and rows.
                        for t in tokens(&source[from..at]) {
                            out.tokens.push(Token { start: t.start + from as u32, end: t.end + from as u32, ..t });
                        }
                    }
                    out.push(at, end, Kind::Fence, NONE);
                    open = None;
                } else if !table {
                    out.push(at, end, Kind::Code, NONE);
                }
                at = next;
                continue;
            }
            if l == b"---" {
                out.push(at, at + 3, Kind::Fence, NONE);
                at = next;
                break;
            }
            if let Some((ticks, table)) = statement(source, at, end, &mut named, &mut tables, &mut out) {
                open = Some((ticks, table, next));
            }
            at = next;
        }
    }
    if at >= src.len() {
        return out.tokens;
    }
    // The header: each cell's name, its type's parts and its formula.
    let table = out.tokens.len();
    let mut columns: Vec<Option<Type>> = Vec::new();
    let (fields, next) = record(src, at);
    for (k, &(s, e)) in fields.iter().enumerate() {
        columns.push(header_cell(source, s, e, k as u32, &named, &tables, &mut out));
    }
    at = next;
    // The records: every cell by its column's kind.
    while at < src.len() {
        let (fields, next) = record(src, at);
        for (k, &(s, e)) in fields.iter().enumerate() {
            if k > 0 {
                // The comma before it, behind its padding.
                let mut comma = s;
                while comma > 0 && matches!(src[comma - 1], b' ' | b'\t') {
                    comma -= 1;
                }
                out.push(comma - 1, comma, Kind::Punct, k as u32);
            }
            let kind = match columns.get(k) {
                Some(Some(t)) => cell_kind(&t.base),
                Some(None) => Kind::CellText,
                None => Kind::Error,
            };
            if kind != Kind::Error
                && let Some(split) = crate::read::split(&src[s..e])
            {
                // `84.20=`, `32.25=96.75 / 3`: what is stored as its column's values are,
                // the `=`, and the formula's parts.
                let at = s + split;
                if at > s {
                    out.push(s, at, kind, k as u32);
                }
                out.push(at, at + 1, Kind::Punct, k as u32);
                if at + 1 < e {
                    expression(source, at + 1, e, k as u32, &mut out);
                }
                continue;
            }
            out.push(s, e, kind, k as u32);
        }
        at = next;
    }
    // A formula may name a column the header has not reached yet: names resolve once
    // the table is read whole.
    // The header's names, each with its column: a declared column's parts in the prelude
    // take its column, and every formula's names resolve against them.
    let text = |t: &Token| &source[t.start as usize..t.end as usize];
    let names: Vec<(String, u32)> =
        out.tokens[table..].iter().filter(|t| t.kind == Kind::Name).map(|t| (unquoted(text(t)), t.column)).collect();
    for (name, range) in core::mem::take(&mut out.declared) {
        if let Some(&(_, column)) = names.iter().find(|(n, _)| *n == name) {
            for t in &mut out.tokens[range.clone()] {
                t.column = column;
            }
        }
        resolve(source, &mut out.tokens[range], &tables, &names);
    }
    resolve(source, &mut out.tokens[table..], &tables, &names);
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
pub(crate) fn record(src: &[u8], at: usize) -> (Vec<(usize, usize)>, usize) {
    let mut fields = Vec::new();
    let mut i = at;
    loop {
        // A field's padding is no part of it: its span starts after the spaces and tabs
        // before it, a quote among them, and ends before those after it.
        while i < src.len() && matches!(src[i], b' ' | b'\t') {
            i += 1;
        }
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
        fields.push(crate::scan::trimmed(src, start, i));
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
        // A key of another table: one of the words it holds.
        Base::Choice(_) | Base::Ref(_) => Kind::CellChoice,
    }
}

/// A header cell `s..e`: its name, `:`, its type's parts and its formula's. Its type when
/// it reads.
fn header_cell(source: &str, s: usize, e: usize, k: u32, named: &[NamedType], tables: &[String], out: &mut Out) -> Option<Type> {
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
    let (ty, formula) = types::split_formula(spelled);
    let ty_end = from + colon + 1 + ty.len();
    type_parts(source, from + colon + 1, ty_end, k, named, tables, out);
    if formula.is_some() {
        out.push(ty_end, ty_end + 1, Kind::Punct, k);
        expression(source, ty_end + 1, to, k, out);
    }
    types::parse_type(ty, named, tables).ok()
}

/// One prelude line `s..e` outside a fence: a named type, an import, a table's name and
/// what fills it, a fence opening, a comment, or nothing. A line that opens a fence says
/// how many backticks close it, and whether what it holds is a table.
fn statement(
    source: &str,
    s: usize,
    e: usize,
    named: &mut Vec<NamedType>,
    tables: &mut Vec<String>,
    out: &mut Out,
) -> Option<(usize, bool)> {
    let b = source.as_bytes();
    let blank = |c: u8| c == b' ' || c == b'\t';
    let mut i = s;
    while i < e && blank(b[i]) {
        i += 1;
    }
    // A fence of the script's or of what the engine stored under it.
    if b[i..e].starts_with(b"```") {
        let ticks = b[i..e].iter().take_while(|&&c| c == b'`').count();
        out.push(i, e, Kind::Fence, NONE);
        let lang = core::str::from_utf8(&b[i + ticks..e]).unwrap_or("").trim_matches([' ', '\t']);
        return Some((ticks, lang.starts_with("cells")));
    }
    // The comment: `//` outside single and double quotes.
    let said_end = s + crate::prelude::uncomment(&source[s..e]).len();
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
    let skip = |mut j: usize| {
        while j < end && blank(b[j]) {
            j += 1;
        }
        j
    };
    let mut opened = None;
    if i < end {
        let mut word = ident(i);
        if &source[i..word] == "export" && word < end && blank(b[word]) {
            out.push(i, word, Kind::Keyword, NONE);
            i = skip(word);
            word = ident(i);
        }
        let after = skip(word);
        if &source[i..word] == "look" && after > word {
            look(source, i, end, named, out);
        } else if &source[i..word] == "type" && after > word && b.get(after) != Some(&b'=') {
            // `type Name = <type>`
            out.push(i, word, Kind::Keyword, NONE);
            let name_end = ident(after);
            out.push(after, name_end, Kind::Type, NONE);
            let j = skip(name_end);
            if j < end && b[j] == b'=' {
                out.push(j, j + 1, Kind::Punct, NONE);
                type_parts(source, j + 1, end, NONE, named, tables, out);
                if let Ok(ty) = types::parse_type(&source[j + 1..end], named, tables) {
                    named.push(NamedType::new(source[after..name_end].into(), ty));
                }
            } else {
                out.push(j, end, Kind::Error, NONE);
            }
        } else if &source[i..word] == "import" {
            out.push(i, word, Kind::Keyword, NONE);
            import(source, word, end, named, tables, out);
        } else if word > i && b.get(after) == Some(&b'=') {
            // `name = ```cells` or `name = run <schedule>`
            out.push(i, word, Kind::Name, NONE);
            tables.push(source[i..word].into());
            out.push(after, after + 1, Kind::Punct, NONE);
            let v = skip(after + 1);
            if b[v..end].starts_with(b"```") {
                out.push(v, end, Kind::Fence, NONE);
                opened = Some((b[v..end].iter().take_while(|&&c| c == b'`').count(), true));
            } else if b[v..end].starts_with(b"run") {
                out.push(v, v + 3, Kind::Keyword, NONE);
                schedule(source, v + 3, end, out);
            } else {
                out.push(v, end, Kind::Error, NONE);
            }
        } else if let Some((name_end, colon)) = declaration(source, i, end) {
            // `name: type`, `name: type = formula`: a column, whose number the header says.
            let first = out.tokens.len();
            out.push(i, name_end, Kind::Name, NONE);
            out.push(colon, colon + 1, Kind::Punct, NONE);
            let rest = &source[colon + 1..end];
            let (ty, formula) = types::split_formula(rest);
            let ty_end = colon + 1 + ty.len();
            type_parts(source, colon + 1, ty_end, NONE, named, tables, out);
            if formula.is_some() {
                out.push(ty_end, ty_end + 1, Kind::Punct, NONE);
                expression(source, ty_end + 1, end, NONE, out);
            }
            out.declared.push((unquoted(source[i..name_end].trim_matches([' ', '\t'])), first..out.tokens.len()));
        } else {
            out.push(i, end, Kind::Error, NONE);
        }
    }
    out.push(said_end, e, Kind::Comment, NONE);
    opened
}

/// A column's declaration in `s..e` (SPEC.md, Columns): where its name ends and its `:`
/// is; none for a statement whose first `:` is not before its first `=` (a table, a
/// script). A quoted name holds anything, `""` a quote inside.
fn declaration(source: &str, s: usize, e: usize) -> Option<(usize, usize)> {
    let b = source.as_bytes();
    let name_end = if b[s] == b'"' {
        let mut j = s + 1;
        loop {
            match b.get(j) {
                Some(b'"') if b.get(j + 1) == Some(&b'"') => j += 2,
                Some(b'"') => break j + 1,
                Some(_) if j < e => j += 1,
                _ => return None,
            }
        }
    } else {
        let colon = source[s..e].find(':')? + s;
        if source[s..e].find('=').is_some_and(|eq| s + eq < colon) {
            return None;
        }
        let mut end = colon;
        while end > s && matches!(b[end - 1], b' ' | b'\t') {
            end -= 1;
        }
        end
    };
    let mut colon = name_end;
    while colon < e && matches!(b[colon], b' ' | b'\t') {
        colon += 1;
    }
    (colon < e && b[colon] == b':' && name_end > s).then_some((name_end, colon))
}

/// What follows `import` on its line: the names it takes, where from, its attributes.
fn import(source: &str, s: usize, e: usize, named: &mut Vec<NamedType>, tables: &mut Vec<String>, out: &mut Out) {
    let b = source.as_bytes();
    let mut i = s;
    while i < e {
        let c = b[i];
        let start = i;
        if c == b' ' || c == b'\t' {
            i += 1;
        } else if c == b'"' {
            i += 1;
            while i < e && b[i] != b'"' {
                i += 1 + (b[i] == b'\\') as usize;
            }
            i = (i + 1).min(e);
            out.push(start, i, Kind::String, NONE);
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < e && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let w = &source[start..i];
            if matches!(w, "from" | "with") {
                out.push(start, i, Kind::Keyword, NONE);
            } else if b.get(i) == Some(&b':') {
                // An attribute's key.
                out.push(start, i, Kind::Keyword, NONE);
            } else if c.is_ascii_uppercase() {
                named.push(NamedType::new(w.into(), Type::plain(Base::Text)));
                out.push(start, i, Kind::Type, NONE);
            } else {
                tables.push(w.into());
                out.push(start, i, Kind::Name, NONE);
            }
        } else {
            i += source[i..].chars().next().map_or(1, char::len_utf8);
            out.push(start, i, Kind::Punct, NONE);
        }
    }
}

/// What follows `look` on its line (SPEC.md, How a value looks): what it is for, then its
/// rules, each's words, tints, pattern and condition.
fn look(source: &str, s: usize, e: usize, named: &[NamedType], out: &mut Out) {
    const WORDS: [&str; 9] = ["grouped", "decimals", "negative", "positive", "in", "parentheses", "scale", "as", "when"];
    let b = source.as_bytes();
    out.push(s, s + 4, Kind::Keyword, NONE);
    let Some((target, colon, rules)) = crate::look::spans(&source[s..e]) else {
        out.push(s + 4, e, Kind::Error, NONE);
        return;
    };
    let kind = match &source[s + target.start..s + target.end] {
        "row" => Kind::Keyword,
        t if named.iter().any(|n| n.name == t) => Kind::Type,
        _ => Kind::Name,
    };
    out.push(s + target.start, s + target.end, kind, NONE);
    out.push(s + colon, s + colon + 1, Kind::Punct, NONE);
    for (k, rule) in rules.iter().enumerate() {
        let (start, end) = (s + rule.start, s + rule.end);
        if let Some(next) = rules.get(k + 1) {
            let comma = end + source[end..s + next.start].find(',').expect("a comma between two rules");
            out.push(comma, comma + 1, Kind::Punct, NONE);
        }
        let mut i = start;
        while i < end {
            let c = b[i];
            let from = i;
            if c == b' ' || c == b'\t' {
                i += 1;
                continue;
            }
            if c == b'\'' {
                i = source[i + 1..end].rfind('\'').map_or(end, |q| i + 1 + q + 1);
                out.push(from, i, Kind::Format, NONE);
                continue;
            }
            while i < end && !matches!(b[i], b' ' | b'\t') {
                i += 1;
            }
            let w = &source[from..i];
            if w == "when" {
                out.push(from, i, Kind::Keyword, NONE);
                expression(source, i, end, NONE, out);
                break;
            }
            if let Some((a, z)) = w.split_once("..") {
                let tint = |t: &str| if crate::look::Tint::parse(t).is_ok() { Kind::Tint } else { Kind::Error };
                out.push(from, from + a.len(), tint(a), NONE);
                out.push(from + a.len(), from + a.len() + 2, Kind::Punct, NONE);
                out.push(i - z.len(), i, tint(z), NONE);
                continue;
            }
            let kind = if WORDS.contains(&w) {
                Kind::Keyword
            } else if crate::look::Tint::parse(w).is_ok() {
                Kind::Tint
            } else if w.bytes().all(|c| c.is_ascii_digit()) {
                Kind::Number
            } else {
                Kind::Error
            };
            out.push(from, i, kind, NONE);
        }
    }
}

/// A schedule `s..e`: its words, its numbers and times, its zone or path.
fn schedule(source: &str, s: usize, e: usize, out: &mut Out) {
    const WORDS: [&str; 18] = [
        "every",
        "at",
        "in",
        "on",
        "cron",
        "when",
        "changes",
        "minute",
        "hour",
        "day",
        "weekday",
        "month",
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
    ];
    let b = source.as_bytes();
    let mut i = s;
    while i < e {
        if b[i] == b' ' || b[i] == b'\t' || b[i] == b',' {
            if b[i] == b',' {
                out.push(i, i + 1, Kind::Punct, NONE);
            }
            i += 1;
            continue;
        }
        let start = i;
        while i < e && !matches!(b[i], b' ' | b'\t' | b',') {
            i += 1;
        }
        let w = &source[start..i];
        let kind = if WORDS.contains(&w) || w == "sunday" {
            Kind::Keyword
        } else if w.bytes().next().is_some_and(|c| c.is_ascii_digit() || c == b'*') {
            Kind::Number
        } else {
            Kind::Reference
        };
        out.push(start, i, kind, NONE);
    }
}

/// A formula `s..e`, in the header's cell or a cell's own: its numbers, texts, names,
/// keywords and operators.
/// The file's own table and its rows by where they are, as a formula writes them.
const ROWS: [&str; 4] = ["TABLE", "FIRST", "THIS", "LAST"];

fn expression(source: &str, s: usize, e: usize, column: u32, out: &mut Out) {
    const KEYWORDS: [&str; 13] = ["and", "or", "not", "is", "null", "true", "false", "case", "when", "then", "else", "end", "in"];
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
            out.push(start, i, Kind::Number, column);
        } else if c == b'\'' || c == b'"' {
            i += 1;
            while i < e {
                if b[i] == c {
                    if b.get(i + 1) == Some(&c) && i + 1 < e {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push(start, i, if c == b'\'' { Kind::String } else { Kind::Reference }, column);
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < e && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let w = &source[start..i];
            let call = b.get(i) == Some(&b'(');
            // The language's own words: a function, SQL's, and the table and its rows by
            // where they are, in capitals as the language writes them.
            let kind = if call || KEYWORDS.iter().any(|k| k.eq_ignore_ascii_case(w)) || ROWS.contains(&w) {
                Kind::Keyword
            } else {
                Kind::Reference
            };
            out.push(start, i, kind, column);
        } else if matches!(c, b'+' | b'-' | b'*' | b'/' | b'(' | b')' | b'[' | b']' | b'.' | b',' | b'=' | b'<' | b'>' | b'!' | b'|' | b':')
        {
            i += 1;
            out.push(start, i, Kind::Operator, column);
        } else {
            i += source[i..].chars().next().map_or(1, char::len_utf8);
            out.push(start, i, Kind::Error, column);
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
fn type_parts(source: &str, s: usize, e: usize, column: u32, named: &[NamedType], tables: &[String], out: &mut Out) {
    let reference = |w: &str| w.split_once('.').is_some_and(|(t, c)| !c.is_empty() && tables.iter().any(|n| n == t));
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
            Lex::Op("|" | "/" | "#") => Kind::Punct,
            Lex::Op("!" | "?") => Kind::Mark,
            Lex::Op(_) => Kind::Bound,
            Lex::Word => {
                let w = word(k);
                let tinted = op_at(prev, "#") || prev.is_some_and(|p| lexed[p].2 == Lex::Word && word(p) == "tint");
                if tinted {
                    if crate::look::Tint::after_hash(w).is_ok() { Kind::Tint } else { Kind::Error }
                } else if w == "tint" && (listing || prev.is_some_and(|p| lexed[p].2 == Lex::Quoted)) {
                    Kind::Keyword
                } else if reference(w) {
                    Kind::Reference
                } else if matches!(w, "key" | "optional") {
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
                    || op_at(next, "#")
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
        let src = "---\ntype Cat = travel|stay // why\n---\nitem:text!,cat:Cat,cost:EUR>=0,on:date(DD/MM/YYYY),total:EUR=cost*2\nfuel,travel,84.20,14/08/2026,\n";
        let t = shown(src);
        let kinds = |k: Kind| t.iter().filter(|x| x.1 == k).map(|x| x.0.as_str()).collect::<Vec<_>>();
        assert_eq!(kinds(Kind::Fence), ["---", "---"]);
        assert_eq!(kinds(Kind::Keyword), ["type"]);
        assert_eq!(kinds(Kind::Comment), ["// why"]);
        assert_eq!(kinds(Kind::Type), ["Cat", "text", "Cat", "EUR", "date", "EUR"]);
        assert_eq!(kinds(Kind::Choice), ["travel", "stay"]);
        assert_eq!(kinds(Kind::Name), ["item", "cat", "cost", "on", "total"]);
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
    fn tables_scripts_and_formulas() {
        let src = "---\nimport accounts from \"../accounts.cells\"\nfees = ```cells\nkind:text!,pct:pct\ncard,1.4\n```\nfx = run every day at 18:00 in Europe/Madrid\n```ts\nexport default () => []\n```\n```cells\nrate:num\n```\n---\nto:accounts.account,cost:EUR\nrent,32.25=96.75/3\n";
        let t = shown(src);
        let kinds = |k: Kind| t.iter().filter(|x| x.1 == k).map(|x| x.0.as_str()).collect::<Vec<_>>();
        assert_eq!(kinds(Kind::String), ["\"../accounts.cells\""]);
        assert_eq!(kinds(Kind::Code), ["export default () => []"]);
        assert!(kinds(Kind::Name).contains(&"pct"), "{t:?}");
        assert_eq!(kinds(Kind::CellChoice), ["rent"]);
        assert_eq!(kinds(Kind::Number), ["18:00", "96.75", "3"]);
        assert_eq!(kinds(Kind::CellNumber), ["1.4", "32.25"]);
        assert!(kinds(Kind::Error).is_empty(), "{t:?}");
    }

    #[test]
    fn every_kind_numbered_by_its_place() {
        for (i, k) in Kind::ALL.iter().enumerate() {
            assert_eq!(*k as usize, i, "{k:?}");
        }
        assert_eq!(Kind::Tint as usize + 1, Kind::ALL.len(), "a kind declared after Tint is missing from ALL");
    }

    #[test]
    fn tints_and_looks() {
        let src = "---\ntype Kind = income#green|cost#3fa34d|pending\nlook Kind: grouped\nlook amount: negative in parentheses, red when amount < -10, scale red..green\nlook on: as 'D MMM YYYY'\nlook row: gray when paid\n---\nkind:Kind,amount:EUR,on:date,paid:yes/no,mood:one of yay tint blue meh\n";
        let t = shown(src);
        let kinds = |k: Kind| t.iter().filter(|x| x.1 == k).map(|x| x.0.as_str()).collect::<Vec<_>>();
        assert_eq!(kinds(Kind::Tint), ["green", "3fa34d", "red", "red", "green", "gray", "blue"]);
        assert_eq!(kinds(Kind::Choice), ["income", "cost", "pending", "yes", "no", "yay", "meh"]);
        assert_eq!(
            kinds(Kind::Keyword),
            [
                "type",
                "look",
                "grouped",
                "look",
                "negative",
                "in",
                "parentheses",
                "when",
                "scale",
                "look",
                "as",
                "look",
                "row",
                "when",
                "tint"
            ]
        );
        assert_eq!(kinds(Kind::Format), ["'D MMM YYYY'"]);
        assert_eq!(kinds(Kind::Reference), ["amount", "paid"]);
        assert!(kinds(Kind::Error).is_empty(), "{t:?}");
        assert_eq!(shown("---\nlook amount: negative grene\n---\namount:EUR\n").iter().filter(|x| x.1 == Kind::Error).count(), 1);
    }

    #[test]
    fn a_formula_name_reads_its_column() {
        let src = "---\nimport accounts from \"./a.cells\"\n---\nto:accounts.account,cost:EUR,qty:int,total:EUR=cost * qty,run:EUR=run[-1] + total,\"my \"\"x\"\"\",w:EUR=TABLE['k7'].cost + TABLE.qty + accounts.cost + SUM(total[FIRST:THIS]) + \"my \"\"x\"\"\" + later,later:num\nrent,2,3,6=qty * 2,,1,\n";
        let reads = |name: &str| {
            tokens(src)
                .into_iter()
                .filter(|t| t.kind == Kind::Reference && &src[t.start as usize..t.end as usize] == name)
                .map(|t| t.reads)
                .collect::<Vec<_>>()
        };
        // Bare, with a row, after TABLE's `.` (with its row or none), and in a cell's own formula.
        assert_eq!(reads("cost"), [1, 1, NONE]);
        assert_eq!(reads("qty"), [2, 2, 2]);
        assert_eq!(reads("run"), [4]);
        assert_eq!(reads("total"), [3, 3]);
        // A quoted name, and one the header reaches only later.
        assert_eq!(reads("\"my \"\"x\"\"\""), [5]);
        assert_eq!(reads("later"), [7]);
        // The table itself and a row by where it is are the language's words; a table the
        // prelude names is a table; a range's `:` an operator.
        let kind_of = |spelled: &str| {
            tokens(src).into_iter().filter(|t| &src[t.start as usize..t.end as usize] == spelled).map(|t| t.kind).collect::<Vec<_>>()
        };
        assert_eq!(kind_of("TABLE"), [Kind::Keyword, Kind::Keyword]);
        assert_eq!(kind_of("FIRST"), [Kind::Keyword]);
        assert_eq!(kind_of("THIS"), [Kind::Keyword]);
        assert_eq!(kind_of("accounts"), [Kind::Name, Kind::Table], "the import's name, then the formula's");
        assert!(kind_of(":").contains(&Kind::Operator), "{:?}", kind_of(":"));
        assert!(tokens(src).iter().all(|t| t.kind != Kind::Error), "nothing in it is an error");
    }

    #[test]
    fn a_declared_column_is_its_header_cells_column() {
        let src = "---\ntype Money = EUR>=0\ncost: Money\ntotal: Money = cost * qty // why\n\"price, net\": EUR?\n---\nitem:text!,cost,qty:int,total,\"price, net\"\nfuel,1,2,2=,\n";
        let t = tokens(src);
        let at = |spelled: &str, from: usize| {
            let start = src[from..].find(spelled).unwrap() + from;
            *t.iter().find(|x| x.start as usize == start).unwrap_or_else(|| panic!("no token at {spelled}"))
        };
        // The declaration's name, and its type, take the column the header names it at.
        assert_eq!((at("cost", 0).kind, at("cost", 0).column), (Kind::Name, 1));
        assert_eq!((at("Money", src.find("cost:").unwrap()).kind, at("Money", src.find("cost:").unwrap()).column), (Kind::Type, 1));
        assert_eq!(at("\"price, net\"", 0).column, 4);
        // Its formula's names read the header's columns, a declared one and a typed one.
        let formula = src.find("= cost").unwrap();
        assert_eq!(at("cost", formula).reads, 1);
        assert_eq!(at("qty", formula).reads, 2);
        assert_eq!(at("total", src.find("---\nitem").unwrap()).column, 3, "the header's bare name");
        assert!(t.iter().all(|x| x.kind != Kind::Error), "{t:?}");
    }

    #[test]
    fn a_padded_cell_is_its_value() {
        let t = shown("item:text!,  cost:EUR\n  fuel  ,\t84.20\n");
        assert!(t.contains(&("cost".into(), Kind::Name, 1)), "{t:?}");
        assert!(t.contains(&("fuel".into(), Kind::CellText, 0)), "{t:?}");
        assert!(t.contains(&("84.20".into(), Kind::CellNumber, 1)), "{t:?}");
        assert!(t.contains(&(",".into(), Kind::Punct, 1)), "the comma behind the padding: {t:?}");
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
