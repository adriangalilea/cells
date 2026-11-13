// A `.cells` file's prelude as the engine reads it: its statements, each what it is and
// where it stands, and its edits as the file's text for the host to read again (so an
// import a new line names is followed by whoever can reach it). Nothing here parses the
// prelude's text: the engine reads every statement and writes every line.

import CellsCore

extension Cells {
    /// A statement of the prelude that reads.
    public struct Statement: Sendable, Equatable, Identifiable {
        public enum Kind: Sendable, Equatable {
            /// `type Money = EUR>=0`: the type as written after its `=`.
            case type(name: String, spelled: String)
            /// `import accounts from "accounts.cells"` (`name`), or `import { Kind, rates }
            /// from "…"` (`names`); its `sha256` when pinned.
            case `import`(name: String?, names: [String], from: String, sha256: String?)
            /// `fees = ```cells`: a table written inline, its rows as written.
            case table(name: String, rows: String)
            /// `rates = run every day at 18:00 in Europe/Madrid`: its schedule as written, its
            /// script, and the rows its last run stored.
            case run(name: String, schedule: String, language: String, code: String, rows: String)
            /// `total: EUR = cost * qty`: a column of the file's table, what follows its `:`
            /// as a header cell spells it (`EUR=cost * qty`).
            case column(name: String, spelled: String)
            /// `look amount: grouped, negative red`: how a column's, a named type's or every
            /// row's (`row`) values show, its rules as the core writes them, joined by `, `.
            /// What a rule does is the core's (`Document.looks`), never read from these words.
            case look(target: String, rules: String)
            /// ```` ```chart bars ````: a chart, named only when another reads it, its kind
            /// as its fence's label (empty when read from its entries), its entries as
            /// written. What it draws is the core's (`Document.charts`, `summary`).
            case chart(name: String?, kind: String, entries: String)
        }

        /// Its place among the statements.
        public let id: Int
        public let kind: Kind
        public let exported: Bool
        /// Its prelude lines, 0-based inside the fences.
        public let lines: Range<Int>
        /// A script's code between its fences, as prelude lines: what an edit of the code
        /// alone replaces.
        public let codeLines: Range<Int>?
        /// A table's rows between their fences, a script's stored rows.
        public let rowsLines: Range<Int>?
        /// As written, its comments included.
        public let text: String

        /// The name it gives; nil for an import that names what it takes, and for a look,
        /// which gives none (it names what it looks at).
        public var name: String? {
            switch kind {
            case .type(let n, _), .table(let n, _), .run(let n, _, _, _, _), .column(let n, _): n
            case .import(let n, _, _, _): n
            case .chart(let n, _, _): n
            case .look: nil
            }
        }
    }

    /// A one-line statement, written by the engine as a writer writes it.
    public enum Line: Sendable, Equatable {
        /// `type Money = EUR>=0`, `export type …` when exported.
        case type(String, spelled: String, exported: Bool = false)
        /// `import accounts from "accounts.cells"`.
        case `import`(String, from: String)
        /// `total: EUR = cost * qty`, from what a header cell spells after its `:`.
        case column(String, spelled: String)
        /// A script's first line, `rates = run every hour`: its fences follow it.
        case run(String, schedule: String, exported: Bool = false)
        /// `look amount: grouped, negative red`, a column's or a named type's look (`row`
        /// for every row's), its rules joined as a person writes them.
        case look(String, rules: String)
    }

    /// A change to the prelude: the file's text is the host's to read again.
    public enum PreludeEdit: Sendable, Equatable {
        /// Prelude lines (a statement's, a script's code alone) written as `text` (lines
        /// joined by `\n`; empty takes them out).
        case replace(Range<Int>, with: String)
        /// `text` written after the last statement, a prelude opened for it if none is.
        case add(String)

        /// The statement taken out.
        public static func remove(_ s: Statement) -> PreludeEdit { .replace(s.lines, with: "") }
    }
}

extension Cells.Document {
    /// The prelude's statements that read, in order; empty for a plain file or one
    /// without a prelude. A statement that does not read is in `broken`.
    public var statements: [Cells.Statement] { engine.said.statements }

    /// The file's text with its prelude edited, every other byte as it is now: read it
    /// again (`init(reading:at:)`) so what a new line imports is followed.
    public func text(_ edit: Cells.PreludeEdit) -> String {
        precondition(typed, "Cells: a plain file has no prelude")
        // the engine's lines are 1-based inside the fences, 0 for "after the last"
        let (from, to, text): (UInt32, UInt32, String) =
            switch edit {
            case .replace(let lines, let text):
                (UInt32(lines.lowerBound + 1), UInt32(lines.upperBound + 1), text)
            case .add(let text): (0, 0, text)
            }
        precondition(!text.contains("\r"), "Cells: a prelude edit's lines are joined by \\n")
        return engine.with { d in
            lending([text]) { s in
                string(cells_prelude_edited(d, from, to, s[0].ptr, s[0].len))
            }
        }!
    }

    /// A one-line statement as the engine writes it.
    public func written(_ line: Cells.Line) -> String {
        let (kind, exported, name, said) =
            switch line {
            case .type(let n, let s, let e): (CellsStatementType, e, n, s)
            case .import(let n, let f): (CellsStatementImport, false, n, f)
            case .column(let n, let s): (CellsStatementColumn, false, n, s)
            case .run(let n, let s, let e): (CellsStatementRun, e, n, s)
            case .look(let n, let r): (CellsStatementLook, false, n, r)
            }
        return engine.with { d in
            lending([name, said]) { s in
                string(cells_prelude_line(d, kind, exported, s[0], s[1]))
            }
        }!
    }
}

extension Engine {
    /// The doc's statements, read once per version.
    static func statements(_ d: OpaquePointer) -> [Cells.Statement] {
        (0..<cells_statement_count(d)).map { i in
            let s = cells_statement(d, i)
            func part(_ p: CellsStatementPart, _ word: Int = 0) -> String {
                string(cells_statement_part(d, i, p, word))
            }
            let kind: Cells.Statement.Kind =
                switch s.kind {
                case CellsStatementType:
                    .type(name: part(CellsStatementName), spelled: part(CellsStatementSpelled))
                case CellsStatementImport:
                    .import(
                        name: s.words == 0 ? part(CellsStatementName) : nil,
                        names: (0..<s.words).map { part(CellsStatementWord, $0) },
                        from: part(CellsStatementFrom),
                        sha256: { $0.isEmpty ? nil : $0 }(part(CellsStatementHash)))
                case CellsStatementTable:
                    .table(name: part(CellsStatementName), rows: part(CellsStatementRows))
                case CellsStatementRun:
                    .run(
                        name: part(CellsStatementName), schedule: part(CellsStatementSchedule),
                        language: part(CellsStatementLanguage), code: part(CellsStatementCode),
                        rows: part(CellsStatementRows))
                case CellsStatementColumn:
                    .column(name: part(CellsStatementName), spelled: part(CellsStatementSpelled))
                case CellsStatementLook:
                    .look(target: part(CellsStatementName), rules: part(CellsStatementSpelled))
                case CellsStatementChart:
                    .chart(
                        name: { $0.isEmpty ? nil : $0 }(part(CellsStatementName)),
                        kind: part(CellsStatementSpelled), entries: part(CellsStatementRows))
                default:
                    preconditionFailure(
                        "Cells: the core names a statement kind \(s.kind.rawValue) this binding does not know"
                    )
                }
            func lines(_ from: UInt32, _ to: UInt32) -> Range<Int>? {
                from == 0 ? nil : Int(from) - 1..<Int(to) - 1
            }
            return Cells.Statement(
                id: i, kind: kind, exported: s.exported, lines: lines(s.line, s.end)!,
                codeLines: lines(s.code_line, s.code_end),
                rowsLines: lines(s.rows_line, s.rows_end),
                text: part(CellsStatementText))
        }
    }
}
