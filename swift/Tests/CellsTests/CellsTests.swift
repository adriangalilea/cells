import Foundation
import Testing

@testable import Cells

// The binding's own laws, beside the suite: the Swift surface over the engine (types as
// values, problems by kind and place, copies that never share an edit, rows moved, columns
// read in place, the spans a highlighter paints).

let budget = """
  item:text!,category:travel|stay|food|gear,cost:EUR>=0,paid:yes/no,on:date
  fuel,travel,84.20,yes,2026-08-14
  groceries,food,96.75,no,2026-08-15

  """

@Test func readsTheHeaderTypes() throws {
  let doc = Cells.Document(reading: budget)
  #expect(doc.columns.map(\.name) == ["item", "category", "cost", "paid", "on"])
  #expect(doc.columns[0].type == Cells.ColumnType(base: .text, key: true))
  #expect(doc.columns[1].type?.base == .choice(["travel", "stay", "food", "gear"]))
  #expect(
    doc.columns[2].type
      == Cells.ColumnType(base: .money(currency: "EUR"), bounds: .init(min: 0))
  )
  #expect(doc.columns[3].type?.base == .bool(true: "yes", false: "no"))
  #expect(doc.columns[4].type?.base == .date(format: nil))
  #expect(doc.problems.isEmpty, "\(doc.problems)")
  #expect(
    doc.value(row: 0, column: 2) == .success(.money(Decimal(string: "84.20")!, currency: "EUR"))
  )
  #expect(doc.value(row: 1, column: 3) == .success(.bool(false)))
  #expect(
    doc.value(row: 0, column: 4)
      == .success(.date(DateComponents(year: 2026, month: 8, day: 14))))
}

@Test func aChoicesTintsTravelWithItsType() throws {
  let tinted = try Cells.parseType("income#green|cost|refund#3fa34d")
  #expect(tinted.tints == ["income": .named(.green), "refund": .hex(0x3fa34d)])
  #expect(Cells.shorthand(tinted) == "income#green|cost|refund#3fa34d")
  let doc = Cells.Document(
    reading: "---\ntype Kind = income#green|cost\n---\nkind:Kind?\nincome\n")
  #expect(doc.names["Kind"]?.tints == ["income": .named(.green)])
  #expect(doc.type(of: 0).tints == ["income": .named(.green)])
  #expect(doc.tint(of: "income", in: 0) == .named(.green))
  #expect(Cells.shorthand(doc.type(of: 0)) == "Kind?")
}

@Test func shorthandAndExplicitMeanTheSame() throws {
  let pairs: [(String, String)] = [
    ("text!", "text key"),
    ("text?", "text optional"),
    ("travel|stay", "one of travel stay"),
    ("yes/no", "bool yes no"),
    ("EUR>=0", "EUR min 0"),
    ("0..100", "num between 0 and 100"),
  ]
  for (short, long) in pairs {
    #expect(try Cells.parseType(short) == Cells.parseType(long), "\(short) vs \(long)")
  }
}

@Test func quotedChoicesHoldReservedCharacters() throws {
  let t = try Cells.parseType("'yay!'|meh|'so-so?'|'it''s'")
  #expect(t.base == .choice(["yay!", "meh", "so-so?", "it's"]))
  #expect(t.key == false && t.optional == false)
}

@Test func unknownWordsAreNamed() {
  #expect(throws: Cells.TypeError.self) { try Cells.parseType("EURO") }
  #expect(throws: Cells.TypeError.self) { try Cells.parseType("a,b") }
}

@Test func shorthandIsTheEnginesSpelling() throws {
  for text in [
    "text!", "travel|stay|'sd cards'", "yes/no", "EUR>=0", "0..100", "date(DD/MM/YYYY)", "int?",
    "text<=40", "num>0<1", "id!",
  ] {
    #expect(Cells.shorthand(try Cells.parseType(text)) == text, "\(text)")
  }
  let named = Cells.Document(reading: "---\ntype Price = EUR>=0\n---\ncost:Price?\n")
  #expect(Cells.shorthand(named.columns[0].type!, names: named.names) == "Price?")
}

@Test func writesBackByteForByte() {
  let quirky = "a:int,b\r\n1,\"x, y\"\r\n2,\"say \"\"hi\"\"\"\r\n"
  #expect(Cells.Document(reading: quirky).text == quirky)
  #expect(Cells.Document(reading: budget).text == budget)
}

/// The standard layout as `cells fmt` writes it: aligned, or with no padding, each typed
/// header cell `name: type = formula` however it was spaced, and formatting twice is
/// formatting once.
@Test func formattedIsTheStandardLayout() {
  let doc = Cells.Document(reading: "price:num,total:num?=price * 2\n36.8,73.6=\n1234.5,2469=\n")
  let aligned = "price: num, total: num? = price * 2\n36.8,       73.6=\n1234.5,     2469=\n"
  #expect(doc.formatted() == aligned)
  #expect(Cells.Document(reading: aligned).formatted() == aligned)
  #expect(
    Cells.Document(reading: aligned).formatted(aligned: false)
      == "price: num,total: num? = price * 2\n36.8,73.6=\n1234.5,2469=\n")
}

@Test func anEditRewritesOnlyItsRecord() throws {
  var doc = Cells.Document(reading: "n:int,s\n1,\"kept as written\"\n2,b\n")
  try doc.set(row: 1, column: 1, to: "c, d")
  #expect(doc.text == "n:int,s\n1,\"kept as written\"\n2,\"c, d\"\n")
  try doc.set(row: 0, column: 0, value: .int(7))
  #expect(doc.text == "n:int,s\n7,\"kept as written\"\n2,\"c, d\"\n")
}

@Test func aCopyNeverSharesAnEdit() throws {
  let read = Cells.Document(reading: budget)
  var edited = read
  try edited.set(row: 0, column: 0, to: "diesel")
  #expect(read.text == budget)
  #expect(edited.records[0].cells[0] == "diesel")
  #expect(read != edited)
  // A view of the columns stays the version it was read from.
  let before = read.values(column: 2)
  var later = read
  try later.set(row: 0, column: 2, to: "1.00")
  guard case .decimal(let cost) = before else {
    Issue.record("cost is decimal")
    return
  }
  #expect(cost[0] == Decimal(string: "84.20"))
}

@Test func brokenCellsAreProblemsByKindAndPlace() {
  let doc = Cells.Document(
    reading:
      "item:text!,cost:num>=0,paid:yes/no\nfuel,84.20,yes\ntolls,8x.50,yes\ncabin,310,maybe\nfuel,1,\n"
  )
  #expect(doc.rows == 4)
  let found = doc.problems.map { "\($0.at) \($0.kind)" }
  #expect(
    found == [
      "cell(row: 0, column: 0) key", "cell(row: 1, column: 1) type",
      "cell(row: 2, column: 2) type", "cell(row: 3, column: 0) key",
      "cell(row: 3, column: 2) missing",
    ], "\(found)")
  #expect(doc.describe(doc.problems[1]) == "row 2, cost: \"8x.50\" is not a number")
  #expect(doc.value(row: 1, column: 1) == .failure(doc.problems[1]))
}

@Test func aBrokenGrammarTakesOutOnlyWhatItBreaks() {
  // a record that does not read ends the rows, its bytes kept
  let text = "a:int,b\n1,2\n\"3,4\n"
  let doc = Cells.Document(reading: text)
  #expect(doc.broken.map(\.at) == [.record(row: 1)])
  #expect(doc.broken[0].kind == .grammar)
  #expect(doc.rows == 1 && doc.columns.count == 2)
  #expect(doc.text == text)
  // a header cell that does not read is a column of text, the others typed
  let typo = Cells.Document(reading: "cost:EURO,qty:int\n1,2\n")
  #expect(typo.broken.map(\.at) == [.header(column: 0)])
  #expect(typo.broken[0].message == "cost: no type or currency \"EURO\"")
  #expect(typo.value(row: 0, column: 0) == .success(.text("1")))
  #expect(typo.value(row: 0, column: 1) == .success(.int(2)))
  #expect(Cells.Document(reading: Data([0x61, 0xFF, 0x0A])).broken.map(\.at) == [.file])
}

@Test func namedTypesFromThePreludeAndAFormulaInTheHeader() {
  let doc = Cells.Document(
    reading:
      "---\ntype Price = EUR>=0 // a price\n---\nitem,cost:Price,qty:int,total:EUR=cost * qty\nfuel,84.20,2,\nbad,-1,1,\n"
  )
  #expect(doc.names["Price"]?.base == .money(currency: "EUR"))
  #expect(doc.columns[1].type?.base == .named("Price", .money(currency: "EUR")))
  // the broken cost, and the total its formula gives that the file does not store yet
  #expect(doc.problems.map(\.kind) == [.unstored, .type], "\(doc.problems)")
  #expect(doc.columns[3].formula == "cost * qty")
  #expect(
    doc.value(row: 0, column: 3)
      == .success(.money(Decimal(string: "168.40")!, currency: "EUR")))
  // a broken input makes the computed cell blank, never missing
  #expect(doc.value(row: 1, column: 3) == .success(.missing))
  #expect(doc.prelude == ["---", "type Price = EUR>=0 // a price", "---"])
}

@Test func untypedColumnsInferThePlainestKind() {
  let doc = Cells.Document(
    reading: "a,b,c,d,e\n42,1.5,yes,2026-08-14,x\n7,2,no,2026-08-15,A12\n")
  #expect(doc.type(of: 0).base == .int)
  #expect(doc.type(of: 1).base == .num)
  #expect(doc.type(of: 2).base == .bool(true: "yes", false: "no"))
  #expect(doc.type(of: 3).base == .date(format: nil))
  #expect(doc.type(of: 4).base == .text)
  #expect(doc.type(of: 0).optional)
  #expect(doc.columns.allSatisfy { $0.type == nil })
  #expect(doc.problems.isEmpty, "\(doc.problems)")
}

@Test func aTypeChangedIsWrittenInShorthandAndChecked() throws {
  var doc = Cells.Document(reading: "n,s\n1,a\nx,b\n")
  try doc.setType(Cells.parseType("int min 0"), column: 0)
  #expect(doc.text == "n: int>=0,s\n1,a\nx,b\n")
  #expect(doc.problems.map(\.at) == [.cell(row: 1, column: 0)])
  try doc.setType(nil, column: 0)
  #expect(doc.text == "n,s\n1,a\nx,b\n")
  var computed = Cells.Document(reading: "n:int,t:int=n * 2\n1,\n")
  #expect(throws: Cells.TypeError.self) { try computed.setType(.init(base: .text), column: 0) }
}

@Test func rowsMovedAddedAndTakenOut() {
  var doc = Cells.Document(reading: "k:text!,n:int\na,1\nb,2\n\"c, d\",3\n")
  doc.moveRows(fromOffsets: [2], toOffset: 0)
  #expect(doc.text == "k:text!,n:int\n\"c, d\",3\na,1\nb,2\n")
  doc.insertRows([["e", "5"], ["", ""]], at: 1)
  #expect(doc.text == "k:text!,n:int\n\"c, d\",3\ne,5\n,\na,1\nb,2\n")
  #expect(doc.problems.map(\.kind) == [.missing, .missing])
  doc.removeRows([2])
  #expect(doc.records.map { $0.cells[0] } == ["c, d", "e", "a", "b"])
}

@Test func aDocumentWrittenFromItsParts() {
  let doc = Cells.Document(
    columns: [
      Cells.Column(name: "item", type: .init(base: .text, key: true)),
      Cells.Column(name: "price, net", type: .init(base: .money(currency: "EUR"))),
    ],
    records: [Cells.Record(cells: ["fuel", "84.20"])])
  #expect(doc.text == "item: text!,\"price, net: EUR\"\nfuel,84.20\n")
}

@Test func columnsReadInPlace() {
  let doc = Cells.Document(
    reading:
      "n:int,x:num?,b:bool,on:date,at:datetime,c:a|b,t:text?\n1,2.5,true,1970-01-02,1970-01-01T00:00:01.5Z,b,hi\n2,,false,2026-08-14,2026-08-14T12:30:00Z,a,\n"
  )
  guard case .int(let n) = doc.values(column: 0), case .decimal(let x) = doc.values(column: 1),
    case .bool(let b) = doc.values(column: 2), case .date(let on) = doc.values(column: 3),
    case .instant(let at) = doc.values(column: 4), case .choice(let c) = doc.values(column: 5),
    case .text(let t) = doc.values(column: 6)
  else {
    Issue.record("each column in its layout")
    return
  }
  #expect(Array(n) == [1, 2])
  #expect(x[0] == Decimal(string: "2.5") && x[1] == nil && x.double(0) == 2.5)
  #expect(Array(b) == [true, false])
  #expect(Array(on) == [1, 20679])
  #expect(at[0] == Date(timeIntervalSince1970: 1.5))
  #expect(c.words == ["a", "b"] && Array(c) == ["b", "a"])
  #expect(Array(t) == ["hi", nil])
}

@Test func theEnginesSpans() {
  let text = "---\ntype C = a|b // why\n---\nk:C!,n:int\na,1\n"
  let spans = Cells.tokens(text)
  let utf8 = Array(text.utf8)
  let shown = spans.map { (String(decoding: utf8[$0.utf8], as: UTF8.self), $0.kind, $0.column) }
  #expect(shown.contains { $0 == ("// why", .comment, nil) })
  #expect(shown.contains { $0 == ("C", .type, 0) })
  #expect(shown.contains { $0 == ("!", .mark, 0) })
  #expect(shown.contains { $0 == ("a", .cellChoice, 0) })
  #expect(shown.contains { $0 == ("1", .cellNumber, 1) })
  #expect(Cells.Token.Kind.allCases.count == 25)
  let tinted = "---\ntype K = a#green|b\n---\nk:K\na\n"
  let found = Cells.tokens(tinted).map {
    (String(decoding: Array(tinted.utf8)[$0.utf8], as: UTF8.self), $0.kind)
  }
  #expect(found.contains { $0 == ("green", .tint) }, "a choice's tint is its own span")
}

// How values look (SPEC.md, Looks) as the binding hands them to a table: a choice's tinted
// words, a looked cell's drawn text and tint, a scale's point, a row's tint, in a locale.
@Test func looksAreTheEnginesInALocale() {
  let doc = Cells.Document(
    reading: """
      ---
      type Kind = income#green|cost#red|pending
      look amount: grouped, negative red, positive green
      look row: gray when kind = 'pending'
      ---
      kind:Kind,amount:EUR
      income,1234.50
      cost,-12.00
      pending,0.00

      """)
  let looks = doc.looks(in: .english)
  #expect(looks.words[0] == ["income": .named(.green), "cost": .named(.red)])
  #expect(looks.drawn(row: 0, column: 1) == "1,234.50")
  #expect(looks.tint(row: 0, column: 1, word: nil) == .one(.named(.green)))
  #expect(looks.tint(row: 1, column: 1, word: nil) == .one(.named(.red)))
  #expect(looks.tint(row: 0, column: 0, word: "income") == .one(.named(.green)))
  #expect(looks.tint(row: 2) == .named(.gray))
  #expect(looks.tint(row: 0) == nil)
  #expect(doc.tint(of: "cost", in: 0) == .named(.red))
  #expect(doc.looks(of: 1) == ["grouped", "negative red", "positive green"])
  // a locale of its own: the grouping and decimal marks are the place's
  let german = doc.looks(in: Cells.Locale(Foundation.Locale(identifier: "de_DE")))
  #expect(german.drawn(row: 0, column: 1) == "1.234,50")
  #expect(Cells.Tint.hex(0x3fa34d).spelled == "#3fa34d")
}

@Test func decodeReadsOneCellByAType() throws {
  #expect(
    Cells.decode("84.20", try Cells.parseType("EUR>=0"))
      == .success(.money(Decimal(string: "84.2")!, currency: "EUR")))
  if case .failure(let m) = Cells.decode("-1", try Cells.parseType("EUR>=0")) {
    #expect(m.message == "\"-1\" is below 0")
  } else {
    Issue.record("-1 breaks EUR>=0")
  }
}
