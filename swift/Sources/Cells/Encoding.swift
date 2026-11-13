// The bytes under the text. Cells is UTF-8 (SPEC.md, "The file"); the CSV and TSV
// files people actually hold are also UTF-16 (Excel's "Unicode text") and
// Windows-1252 (an older Excel's export), so a reader of those reads them too, and a
// writer writes each back in its own encoding, its byte order mark included. Bytes
// that are none of them are refused with the reason, never decoded as noise.

import Foundation

extension Cells {
    /// How a file's text is stored: its character set and whether it opens with a
    /// byte order mark. Read from the bytes, kept on write.
    public struct Encoding: Sendable, Hashable, CustomStringConvertible {
        public enum Charset: String, Sendable, Hashable, CaseIterable {
            case utf8 = "UTF-8"
            case utf16LittleEndian = "UTF-16 LE"
            case utf16BigEndian = "UTF-16 BE"
            case windows1252 = "Windows-1252"
        }

        public var charset: Charset
        public var bom: Bool

        public init(_ charset: Charset, bom: Bool) {
            precondition(
                charset != .windows1252 || !bom, "Cells: Windows-1252 has no byte order mark")
            self.charset = charset
            self.bom = bom
        }

        public static let utf8 = Encoding(.utf8, bom: false)
        public static let utf8BOM = Encoding(.utf8, bom: true)
        /// Excel's "Unicode text": UTF-16, little-endian, with its mark.
        public static let utf16 = Encoding(.utf16LittleEndian, bom: true)
        public static let windows1252 = Encoding(.windows1252, bom: false)

        /// What a person picks from: every encoding a file is written in here.
        public static let all: [Encoding] = [.utf8, .utf8BOM, .utf16, .windows1252]

        /// `UTF-8`, `UTF-8 with BOM`, `UTF-16 LE`: what a file's encoding is called.
        public var description: String {
            switch (charset, bom) {
            case (.utf8, true): "UTF-8 with BOM"
            case (.utf16LittleEndian, false), (.utf16BigEndian, false):
                "\(charset.rawValue), no BOM"
            default: charset.rawValue
            }
        }

        var mark: [UInt8] {
            guard bom else { return [] }
            switch charset {
            case .utf8: return [0xEF, 0xBB, 0xBF]
            case .utf16LittleEndian: return [0xFF, 0xFE]
            case .utf16BigEndian: return [0xFE, 0xFF]
            case .windows1252: return []
            }
        }
    }

    /// Bytes that are no text this reader takes, and why, in words.
    public struct Unreadable: Error, Sendable, Equatable, CustomStringConvertible {
        public let reason: String
        public var description: String { reason }
    }

    /// Text that its encoding cannot hold: the first character it has no bytes for.
    public struct Unwritable: Error, Sendable, Equatable, CustomStringConvertible {
        public let character: Character
        public let encoding: Encoding
        public var description: String {
            "\(encoding) has no bytes for \"\(character)\""
        }
    }

    /// The five bytes Windows-1252 leaves undefined: a file holding one is not in it.
    static let undefined1252: Set<UInt8> = [0x81, 0x8D, 0x8F, 0x90, 0x9D]

    /// A file's bytes as text, and how they were stored. A byte order mark says the
    /// encoding; without one, UTF-8 when every byte reads as UTF-8, UTF-16 when the
    /// text's every other byte is zero (Latin letters stored in two bytes), else
    /// Windows-1252 when every byte is one of its characters. `accepting` narrows
    /// it: Cells itself is UTF-8 alone.
    public static func decode(
        _ data: Data, accepting: Set<Encoding.Charset> = Set(Encoding.Charset.allCases)
    ) throws(Unreadable) -> (text: String, encoding: Encoding) {
        let bytes = [UInt8](data)
        let found = try sniff(bytes)
        guard accepting.contains(found.encoding.charset) else {
            let names = Encoding.Charset.allCases.filter(accepting.contains).map(\.rawValue)
            throw Unreadable(
                reason:
                    "it is \(found.encoding.charset.rawValue), and this format is \(names.joined(separator: " or ")) only"
            )
        }
        return found
    }

    static func sniff(_ bytes: [UInt8]) throws(Unreadable) -> (text: String, encoding: Encoding) {
        if bytes.starts(with: [0xEF, 0xBB, 0xBF]) {
            return (try utf8(bytes.dropFirst(3), offset: 3), .utf8BOM)
        }
        if bytes.starts(with: [0xFF, 0xFE]) {
            return (try utf16(bytes.dropFirst(2), little: true), .utf16)
        }
        if bytes.starts(with: [0xFE, 0xFF]) {
            return (
                try utf16(bytes.dropFirst(2), little: false), Encoding(.utf16BigEndian, bom: true)
            )
        }
        if let little = zeroesOfUTF16(bytes) {
            return (
                try utf16(bytes[...], little: little),
                Encoding(little ? .utf16LittleEndian : .utf16BigEndian, bom: false)
            )
        }
        do {
            return (try utf8(bytes[...], offset: 0), .utf8)
        } catch {
            // Not UTF-8: Windows-1252 if every byte is one of its characters.
            if let bad = bytes.firstIndex(where: undefined1252.contains) {
                throw Unreadable(
                    reason:
                        "\(error.reason), and byte \(hex(bytes[bad])) at \(bad) is no Windows-1252 character either"
                )
            }
            guard let text = String(bytes: bytes, encoding: .windowsCP1252) else {
                throw Unreadable(reason: "\(error.reason), and it does not read as Windows-1252")
            }
            return (text, .windows1252)
        }
    }

    /// Whether the bytes are UTF-16 stored with no mark: in the first few thousand,
    /// most bytes on one side of each pair are zero and almost none on the other.
    /// Which side says the byte order. nil: not UTF-16.
    static func zeroesOfUTF16(_ bytes: [UInt8]) -> Bool? {
        let sample = bytes.prefix(4096)
        guard sample.count >= 4, sample.count % 2 == 0 else { return nil }
        var even = 0
        var odd = 0
        for (i, b) in sample.enumerated() where b == 0 {
            if i % 2 == 0 { even += 1 } else { odd += 1 }
        }
        let pairs = sample.count / 2
        if odd * 10 >= pairs * 4, even * 50 <= pairs { return true }
        if even * 10 >= pairs * 4, odd * 50 <= pairs { return false }
        return nil
    }

    static func hex(_ b: UInt8) -> String { "0x" + String(b, radix: 16, uppercase: true) }

    /// UTF-8, or the offset of the first byte that begins no character.
    static func utf8(_ bytes: ArraySlice<UInt8>, offset: Int) throws(Unreadable) -> String {
        if let text = String(validating: bytes, as: UTF8.self) { return text }
        // Invalid: walk it once more to name the byte.
        var parser = Unicode.UTF8.ForwardParser()
        var it = bytes.makeIterator()
        var at = 0
        while case .valid(let encoded) = parser.parseScalar(from: &it) { at += encoded.count }
        let byte = bytes[bytes.startIndex + at]
        throw Unreadable(reason: "byte \(hex(byte)) at \(offset + at) begins no UTF-8 character")
    }

    static func utf16(_ bytes: ArraySlice<UInt8>, little: Bool) throws(Unreadable) -> String {
        guard bytes.count % 2 == 0 else {
            throw Unreadable(reason: "UTF-16 is two bytes a unit, and it has an odd \(bytes.count)")
        }
        var units: [UInt16] = []
        units.reserveCapacity(bytes.count / 2)
        var i = bytes.startIndex
        while i < bytes.endIndex {
            let a = UInt16(bytes[i])
            let b = UInt16(bytes[i + 1])
            units.append(little ? a | b << 8 : a << 8 | b)
            i += 2
        }
        if let text = String(validating: units, as: UTF16.self) { return text }
        var parser = Unicode.UTF16.ForwardParser()
        var it = units.makeIterator()
        var at = 0
        while case .valid(let encoded) = parser.parseScalar(from: &it) { at += encoded.count }
        throw Unreadable(reason: "the UTF-16 unit at byte \(at * 2) is half a character")
    }

    /// The text as bytes in `encoding`, its byte order mark first when it has one.
    public static func encode(_ text: String, _ encoding: Encoding) throws(Unwritable) -> Data {
        var out = encoding.mark
        switch encoding.charset {
        case .utf8:
            out += Array(text.utf8)
        case .utf16LittleEndian, .utf16BigEndian:
            let little = encoding.charset == .utf16LittleEndian
            out.reserveCapacity(out.count + text.utf16.count * 2)
            for u in text.utf16 {
                let lo = UInt8(u & 0xFF)
                let hi = UInt8(u >> 8)
                out += little ? [lo, hi] : [hi, lo]
            }
        case .windows1252:
            guard let data = text.data(using: .windowsCP1252, allowLossyConversion: false) else {
                let first = text.first { String($0).data(using: .windowsCP1252) == nil }
                throw Unwritable(character: first ?? "?", encoding: encoding)
            }
            out += Array(data)
        }
        return Data(out)
    }
}
