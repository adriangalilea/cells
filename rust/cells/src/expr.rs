//! The expression language (SPEC.md, Formulas): SQL's scalar expressions in exact decimal,
//! pure and total. A formula is parsed into an `Expr`, resolved against what it may read
//! (`Scope`) into an `Ir` with every name a place and every operand's kind known, and
//! evaluated a row at a time against a `Source`.

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::exact::Exact;

/// A value inside an expression.
#[derive(Clone, Debug, PartialEq)]
pub enum V {
    Null,
    Num(Exact),
    Text(String),
    Bool(bool),
    /// Days since 1970-01-01.
    Date(i32),
}

impl V {
    /// The value as a key is compared: a number in its plainest decimal, a date ISO.
    pub fn canonical(&self) -> Option<String> {
        Some(match self {
            V::Null => return None,
            V::Num(n) => {
                let mut n = n.clone();
                n.normalize();
                n.to_string()
            }
            V::Text(t) => t.clone(),
            V::Bool(b) => b.to_string(),
            V::Date(d) => crate::value::format_date(*d),
        })
    }
}

/// What an expression gives, known before it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum K {
    Num,
    Text,
    Bool,
    Date,
    /// `null`: any kind.
    Null,
}

impl K {
    pub fn word(self) -> &'static str {
        match self {
            K::Num => "a number",
            K::Text => "text",
            K::Bool => "a boolean",
            K::Date => "a date",
            K::Null => "null",
        }
    }

    /// The kind two operands share, `null` going with either.
    fn meet(self, b: K) -> Option<K> {
        match (self, b) {
            (K::Null, x) | (x, K::Null) => Some(x),
            (a, b) if a == b => Some(a),
            (K::Text, K::Date) | (K::Date, K::Text) => Some(K::Text),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bin {
    Add,
    Sub,
    Mul,
    Div,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// A formula as written.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(Exact),
    Text(String),
    Bool(bool),
    Null,
    /// `cost`, `to.kind`, `fees.pct`: names joined by `.`, where they are written.
    Path(Vec<String>, Sp),
    /// `fees['card'].pct`: a table's row by its key, then a column of it; the table's name
    /// where it is written.
    Lookup(String, Box<Expr>, Vec<String>, Sp),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    /// An operator, where it is written, between its operands.
    Bin(Bin, Box<Expr>, Box<Expr>, Sp),
    /// `x is null`, or `x is not null` when true.
    IsNull(Box<Expr>, bool),
    Case(Vec<(Expr, Expr)>, Option<Box<Expr>>),
    /// A function, its name where it is written.
    Call(String, Vec<Expr>, Sp),
    /// A window: an aggregate or `LAG` over the file's rows in an order (each key, and
    /// whether it descends), `OVER` where it is written.
    Over(Box<Expr>, Vec<(Expr, bool)>, Sp),
    /// `running[-1]`, `cost[FIRST].kind`: a column on a row by where it is from this one,
    /// then the references followed from it; the column's name where it is written.
    At(String, Place, Vec<String>, Sp),
    /// `total[FIRST:THIS]`: a column on the rows from one place to another, an aggregate's
    /// argument; the column's name where it is written.
    Range(String, Place, Place, Sp),
}

/// A row by where it is from the row being computed, in the order of the file's lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    First,
    This,
    Last,
    /// That many lines below (or above, when negative).
    By(i64),
}

impl Place {
    /// The row it is from `row` in a table of `rows`: past the rows when outside them.
    fn row(self, row: usize, rows: usize) -> i64 {
        match self {
            Place::First => 0,
            Place::This => row as i64,
            Place::Last => rows as i64 - 1,
            Place::By(n) => row as i64 + n,
        }
    }

    /// Where it is from this row, `FIRST` before every row and `LAST` after: what says
    /// whether a range holds the row it is computed on.
    fn reach(self) -> i128 {
        match self {
            Place::First => i128::MIN,
            Place::This => 0,
            Place::Last => i128::MAX,
            Place::By(n) => n as i128,
        }
    }
}

/// Whether a range from `a` to `b` holds the row it is read on.
fn holds_this(a: Place, b: Place) -> bool {
    a.reach() <= 0 && 0 <= b.reach()
}

/// Where a part of a formula is in its text: its first byte and its length.
pub type Sp = (u32, u32);

/// Why a formula does not read, and where in its text: the part that broke, or nothing
/// when the whole of it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    pub why: String,
    pub at: Option<Sp>,
}

impl Fault {
    /// The formula as a whole.
    pub fn whole(why: String) -> Fault {
        Fault { why, at: None }
    }

    fn at(at: Sp, why: String) -> Fault {
        Fault { why, at: Some(at) }
    }
}

fn sp(from: usize, to: usize) -> Sp {
    (from as u32, (to - from) as u32)
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(String),
    Str(String),
    Name(String),
    /// A `"quoted name"`: never a keyword.
    Quoted(String),
    Sym(&'static str),
}

const SYMS: [&str; 19] = ["||", "<>", "!=", "<=", ">=", "+", "-", "*", "/", "(", ")", "[", "]", ",", ".", "=", "<", ">", ":"];

/// The language's own words, written in capitals and only so (SPEC.md, The language): a
/// word in lower case is always a name the file gave.
pub const KEYWORDS: [&str; 22] = [
    "AND", "OR", "NOT", "IS", "NULL", "TRUE", "FALSE", "CASE", "WHEN", "THEN", "ELSE", "END", "OVER", "ORDER", "BY", "ASC", "DESC",
    "TABLE", "FIRST", "THIS", "LAST", "LAG",
];

/// Whether `w` is one of the language's words or functions.
fn language(w: &str) -> bool {
    KEYWORDS.contains(&w) || FUNCTIONS.contains(&w)
}

/// A word written in capitals: no lower-case letter, and a letter.
fn capitals(w: &str) -> bool {
    w.bytes().any(|c| c.is_ascii_uppercase()) && !w.bytes().any(|c| c.is_ascii_lowercase())
}

/// What a name in lower case that spells one of the language's words says: the word is
/// written in capitals.
fn spelled_low(w: &str) -> Option<String> {
    let up = w.to_ascii_uppercase();
    (up != w && language(&up)).then(|| format!("\"{w}\" is a name: the language is written in capitals, {up}"))
}

/// The formula's tokens, each with where it is written.
fn lex(src: &str) -> Result<Vec<(Tok, Sp)>, Fault> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let start = i;
        if c == b' ' || c == b'\t' {
            i += 1;
            continue;
        } else if c.is_ascii_digit() {
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if b.get(i) == Some(&b'.') && b.get(i + 1).is_some_and(|d| d.is_ascii_digit()) {
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            out.push((Tok::Num(src[start..i].to_owned()), sp(start, i)));
        } else if c == b'\'' || c == b'"' {
            let mut v = String::new();
            i += 1;
            let mut from = i;
            loop {
                if i >= b.len() {
                    let why = if c == b'\'' { "a text's ' is not closed" } else { "a name's \" is not closed" };
                    return Err(Fault::at(sp(start, b.len()), why.to_owned()));
                }
                if b[i] == c {
                    v.push_str(&src[from..i]);
                    if b.get(i + 1) == Some(&c) {
                        v.push(c as char);
                        i += 2;
                        from = i;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push((if c == b'\'' { Tok::Str(v) } else { Tok::Quoted(v) }, sp(start, i)));
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push((Tok::Name(src[start..i].to_owned()), sp(start, i)));
        } else if let Some(s) = SYMS.iter().find(|s| b[i..].starts_with(s.as_bytes())) {
            i += s.len();
            out.push((Tok::Sym(s), sp(start, i)));
        } else if c == b'#' {
            // `#3`, a row by where a screen shows it: input an app or the language server
            // turns into the row's key, never what a file holds.
            let end = i + 1 + b[i + 1..].iter().take_while(|d| d.is_ascii_digit()).count();
            let why = format!("{} is a row by where it shows, which a file never holds: write its key, TABLE['…']", &src[i..end]);
            return Err(Fault::at(sp(i, end), why));
        } else {
            let ch = src[i..].chars().next().unwrap();
            return Err(Fault::at(sp(i, i + ch.len_utf8()), format!("\"{ch}\" cannot stand in a formula")));
        }
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    spans: Vec<Sp>,
    at: usize,
    /// The formula's length: where its end is.
    len: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    /// Where the next token is written, or the formula's end.
    fn here(&self) -> Sp {
        self.spans.get(self.at).copied().unwrap_or((self.len as u32, 0))
    }

    /// Where the token before the next is written.
    fn last(&self) -> Sp {
        self.spans[self.at - 1]
    }

    /// Why the formula does not read, at the next token.
    fn fail(&self, why: String) -> Fault {
        Fault::at(self.here(), why)
    }

    fn sym(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Sym(x)) if *x == s)
    }

    /// Whether the next token is the language's word `w`, written as it is, in capitals.
    fn word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Name(x)) if x == w)
    }

    fn eat_sym(&mut self, s: &str) -> bool {
        let yes = self.sym(s);
        self.at += yes as usize;
        yes
    }

    fn eat_word(&mut self, w: &str) -> bool {
        let yes = self.word(w);
        self.at += yes as usize;
        yes
    }

    fn expect_sym(&mut self, s: &str) -> Result<(), Fault> {
        if self.eat_sym(s) { Ok(()) } else { Err(self.fail(format!("a {s} was expected, not {}", self.shown()))) }
    }

    /// Why the next token stands where nothing can: what `what` says, or, for a name in
    /// lower case that spells the language's word, that the word is in capitals.
    fn stray(&self, what: String) -> Fault {
        match self.peek() {
            Some(Tok::Name(n)) => self.fail(spelled_low(n).unwrap_or(what)),
            _ => self.fail(what),
        }
    }

    /// A row by where it is, inside a column's `[…]`: `-1`, `+2`, `FIRST`, `THIS`, `LAST`;
    /// nothing when what is there is a key.
    fn place(&mut self) -> Option<Place> {
        let p = match (self.toks.get(self.at), self.toks.get(self.at + 1)) {
            (Some(Tok::Name(w)), _) if w == "FIRST" => (Place::First, 1),
            (Some(Tok::Name(w)), _) if w == "THIS" => (Place::This, 1),
            (Some(Tok::Name(w)), _) if w == "LAST" => (Place::Last, 1),
            (Some(Tok::Sym(s @ ("-" | "+"))), Some(Tok::Num(n))) if !n.contains('.') => {
                let n: i64 = n.parse().ok()?;
                (Place::By(if *s == "-" { -n } else { n }), 2)
            }
            _ => return None,
        };
        // A place stands alone in its brackets, or at an end of a range.
        if !matches!(self.toks.get(self.at + p.1), Some(Tok::Sym("]" | ":"))) {
            return None;
        }
        self.at += p.1;
        Some(p.0)
    }

    fn shown(&self) -> String {
        match self.peek() {
            None => "the end".to_owned(),
            Some(Tok::Num(n) | Tok::Name(n)) => format!("\"{n}\""),
            Some(Tok::Str(s)) => format!("'{s}'"),
            Some(Tok::Quoted(s)) => format!("\"{s}\""),
            Some(Tok::Sym(s)) => format!("\"{s}\""),
        }
    }

    fn or(&mut self) -> Result<Expr, Fault> {
        let mut a = self.and()?;
        while self.eat_word("OR") {
            let op = self.last();
            a = Expr::Bin(Bin::Or, Box::new(a), Box::new(self.and()?), op);
        }
        Ok(a)
    }

    fn and(&mut self) -> Result<Expr, Fault> {
        let mut a = self.not()?;
        while self.eat_word("AND") {
            let op = self.last();
            a = Expr::Bin(Bin::And, Box::new(a), Box::new(self.not()?), op);
        }
        Ok(a)
    }

    fn not(&mut self) -> Result<Expr, Fault> {
        if self.eat_word("NOT") {
            return Ok(Expr::Not(Box::new(self.not()?)));
        }
        self.compare()
    }

    fn compare(&mut self) -> Result<Expr, Fault> {
        let a = self.concat()?;
        if self.eat_word("IS") {
            let negated = self.eat_word("NOT");
            if !self.eat_word("NULL") {
                return Err(self.stray(format!("IS takes NULL, not {}", self.shown())));
            }
            return Ok(Expr::IsNull(Box::new(a), negated));
        }
        let op = match self.peek() {
            Some(Tok::Sym("=")) => Bin::Eq,
            Some(Tok::Sym("<>" | "!=")) => Bin::Ne,
            Some(Tok::Sym("<")) => Bin::Lt,
            Some(Tok::Sym("<=")) => Bin::Le,
            Some(Tok::Sym(">")) => Bin::Gt,
            Some(Tok::Sym(">=")) => Bin::Ge,
            _ => return Ok(a),
        };
        self.at += 1;
        let at = self.last();
        Ok(Expr::Bin(op, Box::new(a), Box::new(self.concat()?), at))
    }

    fn concat(&mut self) -> Result<Expr, Fault> {
        let mut a = self.sum()?;
        while self.eat_sym("||") {
            let op = self.last();
            a = Expr::Bin(Bin::Concat, Box::new(a), Box::new(self.sum()?), op);
        }
        Ok(a)
    }

    fn sum(&mut self) -> Result<Expr, Fault> {
        let mut a = self.product()?;
        loop {
            let op = if self.eat_sym("+") {
                Bin::Add
            } else if self.eat_sym("-") {
                Bin::Sub
            } else {
                return Ok(a);
            };
            let at = self.last();
            a = Expr::Bin(op, Box::new(a), Box::new(self.product()?), at);
        }
    }

    fn product(&mut self) -> Result<Expr, Fault> {
        let mut a = self.unary()?;
        loop {
            let op = if self.eat_sym("*") {
                Bin::Mul
            } else if self.eat_sym("/") {
                Bin::Div
            } else {
                return Ok(a);
            };
            let at = self.last();
            a = Expr::Bin(op, Box::new(a), Box::new(self.unary()?), at);
        }
    }

    fn unary(&mut self) -> Result<Expr, Fault> {
        if self.eat_sym("-") {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn name(&mut self) -> Result<String, Fault> {
        match self.peek().cloned() {
            Some(Tok::Name(n) | Tok::Quoted(n)) => {
                self.at += 1;
                Ok(n)
            }
            _ => Err(self.fail(format!("a name was expected, not {}", self.shown()))),
        }
    }

    fn primary(&mut self) -> Result<Expr, Fault> {
        let Some(t) = self.peek().cloned() else { return Err(self.fail("the formula ends early".to_owned())) };
        match t {
            Tok::Num(n) => {
                self.at += 1;
                Ok(Expr::Num(Exact::parse(&n)))
            }
            Tok::Str(s) => {
                self.at += 1;
                Ok(Expr::Text(s))
            }
            Tok::Sym("(") => {
                let open = self.here();
                self.at += 1;
                let e = self.or()?;
                if !self.eat_sym(")") {
                    return Err(Fault::at(open, "a ( is not closed".to_owned()));
                }
                Ok(e)
            }
            Tok::Name(n) if n == "TRUE" || n == "FALSE" => {
                self.at += 1;
                Ok(Expr::Bool(n == "TRUE"))
            }
            Tok::Name(n) if n == "NULL" => {
                self.at += 1;
                Ok(Expr::Null)
            }
            Tok::Name(n) if n == "CASE" => {
                let case = self.here();
                self.at += 1;
                let mut arms = Vec::new();
                while self.eat_word("WHEN") {
                    let w = self.or()?;
                    if !self.eat_word("THEN") {
                        return Err(self.stray(format!("WHEN takes THEN, not {}", self.shown())));
                    }
                    arms.push((w, self.or()?));
                }
                if arms.is_empty() {
                    return Err(Fault::at(case, "CASE takes WHEN".to_owned()));
                }
                let otherwise = if self.eat_word("ELSE") { Some(Box::new(self.or()?)) } else { None };
                if !self.eat_word("END") {
                    return Err(self.stray(format!("CASE closes with END, not {}", self.shown())));
                }
                Ok(Expr::Case(arms, otherwise))
            }
            // The language's other words stand only where they belong.
            Tok::Name(n) if KEYWORDS.contains(&n.as_str()) && !matches!(n.as_str(), "TABLE" | "LAG") => {
                Err(self.fail(format!("{n} cannot stand there")))
            }
            Tok::Name(_) | Tok::Quoted(_) => {
                let named = self.here();
                let first = self.name()?;
                // A word in capitals is the language's: a name the file gave so is written
                // quoted, so the two are never one another.
                if matches!(t, Tok::Name(_)) && capitals(&first) && first != "TABLE" && !self.sym("(") {
                    return Err(Fault::at(
                        named,
                        format!("{first} is in capitals, the language's: a name the file gave so is written quoted, \"{first}\""),
                    ));
                }
                // The language's `TABLE`, never a quoted name that spells it.
                let first = if matches!(t, Tok::Name(_)) && first == "TABLE" { THIS.to_owned() } else { first };
                if matches!(t, Tok::Name(_)) && self.eat_sym("(") {
                    let mut args = Vec::new();
                    if !self.eat_sym(")") {
                        loop {
                            args.push(self.or()?);
                            if self.eat_sym(")") {
                                break;
                            }
                            self.expect_sym(",")?;
                        }
                    }
                    let call = Expr::Call(first, args, named);
                    let over = self.here();
                    if !self.eat_word("OVER") {
                        return Ok(call);
                    }
                    self.expect_sym("(")?;
                    if !(self.eat_word("ORDER") && self.eat_word("BY")) {
                        return Err(self.stray(format!("OVER takes ORDER BY: OVER (ORDER BY date), not {}", self.shown())));
                    }
                    let mut order = Vec::new();
                    loop {
                        let key = self.or()?;
                        let descending = self.eat_word("DESC");
                        if !descending {
                            self.eat_word("ASC");
                        }
                        order.push((key, descending));
                        if !self.eat_sym(",") {
                            break;
                        }
                    }
                    self.expect_sym(")")?;
                    return Ok(Expr::Over(Box::new(call), order, over));
                }
                if self.eat_sym("[") {
                    // A row by where it is (`running[-1]`), or a range of them
                    // (`total[FIRST:THIS]`); anything else is a key (`fees['card']`).
                    if let Some(a) = self.place() {
                        if self.eat_sym(":") {
                            let Some(b) = self.place() else {
                                return Err(self.fail(format!(
                                    "a range ends at FIRST, THIS, LAST or a row by where it is (-1, +2), not {}",
                                    self.shown()
                                )));
                            };
                            self.expect_sym("]")?;
                            let end = self.last();
                            return Ok(Expr::Range(first, a, b, (named.0, end.0 + end.1 - named.0)));
                        }
                        self.expect_sym("]")?;
                        let mut rest = Vec::new();
                        while self.eat_sym(".") {
                            rest.push(self.name()?);
                        }
                        let end = self.last();
                        return Ok(Expr::At(first, a, rest, (named.0, end.0 + end.1 - named.0)));
                    }
                    let key = self.or()?;
                    self.expect_sym("]")?;
                    let mut rest = Vec::new();
                    while self.eat_sym(".") {
                        rest.push(self.name()?);
                    }
                    if rest.is_empty() {
                        let says = shown(&first);
                        return Err(Fault::at(named, format!("{says}[…] names a row: a column follows, {says}[…].column")));
                    }
                    return Ok(Expr::Lookup(first, Box::new(key), rest, named));
                }
                let mut path = Vec::from([first]);
                while self.eat_sym(".") {
                    path.push(self.name()?);
                }
                let end = self.last();
                Ok(Expr::Path(path, (named.0, end.0 + end.1 - named.0)))
            }
            _ => Err(self.fail(format!("{} cannot stand there", self.shown()))),
        }
    }
}

/// A formula's text, parsed; a fault says where in it the text breaks.
pub fn parse(src: &str) -> Result<Expr, Fault> {
    let (toks, spans): (Vec<Tok>, Vec<Sp>) = lex(src)?.into_iter().unzip();
    let mut p = Parser { toks, spans, at: 0, len: src.len() };
    if p.toks.is_empty() {
        return Err(Fault::whole("the formula is empty".to_owned()));
    }
    let e = p.or()?;
    if p.peek().is_some() {
        return Err(p.stray(format!("{} is left over", p.shown())));
    }
    Ok(e)
}

/// Every word of `src` written in lower case that spells the language's (`sum`, `case`),
/// where it is and in capitals: what a client writes in capitals, once it knows the word
/// names nothing the file has.
pub fn low_words(src: &str) -> Vec<(Sp, String)> {
    let Ok(toks) = lex(src) else { return Vec::new() };
    toks.into_iter()
        .filter_map(|(t, sp)| match t {
            Tok::Name(n) => {
                let up = n.to_ascii_uppercase();
                (up != n && language(&up)).then_some((sp, up))
            }
            _ => None,
        })
        .collect()
}

/// Every function a formula may call, the aggregates first: what a language server
/// completes.
pub const FUNCTIONS: [&str; 15] =
    ["SUM", "AVG", "MIN", "MAX", "COUNT", "ROUND", "FLOOR", "CEIL", "ABS", "COALESCE", "NULLIF", "LENGTH", "LOWER", "UPPER", "TRIM"];

/// A function's call as written and what it gives, in a line: what a completion and a
/// hover say of it. Every one of `FUNCTIONS` has one.
pub fn signature(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "SUM" => ("SUM(column)", "the sum of the column's values, over every row or a range (SUM(total[FIRST:THIS])), blanks left out"),
        "AVG" => ("AVG(column)", "the mean of the column's values that are not blank"),
        "MIN" => ("MIN(column)", "the column's smallest value"),
        "MAX" => ("MAX(column)", "the column's largest value"),
        "COUNT" => ("COUNT(column)", "how many of the column's values are not blank"),
        "ROUND" => ("ROUND(x, digits)", "x rounded half to even to `digits` places after the point, none unless given"),
        "FLOOR" => ("FLOOR(x)", "the whole number at or below x"),
        "CEIL" => ("CEIL(x)", "the whole number at or above x"),
        "ABS" => ("ABS(x)", "x without its sign"),
        "COALESCE" => ("COALESCE(a, b, …)", "the first of its values that is not blank"),
        "NULLIF" => ("NULLIF(a, b)", "blank where a equals b, else a"),
        "LENGTH" => ("LENGTH(text)", "how many characters the text has"),
        "LOWER" => ("LOWER(text)", "the text in lower case"),
        "UPPER" => ("UPPER(text)", "the text in upper case"),
        "TRIM" => ("TRIM(text)", "the text without the spaces and tabs at either end"),
        _ => return None,
    })
}

/// A name a formula reads, each part where it is written: `cost`; `to.kind`, a column
/// and what its reference leads to; `fees['card'].pct`, a table (`table`) and a column of
/// the row its key finds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub parts: Vec<(String, Sp)>,
    pub table: bool,
    /// A table's row found by a key written as text, `'card'`: the key, and where it is
    /// written, its quotes included.
    pub key: Option<(String, Sp)>,
}

/// Every name a formula reads, in the order written: what a language server finds,
/// follows and renames. A fault where the formula does not parse.
pub fn names(src: &str) -> Result<Vec<Named>, Fault> {
    let e = parse(src)?;
    let toks = lex(src)?;
    // The name tokens from the one at `from`, each after a `.`, `n` of them.
    let parts = |mut k: usize, n: usize, first: bool| {
        let mut out = Vec::with_capacity(n);
        while out.len() < n {
            if !(first && out.is_empty()) {
                assert_eq!(toks[k].0, Tok::Sym("."), "a name's parts are joined by .");
                k += 1;
            }
            let name = match &toks[k].0 {
                Tok::Name(s) | Tok::Quoted(s) => s.clone(),
                other => panic!("a name's part is a name, not {other:?}"),
            };
            out.push((name, toks[k].1));
            k += 1;
        }
        out
    };
    let at = |s: Sp| toks.iter().position(|t| t.1.0 == s.0).expect("a name where a token starts");
    let mut out = Vec::new();
    let mut stack = Vec::from([&e]);
    while let Some(e) = stack.pop() {
        match e {
            Expr::Path(segs, s) => out.push(Named { parts: parts(at(*s), segs.len(), true), table: false, key: None }),
            Expr::Lookup(name, key, rest, s) => {
                // The table, then past its `[…]`, the columns.
                let mut k = at(*s) + 1;
                let literal = match (&**key, &toks[k + 1]) {
                    (Expr::Text(t), (Tok::Str(_), sp)) if toks.get(k + 2).is_some_and(|t| t.0 == Tok::Sym("]")) => Some((t.clone(), *sp)),
                    _ => None,
                };
                let mut depth = 0;
                loop {
                    match toks[k].0 {
                        Tok::Sym("[") => depth += 1,
                        Tok::Sym("]") => depth -= 1,
                        _ => {}
                    }
                    k += 1;
                    if depth == 0 {
                        break;
                    }
                }
                let mut all = Vec::from([(name.clone(), *s)]);
                all.extend(parts(k, rest.len(), false));
                out.push(Named { parts: all, table: true, key: literal });
                stack.push(key);
            }
            Expr::Neg(a) | Expr::Not(a) | Expr::IsNull(a, _) => stack.push(a),
            Expr::Bin(_, a, b, _) => stack.extend([&**b, &**a]),
            Expr::Case(arms, other) => {
                stack.extend(other.as_deref());
                for (w, t) in arms.iter().rev() {
                    stack.extend([t, w]);
                }
            }
            Expr::Call(_, args, _) => stack.extend(args.iter().rev()),
            Expr::Over(call, order, _) => {
                stack.extend(order.iter().rev().map(|(k, _)| k));
                stack.push(call);
            }
            // The column, then past its `[…]`, the names its references lead to.
            Expr::At(name, _, rest, s) => {
                let mut k = at(*s);
                while toks[k].0 != Tok::Sym("]") {
                    k += 1;
                }
                let mut all = Vec::from([(name.clone(), toks[at(*s)].1)]);
                all.extend(parts(k + 1, rest.len(), false));
                out.push(Named { parts: all, table: false, key: None });
            }
            Expr::Range(name, _, _, s) => out.push(Named { parts: Vec::from([(name.clone(), toks[at(*s)].1)]), table: false, key: None }),
            Expr::Num(_) | Expr::Text(_) | Expr::Bool(_) | Expr::Null => {}
        }
    }
    out.sort_by_key(|n| n.parts[0].1.0);
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agg {
    Sum,
    Avg,
    Min,
    Max,
    Count,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum F {
    Round,
    Abs,
    Floor,
    Ceil,
    Coalesce,
    Nullif,
    Lower,
    Upper,
    Trim,
    Length,
}

/// Where a column read is: the row being computed's table (`None`), or a table by its
/// index in the scope.
pub type Table = Option<usize>;

/// The file's own table, as a parsed formula holds the language's `TABLE`: never a name a
/// file can give, since a header's names hold no control character, so a column named
/// `"TABLE"`, quoted, is never it.
pub const THIS: &str = "\u{1}TABLE";

/// A name as a message shows it: the file's own table as the formula writes it.
pub fn shown(name: &str) -> &str {
    if name == THIS { "TABLE" } else { name }
}

/// A formula resolved: every name a place, every kind checked.
#[derive(Clone, Debug, PartialEq)]
pub enum Ir {
    Const(V),
    /// A column of the row in context.
    Col(usize),
    /// A column of the file's own table on a row by where it is from the row in context;
    /// past the rows, 0 for a number (`zero`), else null.
    At(usize, Place, bool),
    /// An aggregate of a column of the file's own table over the rows from one place to
    /// another, clipped by the rows; its slot keeps the running sums a range reads.
    Span(Agg, usize, Place, Place, usize),
    /// A key, looked up in each target in turn (its table, the key column, the column
    /// read on the row found).
    Follow(Box<Ir>, Vec<(Table, usize, usize)>),
    Agg(Agg, Table, Box<Ir>, usize),
    /// A window over the file's own rows in an order: what it computes, of what, the
    /// order's keys (each descending or not), its slot.
    Window(Win, Box<Ir>, Vec<(Ir, bool)>, usize),
    Neg(Box<Ir>),
    Not(Box<Ir>),
    Bin(Bin, Box<Ir>, Box<Ir>),
    IsNull(Box<Ir>, bool),
    Case(Vec<(Ir, Ir)>, Box<Ir>),
    Call(F, Vec<Ir>),
}

/// What a formula may read: columns of its own table, other tables by name, and the
/// targets of a reference column.
pub trait Scope {
    /// A column of `table` by its name: its index and kind.
    fn column(&self, table: Table, name: &str) -> Option<(usize, K)>;
    /// A table by its name.
    fn table(&self, name: &str) -> Option<usize>;
    /// The tables a reference column names, each with its key column: none when the
    /// column is no reference.
    fn targets(&self, table: Table, column: usize) -> Vec<(usize, usize)>;
    /// A table's key column.
    fn key(&self, table: Table) -> Option<usize>;
}

/// Resolves a parsed formula; `aggregates` counts the aggregates resolved so far, each one
/// a slot its value is remembered in.
pub fn resolve(e: &Expr, scope: &dyn Scope, aggregates: &mut usize) -> Result<(Ir, K), Fault> {
    Resolver { scope, aggregates, windows: false }.at(e, None, false)
}

/// `resolve` for a column's formula, which may hold a window: one over the column's rows
/// in order.
pub fn resolve_column(e: &Expr, scope: &dyn Scope, aggregates: &mut usize) -> Result<(Ir, K), Fault> {
    Resolver { scope, aggregates, windows: true }.at(e, None, false)
}

/// What a window computes at each row: an aggregate of the rows up to it, or the value
/// on the row before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Win {
    Agg(Agg),
    Lag,
}

struct Resolver<'a, 'b> {
    scope: &'a dyn Scope,
    aggregates: &'b mut usize,
    /// A column's formula, where a window may stand.
    windows: bool,
}

impl Resolver<'_, '_> {
    /// `e` read on a row of `table`; `inside` when within an aggregate.
    fn at(&mut self, e: &Expr, table: Table, inside: bool) -> Result<(Ir, K), Fault> {
        Ok(match e {
            Expr::Num(n) => (Ir::Const(V::Num(n.clone())), K::Num),
            Expr::Text(t) => (Ir::Const(V::Text(t.clone())), K::Text),
            Expr::Bool(b) => (Ir::Const(V::Bool(*b)), K::Bool),
            Expr::Null => (Ir::Const(V::Null), K::Null),
            Expr::Path(path, at) => self.path(path, *at, table)?,
            Expr::Lookup(name, key, rest, at) => {
                let fail = |why: String| Fault::at(*at, why);
                let says = shown(name);
                if self.scope.column(table, name).is_some() && self.named(name).is_some() {
                    return Err(fail(format!("{says} names a column and a table, so it means neither")));
                }
                let t = self.named(name).ok_or_else(|| fail(format!("no table \"{says}\"")))?;
                let k = self.scope.key(t).ok_or_else(|| fail(format!("{says} has no key, so no row is found by one")))?;
                let (key, _) = self.at(key, table, inside)?;
                let (col, kind) = self.scope.column(t, &rest[0]).ok_or_else(|| fail(format!("{says} has no column \"{}\"", rest[0])))?;
                let ir = Ir::Follow(Box::new(key), Vec::from([(t, k, col)]));
                self.follow(ir, t, col, kind, &rest[1..], &rest[0], *at)?
            }
            Expr::Neg(a) => {
                let (a, k) = self.at(a, table, inside)?;
                number(k, "-").map_err(Fault::whole)?;
                (Ir::Neg(Box::new(a)), K::Num)
            }
            Expr::Not(a) => {
                let (a, k) = self.at(a, table, inside)?;
                boolean(k, "not").map_err(Fault::whole)?;
                (Ir::Not(Box::new(a)), K::Bool)
            }
            Expr::IsNull(a, negated) => {
                let (a, _) = self.at(a, table, inside)?;
                (Ir::IsNull(Box::new(a), *negated), K::Bool)
            }
            Expr::Bin(op, a, b, at) => {
                let (a, ka) = self.at(a, table, inside)?;
                let (b, kb) = self.at(b, table, inside)?;
                let fail = |why: String| Fault::at(*at, why);
                let k = match op {
                    Bin::Add | Bin::Sub | Bin::Mul | Bin::Div => {
                        let s = match op {
                            Bin::Add => "+",
                            Bin::Sub => "-",
                            Bin::Mul => "*",
                            _ => "/",
                        };
                        number(ka, s).map_err(fail)?;
                        number(kb, s).map_err(fail)?;
                        K::Num
                    }
                    Bin::Concat => K::Text,
                    Bin::And | Bin::Or => {
                        let s = if *op == Bin::And { "and" } else { "or" };
                        boolean(ka, s).map_err(fail)?;
                        boolean(kb, s).map_err(fail)?;
                        K::Bool
                    }
                    _ => {
                        ka.meet(kb).ok_or_else(|| fail(format!("{} and {} are not compared", ka.word(), kb.word())))?;
                        K::Bool
                    }
                };
                (Ir::Bin(*op, Box::new(a), Box::new(b)), k)
            }
            Expr::Case(arms, otherwise) => {
                let mut out = Vec::with_capacity(arms.len());
                let mut kind = K::Null;
                for (w, t) in arms {
                    let (w, kw) = self.at(w, table, inside)?;
                    boolean(kw, "when").map_err(Fault::whole)?;
                    let (t, kt) = self.at(t, table, inside)?;
                    kind = kind.meet(kt).ok_or_else(|| Fault::whole(format!("a case gives {} and {}", kind.word(), kt.word())))?;
                    out.push((w, t));
                }
                let otherwise = match otherwise {
                    Some(o) => {
                        let (o, ko) = self.at(o, table, inside)?;
                        kind = kind.meet(ko).ok_or_else(|| Fault::whole(format!("a case gives {} and {}", kind.word(), ko.word())))?;
                        o
                    }
                    None => Ir::Const(V::Null),
                };
                (Ir::Case(out, Box::new(otherwise)), kind)
            }
            Expr::Call(name, args, at) => self.call(name, *at, args, table, inside)?,
            Expr::Over(call, order, at) => self.window(call, order, *at, table, inside)?,
            Expr::At(name, place, rest, at) => {
                let (col, kind) = self.placed(name, table, *at)?;
                let ir = Ir::At(col, *place, kind == K::Num);
                self.follow(ir, None, col, kind, rest, name, *at)?
            }
            Expr::Range(name, _, _, at) => {
                return Err(Fault::at(*at, format!("{name}[…:…] is rows of a column, an aggregate's: SUM({name}[FIRST:THIS])")));
            }
        })
    }

    /// The column a row by where it is reads: one of the file's own, on a row of it.
    fn placed(&self, name: &str, table: Table, at: Sp) -> Result<(usize, K), Fault> {
        if table.is_some() {
            return Err(Fault::at(at, format!("{name}[…] is a row by where it is, and only the file's own rows are read so")));
        }
        if self.named(name).is_some() {
            let says = shown(name);
            return Err(Fault::at(
                at,
                format!("{says} is a table: a row by where it is reads a column ({says}['key'] finds a row by its key)"),
            ));
        }
        self.scope.column(None, name).ok_or_else(|| Fault::at(at, format!("no column \"{name}\"")))
    }

    fn window(&mut self, call: &Expr, order: &[(Expr, bool)], at: Sp, table: Table, inside: bool) -> Result<(Ir, K), Fault> {
        let fail = |why: String| Fault::at(at, why);
        let Expr::Call(name, args, named) = call else { unreachable!("over follows a call") };
        if !self.windows {
            return Err(fail("a window is a column's formula's: OVER (ORDER BY …) reads the column's rows in order".to_owned()));
        }
        if inside || table.is_some() {
            return Err(fail("a window inside an aggregate or another table".to_owned()));
        }
        let win = match name.as_str() {
            "SUM" => Win::Agg(Agg::Sum),
            "AVG" => Win::Agg(Agg::Avg),
            "MIN" => Win::Agg(Agg::Min),
            "MAX" => Win::Agg(Agg::Max),
            "COUNT" => Win::Agg(Agg::Count),
            "LAG" => Win::Lag,
            _ => {
                let why = spelled_low(name).unwrap_or_else(|| format!("{name} is no window: SUM, AVG, MIN, MAX, COUNT, LAG"));
                return Err(Fault::at(*named, why));
            }
        };
        if args.len() != 1 {
            return Err(Fault::at(*named, format!("{name} takes one value")));
        }
        let (arg, k) = self.at(&args[0], None, true)?;
        let kind = match win {
            Win::Agg(Agg::Sum | Agg::Avg) => {
                number(k, name).map_err(|why| Fault::at(*named, why))?;
                K::Num
            }
            Win::Agg(Agg::Count) => K::Num,
            Win::Agg(Agg::Min | Agg::Max) | Win::Lag => k,
        };
        let mut keys = Vec::with_capacity(order.len());
        for (key, descending) in order {
            let (ir, _) = self.at(key, None, true)?;
            keys.push((ir, *descending));
        }
        let slot = *self.aggregates;
        *self.aggregates += 1;
        Ok((Ir::Window(win, Box::new(arg), keys, slot), kind))
    }

    /// A table by the name a formula reads it by: `TABLE`, the file's own, or one the
    /// prelude names.
    fn named(&self, name: &str) -> Option<Table> {
        if name == THIS { Some(None) } else { self.scope.table(name).map(Some) }
    }

    fn path(&mut self, path: &[String], at: Sp, table: Table) -> Result<(Ir, K), Fault> {
        let first = &path[0];
        let says = shown(first);
        let column = self.scope.column(table, first);
        let named = self.named(first);
        if column.is_some() && named.is_some() {
            return Err(Fault::at(at, format!("{says} names a column and a table, so it means neither")));
        }
        if let Some((col, kind)) = column {
            return self.follow(Ir::Col(col), table, col, kind, &path[1..], first, at);
        }
        // Inside an aggregate over the table: `SUM(fees.pct)`, `SUM(TABLE.amount)`.
        if let Some(t) = named
            && t == table
            && let Some(name) = path.get(1)
        {
            let (col, kind) = self.scope.column(table, name).ok_or_else(|| Fault::at(at, format!("{says} has no column \"{name}\"")))?;
            return self.follow(Ir::Col(col), table, col, kind, &path[2..], name, at);
        }
        if named.is_some() {
            return Err(Fault::at(
                at,
                match path.get(1) {
                    Some(c) => format!("{says}.{c} is a whole column: read one row ({says}[key].{c}) or all of them (SUM({says}.{c}))"),
                    None => format!("{says} is a table, not a value"),
                },
            ));
        }
        Err(Fault::at(at, spelled_low(first).unwrap_or_else(|| format!("no column \"{first}\""))))
    }

    /// `ir`, column `col` of `table` read, then each name of `rest` followed through the
    /// reference the column before it is; `at`, where the names are written.
    #[allow(clippy::too_many_arguments)]
    fn follow(
        &mut self,
        mut ir: Ir,
        mut table: Table,
        mut col: usize,
        mut kind: K,
        rest: &[String],
        shown: &str,
        at: Sp,
    ) -> Result<(Ir, K), Fault> {
        let fail = |why: String| Fault::at(at, why);
        let mut shown = shown.to_owned();
        for name in rest {
            let targets = self.scope.targets(table, col);
            if targets.is_empty() {
                return Err(fail(format!("{shown} is no reference, so it has no {name}")));
            }
            let mut steps = Vec::with_capacity(targets.len());
            let mut k = K::Null;
            for (t, key) in targets {
                let (c, kc) = self.scope.column(Some(t), name).ok_or_else(|| fail(format!("{shown} leads to no column \"{name}\"")))?;
                k = k.meet(kc).ok_or_else(|| fail(format!("{name} is {} in one table and {} in another", k.word(), kc.word())))?;
                steps.push((Some(t), key, c));
            }
            table = steps[0].0;
            col = steps[0].2;
            kind = k;
            ir = Ir::Follow(Box::new(ir), steps);
            shown = name.clone();
        }
        Ok((ir, kind))
    }

    fn call(&mut self, name: &str, at: Sp, args: &[Expr], table: Table, inside: bool) -> Result<(Ir, K), Fault> {
        let fail = |why: String| Fault::at(at, why);
        if let Some(why) = spelled_low(name) {
            return Err(fail(why));
        }
        let agg = match name {
            "SUM" => Some(Agg::Sum),
            "AVG" => Some(Agg::Avg),
            "MIN" => Some(Agg::Min),
            "MAX" => Some(Agg::Max),
            "COUNT" => Some(Agg::Count),
            _ => None,
        };
        if let Some(agg) = agg {
            if inside {
                return Err(fail(format!("{name} inside an aggregate")));
            }
            if args.len() != 1 {
                return Err(fail(format!("{name} takes one value")));
            }
            // A range of a column's rows, `SUM(total[FIRST:THIS])`.
            if let Expr::Range(col, a, b, span) = &args[0] {
                let (c, k) = self.placed(col, table, *span)?;
                let kind = match agg {
                    Agg::Sum | Agg::Avg => {
                        number(k, name).map_err(fail)?;
                        K::Num
                    }
                    Agg::Count => K::Num,
                    Agg::Min | Agg::Max => k,
                };
                let slot = *self.aggregates;
                *self.aggregates += 1;
                return Ok((Ir::Span(agg, c, *a, *b, slot), kind));
            }
            let over = over(&args[0], self.scope);
            let (arg, k) = self.at(&args[0], over, true)?;
            let kind = match agg {
                Agg::Sum | Agg::Avg => {
                    number(k, name).map_err(fail)?;
                    K::Num
                }
                Agg::Count => K::Num,
                Agg::Min | Agg::Max => k,
            };
            let slot = *self.aggregates;
            *self.aggregates += 1;
            return Ok((Ir::Agg(agg, over, Box::new(arg), slot), kind));
        }
        let mut out = Vec::with_capacity(args.len());
        let mut kinds = Vec::with_capacity(args.len());
        for a in args {
            let (ir, k) = self.at(a, table, inside)?;
            out.push(ir);
            kinds.push(k);
        }
        let arity = |n: core::ops::RangeInclusive<usize>| {
            if n.contains(&args.len()) { Ok(()) } else { Err(fail(format!("{name} takes {} values, not {}", n.end(), args.len()))) }
        };
        let (f, k) = match name {
            "ROUND" => {
                arity(1..=2)?;
                kinds.iter().try_for_each(|k| number(*k, name)).map_err(fail)?;
                (F::Round, K::Num)
            }
            "ABS" | "FLOOR" | "CEIL" => {
                arity(1..=1)?;
                number(kinds[0], name).map_err(fail)?;
                (
                    match name {
                        "ABS" => F::Abs,
                        "FLOOR" => F::Floor,
                        _ => F::Ceil,
                    },
                    K::Num,
                )
            }
            "COALESCE" => {
                if args.is_empty() {
                    return Err(fail("COALESCE takes values".to_owned()));
                }
                let mut k = K::Null;
                for x in &kinds {
                    k = k.meet(*x).ok_or_else(|| fail(format!("COALESCE of {} and {}", k.word(), x.word())))?;
                }
                (F::Coalesce, k)
            }
            "NULLIF" => {
                arity(2..=2)?;
                kinds[0].meet(kinds[1]).ok_or_else(|| fail(format!("{} and {} are not compared", kinds[0].word(), kinds[1].word())))?;
                (F::Nullif, kinds[0])
            }
            "LOWER" | "UPPER" | "TRIM" | "LENGTH" => {
                arity(1..=1)?;
                if !matches!(kinds[0], K::Text | K::Null) {
                    return Err(fail(format!("{name} takes text, not {}", kinds[0].word())));
                }
                match name {
                    "LOWER" => (F::Lower, K::Text),
                    "UPPER" => (F::Upper, K::Text),
                    "TRIM" => (F::Trim, K::Text),
                    _ => (F::Length, K::Num),
                }
            }
            "LAG" => return Err(fail("LAG is a window's: LAG(balance) OVER (ORDER BY date)".to_owned())),
            _ => return Err(fail(format!("no function {name}"))),
        };
        Ok((Ir::Call(f, out), k))
    }
}

/// The table an aggregate runs over: the one its first `table.column` names, else the
/// row's own.
fn over(e: &Expr, scope: &dyn Scope) -> Table {
    fn find(e: &Expr, scope: &dyn Scope) -> Option<usize> {
        match e {
            Expr::Path(p, _) if p.len() > 1 && scope.column(None, &p[0]).is_none() => scope.table(&p[0]),
            Expr::Neg(a) | Expr::Not(a) | Expr::IsNull(a, _) => find(a, scope),
            Expr::Bin(_, a, b, _) => find(a, scope).or_else(|| find(b, scope)),
            Expr::Case(arms, o) => arms
                .iter()
                .find_map(|(w, t)| find(w, scope).or_else(|| find(t, scope)))
                .or_else(|| o.as_deref().and_then(|o| find(o, scope))),
            Expr::Call(_, args, _) => args.iter().find_map(|a| find(a, scope)),
            _ => None,
        }
    }
    find(e, scope)
}

fn number(k: K, op: &str) -> Result<(), String> {
    if matches!(k, K::Num | K::Null) { Ok(()) } else { Err(format!("{op} takes numbers, not {}", k.word())) }
}

fn boolean(k: K, op: &str) -> Result<(), String> {
    if matches!(k, K::Bool | K::Null) { Ok(()) } else { Err(format!("{op} takes a boolean, not {}", k.word())) }
}

/// What an evaluation reads.
pub trait Source {
    /// A cell of a row of `table`: of the table being computed, its value or what its
    /// formula gives.
    fn cell(&mut self, table: Table, row: usize, column: usize) -> V;
    fn rows(&self, table: Table) -> usize;
    /// The row of `table` whose key column `key` holds `value`.
    fn find(&mut self, table: Table, key: usize, value: &V) -> Option<usize>;
    /// An aggregate's value, when it was evaluated already.
    fn recall(&mut self, slot: usize) -> Option<V>;
    fn remember(&mut self, slot: usize, v: V);
    /// A window's value on a row, when it was evaluated already, or that it is being.
    fn window(&mut self, slot: usize, row: usize) -> Windowed;
    /// A window being evaluated, or its values, one a row, and the rows its order does not
    /// tell apart, each with one it ties with.
    fn remember_window(&mut self, slot: usize, values: Option<Evaluated>);
    /// A range's running sums so far: entry `i` the sum and the count of its column's
    /// non-null values on rows before `i`, entry 0 nothing. They grow as rows are read, in
    /// the rows' order, so a range of a column's own rows above reads no row below it.
    fn running(&mut self, slot: usize) -> &mut Vec<(Exact, i64)>;
}

/// A window evaluated: its value on each row, and each row its order does not place,
/// with one it ties with.
pub type Evaluated = (Vec<V>, Vec<(usize, usize)>);

/// Where a window's evaluation is.
#[derive(Clone, Debug, PartialEq)]
pub enum Windowed {
    Unknown,
    Busy,
    Done(V),
}

/// The file's rows in a window's order, each key ascending or descending, a null after
/// every value; and each row the order does not tell from its neighbour, with it.
fn ordered(keys: &[Vec<V>], descending: &[bool]) -> (Vec<usize>, Vec<(usize, usize)>) {
    let cmp = |a: &[V], b: &[V]| {
        for ((x, y), d) in a.iter().zip(b).zip(descending) {
            let o = match (x, y) {
                (V::Null, V::Null) => Ordering::Equal,
                (V::Null, _) => Ordering::Greater,
                (_, V::Null) => Ordering::Less,
                _ => {
                    let o = order(x, y).unwrap_or(Ordering::Equal);
                    if *d { o.reverse() } else { o }
                }
            };
            if o != Ordering::Equal {
                return o;
            }
        }
        Ordering::Equal
    };
    let mut rows: Vec<usize> = (0..keys.len()).collect();
    rows.sort_by(|&a, &b| cmp(&keys[a], &keys[b]));
    let mut ties = Vec::new();
    for w in rows.windows(2) {
        if cmp(&keys[w[0]], &keys[w[1]]) == Ordering::Equal {
            ties.push((w[0], w[1]));
            ties.push((w[1], w[0]));
        }
    }
    (rows, ties)
}

/// `ir` on row `row` of `table`.
pub fn eval(ir: &Ir, table: Table, row: usize, src: &mut dyn Source) -> V {
    match ir {
        Ir::Const(v) => v.clone(),
        Ir::Col(k) => src.cell(table, row, *k),
        Ir::At(k, place, zero) => {
            let n = src.rows(None);
            let r = place.row(row, n);
            if r < 0 || r >= n as i64 {
                // Past the rows: a number reads 0, as an empty cell does in a sheet's sums.
                return if *zero { V::Num(Exact::from_int(0)) } else { V::Null };
            }
            src.cell(None, r as usize, *k)
        }
        Ir::Span(agg, k, a, b, slot) => {
            let n = src.rows(None);
            let lo = a.row(row, n).max(0);
            let hi = b.row(row, n).min(n as i64 - 1);
            if lo > hi {
                return match agg {
                    Agg::Count => V::Num(Exact::from_int(0)),
                    _ => V::Null,
                };
            }
            let (lo, hi) = (lo as usize, hi as usize);
            match agg {
                Agg::Sum | Agg::Avg | Agg::Count => {
                    // The running sums up to the range's end, read row by row in order.
                    loop {
                        let have = src.running(*slot).len();
                        if have > hi + 1 {
                            break;
                        }
                        if have == 0 {
                            src.running(*slot).push((Exact::from_int(0), 0));
                            continue;
                        }
                        let v = src.cell(None, have - 1, *k);
                        let (s, c) = src.running(*slot)[have - 1].clone();
                        let next = match v {
                            V::Num(x) => (s.add(&x), c + 1),
                            V::Null => (s, c),
                            _ => (s, c + 1),
                        };
                        src.running(*slot).push(next);
                    }
                    let (s1, c1) = src.running(*slot)[hi + 1].clone();
                    let (s0, c0) = src.running(*slot)[lo].clone();
                    let (sum, count) = (s1.sub(&s0), c1 - c0);
                    match agg {
                        Agg::Count => V::Num(Exact::from_int(count)),
                        _ if count == 0 => V::Null,
                        Agg::Sum => V::Num(sum),
                        _ => sum.div(&Exact::from_int(count)).map_or(V::Null, V::Num),
                    }
                }
                Agg::Min | Agg::Max => {
                    let mut acc: Option<V> = None;
                    for r in lo..=hi {
                        let v = src.cell(None, r, *k);
                        if v == V::Null {
                            continue;
                        }
                        let want = if *agg == Agg::Min { Ordering::Less } else { Ordering::Greater };
                        acc = Some(match acc {
                            Some(a) if order(&v, &a) != Some(want) => a,
                            _ => v,
                        });
                    }
                    acc.unwrap_or(V::Null)
                }
            }
        }
        Ir::Follow(key, targets) => {
            let k = eval(key, table, row, src);
            if k == V::Null {
                return V::Null;
            }
            for &(t, key_col, col) in targets {
                if let Some(r) = src.find(t, key_col, &k) {
                    return src.cell(t, r, col);
                }
            }
            V::Null
        }
        Ir::Agg(agg, over, arg, slot) => {
            if let Some(v) = src.recall(*slot) {
                return v;
            }
            let n = src.rows(*over);
            let mut acc: Option<V> = None;
            let mut count = 0i64;
            for r in 0..n {
                let v = eval(arg, *over, r, src);
                if v == V::Null {
                    continue;
                }
                count += 1;
                acc = Some(match (agg, acc) {
                    (_, None) => v,
                    (Agg::Sum | Agg::Avg, Some(V::Num(a))) => match v {
                        V::Num(b) => V::Num(a.add(&b)),
                        _ => unreachable!("a sum of numbers"),
                    },
                    (Agg::Min, Some(a)) => {
                        if order(&v, &a) == Some(Ordering::Less) {
                            v
                        } else {
                            a
                        }
                    }
                    (Agg::Max, Some(a)) => {
                        if order(&v, &a) == Some(Ordering::Greater) {
                            v
                        } else {
                            a
                        }
                    }
                    (_, Some(a)) => a,
                });
            }
            let v = match agg {
                Agg::Count => V::Num(Exact::from_int(count)),
                Agg::Sum => acc.unwrap_or(V::Null),
                Agg::Avg => match acc {
                    Some(V::Num(s)) => s.div(&Exact::from_int(count)).map_or(V::Null, V::Num),
                    _ => V::Null,
                },
                Agg::Min | Agg::Max => acc.unwrap_or(V::Null),
            };
            src.remember(*slot, v.clone());
            v
        }
        Ir::Window(win, arg, by, slot) => {
            match src.window(*slot, row) {
                Windowed::Done(v) => return v,
                // The window reads itself: a cycle, as a cell reading itself is.
                Windowed::Busy => return V::Null,
                Windowed::Unknown => {}
            }
            src.remember_window(*slot, None);
            let n = src.rows(None);
            let keys: Vec<Vec<V>> = (0..n).map(|r| by.iter().map(|(k, _)| eval(k, None, r, src)).collect()).collect();
            let descending: Vec<bool> = by.iter().map(|(_, d)| *d).collect();
            let (rows, mut ties) = ordered(&keys, &descending);
            if *win == Win::Lag {
                // The row after rows that tie has no one row before it.
                let tied: Vec<usize> = ties.iter().map(|t| t.0).collect();
                for w in rows.windows(2) {
                    if tied.contains(&w[0]) && !tied.contains(&w[1]) {
                        ties.push((w[1], w[0]));
                    }
                }
            }
            let mut values = alloc::vec![V::Null; n];
            let (mut acc, mut count): (Option<V>, i64) = (None, 0);
            let mut before = V::Null;
            for &r in &rows {
                let v = eval(arg, None, r, src);
                values[r] = match win {
                    Win::Lag => core::mem::replace(&mut before, v),
                    Win::Agg(agg) => {
                        if v != V::Null {
                            count += 1;
                            acc = Some(match (agg, acc.take()) {
                                (_, None) => v,
                                (Agg::Sum | Agg::Avg, Some(V::Num(a))) => match v {
                                    V::Num(b) => V::Num(a.add(&b)),
                                    _ => unreachable!("a sum of numbers"),
                                },
                                (Agg::Min, Some(a)) => {
                                    if order(&v, &a) == Some(Ordering::Less) {
                                        v
                                    } else {
                                        a
                                    }
                                }
                                (Agg::Max, Some(a)) => {
                                    if order(&v, &a) == Some(Ordering::Greater) {
                                        v
                                    } else {
                                        a
                                    }
                                }
                                (_, Some(a)) => a,
                            });
                        }
                        match agg {
                            Agg::Count => V::Num(Exact::from_int(count)),
                            Agg::Avg => match &acc {
                                Some(V::Num(s)) => s.div(&Exact::from_int(count)).map_or(V::Null, V::Num),
                                _ => V::Null,
                            },
                            _ => acc.clone().unwrap_or(V::Null),
                        }
                    }
                };
            }
            // A row the order does not place has no value: which comes first is never
            // decided by where a row is written.
            for &(r, _) in &ties {
                values[r] = V::Null;
            }
            let v = values[row].clone();
            src.remember_window(*slot, Some((values, ties)));
            v
        }
        Ir::Neg(a) => match eval(a, table, row, src) {
            V::Num(n) => V::Num(n.neg()),
            _ => V::Null,
        },
        Ir::Not(a) => match eval(a, table, row, src) {
            V::Bool(b) => V::Bool(!b),
            _ => V::Null,
        },
        Ir::IsNull(a, negated) => V::Bool((eval(a, table, row, src) == V::Null) != *negated),
        Ir::Bin(op, a, b) => {
            // SQL's three values: false and anything is false, true or anything is true.
            if matches!(op, Bin::And | Bin::Or) {
                let x = eval(a, table, row, src);
                let stop = V::Bool(*op == Bin::Or);
                if x == stop {
                    return stop;
                }
                let y = eval(b, table, row, src);
                return match (x, y) {
                    (_, y) if y == stop => stop,
                    (V::Bool(_), V::Bool(y)) => V::Bool(y),
                    _ => V::Null,
                };
            }
            let x = eval(a, table, row, src);
            let y = eval(b, table, row, src);
            binary(*op, x, y)
        }
        Ir::Case(arms, otherwise) => {
            for (w, t) in arms {
                if eval(w, table, row, src) == V::Bool(true) {
                    return eval(t, table, row, src);
                }
            }
            eval(otherwise, table, row, src)
        }
        Ir::Call(f, args) => {
            if *f == F::Coalesce {
                for a in args {
                    let v = eval(a, table, row, src);
                    if v != V::Null {
                        return v;
                    }
                }
                return V::Null;
            }
            let vs: Vec<V> = args.iter().map(|a| eval(a, table, row, src)).collect();
            call(*f, vs)
        }
    }
}

/// Text as `||` joins it.
fn joined(v: &V) -> Option<String> {
    v.canonical()
}

fn order(a: &V, b: &V) -> Option<Ordering> {
    Some(match (a, b) {
        (V::Num(x), V::Num(y)) => x.order(y),
        (V::Text(x), V::Text(y)) => x.cmp(y),
        (V::Bool(x), V::Bool(y)) => x.cmp(y),
        (V::Date(x), V::Date(y)) => x.cmp(y),
        (V::Date(_), V::Text(_)) | (V::Text(_), V::Date(_)) => a.canonical()?.cmp(&b.canonical()?),
        _ => return None,
    })
}

fn binary(op: Bin, x: V, y: V) -> V {
    if op == Bin::Concat {
        return match (joined(&x), joined(&y)) {
            (Some(a), Some(b)) => V::Text(a + &b),
            _ => V::Null,
        };
    }
    if x == V::Null || y == V::Null {
        return V::Null;
    }
    match op {
        Bin::Add | Bin::Sub | Bin::Mul | Bin::Div => {
            let (V::Num(a), V::Num(b)) = (x, y) else { unreachable!("arithmetic on numbers, checked when resolved") };
            match op {
                Bin::Add => V::Num(a.add(&b)),
                Bin::Sub => V::Num(a.sub(&b)),
                Bin::Mul => V::Num(a.mul(&b)),
                _ => a.div(&b).map_or(V::Null, V::Num),
            }
        }
        _ => {
            let Some(o) = order(&x, &y) else { return V::Null };
            V::Bool(match op {
                Bin::Eq => o == Ordering::Equal,
                Bin::Ne => o != Ordering::Equal,
                Bin::Lt => o == Ordering::Less,
                Bin::Le => o != Ordering::Greater,
                Bin::Gt => o == Ordering::Greater,
                _ => o != Ordering::Less,
            })
        }
    }
}

fn call(f: F, vs: Vec<V>) -> V {
    if vs.contains(&V::Null) && f != F::Nullif {
        return V::Null;
    }
    let num = |v: &V| match v {
        V::Num(n) => n.clone(),
        _ => unreachable!("a number, checked when resolved"),
    };
    let text = |v: &V| match v {
        V::Text(t) => t.clone(),
        other => other.canonical().unwrap(),
    };
    match f {
        F::Round => {
            let digits = match vs.get(1) {
                Some(d) => {
                    let d = num(d);
                    // A scale past 38 digits rounds nothing a column can hold.
                    match d.trunc().to_i128(0) {
                        Some(n) if (0..=38).contains(&n) => n as u32,
                        _ => return V::Null,
                    }
                }
                None => 0,
            };
            let mut r = num(&vs[0]).round(digits);
            r.normalize();
            V::Num(r)
        }
        F::Abs => V::Num(num(&vs[0]).abs()),
        F::Floor => V::Num(num(&vs[0]).floor()),
        F::Ceil => V::Num(num(&vs[0]).neg().floor().neg()),
        F::Nullif => {
            if vs[0] != V::Null && order(&vs[0], &vs[1]) == Some(Ordering::Equal) {
                V::Null
            } else {
                vs[0].clone()
            }
        }
        F::Lower => V::Text(text(&vs[0]).to_lowercase()),
        F::Upper => V::Text(text(&vs[0]).to_uppercase()),
        F::Trim => V::Text(text(&vs[0]).trim_matches([' ', '\t']).to_owned()),
        F::Length => V::Num(Exact::from_int(text(&vs[0]).chars().count() as i64)),
        F::Coalesce => unreachable!("coalesce stops at its first value"),
    }
}

/// Every column of the row's own table an `Ir` reads, so the order formulas are evaluated
/// in is known, and a column that reads itself is found before it runs.
pub fn reads(ir: &Ir, table: Table, out: &mut Vec<usize>) {
    match ir {
        Ir::Const(_) => {}
        Ir::Col(k) => {
            if table.is_none() {
                out.push(*k)
            }
        }
        // Another row of a column is no read of the row's own cell: `running[-1]` in
        // `running`'s formula reads the line above. Only a place that is this row is.
        Ir::At(k, place, _) => {
            if matches!(place, Place::This | Place::By(0)) {
                out.push(*k)
            }
        }
        Ir::Span(_, k, a, b, _) => {
            if holds_this(*a, *b) {
                out.push(*k)
            }
        }
        Ir::Follow(key, _) => reads(key, table, out),
        Ir::Agg(_, over, arg, _) => reads(arg, *over, out),
        Ir::Window(_, arg, order, _) => {
            reads(arg, None, out);
            order.iter().for_each(|(k, _)| reads(k, None, out));
        }
        Ir::Neg(a) | Ir::Not(a) | Ir::IsNull(a, _) => reads(a, table, out),
        Ir::Bin(_, a, b) => {
            reads(a, table, out);
            reads(b, table, out);
        }
        Ir::Case(arms, o) => {
            for (w, t) in arms {
                reads(w, table, out);
                reads(t, table, out);
            }
            reads(o, table, out);
        }
        Ir::Call(_, args) => args.iter().for_each(|a| reads(a, table, out)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct One;

    impl Scope for One {
        fn column(&self, table: Table, name: &str) -> Option<(usize, K)> {
            match (table, name) {
                (None, "cost") => Some((0, K::Num)),
                (None, "qty") => Some((1, K::Num)),
                (None, "item") => Some((2, K::Text)),
                _ => None,
            }
        }
        fn table(&self, _: &str) -> Option<usize> {
            None
        }
        fn targets(&self, _: Table, _: usize) -> Vec<(usize, usize)> {
            Vec::new()
        }
        fn key(&self, _: Table) -> Option<usize> {
            None
        }
    }

    struct Rows(Vec<[V; 3]>, Vec<Option<V>>, Vec<Option<Option<Vec<V>>>>, Vec<Vec<(Exact, i64)>>);

    impl Source for Rows {
        fn cell(&mut self, _: Table, row: usize, column: usize) -> V {
            self.0[row][column].clone()
        }
        fn rows(&self, _: Table) -> usize {
            self.0.len()
        }
        fn find(&mut self, _: Table, _: usize, _: &V) -> Option<usize> {
            None
        }
        fn recall(&mut self, slot: usize) -> Option<V> {
            self.1.get(slot).cloned().flatten()
        }
        fn remember(&mut self, slot: usize, v: V) {
            self.1.resize(slot + 1, None);
            self.1[slot] = Some(v);
        }
        fn window(&mut self, slot: usize, row: usize) -> Windowed {
            match self.2.get(slot) {
                Some(Some(Some(v))) => Windowed::Done(v[row].clone()),
                Some(Some(None)) => Windowed::Busy,
                _ => Windowed::Unknown,
            }
        }
        fn remember_window(&mut self, slot: usize, values: Option<Evaluated>) {
            self.2.resize(slot + 1, None);
            self.2[slot] = Some(values.map(|(v, _)| v));
        }
        fn running(&mut self, slot: usize) -> &mut Vec<(Exact, i64)> {
            if self.3.len() <= slot {
                self.3.resize(slot + 1, Vec::new());
            }
            &mut self.3[slot]
        }
    }

    fn run(src: &str) -> String {
        let e = parse(src).unwrap_or_else(|e| panic!("{src}: {}", e.why));
        let mut n = 0;
        let (ir, _) = resolve(&e, &One, &mut n).unwrap_or_else(|e| panic!("{src}: {}", e.why));
        let mut rows = Rows(
            Vec::from([
                [V::Num(Exact::parse("84.20")), V::Num(Exact::parse("2")), V::Text("fuel".into())],
                [V::Num(Exact::parse("10")), V::Null, V::Text("maps".into())],
            ]),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        match eval(&ir, None, 0, &mut rows) {
            V::Num(n) => n.to_string(),
            V::Text(t) => format!("'{t}'"),
            V::Bool(b) => b.to_string(),
            V::Date(d) => crate::value::format_date(d),
            V::Null => "null".into(),
        }
    }

    #[test]
    fn sql() {
        assert_eq!(run("cost * qty"), "168.40");
        assert_eq!(run("ROUND(cost / 3, 2)"), "28.07");
        assert_eq!(run("CASE WHEN qty > 1 THEN 'many' ELSE 'one' END"), "'many'");
        assert_eq!(run("item || ' x' || qty"), "'fuel x2'");
        assert_eq!(run("SUM(cost)"), "94.20");
        assert_eq!(run("COUNT(qty)"), "1");
        assert_eq!(run("COALESCE(NULL, 3)"), "3");
        assert_eq!(run("UPPER(item)"), "'FUEL'");
        assert_eq!(run("1 / 0"), "null");
        assert_eq!(run("NOT (qty = 2) OR FALSE"), "false");
        assert_eq!(run("NULL AND FALSE"), "false");
        assert_eq!(run("qty IS NOT NULL"), "true");
        assert_eq!(run("-cost + 0.20"), "-84.00");
        assert_eq!(run("FLOOR(-2.5)"), "-3");
        assert_eq!(run("CEIL(2.1)"), "3");
        for bad in ["cost +", "cost * item", "item AND TRUE", "nope", "SUM(SUM(cost))", "CASE WHEN 1 THEN 2 END", "'open"] {
            let refused = parse(bad).and_then(|e| resolve(&e, &One, &mut 0).map(|_| ()));
            assert!(refused.is_err(), "{bad} should not resolve");
        }
    }

    #[test]
    fn the_language_is_written_in_capitals() {
        let why = |src: &str| parse(src).and_then(|e| resolve(&e, &One, &mut 0).map(|_| ())).unwrap_err().why;
        assert_eq!(why("sum(cost)"), "\"sum\" is a name: the language is written in capitals, SUM");
        assert_eq!(why("cost is null"), "\"is\" is a name: the language is written in capitals, IS");
        assert_eq!(why("case"), "\"case\" is a name: the language is written in capitals, CASE");
        assert!(why("VAT * 2").contains("written quoted, \"VAT\""), "{}", why("VAT * 2"));
        assert!(why("FIRST").contains("cannot stand there"));
    }

    #[test]
    fn rows_by_where_they_are() {
        let column = |src: &str| -> Vec<V> {
            let (ir, _) = parse(src).and_then(|e| resolve_column(&e, &One, &mut 0)).unwrap_or_else(|f| panic!("{src}: {}", f.why));
            let num = |s: &str| V::Num(Exact::parse(s));
            let mut r = Rows(
                Vec::from([
                    [num("10"), num("1"), V::Text("a".into())],
                    [num("20"), V::Null, V::Text("b".into())],
                    [num("30"), num("3"), V::Text("c".into())],
                ]),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
            (0..3).map(|row| eval(&ir, None, row, &mut r)).collect()
        };
        let num = |s: &str| V::Num(Exact::parse(s));
        assert_eq!(column("cost[-1]"), [num("0"), num("10"), num("20")]);
        assert_eq!(column("item[+1]"), [V::Text("b".into()), V::Text("c".into()), V::Null]);
        assert_eq!(column("cost[FIRST] + cost[LAST]"), [num("40"), num("40"), num("40")]);
        assert_eq!(column("SUM(cost[FIRST:THIS])"), [num("10"), num("30"), num("60")]);
        assert_eq!(column("SUM(cost[-1:+1])"), [num("30"), num("60"), num("50")]);
        assert_eq!(column("COUNT(qty[FIRST:THIS])"), [num("1"), num("1"), num("2")]);
        assert_eq!(column("MAX(cost[THIS:LAST])"), [num("30"), num("30"), num("30")]);
        assert_eq!(column("SUM(cost[FIRST:-1])"), [V::Null, num("10"), num("30")]);
        for bad in ["cost[FIRST:THIS]", "ROUND(cost[FIRST:THIS])", "nope[-1]", "SUM(nope[FIRST:THIS])", "cost[FIRST:+x]"] {
            assert!(parse(bad).and_then(|e| resolve_column(&e, &One, &mut 0).map(|_| ())).is_err(), "{bad} should not resolve");
        }
        // A range holding the row it is read on reads that row's cell; one of rows above
        // does not.
        let reads_of = |src: &str| {
            let (ir, _) = parse(src).and_then(|e| resolve_column(&e, &One, &mut 0)).unwrap();
            let mut out = Vec::new();
            reads(&ir, None, &mut out);
            out
        };
        assert_eq!(reads_of("SUM(cost[FIRST:THIS])"), [0]);
        assert_eq!(reads_of("SUM(cost[FIRST:-1]) + cost[-1]"), Vec::<usize>::new());
    }

    #[test]
    fn a_window_runs_over_the_rows_in_order() {
        // Each row's value, a window in a column's formula over three rows.
        let column = |src: &str, rows: Vec<[V; 3]>| -> Vec<V> {
            let (ir, _) = parse(src).and_then(|e| resolve_column(&e, &One, &mut 0)).unwrap_or_else(|f| panic!("{src}: {}", f.why));
            let n = rows.len();
            let mut r = Rows(rows, Vec::new(), Vec::new(), Vec::new());
            (0..n).map(|row| eval(&ir, None, row, &mut r)).collect()
        };
        let num = |s: &str| V::Num(Exact::parse(s));
        let rows = || {
            Vec::from([
                [num("10"), num("3"), V::Text("c".into())],
                [num("20"), num("1"), V::Text("a".into())],
                [num("30"), num("2"), V::Text("b".into())],
            ])
        };
        assert_eq!(column("SUM(cost) OVER (ORDER BY qty)", rows()), [num("60"), num("20"), num("50")]);
        assert_eq!(column("SUM(cost) OVER (ORDER BY item DESC)", rows()), [num("10"), num("60"), num("40")]);
        assert_eq!(column("LAG(cost) OVER (ORDER BY qty)", rows()), [num("30"), V::Null, num("20")]);
        assert_eq!(column("COUNT(cost) OVER (ORDER BY qty)", rows()), [num("3"), num("1"), num("2")]);
        // A tie the order does not break: neither row has a value, nor the one after them
        // for lag.
        let tied = || {
            Vec::from([
                [num("10"), num("1"), V::Text("a".into())],
                [num("20"), num("1"), V::Text("b".into())],
                [num("5"), num("2"), V::Text("c".into())],
            ])
        };
        assert_eq!(column("SUM(cost) OVER (ORDER BY qty)", tied()), [V::Null, V::Null, num("35")]);
        assert_eq!(column("LAG(cost) OVER (ORDER BY qty)", tied()), [V::Null, V::Null, V::Null]);
        // Only a column's formula holds one.
        assert!(parse("SUM(cost) OVER (ORDER BY qty)").and_then(|e| resolve(&e, &One, &mut 0)).is_err());
    }

    #[test]
    fn every_function_named_resolves() {
        for f in FUNCTIONS {
            let call = match f {
                "LOWER" | "UPPER" | "TRIM" | "LENGTH" => format!("{f}(item)"),
                "ROUND" | "NULLIF" | "COALESCE" => format!("{f}(cost, 1)"),
                _ => format!("{f}(cost)"),
            };
            let resolved = parse(&call).and_then(|e| resolve(&e, &One, &mut 0));
            assert!(resolved.is_ok(), "{call}: {:?}", resolved.err());
        }
    }

    #[test]
    fn names_are_where_they_are_written() {
        let src = "cost * \"the qty\" + fees['card'].pct + to.kind + SUM(total[FIRST:THIS]) + to[-1].kind";
        let got: Vec<Vec<&str>> =
            names(src).unwrap().iter().map(|n| n.parts.iter().map(|(_, (s, l))| &src[*s as usize..(s + l) as usize]).collect()).collect();
        assert_eq!(
            got,
            [
                Vec::from(["cost"]),
                Vec::from(["\"the qty\""]),
                Vec::from(["fees", "pct"]),
                Vec::from(["to", "kind"]),
                Vec::from(["total"]),
                Vec::from(["to", "kind"])
            ]
        );
        let fees = &names(src).unwrap()[2];
        assert!(fees.table);
        assert_eq!(fees.key.as_ref().map(|(k, (s, l))| (k.as_str(), &src[*s as usize..(s + l) as usize])), Some(("card", "'card'")));
    }

    #[test]
    fn a_fault_spans_what_broke() {
        fn at(src: &str) -> Option<&str> {
            let fault = parse(src).and_then(|e| resolve(&e, &One, &mut 0).map(|_| ())).unwrap_err();
            fault.at.map(|(s, l)| &src[s as usize..(s + l) as usize])
        }
        assert_eq!(at("cost + nope"), Some("nope"));
        assert_eq!(at("cost * item"), Some("*"));
        assert_eq!(at("ROUND(cost) + SUM(SUM(cost))"), Some("SUM"));
    }
}
