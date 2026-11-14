import Foundation
import Testing

@testable import Cells

// The conformance suite (tests/README.md), read where it lives in this repository: every
// case read through this binding, compared and written back, as the engine's own runners
// do, its imports followed through the suite's own files. Prints every case it fails with
// its path.

@Test func conformance() throws {
    let root = URL(filePath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().appending(path: "tests")
    // A `.cells`, or under plain/ a `.csv`, `.tsv` or `.txt`; never an edit's `.out.` file.
    let all = FileManager.default.enumerator(at: root, includingPropertiesForKeys: nil)!
        .compactMap { $0 as? URL }
        .filter { url in
            let plain = url.deletingLastPathComponent().lastPathComponent == "plain"
            let kinds = plain ? ["csv", "tsv", "txt"] : ["cells"]
            return kinds.contains(url.pathExtension)
                && !url.deletingPathExtension().lastPathComponent.hasSuffix(".out")
        }
        .sorted { $0.path < $1.path }
    func relative(_ url: URL) -> String { String(url.path.dropFirst(root.path.count + 1)) }
    // Every `.cells` file by its path in the suite, for a case's imports to follow.
    var files: [String: String] = [:]
    for url in all where url.pathExtension == "cells" {
        files[relative(url)] = String(decoding: try Data(contentsOf: url), as: UTF8.self)
    }
    var failed: [String] = []
    for path in all {
        let bytes = try Data(contentsOf: path)
        let stem = path.deletingPathExtension()
        let edit = stem.appendingPathExtension("edit.json")
        var fails: [String] = []
        let plain = path.pathExtension != "cells"
        let doc =
            plain
            ? Cells.Document(reading: bytes, as: .plain())
            : Cells.Document(reading: bytes, at: relative(path), files: files)
        if FileManager.default.fileExists(atPath: edit.path) {
            let out = try Data(contentsOf: stem.appendingPathExtension("out.\(path.pathExtension)"))
            if let refusal = doc.broken.first {
                fails.append("does not read: \(refusal)")
            } else {
                if Data(doc.text.utf8) != bytes { fails.append("does not round-trip") }
                var edited = doc
                switch apply(&edited, try json(edit) as! [[String: Any]]) {
                case .failure(let why): fails.append(why.reason)
                case .success(let text) where Data(text.utf8) != out:
                    fails.append(
                        "writes \(text.debugDescription), the suite says \(String(decoding: out, as: UTF8.self).debugDescription)"
                    )
                case .success: break
                }
            }
        } else {
            let want =
                try json(path.deletingPathExtension().appendingPathExtension("json"))
                as! [String: Any]
            switch (doc.broken.first { $0.at == .file }, want["error"] as? [String: Any]) {
            case (let refusal?, let error?):
                let at = error["at"] as! String
                if words(refusal.at) != at {
                    fails.append("refused at \(refusal), the suite says \(at)")
                }
            case (let refusal?, nil): fails.append("does not read: \(refusal)")
            case (nil, let error?):
                fails.append("reads, the suite says it breaks at \(error["at"]!)")
            case (nil, nil):
                fails += read(doc, want)
                fails += looked(doc, want)
                fails += charted(doc, want)
                if plain {
                    let ours = String(doc.layout.delimiter)
                    if ours != want["separator"] as? String {
                        fails.append(
                            "separator \(ours.debugDescription), the suite says \(want["separator"] ?? "none")"
                        )
                    }
                }
                if Data(doc.text.utf8) != bytes { fails.append("does not round-trip") }
            }
        }
        if !fails.isEmpty {
            failed.append(
                "FAIL \(relative(path))\n" + fails.map { "    \($0)" }.joined(separator: "\n"))
        }
    }
    print("\(all.count - failed.count) passed, \(failed.count) failed, of \(all.count)")
    #expect(!all.isEmpty, "no case found under \(root.path)")
    #expect(failed.isEmpty, "\(failed.joined(separator: "\n"))")
}

/// How the values look, as the suite says (its README, `tints`, `looks`, `drawn`): each
/// choice's tinted words, each look statement's rules that read, each cell drawn in English and the
/// tint it is drawn in, each row's tint.
private func looked(_ doc: Cells.Document, _ want: [String: Any]) -> [String] {
    var fails: [String] = []
    let looks = doc.looks(in: .english)
    if let theirs = want["tints"] as? [String: [String: String]] {
        // the type's own tints, a named type's through its definition; the looks the same
        var ours: [String: [String: String]] = [:]
        for c in doc.columns.indices where !doc.type(of: c).tints.isEmpty {
            ours[doc.columns[c].name] = doc.type(of: c).tints.mapValues(\.spelled)
            if looks.words[c] != doc.type(of: c).tints {
                fails.append("column \(doc.columns[c].name): looks' tints \(looks.words[c] ?? [:])")
            }
        }
        if ours != theirs { fails.append("tints \(ours), the suite says \(theirs)") }
    }
    if let theirs = want["looks"] as? [String: [String]] {
        var ours: [String: [String]] = [:]
        for s in doc.lookStatements {
            switch s.target {
            case .row: ours["row"] = s.rules
            case .name(let n): ours[n] = s.rules
            }
        }
        if ours != theirs { fails.append("looks \(ours), the suite says \(theirs)") }
    }
    // A tint as the suite writes it: its spelling, or a scale's point.
    func said(_ t: Cells.Tinted?) -> String {
        switch t {
        case nil: "none"
        case .one(let t): t.spelled
        case .between(let from, let to, let at): "\(from.spelled)..\(to.spelled) at \(at)"
        }
    }
    func theirs(_ t: Any?) -> String {
        switch t {
        case let s as String: s
        case let p as [String: Any]: "\(p["from"]!)..\(p["to"]!) at \(p["at"]!)"
        default: "none"
        }
    }
    for d in want["drawn"] as? [[String: Any]] ?? [] {
        let r = d["row"] as! Int
        guard let name = d["column"] as? String else {
            let ours = looks.tint(row: r).map { said(.one($0)) } ?? "none"
            if ours != theirs(d["tint"]) {
                fails.append("row \(r) drawn in \(ours), the suite says \(theirs(d["tint"]))")
            }
            continue
        }
        let c = doc.columns.firstIndex { $0.name == name }!
        // a column with no look is drawn as the file writes its values
        let text = looks.drawn(row: r, column: c) ?? doc.text(row: r, column: c)
        if let want = d["text"] as? String, text != want {
            fails.append(
                "row \(r), \(name) drawn \(text.debugDescription), the suite says \(want.debugDescription)"
            )
        }
        let ours = said(looks.tint(row: r, column: c, word: doc.text(row: r, column: c)))
        if ours != theirs(d["tint"]) {
            fails.append("row \(r), \(name) drawn in \(ours), the suite says \(theirs(d["tint"]))")
        }
    }
    return fails
}

/// What the file's charts draw, as the suite says (its README, `charts`, `inferred`): each
/// chart's summary through this binding's own types, and the chart a file with none is
/// drawn as.
private func charted(_ doc: Cells.Document, _ want: [String: Any]) -> [String] {
    var fails: [String] = []
    if let theirs = want["charts"] as? [Any] {
        let ours = doc.charts.indices.map { canon(said(doc.summary(chart: $0))) }
        let wanted = theirs.map { canon($0) }
        if ours.count != wanted.count {
            fails.append("\(ours.count) charts, the suite says \(wanted.count)")
        }
        for (i, (a, b)) in zip(ours, wanted).enumerated() where a != b {
            fails.append("chart \(i + 1) draws \(a), the suite says \(b)")
        }
    }
    if let theirs = want["inferred"] {
        let ours = canon(doc.inferredSummary.map(said) ?? NSNull())
        if ours != canon(theirs) {
            fails.append("drawn from its columns as \(ours), the suite says \(canon(theirs))")
        }
    }
    return fails
}

/// A summary as the suite writes it.
private func said(_ s: Cells.ChartSummary) -> [String: Any] {
    func shade(_ s: Cells.ChartSummary.Shade) -> [String: Any] {
        switch s {
        case .tint(.named(let n)): ["tint": "\(n)"]
        case .tint(.hex(let v)): ["hex": String(format: "#%06x", v)]
        case .column(let k): ["column": k]
        case .quiet: ["quiet": true]
        }
    }
    return [
        "kind": s.kind, "title": s.title, "stack": s.stack, "axis": s.axis, "places": s.places,
        "series": s.series.map { ["name": $0.name, "shade": shade($0.shade)] as [String: Any] },
        "values": s.values.map { $0.map { $0 as Any? ?? NSNull() } },
        "unit": s.unit as Any? ?? NSNull(), "skipped": s.skipped, "rows": s.rows,
        "layers": s.layers.map(said),
    ]
}

/// JSON in one spelling: an object's keys sorted, and under `values` (a chart's) every
/// decimal in one spelling, so two summaries compare as the suite compares them.
private func canon(_ v: Any, values: Bool = false) -> String {
    switch v {
    case is NSNull: return "null"
    case let a as [Any]: return "[\(a.map { canon($0, values: values) }.joined(separator: ","))]"
    case let o as [String: Any]:
        let pairs = o.keys.sorted().map {
            "\($0.debugDescription):\(canon(o[$0]!, values: $0 == "values"))"
        }
        return "{\(pairs.joined(separator: ","))}"
    case let s as String: return values ? normal(s) : s.debugDescription
    case let n as NSNumber:
        return CFGetTypeID(n) == CFBooleanGetTypeID()
            ? (n.boolValue ? "true" : "false") : n.stringValue
    default: preconditionFailure("a chart's JSON holds \(type(of: v))")
    }
}

private func json(_ url: URL) throws -> Any {
    try JSONSerialization.jsonObject(with: Data(contentsOf: url), options: .fragmentsAllowed)
}

/// A refusal's place in the suite's words.
private func words(_ at: Cells.Problem.Where) -> String {
    switch at {
    case .file: "the file"
    case .header(nil): "the header"
    case .header(let c?): "column \(c + 1)"
    case .prelude(nil): "the prelude"
    case .prelude(let l?): "prelude line \(l + 1)"
    case .record(let r): "row \(r + 1)"
    case .cell(let r, _): "row \(r + 1)"
    }
}

/// A decimal string in one spelling: no sign on zero, no leading or trailing zeros.
private func normal(_ n: String) -> String {
    var digits = Substring(n)
    let negative = digits.first == "-"
    if digits.first == "-" || digits.first == "+" { digits = digits.dropFirst() }
    let parts = digits.split(separator: ".", omittingEmptySubsequences: false)
    var int = String(parts[0].drop { $0 == "0" })
    var frac = parts.count > 1 ? String(parts[1]) : ""
    while frac.last == "0" { frac.removeLast() }
    if int.isEmpty { int = "0" }
    let body = frac.isEmpty ? int : "\(int).\(frac)"
    return negative && body != "0" ? "-\(body)" : body
}

/// A mantissa at a scale, written as a decimal.
private func decimal(_ m: Int128, scale: Int) -> String {
    var digits = String(m.magnitude)
    if scale > 0 {
        if digits.count <= scale {
            digits = String(repeating: "0", count: scale - digits.count + 1) + digits
        }
        digits.insert(".", at: digits.index(digits.endIndex, offsetBy: -scale))
    }
    return normal((m < 0 ? "-" : "") + digits)
}

private func two(_ n: Int) -> String { n < 10 ? "0\(n)" : "\(n)" }

private func date(days: Int32) -> String {
    let c = Cells.civil(days: days)
    return "\(String(format: "%04d", c.year!))-\(two(c.month!))-\(two(c.day!))"
}

private func time(seconds s: Int) -> String {
    "\(two(s / 3600)):\(two(s / 60 % 60)):\(two(s % 60))"
}

/// An instant as the suite writes it: UTC, a fraction only when it is not zero, no
/// trailing zero in it.
private func instant(_ v: Int64, unit: Int) -> String {
    let per = Int64(pow(10, Double(unit)))
    var seconds = v / per
    var fraction = v % per
    if fraction < 0 {
        seconds -= 1
        fraction += per
    }
    let days = Int32((seconds - (seconds >= 0 ? 0 : 86_399)) / 86_400)
    let within = Int(seconds - Int64(days) * 86_400)
    var text = "\(date(days: days))T\(time(seconds: within))"
    if fraction != 0 {
        var f = String(fraction)
        f = String(repeating: "0", count: unit - f.count) + f
        while f.last == "0" { f.removeLast() }
        text += ".\(f)"
    }
    return text + "Z"
}

/// A value as the suite tags it, and where it comes from: `written`, `formula`, `cell`.
private struct Tagged: Equatable, CustomStringConvertible {
    let tag: String?
    let text: String?
    let currency: String?
    let from: String
    var description: String {
        let value = tag.map { "\($0) \(text!)\(currency.map { " \($0)" } ?? "")" } ?? "null"
        return from == "written" ? value : "\(value) from \(from)"
    }
}

/// Our value as the suite tags it.
private func tagged(_ doc: Cells.Document, _ values: Cells.Values, column: Int, _ row: Int)
    -> Tagged
{
    let from =
        switch doc.origin(row: row, column: column) {
        case .written: "written"
        case .formula: "formula"
        case .cell: "cell"
        }
    if values.isNull(row) { return Tagged(tag: nil, text: nil, currency: nil, from: from) }
    func base(_ b: Cells.Base) -> Cells.Base { if case .named(_, let r) = b { base(r) } else { b } }
    let b = base(doc.type(of: column).base)
    let tag: String =
        switch b {
        case .text: "text"
        case .int: "int"
        case .num: "num"
        case .pct: "pct"
        case .bool: "bool"
        case .date: "date"
        case .time: "time"
        case .datetime: "datetime"
        case .url: "url"
        case .email: "email"
        case .id: "id"
        case .money: "money"
        case .choice: "choice"
        case .ref: "ref"
        case .named: preconditionFailure("resolved above")
        }
    let currency: String? = if case .money(let c) = b { c } else { nil }
    let text: String =
        switch values {
        case .text(let v): v[row]!
        case .int(let v): String(v[row]!)
        case .decimal(let v): decimal(v.mantissas[row]!, scale: v.scale)
        case .bool(let v): v[row]! ? "true" : "false"
        case .date(let v): date(days: v[row]!)
        case .time(let v): time(seconds: Int(v[row]!))
        case .instant(let v): instant(v.values[row]!, unit: v.unit)
        case .choice(let v): v[row]!
        }
    return Tagged(tag: tag, text: text, currency: currency, from: from)
}

/// The suite's value: its tag and text, and where it comes from (`written` when it says
/// nowhere).
private func expected(_ v: Any) -> Tagged {
    guard let o = v as? [String: Any] else {
        return Tagged(tag: nil, text: nil, currency: nil, from: "written")
    }
    let from = o["from"] as? String ?? "written"
    guard let (tag, value) = o.first(where: { $0.key != "currency" && $0.key != "from" }) else {
        return Tagged(tag: nil, text: nil, currency: nil, from: from)
    }
    var text: String
    if let n = value as? NSNumber, CFGetTypeID(n) == CFBooleanGetTypeID() {
        text = n.boolValue ? "true" : "false"
    } else if let s = value as? String {
        text = s
    } else {
        text = "\(value)"
    }
    if ["int", "num", "pct", "money"].contains(tag) { text = normal(text) }
    return Tagged(tag: tag, text: text, currency: o["currency"] as? String, from: from)
}

private func read(_ doc: Cells.Document, _ want: [String: Any]) -> [String] {
    var fails: [String] = []
    let wanted = want["columns"] as! [[String: Any]]
    guard wanted.count == doc.columns.count else {
        return ["\(doc.columns.count) columns, the suite says \(wanted.count)"]
    }
    for (k, w) in wanted.enumerated() {
        let name = w["name"] as! String
        let column = doc.columns[k]
        if column.name != name {
            fails.append("column \(k): name \(column.name), the suite says \(name)")
        }
        let shorthand = doc.shorthand(of: k)
        if shorthand != w["type"] as! String {
            fails.append("column \(name): type \(shorthand), the suite says \(w["type"]!)")
        }
        let inferred = column.type == nil
        if inferred != (w["inferred"] as? Bool ?? false) {
            fails.append("column \(name): inferred \(inferred)")
        }
        if column.formula != w["formula"] as? String {
            fails.append(
                "column \(name): formula \(column.formula ?? "nil"), the suite says \(w["formula"] ?? "nil")"
            )
        }
    }
    let named = want["types"] as? [String: String] ?? [:]
    if named.count != doc.names.count {
        fails.append("\(doc.names.count) named types, the suite says \(named.count)")
    }
    for (name, def) in named {
        guard let t = doc.names[name] else {
            fails.append("no named type \(name)")
            continue
        }
        // A named type's own definition, written as the prelude writes it: under the
        // names it uses, never itself or one that uses it.
        func uses(_ t: Cells.ColumnType, _ n: String) -> Bool {
            guard case .named(let under, _) = t.base else { return false }
            return under == n || doc.names[under].map { uses($0, n) } ?? false
        }
        let others = doc.names.filter { $0.key != name && !uses($0.value, name) }
        let ours = Cells.shorthand(t, names: others)
        if ours != def { fails.append("type \(name): \(ours), the suite says \(def)") }
    }
    let rows = want["rows"] as! [[String: Any]]
    if rows.count != doc.rows {
        fails.append("\(doc.rows) rows, the suite says \(rows.count)")
    } else {
        let values = doc.columns.indices.map { doc.values(column: $0) }
        for (r, w) in rows.enumerated() {
            for k in doc.columns.indices {
                let name = doc.columns[k].name
                let ours = tagged(doc, values[k], column: k, r)
                let theirs = expected(w[name] as Any)
                if ours != theirs {
                    fails.append("row \(r + 1), \(name): \(ours), the suite says \(theirs)")
                }
            }
        }
    }
    let kinds: [Cells.Problem.Kind: String] = [
        .type: "type", .missing: "missing", .key: "key", .ragged: "ragged",
        .formula: "formula", .reference: "reference", .stale: "stale", .unreached: "unreached",
    ]
    // A span as the suite writes it, `[start, length]` in bytes.
    let spelled = { (s: Range<Int>?) in s.map { "[\($0.lowerBound), \($0.count)]" } ?? "none" }
    let theirSpan = { (j: [String: Any]) in
        let s = j["span"] as! [Int]
        return "[\(s[0]), \(s[1])]"
    }
    let broken = doc.broken.map { "\(words($0.at)) \(spelled($0.span))" }
    let breaks = (want["broken"] as? [[String: Any]] ?? []).map {
        "\($0["at"] as! String) \(theirSpan($0))"
    }
    if broken != breaks {
        fails.append("broken at \(broken), the suite says \(breaks) (\(doc.broken))")
    }
    let cellProblems = doc.problems.filter { $0.kind != .name && $0.kind != .grammar }
    let ours = cellProblems.map { p -> String in
        guard case .cell(let r, let c) = p.at else { return "\(p)" }
        return "\(r) \(doc.columns[c].name) \(kinds[p.kind]!) \(spelled(p.span))"
    }
    let theirs = (want["problems"] as! [[String: Any]]).map {
        "\($0["row"]!) \($0["column"]!) \($0["kind"]!) \(theirSpan($0))"
    }
    if ours != theirs { fails.append("problems \(ours), the suite says \(theirs)") }
    for p in cellProblems {
        guard case .cell(let r, let c) = p.at else { continue }
        if p.description != "row \(r + 1), \(doc.columns[c].name): \(p.message)" {
            fails.append(
                "problem \(p.description.debugDescription) is not its message at its place")
        }
    }
    let header = doc.problems.filter { $0.kind == .name }.map { p -> String in
        guard case .header(let c?) = p.at else { return "\(p)" }
        return "\(c) \(p.message == "has no name" ? "blank" : "twice")"
    }
    let names = (want["names"] as? [[String: Any]] ?? []).map { "\($0["column"]!) \($0["kind"]!)" }
    if header != names { fails.append("names \(header), the suite says \(names)") }
    return fails
}

private struct Failed: Error {
    let reason: String
}

/// The edits applied in order, and the file's text after them: as written, or after a last
/// `as` edit the file written in that dialect.
private func apply(_ doc: inout Cells.Document, _ edits: [[String: Any]]) -> Result<String, Failed>
{
    do {
        return .success(try edited(&doc, edits))
    } catch let failed as Failed {
        return .failure(failed)
    } catch {
        return .failure(Failed(reason: "an edit fails: \(error)"))
    }
}

private func edited(_ doc: inout Cells.Document, _ edits: [[String: Any]]) throws -> String {
    func column(_ name: Any?) throws -> Int {
        guard let c = doc.columns.firstIndex(where: { $0.name == name as? String }) else {
            throw Failed(reason: "no column \(name ?? "nil")")
        }
        return c
    }
    for e in edits {
        if let to = e["as"] as? String {
            let header = e["header"] as? [String]
            return doc.written(sep: to == "cells" ? 0 : Character(to).asciiValue!, header: header)
        }
        // `cells fix`: every stale cell given its formula's value.
        if e["fix"] != nil {
            doc.fixStale()
            continue
        }
        /// The edit made, against what the suite says of it: taken, or refused at `refused`.
        func taken(_ edit: () throws(Cells.Problem) -> Void) throws {
            let refused = e["refused"] as? String
            do throws(Cells.Problem) {
                try edit()
            } catch {
                if words(error.at) != refused {
                    throw Failed(
                        reason: "refused at \(error), the suite says \(refused ?? "taken")")
                }
                return
            }
            if let refused { throw Failed(reason: "taken, the suite says refused at \(refused)") }
        }
        if let at = e["insert"] as? Int {
            try taken { () throws(Cells.Problem) in
                try doc.insertColumn(
                    header: e["header"] as! String, at: at, cells: e["cells"] as? [String])
            }
        } else if e["remove"] != nil {
            let c = try column(e["remove"])
            try taken { () throws(Cells.Problem) in try doc.removeColumn(at: c) }
        } else if e["move"] != nil {
            doc.moveColumn(from: try column(e["move"]), to: e["to"] as! Int)
        } else if e["rename"] != nil {
            let c = try column(e["rename"])
            try taken { () throws(Cells.Problem) in
                try doc.rename(column: c, to: e["to"] as! String)
            }
        } else if let spelled = e["type"] as? String {
            let c = try column(e["column"])
            try taken { () throws(Cells.Problem) in try doc.setSpelled(spelled, column: c) }
        } else {
            let c = try column(e["column"])
            let row = e["row"] as! Int
            if let text = e["text"] as? String {
                try taken { () throws(Cells.Problem) in try doc.set(row: row, column: c, to: text) }
            } else {
                let v = expected(e["value"] as Any)
                let value = v.text.map { Cells.Value.text($0) } ?? .missing
                try taken { () throws(Cells.Problem) in
                    try doc.set(row: row, column: c, value: value)
                }
            }
        }
    }
    return doc.text
}
