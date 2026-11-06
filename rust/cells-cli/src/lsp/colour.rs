//! Semantic tokens: the engine's reading of a file (`cells::tokens`) in the protocol's
//! terms. A part with a standard type (a keyword, a type, a number, an operator) is
//! served as it, so any theme colours it. A part coloured by its column (a header's
//! name, a cell, a formula's name that reads a column) is `column` or `cell`, types no
//! theme has a rule for, so the editor's grammar keeps colouring it, and carries its
//! column as a modifier, `column1` to `column6` in turn, for a client that colours
//! columns from the server (as Rainbow CSV's extension does). Punctuation, errors and a
//! script's code are not served: the grammar draws the first, diagnostics the second,
//! the script's own language the third.

use cells::tokens::{Kind, Token};
use tower_lsp::lsp_types::{
    InlayHint, InlayHintLabel, Range, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensLegend,
};

use super::text::Lines;

/// The columns a rainbow cycles through, as the grammars do.
const COLUMNS: u32 = 6;

const TYPES: [&str; 13] = [
    "comment", "keyword", "type", "modifier", "operator", "string", "enumMember", "number", "variable", "column", "cell", "macro",
    "namespace",
];

fn of(name: &str) -> u32 {
    TYPES.iter().position(|t| *t == name).unwrap_or_else(|| panic!("{name} is not in the legend")) as u32
}

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TYPES.iter().map(|t| SemanticTokenType::new(t)).collect(),
        token_modifiers: (1..=COLUMNS).map(|n| SemanticTokenModifier::new(Box::leak(format!("column{n}").into_boxed_str()))).collect(),
    }
}

/// A token's type in the legend, and the column it is coloured by; none for a part not
/// served.
fn served(t: &Token) -> Option<(u32, Option<u32>)> {
    let column = (t.column != u32::MAX).then_some(t.column);
    Some(match t.kind {
        Kind::Comment => (of("comment"), None),
        Kind::Keyword => (of("keyword"), None),
        Kind::Type => (of("type"), None),
        Kind::Mark => (of("modifier"), None),
        Kind::Bound | Kind::Operator => (of("operator"), None),
        Kind::Format | Kind::String => (of("string"), None),
        Kind::Choice => (of("enumMember"), None),
        Kind::Number => (of("number"), None),
        Kind::Fence => (of("macro"), None),
        Kind::Reference if t.reads != u32::MAX => (of("column"), Some(t.reads)),
        Kind::Reference => (of("variable"), None),
        // A table, where the prelude names it (a name with no column) and where a formula
        // reads it.
        Kind::Table => (of("namespace"), None),
        Kind::Name => match column {
            Some(c) => (of("column"), Some(c)),
            None => (of("namespace"), None),
        },
        Kind::CellText
        | Kind::CellNumber
        | Kind::CellBool
        | Kind::CellDate
        | Kind::CellChoice
        | Kind::CellLink
        | Kind::CellId => (of("cell"), column),
        Kind::Punct | Kind::Error | Kind::Code => return None,
    })
}

/// The file's tokens, encoded as the protocol sends them: each relative to the one before,
/// a part across a line break sent a line at a time.
pub fn semantic_tokens(text: &str) -> Vec<SemanticToken> {
    let lines = Lines::new(text);
    let mut out = Vec::new();
    let (mut line, mut start) = (0u32, 0u32);
    for t in cells::tokens::tokens(text) {
        let Some((token_type, column)) = served(&t) else { continue };
        let modifiers = column.map_or(0, |c| 1 << (c % COLUMNS));
        let (s, e) = (t.start as usize, t.end as usize);
        let mut from = s;
        for piece in text[s..e].split_inclusive('\n') {
            let to = from + piece.trim_end_matches(['\n', '\r']).len();
            if to > from {
                let p = lines.position(from);
                let length = text[from..to].encode_utf16().count() as u32;
                let delta_line = p.line - line;
                let delta_start = if delta_line == 0 { p.character - start } else { p.character };
                out.push(SemanticToken { delta_line, delta_start, length, token_type, token_modifiers_bitset: modifiers });
                (line, start) = (p.line, p.character);
            }
            from += piece.len();
        }
    }
    out
}

/// The file's tables aligned, as inlay hints: before each value the spaces that bring it
/// to its column's start where the file has fewer (`cells::layout`), drawn by the editor
/// and never written, so a file not formatted shows what `cells fmt` would write. Those
/// in `range` alone. No value is folded: an editor shows the whole text.
pub fn aligned(text: &str, range: Range) -> Vec<InlayHint> {
    let lines = Lines::new(text);
    let (pads, _) = cells::layout::layout(text, None);
    pads.into_iter()
        .map(|p| (lines.position(p.at as usize), p.width))
        .filter(|(at, _)| range.start <= *at && *at <= range.end)
        .map(|(position, width)| InlayHint {
            position,
            label: InlayHintLabel::String(" ".repeat(width as usize)),
            kind: None,
            text_edits: None,
            tooltip: None,
            padding_left: None,
            padding_right: None,
            data: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each token back in absolute terms: its line, its start, its text, its type, its
    /// column modifier.
    fn absolute(text: &str) -> Vec<(u32, u32, String, &'static str, u32)> {
        let lines: Vec<&str> = text.split('\n').collect();
        let (mut line, mut start) = (0, 0);
        semantic_tokens(text)
            .into_iter()
            .map(|t| {
                if t.delta_line > 0 {
                    start = 0;
                }
                line += t.delta_line;
                start += t.delta_start;
                let units: Vec<u16> = lines[line as usize].encode_utf16().collect();
                let spelled = String::from_utf16(&units[start as usize..(start + t.length) as usize]).unwrap();
                (line, start, spelled, TYPES[t.token_type as usize], t.token_modifiers_bitset)
            })
            .collect()
    }

    #[test]
    fn columns_as_modifiers_and_names_as_what_they_read() {
        let t = absolute("item:text!,cost:EUR,total:EUR=cost * 2\nfuel,3,6=\n");
        let find = |s: &str| t.iter().filter(|x| x.2 == s).map(|x| (x.3, x.4)).collect::<Vec<_>>();
        assert_eq!(find("item"), [("column", 1)]);
        assert_eq!(find("cost"), [("column", 2), ("column", 2)], "the header's name, and the formula's that reads it");
        assert_eq!(find("total"), [("column", 4)]);
        assert_eq!(find("fuel"), [("cell", 1)]);
        assert_eq!(find("6"), [("cell", 4)]);
        assert_eq!(find("text"), [("type", 0)]);
        assert_eq!(find("*"), [("operator", 0)]);
        assert!(find(",").is_empty() && find("=").is_empty(), "punctuation is the grammar's");
    }

    #[test]
    fn columns_aligned_by_hints_in_the_range_asked() {
        use tower_lsp::lsp_types::Position;
        let text = "item,cost\nfuel,1\nsplit dinner,2\n";
        let all = Range { start: Position { line: 0, character: 0 }, end: Position { line: 3, character: 0 } };
        let hints: Vec<(u32, u32, usize)> = aligned(text, all)
            .into_iter()
            .map(|h| match h.label {
                InlayHintLabel::String(s) => (h.position.line, h.position.character, s.len()),
                InlayHintLabel::LabelParts(_) => unreachable!(),
            })
            .collect();
        // `split dinner` is 12, with its comma and a space 14, its stop 16: each value of the
        // second column starts there.
        assert_eq!(hints, [(0, 5, 11), (1, 5, 11), (2, 13, 3)]);
        let first = Range { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 9 } };
        assert_eq!(aligned(text, first).len(), 1, "only the range asked");
    }

    #[test]
    fn a_cell_across_a_line_break_is_sent_a_line_at_a_time() {
        let t = absolute("a,b\n\"x\ny\",2\n");
        assert!(t.contains(&(1, 0, "\"x".into(), "cell", 1)), "{t:?}");
        assert!(t.contains(&(2, 0, "y\"".into(), "cell", 1)), "{t:?}");
    }
}
