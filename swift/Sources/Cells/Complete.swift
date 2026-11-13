// What may be written at a place of a `.cells` file's text, as the engine finds it: the
// completions `cells lsp` gives an editor (a cell's values, a type's names, a formula's
// columns, tables, rows and functions, the prelude's statements), for a field or an
// editor of the host's. Offsets are UTF-16, as text views count them; the engine's bytes
// are converted here.

import CellsCore

extension Cells {
    /// Something that may be written at a place: `insert` in place of `range` (the word
    /// being typed), shown as `label` with its `detail`.
    public struct Completion: Sendable, Equatable, Hashable {
        public enum Kind: Sendable, Equatable, Hashable {
            /// A row's key, a yes or a no.
            case value
            /// One of a choice's words.
            case word
            /// A named type.
            case type
            /// A table's key column, `accounts.account`.
            case reference
            /// The language's or the prelude's word, a base type.
            case keyword
            /// A column.
            case column
            /// A table.
            case table
            case function
            /// A tint, after a choice's `#` or in a look's rule.
            case tint
        }

        public let label: String
        public let kind: Kind
        public let detail: String
        public let insert: String
        /// What it replaces, in UTF-16 offsets of the text.
        public let range: Range<Int>
        /// What it is matched by (`#3 card`), else its label.
        public let filter: String
        /// Where it sorts, else by its label.
        public let sort: String
    }
}

extension Cells {
    /// A place in a text as a text view counts it (a UTF-16 offset) and as the engine does
    /// (a byte), and the engine's bytes near it brought back to UTF-16 offsets.
    struct TextPlace {
        let at: Int
        let byte: Int
        let utf8: [UInt8]

        init(_ text: String, at: Int) {
            let utf16 = text.utf16
            precondition(
                at >= 0 && at <= utf16.count, "Cells: no place \(at) in a text of \(utf16.count)")
            let index = utf16.index(utf16.startIndex, offsetBy: at)
            guard let scalar = index.samePosition(in: text.unicodeScalars) else {
                preconditionFailure("Cells: place \(at) splits a character")
            }
            self.at = at
            byte = text.utf8.distance(
                from: text.utf8.startIndex, to: scalar.samePosition(in: text.utf8)!)
            utf8 = Array(text.utf8)
        }

        /// A byte offset the engine names (near the place: the word there), as a UTF-16
        /// offset of the same text, counted from the place.
        func offset(_ b: UInt32) -> Int {
            let b = Int(b)
            precondition(b <= utf8.count, "Cells: the engine names byte \(b) of \(utf8.count)")
            return b <= byte
                ? at - String(decoding: utf8[b..<byte], as: UTF8.self).utf16.count
                : at + String(decoding: utf8[byte..<b], as: UTF8.self).utf16.count
        }

        /// A call given the text's bytes and their count.
        func call<T>(_ body: (UnsafePointer<CChar>, Int) -> T) -> T {
            utf8.withUnsafeBufferPointer { raw in
                raw.baseAddress!.withMemoryRebound(to: CChar.self, capacity: max(raw.count, 1)) {
                    body($0, raw.count)
                }
            }
        }
    }
}

extension Cells.Document {
    /// What may be written at `at` (a UTF-16 offset) of `text`, the text this document was
    /// read from: the tables it names are the ones it holds, so a document read with its
    /// folder completes their keys and columns.
    public func completions(in text: String, at: Int) -> [Cells.Completion] {
        let place = Cells.TextPlace(text, at: at)
        return engine.with { d in
            let count = place.call { cells_complete(d, $0, $1, place.byte) }
            return (0..<count).map { i in
                let c = cells_completion(d, i)
                func part(_ p: CellsCompletionPart) -> String {
                    string(cells_completion_part(d, i, p))
                }
                let kind: Cells.Completion.Kind =
                    switch c.kind {
                    case CellsCompletionValue: .value
                    case CellsCompletionEnumMember: .word
                    case CellsCompletionClass: .type
                    case CellsCompletionReference: .reference
                    case CellsCompletionKeyword: .keyword
                    case CellsCompletionField: .column
                    case CellsCompletionModule: .table
                    case CellsCompletionFunction: .function
                    case CellsCompletionColor: .tint
                    default:
                        preconditionFailure(
                            "Cells: the core names a completion kind \(c.kind.rawValue) this binding does not know"
                        )
                    }
                let label = part(CellsCompletionLabel)
                return Cells.Completion(
                    label: label, kind: kind, detail: part(CellsCompletionDetail),
                    insert: part(CellsCompletionInsert),
                    range: place.offset(c.from)..<place.offset(c.to),
                    filter: { $0.isEmpty ? label : $0 }(part(CellsCompletionFilter)),
                    sort: { $0.isEmpty ? label : $0 }(part(CellsCompletionSort)))
            }
        } ?? []
    }
}
