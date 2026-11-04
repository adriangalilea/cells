//! The prelude: named types, computed columns and `//` comments between two `---` lines,
//! and the evaluator of computed columns, pure and total in exact decimals.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::exact::Exact;
use crate::types::{self, NamedType};

/// A computed column: `total = cost * qty`.
#[derive(Clone, Debug, PartialEq)]
pub struct Computed {
    pub name: String,
    /// As written after the `=`, trimmed.
    pub expression: String,
    /// Its prelude line, 1-based inside the fences.
    pub line: u32,
    /// Postfix, column names resolved to column indices by `resolve`.
    pub program: Vec<Op>,
    /// The stack the program needs.
    pub depth: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Number(Exact),
    Name(String),
    Column(usize),
    Negate,
    Add,
    Sub,
    Mul,
    Div,
}

/// What the prelude says.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prelude {
    pub named: Vec<NamedType>,
    pub computed: Vec<Computed>,
}

/// The line with its `//` comment cut, outside single quotes.
pub(crate) fn uncomment(line: &str) -> &str {
    let b = line.as_bytes();
    let mut quoted = false;
    for i in 0..b.len() {
        if b[i] == b'\'' {
            quoted = !quoted;
        } else if !quoted && b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            return &line[..i];
        }
    }
    line
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// The prelude's lines (inside the fences, without their line breaks), read. An error
/// names its line, 1-based.
pub fn parse(lines: &[&str]) -> Result<Prelude, (u32, String)> {
    let mut p = Prelude::default();
    for (n, raw) in lines.iter().enumerate() {
        let line = (n + 1) as u32;
        const BLANK: [char; 2] = [' ', '\t'];
        let said = uncomment(raw).trim_matches(BLANK);
        if said.is_empty() {
            continue;
        }
        let fail = |why: String| (line, why);
        if let Some(rest) = said.strip_prefix("type")
            && rest.starts_with(BLANK)
            && !rest.trim_start_matches(BLANK).starts_with('=')
        {
            let rest = rest.trim_start_matches(BLANK);
            let end = rest.bytes().position(|c| !is_ident(c)).unwrap_or(rest.len());
            let name = &rest[..end];
            let Some(spelled) = rest[end..].trim_start_matches(BLANK).strip_prefix('=') else {
                return Err(fail("a named type is `type Name = <type>`".to_owned()));
            };
            if !name.as_bytes().first().is_some_and(|c| c.is_ascii_uppercase()) {
                return Err(fail(format!("a type's name starts with a capital letter, not \"{name}\"")));
            }
            if types::currency(name).is_some() {
                return Err(fail(format!("{name} is a currency")));
            }
            if p.named.iter().any(|t| t.name == name) {
                return Err(fail(format!("the type {name} is named twice")));
            }
            let ty = types::parse_type(spelled, &p.named).map_err(fail)?;
            p.named.push(NamedType { name: name.to_owned(), ty });
            continue;
        }
        let end = said.bytes().position(|c| !is_ident(c)).unwrap_or(said.len());
        let name = &said[..end];
        let Some(expression) = said[end..].trim_start_matches(BLANK).strip_prefix('=') else {
            return Err(fail("is neither `type Name = <type>` nor `name = expression`".to_owned()));
        };
        if !name.as_bytes().first().is_some_and(|&c| is_ident_start(c)) {
            return Err(fail("is neither `type Name = <type>` nor `name = expression`".to_owned()));
        }
        if p.computed.iter().any(|c| c.name == name) {
            return Err(fail(format!("the column {name} is computed twice")));
        }
        let expression = expression.trim_matches(BLANK);
        let (program, depth) = compile(expression).map_err(fail)?;
        p.computed.push(Computed { name: name.to_owned(), expression: expression.to_owned(), line, program, depth });
    }
    Ok(p)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tok<'a> {
    Number(&'a str),
    Name(&'a str),
    Sym(u8),
}

fn lex(src: &str) -> Result<Vec<Tok<'_>>, String> {
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
            out.push(Tok::Number(&src[start..i]));
        } else if is_ident_start(c) {
            let start = i;
            while i < b.len() && is_ident(b[i]) {
                i += 1;
            }
            out.push(Tok::Name(&src[start..i]));
        } else if matches!(c, b'+' | b'-' | b'*' | b'/' | b'(' | b')') {
            out.push(Tok::Sym(c));
            i += 1;
        } else {
            let ch = src[i..].chars().next().unwrap();
            return Err(format!("\"{ch}\" cannot stand in an expression"));
        }
    }
    Ok(out)
}

struct Compiler<'a> {
    toks: Vec<Tok<'a>>,
    at: usize,
    out: Vec<Op>,
}

impl<'a> Compiler<'a> {
    fn peek(&self) -> Option<Tok<'a>> {
        self.toks.get(self.at).copied()
    }

    fn sum(&mut self) -> Result<(), String> {
        self.product()?;
        while let Some(Tok::Sym(c @ (b'+' | b'-'))) = self.peek() {
            self.at += 1;
            self.product()?;
            self.out.push(if c == b'+' { Op::Add } else { Op::Sub });
        }
        Ok(())
    }

    fn product(&mut self) -> Result<(), String> {
        self.unary()?;
        while let Some(Tok::Sym(c @ (b'*' | b'/'))) = self.peek() {
            self.at += 1;
            self.unary()?;
            self.out.push(if c == b'*' { Op::Mul } else { Op::Div });
        }
        Ok(())
    }

    fn unary(&mut self) -> Result<(), String> {
        let t = self.peek().ok_or("the expression ends early")?;
        self.at += 1;
        match t {
            Tok::Sym(b'-') => {
                self.unary()?;
                self.out.push(Op::Negate);
            }
            Tok::Sym(b'(') => {
                self.sum()?;
                if self.peek() != Some(Tok::Sym(b')')) {
                    return Err("a ( is not closed".to_owned());
                }
                self.at += 1;
            }
            // Digits with at most one point: always a binary64.
            Tok::Number(n) => self.out.push(Op::Number(Exact::parse(n))),
            Tok::Name(n) => self.out.push(Op::Name(n.to_owned())),
            Tok::Sym(c) => return Err(format!("\"{}\" cannot stand there", c as char)),
        }
        Ok(())
    }
}

/// `cost * qty` as postfix, and the stack it needs.
fn compile(src: &str) -> Result<(Vec<Op>, usize), String> {
    let mut c = Compiler { toks: lex(src)?, at: 0, out: Vec::new() };
    if c.toks.is_empty() {
        return Err("the expression is empty".to_owned());
    }
    c.sum()?;
    if let Some(t) = c.peek() {
        let shown = match t {
            Tok::Number(n) | Tok::Name(n) => n.to_owned(),
            Tok::Sym(s) => (s as char).into(),
        };
        return Err(format!("\"{shown}\" is left over"));
    }
    let (mut depth, mut max) = (0usize, 0usize);
    for op in &c.out {
        match op {
            Op::Number(_) | Op::Name(_) | Op::Column(_) => depth += 1,
            Op::Negate => {}
            _ => depth -= 1,
        }
        max = max.max(depth);
    }
    debug_assert_eq!(depth, 1);
    Ok((c.out, max))
}

/// Evaluates a resolved program on one row; `input(k)` is column `k`'s value there, or
/// nothing for a blank or broken cell. A blank input or a division by zero makes the
/// result blank.
pub fn evaluate(c: &Computed, input: impl Fn(usize) -> Option<Exact>) -> Option<Exact> {
    let mut stack: Vec<Exact> = Vec::with_capacity(c.depth);
    for op in &c.program {
        match op {
            Op::Number(v) => stack.push(v.clone()),
            Op::Column(k) => stack.push(input(*k)?),
            Op::Name(_) => unreachable!("an unresolved program"),
            Op::Negate => {
                let a = stack.pop().unwrap();
                stack.push(a.neg());
            }
            bin => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                stack.push(match bin {
                    Op::Add => a.add(&b),
                    Op::Sub => a.sub(&b),
                    Op::Mul => a.mul(&b),
                    _ => a.div(&b)?,
                });
            }
        }
    }
    debug_assert_eq!(stack.len(), 1);
    stack.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements() {
        let p = parse(&["type Category = travel|stay // the kinds", "", "total = cost * (qty + 1) // computed"]).unwrap();
        assert_eq!(p.named[0].name, "Category");
        assert_eq!(p.computed[0].expression, "cost * (qty + 1)");
        assert_eq!(p.computed[0].line, 3);
        assert_eq!(p.computed[0].depth, 3);
        assert_eq!(parse(&["nonsense"]).unwrap_err().0, 1);
        assert_eq!(parse(&["", "total = cost *"]).unwrap_err().0, 2);
        assert_eq!(parse(&["type lower = int"]).unwrap_err().0, 1);
        assert!(parse(&["type Mood = 'a//b'|c"]).is_ok());
    }
}
