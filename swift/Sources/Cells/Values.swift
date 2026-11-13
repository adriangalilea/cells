// A column's values as the engine holds them: its Arrow buffers (the C Data Interface),
// read in place, never copied. Each view keeps the export it reads alive, so it stays valid
// after the document changes (the engine copies a buffer someone still holds before
// writing it) and after the document is gone.

import CellsCore
import Foundation

extension Cells {
    /// A column's values, by how the engine lays them out (Arrow's types).
    public enum Values: Sendable {
        /// `text`, `url`, `email`, `id`, and an untyped column read as text: UTF-8.
        case text(Texts)
        /// `int`: 64-bit.
        case int(Scalars<Int64>)
        /// `num`, `pct` and money: a 128-bit mantissa at one
        /// scale for the column.
        case decimal(Decimals)
        case bool(Bools)
        /// Days since 1970-01-01.
        case date(Scalars<Int32>)
        /// Seconds since midnight.
        case time(Scalars<Int32>)
        /// Instants in UTC, in units of `10^-unit` seconds.
        case instant(Instants)
        /// Indices into the choice's words.
        case choice(Choices)

        init(_ export: Export) {
            let a = export.array
            precondition(a.offset == 0, "Cells: the engine exports from the start of a buffer")
            let format = export.format
            let n = Int(a.length)
            let valid = a.buffers[0]?.assumingMemoryBound(to: UInt8.self)
            func scalars<T>(_ type: T.Type, buffer: Int = 1) -> Scalars<T> {
                Scalars(
                    export: export,
                    values: a.buffers[buffer]!.assumingMemoryBound(to: T.self), valid: valid,
                    count: n)
            }
            switch format {
            case "u":
                self = .text(
                    Texts(
                        export: export, valid: valid,
                        offsets: a.buffers[1]!.assumingMemoryBound(to: Int32.self),
                        bytes: a.buffers[2]?.assumingMemoryBound(to: UInt8.self), count: n))
            case "l": self = .int(scalars(Int64.self))
            case "b":
                self = .bool(
                    Bools(
                        export: export, valid: valid,
                        bits: a.buffers[1]!.assumingMemoryBound(to: UInt8.self), count: n))
            case "tdD": self = .date(scalars(Int32.self))
            case "tts": self = .time(scalars(Int32.self))
            case "s":
                let words = a.dictionary!.pointee
                let wordOffsets = words.buffers[1]!.assumingMemoryBound(to: Int32.self)
                let wordBytes = words.buffers[2]?.assumingMemoryBound(to: UInt8.self)
                let list = (0..<Int(words.length)).map { k in
                    String(
                        decoding: UnsafeBufferPointer(
                            start: wordBytes.map { $0 + Int(wordOffsets[k]) },
                            count: Int(wordOffsets[k + 1] - wordOffsets[k])), as: UTF8.self)
                }
                self = .choice(Choices(words: list, indices: scalars(Int16.self)))
            case _ where format.hasPrefix("d:38,"):
                self = .decimal(
                    Decimals(scale: Int(format.dropFirst(5))!, mantissas: scalars(Int128.self)))
            case _ where format.hasPrefix("ts"):
                let unit =
                    switch format.dropFirst(2).first {
                    case "s": 0
                    case "m": 3
                    case "u": 6
                    default: 9
                    }
                self = .instant(Instants(unit: unit, values: scalars(Int64.self)))
            default:
                preconditionFailure("Cells: the engine exports \(format), unknown to this binding")
            }
        }

        public var count: Int {
            switch self {
            case .text(let v): v.count
            case .int(let v): v.count
            case .decimal(let v): v.count
            case .bool(let v): v.count
            case .date(let v), .time(let v): v.count
            case .instant(let v): v.count
            case .choice(let v): v.count
            }
        }

        /// Whether the cell is blank or broken: no value.
        public func isNull(_ i: Int) -> Bool {
            switch self {
            case .text(let v): v[i] == nil
            case .int(let v): v[i] == nil
            case .decimal(let v): v.mantissas[i] == nil
            case .bool(let v): v[i] == nil
            case .date(let v), .time(let v): v[i] == nil
            case .instant(let v): v.values[i] == nil
            case .choice(let v): v.indices[i] == nil
            }
        }
    }

    /// Fixed-width values in the engine's buffer, nil where a cell has none.
    public struct Scalars<Element: BitwiseCopyable>: RandomAccessCollection, @unchecked Sendable {
        let export: Export
        let values: UnsafePointer<Element>
        let valid: UnsafePointer<UInt8>?
        public let count: Int

        public var startIndex: Int { 0 }
        public var endIndex: Int { count }

        public subscript(i: Int) -> Element? {
            precondition(i >= 0 && i < count, "Cells: no row \(i)")
            return isValid(i) ? values[i] : nil
        }

        public func isValid(_ i: Int) -> Bool {
            valid.map { $0[i >> 3] >> UInt8(i & 7) & 1 == 1 } ?? true
        }

        /// The buffer itself: a blank cell's slot holds whatever the engine left there,
        /// so read it beside `isValid`.
        public var buffer: UnsafeBufferPointer<Element> {
            UnsafeBufferPointer(start: values, count: count)
        }
    }

    /// Text in the engine's buffer: offsets into one run of UTF-8.
    public struct Texts: RandomAccessCollection, @unchecked Sendable {
        let export: Export
        let valid: UnsafePointer<UInt8>?
        let offsets: UnsafePointer<Int32>
        let bytes: UnsafePointer<UInt8>?
        public let count: Int

        public var startIndex: Int { 0 }
        public var endIndex: Int { count }

        public subscript(i: Int) -> String? {
            precondition(i >= 0 && i < count, "Cells: no row \(i)")
            guard valid.map({ $0[i >> 3] >> UInt8(i & 7) & 1 == 1 }) ?? true else { return nil }
            return String(decoding: utf8(i), as: UTF8.self)
        }

        /// A cell's UTF-8 in place.
        public func utf8(_ i: Int) -> UnsafeBufferPointer<UInt8> {
            UnsafeBufferPointer(
                start: bytes.map { $0 + Int(offsets[i]) }, count: Int(offsets[i + 1] - offsets[i]))
        }
    }

    /// Decimals in the engine's buffer: each a 128-bit mantissa, `value = m / 10^scale`.
    public struct Decimals: RandomAccessCollection, Sendable {
        public let scale: Int
        public let mantissas: Scalars<Int128>

        public var startIndex: Int { 0 }
        public var endIndex: Int { mantissas.count }

        public subscript(i: Int) -> Decimal? {
            mantissas[i].map { m in
                let magnitude =
                    m.magnitude <= UInt64.max
                    ? Decimal(UInt64(m.magnitude)) : Decimal(string: String(m.magnitude))!
                return Decimal(
                    sign: m < 0 ? .minus : .plus, exponent: -scale, significand: magnitude)
            }
        }

        /// The value as the nearest double: what a chart plots.
        public func double(_ i: Int) -> Double? {
            mantissas[i].map { Double($0) / pow(10, Double(scale)) }
        }
    }

    /// Booleans in the engine's bitmap.
    public struct Bools: RandomAccessCollection, @unchecked Sendable {
        let export: Export
        let valid: UnsafePointer<UInt8>?
        let bits: UnsafePointer<UInt8>
        public let count: Int

        public var startIndex: Int { 0 }
        public var endIndex: Int { count }

        public subscript(i: Int) -> Bool? {
            precondition(i >= 0 && i < count, "Cells: no row \(i)")
            guard valid.map({ $0[i >> 3] >> UInt8(i & 7) & 1 == 1 }) ?? true else { return nil }
            return bits[i >> 3] >> UInt8(i & 7) & 1 == 1
        }
    }

    /// Instants in the engine's buffer, in units of `10^-unit` seconds since 1970 (UTC).
    public struct Instants: RandomAccessCollection, Sendable {
        /// 0 (seconds), 3, 6 or 9 (nanoseconds).
        public let unit: Int
        public let values: Scalars<Int64>

        public var startIndex: Int { 0 }
        public var endIndex: Int { values.count }

        public subscript(i: Int) -> Date? {
            values[i].map { Date(timeIntervalSince1970: Double($0) / pow(10, Double(unit))) }
        }
    }

    /// A choice's words and each cell's index into them.
    public struct Choices: RandomAccessCollection, Sendable {
        public let words: [String]
        public let indices: Scalars<Int16>

        public var startIndex: Int { 0 }
        public var endIndex: Int { indices.count }

        public subscript(i: Int) -> String? { indices[i].map { words[Int($0)] } }
    }
}
