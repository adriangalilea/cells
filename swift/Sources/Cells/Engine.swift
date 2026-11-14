// The one engine, held: a doc of the cells core (Rust, behind its C ABI), owned by one
// `Engine` and read under its lock, since every call on a doc writes the text it returns
// into the doc. A Document is a value over an Engine: copied on write, so two copies never
// share an edit. What the core refuses (a broken grammar) is an Engine with no doc, its
// text and its refusal kept.

import CellsCore
import Foundation

final class Engine: @unchecked Sendable {
    /// The core's doc; nil when the file does not read.
    private let doc: OpaquePointer?
    /// What the core refused, with the text it refused: the file as it is.
    let refusal: (problem: Cells.Problem, text: String)?
    private let lock = NSLock()
    // What has been read out of the doc, kept until the doc changes.
    private var exports: [Int: Export] = [:]
    private var read: Read?
    private var looked: [Cells.Locale: Cells.Looks] = [:]

    /// What a doc says about itself, read once per version.
    struct Read {
        let columns: [Cells.Column]
        let names: [String: Cells.ColumnType]
        let inferred: [Int: Cells.ColumnType]
        let problems: [Cells.Problem]
        /// `row << 32 | column` of each cell problem that leaves the cell no value (a
        /// ragged row's does not, nor a note: a stored value standing, one not stored
        /// yet), in order, for a cell's lookup, and its index in `problems`.
        let places: [UInt64]
        let placed: [Int]
        let rows: Int
        let layout: Cells.Layout
        let statements: [Cells.Statement]
    }

    /// How the file was read: 0 for Cells, else a plain file's separator byte.
    let sep: UInt8

    private init(doc: OpaquePointer?, refusal: (Cells.Problem, String)?, sep: UInt8) {
        precondition((doc == nil) != (refusal == nil), "Cells: a doc or a refusal")
        self.doc = doc
        self.refusal = refusal
        self.sep = sep
    }

    deinit {
        if let doc { cells_free(doc) }
    }

    /// The bytes read in a dialect on every core: `sep` 0 for Cells, else a plain file's
    /// separator byte.
    static func reading(
        _ bytes: UnsafeRawBufferPointer, sep: UInt8, text: @autoclosure () -> String
    )
        -> Engine
    {
        var failure = CellsFailure()
        let doc = cells_read_as(
            bytes.baseAddress?.assumingMemoryBound(to: UInt8.self), bytes.count, sep,
            ProcessInfo.processInfo.activeProcessorCount, &failure)
        return made(doc, &failure, text: text(), sep: sep)
    }

    /// A `.cells` file's bytes read, its imports followed by `follow` (the core's
    /// `cells_read_at` or `cells_read_with`, handed the bytes and the failure to set).
    static func reading(
        _ bytes: UnsafeRawBufferPointer, text: @autoclosure () -> String,
        following follow: (UnsafePointer<UInt8>?, Int, inout CellsFailure) -> OpaquePointer?
    ) -> Engine {
        var failure = CellsFailure()
        let doc = follow(
            bytes.baseAddress?.assumingMemoryBound(to: UInt8.self), bytes.count, &failure)
        return made(doc, &failure, text: text(), sep: 0)
    }

    /// A doc the core made, or its refusal of `text`.
    static func made(_ doc: OpaquePointer?, _ failure: inout CellsFailure, text: String, sep: UInt8)
        -> Engine
    {
        if let doc { return Engine(doc: doc, refusal: nil, sep: sep) }
        defer { cells_failure_free(&failure) }
        return Engine(doc: nil, refusal: (Cells.Problem(refused: failure), text), sep: sep)
    }

    /// A call on the doc, under the lock; nil when the file did not read.
    func with<T>(_ body: (OpaquePointer) -> T) -> T? {
        guard let doc else { return nil }
        return lock.withLock { body(doc) }
    }

    /// A copy of the doc to change on its own.
    func copy() -> Engine {
        guard let doc else { return self }
        return lock.withLock { Engine(doc: cells_clone(doc), refusal: nil, sep: sep) }
    }

    /// The doc changed: what was read from it is read again when asked.
    func changed(_ body: (OpaquePointer) -> Void) {
        guard let doc else {
            preconditionFailure("Cells: a file that does not read has nothing to change")
        }
        lock.withLock {
            body(doc)
            exports = [:]
            read = nil
            looked = [:]
        }
    }

    /// How the doc's values look in `locale`, read once per version.
    func looks(_ locale: Cells.Locale) -> Cells.Looks {
        guard let doc else { return .none }
        let said = self.said
        return lock.withLock {
            if let l = looked[locale] { return l }
            let l = Engine.looks(doc, said, locale)
            looked[locale] = l
            return l
        }
    }

    /// A doc made from this one (rows rearranged, columns reshaped), or the core's refusal
    /// of it, this one untouched.
    func derived(_ body: (OpaquePointer, inout CellsFailure) -> OpaquePointer?) -> Result<
        Engine, Cells.Problem
    > {
        guard let doc else {
            preconditionFailure("Cells: a file that does not read has nothing to change")
        }
        var failure = CellsFailure()
        let made = lock.withLock { body(doc, &failure) }
        if let made { return .success(Engine(doc: made, refusal: nil, sep: sep)) }
        defer { cells_failure_free(&failure) }
        return .failure(Cells.Problem(refused: failure))
    }

    var said: Read {
        lock.withLock {
            if let read { return read }
            let fresh =
                doc.map(Engine.read)
                ?? Read(
                    columns: [], names: [:], inferred: [:],
                    problems: [refusal!.problem], places: [], placed: [], rows: 0,
                    layout: Cells.Layout(), statements: [])
            read = fresh
            return fresh
        }
    }

    /// A column's values as the core holds them, exported once per version.
    func export(_ column: Int) -> Export {
        guard let doc else {
            preconditionFailure("Cells: a file that does not read has no columns")
        }
        return lock.withLock {
            if let e = exports[column] { return e }
            let e = Export(doc, column)
            exports[column] = e
            return e
        }
    }

    // MARK: - Reading what the doc says

    private static func read(_ d: OpaquePointer) -> Read {
        var columns: [Cells.Column] = []
        var inferred: [Int: Cells.ColumnType] = [:]
        for c in 0..<cells_columns(d) {
            let shape = cells_type(d, CellsOfColumn, c)
            let name = part(d, CellsOfColumn, c, CellsPartName)
            let spelled = part(d, CellsOfColumn, c, CellsPartSpelled)
            let type = columnType(d, CellsOfColumn, c)
            if shape.inferred { inferred[c] = type }
            columns.append(
                Cells.Column(
                    name: name, type: shape.inferred ? nil : type,
                    formula: shape.formula ? part(d, CellsOfColumn, c, CellsPartFormula) : nil,
                    spelled: shape.inferred ? nil : spelled))
        }
        var names: [String: Cells.ColumnType] = [:]
        for n in 0..<cells_named_count(d) {
            names[part(d, CellsOfNamed, n, CellsPartName)] = columnType(d, CellsOfNamed, n)
        }
        var problems: [Cells.Problem] = []
        for i in 0..<cells_naming_count(d) {
            let n = cells_naming(d, i)
            let why = string(cells_naming_why(d, i))
            problems.append(
                Cells.Problem(
                    at: .header(column: Int(n.column)), kind: .name, message: why,
                    said: "the header, column \(n.column + 1): \(why)", span: nil,
                    severity: .warning))
        }
        // Where the grammar breaks, the rest read: before the cells, in the file's order.
        for i in 0..<cells_broken_count(d) {
            let b = cells_broken(d, i)
            problems.append(
                Cells.Problem(
                    grammar: b.at, Int(b.number), why: string(cells_broken_why(d, i)),
                    span: b.spanned ? Int(b.span.start)..<Int(b.span.start + b.span.len) : nil))
        }
        var places: [UInt64] = []
        var placed: [Int] = []
        let count = cells_problem_count(d)
        let spans = Array(UnsafeBufferPointer(start: cells_problem_spans(d), count: count))
        problems.reserveCapacity(problems.count + count)
        places.reserveCapacity(count)
        placed.reserveCapacity(count)
        for i in 0..<count {
            let p = cells_problem(d, i)
            let kind: Cells.Problem.Kind =
                switch p.kind {
                case CellsKindType: .type
                case CellsKindMissing: .missing
                case CellsKindKey: .key
                case CellsKindRagged: .ragged
                case CellsKindFormula: .formula
                case CellsKindReference: .reference
                case CellsKindStale: .stale
                case CellsKindUnreached: .unreached
                case CellsKindUnstored: .unstored
                default:
                    preconditionFailure(
                        "Cells: the core names a problem kind \(p.kind.rawValue) this binding does not know"
                    )
                }
            if kind != .ragged, p.severity != CellsSeverityNote {
                places.append(UInt64(p.row) << 32 | UInt64(p.column))
                placed.append(problems.count)
            }
            problems.append(
                Cells.Problem(
                    at: .cell(row: Int(p.row), column: Int(p.column)), kind: kind,
                    message: string(cells_problem_why(d, i)), said: string(cells_describe(d, i)),
                    span: Int(spans[i].start)..<Int(spans[i].start + spans[i].len),
                    severity: severity(p.severity)))
        }
        let sep = cells_dialect(d)
        let layout = Cells.Layout(
            delimiter: sep == 0 ? "," : Character(Unicode.Scalar(sep)),
            newline: cells_crlf(d) ? "\r\n" : "\n", trailing: cells_trailing(d),
            quotesAll: cells_quotes_all(d))
        return Read(
            columns: columns, names: names, inferred: inferred,
            problems: problems, places: places, placed: placed, rows: cells_rows(d),
            layout: layout, statements: sep == 0 ? statements(d) : [])
    }

    /// A problem's weight as the core says it.
    static func severity(_ s: CellsSeverity) -> Cells.Severity {
        switch s {
        case CellsSeverityError: .error
        case CellsSeverityWarning: .warning
        case CellsSeverityNote: .note
        default:
            preconditionFailure("Cells: the core names a severity \(s.rawValue) this binding does not know")
        }
    }

    static func part(_ d: OpaquePointer, _ of: CellsOf, _ i: Int, _ part: CellsPart, word: Int = 0)
        -> String
    {
        string(cells_type_part(d, of, i, part, word))
    }

    /// A type as the core read it.
    static func columnType(_ d: OpaquePointer, _ of: CellsOf, _ i: Int) -> Cells.ColumnType {
        let t = cells_type(d, of, i)
        let format = t.has_format ? part(d, of, i, CellsPartFormat) : nil
        var base: Cells.Base =
            switch t.base {
            case CellsBaseText: .text
            case CellsBaseInt: .int
            case CellsBaseNum: .num
            case CellsBasePct: .pct
            case CellsBaseBool:
                .bool(
                    true: part(d, of, i, CellsPartWord, word: 0),
                    false: part(d, of, i, CellsPartWord, word: 1))
            case CellsBaseDate: .date(format: format)
            case CellsBaseTime: .time(format: format)
            case CellsBaseDateTime: .datetime
            case CellsBaseUrl: .url
            case CellsBaseEmail: .email
            case CellsBaseId: .id
            case CellsBaseMoney: .money(currency: part(d, of, i, CellsPartCurrency))
            case CellsBaseChoice:
                .choice((0..<t.words).map { part(d, of, i, CellsPartWord, word: $0) })
            case CellsBaseRef:
                .ref((0..<t.words).map { part(d, of, i, CellsPartWord, word: $0) })
            default:
                preconditionFailure(
                    "Cells: the core names a base \(t.base.rawValue) this binding does not know")
            }
        if t.named { base = .named(part(d, of, i, CellsPartNamed), base) }
        let bounds: Cells.Bounds? =
            t.has_min || t.has_max
            ? Cells.Bounds(
                min: t.has_min ? Decimal(string: part(d, of, i, CellsPartMin)) : nil,
                max: t.has_max ? Decimal(string: part(d, of, i, CellsPartMax)) : nil,
                minExclusive: t.has_min && !t.min_inclusive,
                maxExclusive: t.has_max && !t.max_inclusive)
            : nil
        var tints: [String: Cells.Tint] = [:]
        if t.base == CellsBaseChoice {
            for k in 0..<t.words {
                if let tint = Cells.Tint(cells_type_tint(d, of, i, k)) {
                    tints[part(d, of, i, CellsPartWord, word: k)] = tint
                }
            }
        }
        return Cells.ColumnType(
            base: base, bounds: bounds, optional: t.optional, key: t.key, tints: tints)
    }
}

/// UTF-8 the core lent: copied out before the next call on its doc.
func string(_ s: CellsStr) -> String {
    String(decoding: UnsafeRawBufferPointer(start: s.ptr, count: s.len), as: UTF8.self)
}

/// Strings lent to the core for one call, as the `CellsStr`s it reads.
func lending<T>(_ strings: [String], _ body: ([CellsStr]) -> T) -> T {
    // One buffer for all of them, so a row of cells is one allocation.
    var joined = [UInt8]()
    var spans: [(Int, Int)] = []
    for s in strings {
        let start = joined.count
        joined.append(contentsOf: s.utf8)
        spans.append((start, joined.count - start))
    }
    return joined.withUnsafeBufferPointer { buffer in
        let base = UnsafeRawPointer(buffer.baseAddress)?.assumingMemoryBound(to: CChar.self)
        return body(spans.map { span in CellsStr(ptr: base.map { $0 + span.0 }, len: span.1) })
    }
}

/// A column exported through the Arrow C Data Interface: the core's own buffers, shared,
/// released when the last view of them goes.
final class Export: @unchecked Sendable {
    var schema = ArrowSchema()
    var array = ArrowArray()

    init(_ doc: OpaquePointer, _ column: Int) {
        cells_export_column(doc, column, &schema, &array)
    }

    deinit {
        schema.release(&schema)
        array.release(&array)
    }

    var format: String { String(cString: schema.format) }
}

extension Cells.Problem {
    /// The core's refusal of a file or an edit, in this binding's places (0-based columns
    /// and lines).
    init(refused f: CellsFailure) {
        let why = String(
            decoding: UnsafeRawBufferPointer(start: f.why, count: f.why_len), as: UTF8.self)
        self.init(grammar: f.at, Int(f.number), why: why, span: nil)
    }

    /// Where the grammar breaks, at the core's place `at`, `n` its number, and `span` in
    /// the text.
    init(grammar place: CellsAt, _ n: Int, why: String, span: Range<Int>?) {
        let (at, words): (Where, String) =
            switch place {
            case CellsAtHeader: (.header(column: nil), "the header")
            case CellsAtColumn: (.header(column: n - 1), "column \(n)")
            case CellsAtPreludeLine: (.prelude(line: n - 1), "prelude line \(n)")
            case CellsAtPrelude: (.prelude(line: nil), "the prelude")
            case CellsAtRow: (.record(row: n), "row \(n + 1)")
            case CellsAtFile: (.file, "the file")
            default: preconditionFailure("Cells: a refusal at no place")
            }
        self.init(at: at, kind: .grammar, message: why, said: "\(words): \(why)", span: span)
    }
}
