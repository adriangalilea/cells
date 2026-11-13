import Foundation
import Testing

@testable import Cells

// What completes at a place, the engine's, its offsets UTF-16.

@Test func aFormulasColumns() throws {
    let text = "item:text!,cost:EUR,total:EUR=co\nfuel,84.20,\n"
    let doc = Cells.Document(reading: text)
    let at = (text as NSString).range(of: "=co").upperBound
    let found = doc.completions(in: text, at: at)
    let cost = try #require(found.first { $0.label == "cost" })
    #expect(cost.kind == .column)
    #expect(cost.range == at - 2..<at)
    #expect(found.contains { $0.label == "SUM" && $0.kind == .function })
}

@Test func thePreludesStatementsAndTypes() throws {
    let text = "---\ntype Money = EUR>=0\nprice: Mo\n---\nprice,cost:EUR\n1,2.00\n"
    let doc = Cells.Document(reading: text)
    let at = (text as NSString).range(of: "Mo\n---").location + 2
    let found = doc.completions(in: text, at: at)
    #expect(found.contains { $0.label == "Money" && $0.kind == .type })
    #expect(found.contains { $0.label == "num" && $0.kind == .keyword })
    let blank = "---\nty\n---\nitem:text!,cost:EUR\nfuel,1.00\n"
    let words = Cells.Document(reading: blank).completions(
        in: blank, at: (blank as NSString).range(of: "ty\n").location + 2)
    #expect(
        Set(["import", "export", "type", "item", "cost"]).isSubset(of: Set(words.map(\.label))))
}

@Test func offsetsCountUTF16() throws {
    // a choice's words offered on a cell after text wider than a byte a character
    let text = "note:text,mood:'😀'|meh\nwide 😀 text,me\n"
    let doc = Cells.Document(reading: text)
    let at = (text as NSString).length - 1
    let found = doc.completions(in: text, at: at)
    let meh = try #require(found.first { $0.label == "meh" })
    #expect(meh.kind == .word)
    #expect(meh.range == at - 2..<at)
}
