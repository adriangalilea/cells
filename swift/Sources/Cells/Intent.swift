#if os(macOS)
    import Foundation

    extension Cells {
        /// An edit of one file addressed by identity (the Cells repo's ENGINE.md, Writing):
        /// sent through `Cells.Daemon.apply`, applied once by its id however often it is
        /// sent, journalled as a root of the file's history, and written only over the
        /// version it was made against, or a later one where its ops say what they saw.
        public struct Intent: Sendable {
            /// Eight characters of `0-9a-z` but `i l o u`, made by the client: the intent's
            /// identity, what makes sending it again harmless.
            public let id: String
            public let ops: [Op]

            public init(id: String = Intent.newID(), ops: [Op]) {
                self.id = id
                self.ops = ops
            }

            /// A row, by what it is: its key's text, its id, or, in a file that has given its
            /// rows neither yet, its line exactly.
            public enum Row: Sendable, Hashable {
                case key(String)
                case id(String)
                case line(String)

                var json: [String: Any] {
                    switch self {
                    case .key(let k): ["key": k]
                    case .id(let i): ["id": i]
                    case .line(let l): ["line": l]
                    }
                }
            }

            public enum Op: Sendable, Hashable {
                /// A cell given `text`; `was`, the text it held when the edit was made, so an
                /// edit made against an earlier version applies only to the cell it saw.
                case set(row: Row, column: String, was: String?, text: String)
                /// A row added after the last, each named column given its text; its id, in a
                /// file identified by one, the engine's.
                case add(cells: [(column: String, text: String)])
                /// A row removed; `was`, its line as it was when the edit was made.
                case delete(row: Row, was: String?)
                /// The file whole, as a document holds it: an intent's one op. Made against
                /// the version on disk it is the file; made against an earlier one, the cells
                /// it set and rows it added and deleted are laid over the later writer's, and
                /// a change to the prelude or header, or a row moved, is refused.
                case text(String)

                var json: [String: Any] {
                    switch self {
                    case .text(let text):
                        return ["op": "text", "text": text]
                    case .set(let row, let column, let was, let text):
                        var o: [String: Any] = [
                            "op": "set", "row": row.json, "column": column, "text": text,
                        ]
                        if let was { o["was"] = was }
                        return o
                    case .add(let cells):
                        return [
                            "op": "add",
                            "cells": cells.map { ["column": $0.column, "text": $0.text] },
                        ]
                    case .delete(let row, let was):
                        var o: [String: Any] = ["op": "delete", "row": row.json]
                        if let was { o["was"] = was }
                        return o
                    }
                }

                public static func == (a: Op, b: Op) -> Bool {
                    NSDictionary(dictionary: a.json).isEqual(to: b.json)
                }

                public func hash(into hasher: inout Hasher) {
                    hasher.combine(String(describing: json))
                }
            }

            /// What the engine wrote: the file's version (its SHA-256) and the ids it gave
            /// rows that had none, in the rows' order.
            public struct Applied: Sendable, Hashable {
                public let version: String
                public let assigned: [String]
            }

            /// Crockford's base 32 in lower case, the engine's id alphabet.
            static let alphabet = Array("0123456789abcdefghjkmnpqrstvwxyz")

            /// A fresh intent id, random.
            public static func newID() -> String {
                String((0..<8).map { _ in alphabet[Int.random(in: 0..<32)] })
            }
        }
    }
#endif
