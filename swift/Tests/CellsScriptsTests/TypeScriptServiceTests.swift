import Foundation
import Testing

@testable import CellsScripts

/// A script as Cells keeps one, completed where `|` stands.
private func ask(
    _ service: TypeScriptService, _ marked: String, columns: [String] = ["date", "rate", "run"]
)
    async throws -> (code: String, items: [ScriptCompletion])
{
    let at = (marked as NSString).range(of: "|").location
    precondition(at != NSNotFound, "no | in the script")
    let code = (marked as NSString).replacingCharacters(
        in: NSRange(location: at, length: 1), with: "")
    return (code, try await service.completions(code: code, at: at, columns: columns))
}

private let script = """
    export default async function ({ last }) {
      const res = await fetch(`https://api.example/usdeur?since=${last.date}`)
      ##
      return []
    }
    """

struct TypeScriptServiceTests {
    let service = TypeScriptService()

    @Test func aResponseCompletesItsMembers() async throws {
        let (_, items) = try await ask(
            service, script.replacingOccurrences(of: "##", with: "res.|"))
        let labels = Set(items.map(\.label))
        #expect(labels.isSuperset(of: ["json", "text", "ok", "status", "headers"]))
        #expect(items.first { $0.label == "json" }?.kind == "method")
    }

    @Test func theRowItIsCalledWithCompletesTheTablesColumns() async throws {
        let (code, items) = try await ask(
            service, script.replacingOccurrences(of: "##", with: "last.ra|"))
        #expect(items.map(\.label).sorted() == ["date", "rate", "run"])
        // what it replaces is the word typed so far, in the code as written
        let rate = try #require(items.first { $0.label == "rate" })
        let replaced = (code as NSString).substring(with: NSRange(rate.replace))
        #expect(replaced == "ra")
    }

    @Test func fetchIsAGlobal() async throws {
        let (code, items) = try await ask(
            service, script.replacingOccurrences(of: "##", with: "fet|"))
        let fetch = try #require(items.first { $0.label == "fetch" })
        #expect(fetch.kind == "function")
        #expect((code as NSString).substring(with: NSRange(fetch.replace)) == "fet")
    }

    @Test func aCallOfFetchCompletesItsOptions() async throws {
        let (_, items) = try await ask(
            service, script.replacingOccurrences(of: "##", with: "await fetch('https://x', { |"))
        #expect(Set(items.map(\.label)).isSuperset(of: ["method", "headers", "body"]))
    }

    @Test func otherColumnsGiveOtherRows() async throws {
        let (_, items) = try await ask(
            service, script.replacingOccurrences(of: "##", with: "last.|"),
            columns: ["day", "close"])
        #expect(items.map(\.label).sorted() == ["close", "day"])
    }

    @Test func aScriptWithNoDefaultExportStillCompletes() async throws {
        let (_, items) = try await ask(service, "const u = new URL('https://x')\nu.|")
        #expect(Set(items.map(\.label)).isSuperset(of: ["host", "pathname", "searchParams"]))
    }

    @Test func aCompletionsDetailsSayItsSignature() async throws {
        let (code, _) = try await ask(service, script.replacingOccurrences(of: "##", with: "res.|"))
        let at =
            (script.replacingOccurrences(of: "##", with: "res.") as NSString)
            .range(of: "res.\n").location + 4
        let said = try #require(
            try await service.details(
                code: code, at: at, columns: ["date", "rate", "run"], label: "json"))
        #expect(said.contains("json(): Promise<"), "\(said)")
        let none = try await service.details(
            code: code, at: at, columns: ["date", "rate", "run"], label: "nothing")
        #expect(none == nil)
    }

    @Test func hoverSaysWhatANameIs() async throws {
        let code = script.replacingOccurrences(of: "##", with: "")
        let at = (code as NSString).range(of: "res =").location + 1
        let (text, range) = try #require(
            try await service.hover(code: code, at: at, columns: ["date", "rate", "run"]))
        #expect(text.contains("const res: Response"), "\(text)")
        #expect(range == at - 1..<at + 2)
        // the row it is called with: a field of the table's columns, through the shadow
        let last = (code as NSString).range(of: "last.date").location + 6
        let (date, _) = try #require(
            try await service.hover(code: code, at: last, columns: ["date", "rate", "run"]))
        #expect(date.contains("date: string"), "\(date)")
        // a blank line (its indentation alone) says nothing
        let line = (code as NSString).range(of: "\n  \n")
        #expect(line.location != NSNotFound)
        let blank = line.location + 2
        #expect(try await service.hover(code: code, at: blank, columns: []) == nil)
    }
}
