// What is said of a place of a `.cells` file's text, as the engine says it: the hover
// `cells lsp` gives an editor (a column, a cell's value and what is wrong with it, a
// formula's name or function, a named type, a table), in plain lines for a host's
// editor. Offsets are UTF-16, as text views count them.

import CellsCore

extension Cells {
    /// What is said of a place: its lines (a paragraph a line), and the part of the text
    /// it speaks of.
    public struct Hover: Sendable, Equatable, Hashable {
        public let text: String
        /// What it speaks of, in UTF-16 offsets of the text.
        public let range: Range<Int>
    }
}

extension Cells.Document {
    /// What is said of `at` (a UTF-16 offset) of `text`, the text this document was read
    /// from; nothing where nothing is. The tables it names are the ones it holds.
    public func hover(in text: String, at: Int) -> Cells.Hover? {
        let place = Cells.TextPlace(text, at: at)
        return engine.with { d in
            guard place.call({ cells_hover(d, $0, $1, place.byte) }) else { return nil }
            let h = cells_hovered(d)
            return Cells.Hover(
                text: string(cells_hover_text(d)),
                range: place.offset(h.from)..<place.offset(h.to))
        } ?? nil
    }
}
