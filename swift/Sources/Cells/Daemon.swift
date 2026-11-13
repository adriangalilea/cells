#if os(macOS)
    import Foundation

    extension Cells {
        /// A client of Cells' engine kept running (the daemon, `cells serve`, the Cells repo's
        /// ENGINE.md): one process per user that hears the disk and stores what a change
        /// reaches in every file reading it, for every app and editor at once. Over its
        /// socket, one JSON object per line (the Cells repo's `rust/cells-daemon/src/wire.rs`):
        /// a client greets, asks for folders to be served, and hears every file whose version
        /// changed, whoever changed it, the engine's own writes included.
        ///
        /// The engine admits a client by its code signature (an app signed as Cells, or the
        /// `cells` command it carries), so a process signed otherwise is refused at the
        /// greeting, told why.
        ///
        ///     let daemon = try await Cells.Daemon.connectOrStart(client: "app", command: cells)
        ///     _ = try daemon.serve(folder)
        ///     for await paths in daemon.changes { reload(paths) }
        public final class Daemon: @unchecked Sendable {
            /// The engine's process.
            public let pid: Int32
            /// Every file whose version changed, as the engine tells them, in batches.
            public let changes: AsyncStream<[URL]>

            private let handle: FileHandle
            private let telling: AsyncStream<[URL]>.Continuation
            /// One request answered at a time, so each answer is its request's.
            private let asking = NSLock()
            private let answered = DispatchSemaphore(value: 0)
            private var answer: Result<[String: Any], DaemonError>?

            /// The protocol's version, the engine's own (`wire::VERSION`).
            static let version = 2

            /// A path as the engine names it: real, every link resolved (`realpath`), so
            /// `/tmp/work` is `/private/tmp/work`. Never `resolvingSymlinksInPath()`, which
            /// strips a leading `/private` and names a file the engine never says.
            public static func canonical(_ url: URL) -> String {
                guard let real = realpath(url.path, nil) else {
                    return url.standardizedFileURL.path
                }
                defer { free(real) }
                return String(cString: real)
            }

            /// Where the engine answers: `Cells/engine.sock` in the user's application support
            /// folder, as the engine makes it.
            public static var socket: URL {
                FileManager.default.homeDirectoryForCurrentUser
                    .appending(path: "Library/Application Support/Cells/engine.sock")
            }

            /// The running engine, greeted as `client` (`app`, `lsp`, `cli`: for its trace).
            public static func connect(client: String) throws(DaemonError) -> Daemon {
                let fd = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
                guard fd >= 0 else { throw .unreachable(String(cString: strerror(errno))) }
                var address = sockaddr_un()
                address.sun_family = sa_family_t(AF_UNIX)
                let path = Array(Self.socket.path.utf8CString)
                precondition(
                    path.count <= MemoryLayout.size(ofValue: address.sun_path),
                    "the engine's socket path fits a sockaddr_un")
                withUnsafeMutableBytes(of: &address.sun_path) { raw in
                    path.withUnsafeBytes { raw.copyMemory(from: $0) }
                }
                let joined = withUnsafePointer(to: &address) {
                    $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                        Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
                    }
                }
                guard joined == 0 else {
                    let why = String(cString: strerror(errno))
                    close(fd)
                    throw .unreachable("\(Self.socket.path): \(why)")
                }
                let handle = FileHandle(fileDescriptor: fd, closeOnDealloc: true)
                var lines = Lines(handle)
                send(["say": "hello", "version": version, "client": client], to: handle)
                guard let hello = lines.next() else {
                    throw .refused("the engine closed at the greeting")
                }
                switch hello["say"] as? String {
                case "hello" where hello["version"] as? Int == version:
                    return Daemon(
                        handle: handle, lines: lines, pid: Int32(hello["pid"] as? Int ?? -1))
                case "refused":
                    throw .refused(hello["why"] as? String ?? "refused")
                default:
                    throw .refused("the engine answered the greeting with \(hello)")
                }
            }

            /// The running engine, or one started from `command` (the `cells` the app carries)
            /// when nothing answers: detached, its trace in `engine.trace` beside the socket, as
            /// the language server starts it. Two clients starting one at once start two; the
            /// second finds the first holding the engine's lock and hands it its folders.
            public static func connectOrStart(client: String, command: URL)
                async throws(DaemonError) -> Daemon
            {
                if let daemon = try? connect(client: client) { return daemon }
                let folder = socket.deletingLastPathComponent()
                try? FileManager.default.createDirectory(
                    at: folder, withIntermediateDirectories: true)
                let trace = folder.appending(path: "engine.trace")
                if !FileManager.default.fileExists(atPath: trace.path) {
                    FileManager.default.createFile(atPath: trace.path, contents: nil)
                }
                let process = Process()
                process.executableURL = command
                process.arguments = ["serve"]
                guard let out = try? FileHandle(forWritingTo: trace) else {
                    throw .unreachable("\(trace.path): not writable")
                }
                out.seekToEndOfFile()
                process.standardOutput = out
                process.standardError = out
                process.standardInput = FileHandle.nullDevice
                do { try process.run() } catch {
                    throw .unreachable("\(command.path) serve: \(error)")
                }
                let began = ContinuousClock.now
                while true {
                    do {
                        return try connect(client: client)
                    } catch {
                        // the start of a process, never a folder's size: its folders are read
                        // after the socket answers
                        if ContinuousClock.now - began > .seconds(5) {
                            throw .unreachable(
                                "started \(command.path) serve, and it did not answer in 5 s: \(error) (its trace: \(trace.path))"
                            )
                        }
                        try? await Task.sleep(for: .milliseconds(20))
                    }
                }
            }

            private init(handle: FileHandle, lines: Lines, pid: Int32) {
                self.handle = handle
                self.pid = pid
                (changes, telling) = AsyncStream.makeStream(of: [URL].self)
                let reader = Thread { [unowned self] in self.hear(lines) }
                reader.name = "cells-daemon"
                reader.start()
            }

            /// `folder` served by the engine from now on: the folder served (it, or the folder
            /// served already holding it), or why the engine refused (a folder too wide to
            /// serve whole). Blocks until the engine answers, a moment.
            public func serve(_ folder: URL) throws(DaemonError) -> URL {
                let answer = try ask(["say": "serve", "folder": folder.path])
                precondition(
                    answer["say"] as? String == "served",
                    "the engine answered a request to serve with \(answer)")
                return URL(filePath: answer["folder"] as! String)
            }

            /// `script` of the file at `file` allowed to run at `version`
            /// (`Cells.Approvals.version`), the one its person was just shown: called only
            /// after their yes, or as they wrote it. The engine keeps it.
            public func allow(file: URL, script: String, version: String) throws(DaemonError) {
                let answer = try ask([
                    "say": "allow", "file": file.path, "script": script, "version": version,
                ])
                precondition(
                    answer["say"] as? String == "allowed",
                    "the engine answered a request to allow with \(answer)")
            }

            /// The engine told this client holds `text` of `file`, unsaved or about to be
            /// written, so the version it lands as is logged as this client's (history's
            /// "saved by"). Never answered: say it before every write, and as it is typed.
            public func editing(file: URL, text: String) {
                let version = History.version(of: text)
                Self.send(["say": "editing", "file": file.path, "version": version], to: handle)
            }

            /// What changed in `file` since `version` (the one its person last worked on): each
            /// cell changed, as the file numbers it now, and the chains that changed them;
            /// nil when the engine's log names no such version, so what came after is not known.
            public func since(file: URL, version: String) throws(DaemonError) -> (
                marks: [History.Mark], chains: [History.Chain]
            )? {
                let answer = try ask(["say": "since", "file": file.path, "version": version])
                guard answer["known"] as? Bool == true else { return nil }
                return (
                    decoded([History.Mark].self, answer, "marks"),
                    decoded([History.Chain].self, answer, "chains")
                )
            }

            /// The chains reaching anything under `folder`, or `file` alone, newest first, at
            /// most `limit`.
            public func history(folder: URL, file: URL? = nil, limit: Int = 200) throws(DaemonError)
                -> [History.Chain]
            {
                var asked: [String: Any] = [
                    "say": "history", "folder": folder.path, "limit": limit,
                ]
                if let file { asked["file"] = file.path }
                return decoded([History.Chain].self, try ask(asked), "chains")
            }

            /// Every value the cell at `row` (from 1, as the file numbers it now) of `column`
            /// held, newest first, each beside its chain's root.
            public func cell(file: URL, row: Int, column: String) throws(DaemonError) -> [History
                .Held]
            {
                let asked: [String: Any] = [
                    "say": "cell", "file": file.path, "row": row, "column": column,
                ]
                return decoded([History.Held].self, try ask(asked), "values")
            }

            /// The version of `file` whose SHA-256 is `version`, whole, as the log names it.
            public func version(file: URL, version: String) throws(DaemonError) -> String {
                let answer = try ask(["say": "version", "file": file.path, "version": version])
                return answer["text"] as! String
            }

            /// `file` returned to `version`: a change of its own, a root in history, never
            /// history rewritten. The version it held before.
            public func restore(file: URL, version: String) throws(DaemonError) -> String {
                let answer = try ask(["say": "restore", "file": file.path, "version": version])
                precondition(
                    answer["say"] as? String == "restored",
                    "the engine answered a restore with \(answer)")
                return answer["was"] as! String
            }

            /// The script `script` of `file` run now by the engine, as its person allowed it
            /// (a refusal says how to allow it): its run's number and what came of it, once its
            /// rows have landed. Blocks for as long as the script runs.
            public func run(file: URL, script: String) throws(DaemonError) -> (
                run: Int, outcome: History.Entry.Outcome
            ) {
                let answer = try ask(["say": "run", "file": file.path, "script": script])
                precondition(
                    answer["say"] as? String == "ran", "the engine answered a run with \(answer)")
                let outcome: History.Entry.Outcome =
                    answer["outcome"] as? String == "ok"
                    ? .ok(added: answer["added"] as! Int, changed: answer["changed"] as! Int)
                    : .failed(why: answer["why"] as! String)
                return (answer["run"] as! Int, outcome)
            }

            /// `intent` applied to `file`, made against the version whose SHA-256 is
            /// `against`: the version written and the ids the engine gave rows, or the refusal
            /// with its reason (a cell changed since, a rule it would break), nothing written.
            public func apply(_ intent: Intent, to file: URL, against: String) throws(DaemonError)
                -> Intent.Applied
            {
                let answer = try ask([
                    "say": "intent", "id": intent.id, "file": file.path, "against": against,
                    "ops": intent.ops.map(\.json),
                ])
                precondition(
                    answer["say"] as? String == "applied",
                    "the engine answered an intent with \(answer)")
                return Intent.Applied(
                    version: answer["version"] as! String, assigned: answer["assigned"] as! [String]
                )
            }

            /// `key` of an answer decoded as `type`: the engine's own shapes, so a mismatch is
            /// a bug in one of the two, and screams.
            private func decoded<T: Decodable>(
                _ type: T.Type, _ answer: [String: Any], _ key: String
            ) -> T {
                let data = try! JSONSerialization.data(withJSONObject: answer[key] as Any)
                do {
                    return try JSONDecoder().decode(type, from: data)
                } catch {
                    preconditionFailure("the engine's \(key) do not read as \(type): \(error)")
                }
            }

            /// `request` sent and its answer read, a moment's wait; a refusal is the error.
            private func ask(_ request: [String: Any]) throws(DaemonError) -> [String: Any] {
                asking.lock()
                defer { asking.unlock() }
                Self.send(request, to: handle)
                answered.wait()
                let answer = self.answer!
                self.answer = nil
                return try answer.get()
            }

            /// What the engine says, sorted: changes to `changes`, answers to whoever asked.
            /// Ends when the engine closes, which ends `changes`.
            private func hear(_ lines: Lines) {
                var lines = lines
                while let said = lines.next() {
                    switch said["say"] as? String {
                    case "changed":
                        telling.yield((said["paths"] as? [String] ?? []).map { URL(filePath: $0) })
                    case "served", "allowed", "history", "values", "version", "restored", "ran",
                        "since", "applied":
                        answer = .success(said)
                        answered.signal()
                    case "refused":
                        answer = .failure(.refused(said["why"] as? String ?? "refused"))
                        answered.signal()
                    default:
                        assertionFailure(
                            "the engine said something this client does not read: \(said)")
                    }
                }
                telling.finish()
                // a request waiting when the engine closed is answered with that
                answer = .failure(.unreachable("the engine closed"))
                answered.signal()
            }

            private static func send(_ message: [String: Any], to handle: FileHandle) {
                var line = try! JSONSerialization.data(withJSONObject: message)
                line.append(UInt8(ascii: "\n"))
                // a closed engine is heard by the reader, which ends `changes`
                try? handle.write(contentsOf: line)
            }
        }

        public enum DaemonError: Error, CustomStringConvertible {
            /// No engine answers, or it closed.
            case unreachable(String)
            /// The engine refused: the client's signature, its version, a request.
            case refused(String)

            public var description: String {
                switch self {
                case .unreachable(let why): "the Cells engine is unreachable: \(why)"
                case .refused(let why): "the Cells engine refused: \(why)"
                }
            }
        }

        /// The engine's lines, each a JSON object: nothing once it has closed.
        struct Lines {
            let handle: FileHandle
            var buffer = Data()

            init(_ handle: FileHandle) { self.handle = handle }

            mutating func next() -> [String: Any]? {
                while true {
                    if let end = buffer.firstIndex(of: UInt8(ascii: "\n")) {
                        let line = buffer[buffer.startIndex..<end]
                        buffer = Data(buffer[(end + 1)...])
                        guard
                            let object = try? JSONSerialization.jsonObject(with: line)
                                as? [String: Any]
                        else {
                            preconditionFailure(
                                "the engine sent a line that is not a JSON object: \(String(decoding: line, as: UTF8.self))"
                            )
                        }
                        return object
                    }
                    // what has arrived, as soon as any has: `read(upToCount:)` would wait for
                    // the whole count, which a socket's conversation never fills
                    let more = handle.availableData
                    guard !more.isEmpty else { return nil }
                    buffer.append(more)
                }
            }
        }
    }
#endif
