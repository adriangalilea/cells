// A Cells file's script read as TypeScript reads it: TypeScript's own language service
// (`TypeScript/typescript.js`, the npm package's, fetched by swift/scripts/typescript.sh) run in
// JavaScriptCore inside the process, so a sandboxed app completes a script with no node and
// no child process. `Script/service.js` is its host: the script as one file in memory, the
// ES2022 library and `Script/cells-script.d.ts` (what a script sees of the web, and the
// argument it is called with), and the script's table's columns as the type of a row.
//
// Loaded on the first question (about 90 ms), the first completion about 230 ms, each one
// after a few milliseconds (measured on an M-series Mac, TypeScript 6.0.3).

import Foundation
import JavaScriptCore

/// One completion: what to show, what kind of thing it is, and the text it replaces.
public struct ScriptCompletion: Sendable, Equatable {
    /// What TypeScript calls the thing (`function`, `method`, `property`, `const`, `keyword`,
    /// `interface`, …), its `ScriptElementKind`.
    public let kind: String
    public let label: String
    /// Said beside it: TypeScript's modifiers of it (`declare`, `optional`, `deprecated`),
    /// none when it has none.
    public let detail: String?
    /// The text that goes in, the label unless TypeScript says otherwise.
    public let insert: String
    /// The UTF-16 offsets of the script's text it replaces (empty: inserted at the place).
    public let replace: Range<Int>
}

public enum TypeScriptServiceError: Error, CustomStringConvertible {
    /// A file the service is built from is not in the bundle.
    case missing(String)
    /// The service's JavaScript threw.
    case script(String)
    /// The service answered with something that is not its answer.
    case answer(String)

    public var description: String {
        switch self {
        case .missing(let name): "CellsScripts:\(name) is not in the bundle"
        case .script(let message): "CellsScripts:the language service threw: \(message)"
        case .answer(let text): "CellsScripts:not the service's answer: \(text.prefix(200))"
        }
    }
}

/// TypeScript's language service for one script at a time.
///
///     let service = TypeScriptService()
///     let items = try await service.completions(code: code, at: caret, columns: ["date", "rate"])
public actor TypeScriptService {
    /// `service.js`'s `cellsService`: its `complete`, `details` and `hover`.
    private var service: JSValue?
    /// What the JavaScript last threw, written by the context's handler during the call
    /// that threw (on this actor), read right after it.
    private let thrown = Thrown()
    /// Kept while the service lives: its values belong to it.
    private var context: JSContext?

    private final class Thrown: @unchecked Sendable {
        var message: String?
    }

    public init() {}

    /// What may be written at `offset` (UTF-16) of `code`, a script whose table's columns are
    /// `columns`, each a string field of the rows it is called with and gives back. In
    /// TypeScript's order of relevance.
    public func completions(code: String, at offset: Int, columns: [String]) throws
        -> [ScriptCompletion]
    {
        let length = (code as NSString).length
        let json = try call("complete", code: code, at: offset, columns: columns)
        guard let rows = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [[Any]]
        else { throw TypeScriptServiceError.answer(json) }
        let items = try rows.map { row -> (String, ScriptCompletion) in
            guard row.count == 7, let label = row[0] as? String, let kind = row[1] as? String,
                let modifiers = row[2] as? String, let sort = row[3] as? String,
                let from = row[4] as? Int, let to = row[5] as? Int, let insert = row[6] as? String
            else { throw TypeScriptServiceError.answer("\(row)") }
            assert(
                0 <= from && from <= to && to <= length,
                "\(label) replaces \(from)..<\(to) of \(length)")
            return (
                sort,
                ScriptCompletion(
                    kind: kind, label: label, detail: modifiers.isEmpty ? nil : modifiers,
                    insert: insert.isEmpty ? label : insert, replace: from..<to)
            )
        }
        // TypeScript's sort text first (locals, then members, then globals), then the name
        return items.sorted { $0.0 == $1.0 ? $0.1.label < $1.1.label : $0.0 < $1.0 }.map(\.1)
    }

    /// What the completion named `label` offered at `offset` is: its signature as TypeScript
    /// writes it (`(method) Body.json(): Promise<any>`), then what its documentation says on
    /// the lines after; nothing where TypeScript says nothing.
    public func details(code: String, at offset: Int, columns: [String], label: String) throws
        -> String?
    {
        let text = try call("details", code: code, at: offset, columns: columns, label)
        return text.isEmpty ? nil : text
    }

    /// What is at `offset` of `code`, as TypeScript says it (its signature, then its
    /// documentation), and the UTF-16 range of the code it speaks of; nothing where there
    /// is nothing to say.
    public func hover(code: String, at offset: Int, columns: [String]) throws -> (
        text: String, range: Range<Int>
    )? {
        let length = (code as NSString).length
        let json = try call("hover", code: code, at: offset, columns: columns)
        if json == "null" { return nil }
        guard let row = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [Any],
            row.count == 3, let text = row[0] as? String, let from = row[1] as? Int,
            let to = row[2] as? Int
        else { throw TypeScriptServiceError.answer(json) }
        assert(0 <= from && from <= to && to <= length, "a hover over \(from)..<\(to) of \(length)")
        return (text, from..<to)
    }

    /// `service.js`'s function `name` asked of `code` at `offset`, the script's table's
    /// `columns` handed over as JSON, `more` after them; what it answers, a string.
    private func call(
        _ name: String, code: String, at offset: Int, columns: [String], _ more: Any...
    ) throws -> String {
        let length = (code as NSString).length
        precondition(
            (0...length).contains(offset), "offset \(offset) outside a script of \(length)")
        let service = try loaded()
        let names = String(
            decoding: try JSONSerialization.data(withJSONObject: columns), as: UTF8.self)
        thrown.message = nil
        let answer = service.invokeMethod(name, withArguments: [code, offset, names] + more)
        if let message = thrown.message { throw TypeScriptServiceError.script(message) }
        guard let answer, answer.isString, let text = answer.toString() else {
            throw TypeScriptServiceError.answer(answer?.toString() ?? "nothing")
        }
        return text
    }

    /// The service, made on the first question: TypeScript evaluated, its libraries and the
    /// script's declarations handed to the host, the host evaluated.
    private func loaded() throws -> JSValue {
        if let service { return service }
        guard let context = JSContext() else { throw TypeScriptServiceError.script("no JSContext") }
        let thrown = self.thrown
        thrown.message = nil
        context.exceptionHandler = { _, exception in
            thrown.message = exception?.toString() ?? "an exception"
        }
        let typescript = try resource("typescript", "js", in: "TypeScript")
        context.evaluateScript(
            try String(contentsOf: typescript, encoding: .utf8), withSourceURL: typescript)
        if let message = thrown.message { throw TypeScriptServiceError.script(message) }
        guard context.objectForKeyedSubscript("ts")?.isObject == true else {
            throw TypeScriptServiceError.script("typescript.js defined no `ts`")
        }
        var libs: [String: String] = [:]
        let folder = typescript.deletingLastPathComponent()
        for name in try FileManager.default.contentsOfDirectory(atPath: folder.path)
        where name.hasPrefix("lib.") && name.hasSuffix(".d.ts") {
            libs[name] = try String(
                contentsOf: folder.appendingPathComponent(name), encoding: .utf8)
        }
        assert(
            libs["lib.es2022.d.ts"] != nil && libs["lib.es5.d.ts"] != nil,
            "the ES2022 chain is missing")
        let ambient = try resource("cells-script", "d.ts", in: "Script")
        libs["cells-script.d.ts"] = try String(contentsOf: ambient, encoding: .utf8)
        context.setObject(libs, forKeyedSubscript: "cellsLibs" as NSString)
        let host = try resource("service", "js", in: "Script")
        context.evaluateScript(try String(contentsOf: host, encoding: .utf8), withSourceURL: host)
        if let message = thrown.message { throw TypeScriptServiceError.script(message) }
        guard let service = context.objectForKeyedSubscript("cellsService"), service.isObject,
            ["complete", "details", "hover"].allSatisfy({
                service.objectForKeyedSubscript($0)?.isUndefined == false
            })
        else {
            throw TypeScriptServiceError.script(
                "service.js defined no `cellsService` with complete, details and hover")
        }
        self.context = context
        self.service = service
        return service
    }

    private func resource(_ name: String, _ ext: String, in folder: String) throws -> URL {
        guard
            let url = Bundle.module.url(forResource: name, withExtension: ext, subdirectory: folder)
        else { throw TypeScriptServiceError.missing("\(folder)/\(name).\(ext)") }
        return url
    }
}
