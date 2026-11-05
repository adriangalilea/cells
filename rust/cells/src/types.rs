//! The header language: a column's type as a header cell spells it after its `:`, parsed
//! into a `Type`, and a `Type` written back as canonical shorthand.
//!
//! A type opens with what the column holds (a base, a currency, a named type, a choice or
//! a pair), then its format, bounds and marks in any order, each said once. A type that
//! opens with a bound holds a number.

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::value::Decimal;

/// What a column holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Base {
    Text,
    Int,
    Num,
    Pct,
    /// A boolean in the file's words for true and false (`true`/`false` when plain).
    Bool {
        yes: String,
        no: String,
    },
    Date,
    Time,
    DateTime,
    Url,
    Email,
    /// A row's identity, eight characters of Crockford's base 32.
    Id,
    /// An amount in an ISO 4217 currency.
    Money(Currency),
    /// One of the words, in the order written.
    Choice(Vec<String>),
    /// A key of another table, of any of them when there are several (`accounts.account`,
    /// `shared.account|clients.account`).
    Ref(Vec<Target>),
}

/// A table a reference names, and its key column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub table: String,
    pub column: String,
}

impl Base {
    pub fn is_number(&self) -> bool {
        matches!(self, Base::Int | Base::Num | Base::Pct | Base::Money(_))
    }

    /// The suite's tag for a value of this base.
    pub fn tag(&self) -> &'static str {
        match self {
            Base::Text => "text",
            Base::Int => "int",
            Base::Num => "num",
            Base::Pct => "pct",
            Base::Bool { .. } => "bool",
            Base::Date => "date",
            Base::Time => "time",
            Base::DateTime => "datetime",
            Base::Url => "url",
            Base::Email => "email",
            Base::Id => "id",
            Base::Money(_) => "money",
            Base::Choice(_) => "choice",
            Base::Ref(_) => "ref",
        }
    }
}

/// An ISO 4217 code, three capital letters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Currency(pub [u8; 3]);

impl Currency {
    pub fn as_str(&self) -> &str {
        // Built only from CURRENCIES, ASCII.
        core::str::from_utf8(&self.0).unwrap()
    }
}

/// ISO 4217's current alphabetic codes (list one), sorted: any of them is a type, an amount
/// in that currency. Fund codes, the metals and the units of account are in; `XTS` (for
/// testing) and `XXX` (no currency) are not.
pub const CURRENCIES: &[&str] = &[
    "AED", "AFN", "ALL", "AMD", "AOA", "ARS", "AUD", "AWG", "AZN", "BAM", "BBD", "BDT", "BGN", "BHD", "BIF", "BMD", "BND", "BOB", "BOV",
    "BRL", "BSD", "BTN", "BWP", "BYN", "BZD", "CAD", "CDF", "CHE", "CHF", "CHW", "CLF", "CLP", "CNY", "COP", "COU", "CRC", "CUP", "CVE",
    "CZK", "DJF", "DKK", "DOP", "DZD", "EGP", "ERN", "ETB", "EUR", "FJD", "FKP", "GBP", "GEL", "GHS", "GIP", "GMD", "GNF", "GTQ", "GYD",
    "HKD", "HNL", "HTG", "HUF", "IDR", "ILS", "INR", "IQD", "IRR", "ISK", "JMD", "JOD", "JPY", "KES", "KGS", "KHR", "KMF", "KPW", "KRW",
    "KWD", "KYD", "KZT", "LAK", "LBP", "LKR", "LRD", "LSL", "LYD", "MAD", "MDL", "MGA", "MKD", "MMK", "MNT", "MOP", "MRU", "MUR", "MVR",
    "MWK", "MXN", "MXV", "MYR", "MZN", "NAD", "NGN", "NIO", "NOK", "NPR", "NZD", "OMR", "PAB", "PEN", "PGK", "PHP", "PKR", "PLN", "PYG",
    "QAR", "RON", "RSD", "RUB", "RWF", "SAR", "SBD", "SCR", "SDG", "SEK", "SGD", "SHP", "SLE", "SOS", "SRD", "SSP", "STN", "SVC", "SYP",
    "SZL", "THB", "TJS", "TMT", "TND", "TOP", "TRY", "TTD", "TWD", "TZS", "UAH", "UGX", "USD", "USN", "UYI", "UYU", "UYW", "UZS", "VED",
    "VES", "VND", "VUV", "WST", "XAF", "XAG", "XAU", "XBA", "XBB", "XBC", "XBD", "XCD", "XCG", "XDR", "XOF", "XPD", "XPF", "XPT", "XSU",
    "XUA", "YER", "ZAR", "ZMW", "ZWG",
];

impl Currency {
    /// Its minor unit, ISO 4217's: the digits after the point an amount of it has (`EUR`
    /// 2, `JPY` 0, `KWD` 3); nothing for a unit that has none (the metals, `XDR`).
    pub fn minor_unit(self) -> Option<u32> {
        const NONE: [&str; 11] = ["XAG", "XAU", "XBA", "XBB", "XBC", "XBD", "XDR", "XPD", "XPT", "XSU", "XUA"];
        const ZERO: [&str; 17] =
            ["BIF", "CLP", "DJF", "GNF", "ISK", "JPY", "KMF", "KRW", "PYG", "RWF", "UGX", "UYI", "VND", "VUV", "XAF", "XOF", "XPF"];
        const THREE: [&str; 7] = ["BHD", "IQD", "JOD", "KWD", "LYD", "OMR", "TND"];
        const FOUR: [&str; 2] = ["CLF", "UYW"];
        let c = self.as_str();
        if NONE.contains(&c) {
            None
        } else if ZERO.contains(&c) {
            Some(0)
        } else if THREE.contains(&c) {
            Some(3)
        } else if FOUR.contains(&c) {
            Some(4)
        } else {
            Some(2)
        }
    }
}

pub fn currency(word: &str) -> Option<Currency> {
    let b = word.as_bytes();
    if b.len() != 3 || CURRENCIES.binary_search(&word).is_err() {
        return None;
    }
    Some(Currency([b[0], b[1], b[2]]))
}

/// The bases spelled by one word; `bool` is built with its words in `parse_type`.
pub(crate) const BASES: [(&str, Base); 10] = [
    ("text", Base::Text),
    ("int", Base::Int),
    ("num", Base::Num),
    ("pct", Base::Pct),
    ("date", Base::Date),
    ("time", Base::Time),
    ("datetime", Base::DateTime),
    ("url", Base::Url),
    ("email", Base::Email),
    ("id", Base::Id),
];

/// The explicit form's words: they end a `one of` list and are never a pair's words.
pub(crate) const EXPLICIT: [&str; 11] = ["key", "optional", "one", "of", "bool", "min", "max", "above", "below", "between", "and"];

/// Inside a type, outside single quotes.
pub(crate) const RESERVED: &[u8] = b"|/!?<>=()':";

/// A bound on a number's value or a text's length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bound {
    pub value: Decimal,
    pub inclusive: bool,
}

/// A date or time's spelling in the file: `DD/MM/YYYY`, `HH:mm`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Format {
    pub spelled: String,
    pub pieces: Vec<Piece>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    Literal(u8),
}

impl Format {
    fn parse(spelled: &str) -> Result<Format, String> {
        if spelled.is_empty() {
            return Err("a format is never empty".to_owned());
        }
        let s = spelled.as_bytes();
        let mut pieces = Vec::new();
        let mut i = 0;
        while i < s.len() {
            let at = |t: &str| s[i..].starts_with(t.as_bytes());
            let (p, n) = if at("YYYY") {
                (Piece::Year, 4)
            } else if at("MM") {
                (Piece::Month, 2)
            } else if at("DD") {
                (Piece::Day, 2)
            } else if at("HH") {
                (Piece::Hour, 2)
            } else if at("mm") {
                (Piece::Minute, 2)
            } else if at("ss") {
                (Piece::Second, 2)
            } else {
                (Piece::Literal(s[i]), 1)
            };
            if !matches!(p, Piece::Literal(_)) && pieces.contains(&p) {
                return Err(format!("the format {spelled} says {} twice", &spelled[i..i + n]));
            }
            pieces.push(p);
            i += n;
        }
        Ok(Format { spelled: spelled.to_owned(), pieces })
    }

    fn has(&self, p: Piece) -> bool {
        self.pieces.contains(&p)
    }

    /// A value in this format: the date's days and the time's seconds, each when the
    /// format has it.
    pub fn write(&self, days: Option<i32>, seconds: Option<i32>) -> String {
        let (y, mo, d) = days.map(crate::value::civil_from_days).unwrap_or((0, 0, 0));
        let s = seconds.unwrap_or(0);
        let mut out = String::new();
        for p in &self.pieces {
            match p {
                Piece::Year => out.push_str(&format!("{y:04}")),
                Piece::Month => out.push_str(&format!("{mo:02}")),
                Piece::Day => out.push_str(&format!("{d:02}")),
                Piece::Hour => out.push_str(&format!("{:02}", s / 3600)),
                Piece::Minute => out.push_str(&format!("{:02}", s / 60 % 60)),
                Piece::Second => out.push_str(&format!("{:02}", s % 60)),
                Piece::Literal(c) => out.push(*c as char),
            }
        }
        out
    }
}

/// A column's type, as read from its header cell or inferred from its values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Type {
    pub base: Base,
    pub min: Option<Bound>,
    pub max: Option<Bound>,
    pub format: Option<Format>,
    /// `!`: present in every row and unique.
    pub key: bool,
    /// `?`: may be blank.
    pub optional: bool,
    /// The prelude's named type the cell used (`Category`), as it was defined.
    pub named: Option<Box<NamedType>>,
    /// Read from the values; the header said nothing.
    pub inferred: bool,
}

/// `type Name = <type>` from the prelude.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedType {
    pub name: String,
    pub ty: Type,
}

impl Type {
    pub fn plain(base: Base) -> Type {
        Type { base, min: None, max: None, format: None, key: false, optional: false, named: None, inferred: false }
    }

    pub fn inferred(base: Base) -> Type {
        Type { optional: true, inferred: true, ..Type::plain(base) }
    }

    /// The shorthand a writer writes: what it holds, then its format, its lower bound, its
    /// upper bound, then `!` or `?`. A named type writes its name and only what the cell
    /// adds to it; a `num` with both bounds inclusive writes its range alone.
    pub fn shorthand(&self) -> String {
        let under = self.named.as_deref().map(|n| &n.ty);
        let mut out = match &self.named {
            Some(n) => n.name.clone(),
            None => base_shorthand(&self.base),
        };
        let mine = |v: &Option<Bound>, theirs: Option<&Option<Bound>>| match theirs {
            Some(t) if t.is_some() => None,
            _ => *v,
        };
        let min = mine(&self.min, under.map(|t| &t.min));
        let max = mine(&self.max, under.map(|t| &t.max));
        if let Some(f) = &self.format
            && under.is_none_or(|t| t.format.is_none())
        {
            out.push('(');
            out.push_str(&f.spelled);
            out.push(')');
        }
        match (min, max) {
            (Some(a), Some(b)) if under.is_none() && self.base == Base::Num && a.inclusive && b.inclusive => {
                out = format!("{}..{}", a.value.normal(), b.value.normal());
            }
            _ => {
                if let Some(a) = min {
                    out.push_str(if a.inclusive { ">=" } else { ">" });
                    out.push_str(&a.value.normal().to_string());
                }
                if let Some(b) = max {
                    out.push_str(if b.inclusive { "<=" } else { "<" });
                    out.push_str(&b.value.normal().to_string());
                }
            }
        }
        if self.key && under.is_none_or(|t| !t.key) {
            out.push('!');
        }
        if self.optional && under.is_none_or(|t| !t.optional) {
            out.push('?');
        }
        out
    }
}

fn base_shorthand(base: &Base) -> String {
    match base {
        Base::Bool { yes, no } if yes == "true" && no == "false" => "bool".to_owned(),
        Base::Bool { yes, no } => format!("{}/{}", quote(yes), quote(no)),
        Base::Money(c) => c.as_str().to_owned(),
        Base::Choice(words) if words.len() == 1 => quoted(&words[0]),
        Base::Choice(words) => {
            let mut s = String::new();
            for (k, w) in words.iter().enumerate() {
                if k > 0 {
                    s.push('|');
                }
                s.push_str(&quote(w));
            }
            s
        }
        Base::Ref(targets) => targets.iter().map(|t| format!("{}.{}", t.table, t.column)).collect::<Vec<_>>().join("|"),
        other => other.tag().to_owned(),
    }
}

fn quoted(word: &str) -> String {
    format!("'{}'", word.replace('\'', "''"))
}

/// A word bare, or quoted when it holds a space or a reserved character.
fn quote(word: &str) -> String {
    let bare = !word.bytes().any(|c| c == b' ' || c == b'\t' || RESERVED.contains(&c)) && !word.contains("..");
    if bare { word.to_owned() } else { quoted(word) }
}

#[derive(Clone, Debug, PartialEq)]
enum Token<'a> {
    Word(&'a str),
    Quoted(String),
    Op(&'static str),
    Format(&'a str),
}

impl Token<'_> {
    /// A word that is a choice's or a pair's: bare, or quoted.
    fn text(&self) -> Option<&str> {
        match self {
            Token::Word(w) => Some(w),
            Token::Quoted(q) => Some(q),
            _ => None,
        }
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self, Token::Word(x) if *x == w)
    }

    fn is_op(&self, o: &str) -> bool {
        matches!(self, Token::Op(x) if *x == o)
    }

    /// A word a `one of` list or a pair takes: quoted, or bare and no explicit word.
    fn listed(&self) -> Option<&str> {
        match self {
            Token::Word(w) if !EXPLICIT.contains(w) => Some(w),
            Token::Quoted(q) => Some(q),
            _ => None,
        }
    }

    fn shown(&self) -> String {
        match self {
            Token::Word(w) => (*w).to_owned(),
            Token::Quoted(q) => quoted(q),
            Token::Op(o) => (*o).to_owned(),
            Token::Format(f) => format!("({f})"),
        }
    }
}

pub(crate) const OPS: [&str; 9] = [">=", "<=", "..", "|", "/", "!", "?", ">", "<"];

fn tokens(spelled: &str) -> Result<Vec<Token<'_>>, String> {
    let s = spelled.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        if c == b' ' || c == b'\t' {
            i += 1;
            continue;
        }
        if c == b'\'' {
            let mut v = String::new();
            i += 1;
            let mut from = i;
            loop {
                if i >= s.len() {
                    return Err("a quote is not closed".to_owned());
                }
                if s[i] == b'\'' {
                    v.push_str(&spelled[from..i]);
                    if s.get(i + 1) == Some(&b'\'') {
                        v.push('\'');
                        i += 2;
                        from = i;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push(Token::Quoted(v));
            continue;
        }
        if c == b'(' {
            let Some(end) = spelled[i..].find(')') else {
                return Err("a format's ( is not closed".to_owned());
            };
            out.push(Token::Format(&spelled[i + 1..i + end]));
            i += end + 1;
            continue;
        }
        if let Some(op) = OPS.iter().find(|o| s[i..].starts_with(o.as_bytes())) {
            out.push(Token::Op(op));
            i += op.len();
            continue;
        }
        let start = i;
        while i < s.len() && s[i] != b' ' && s[i] != b'\t' && !RESERVED.contains(&s[i]) && !s[i..].starts_with(b"..") {
            i += 1;
        }
        if i == start {
            return Err(format!("\"{}\" cannot stand in a type", c as char));
        }
        out.push(Token::Word(&spelled[start..i]));
    }
    Ok(out)
}

fn number(t: Option<&Token>) -> Result<Decimal, String> {
    match t {
        Some(Token::Word(w)) => Decimal::parse(w.as_bytes()).map_err(|_| format!("a number was expected, not \"{w}\"")),
        Some(other) => Err(format!("a number was expected, not \"{}\"", other.shown())),
        None => Err("a number was expected, not the end".to_owned()),
    }
}

/// What a type has said so far, a named type's parts included.
#[derive(Default)]
struct Said {
    min: bool,
    max: bool,
    format: bool,
    key: bool,
    optional: bool,
}

fn starts_bound(t: &[Token], i: usize) -> bool {
    match &t[i] {
        Token::Op(o) => matches!(*o, ">=" | ">" | "<=" | "<"),
        Token::Word(w) => matches!(*w, "min" | "max" | "above" | "below" | "between") || t.get(i + 1).is_some_and(|n| n.is_op("..")),
        _ => false,
    }
}

/// `table.column` when `table` is one of `tables`.
fn target<'a>(t: &Token<'a>, tables: &[String]) -> Option<Target> {
    let Token::Word(w) = t else { return None };
    let (table, column) = w.split_once('.')?;
    (!column.is_empty() && tables.iter().any(|n| n == table)).then(|| Target { table: table.to_owned(), column: column.to_owned() })
}

/// What the column holds, from the type's first tokens.
fn holder(t: &[Token], i: &mut usize, named: &[NamedType], tables: &[String]) -> Result<Type, String> {
    let x = &t[*i];
    let next = t.get(*i + 1);
    if let Some(first) = target(x, tables) {
        let mut targets = Vec::from([first]);
        *i += 1;
        while t.get(*i).is_some_and(|n| n.is_op("|")) {
            let Some(more) = t.get(*i + 1).and_then(|n| target(n, tables)) else {
                return Err("a reference joins other references with |: shared.account|clients.account".to_owned());
            };
            if targets.contains(&more) {
                return Err(format!("{}.{} is named twice", more.table, more.column));
            }
            targets.push(more);
            *i += 2;
        }
        return Ok(Type::plain(Base::Ref(targets)));
    }
    if let Some(word) = x.text()
        && next.is_some_and(|n| n.is_op("|"))
    {
        let mut words = Vec::from([word.to_owned()]);
        *i += 1;
        while t.get(*i).is_some_and(|n| n.is_op("|")) {
            let Some(w) = t.get(*i + 1).and_then(|n| n.text()) else {
                return Err("a | is followed by no choice".to_owned());
            };
            words.push(w.to_owned());
            *i += 2;
        }
        return Ok(Type::plain(Base::Choice(choices(words)?)));
    }
    if let Some(yes) = x.text()
        && next.is_some_and(|n| n.is_op("/"))
    {
        let Some(no) = t.get(*i + 2).and_then(|n| n.text()) else {
            return Err("a / is followed by no word for false".to_owned());
        };
        *i += 3;
        return Ok(Type::plain(pair(yes, no)?));
    }
    if let Token::Quoted(q) = x {
        *i += 1;
        return Ok(Type::plain(Base::Choice(choices(Vec::from([q.clone()]))?)));
    }
    if x.is_word("one") && next.is_some_and(|n| n.is_word("of")) {
        *i += 2;
        let mut words = Vec::new();
        while let Some(w) = t.get(*i).and_then(Token::listed) {
            words.push(w.to_owned());
            *i += 1;
        }
        if words.is_empty() {
            return Err("one of names no choice".to_owned());
        }
        return Ok(Type::plain(Base::Choice(choices(words)?)));
    }
    if x.is_word("bool") {
        if let (Some(yes), Some(no)) = (next.and_then(Token::listed), t.get(*i + 2).and_then(Token::listed)) {
            *i += 3;
            return Ok(Type::plain(pair(yes, no)?));
        }
        *i += 1;
        return Ok(Type::plain(Base::Bool { yes: "true".to_owned(), no: "false".to_owned() }));
    }
    match x {
        Token::Op("!" | "?") | Token::Word("key" | "optional") => Err("a mark comes after what the column holds".to_owned()),
        Token::Word(w) => {
            *i += 1;
            if let Some((_, b)) = BASES.iter().find(|(n, _)| n == w) {
                Ok(Type::plain(b.clone()))
            } else if let Some(c) = currency(w) {
                Ok(Type::plain(Base::Money(c)))
            } else if let Some(n) = named.iter().find(|n| n.name == *w) {
                Ok(Type { named: Some(Box::new(n.clone())), ..n.ty.clone() })
            } else if let Some((table, column)) = w.split_once('.') {
                // `table.column`, a reference to a table the prelude does not name, or to
                // no column of one it does.
                match tables.iter().any(|n| n == table) {
                    true if column.is_empty() => Err(format!("a reference names {table}'s key: {table}.<key>")),
                    _ => Err(format!("no table \"{table}\": the prelude names a table by an import or inline")),
                }
            } else {
                Err(format!("no type or currency \"{w}\""))
            }
        }
        other => Err(format!("\"{}\" cannot open a type", other.shown())),
    }
}

/// A type from what follows a header cell's `:` (its formula cut, `split_formula`).
/// `named` are the prelude's types defined so far, `tables` the tables a reference may
/// name. The error is why, without the place; the caller knows the place.
pub fn parse_type(spelled: &str, named: &[NamedType], tables: &[String]) -> Result<Type, String> {
    if spelled.contains(',') || spelled.contains('"') {
        return Err("a type never holds , or \"".to_owned());
    }
    let t = tokens(spelled)?;
    if t.is_empty() {
        return Err("no type".to_owned());
    }
    let mut i = 0;
    let mut ty = if starts_bound(&t, 0) { Type::plain(Base::Num) } else { holder(&t, &mut i, named, tables)? };
    let mut said = match ty.named.as_deref() {
        Some(n) => {
            Said { min: n.ty.min.is_some(), max: n.ty.max.is_some(), format: n.ty.format.is_some(), key: n.ty.key, optional: n.ty.optional }
        }
        None => Said::default(),
    };
    let mut format: Option<&str> = None;
    let lower = |ty: &mut Type, said: &mut Said, value: Decimal, inclusive: bool| {
        if said.min {
            return Err("the lower bound is said twice".to_owned());
        }
        said.min = true;
        ty.min = Some(Bound { value, inclusive });
        Ok(())
    };
    let upper = |ty: &mut Type, said: &mut Said, value: Decimal, inclusive: bool| {
        if said.max {
            return Err("the upper bound is said twice".to_owned());
        }
        said.max = true;
        ty.max = Some(Bound { value, inclusive });
        Ok(())
    };
    while i < t.len() {
        let x = &t[i];
        let next = t.get(i + 1);
        match x {
            Token::Format(f) => {
                if said.format {
                    return Err("the format is said twice".to_owned());
                }
                said.format = true;
                format = Some(f);
                i += 1;
            }
            Token::Op(op @ (">=" | ">" | "<=" | "<")) => {
                let v = number(next)?;
                match *op {
                    ">=" => lower(&mut ty, &mut said, v, true)?,
                    ">" => lower(&mut ty, &mut said, v, false)?,
                    "<=" => upper(&mut ty, &mut said, v, true)?,
                    _ => upper(&mut ty, &mut said, v, false)?,
                }
                i += 2;
            }
            Token::Word("min") => {
                lower(&mut ty, &mut said, number(next)?, true)?;
                i += 2;
            }
            Token::Word("above") => {
                lower(&mut ty, &mut said, number(next)?, false)?;
                i += 2;
            }
            Token::Word("max") => {
                upper(&mut ty, &mut said, number(next)?, true)?;
                i += 2;
            }
            Token::Word("below") => {
                upper(&mut ty, &mut said, number(next)?, false)?;
                i += 2;
            }
            Token::Word("between") => {
                if !t.get(i + 2).is_some_and(|a| a.is_word("and")) {
                    return Err("between takes two numbers joined by and: between 0 and 100".to_owned());
                }
                lower(&mut ty, &mut said, number(next)?, true)?;
                upper(&mut ty, &mut said, number(t.get(i + 3))?, true)?;
                i += 4;
            }
            Token::Word(w) if next.is_some_and(|n| n.is_op("..")) => {
                let lo = number(Some(x)).map_err(|_| format!("a range opens with a number, not \"{w}\""))?;
                lower(&mut ty, &mut said, lo, true)?;
                upper(&mut ty, &mut said, number(t.get(i + 2))?, true)?;
                i += 3;
            }
            Token::Op("!") | Token::Word("key") => {
                if said.key {
                    return Err("the key is said twice".to_owned());
                }
                said.key = true;
                ty.key = true;
                i += 1;
            }
            Token::Op("?") | Token::Word("optional") => {
                if said.optional {
                    return Err("optional is said twice".to_owned());
                }
                said.optional = true;
                ty.optional = true;
                i += 1;
            }
            Token::Word(w)
                if BASES.iter().any(|(n, _)| n == w) || currency(w).is_some() || named.iter().any(|n| n.name == *w) || *w == "bool" =>
            {
                return Err("two types in one cell".to_owned());
            }
            Token::Word(w) if !EXPLICIT.contains(w) => return Err(format!("no type or currency \"{w}\"")),
            other => return Err(format!("\"{}\" cannot stand here", other.shown())),
        }
    }
    check(&mut ty, format)?;
    Ok(ty)
}

fn check(ty: &mut Type, format: Option<&str>) -> Result<(), String> {
    if ty.min.is_some() || ty.max.is_some() {
        if !(ty.base.is_number() || ty.base == Base::Text) {
            let tag = ty.base.tag();
            let a = if tag.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
            return Err(format!("{a} {tag} takes no bounds; only a number or text does"));
        }
        if ty.base == Base::Text {
            for b in [ty.min, ty.max].into_iter().flatten() {
                let v = b.value.normal();
                if v.scale != 0 || v.mantissa < 0 {
                    return Err(format!("a text's length is a whole number, at least 0, not {}", b.value));
                }
            }
        }
        if let (Some(a), Some(b)) = (ty.min, ty.max) {
            let o = a.value.cmp(&b.value);
            if o.is_gt() || (o.is_eq() && !(a.inclusive && b.inclusive)) {
                return Err("no value meets these bounds".to_owned());
            }
        }
    }
    if let Some(f) = format {
        if !matches!(ty.base, Base::Date | Base::Time) {
            return Err("only a date or a time takes a format".to_owned());
        }
        let f = Format::parse(f)?;
        let date = [Piece::Year, Piece::Month, Piece::Day];
        let time = [Piece::Hour, Piece::Minute, Piece::Second];
        let ok = match ty.base {
            Base::Date => date.iter().all(|p| f.has(*p)) && !time.iter().any(|p| f.has(*p)),
            _ => f.has(Piece::Hour) && f.has(Piece::Minute) && !date.iter().any(|p| f.has(*p)),
        };
        if !ok {
            let needs = if ty.base == Base::Date { "YYYY, MM and DD, and no time" } else { "HH and mm, ss or not, and no date" };
            return Err(format!("the format {} is no {}: it holds {needs}", f.spelled, ty.base.tag()));
        }
        ty.format = Some(f);
    }
    if ty.key && ty.optional {
        return Err("a key is present in every row, so it is never optional".to_owned());
    }
    Ok(())
}

fn choices(words: Vec<String>) -> Result<Vec<String>, String> {
    for (k, w) in words.iter().enumerate() {
        if w.is_empty() {
            return Err("a choice is never blank".to_owned());
        }
        if words[..k].contains(w) {
            return Err(format!("the choice \"{w}\" is written twice"));
        }
    }
    Ok(words)
}

fn pair(yes: &str, no: &str) -> Result<Base, String> {
    if yes.is_empty() || no.is_empty() {
        return Err("a boolean's word is never blank".to_owned());
    }
    if yes == no {
        return Err(format!("true and false are both \"{yes}\""));
    }
    Ok(Base::Bool { yes: yes.to_owned(), no: no.to_owned() })
}

/// A header cell: its name before the first `:` and the type after it, or no type for a
/// plain column.
pub fn split_header(cell: &str) -> (&str, Option<&str>) {
    match cell.find(':') {
        Some(at) => (&cell[..at], Some(&cell[at + 1..])),
        None => (cell, None),
    }
}

/// What follows a header cell's `:` split at its formula: the type, and what follows the
/// first `=` outside single quotes that closes no `>=` or `<=`.
pub fn split_formula(spelled: &str) -> (&str, Option<&str>) {
    let b = spelled.as_bytes();
    let mut quoted = false;
    for i in 0..b.len() {
        match b[i] {
            b'\'' => quoted = !quoted,
            b'=' if !quoted && !(i > 0 && matches!(b[i - 1], b'<' | b'>')) => return (&spelled[..i], Some(&spelled[i + 1..])),
            _ => {}
        }
    }
    (spelled, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(s: &str) -> String {
        parse_type(s, &[], &[]).unwrap_or_else(|e| panic!("{s}: {e}")).shorthand()
    }

    #[test]
    fn currencies_sorted() {
        assert!(CURRENCIES.windows(2).all(|w| w[0] < w[1]));
        assert!(currency("XAU").is_some() && currency("XTS").is_none() && currency("HRK").is_none());
    }

    #[test]
    fn shorthand_both_ways() {
        assert_eq!(sh("text!"), "text!");
        assert_eq!(sh("text key"), "text!");
        assert_eq!(sh("text optional"), "text?");
        assert_eq!(sh("one of travel stay food"), "travel|stay|food");
        assert_eq!(sh("bool yes no"), "yes/no");
        assert_eq!(sh("bool true false"), "bool");
        assert_eq!(sh("true/false"), "bool");
        assert_eq!(sh("EUR min 0"), "EUR>=0");
        assert_eq!(sh("num max 10"), "num<=10");
        assert_eq!(sh("num between 0 and 100"), "0..100");
        assert_eq!(sh("int above 0"), "int>0");
        assert_eq!(sh(">=0"), "num>=0");
        assert_eq!(sh("int 1..5"), "int>=1<=5");
        assert_eq!(sh("num max 1e3 min 0.50"), "0.5..1000");
        assert_eq!(sh("one of travel 'sd cards'"), "travel|'sd cards'");
        assert_eq!(sh("'yay!'|meh|'so-so?'|'it''s fine'"), "'yay!'|meh|'so-so?'|'it''s fine'");
        assert_eq!(sh("travel | stay"), "travel|stay");
        assert_eq!(sh("key|lock"), "key|lock");
        assert_eq!(sh("date?(DD/MM/YYYY)"), "date(DD/MM/YYYY)?");
        assert_eq!(sh("'only'"), "'only'");
        assert_eq!(sh("EUR min 0 key"), "EUR>=0!");
        assert_eq!(sh("one of 'so so' meh optional"), "'so so'|meh?");
        assert_eq!(sh("id!"), "id!");
    }

    #[test]
    fn refusals() {
        for bad in [
            "EURO",
            "int text",
            "yes/no>=1",
            "text(DD)",
            "date(MM/YYYY)",
            "'open",
            "",
            "text!?",
            "5..1",
            "!text",
            "optional text",
            "0..5 int",
            "text??",
            "text! key",
            "a:b|c",
            "|a|b",
            "a|b|",
            "a|a",
            "''",
            "a/b/c",
            "bool yes",
            "datetime(YYYY)",
            "text<1.5",
            "text>=-1",
            ">5<5",
            "date()",
            "one of key",
            "int min",
            "id>=1",
            "eur",
        ] {
            assert!(parse_type(bad, &[], &[]).is_err(), "{bad:?} should not read");
        }
    }

    #[test]
    fn named() {
        let cat = NamedType { name: "Category".into(), ty: parse_type("travel|stay", &[], &[]).unwrap() };
        let t = parse_type("Category?", core::slice::from_ref(&cat), &[]).unwrap();
        assert_eq!(t.base, Base::Choice(alloc::vec!["travel".into(), "stay".into()]));
        assert!(t.optional);
        assert_eq!(t.shorthand(), "Category?");
        let opt = NamedType { name: "Note".into(), ty: parse_type("text?", &[], &[]).unwrap() };
        assert!(parse_type("Note?", core::slice::from_ref(&opt), &[]).is_err());
    }

    #[test]
    fn references_and_formulas() {
        let tables = ["accounts".to_owned(), "clients".to_owned()];
        let t = parse_type("accounts.account|clients.account?", &[], &tables).unwrap();
        assert_eq!(t.shorthand(), "accounts.account|clients.account?");
        assert!(matches!(&t.base, Base::Ref(r) if r.len() == 2));
        assert!(parse_type("accounts.account", &[], &[]).is_err());
        assert_eq!(split_formula("EUR>=0=cost*qty"), ("EUR>=0", Some("cost*qty")));
        assert_eq!(split_formula("'a=b'|c"), ("'a=b'|c", None));
        assert_eq!(split_formula("int<=5"), ("int<=5", None));
    }
}
