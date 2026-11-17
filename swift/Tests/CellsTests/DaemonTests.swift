#if os(macOS)
    import Foundation
    import Testing

    @testable import Cells

    // The engine's conversation as this client reads it: one JSON object per line, however
    // the bytes arrive.

    // A script's version as the engine computes it (the Cells repo's allowed.rs pins the same
    // value): the SHA-256 of its language, a line break and its code, which ends with its own.
    // A path as the engine says it: `/tmp` is `/private/tmp`, which `resolvingSymlinksInPath`
    // would have stripped, so a client matching the engine's paths heard nothing.
    @Test func aPathAsTheEngineSaysIt() {
        #expect(Cells.Daemon.canonical(URL(filePath: "/tmp")) == "/private/tmp")
        #expect(URL(filePath: "/private/tmp").resolvingSymlinksInPath().path == "/tmp", "the trap")
    }

    @Test func aScriptsVersionIsTheEnginesOwn() throws {
        let text =
            "---\nrates = run every hour\n```ts\nexport default async () => []\n```\n```cells\nday:date!,rate:num\n```\n---\na\n1\n"
        let doc = Cells.Document(reading: text)
        let run = try #require(doc.statements.first)
        guard case .run(_, _, let language, let code, _) = run.kind else {
            Issue.record("a script")
            return
        }
        #expect(
            Cells.Approvals.version(language: language, code: code)
                == "417af8deda266572d175f2fcc48b7429b87322668d511232a25569815a21010d")
    }

    @Test func theEnginesLinesReadAsTheyArrive() throws {
        let pipe = Pipe()
        var lines = Cells.Lines(pipe.fileHandleForReading)
        let w = pipe.fileHandleForWriting
        // a line read while the other side stays open, as a socket's greeting is
        try w.write(
            contentsOf: Data("{\"say\":\"hello\",\"version\":1,\"pid\":7}\n{\"say\":\"chan".utf8))
        let first = lines.next()
        try w.write(contentsOf: Data("ged\",\"paths\":[\"/w/a.cells\",\"/w/b.cells\"]}\n".utf8))
        try w.close()
        let hello = try #require(first)
        #expect(hello["say"] as? String == "hello")
        #expect(hello["pid"] as? Int == 7)
        let second = lines.next()
        let changed = try #require(second)
        #expect(changed["paths"] as? [String] == ["/w/a.cells", "/w/b.cells"])
        let third = lines.next()
        #expect(third == nil, "closed")
    }

    // A chain and a cell's values as an engine answered them: q4 saved, its own write and the
    // quarters under the save, the year under the quarters; the year's cell traced to the save.
    @Test func historyReadsAsTheEngineSaysIt() throws {
        let chains = try JSONDecoder().decode(
            [Cells.History.Chain].self, from: Data(answeredChains.utf8))
        let chain = try #require(chains.first)
        guard case .external(let path, _, _, let by, let edited) = chain.root.entry else {
            Issue.record("an outside edit")
            return
        }
        #expect(path == "/private/tmp/hist/q4/money.cells" && by == nil)
        #expect(
            edited?.cells == [
                .init(row: 2, key: nil, column: "amount", was: "200.00", now: "230.00")
            ])
        #expect(chain.root.file.path == "/private/tmp/hist/q4/money.cells")
        #expect(chain.root.identity == "k7m2q9xa" && chain.id == 5)
        #expect(
            chain.followed.map(\.event.file.lastPathComponent) == [
                "money.cells", "quarters.cells", "year.cells",
            ])
        #expect(chain.followed[0].event.identity == chain.root.identity, "one file")
        #expect(chain.followed.map(\.under) == [nil, nil, 1])
        guard case .store(_, let via, _, _, let stored) = chain.followed[2].event.entry else {
            Issue.record("a store")
            return
        }
        #expect(via == ["/private/tmp/hist/quarters.cells"] && stored.first?.rowName == "2026")
        let values = try JSONDecoder().decode(
            [Cells.History.Held].self, from: Data(answeredValues.utf8))
        #expect(values.map(\.value) == ["230.00", "200.00"])
        #expect(values[0].root == chain.root.id)
    }

    // A script's run as a root, as the engine logs it: its outcome's fields beside the rest.
    @Test func aRunReadsAsARoot() throws {
        let line = #"""
            {"n":9,"prev":"ab","at":1794000000000,"file":"k7m2q9xa","kind":"run","path":"/w/trip.cells","script":"rates","run":2,"outcome":"ok","added":0,"changed":1,"by":"its schedule","was":"aa","now":"bb"}
            """#
        let event = try JSONDecoder().decode(Cells.History.Event.self, from: Data(line.utf8))
        guard case .run(let path, let script, let run, let outcome, let by, _) = event.entry else {
            Issue.record("a run")
            return
        }
        #expect(path == "/w/trip.cells" && script == "rates" && run == 2 && by == "its schedule")
        #expect(outcome == .ok(added: 0, changed: 1))
        #expect(event.entry.isRoot)
        let failed = line.replacing(
            #""outcome":"ok","added":0,"changed":1"#, with: #""outcome":"failed","why":"503""#)
        let other = try JSONDecoder().decode(Cells.History.Event.self, from: Data(failed.utf8))
        guard case .run(_, _, _, .failed(let why), _, _) = other.entry else {
            Issue.record("a failed run")
            return
        }
        #expect(why == "503")
    }

    // A client's intent as a root, as the engine logs it: its id, who sent it, the versions.
    @Test func anIntentReadsAsARoot() throws {
        let line = #"""
            {"n":4,"prev":"ab","at":1794000000000,"file":"k7m2q9xa","kind":"edit","path":"/w/trip.cells","intent":"i-7","by":"Cells","ops":[{"op":"set","row":{"key":"fuel"},"column":"cost","was":"84.20","text":"90.00"}],"was":"aa","now":"bb","edited":{"cells":[{"row":1,"key":"fuel","column":"cost","was":"84.20","now":"90.00"}]}}
            """#
        let event = try JSONDecoder().decode(Cells.History.Event.self, from: Data(line.utf8))
        guard case .edit(let path, let intent, let by, let was, let now, let edited) = event.entry
        else {
            Issue.record("an edit")
            return
        }
        #expect(path == "/w/trip.cells" && intent == "i-7" && by == "Cells")
        #expect(was == "aa" && now == "bb" && edited?.cells.first?.now == "90.00")
        #expect(event.entry.isRoot)
    }

    // What changed since a version, as the engine answers it: a cell changed twice marked
    // once, its entry and root as the history names them, and a row added marked whole.
    @Test func whatChangedSinceAVersionReadsAsTheEngineSaysIt() throws {
        let marks = try JSONDecoder().decode(
            [Cells.History.Mark].self,
            from: Data(
                #"""
                [{"row":1,"key":"q4","column":"costs","was":"180.00","now":"250.00","at":1794000000000,"entry":5,"root":4},
                 {"row":2,"key":null,"column":null,"was":null,"now":"","at":1794000000000,"entry":6,"root":6}]
                """#.utf8))
        #expect(marks[0].rowName == "q4" && marks[0].was == "180.00" && marks[0].now == "250.00")
        #expect(marks[0].root == 4 && marks[0].entry == 5)
        #expect(marks[1].column == nil && marks[1].was == nil && marks[1].rowName == "row 2")
        // the engine's SHA-256 of a text, the same the approvals pin
        #expect(
            Cells.History.version(of: "ts\nexport default async () => []\n")
                == "417af8deda266572d175f2fcc48b7429b87322668d511232a25569815a21010d")
    }

    // An intent's ops as the engine reads them (the Cells repo's wire.rs pins the same line),
    // and a fresh id in the engine's alphabet.
    @Test func anIntentIsSentAsTheEngineReadsIt() throws {
        let ops: [Cells.Intent.Op] = [
            .set(row: .key("fuel"), column: "cost", was: "84.20", text: "90.00"),
            .add(cells: [(column: "item", text: "taxi")]),
            .delete(row: .id("q4x9t1mz"), was: nil),
            .text("item:text!\nfuel\n"),
        ]
        let sent = try JSONSerialization.data(withJSONObject: ops.map(\.json), options: .sortedKeys)
        let engine = #"""
            [{"op":"set","row":{"key":"fuel"},"column":"cost","was":"84.20","text":"90.00"},{"op":"add","cells":[{"column":"item","text":"taxi"}]},{"op":"delete","row":{"id":"q4x9t1mz"}},{"op":"text","text":"item:text!\nfuel\n"}]
            """#
        let reads = try JSONSerialization.jsonObject(with: Data(engine.utf8))
        let pinned = try JSONSerialization.data(withJSONObject: reads, options: .sortedKeys)
        #expect(sent == pinned)
        let id = Cells.Intent.newID()
        #expect(id.count == 8 && id.allSatisfy { Cells.Intent.alphabet.contains($0) })
    }

    // A file moved, as the engine logs it: a root, the path it came from beside the one it is.
    @Test func aMoveReadsAsARoot() throws {
        let line = #"""
            {"n":3,"prev":"ab","at":1794000000000,"file":"k7m2q9xa","kind":"relocated","path":"/h/Music/q4.cells","from":"/h/w/q4.cells","now":"aa"}
            """#
        let event = try JSONDecoder().decode(Cells.History.Event.self, from: Data(line.utf8))
        guard case .relocated(let path, let from, let now) = event.entry else {
            Issue.record("a move")
            return
        }
        #expect(path == "/h/Music/q4.cells" && from == "/h/w/q4.cells" && now == "aa")
        #expect(event.entry.isRoot && event.file.path == "/h/Music/q4.cells")
    }

    private let answeredChains = #"""
        [{"root":{"n":5,"prev":"9b38","at":1794141403923,"file":"k7m2q9xa","kind":"external","path":"/private/tmp/hist/q4/money.cells","was":"4ba2","now":"1793","by":null,"edited":{"cells":[{"row":2,"column":"amount","was":"200.00","now":"230.00"}]}},"followed":[{"n":6,"prev":"e8c4","at":1794141403940,"file":"k7m2q9xa","kind":"store","path":"/private/tmp/hist/q4/money.cells","cause":{"n":5},"via":["/private/tmp/hist/q4/money.cells"],"against":"1793","now":"9a4b","stored":[{"row":2,"column":"cost","was":"200.00","now":"230.00"}],"under":null},{"n":7,"prev":"7c86","at":1794141403955,"file":"q4x9t1mz","kind":"store","path":"/private/tmp/hist/quarters.cells","cause":{"n":5},"via":["/private/tmp/hist/q4/money.cells"],"against":"e35d","now":"f42b","stored":[{"row":1,"key":"q4","column":"costs","was":"200.00","now":"230.00"}],"under":null},{"n":8,"prev":"8651","at":1794141403970,"file":"y3ar2026","kind":"store","path":"/private/tmp/hist/year.cells","cause":{"n":5},"via":["/private/tmp/hist/quarters.cells"],"against":"9ef7","now":"c524","stored":[{"row":1,"key":"2026","column":"costs","was":"200.00","now":"230.00"}],"under":1}]}]
        """#

    private let answeredValues = #"""
        [{"value":"230.00","was":"200.00","at":1794141403970,"n":8,"root":5},{"value":"200.00","was":"180.00","at":1794140680827,"n":4,"root":1}]
        """#
#endif
