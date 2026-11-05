//! A file's text by lines: a byte offset as the protocol's position (a line, and UTF-16
//! units into it, its default), and back.

use tower_lsp::lsp_types::{Position, Range};

use cells::Span;

pub struct Lines<'a> {
    text: &'a str,
    /// Where each line starts.
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    pub fn new(text: &'a str) -> Lines<'a> {
        let starts = std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
        Lines { text, starts }
    }

    pub fn position(&self, at: usize) -> Position {
        assert!(at <= self.text.len(), "offset {at} is past the text's {} bytes", self.text.len());
        let line = self.starts.partition_point(|&s| s <= at) - 1;
        let units = self.text[self.starts[line]..at].encode_utf16().count();
        Position { line: line as u32, character: units as u32 }
    }

    pub fn range(&self, s: Span) -> Range {
        Range { start: self.position(s.start as usize), end: self.position((s.start + s.len) as usize) }
    }

    /// The byte a position names: past a line's end, the line's end; past the last line,
    /// the text's end.
    pub fn offset(&self, p: Position) -> usize {
        let Some(&start) = self.starts.get(p.line as usize) else { return self.text.len() };
        let end = self.starts.get(p.line as usize + 1).map_or(self.text.len(), |&e| e);
        let line = &self.text[start..end];
        let mut units = 0;
        for (i, c) in line.char_indices() {
            if units >= p.character as usize || c == '\n' || c == '\r' {
                return start + i;
            }
            units += c.len_utf16();
        }
        end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_count_utf16() {
        let text = "a,é\n𝄞b,c\n";
        let l = Lines::new(text);
        assert_eq!(l.position(4), Position { line: 0, character: 3 });
        let b = text.find('b').unwrap();
        assert_eq!(l.position(b), Position { line: 1, character: 2 });
        assert_eq!(l.offset(Position { line: 1, character: 2 }), b);
        assert_eq!(l.offset(Position { line: 0, character: 99 }), 4);
    }
}
