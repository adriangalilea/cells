import Testing

@testable import Cells

// The prelude as statements the engine read, and its edits as the text a host reads again
// with the imports a new line names.

let money = """
    ---
    type Money = EUR>0 // what moves
    rates = run every day at 18:00 in Europe/Madrid
    ```ts
    export default () => []
    ```
    vat: EUR = amount * 21 / 121
    ---
    date,amount:Money,vat
    2026-11-06,3.80,

    """

@Test func readsEachStatement() throws {
    let doc = Cells.Document(reading: money)
    let s = doc.statements
    #expect(s.count == 3, "\(doc.broken)")
    #expect(s[0].kind == .type(name: "Money", spelled: "EUR>0"))
    #expect(s[0].text == "type Money = EUR>0 // what moves")
    #expect(s[0].lines == 0..<1)
    guard case .run(let name, let schedule, let language, let code, _) = s[1].kind else {
        Issue.record("\(s[1])")
        return
    }
    #expect(name == "rates" && schedule == "every day at 18:00 in Europe/Madrid")
    #expect(language == "ts" && code == "export default () => []\n")
    #expect(s[1].lines == 1..<5)
    #expect(s[1].codeLines == 3..<4)
    #expect(s[2].kind == .column(name: "vat", spelled: "EUR=amount * 21 / 121"))
}

@Test func aScriptsScheduleAndCodeEachEditedAlone() throws {
    let doc = Cells.Document(reading: money)
    let run = doc.statements[1]
    let head = run.lines.lowerBound..<run.lines.lowerBound + 1
    let rescheduled = Cells.Document(
        reading: doc.text(.replace(head, with: doc.written(.run("rates", schedule: "every hour")))))
    guard case .run(_, let schedule, _, let code, _) = rescheduled.statements[1].kind else {
        Issue.record("\(rescheduled.broken)")
        return
    }
    #expect(schedule == "every hour" && code == "export default () => []\n")
    let recoded = Cells.Document(
        reading: doc.text(.replace(run.codeLines!, with: "export default () => [{ a: 1 }]")))
    guard case .run(_, let kept, _, let new, _) = recoded.statements[1].kind else {
        Issue.record("\(recoded.broken)")
        return
    }
    #expect(
        kept == "every day at 18:00 in Europe/Madrid" && new == "export default () => [{ a: 1 }]\n")
    #expect(doc.written(.type("Kind", spelled: "a|b", exported: true)) == "export type Kind = a|b")
}

@Test func editsAreTextReadAgain() throws {
    let doc = Cells.Document(reading: money)
    let line = doc.written(.import("accounts", from: "accounts.cells"))
    #expect(line == "import accounts from \"accounts.cells\"")
    let added = Cells.Document(
        reading: doc.text(.add(line)), at: "money.cells",
        files: ["accounts.cells": "account:text!\ncoffee\n"])
    #expect(added.statements.count == 4)
    #expect(
        added.statements[3].kind
            == .import(name: "accounts", names: [], from: "accounts.cells", sha256: nil))
    let removed = Cells.Document(reading: added.text(.remove(added.statements[1])))
    #expect(removed.statements.map(\.name) == ["Money", "vat", "accounts"])
    let retyped = Cells.Document(
        reading: doc.text(
            .replace(doc.statements[0].lines, with: doc.written(.type("Money", spelled: "EUR>=0"))))
    )
    #expect(retyped.statements[0].kind == .type(name: "Money", spelled: "EUR>=0"))
    #expect(retyped.records.count == 1)
    // a file without a prelude gets one
    let bare = Cells.Document(reading: "a:int\n1\n")
    #expect(bare.text(.add("type A = int")) == "---\ntype A = int\n---\na:int\n1\n")
}
