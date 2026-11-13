import Foundation
import Testing

@testable import Cells

// A file's rows and columns changed, and its types written for a Cells.

@Test func columnsAreInsertedMovedRenamedAndRemoved() throws {
    var doc = Cells.Document(reading: "seed,qty:int\nbasil,3\nleek,12\n")
    try doc.insertColumn(Cells.Column(name: "bed"), at: 1, cells: ["north", "south"])
    #expect(doc.text == "seed,bed,qty:int\nbasil,north,3\nleek,south,12\n")
    doc.moveColumn(from: 2, to: 0)
    #expect(doc.text == "qty:int,seed,bed\n3,basil,north\n12,leek,south\n")
    try doc.rename(column: 2, to: "plot")
    #expect(doc.text == "qty:int,seed,plot\n3,basil,north\n12,leek,south\n")
    let (column, cells) = try doc.removeColumn(at: 0)
    #expect(column.type?.base == .int && cells == ["3", "12"])
    #expect(doc.text == "seed,plot\nbasil,north\nleek,south\n")
    doc.insertRows([["chard", "east"]], at: 1)
    doc.removeRows([0])
    #expect(doc.text == "seed,plot\nchard,east\nleek,south\n")
}

@Test func aColumnChangeTheFileCannotHoldIsRefused() {
    let doc = Cells.Document(reading: "seed,cost:EUR,total:EUR=cost * 2\nbasil,2.50,\n")
    var renamed = doc
    #expect(throws: Cells.Problem.self) { try renamed.rename(column: 0, to: "cost") }
    #expect(renamed.text == doc.text)
    var removed = doc
    #expect(throws: Cells.Problem.self) { try removed.removeColumn(at: 1) }
    // a plain file's columns are names alone, so two may share one
    var plain = Cells.Document(reading: "seed;qty\nbasil;3\n", as: .plain())
    try? plain.rename(column: 1, to: "seed")
    #expect(plain.text == "seed;seed\nbasil;3\n")
}

@Test func twoColumnsOfOneNameAreAProblem() {
    let doc = Cells.Document(reading: "a,a\n1,2\n", as: .plain())
    #expect(
        doc.problems.map { doc.describe($0) } == ["the header, column 2: \"a\" names column 1 too"])
    // in a Cells file the second is a break
    #expect(Cells.Document(reading: "a,a\n1,2\n").broken.map(\.at) == [.header(column: 1)])
}

@Test func typesAreSuggestedForAConversion() {
    let csv = """
        item;category;cost;paid
        fuel;travel;84,20;yes
        tolls;travel;1.023,50;yes
        cabin;stay;310;no
        ferry;travel;58;
        lunch;food;12,5;no
        dinner;food;30;yes

        """
    let doc = Cells.Document(reading: csv, as: .plain())
    #expect(doc.suggestedType(of: 1).base == .choice(["food", "stay", "travel"]))
    #expect(doc.suggestedType(of: 2) == Cells.ColumnType(base: .num))
    #expect(
        doc.suggestedType(of: 3)
            == Cells.ColumnType(base: .bool(true: "yes", false: "no"), optional: true))
    #expect(doc.suggestedValue(row: 1, column: 2, type: doc.suggestedType(of: 2)) == .num(1023.5))
    let typed = doc.withTypesWritten()
    #expect(typed.dialect == .cells && typed.layout.delimiter == ",")
    #expect(typed.problems.isEmpty, "\(typed.problems)")
    #expect(
        typed.text.hasPrefix(
            "item: text,category: food|stay|travel,cost: num,paid: yes/no?\nfuel,travel,84.20,yes\n"))
    // the original is untouched
    #expect(doc.text == csv)
}

@Test func aFormulaIsKeptAsWritten() throws {
    for text in ["cost * qty", "(cost + fee) * qty", "cost - (fee - 2)", "-cost / 2", "cost+fee*2"]
    {
        let doc = Cells.Document(reading: "cost:num,fee:num,qty:int,x:num=\(text)\n1,2,3,\n")
        #expect(doc.columns.map(\.formula) == [nil, nil, nil, text], "\(doc.problems)")
        #expect(doc.origin(row: 0, column: 3) == .formula)
    }
}

@Test func aCellWrittenAsACSVDropsItsTypesAndKeepsItsComputedValues() {
    let doc = Cells.Document(reading: "seed,cost:EUR,qty:int,total:EUR=cost * qty\nbasil,2.50,4,\n")
    #expect(doc.written(delimiter: ";", typed: false) == "seed;cost;qty;total\nbasil;2.50;4;10\n")
    #expect(doc.written(delimiter: ",", typed: true) == doc.text)
}

@Test func aFormulaSetRewritesOnlyItsHeaderCell() throws {
    let text =
        "---\ntype Price = EUR>=0\n---\nseed,cost: Price,qty: int,total: Price = cost * qty\nbasil,2.50,4,\n"
    var doc = Cells.Document(reading: text)
    try doc.setFormula("cost * qty + 1", column: 3)
    // The write stores what the new formula gives beside the cell's `=`.
    #expect(
        doc.text
            == text.replacingOccurrences(of: "= cost * qty\n", with: "= cost * qty + 1\n")
            .replacingOccurrences(of: "basil,2.50,4,\n", with: "basil,2.50,4,11.00=\n"))
    #expect(doc.value(row: 0, column: 3) == .success(.money(11, currency: "EUR")))
    for bad in ["cost * qtty", "seed * 2", "cost *"] {
        var tried = doc
        #expect(throws: Cells.FormulaError.self) { try tried.setFormula(bad, column: 3) }
        #expect(tried.text == doc.text)
    }
    // a written value is the row's override; blank, the formula computes it again
    try doc.set(row: 0, column: 3, to: "12.00")
    #expect(doc.origin(row: 0, column: 3) == .written)
    try doc.set(row: 0, column: 3, to: "")
    #expect(doc.origin(row: 0, column: 3) == .formula)
    // a type changed keeps the formula
    try doc.setType(Cells.ColumnType(base: .money(currency: "EUR")), column: 3)
    #expect(doc.columns[3].formula == "cost * qty + 1")
    try doc.setFormula(nil, column: 3)
    #expect(doc.columns[3].header == "total: EUR")
    var plain = Cells.Document(reading: "seed,cost: EUR\nbasil,2.50\n")
    try plain.insertColumn(
        Cells.Column(
            name: "double", type: .init(base: .money(currency: "EUR")), formula: "cost * 2"),
        at: 2)
    #expect(plain.text == "seed,cost: EUR,double: EUR = cost * 2\nbasil,2.50,5.00=\n")
}

@Test func aCellHoldsItsOwnFormula() throws {
    var doc = Cells.Document(reading: "item:text,cost:EUR\nfuel,32.25=96.75 / 3\nmaps,8.00\n")
    #expect(doc.origin(row: 0, column: 1) == .cell)
    #expect(doc.value(row: 0, column: 1) == .success(.money(32.25, currency: "EUR")))
    try doc.set(row: 1, column: 1, to: "=4 * 2")
    #expect(doc.origin(row: 1, column: 1) == .cell)
    #expect(throws: Cells.Problem.self) { try doc.set(row: 1, column: 0, to: "a\u{7}") }
    #expect(doc.text == "item:text,cost:EUR\nfuel,32.25=96.75 / 3\nmaps,8.00=4 * 2\n")
}

@Test func aStaleCellIsFixedOrKept() throws {
    let head = "item:text,cost:EUR,qty:int,total:EUR=cost * qty\n"
    var doc = Cells.Document(reading: head + "fuel,90.00,1,84.20=\nmaps,4.00,2,8.00=\n")
    #expect(doc.problems.map(\.kind) == [.stale])
    #expect(
        doc.problems.first?.description
            == "row 1, total: the formula gives 90.00, the file stores 84.20")
    guard case .failure(let stale) = doc.value(row: 0, column: 3) else {
        Issue.record("a stale cell has no value")
        return
    }
    #expect(stale.kind == .stale)
    var kept = doc
    doc.fixStale()
    #expect(doc.text == head + "fuel,90.00,1,90.00=\nmaps,4.00,2,8.00=\n")
    #expect(doc.problems.isEmpty)
    try kept.keepStale(row: 0, column: 3)
    #expect(kept.text == head + "fuel,90.00,1,84.20\nmaps,4.00,2,8.00=\n")
    #expect(kept.origin(row: 0, column: 3) == .written)
}

@Test func importsAreFollowedFromTheFilesHeldAndFromTheDisk() throws {
    let accounts = "account:text!\nrent\nbank\n"
    let money =
        "---\nimport accounts from \"../accounts.cells\"\n---\namount:EUR,to:accounts.account\n950,rent\n9,rnet\n"
    let held = Cells.Document(
        reading: money, at: "books/home/money.cells", files: ["books/accounts.cells": accounts])
    #expect(held.problems.map(\.kind) == [.reference])
    #expect(held.problems.first?.description == "row 2, to: \"rnet\" is no key of accounts")
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("cells-\(UUID())")
    try FileManager.default.createDirectory(
        at: dir.appendingPathComponent("home"), withIntermediateDirectories: true)
    try Data(accounts.utf8).write(to: dir.appendingPathComponent("accounts.cells"))
    let at = dir.appendingPathComponent("home/money.cells")
    try Data(money.utf8).write(to: at)
    let disk = Cells.Document(reading: try Data(contentsOf: at), at: at)
    #expect(disk.problems.map(\.kind) == [.reference])
    // read alone, the reference is unchecked
    #expect(Cells.Document(reading: money).problems.isEmpty)
    try FileManager.default.trashItem(at: dir, resultingItemURL: nil)
}
