// A column's type, read from its header cell, `name: type`, by the engine: the one parser
// of the header language. Every mark has a word and the symbol is its shorthand, so a
// person writes whichever reads better and both mean the same:
//
//   item: text!                item: text key         the key: present and unique
//   note: text?                note: text optional    may be blank
//   category: travel|stay      category: one of travel stay
//   paid: yes/no               paid: bool yes no      true and false, as the file writes them
//   cost: EUR>=0               cost: EUR min 0        money in a currency, at least 0
//   pct: 0..100                pct: num between 0 and 100

import CellsCore
import Foundation

extension Cells {
    /// What a column holds.
    public indirect enum Base: Sendable, Equatable {
        case text
        case int
        case num
        /// A percentage, written `12.5` or `12.5%`.
        case pct
        /// True and false, in the file's own words (`true`/`false`, `yes`/`no`).
        case bool(true: String, false: String)
        /// A date, ISO (`YYYY-MM-DD`) unless a format is given.
        case date(format: String?)
        /// A time of day, `HH:mm` or `HH:mm:ss` unless a format is given.
        case time(format: String?)
        case datetime
        case url
        case email
        /// A row's identity: eight characters of Crockford's base 32, the engine's.
        case id
        /// Money in a currency, by its ISO 4217 code (`EUR`, `USD`).
        case money(currency: String)
        /// One of these words.
        case choice([String])
        /// A key of another table, `accounts.account`: each `table.column` a value may name
        /// (several joined by `|`).
        case ref([String])
        /// A type named in the prelude (`type Price = EUR>=0`), resolved.
        case named(String, Base)
    }

    /// A bound on a value: a number's or a money's size, a text's length.
    public struct Bounds: Sendable, Equatable {
        public var min: Decimal?
        public var max: Decimal?
        public var minExclusive = false
        public var maxExclusive = false

        public init(
            min: Decimal? = nil, max: Decimal? = nil, minExclusive: Bool = false,
            maxExclusive: Bool = false
        ) {
            self.min = min
            self.max = max
            self.minExclusive = minExclusive
            self.maxExclusive = maxExclusive
        }
    }

    /// A column's type as its header declares it.
    public struct ColumnType: Sendable, Equatable {
        public var base: Base
        public var bounds: Bounds?
        /// A blank cell is allowed (missing), where otherwise it is a problem.
        public var optional = false
        /// The column identifies the row: present and unique.
        public var key = false
        /// A choice's words that have a tint (`income#green`), each its tint; a named
        /// type's come from its definition.
        public var tints: [String: Tint]

        public init(
            base: Base, bounds: Bounds? = nil, optional: Bool = false, key: Bool = false,
            tints: [String: Tint] = [:]
        ) {
            self.base = base
            self.bounds = bounds
            self.optional = optional
            self.key = key
            self.tints = tints
        }
    }

    /// Why a type could not be read: the text, and what in it is wrong.
    public struct TypeError: Error, Sendable, Equatable, CustomStringConvertible {
        public let type: String
        public let reason: String
        public var description: String { "\"\(type)\": \(reason)" }
    }

    /// A type's text read into a type, named types resolved from `names`.
    public static func parseType(_ raw: String, names: [String: ColumnType] = [:])
        throws(TypeError) -> ColumnType
    {
        try readType(raw, prelude: prelude(names))
    }

    /// A type's text read by the engine under `prelude` (the bytes before a header).
    static func readType(_ raw: String, prelude: String) throws(TypeError) -> ColumnType {
        let doc = Document(reading: prelude + "x:" + raw + "\n")
        if let broken = doc.broken.first {
            // The type is the header's one cell; a break anywhere else is the names'.
            precondition(
                broken.at == .header(column: 0), "Cells: the named types do not read: \(broken)")
            // The words name the column, `x`, which is this call's own.
            let reason =
                broken.message.hasPrefix("x: ")
                ? String(broken.message.dropFirst(3)) : broken.message
            throw TypeError(type: raw, reason: reason)
        }
        return doc.columns[0].type!
    }

    /// A prelude declaring `names`, each after the ones it uses.
    static func prelude(_ names: [String: ColumnType]) -> String {
        guard !names.isEmpty else { return "" }
        var written: [String] = []
        func write(_ name: String) {
            guard !written.contains(name), let type = names[name] else { return }
            if case .named(let under, _) = type.base { write(under) }
            written.append(name)
        }
        names.keys.sorted().forEach(write)
        let lines = written.map { "type \($0) = \(spelling(names[$0]!, names: names))" }
        return (["---"] + lines + ["---"]).joined(separator: "\n") + "\n"
    }

    /// A type written back as its shorthand, the engine's canonical spelling: what a
    /// writer changing a type writes. A named type is its name and what the column adds
    /// to the definition `names` gives (a document's `names`); without it, every bound
    /// and mark the type holds.
    public static func shorthand(_ type: ColumnType, names given: [String: ColumnType] = [:])
        -> String
    {
        var names = given
        if case .named(let name, let resolved) = type.base, names[name] == nil {
            names[name] = ColumnType(base: resolved, tints: type.tints)
        }
        let doc = Document(reading: prelude(names) + "x:" + spelling(type, names: names) + "\n")
        precondition(doc.broken.isEmpty, "Cells: \(type) has no spelling: \(doc.broken)")
        return doc.engine.with { Engine.part($0, CellsOfColumn, 0, CellsPartShorthand) }!
    }

    /// A typed header cell as a writer writes it, from its name and what follows its `:`
    /// (`EUR=cost * qty`): `total: EUR = cost * qty`, the engine's one spelling of a column
    /// (`cells_header_cell`), the form a declaration has. Unquoted: the table quotes it.
    public static func headerCell(name: String, spelled: String) -> String {
        Document(reading: "").engine.with { d in
            lending([name, spelled]) { s in
                string(cells_header_cell(d, s[0].ptr, s[0].len, s[1].ptr, s[1].len))
            }
        }!
    }

    /// Whether the engine serves `folder` whole, every file under it, or shallow, its own
    /// files alone (the home folder, the disk's root): the engine's one rule
    /// (`cells_served_whole`), which a host lists a folder by and finds a file's books by.
    public static func servedWhole(_ folder: URL) -> Bool {
        let root = folder.standardizedFileURL.resolvingSymlinksInPath().path
        let home = FileManager.default.homeDirectoryForCurrentUser.standardizedFileURL
            .resolvingSymlinksInPath().path
        return lending([root, home]) { s in
            cells_served_whole(s[0].ptr, s[0].len, s[1].ptr, s[1].len)
        }
    }

    /// A type spelled so the engine reads it back as itself under `names`: every word
    /// quoted, every part said once, a named type's own parts left to its definition.
    /// Never shown; the engine writes the shorthand from it.
    static func spelling(_ type: ColumnType, names: [String: ColumnType]) -> String {
        var text = spelling(base: type.base, tints: type.tints)
        var under: ColumnType?
        if case .named(let name, _) = type.base { under = names[name] }
        if let b = type.bounds {
            func num(_ d: Decimal) -> String { NSDecimalNumber(decimal: d).stringValue }
            if let lo = b.min, under?.bounds?.min == nil {
                text += (b.minExclusive ? " >" : " >=") + num(lo)
            }
            if let hi = b.max, under?.bounds?.max == nil {
                text += (b.maxExclusive ? " <" : " <=") + num(hi)
            }
        }
        if type.key, under?.key != true { text += "!" }
        if type.optional, under?.optional != true { text += "?" }
        return text
    }

    /// A base spelled as `spelling` spells it: a choice's words with their `tints` (the
    /// explicit `tint green`, `tint 3fa34d`), a named type's left to its definition.
    static func spelling(base: Base, tints: [String: Tint]) -> String {
        func word(_ w: String) -> String { "'" + w.replacingOccurrences(of: "'", with: "''") + "'" }
        func tinted(_ w: String) -> String {
            switch tints[w] {
            case nil: word(w)
            case .named(let n): "\(word(w)) tint \(n)"
            case .hex(let rgb): "\(word(w)) tint " + String(format: "%06x", rgb)
            }
        }
        switch base {
        case .text: return "text"
        case .int: return "int"
        case .num: return "num"
        case .pct: return "pct"
        case .bool(let t, let f): return "bool \(word(t)) \(word(f))"
        case .date(let format): return format.map { "date(\($0))" } ?? "date"
        case .time(let format): return format.map { "time(\($0))" } ?? "time"
        case .datetime: return "datetime"
        case .url: return "url"
        case .email: return "email"
        case .id: return "id"
        case .money(let c): return c
        case .choice(let words):
            precondition(
                tints.keys.allSatisfy(words.contains),
                "Cells: tints \(tints) name words the choice \(words) does not hold")
            return "one of " + words.map(tinted).joined(separator: " ")
        case .ref(let targets): return targets.joined(separator: "|")
        case .named(let name, _): return name
        }
    }

    /// A cell's text read by a type: its value, or what is wrong with it.
    public static func decode(_ raw: String, _ type: ColumnType) -> Result<Value, Mismatch> {
        var names: [String: ColumnType] = [:]
        if case .named(let name, let resolved) = type.base {
            names[name] = ColumnType(base: resolved, tints: type.tints)
        }
        var doc = Document(reading: prelude(names) + "x:" + spelling(type, names: names) + "\n")
        precondition(doc.broken.isEmpty, "Cells: \(type) does not read: \(doc.broken)")
        doc.insertRows([[raw]], at: 0)
        return doc.value(row: 0, column: 0).mapError { Mismatch(message: $0.message) }
    }

    /// What is wrong with a cell's text for its type, in words.
    public struct Mismatch: Error, Sendable, Equatable {
        public let message: String
    }
}
