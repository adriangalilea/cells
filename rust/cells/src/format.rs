//! What a file is: a `.cell` file read by the format's own law, or any other delimited text (a
//! `.csv`, a `.tsv`, plain text) read as the files people hold are, its separator found in
//! its first lines.

use crate::scan::Records;

/// The byte between a record's fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Separator {
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

impl Separator {
    /// Every separator, in the order a tie between them is settled: CSV's own comma, then
    /// the tab, the semicolon of a European export, the pipe.
    pub const ALL: [Separator; 4] = [Separator::Comma, Separator::Tab, Separator::Semicolon, Separator::Pipe];

    pub fn byte(self) -> u8 {
        match self {
            Separator::Comma => b',',
            Separator::Semicolon => b';',
            Separator::Tab => b'\t',
            Separator::Pipe => b'|',
        }
    }

    pub fn of(byte: u8) -> Option<Separator> {
        Separator::ALL.into_iter().find(|s| s.byte() == byte)
    }

    /// What a person calls it: `comma`, `semicolon`, `tab`, `pipe`.
    pub fn name(self) -> &'static str {
        match self {
            Separator::Comma => "comma",
            Separator::Semicolon => "semicolon",
            Separator::Tab => "tab",
            Separator::Pipe => "pipe",
        }
    }
}

/// How a file is read and written: Cells, or another dialect of delimited text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dialect {
    /// A `.cell` file (SPEC.md): fields separated by commas, a prelude, typed header cells,
    /// and a broken grammar refused.
    Cells,
    /// Delimited text that is not Cells: the header's cells are its columns' names (types
    /// inferred, no prelude), and nothing in its records is refused. A row of another
    /// width than the header reads as blanks with a `ragged` problem, a quote out of
    /// RFC 4180's places is text, a quoted field never closed runs to the end of the file,
    /// a lone `\r` ends a line, and a blank or repeated name is a problem of the header.
    Plain(Separator),
}

impl Dialect {
    pub fn separator(self) -> Separator {
        match self {
            Dialect::Cells => Separator::Comma,
            Dialect::Plain(s) => s,
        }
    }

    /// A cell's text as a new file in this dialect writes it: quoted only when it holds
    /// the separator, `"` or a line break.
    pub fn quote(self, text: &str) -> alloc::borrow::Cow<'_, str> {
        crate::scan::quote_in(text, self.separator().byte(), false)
    }

    pub(crate) fn records(self) -> Records {
        match self {
            Dialect::Cells => Records::CELLS,
            Dialect::Plain(s) => Records { sep: s.byte(), lenient: true },
        }
    }
}

/// The separator a text's first lines agree on: the one splitting each of them (outside
/// quotes) into as many fields as the first, the most fields winning, a tie settled in
/// `Separator::ALL`'s order. The first line alone would mislead: a Cells header's choices
/// hold pipes (`seed:basil|leek|chard,grams:num`) that no row does. Blank lines say
/// nothing; a text of one line takes the separator it holds most, and one holding none the
/// comma.
pub fn detect(bytes: &[u8]) -> Separator {
    /// The lines read: enough to agree, few enough to be instant on any file.
    const LINES: usize = 6;
    let mut lines: [[u32; 4]; LINES] = [[0; 4]; LINES];
    let mut n = 0;
    let mut counts = [0u32; 4];
    let mut quoted = false;
    let mut i = 0;
    while i < bytes.len() && n < LINES {
        let c = bytes[i];
        if c == b'"' {
            quoted = !quoted;
        } else if !quoted {
            if c == b'\n' || c == b'\r' {
                lines[n] = counts;
                n += 1;
                counts = [0; 4];
                if c == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
            } else if let Some(k) = Separator::ALL.iter().position(|s| s.byte() == c) {
                counts[k] += 1;
            }
        }
        i += 1;
    }
    if n < LINES && counts != [0; 4] {
        lines[n] = counts;
        n += 1;
    }
    if n == 0 {
        return Separator::Comma;
    }
    let first = lines[0];
    let telling = || lines[..n].iter().filter(|l| **l != [0; 4]);
    let agreed: [bool; 4] = core::array::from_fn(|k| first[k] > 0 && telling().all(|l| l[k] == first[k]));
    let none = !agreed.contains(&true);
    let mut best = 0;
    for k in 1..4 {
        if (none || agreed[k]) && (!(none || agreed[best]) || first[k] > first[best]) {
            best = k;
        }
    }
    Separator::ALL[best]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lines_agree() {
        assert_eq!(detect(b"item;cost\nfuel;84,20\n"), Separator::Semicolon);
        assert_eq!(detect(b"item\tcost\nfuel\t84.20\n"), Separator::Tab);
        assert_eq!(detect(b"item|cost\nfuel|84.20\n"), Separator::Pipe);
        assert_eq!(detect(b"seed:basil|leek|kale,g:num\nleek,3\n"), Separator::Comma);
        assert_eq!(detect(b"a;b;c\n\n1;2;3\r\n"), Separator::Semicolon);
        assert_eq!(detect(b"\"a;b\",c\n\"1;2\",3\n"), Separator::Comma);
        assert_eq!(detect(b"one column\nno separator\n"), Separator::Comma);
        assert_eq!(detect(b""), Separator::Comma);
        // One line: the one it holds most.
        assert_eq!(detect(b"a;b;c,d"), Separator::Semicolon);
        // A tie: the comma first.
        assert_eq!(detect(b"a,b;c\n1,2;3\n"), Separator::Comma);
    }
}
