// A Cells document, read and written by the one engine: the cells core (Rust), the same
// reader every surface uses (the command line, the page as WebAssembly, these platforms
// behind its C header). The spec is github.com/adriangalilea/cells (SPEC.md), held to its
// conformance suite here as there (Tests/CellsTests).
//
// A cell that breaks its type never stops a file: it is a `Problem`, its text kept, so the
// tool that can fix it can show it. A broken grammar (an unknown type, a row longer than
// the header, a prelude never closed) is the one refusal of a `.cells` file: the document
// then holds the file as it is and that refusal, and no rows. A plain file (a `.csv`, a
// `.tsv`) is read as the files people hold are, never refused for its records. Written
// back so every byte nobody changed is the byte it was read as.

import CellsCore
import Foundation

/// Cells, data and its logic in one plain-text file: the types in the header cells
/// (`cost:EUR>=0`) and their formulas (`total:EUR=cost * qty`), the values CSV.
public enum Cells {}

extension Cells {
  /// How a file is read and written: a `.cells` file by the format's own law, or plain
  /// delimited text (a `.csv`, a `.tsv`, plain text) by its delimiter, the one its first
  /// lines agree on when none is given (a `.tsv` is tabs whatever its first line holds).
  /// A plain file's header cells are its columns' names (`cost:EUR` is a name), its types
  /// inferred, no prelude read; a row of another width reads as blanks with a `.ragged`
  /// problem, a stray quote is text, a blank or repeated name a `.name` problem.
  public enum Dialect: Sendable, Hashable {
    case cells
    case plain(delimiter: Character? = nil)
  }

  /// A column: its name, its type (nil: untyped, read as what its values infer) and its
  /// formula (nil: none), which computes every cell of it left blank.
  public struct Column: Sendable, Equatable {
    public var name: String
    public var type: ColumnType?
    /// As the header cell writes it after the type's `=`: `cost * qty`.
    public var formula: String?
    /// The type and formula as the header cell spells them, when read from one.
    var spelled: String?

    public init(name: String, type: ColumnType? = nil, formula: String? = nil) {
      precondition(!name.contains(":"), "Cells: a column's name holds no \":\" (\(name))")
      precondition(
        formula == nil || type != nil, "Cells: a formula needs a type before its =")
      self.name = name
      self.type = type
      self.formula = formula
      self.spelled = nil
    }

    init(name: String, type: ColumnType?, formula: String?, spelled: String?) {
      self.name = name
      self.type = type
      self.formula = formula
      self.spelled = spelled
    }

    /// The header cell: `name`, `name:type` or `name:type=formula`, as written (a new
    /// column's type in shorthand).
    public var header: String {
      if let spelled { return "\(name):\(spelled)" }
      guard let type else { return name }
      return "\(name):\(Cells.shorthand(type))" + (formula.map { "=\($0)" } ?? "")
    }
  }

  /// Where a cell's value comes from.
  public enum Origin: Sendable, Equatable {
    /// Its text: written, or in a column with a formula, the row's override.
    case written
    /// Its column's formula: `84.20=`, or a field not stored yet.
    case formula
    /// Its own formula, right of its `=`: `32.25=96.75 / 3`.
    case cell
  }

  /// What is wrong, and where: a cell that breaks its type (the suite's `type`,
  /// `missing`, `key`, `formula`, `reference`), a row of another width (`ragged`), a
  /// plain file's header cell naming its column badly (`name`), or where the grammar
  /// breaks (`grammar`: a header cell read as text, a prelude statement left out, a
  /// column formula's cells blank, the rows ending at a record; for bytes that are not
  /// UTF-8, the file's one refusal).
  public struct Problem: Error, Sendable, Equatable, CustomStringConvertible {
    /// Rows from the first record and columns from the header's first cell, both
    /// from 0, and prelude lines from 0 inside the fences.
    public enum Where: Sendable, Equatable {
      /// The bytes: not UTF-8, or larger than 4 GiB.
      case file
      /// The header: one of its cells, or the line as a whole (nil).
      case header(column: Int?)
      /// The prelude: one of its lines, or the prelude as a whole (nil: never closed).
      case prelude(line: Int?)
      /// A record as a whole: its quoting, its length.
      case record(row: Int)
      case cell(row: Int, column: Int)
    }

    public enum Kind: Sendable, Hashable {
      /// The cell breaks its type, its bounds included.
      case type
      /// Blank where the type has no `?`.
      case missing
      /// The key of another row too.
      case key
      /// A cell's own formula that does not compute.
      case formula
      /// A reference to no row of the tables it names, or to rows of two of them.
      case reference
      /// A computed cell storing another value than its formula gives: its value is
      /// the formula's; `fixStale()` or `keepStale(row:column:)` resolves it.
      case stale
      /// A row longer than its header, on its last cell (its fields past the header
      /// kept), or a plain file's row shorter, on the first cell it lacks. The cell
      /// keeps its value.
      case ragged
      /// A plain file's header cell that is blank, or names another column too.
      case name
      /// The file's grammar is broken here, the rest read; or its bytes are not
      /// UTF-8, and nothing reads.
      case grammar
    }

    public let at: Where
    public let kind: Kind
    /// What is wrong, without the place: `"8x.50" is not a number`.
    public let message: String
    /// The engine's sentence for it, the place included.
    let said: String
    /// Where it is in the document's `text`, in offsets of its UTF-8: a cell's text
    /// inside its quotes, the control character it holds, the part of a formula that
    /// does not read, an empty range where a missing cell stands; a header cell's type,
    /// a prelude statement's line, the record the rows end at. Nil for the file as a
    /// whole and a plain file's header problems.
    public let span: Range<Int>?
    /// How much it weighs, the engine's one rule (`cells::Kind::severity`): what is
    /// wrong, or what is absent or behind. A broken grammar is wrong.
    public var severity: Severity = .error

    public var description: String { said }
  }

  /// What a problem is: wrong (a value breaking its type, a key repeated, a reference
  /// to no row, a formula that does not compute, a broken grammar), or a warning (a
  /// value not written yet, a stored value its formula has moved past, a ragged row, a
  /// plain header's name).
  public enum Severity: Sendable, Hashable {
    case error, warning
  }

  /// A value read from a cell by its column's type. A time, an instant, a choice and
  /// the text kinds read as their text.
  public enum Value: Sendable, Equatable {
    case missing
    case text(String)
    case int(Int)
    case num(Decimal)
    case bool(Bool)
    case date(DateComponents)
    case money(Decimal, currency: String)
  }

  /// Why a column's formula was refused, in words: `no column "qtty"`.
  public struct FormulaError: Error, Sendable, Equatable, CustomStringConvertible {
    public let formula: String
    public let reason: String
    public var description: String { "\"\(formula)\": \(reason)" }
  }

  /// How the file separates cells and lines, read from the file, kept on write.
  public struct Layout: Sendable, Equatable {
    /// The delimiters a file is read with: CSV's comma, the semicolon of a European
    /// export (where the comma is the decimal mark), the tab of a TSV and the pipe.
    /// Cells itself is the comma (SPEC.md, "Records").
    public static let delimiters: [Character] = [",", ";", "\t", "|"]

    public var delimiter: Character
    public var newline: String
    /// The text ends with a line break.
    public var trailing: Bool
    /// Every field of a plain file is quoted (`"fuel","84.20"`), so an edited cell and
    /// a new row are quoted so too.
    public var quotesAll: Bool

    public init(
      delimiter: Character = ",", newline: String = "\n", trailing: Bool = true,
      quotesAll: Bool = false
    ) {
      precondition(Layout.delimiters.contains(delimiter), "Cells: delimiter \(delimiter)")
      precondition(newline == "\n" || newline == "\r\n", "Cells: newline")
      self.delimiter = delimiter
      self.newline = newline
      self.trailing = trailing
      self.quotesAll = quotesAll
    }

    /// The delimiter as a person names it.
    public static func name(_ delimiter: Character) -> String {
      switch delimiter {
      case ",": "comma"
      case ";": "semicolon"
      case "\t": "tab"
      case "|": "pipe"
      default: preconditionFailure("Cells: delimiter \(delimiter)")
      }
    }

    /// The byte the engine separates fields by.
    var byte: UInt8 { delimiter.asciiValue! }
  }

  /// One record's cells, as written, unquoted; a short row's missing cells blank.
  public struct Record: Sendable, Equatable {
    public var cells: [String]

    public init(cells: [String]) {
      self.cells = cells
    }
  }

  /// The records, each read from the engine when asked: a million rows cost nothing
  /// until one is looked at.
  public struct Records: RandomAccessCollection, Sendable {
    let document: Document
    public var startIndex: Int { 0 }
    public var endIndex: Int { document.rows }
    public subscript(row: Int) -> Record {
      Record(cells: document.columns.indices.map { document.text(row: row, column: $0) })
    }
  }

  public struct Document: Sendable, Equatable {
    var engine: Engine

    init(engine: Engine) {
      self.engine = engine
    }

    /// The text read as `dialect` (a `.cells` file by default): never throws, and a
    /// `.cells` file's broken grammar reads around what it breaks (`broken`).
    public init(reading text: String, as dialect: Dialect = .cells) {
      var bytes = text
      engine = bytes.withUTF8 { raw in
        let raw = UnsafeRawBufferPointer(raw)
        return Engine.reading(raw, sep: Document.sep(dialect, raw), text: text)
      }
    }

    /// The bytes read as `dialect`; bytes that are not UTF-8 are a refusal of the file.
    public init(reading data: Data, as dialect: Dialect = .cells) {
      engine = data.withUnsafeBytes { raw in
        Engine.reading(
          raw, sep: Document.sep(dialect, raw),
          text: String(decoding: data, as: UTF8.self))
      }
    }

    /// The `.cells` file at `url`, its bytes `data`, its imports followed from the disk:
    /// a path read from the file's folder as it is now. A URL, or a file at a revision,
    /// reads as unreachable (a reference into it unchecked), as one never found does.
    public init(reading data: Data, at url: URL) {
      let path = url.path
      engine = data.withUnsafeBytes { raw in
        Engine.reading(raw, text: String(decoding: data, as: UTF8.self)) {
          bytes, count, failure in
          lending([path]) { s in
            cells_read_at(
              bytes, count, s[0], ProcessInfo.processInfo.activeProcessorCount,
              &failure)
          }
        }
      }
    }

    /// The `.cells` file at `path`, its imports followed through `files` (each file's
    /// text by its path, as imports name them from `path`'s folder). A file `files`
    /// does not hold reads as unreachable.
    public init(reading text: String, at path: String, files: [String: String]) {
      self.init(reading: Data(text.utf8), at: path, files: files)
    }

    /// The bytes of the `.cells` file at `path`, its imports followed through `files`;
    /// bytes that are not UTF-8 are a refusal of the file.
    public init(reading data: Data, at path: String, files: [String: String]) {
      let held = files.sorted { $0.key < $1.key }
      engine = data.withUnsafeBytes { raw in
        Engine.reading(raw, text: String(decoding: data, as: UTF8.self)) {
          b, count, failure in
          lending([path] + held.map(\.key) + held.map(\.value)) { s in
            let paths = Array(s[1...held.count])
            let texts = Array(s[(held.count + 1)...])
            return cells_read_with(
              b, count, s[0], paths, texts, held.count,
              ProcessInfo.processInfo.activeProcessorCount, &failure)
          }
        }
      }
    }

    /// The engine's name for a dialect: 0 for Cells, else a plain file's delimiter
    /// byte, found in `bytes` when none is given.
    static func sep(_ dialect: Dialect, _ bytes: UnsafeRawBufferPointer) -> UInt8 {
      switch dialect {
      case .cells: return 0
      case .plain(let delimiter?):
        precondition(Layout.delimiters.contains(delimiter), "Cells: delimiter \(delimiter)")
        return delimiter.asciiValue!
      case .plain(nil):
        return cells_detect(
          bytes.baseAddress?.assumingMemoryBound(to: UInt8.self), bytes.count)
      }
    }

    /// A document written from its columns and records, as `dialect` (its delimiter
    /// the layout's).
    public init(
      columns: [Column], records: [Record] = [], layout: Layout = Layout(),
      as dialect: Dialect = .cells
    ) {
      let sep: UInt8 =
        switch dialect {
        case .cells:
          {
            precondition(
              layout.delimiter == ",", "Cells: a .cells file's delimiter is the comma"
            )
            return 0
          }()
        case .plain: layout.byte
        }
      let header = columns.map { sep == 0 ? $0.header : $0.name }
      var failure = CellsFailure()
      let doc = lending(header) { head in
        withRows(records.map { .new($0.cells) }) { rows in
          cells_new(
            head, head.count, rows, rows.count, sep, layout.newline == "\r\n",
            layout.trailing, &failure)
        }
      }
      engine = Engine.made(
        doc, &failure, text: header.joined(separator: String(layout.delimiter)), sep: sep)
    }

    public static func == (a: Document, b: Document) -> Bool {
      if a.engine === b.engine { return true }
      return a.text == b.text && a.dialect == b.dialect
    }

    // MARK: - What it holds

    /// How the file was read: `.cells`, or a plain file by its delimiter.
    public var dialect: Dialect {
      engine.sep == 0 ? .cells : .plain(delimiter: Character(Unicode.Scalar(engine.sep)))
    }

    /// Whether the file is Cells: a plain file holds no types until written as one.
    public var typed: Bool { engine.sep == 0 }

    /// How many records the file has below its header.
    public var rows: Int { engine.said.rows }

    /// The columns, in the header's order.
    public var columns: [Column] { engine.said.columns }

    /// Named types the prelude declares.
    public var names: [String: ColumnType] { engine.said.names }

    /// The type each untyped column reads as, from its values: always optional.
    public var inferred: [Int: ColumnType] { engine.said.inferred }

    /// The records, read when looked at.
    public var records: Records { Records(document: self) }

    /// The prelude's lines as written, its two `---` included; empty without one.
    public var prelude: [String] {
      let text =
        engine.with { string(cells_prelude_text($0)) } ?? ""
      let lines = text.drop(while: { $0 == "\u{FEFF}" }).split(
        omittingEmptySubsequences: false, whereSeparator: \.isNewline)
      return lines.dropLast().map(String.init)
    }

    public var layout: Layout { engine.said.layout }

    /// Where the grammar breaks, in the file's order, the rest read: a header cell
    /// read as text, a prelude statement left out, a column formula's cells blank, the
    /// rows ending at a record. Empty for a file that reads whole.
    public var broken: [Problem] { problems.filter { $0.kind == .grammar } }

    /// Every problem: a plain file's header problems and where the grammar breaks,
    /// then every cell that breaks its type, every key missing or repeated, every
    /// ragged row, by row then column.
    public var problems: [Problem] { engine.said.problems }

    /// A column's type: its header's, or the one its values infer.
    public func type(of column: Int) -> ColumnType {
      columns[column].type ?? inferred[column]!
    }

    /// A column's type as a writer writes it, `EUR>=0!`: its header's, or the one its
    /// values infer. The engine's, since a reference is spelled only where its tables
    /// are named.
    public func shorthand(of column: Int) -> String {
      precondition(column < columns.count, "Cells: no column \(column)")
      return engine.with { Engine.part($0, CellsOfColumn, column, CellsPartShorthand) }!
    }

    /// A problem as a person reads it: `row 3, cost: "8x.50" is not a number`.
    public func describe(_ problem: Problem) -> String { problem.said }

    /// The file's text: every byte nobody changed as it was read.
    public var text: String {
      engine.with { string(cells_write($0)) } ?? engine.refusal!.text
    }

    /// The file's text as the format's standard lays it out (`cells fmt`): every
    /// table's columns aligned, or with no padding when `aligned` is false. A file
    /// whose grammar breaks is its text as read.
    public func formatted(aligned: Bool = true) -> String {
      engine.with { string(cells_formatted($0, aligned)) } ?? engine.refusal!.text
    }

    /// A cell's text as written, unquoted (a cell's formula with its backticks, a
    /// computed cell blank); blank where a short row has none.
    public func text(row: Int, column: Int) -> String {
      precondition(row < rows && column < columns.count, "Cells: no cell \(row), \(column)")
      return engine.with { string(cells_text($0, row, column)) }!
    }

    /// How many fields a row's record holds: a short row fewer than the header, a
    /// plain file's ragged row more.
    public func width(row: Int) -> Int {
      precondition(row < rows, "Cells: no row \(row)")
      return engine.with { cells_width($0, row) }!
    }

    /// What a cell of a column may hold, to offer as it is typed: a choice's words, a
    /// boolean's two, the keys of the table a reference names (read with its imports);
    /// empty for any other type.
    public func offers(column: Int) -> [String] {
      precondition(column < columns.count, "Cells: no column \(column)")
      let joined = engine.with { string(cells_offers($0, column)) } ?? ""
      return joined.split(separator: "\u{1f}", omittingEmptySubsequences: true).map(
        String.init)
    }

    /// The cells of this table the cell at `row` of `column` is computed from, as its
    /// formula reads them on its row: each a column and the rows of it read (this row,
    /// the one above, a range, the column whole for an aggregate or a window). None for
    /// a written cell.
    public func reads(row: Int, column: Int) -> [(column: Int, rows: ClosedRange<Int>)] {
      precondition(row < rows && column < columns.count, "Cells: no cell \(row), \(column)")
      return engine.with { d in
        let reads = cells_reads(d, row, column)
        defer { cells_reads_free(reads) }
        return UnsafeBufferPointer(start: reads.reads, count: reads.count).map {
          (column: $0.column, rows: $0.first...$0.last)
        }
      } ?? []
    }

    /// A cell's value by its column's type, or what is wrong with it.
    public func value(row: Int, column: Int) -> Result<Value, Problem> {
      precondition(row < rows && column < columns.count, "Cells: no cell \(row), \(column)")
      let said = engine.said
      let place = UInt64(row) << 32 | UInt64(column)
      var lo = 0
      var hi = said.places.count
      while lo < hi {
        let mid = (lo + hi) / 2
        if said.places[mid] < place { lo = mid + 1 } else { hi = mid }
      }
      if lo < said.places.count, said.places[lo] == place {
        return .failure(said.problems[said.placed[lo]])
      }
      let values = self.values(column: column)
      let type = self.type(of: column)
      func base(_ b: Base) -> Base { if case .named(_, let r) = b { base(r) } else { b } }
      switch (values, base(type.base)) {
      case (_, _) where values.isNull(row): return .success(.missing)
      case (.int(let v), _): return .success(.int(Int(v[row]!)))
      case (.decimal(let v), .money(let currency)):
        return .success(.money(v[row]!, currency: currency))
      case (.decimal(let v), _): return .success(.num(v[row]!))
      case (.bool(let v), _): return .success(.bool(v[row]!))
      case (.date(let v), _): return .success(.date(Cells.civil(days: v[row]!)))
      case (.text(let v), _): return .success(.text(v[row]!))
      case (.choice(let v), _): return .success(.text(v[row]!))
      case (.time, _), (.instant, _): return .success(.text(text(row: row, column: column)))
      }
    }

    /// Where a cell's value comes from: its text, its column's formula, or its own.
    public func origin(row: Int, column: Int) -> Origin {
      precondition(row < rows && column < columns.count, "Cells: no cell \(row), \(column)")
      return engine.with { d in
        switch cells_origin(d, row, column) {
        case CellsOriginWritten: Origin.written
        case CellsOriginFormula: Origin.formula
        case CellsOriginCell: Origin.cell
        default:
          preconditionFailure(
            "Cells: the core names an origin this binding does not know")
        }
      }!
    }

    /// A column's values as the engine holds them, read in place: a computed cell's
    /// value what its formula gives.
    public func values(column: Int) -> Values {
      precondition(column < columns.count, "Cells: no column \(column)")
      return Values(engine.export(column))
    }

    /// A type's text read in this file's prelude, its named types known.
    public func parseType(_ raw: String) throws(TypeError) -> ColumnType {
      let prelude = engine.with { string(cells_prelude_text($0)) } ?? ""
      return try Cells.readType(raw, prelude: prelude)
    }

    // MARK: - Changing it

    /// The engine to change, this copy's alone.
    private mutating func unique() -> Engine {
      if !isKnownUniquelyReferenced(&engine) { engine = engine.copy() }
      return engine
    }

    /// A cell set to `text`; its record is written from its cells from then on, every
    /// other byte as read but the computed values the change reaches, stored left of
    /// their `=`. `=96.75 / 3` is the cell's own formula (its value stored by this
    /// write); blank or a lone `=` in a column with a formula, the cell is computed; a
    /// value with no `=` is written. Refused, the document as it was, when the text
    /// holds a control character, which no writer writes.
    public mutating func set(row: Int, column: Int, to text: String) throws(Problem) {
      precondition(row < rows && column < columns.count, "Cells: no cell \(row), \(column)")
      var failure = CellsFailure()
      var set = false
      unique().changed { d in
        set = lending([text]) { s in
          cells_set_cell(d, row, column, s[0].ptr, s[0].len, &failure)
        }
      }
      guard set else {
        defer { cells_failure_free(&failure) }
        throw Problem(refused: failure)
      }
    }

    /// Every stale cell given its formula's value (`cells fix`); an override is never
    /// touched.
    public mutating func fixStale() {
      unique().changed { d in cells_fix_stale(d) }
    }

    /// One stale cell's stored value kept as its row's override: its `=` and formula
    /// dropped.
    public mutating func keepStale(row: Int, column: Int) throws(Problem) {
      precondition(row < rows && column < columns.count, "Cells: no cell \(row), \(column)")
      var failure = CellsFailure()
      var kept = false
      unique().changed { d in kept = cells_keep_stale(d, row, column, &failure) }
      guard kept else {
        defer { cells_failure_free(&failure) }
        throw Problem(refused: failure)
      }
    }

    /// Cells set at once (a paste, a fill): all of them, or, when one is refused, none.
    public mutating func set(_ cells: [(row: Int, column: Int, text: String)]) throws(Problem) {
      for (row, column, _) in cells {
        precondition(
          row < rows && column < columns.count, "Cells: no cell \(row), \(column)")
      }
      let next = engine.copy()
      var failure = CellsFailure()
      var set = true
      next.changed { d in
        for (row, column, text) in cells where set {
          set = lending([text]) { s in
            cells_set_cell(d, row, column, s[0].ptr, s[0].len, &failure)
          }
        }
      }
      guard set else {
        defer { cells_failure_free(&failure) }
        throw Problem(refused: failure)
      }
      engine = next
    }

    /// A cell set to a value, written in its column's form (its pair's words, its
    /// format); `.missing` blanks it. A `.text` is the value's canonical spelling for
    /// a column of any kind (`14:30:00`, `2026-08-14T12:30:00Z`, `84.20` as written).
    /// Refused, the document as it was, when the text holds a control character.
    public mutating func set(row: Int, column: Int, value: Value) throws(Problem) {
      precondition(row < rows && column < columns.count, "Cells: no cell \(row), \(column)")
      let canonical: String? =
        switch value {
        case .missing: nil
        case .text(let t): t
        case .int(let i): String(i)
        case .num(let d), .money(let d, _): NSDecimalNumber(decimal: d).stringValue
        case .bool(let b): b ? "true" : "false"
        case .date(let c):
          String(format: "%04d-%02d-%02d", c.year!, c.month!, c.day!)
        }
      var failure = CellsFailure()
      var set = false
      unique().changed { d in
        set = lending([canonical ?? ""]) { s in
          cells_set_value(d, row, column, canonical == nil, s[0].ptr, s[0].len, &failure)
        }
      }
      guard set else {
        defer { cells_failure_free(&failure) }
        throw Problem(refused: failure)
      }
    }

    /// A column's type set (nil: untyped again, its formula gone with it), its header
    /// cell written in shorthand from then on, its formula kept. No cell is rewritten: a
    /// cell the new type breaks is a problem. Refused when the type does not read in
    /// this file, or a formula no longer reads (one that reads the column needs another
    /// kind, or the column's own gives what the type cannot hold). A plain file holds no
    /// types: written as a Cells file (`withTypesWritten`), it does.
    public mutating func setType(_ type: ColumnType?, column: Int) throws(TypeError) {
      precondition(column < columns.count, "Cells: no column \(column)")
      precondition(typed, "Cells: a plain file holds no types")
      let spelled = type.map { Cells.spelling($0, names: names) } ?? ""
      let formula = type == nil ? nil : columns[column].formula
      if let refusal = written(type: spelled, formula: formula, column: column) {
        throw TypeError(type: spelled, reason: refusal.message)
      }
    }

    /// A column's formula set (nil: taken out), its type kept: every cell of the column
    /// left blank computed by it from then on, a written one its row's override. Refused
    /// when the formula does not read, names what is not there, gives what the column
    /// cannot hold, or reads itself through others.
    public mutating func setFormula(_ formula: String?, column: Int) throws(FormulaError) {
      precondition(column < columns.count, "Cells: no column \(column)")
      precondition(typed, "Cells: a plain file computes nothing")
      guard let type = columns[column].type else {
        preconditionFailure("Cells: a formula needs a type before its =")
      }
      precondition(
        formula?.contains(where: \.isNewline) != true, "Cells: a formula is one line")
      if let refusal = written(
        type: Cells.spelling(type, names: names), formula: formula, column: column)
      {
        throw FormulaError(formula: formula ?? "", reason: refusal.message)
      }
    }

    /// A header cell's type and formula written (`EUR>=0=cost * qty`); the engine's
    /// refusal when they do not read, this document as it was.
    private mutating func written(type: String, formula: String?, column: Int) -> Problem? {
      written(
        spelled: type.isEmpty ? "" : type + (formula.map { "=\($0)" } ?? ""), column: column
      )
    }

    /// A column's type and formula set as a person writes them, `EUR>=0 = cost * qty`
    /// (empty: a plain column again; the suite's `type` edit): its header cell, or its
    /// declaration in the prelude, written so. Refused, with the engine's reason and
    /// the document as it was, when they do not read.
    public mutating func setSpelled(_ spelled: String, column: Int) throws(Problem) {
      precondition(column < columns.count, "Cells: no column \(column)")
      if let refusal = written(spelled: spelled, column: column) { throw refusal }
    }

    private mutating func written(spelled: String, column: Int) -> Problem? {
      var failure = CellsFailure()
      var set = false
      unique().changed { d in
        set = lending([spelled]) { s in
          cells_set_type(d, column, s[0].ptr, s[0].len, &failure)
        }
      }
      guard !set else { return nil }
      defer { cells_failure_free(&failure) }
      return Problem(refused: failure)
    }

    // MARK: - Rows

    /// The rows at `offsets` moved, in their order, to stand before the row at
    /// `destination` (`rows` for the end), as SwiftUI's `move(fromOffsets:toOffset:)`
    /// counts it.
    public mutating func moveRows(fromOffsets offsets: IndexSet, toOffset destination: Int) {
      precondition(
        offsets.allSatisfy { $0 < rows } && destination <= rows, "Cells: no such move")
      let before = (0..<destination).filter { !offsets.contains($0) }
      let after = (destination..<rows).filter { !offsets.contains($0) }
      arrange((before + Array(offsets) + after).map { .kept($0) })
    }

    /// Rows inserted at `index`, each written from its cells.
    public mutating func insertRows(_ rows: [[String]], at index: Int) {
      precondition((0...self.rows).contains(index), "Cells: no place \(index) for rows")
      let kept = (0..<self.rows).map { Arranged.kept($0) }
      arrange(Array(kept[..<index]) + rows.map { .new($0) } + Array(kept[index...]))
    }

    /// The rows at `indices` taken out.
    public mutating func removeRows(_ indices: IndexSet) {
      precondition(indices.allSatisfy { $0 < rows }, "Cells: no rows \(indices)")
      arrange((0..<rows).filter { !indices.contains($0) }.map { .kept($0) })
    }

    /// The file with exactly these rows, read again: a kept row its record as it is
    /// now, a new one written from its cells.
    private mutating func arrange(_ order: [Arranged]) {
      precondition(!columns.isEmpty, "Cells: a file with no header has no rows")
      let next = engine.derived { d, failure in
        withRows(order) { rows in
          cells_rearranged(
            d, rows, rows.count, ProcessInfo.processInfo.activeProcessorCount,
            &failure)
        }
      }
      switch next {
      case .success(let e): engine = e
      case .failure(let p): preconditionFailure("Cells: rows rearranged do not read: \(p)")
      }
    }

    // MARK: - Columns

    /// A column inserted at `index`, its cells `cells` (blank when nil), every other
    /// field of every row as written. Refused when its name is another column's in a
    /// `.cells` file, or its type does not read there. A plain file's column is its name.
    public mutating func insertColumn(_ column: Column, at index: Int, cells: [String]? = nil)
      throws(Problem)
    {
      precondition(
        (0...columns.count).contains(index), "Cells: no place \(index) for a column")
      precondition(cells == nil || cells!.count == rows, "Cells: a cell for each row")
      try insertColumn(header: typed ? column.header : column.name, at: index, cells: cells)
    }

    /// A column inserted from its header cell as the file spells it (the suite's
    /// `insert` edit).
    mutating func insertColumn(header: String, at index: Int, cells: [String]?) throws(Problem) {
      var order = columns.indices.map { Reshape.kept($0) }
      order.insert(.new(header, cells ?? []), at: index)
      try reshape(order)
    }

    /// The column at `index` taken out, and its cells, row by row. Refused when a
    /// formula reads it. The last column takes the rows with it: a file with no
    /// header has none.
    @discardableResult
    public mutating func removeColumn(at index: Int) throws(Problem) -> (
      column: Column, cells: [String]
    ) {
      precondition(columns.indices.contains(index), "Cells: no column \(index)")
      let column = columns[index]
      let cells = (0..<rows).map { text(row: $0, column: index) }
      var order = columns.indices.map { Reshape.kept($0) }
      order.remove(at: index)
      try reshape(order)
      return (column, cells)
    }

    /// The column at `from` moved to stand at `to` among the columns.
    public mutating func moveColumn(from: Int, to: Int) {
      precondition(
        columns.indices.contains(from) && columns.indices.contains(to),
        "Cells: move \(from) to \(to)")
      guard from != to else { return }
      var order = columns.indices.map { Reshape.kept($0) }
      order.insert(order.remove(at: from), at: to)
      do throws(Problem) { try reshape(order) } catch {
        preconditionFailure("Cells: columns moved do not read: \(error)")
      }
    }

    /// A column renamed, its type and formula kept. Refused in a `.cells` file when
    /// another column has the name, or a formula reads the old one.
    public mutating func rename(column: Int, to name: String) throws(Problem) {
      precondition(columns.indices.contains(column), "Cells: no column \(column)")
      precondition(
        !typed || !name.contains(":"), "Cells: a column's name holds no \":\" (\(name))")
      var order = columns.indices.map { Reshape.kept($0) }
      order[column] = .renamed(column, name)
      try reshape(order)
    }

    /// The file with exactly these columns, read again: every kept field as written.
    private mutating func reshape(_ order: [Reshape]) throws(Problem) {
      var texts: [String] = []
      var runs: [(kept: Int, renamed: Bool, name: Int, cells: Int, count: Int)] = []
      for c in order {
        switch c {
        case .kept(let k): runs.append((k, false, -1, 0, 0))
        case .renamed(let k, let name):
          texts.append(name)
          runs.append((k, true, texts.count - 1, 0, 0))
        case .new(let header, let cells):
          texts.append(header)
          let at = texts.count
          texts += cells
          runs.append((-1, false, at - 1, at, cells.count))
        }
      }
      let next = engine.derived { d, failure in
        lending(texts) { strs in
          strs.withUnsafeBufferPointer { s in
            let columns = runs.map { r in
              CellsColumnAt(
                kept: Int64(r.kept), renamed: r.renamed,
                name: r.name < 0 ? CellsStr(ptr: nil, len: 0) : s[r.name],
                cells: r.count == 0 ? nil : s.baseAddress! + r.cells,
                count: r.count)
            }
            return cells_reshaped(
              d, columns, columns.count,
              ProcessInfo.processInfo.activeProcessorCount, &failure)
          }
        }
      }
      engine = try next.get()
    }

    // MARK: - Writing it as another dialect

    /// The text written for another format. `typed` writes a cell: the prelude and the
    /// header's types and formulas, the records separated by commas. Otherwise a plain
    /// CSV or TSV separated by `delimiter`: the header the names alone, no prelude, and
    /// every computed cell its value, so the numbers a person saw are in the file. A
    /// record whose bytes already are what the delimiter writes is kept as read.
    public func written(delimiter: Character, typed: Bool) -> String {
      precondition(
        !typed || delimiter == ",", "Cells: a .cells file's delimiter is the comma")
      precondition(Layout.delimiters.contains(delimiter), "Cells: delimiter \(delimiter)")
      return written(sep: typed ? 0 : delimiter.asciiValue!, header: nil)
    }

    /// The text in a dialect (`sep` 0 for Cells), with these header cells when given.
    func written(sep: UInt8, header: [String]?) -> String {
      engine.with { d in
        lending(header ?? []) { s in string(cells_written_as(d, sep, s, s.count)) }
      } ?? engine.refusal!.text
    }
  }
}

/// A column of a file reshaped: one it has, renamed, or new (its header cell and cells).
enum Reshape {
  case kept(Int)
  case renamed(Int, String)
  case new(String, [String])
}

/// A row of a file rearranged: one it has, or new cells.
enum Arranged {
  case kept(Int)
  case new([String])
}

/// The rows as the core reads them, for one call.
func withRows<T>(_ rows: [Arranged], _ body: ([CellsRow]) -> T) -> T {
  // Every new row's cells in one lent buffer, each row a run of its strings.
  var cells: [String] = []
  var runs: [(kept: Int, from: Int, count: Int)] = []
  for r in rows {
    switch r {
    case .kept(let k): runs.append((k, 0, 0))
    case .new(let c):
      runs.append((-1, cells.count, c.count))
      cells += c
    }
  }
  return lending(cells) { strs in
    strs.withUnsafeBufferPointer { s in
      body(
        runs.map { r in
          CellsRow(
            kept: Int64(r.kept), cells: r.count == 0 ? nil : s.baseAddress! + r.from,
            count: r.count)
        })
    }
  }
}

extension Cells {
  /// A date's year, month and day from its days since 1970-01-01.
  static func civil(days: Int32) -> DateComponents {
    var utc = Calendar(identifier: .gregorian)
    utc.timeZone = TimeZone(identifier: "UTC")!
    let d = utc.dateComponents(
      [.year, .month, .day], from: Date(timeIntervalSince1970: Double(days) * 86_400))
    return DateComponents(year: d.year, month: d.month, day: d.day)
  }
}
