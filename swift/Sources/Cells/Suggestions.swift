// What a writer converting a plain file to Cells declares: more than the spec infers, since a
// person reviews it. A few words repeated are a choice, and in a file whose delimiter is not
// the comma, numbers with a decimal comma (`84,20`) are numbers. The engine infers the rest;
// this is only what goes beyond it, and the conversion that writes it.

import Foundation

extension Cells {
    /// A number written with a decimal comma, as a European export writes it (`84,20`,
    /// `1.234,56`, `-3`), in Cells's own digits (`84.20`, `1234.56`); nil when it is no such
    /// number.
    static func decimalComma(_ text: String) -> String? {
        guard text.wholeMatch(of: /[+-]?(\d+|\d{1,3}(\.\d{3})+)(,\d+)?/) != nil else {
            return nil
        }
        return text.replacingOccurrences(of: ".", with: "").replacingOccurrences(of: ",", with: ".")
    }
}

extension Cells.Document {

    /// The type a writer converting the file declares for a column: its own, else what
    /// its values read as, more than the spec infers since a person reviews it: a few
    /// words repeated are a choice, and in a file whose delimiter is not the comma,
    /// numbers with a decimal comma (`84,20`) are numbers. Optional only when a cell is
    /// blank.
    public func suggestedType(of column: Int) -> Cells.ColumnType {
        if let declared = columns[column].type { return declared }
        let values = self.values(column: column)
        var type = inferred[column]!
        guard case .text(let texts) = values else {
            // Inferred from its values, so every value it holds is one: blank is null.
            type.optional = (0..<values.count).contains { values.isNull($0) }
            return type
        }
        let cells = texts.map { ($0 ?? "").trimmingCharacters(in: .whitespaces) }
        let blank = cells.contains("")
        type.optional = blank
        let filled = cells.filter { !$0.isEmpty }
        if layout.delimiter != ",", !filled.isEmpty,
            filled.allSatisfy({ Cells.decimalComma($0) != nil })
        {
            return Cells.ColumnType(base: .num, optional: blank)
        }
        var counts: [String: Int] = [:]
        for t in filled.prefix(10_000) { counts[t, default: 0] += 1 }
        let seen = filled.prefix(10_000).count
        // each word seen twice on average, a dozen words at most
        if counts.count >= 2, counts.count <= 12, seen >= counts.count * 2,
            counts.keys.allSatisfy({ !$0.contains(",") && !$0.contains("\"") })
        {
            return Cells.ColumnType(base: .choice(counts.keys.sorted()), optional: blank)
        }
        return type
    }

    /// A cell's value read by the column's `suggestedType`: what a chart or a conversion
    /// reads a plain file's untyped cells as. Blank or broken is missing.
    public func suggestedValue(row: Int, column: Int, type: Cells.ColumnType) -> Cells.Value {
        let own = self.type(of: column)
        if type.base == own.base {
            return (try? value(row: row, column: column).get()) ?? .missing
        }
        let raw = text(row: row, column: column).trimmingCharacters(in: .whitespaces)
        if raw.isEmpty { return .missing }
        switch type.base {
        case .num where layout.delimiter != ",":
            return Cells.decimalComma(raw).flatMap { Decimal(string: $0) }.map(Cells.Value.num)
                ?? .missing
        case .choice(let words):
            return words.contains(raw) ? .text(raw) : .missing
        default:
            return (try? Cells.decode(raw, type).get()) ?? .missing
        }
    }

    /// The document as a Cells: every untyped column given its `suggestedType`, the records
    /// separated by commas, decimal commas written with a point. A new document, read from
    /// its own text; this one is untouched.
    public func withTypesWritten() -> Cells.Document {
        var doc = self
        var header: [String] = []
        var cells: [(row: Int, column: Int, text: String)] = []
        for (c, column) in columns.enumerated() {
            if column.type != nil {
                header.append(column.header)
                continue
            }
            let type = suggestedType(of: c)
            header.append(
                Cells.headerCell(name: column.name, spelled: Cells.shorthand(type, names: names)))
            guard type.base == .num, layout.delimiter != "," else { continue }
            for r in 0..<rows {
                let raw = text(row: r, column: c)
                if let dotted = Cells.decimalComma(raw.trimmingCharacters(in: .whitespaces)),
                    dotted != raw
                {
                    cells.append((r, c, dotted))
                }
            }
        }
        if !cells.isEmpty {
            // A number with its decimal comma made a point holds no control character.
            do throws(Cells.Problem) { try doc.set(cells) } catch {
                preconditionFailure("Cells: a number rewritten was refused: \(error)")
            }
        }
        return Cells.Document(reading: doc.written(sep: 0, header: header))
    }
}
