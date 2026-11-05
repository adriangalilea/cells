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
    /// `cost`, `to.kind`, `fees.pct`: names joined by `.`.
    Path(Vec<String>),
    /// `fees['card'].pct`: a table's row by its key, then a column of it.
    Lookup(String, Box<Expr>, Vec<String>),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(Bin, Box<Expr>, Box<Expr>),
    /// `x is null`, or `x is not null` when true.
    IsNull(Box<Expr>, bool),
    Case(Vec<(Expr, Expr)>, Option<Box<Expr>>),
    Call(String, Vec<Expr>),
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

const SYMS: [&str; 18] = ["||", "<>", "!=", "<=", ">=", "+", "-", "*", "/", "(", ")", "[", "]", ",", ".", "=", "<", ">"];

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b' ' || c == b'\t' {
            i += 1;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if b.get(i) == Some(&b'.') && b.get(i + 1).is_some_and(|d| d.is_ascii_digit()) {
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            out.push(Tok::Num(src[start..i].to_owned()));
        } else if c == b'\'' || c == b'"' {
            let mut v = String::new();
            i += 1;
            let mut from = i;
            loop {
                if i >= b.len() {
                    return Err(if c == b'\'' { "a text's ' is not closed" } else { "a name's \" is not closed" }.to_owned());
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
            out.push(if c == b'\'' { Tok::Str(v) } else { Tok::Quoted(v) });
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(Tok::Name(src[start..i].to_owned()));
        } else if let Some(s) = SYMS.iter().find(|s| b[i..].starts_with(s.as_bytes())) {
            out.push(Tok::Sym(s));
            i += s.len();
        } else {
            let ch = src[i..].chars().next().unwrap();
            return Err(format!("\"{ch}\" cannot stand in a formula"));
        }
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    fn sym(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Sym(x)) if *x == s)
    }

    fn word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Name(x)) if x.eq_ignore_ascii_case(w))
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

    fn expect_sym(&mut self, s: &str) -> Result<(), String> {
        if self.eat_sym(s) { Ok(()) } else { Err(format!("a {s} was expected, not {}", self.shown())) }
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

    fn or(&mut self) -> Result<Expr, String> {
        let mut a = self.and()?;
        while self.eat_word("or") {
            a = Expr::Bin(Bin::Or, Box::new(a), Box::new(self.and()?));
        }
        Ok(a)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut a = self.not()?;
        while self.eat_word("and") {
            a = Expr::Bin(Bin::And, Box::new(a), Box::new(self.not()?));
        }
        Ok(a)
    }

    fn not(&mut self) -> Result<Expr, String> {
        if self.eat_word("not") {
            return Ok(Expr::Not(Box::new(self.not()?)));
        }
        self.compare()
    }

    fn compare(&mut self) -> Result<Expr, String> {
        let a = self.concat()?;
        if self.eat_word("is") {
            let negated = self.eat_word("not");
            if !self.eat_word("null") {
                return Err(format!("is takes null, not {}", self.shown()));
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
        Ok(Expr::Bin(op, Box::new(a), Box::new(self.concat()?)))
    }

    fn concat(&mut self) -> Result<Expr, String> {
        let mut a = self.sum()?;
        while self.eat_sym("||") {
            a = Expr::Bin(Bin::Concat, Box::new(a), Box::new(self.sum()?));
        }
        Ok(a)
    }

    fn sum(&mut self) -> Result<Expr, String> {
        let mut a = self.product()?;
        loop {
            let op = if self.eat_sym("+") {
                Bin::Add
            } else if self.eat_sym("-") {
                Bin::Sub
            } else {
                return Ok(a);
            };
            a = Expr::Bin(op, Box::new(a), Box::new(self.product()?));
        }
    }

    fn product(&mut self) -> Result<Expr, String> {
        let mut a = self.unary()?;
        loop {
            let op = if self.eat_sym("*") {
                Bin::Mul
            } else if self.eat_sym("/") {
                Bin::Div
            } else {
                return Ok(a);
            };
            a = Expr::Bin(op, Box::new(a), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat_sym("-") {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn name(&mut self) -> Result<String, String> {
        match self.peek().cloned() {
            Some(Tok::Name(n) | Tok::Quoted(n)) => {
                self.at += 1;
                Ok(n)
            }
            _ => Err(format!("a name was expected, not {}", self.shown())),
        }
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let Some(t) = self.peek().cloned() else { return Err("the formula ends early".to_owned()) };
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
                self.at += 1;
                let e = self.or()?;
                if !self.eat_sym(")") {
                    return Err("a ( is not closed".to_owned());
                }
                Ok(e)
            }
            Tok::Name(n) if n.eq_ignore_ascii_case("true") || n.eq_ignore_ascii_case("false") => {
                self.at += 1;
                Ok(Expr::Bool(n.eq_ignore_ascii_case("true")))
            }
            Tok::Name(n) if n.eq_ignore_ascii_case("null") => {
                self.at += 1;
                Ok(Expr::Null)
            }
            Tok::Name(n) if n.eq_ignore_ascii_case("case") => {
                self.at += 1;
                let mut arms = Vec::new();
                while self.eat_word("when") {
                    let w = self.or()?;
                    if !self.eat_word("then") {
                        return Err(format!("when takes then, not {}", self.shown()));
                    }
                    arms.push((w, self.or()?));
                }
                if arms.is_empty() {
                    return Err("case takes when".to_owned());
                }
                let otherwise = if self.eat_word("else") { Some(Box::new(self.or()?)) } else { None };
                if !self.eat_word("end") {
                    return Err(format!("case closes with end, not {}", self.shown()));
                }
                Ok(Expr::Case(arms, otherwise))
            }
            Tok::Name(_) | Tok::Quoted(_) => {
                let first = self.name()?;
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
                    return Ok(Expr::Call(first.to_ascii_lowercase(), args));
                }
                if self.eat_sym("[") {
                    let key = self.or()?;
                    self.expect_sym("]")?;
                    let mut rest = Vec::new();
                    while self.eat_sym(".") {
                        rest.push(self.name()?);
                    }
                    if rest.is_empty() {
                        return Err(format!("{first}[…] names a row: a column follows, {first}[…].column"));
                    }
                    return Ok(Expr::Lookup(first, Box::new(key), rest));
                }
                let mut path = Vec::from([first]);
                while self.eat_sym(".") {
                    path.push(self.name()?);
                }
                Ok(Expr::Path(path))
            }
            _ => Err(format!("{} cannot stand there", self.shown())),
        }
    }
}

/// A formula's text, parsed.
pub fn parse(src: &str) -> Result<Expr, String> {
    let mut p = Parser { toks: lex(src)?, at: 0 };
    if p.toks.is_empty() {
        return Err("the formula is empty".to_owned());
    }
    let e = p.or()?;
    if p.peek().is_some() {
        return Err(format!("{} is left over", p.shown()));
    }
    Ok(e)
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

/// A formula resolved: every name a place, every kind checked.
#[derive(Clone, Debug, PartialEq)]
pub enum Ir {
    Const(V),
    /// A column of the row in context.
    Col(usize),
    /// A key, looked up in each target in turn (its table, the key column, the column
    /// read on the row found).
    Follow(Box<Ir>, Vec<(usize, usize, usize)>),
    Agg(Agg, Table, Box<Ir>, usize),
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
    fn key(&self, table: usize) -> Option<usize>;
}

/// Resolves a parsed formula; `aggregates` counts the aggregates resolved so far, each one
/// a slot its value is remembered in.
pub fn resolve(e: &Expr, scope: &dyn Scope, aggregates: &mut usize) -> Result<(Ir, K), String> {
    Resolver { scope, aggregates }.at(e, None, false)
}

struct Resolver<'a, 'b> {
    scope: &'a dyn Scope,
    aggregates: &'b mut usize,
}

impl Resolver<'_, '_> {
    /// `e` read on a row of `table`; `inside` when within an aggregate.
    fn at(&mut self, e: &Expr, table: Table, inside: bool) -> Result<(Ir, K), String> {
        Ok(match e {
            Expr::Num(n) => (Ir::Const(V::Num(n.clone())), K::Num),
            Expr::Text(t) => (Ir::Const(V::Text(t.clone())), K::Text),
            Expr::Bool(b) => (Ir::Const(V::Bool(*b)), K::Bool),
            Expr::Null => (Ir::Const(V::Null), K::Null),
            Expr::Path(path) => self.path(path, table)?,
            Expr::Lookup(name, key, rest) => {
                let t = self.scope.table(name).ok_or_else(|| format!("no table \"{name}\""))?;
                let k = self.scope.key(t).ok_or_else(|| format!("{name} has no key, so no row is found by one"))?;
                let (key, _) = self.at(key, table, inside)?;
                let (col, kind) = self.scope.column(Some(t), &rest[0]).ok_or_else(|| format!("{name} has no column \"{}\"", rest[0]))?;
                let ir = Ir::Follow(Box::new(key), Vec::from([(t, k, col)]));
                self.follow(ir, Some(t), col, kind, &rest[1..], &rest[0])?
            }
            Expr::Neg(a) => {
                let (a, k) = self.at(a, table, inside)?;
                number(k, "-")?;
                (Ir::Neg(Box::new(a)), K::Num)
            }
            Expr::Not(a) => {
                let (a, k) = self.at(a, table, inside)?;
                boolean(k, "not")?;
                (Ir::Not(Box::new(a)), K::Bool)
            }
            Expr::IsNull(a, negated) => {
                let (a, _) = self.at(a, table, inside)?;
                (Ir::IsNull(Box::new(a), *negated), K::Bool)
            }
            Expr::Bin(op, a, b) => {
                let (a, ka) = self.at(a, table, inside)?;
                let (b, kb) = self.at(b, table, inside)?;
                let k = match op {
                    Bin::Add | Bin::Sub | Bin::Mul | Bin::Div => {
                        let s = match op {
                            Bin::Add => "+",
                            Bin::Sub => "-",
                            Bin::Mul => "*",
                            _ => "/",
                        };
                        number(ka, s)?;
                        number(kb, s)?;
                        K::Num
                    }
                    Bin::Concat => K::Text,
                    Bin::And | Bin::Or => {
                        let s = if *op == Bin::And { "and" } else { "or" };
                        boolean(ka, s)?;
                        boolean(kb, s)?;
                        K::Bool
                    }
                    _ => {
                        ka.meet(kb).ok_or_else(|| format!("{} and {} are not compared", ka.word(), kb.word()))?;
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
                    boolean(kw, "when")?;
                    let (t, kt) = self.at(t, table, inside)?;
                    kind = kind.meet(kt).ok_or_else(|| format!("a case gives {} and {}", kind.word(), kt.word()))?;
                    out.push((w, t));
                }
                let otherwise = match otherwise {
                    Some(o) => {
                        let (o, ko) = self.at(o, table, inside)?;
                        kind = kind.meet(ko).ok_or_else(|| format!("a case gives {} and {}", kind.word(), ko.word()))?;
                        o
                    }
                    None => Ir::Const(V::Null),
                };
                (Ir::Case(out, Box::new(otherwise)), kind)
            }
            Expr::Call(name, args) => self.call(name, args, table, inside)?,
        })
    }

    fn path(&mut self, path: &[String], table: Table) -> Result<(Ir, K), String> {
        let first = &path[0];
        if let Some((col, kind)) = self.scope.column(table, first) {
            return self.follow(Ir::Col(col), table, col, kind, &path[1..], first);
        }
        if self.scope.table(first).is_some() {
            return Err(match path.get(1) {
                Some(c) => format!("{first}.{c} is a whole column: read one row ({first}[key].{c}) or all of them (sum({first}.{c}))"),
                None => format!("{first} is a table, not a value"),
            });
        }
        Err(format!("no column \"{first}\""))
    }

    /// `ir`, column `col` of `table` read, then each name of `rest` followed through the
    /// reference the column before it is.
    #[allow(clippy::too_many_arguments)]
    fn follow(
        &mut self,
        mut ir: Ir,
        mut table: Table,
        mut col: usize,
        mut kind: K,
        rest: &[String],
        shown: &str,
    ) -> Result<(Ir, K), String> {
        let mut shown = shown.to_owned();
        for name in rest {
            let targets = self.scope.targets(table, col);
            if targets.is_empty() {
                return Err(format!("{shown} is no reference, so it has no {name}"));
            }
            let mut steps = Vec::with_capacity(targets.len());
            let mut k = K::Null;
            for (t, key) in targets {
                let (c, kc) = self.scope.column(Some(t), name).ok_or_else(|| format!("{shown} leads to no column \"{name}\""))?;
                k = k.meet(kc).ok_or_else(|| format!("{name} is {} in one table and {} in another", k.word(), kc.word()))?;
                steps.push((t, key, c));
            }
            table = Some(steps[0].0);
            col = steps[0].2;
            kind = k;
            ir = Ir::Follow(Box::new(ir), steps);
            shown = name.clone();
        }
        Ok((ir, kind))
    }

    fn call(&mut self, name: &str, args: &[Expr], table: Table, inside: bool) -> Result<(Ir, K), String> {
        let agg = match name {
            "sum" => Some(Agg::Sum),
            "avg" => Some(Agg::Avg),
            "min" => Some(Agg::Min),
            "max" => Some(Agg::Max),
            "count" => Some(Agg::Count),
            _ => None,
        };
        if let Some(agg) = agg {
            if inside {
                return Err(format!("{name} inside an aggregate"));
            }
            if args.len() != 1 {
                return Err(format!("{name} takes one value"));
            }
            let over = over(&args[0], self.scope);
            let (arg, k) = self.at(&args[0], over, true)?;
            let kind = match agg {
                Agg::Sum | Agg::Avg => {
                    number(k, name)?;
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
            if n.contains(&args.len()) { Ok(()) } else { Err(format!("{name} takes {} values, not {}", n.end(), args.len())) }
        };
        let (f, k) = match name {
            "round" => {
                arity(1..=2)?;
                kinds.iter().try_for_each(|k| number(*k, name))?;
                (F::Round, K::Num)
            }
            "abs" | "floor" | "ceil" => {
                arity(1..=1)?;
                number(kinds[0], name)?;
                (
                    match name {
                        "abs" => F::Abs,
                        "floor" => F::Floor,
                        _ => F::Ceil,
                    },
                    K::Num,
                )
            }
            "coalesce" => {
                if args.is_empty() {
                    return Err("coalesce takes values".to_owned());
                }
                let mut k = K::Null;
                for x in &kinds {
                    k = k.meet(*x).ok_or_else(|| format!("coalesce of {} and {}", k.word(), x.word()))?;
                }
                (F::Coalesce, k)
            }
            "nullif" => {
                arity(2..=2)?;
                kinds[0].meet(kinds[1]).ok_or_else(|| format!("{} and {} are not compared", kinds[0].word(), kinds[1].word()))?;
                (F::Nullif, kinds[0])
            }
            "lower" | "upper" | "trim" | "length" => {
                arity(1..=1)?;
                if !matches!(kinds[0], K::Text | K::Null) {
                    return Err(format!("{name} takes text, not {}", kinds[0].word()));
                }
                match name {
                    "lower" => (F::Lower, K::Text),
                    "upper" => (F::Upper, K::Text),
                    "trim" => (F::Trim, K::Text),
                    _ => (F::Length, K::Num),
                }
            }
            _ => return Err(format!("no function {name}")),
        };
        Ok((Ir::Call(f, out), k))
    }
}

/// The table an aggregate runs over: the one its first `table.column` names, else the
/// row's own.
fn over(e: &Expr, scope: &dyn Scope) -> Table {
    fn find(e: &Expr, scope: &dyn Scope) -> Option<usize> {
        match e {
            Expr::Path(p) if p.len() > 1 && scope.column(None, &p[0]).is_none() => scope.table(&p[0]),
            Expr::Neg(a) | Expr::Not(a) | Expr::IsNull(a, _) => find(a, scope),
            Expr::Bin(_, a, b) => find(a, scope).or_else(|| find(b, scope)),
            Expr::Case(arms, o) => arms
                .iter()
                .find_map(|(w, t)| find(w, scope).or_else(|| find(t, scope)))
                .or_else(|| o.as_deref().and_then(|o| find(o, scope))),
            Expr::Call(_, args) => args.iter().find_map(|a| find(a, scope)),
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
    fn find(&mut self, table: usize, key: usize, value: &V) -> Option<usize>;
    /// An aggregate's value, when it was evaluated already.
    fn recall(&mut self, slot: usize) -> Option<V>;
    fn remember(&mut self, slot: usize, v: V);
}

/// `ir` on row `row` of `table`.
pub fn eval(ir: &Ir, table: Table, row: usize, src: &mut dyn Source) -> V {
    match ir {
        Ir::Const(v) => v.clone(),
        Ir::Col(k) => src.cell(table, row, *k),
        Ir::Follow(key, targets) => {
            let k = eval(key, table, row, src);
            if k == V::Null {
                return V::Null;
            }
            for &(t, key_col, col) in targets {
                if let Some(r) = src.find(t, key_col, &k) {
                    return src.cell(Some(t), r, col);
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
        Ir::Follow(key, _) => reads(key, table, out),
        Ir::Agg(_, over, arg, _) => reads(arg, *over, out),
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
        fn key(&self, _: usize) -> Option<usize> {
            None
        }
    }

    struct Rows(Vec<[V; 3]>, Vec<Option<V>>);

    impl Source for Rows {
        fn cell(&mut self, _: Table, row: usize, column: usize) -> V {
            self.0[row][column].clone()
        }
        fn rows(&self, _: Table) -> usize {
            self.0.len()
        }
        fn find(&mut self, _: usize, _: usize, _: &V) -> Option<usize> {
            None
        }
        fn recall(&mut self, slot: usize) -> Option<V> {
            self.1.get(slot).cloned().flatten()
        }
        fn remember(&mut self, slot: usize, v: V) {
            self.1.resize(slot + 1, None);
            self.1[slot] = Some(v);
        }
    }

    fn run(src: &str) -> String {
        let e = parse(src).unwrap_or_else(|e| panic!("{src}: {e}"));
        let mut n = 0;
        let (ir, _) = resolve(&e, &One, &mut n).unwrap_or_else(|e| panic!("{src}: {e}"));
        let mut rows = Rows(
            Vec::from([
                [V::Num(Exact::parse("84.20")), V::Num(Exact::parse("2")), V::Text("fuel".into())],
                [V::Num(Exact::parse("10")), V::Null, V::Text("maps".into())],
            ]),
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
        assert_eq!(run("round(cost / 3, 2)"), "28.07");
        assert_eq!(run("case when qty > 1 then 'many' else 'one' end"), "'many'");
        assert_eq!(run("item || ' x' || qty"), "'fuel x2'");
        assert_eq!(run("sum(cost)"), "94.20");
        assert_eq!(run("count(qty)"), "1");
        assert_eq!(run("coalesce(null, 3)"), "3");
        assert_eq!(run("upper(item)"), "'FUEL'");
        assert_eq!(run("1 / 0"), "null");
        assert_eq!(run("not (qty = 2) or false"), "false");
        assert_eq!(run("null and false"), "false");
        assert_eq!(run("qty is not null"), "true");
        assert_eq!(run("-cost + 0.20"), "-84.00");
        assert_eq!(run("floor(-2.5)"), "-3");
        assert_eq!(run("ceil(2.1)"), "3");
        for bad in ["cost +", "cost * item", "item and true", "nope", "sum(sum(cost))", "case when 1 then 2 end", "'open"] {
            let refused = parse(bad).and_then(|e| resolve(&e, &One, &mut 0).map(|_| ()));
            assert!(refused.is_err(), "{bad} should not resolve");
        }
    }
}
