//! The JSON a script and its runner pass: objects of scalars, as much as a row is.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// A scalar, a number kept as written.
#[derive(Clone, Debug, PartialEq)]
pub enum Scalar {
    Null,
    Bool(bool),
    Number(String),
    Str(String),
}

/// A string as JSON writes it.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("a {} was expected at byte {}", c as char, self.i))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let Some(&c) = self.s.get(self.i) else { return Err("a string is not closed".into()) };
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = *self.s.get(self.i).ok_or("a string is not closed")?;
                    self.i += 1;
                    match e {
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let hex = |p: &mut Parser| -> Result<u32, String> {
                                let h = core::str::from_utf8(p.s.get(p.i..p.i + 4).ok_or("a \\u escape is cut")?)
                                    .map_err(|e| format!("{e}"))?;
                                p.i += 4;
                                u32::from_str_radix(h, 16).map_err(|e| format!("{e}"))
                            };
                            let mut cp = hex(self)?;
                            if (0xD800..0xDC00).contains(&cp) {
                                self.i += 2;
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (hex(self)? - 0xDC00);
                            }
                            out.push(char::from_u32(cp).ok_or("an escape names no character")?);
                        }
                        other => out.push(other as char),
                    }
                }
                _ => {
                    let start = self.i - 1;
                    let len = match c {
                        0..0x80 => 1,
                        0xC0..0xE0 => 2,
                        0xE0..0xF0 => 3,
                        _ => 4,
                    };
                    out.push_str(core::str::from_utf8(&self.s[start..start + len]).map_err(|e| format!("{e}"))?);
                    self.i = start + len;
                }
            }
        }
    }

    fn scalar(&mut self) -> Result<Scalar, String> {
        self.ws();
        match self.s.get(self.i) {
            Some(b'"') => Ok(Scalar::Str(self.string()?)),
            Some(b't') if self.s[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(Scalar::Bool(true))
            }
            Some(b'f') if self.s[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(Scalar::Bool(false))
            }
            Some(b'n') if self.s[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(Scalar::Null)
            }
            Some(c) if c.is_ascii_digit() || *c == b'-' => {
                let start = self.i;
                while self.i < self.s.len() && matches!(self.s[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.i += 1;
                }
                Ok(Scalar::Number(String::from_utf8_lossy(&self.s[start..self.i]).into_owned()))
            }
            _ => Err(format!("a value of a row is text, a number, a boolean or null (byte {})", self.i)),
        }
    }
}

/// One line a script wrote: a row, its columns by name, in the order written.
pub fn row(line: &str) -> Result<Vec<(String, Scalar)>, String> {
    let mut p = Parser { s: line.as_bytes(), i: 0 };
    p.eat(b'{')?;
    let mut out = Vec::new();
    p.ws();
    if p.s.get(p.i) == Some(&b'}') {
        return Ok(out);
    }
    loop {
        let k = p.string()?;
        p.eat(b':')?;
        out.push((k, p.scalar()?));
        p.ws();
        match p.s.get(p.i) {
            Some(b',') => p.i += 1,
            Some(b'}') => return Ok(out),
            _ => return Err("a row is an object of the table's columns".into()),
        }
    }
}
