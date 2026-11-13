// A file's text as the engine's spans: what each part of it is, the fences, the prelude's
// statements and comments, every header cell's name and its type's parts, every cell by its
// column. What `Syntax` colours a `.cells` file by, and what a language server's semantic
// tokens carry. Read from the text alone, never refused: a broken file has its spans too,
// a part with no meaning marked `.error`.

import CellsCore

extension Cells {
    public struct Token: Sendable, Equatable {
        /// The engine's kinds, in its order (`cells::tokens::Kind`).
        public enum Kind: Sendable, Equatable, CaseIterable {
            /// A prelude's `---`, and a fence's backticks with its language.
            case fence
            /// `// …` to the end of a prelude line.
            case comment
            /// `type`, `export`, `import`, `from`, `with`, `run` and a schedule's words; a
            /// formula's `and`, `case`, `null`, and its functions.
            case keyword
            /// A column's name, and a table's where the prelude names it.
            case name
            /// A base, a currency, a named type, `one of`, `bool`.
            case type
            /// `!`, `?`, `key`, `optional`.
            case mark
            /// `>=`, `..`, `min`, `between`, `and`, and the bounds' numbers.
            case bound
            /// A date or time's format, parentheses included.
            case format
            /// A choice's word, or a boolean pair's.
            case choice
            /// `:` after a name, `|` and `/` in a type, `=` before a formula or in a
            /// statement, a record's commas, a quoted header cell's quotes, a cell
            /// formula's backticks.
            case punct
            /// A number in a formula or a schedule.
            case number
            /// A column or a table a formula names, a schedule's zone or path.
            case reference
            /// `+ - * / || = < ( ) [ ] . ,` in a formula.
            case `operator`
            case cellText, cellNumber, cellBool, cellDate, cellChoice, cellLink, cellId
            /// A part that has no meaning where it stands.
            case error
            /// A quoted string: where an import comes from, an attribute's value, a
            /// formula's text.
            case string
            /// A script's code, inside its fence.
            case code
            /// A table's name where a statement names it or a formula reads it.
            case table
            /// A tint: after a choice's `#` or `tint`, and in a look's rules.
            case tint
        }

        /// The span, in UTF-8 bytes of the text.
        public let utf8: Range<Int>
        public let kind: Kind
        /// The column a header cell's parts, a cell and a record's comma before it belong
        /// to.
        public let column: Int?
        /// The column a formula's name reads, so it wears that column's colour.
        public let reads: Int?
    }

    /// The text's spans, in order, none overlapping; what lies between them (spaces, line
    /// breaks) is nothing.
    public static func tokens(_ text: String) -> [Token] {
        var text = text
        return text.withUTF8 { utf8 in
            let spans = cells_tokens(
                UnsafeRawPointer(utf8.baseAddress)?.assumingMemoryBound(to: CChar.self), utf8.count)
            defer { cells_tokens_free(spans) }
            return UnsafeBufferPointer(start: spans.tokens, count: spans.count).map { t in
                Token(
                    utf8: Int(t.start)..<Int(t.end),
                    kind: Token.Kind.allCases[Int(t.kind.rawValue)],
                    column: t.column == .max ? nil : Int(t.column),
                    reads: t.reads == .max ? nil : Int(t.reads))
            }
        }
    }
}
