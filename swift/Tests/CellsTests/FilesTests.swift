import Foundation
import Testing

@testable import Cells

// The files people hold: their encodings and delimiters read, each written back
// as it came.

@Test func everyEncodingReadsAndWritesBackItsOwnBytes() throws {
    let text = "seed,sown,note\nbasil,2026-04-02,\"señal, sí\"\nleek,2026-04-09,€ por sobre\n"
    for encoding in Cells.Encoding.all {
        let bytes = try Cells.encode(text, encoding)
        let (read, found) = try Cells.decode(bytes)
        #expect(found == encoding, "\(encoding)")
        #expect(read == text, "\(encoding)")
        #expect(try Cells.encode(read, found) == bytes, "\(encoding)")
    }
}

@Test func utf16WithoutAMarkIsFoundByItsZeroes() throws {
    let bytes = try Cells.encode(
        "gig,city\nfirst,Lisbon\n", Cells.Encoding(.utf16LittleEndian, bom: false))
    let (text, found) = try Cells.decode(bytes)
    #expect(found == Cells.Encoding(.utf16LittleEndian, bom: false))
    #expect(text == "gig,city\nfirst,Lisbon\n")
}

@Test func bytesThatAreNoTextAreRefusedByReason() {
    // 0x81 is undefined in Windows-1252, and a lone continuation byte in UTF-8
    let bytes = Data([0x61, 0x2C, 0x62, 0x0A, 0x81, 0x2C, 0x63, 0x0A])
    #expect(throws: Cells.Unreadable.self) { try Cells.decode(bytes) }
    // a Cells is UTF-8 alone
    let latin = try! Cells.encode("café\n", .windows1252)
    #expect(throws: Cells.Unreadable.self) { try Cells.decode(latin, accepting: [.utf8]) }
    #expect(throws: Cells.Unwritable.self) { try Cells.encode("日本\n", .windows1252) }
}

@Test func theByteOrderMarkIsNoPartOfTheHeader() {
    let doc = Cells.Document(reading: "\u{FEFF}seed,qty\nbasil,3\n")
    #expect(doc.columns.map(\.name) == ["seed", "qty"])
}

@Test func semicolonsTabsAndPipesAreDetectedAndKept() {
    for (text, delimiter) in [
        ("item;cost\nfuel;84,20\n", Character(";")), ("item\tcost\nfuel\t84.20\n", "\t"),
        ("item|cost\nfuel|84.20\n", "|"),
    ] {
        let doc = Cells.Document(reading: text, as: .plain())
        #expect(doc.layout.delimiter == delimiter)
        #expect(doc.dialect == .plain(delimiter: delimiter))
        #expect(doc.text == text)
    }
    // a header's choices hold pipes no row does: the lines agree on the comma
    #expect(
        Cells.Document(reading: "seed:basil|leek|kale,g:num\nleek,3\n", as: .plain()).layout
            .delimiter == ",")
    // a .tsv of one column holds no tab: the delimiter is given
    #expect(Cells.Document(reading: "a,b\n1,2\n", as: .plain(delimiter: "\t")).columns.count == 1)
}

@Test func aPlainFileIsNeverRefusedForItsRecords() {
    let doc = Cells.Document(
        reading: "item,size,note\npipe,5\" long\n\"ab\"c,d,e,f\n", as: .plain())
    #expect(doc.broken.isEmpty)
    #expect(doc.records.map(\.cells) == [["pipe", "5\" long", ""], ["abc", "d", "e"]])
    #expect(doc.problems.map(\.kind) == [.ragged, .ragged])
    #expect(doc.problems.map(\.at) == [.cell(row: 0, column: 2), .cell(row: 1, column: 2)])
    #expect(doc.value(row: 1, column: 2) == .success(.text("e")))
    #expect(doc.width(row: 1) == 4)
    // the same text as Cells ends its rows at the stray quote
    let strict = Cells.Document(reading: doc.text)
    #expect(strict.broken.map(\.at) == [.record(row: 0)] && strict.rows == 0)
}

@Test func aFileThatQuotesEveryFieldIsWrittenSo() throws {
    var doc = Cells.Document(
        reading: "\"gig\",\"fee\"\n\"Porto\",\"300\"\n\"Braga\",\"250\"\n", as: .plain())
    #expect(doc.layout.quotesAll)
    try doc.set(row: 1, column: 1, to: "275")
    #expect(doc.text == "\"gig\",\"fee\"\n\"Porto\",\"300\"\n\"Braga\",\"275\"\n")
}
