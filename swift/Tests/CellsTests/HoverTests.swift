import Foundation
import Testing

@testable import Cells

// What is said of a place, the engine's, its offsets UTF-16.

@Test func aFormulasColumnSaid() throws {
    let text = "item: text!,cost: EUR>=0,qty: int,total: EUR = cost * qty\nfuel,84.20,1,84.20=\n"
    let doc = Cells.Document(reading: text)
    let at = (text as NSString).range(of: "cost *").location + 1
    let h = try #require(doc.hover(in: text, at: at))
    #expect(h.text == "cost: EUR>=0")
    #expect(h.range == at - 1..<at + 3)
}

@Test func aFunctionsCallAndWhatItGives() throws {
    let text = "😀:text,cost:EUR,total:EUR=ROUND(cost, 1)\nx,84.20,84.20=\n"
    let doc = Cells.Document(reading: text)
    let at = (text as NSString).range(of: "ROUND").location + 2
    let h = try #require(doc.hover(in: text, at: at))
    #expect(h.text.hasPrefix("ROUND(x, digits)\n"))
    // past a character two UTF-16 units wide, the range is still the word's
    #expect(h.range == at - 2..<at + 3)
    // and a function's completion carries the same line
    let complete = doc.completions(in: text, at: at)
    #expect(complete.contains { $0.label == "ROUND" && $0.detail.hasPrefix("ROUND(x, digits): ") })
}

@Test func aNamedTypeInThePrelude() throws {
    let text = "---\ntype Money = EUR>=0\n---\ncost:Money\n1.00\n"
    let doc = Cells.Document(reading: text)
    let h = try #require(
        doc.hover(in: text, at: (text as NSString).range(of: "Money =").location + 2))
    #expect(h.text == "type Money = EUR>=0")
    #expect(doc.hover(in: text, at: 1) == nil)
}
