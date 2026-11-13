// How a file's values look (SPEC.md, Choices and Looks): a choice's word's tint, and what a
// `look` gives a column's cells, a named type's, every row: the text a cell is drawn as
// (grouping, decimals, parentheses, a date's form) and the tint it is drawn in. The core
// decides every bit of it, in a locale the host hands it; here it is read once per version
// of the doc and held, so a table draws a frame without asking the core again.
//
// Presentation alone: a value, a sum, a query never sees any of it.

import CellsCore
import Foundation

extension Cells {
    /// One of the format's ten tints, drawn in the host's theme's own colour, or an exact
    /// colour, the same everywhere.
    public enum Tint: Sendable, Hashable {
        case named(Name)
        /// `0xRRGGBB`.
        case hex(UInt32)

        /// The ten, in the core's order (`cells::look::TINTS`).
        public enum Name: Int, Sendable, CaseIterable {
            case red, orange, yellow, green, teal, blue, purple, pink, brown, gray
        }

        /// As the file spells it: `green`, `#3fa34d`.
        public var spelled: String {
            switch self {
            case .named(let n): "\(n)"
            case .hex(let rgb): "#" + String(format: "%06x", rgb)
            }
        }

        /// The core's tint; nil for none.
        init?(_ t: CellsTint) {
            switch t.kind {
            case 0: return nil
            case 1:
                guard let n = Name(rawValue: Int(t.value)) else {
                    preconditionFailure("Cells: the core names tint \(t.value), past the ten")
                }
                self = .named(n)
            case 2: self = .hex(t.value)
            default: preconditionFailure("Cells: the core names a tint of kind \(t.kind)")
            }
        }
    }

    /// What a value is drawn in: one tint, or a point between two, a scale's, `at` from 0
    /// (`from`) to 1000 (`to`), for the host to mix in its theme.
    public enum Tinted: Sendable, Hashable {
        case one(Tint)
        case between(from: Tint, to: Tint, at: Int)

        init?(_ t: CellsTinted) {
            guard let from = Tint(t.from) else { return nil }
            if let to = Tint(t.to) {
                self = .between(from: from, to: to, at: Int(t.at))
            } else {
                self = .one(from)
            }
        }
    }

    /// How values read in a place: its grouping and decimal marks, its months' and
    /// weekdays' names, its words for before and after noon, as the core takes them
    /// (`cells::Locale::lines`).
    public struct Locale: Sendable, Hashable {
        /// The 42 lines the core reads; empty for English.
        let lines: String

        /// English, as the suite reads: `,` grouping, `.` for the point.
        public static let english = Locale(lines: "")

        /// The person's own, as the Mac says it.
        public static var current: Locale { Locale(Foundation.Locale.current) }

        public init(_ locale: Foundation.Locale) {
            let numbers = NumberFormatter()
            numbers.locale = locale
            let dates = DateFormatter()
            dates.locale = locale
            // the core's weeks start on Monday; Foundation's on Sunday
            func monday(_ days: [String]) -> [String] { Array(days[1...] + days[..<1]) }
            var lines: [String] = [
                numbers.groupingSeparator ?? ",", numbers.decimalSeparator ?? ".",
            ]
            lines += dates.monthSymbols
            lines += dates.shortMonthSymbols
            lines += monday(dates.weekdaySymbols)
            lines += monday(dates.shortWeekdaySymbols)
            lines += [dates.amSymbol, dates.pmSymbol]
            precondition(lines.count == 42, "Cells: a locale is 42 lines, this one \(lines.count)")
            precondition(
                !lines.contains { $0.contains("\n") }, "Cells: a locale's words are one line each")
            self.lines = lines.joined(separator: "\n")
        }

        private init(lines: String) { self.lines = lines }
    }

    /// How a file's values look in a locale, read once per version: for each column a look
    /// gives rules, every cell as drawn and its tint; for each choice column, its words'
    /// tints; and every row's tint when a look tints rows.
    public struct Looks: Sendable {
        /// By column, each row's text as drawn: only the columns a look gives rules.
        public let drawn: [Int: [String]]
        /// By column, each row's tint: the columns a look gives rules.
        public let tinted: [Int: [Tinted?]]
        /// By column, each tinted word of its choice and its tint.
        public let words: [Int: [String: Tint]]
        /// Each row's tint, by `look row:`; empty when no look tints rows.
        public let rows: [Tint?]

        static let none = Looks(drawn: [:], tinted: [:], words: [:], rows: [])

        /// The cell's text as drawn, nil when its column has no look (drawn as it reads).
        public func drawn(row: Int, column: Int) -> String? { drawn[column]?[row] }

        /// What the cell is drawn in: its column's look's, else its word's tint.
        public func tint(row: Int, column: Int, word: String?) -> Tinted? {
            if let t = tinted[column] { return t[row] }
            return word.flatMap { words[column]?[$0] }.map(Tinted.one)
        }

        /// The row's tint, nil when no look tints it.
        public func tint(row: Int) -> Tint? { rows.isEmpty ? nil : rows[row] }
    }

    /// A `look` statement as the engine read it: what it is for, and its rules that read.
    public struct LookStatement: Sendable, Hashable {
        /// A look is for a column or a named type, by its name, or for every row.
        public enum Target: Sendable, Hashable {
            case row
            case name(String)
        }

        public let target: Target
        /// Its rules that read, as written (`grouped`, `negative red`), in order.
        public let rules: [String]
    }
}

extension Cells.Document {
    /// How the file's values look in `locale` (the person's own unless asked), read once
    /// per version of the doc.
    public func looks(in locale: Cells.Locale = .current) -> Cells.Looks {
        engine.looks(locale)
    }

    /// The rules a look gives `column`, as written, its named types' first.
    public func looks(of column: Int) -> [String] {
        engine.with { d in
            let s = string(cells_looks_of(d, column))
            return s.isEmpty ? [] : s.components(separatedBy: "\n")
        } ?? []
    }

    /// Each look statement of the prelude that holds a rule that reads, in the file's order:
    /// what it is for and those rules, as written. A rule that does not read is the file's
    /// problem, and not here.
    public var lookStatements: [Cells.LookStatement] {
        engine.with { d in
            (0..<cells_look_count(d)).map { i in
                let target = string(cells_look_target(d, i))
                return Cells.LookStatement(
                    target: target.isEmpty ? .row : .name(target),
                    rules: string(cells_look_rules(d, i)).components(separatedBy: "\n"))
            }
        } ?? []
    }

    /// The tint `word` has in `column`'s choice: what a chip or a picker of the word shows
    /// where no cell holds it.
    public func tint(of word: String, in column: Int) -> Cells.Tint? {
        self.type(of: column).tints[word]
    }
}

extension Engine {
    /// What the doc's looks give, asked of the core once per version and locale.
    static func looks(_ d: OpaquePointer, _ said: Read, _ locale: Cells.Locale) -> Cells.Looks {
        let rows = Int(cells_rows(d))
        var drawn: [Int: [String]] = [:]
        var tinted: [Int: [Cells.Tinted?]] = [:]
        var words: [Int: [String: Cells.Tint]] = [:]
        var lines = locale.lines
        lines.withUTF8 { place in
            let at = UnsafeRawPointer(place.baseAddress)?.assumingMemoryBound(to: CChar.self)
            for c in 0..<said.columns.count {
                let looked = !string(cells_looks_of(d, c)).isEmpty
                if looked {
                    drawn[c] = (0..<rows).map { r in string(cells_drawn(d, r, c, at, place.count)) }
                    tinted[c] = (0..<rows).map { r in Cells.Tinted(cells_tint(d, r, c)) }
                }
                if let tints = (said.columns[c].type ?? said.inferred[c])?.tints, !tints.isEmpty {
                    words[c] = tints
                }
            }
        }
        // only a look on `row` tints rows; reading every row's tint for none is waste
        let rowsTinted = said.statements.contains { s in
            if case .look(let target, _) = s.kind { target == "row" } else { false }
        }
        let rowTints = rowsTinted ? (0..<rows).map { Cells.Tint(cells_row_tint(d, $0)) } : []
        return Cells.Looks(drawn: drawn, tinted: tinted, words: words, rows: rowTints)
    }
}
