#if os(macOS)
    import CryptoKit
    import Foundation

    extension Cells {
        /// What the engine's history says (the Cells repo's ENGINE.md, History, and
        /// `rust/cells-daemon/src/history.rs`): every version of every file, by what made it
        /// and what it set off. A **chain** is a root (an edit saved by an editor, an app or
        /// a program outside Cells; a file returned to a version) and every file it reached,
        /// drawn as a tree; a cell's **values** are every value it held, each beside its
        /// chain's root. Asked of `Cells.Daemon` (`history`, `cell`), so every surface reads
        /// the same events the same way.
        public enum History {
            /// A root and everything it reached, in the order it happened, each entry under
            /// the one it followed (`Followed.under`).
            public struct Chain: Decodable, Sendable, Identifiable {
                public let root: Event
                public let followed: [Followed]
                public var id: Event.ID { root.id }
            }

            /// An entry of a chain, and the index in `followed` of the entry it is drawn under:
            /// nil when it hangs from the root.
            public struct Followed: Decodable, Sendable, Identifiable {
                public let event: Event
                public let under: Int?
                public var id: Event.ID { event.id }

                public init(from decoder: any Decoder) throws {
                    event = try Event(from: decoder)
                    under = try decoder.container(keyedBy: Key.self).decodeIfPresent(
                        Int.self, forKey: .under)
                }

                enum Key: String, CodingKey { case under }
            }

            /// A line of a folder's log: its folder, its number there, when, and what it says.
            public struct Event: Decodable, Sendable, Identifiable {
                /// The folder whose log holds it, and its number there: unique on this Mac.
                public struct ID: Hashable, Sendable {
                    public let folder: String
                    public let n: Int
                }

                public let folder: URL
                public let n: Int
                public let at: Date
                public let entry: Entry
                public var id: ID { ID(folder: folder.path, n: n) }
                /// The file it is about, whole.
                public var file: URL { folder.appending(path: entry.path) }

                public init(from decoder: any Decoder) throws {
                    let c = try decoder.container(keyedBy: Key.self)
                    folder = URL(filePath: try c.decode(String.self, forKey: .folder))
                    n = try c.decode(Int.self, forKey: .n)
                    at = Date(
                        timeIntervalSince1970: Double(try c.decode(Int64.self, forKey: .at)) / 1000)
                    entry = try Entry(from: decoder)
                }

                enum Key: String, CodingKey { case folder, n, at }
            }

            /// What a line says. `external`, `restore`, `run` and `edit` are roots; `store`
            /// and `moved` follow one.
            public enum Entry: Decodable, Sendable {
                /// A version the engine did not write: added (`was` nil), removed (`now`
                /// nil) or saved, by the client that said it held it (`by`), else by a program
                /// outside Cells; what it changed when both versions read (`edited`).
                case external(path: String, was: String?, now: String?, by: String?, edited: Edit?)
                /// A file returned to the version `now` by a client's request.
                case restore(path: String, was: String, now: String, by: String, edited: Edit?)
                /// A script's run (`run` its number), started by `by` (its schedule, a change to
                /// a file, a client): its rows landed in its table, or only its failure.
                case run(
                    path: String, script: String, run: Int, outcome: Outcome, by: String,
                    edited: Edit?)
                /// A client's intent (`intent` its id), sent by `by` and written by the engine
                /// over the version `was`, giving the version `now`.
                case edit(
                    path: String, intent: String, by: String, was: String, now: String,
                    edited: Edit?)
                /// The engine stored what its root reached: the files of the chain it reads
                /// (`via`), each computed cell stored.
                case store(path: String, via: [String], was: String, now: String, stored: [Cell])
                /// A write not made: the file changed since it was read; that change is a
                /// root of its own.
                case moved(path: String)

                public var path: String {
                    switch self {
                    case .external(let path, _, _, _, _), .restore(let path, _, _, _, _),
                        .run(let path, _, _, _, _, _), .edit(let path, _, _, _, _, _),
                        .store(let path, _, _, _, _),
                        .moved(let path):
                        path
                    }
                }

                public var isRoot: Bool {
                    switch self {
                    case .external, .restore, .run, .edit: true
                    case .store, .moved: false
                    }
                }

                /// What came of a run: the rows its table gained and had replaced, or why it
                /// failed.
                public enum Outcome: Sendable, Hashable {
                    case ok(added: Int, changed: Int)
                    case failed(why: String)
                }

                public init(from decoder: any Decoder) throws {
                    let c = try decoder.container(keyedBy: Key.self)
                    let path = try c.decode(String.self, forKey: .path)
                    switch try c.decode(String.self, forKey: .kind) {
                    case "external":
                        self = .external(
                            path: path, was: try c.decodeIfPresent(String.self, forKey: .was),
                            now: try c.decodeIfPresent(String.self, forKey: .now),
                            by: try c.decodeIfPresent(String.self, forKey: .by),
                            edited: try c.decodeIfPresent(Edit.self, forKey: .edited))
                    case "restore":
                        self = .restore(
                            path: path, was: try c.decode(String.self, forKey: .was),
                            now: try c.decode(String.self, forKey: .now),
                            by: try c.decode(String.self, forKey: .by),
                            edited: try c.decodeIfPresent(Edit.self, forKey: .edited))
                    case "run":
                        let outcome: Outcome =
                            try c.decode(String.self, forKey: .outcome) == "ok"
                            ? .ok(
                                added: try c.decode(Int.self, forKey: .added),
                                changed: try c.decode(Int.self, forKey: .changed))
                            : .failed(why: try c.decode(String.self, forKey: .why))
                        self = .run(
                            path: path, script: try c.decode(String.self, forKey: .script),
                            run: try c.decode(Int.self, forKey: .run), outcome: outcome,
                            by: try c.decode(String.self, forKey: .by),
                            edited: try c.decodeIfPresent(Edit.self, forKey: .edited))
                    case "edit":
                        self = .edit(
                            path: path, intent: try c.decode(String.self, forKey: .intent),
                            by: try c.decode(String.self, forKey: .by),
                            was: try c.decode(String.self, forKey: .was),
                            now: try c.decode(String.self, forKey: .now),
                            edited: try c.decodeIfPresent(Edit.self, forKey: .edited))
                    case "store":
                        self = .store(
                            path: path, via: try c.decode([String].self, forKey: .via),
                            was: try c.decode(String.self, forKey: .against),
                            now: try c.decode(String.self, forKey: .now),
                            stored: try c.decode([Cell].self, forKey: .stored))
                    case "moved":
                        self = .moved(path: path)
                    case let kind:
                        throw DecodingError.dataCorruptedError(
                            forKey: .kind, in: c, debugDescription: "no entry of kind \(kind)")
                    }
                }

                enum Key: String, CodingKey {
                    case kind, path, was, now, by, edited, via, against, stored
                    case script, run, outcome, added, changed, why, intent
                }
            }

            /// A cell changed: its row as the table numbers it (from 1), the row's key when
            /// the table has one, its column, its text before and after.
            public struct Cell: Decodable, Sendable, Hashable {
                public let row: Int
                public let key: String?
                public let column: String
                public let was: String
                public let now: String

                /// The row as a person names it: its key, else its number.
                public var rowName: String { key ?? "row \(row)" }
            }

            /// What a version changed beside the one before: cells, rows added (numbered as
            /// after) and removed (numbered as before), columns added, removed and renamed.
            public struct Edit: Decodable, Sendable, Hashable {
                public let cells: [Cell]
                public let rowsAdded: [Int]
                public let rowsRemoved: [Int]
                public let columnsAdded: [String]
                public let columnsRemoved: [String]
                public let columnsRenamed: [Renamed]

                /// A column held under another name: its name before, its name now.
                public struct Renamed: Decodable, Sendable, Hashable {
                    public let from: String
                    public let to: String
                }

                public init(from decoder: any Decoder) throws {
                    let c = try decoder.container(keyedBy: Key.self)
                    cells = try c.decodeIfPresent([Cell].self, forKey: .cells) ?? []
                    rowsAdded = try c.decodeIfPresent([Int].self, forKey: .rowsAdded) ?? []
                    rowsRemoved = try c.decodeIfPresent([Int].self, forKey: .rowsRemoved) ?? []
                    columnsAdded =
                        try c.decodeIfPresent([String].self, forKey: .columnsAdded) ?? []
                    columnsRemoved =
                        try c.decodeIfPresent([String].self, forKey: .columnsRemoved) ?? []
                    columnsRenamed =
                        try c.decodeIfPresent([Renamed].self, forKey: .columnsRenamed) ?? []
                }

                /// The engine's words, snake case.
                enum Key: String, CodingKey {
                    case cells
                    case rowsAdded = "rows_added"
                    case rowsRemoved = "rows_removed"
                    case columnsAdded = "columns_added"
                    case columnsRemoved = "columns_removed"
                    case columnsRenamed = "columns_renamed"
                }
            }

            /// A value a cell held, the entry that wrote it and its chain's root.
            public struct Held: Decodable, Sendable, Identifiable {
                public let value: String
                public let was: String
                public let at: Date
                public let entry: Event.ID
                public let root: Event.ID
                public var id: Event.ID { entry }

                public init(from decoder: any Decoder) throws {
                    let c = try decoder.container(keyedBy: Key.self)
                    value = try c.decode(String.self, forKey: .value)
                    was = try c.decode(String.self, forKey: .was)
                    at = Date(
                        timeIntervalSince1970: Double(try c.decode(Int64.self, forKey: .at)) / 1000)
                    entry = Event.ID(
                        folder: try c.decode(String.self, forKey: .folder),
                        n: try c.decode(Int.self, forKey: .n))
                    var root = try c.nestedUnkeyedContainer(forKey: .root)
                    self.root = Event.ID(
                        folder: try root.decode(String.self), n: try root.decode(Int.self))
                }

                enum Key: String, CodingKey { case value, was, at, folder, n, root }
            }

            /// A cell the versions after one changed (`Cells.Daemon.since`), its row as the
            /// file numbers it now (from 1): what it held at that version and holds now, and the
            /// entry that last wrote it with that entry's root, so it is said and lit with the
            /// history's own words. A row added since is marked whole: no column, nothing held.
            public struct Mark: Decodable, Sendable, Hashable {
                public let row: Int
                public let key: String?
                public let column: String?
                public let was: String?
                public let now: String
                public let at: Date
                public let entry: Event.ID
                public let root: Event.ID

                /// The row as a person names it: its key, else its number.
                public var rowName: String { key ?? "row \(row)" }

                public init(from decoder: any Decoder) throws {
                    let c = try decoder.container(keyedBy: Key.self)
                    row = try c.decode(Int.self, forKey: .row)
                    key = try c.decodeIfPresent(String.self, forKey: .key)
                    column = try c.decodeIfPresent(String.self, forKey: .column)
                    was = try c.decodeIfPresent(String.self, forKey: .was)
                    now = try c.decode(String.self, forKey: .now)
                    at = Date(
                        timeIntervalSince1970: Double(try c.decode(Int64.self, forKey: .at)) / 1000)
                    func id(_ key: Key) throws -> Event.ID {
                        var pair = try c.nestedUnkeyedContainer(forKey: key)
                        return Event.ID(
                            folder: try pair.decode(String.self), n: try pair.decode(Int.self))
                    }
                    entry = try id(.entry)
                    root = try id(.root)
                }

                enum Key: String, CodingKey { case row, key, column, was, now, at, entry, root }
            }

            /// A version of a file as the engine names it: the SHA-256 of its text.
            public static func version(of text: String) -> String {
                SHA256.hash(data: Data(text.utf8)).map { String(format: "%02x", $0) }.joined()
            }
        }
    }
#endif
