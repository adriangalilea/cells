// A `.cells` file's charts as the engine reads them (SPEC.md, Charts): each chart the file
// declares, and what one draws, summarized by the core from the rows as they are now. The
// core decides the kind, the places, the series and their tints; a host only draws.

import CellsCore
import Foundation

extension Cells {
    /// A chart the file declares, as written: its place among the file's charts is its
    /// place in `Document.charts`.
    public struct Chart: Decodable, Sendable, Equatable {
        /// One line of its fence.
        public struct Entry: Decodable, Sendable, Equatable {
            public let key: String
            public let value: String
            /// Its prelude line, 1-based inside the fences.
            public let line: Int
        }

        /// The prelude line its fence opens on; nil for the one read from the columns.
        public let line: Int?
        /// Its name, written only when another chart reads it.
        public let name: String?
        /// Its `title`, else what its entries say.
        public let title: String
        /// `bars`, `line`, `area`, `steps`, `points`, `heat`, `figure`.
        public let kind: String
        /// Whether its fence names its kind, or it is read from its entries.
        public let written: Bool
        public let entries: [Entry]

        /// Its entry's value for `key`, as written.
        public func value(_ key: String) -> String? { entries.first { $0.key == key }?.value }

        /// A chart's fence as the file holds it (SPEC.md, Charts): its kind the label (none
        /// read from its entries), named when another chart reads it, one entry a line in
        /// the order given, keys aligned.
        public static func fence(kind: String?, name: String?, entries: [(key: String, value: String)])
            -> String
        {
            precondition(
                entries.allSatisfy { !$0.value.contains("\n") }, "Cells: an entry is one line")
            let width = entries.map(\.key.count).max() ?? 0
            let lines = entries.map { e in
                e.key + ":" + String(repeating: " ", count: width - e.key.count + 1) + e.value
            }
            let open = (name.map { "\($0) = " } ?? "") + "```chart" + (kind.map { " \($0)" } ?? "")
            return ([open] + lines + ["```"]).joined(separator: "\n")
        }
    }

    /// What a chart draws: its places along its axis, its series and each one's shade,
    /// and each place's value of each series, exact decimals as text.
    public struct ChartSummary: Decodable, Sendable, Equatable {
        /// What a series is drawn in.
        public enum Shade: Decodable, Sendable, Equatable {
            /// One of the ten tints, or six hex digits: the word's own, or the next free.
            case tint(Tint)
            /// A column's own colour (`Palette.columns`, in turn).
            case column(Int)
            /// `other`, and a blank value.
            case quiet

            private enum Key: String, CodingKey { case tint, hex, column, quiet }

            public init(from decoder: Decoder) throws {
                let c = try decoder.container(keyedBy: Key.self)
                if let name = try c.decodeIfPresent(String.self, forKey: .tint) {
                    guard let n = Tint.Name.allCases.first(where: { "\($0)" == name }) else {
                        preconditionFailure("Cells: the core names tint \(name), past the ten")
                    }
                    self = .tint(.named(n))
                } else if let hex = try c.decodeIfPresent(String.self, forKey: .hex) {
                    guard hex.hasPrefix("#"), let v = UInt32(hex.dropFirst(), radix: 16) else {
                        preconditionFailure("Cells: the core writes a hex tint as #rrggbb, not \(hex)")
                    }
                    self = .tint(.hex(v))
                } else if let k = try c.decodeIfPresent(Int.self, forKey: .column) {
                    self = .column(k)
                } else {
                    let quiet = try c.decode(Bool.self, forKey: .quiet)
                    precondition(quiet, "Cells: a shade of none")
                    self = .quiet
                }
            }
        }

        /// A chart's scale (SPEC.md, Charts): its ends, whole steps, and its step, 1, 2 or
        /// 5 times a power of ten.
        public struct Scale: Decodable, Sendable, Equatable {
            public let lo: Double
            public let hi: Double
            public let step: Double
        }

        public struct Series: Decodable, Sendable, Equatable {
            /// Its value of the `color` entry; empty for a chart's one series.
            public let name: String
            public let shade: Shade
        }

        public let kind: String
        public let title: String
        /// `stacked`, `grouped`, `percent`.
        public let stack: String
        /// What runs along the axis: `date`, `number` or `text`.
        public let axis: String
        /// Each place, as its value is written (a date ISO, a number plain).
        public let places: [String]
        public let series: [Series]
        /// `values[place][series]`; nil where a series has no row at a place.
        public let values: [[String?]]
        /// The measure's currency, when it is money.
        public let unit: String?
        /// What its values are drawn on, the same in every view: `lo` to `hi` in whole
        /// steps of `step`, every layer's; nil for a figure or a heat drawn alone.
        public let scale: Scale?
        /// Rows `where` held for that had no place or no value, of the rows read.
        public let skipped: Int
        public let rows: Int
        /// A chart of layers: each one's own, drawn on one plot.
        public let layers: [ChartSummary]
    }
}

extension Cells.Document {
    /// The charts the file declares, in the order written: chart 1 the first.
    public var charts: [Cells.Chart] {
        let json = engine.with { string(cells_charts($0)) } ?? "[]"
        return Self.decode([Cells.Chart].self, json)
    }

    /// What the file's chart at `index` draws, read from its rows now.
    public func summary(chart index: Int) -> Cells.ChartSummary {
        precondition(index < charts.count, "Cells: no chart \(index)")
        let json = engine.with { string(cells_chart_summary($0, index)) }!
        return Self.decode(Cells.ChartSummary.self, json)
    }

    /// The chart a reader draws for a file with none, its entries what keeping it writes;
    /// nil when there is nothing to measure (no date, choice or number).
    public var inferredChart: Cells.Chart? {
        let json = engine.with { string(cells_inferred_chart($0)) } ?? ""
        return json.isEmpty ? nil : Self.decode(Cells.Chart.self, json)
    }

    /// What the chart a reader draws for a file with none draws; nil when there is nothing
    /// to measure (no date, choice or number).
    public var inferredSummary: Cells.ChartSummary? {
        let json = engine.with { string(cells_inferred_summary($0)) } ?? ""
        return json.isEmpty ? nil : Self.decode(Cells.ChartSummary.self, json)
    }

    private static func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T {
        do {
            return try JSONDecoder().decode(type, from: Data(json.utf8))
        } catch {
            preconditionFailure("Cells: the core's chart JSON does not read: \(error)\n\(json)")
        }
    }
}
