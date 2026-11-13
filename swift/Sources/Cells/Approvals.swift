#if os(macOS)
    import CryptoKit
    import Foundation

    extension Cells {
        /// Which scripts may run on this Mac, as the Cells engine keeps them (the Cells repo's
        /// SPEC.md, Running, and `rust/cells-daemon/src/allowed.rs`): a script runs only at a
        /// version its person allowed after reading it, its file, its name and the SHA-256 of
        /// its language and code. The engine is the one writer (`Cells.Daemon.allow`); this
        /// reads what it kept, from `allowed.jsonl` beside its socket.
        public struct Approvals: Sendable {
            private let kept: Set<String>

            /// A script's version: the SHA-256 of its language, a line break and its code,
            /// in lower-case hex, as the engine computes it.
            public static func version(language: String, code: String) -> String {
                SHA256.hash(data: Data("\(language)\n\(code)".utf8)).map {
                    String(format: "%02x", $0)
                }.joined()
            }

            /// Where the engine keeps them.
            public static var file: URL {
                Daemon.socket.deletingLastPathComponent().appending(path: "allowed.jsonl")
            }

            /// The approvals as they are now; none when nothing was ever allowed.
            public static func read() -> Approvals {
                guard let text = try? String(contentsOf: file, encoding: .utf8) else {
                    return Approvals(kept: [])
                }
                let kept = text.split(separator: "\n").map { line -> String in
                    guard
                        let a = try? JSONSerialization.jsonObject(with: Data(line.utf8))
                            as? [String: Any],
                        let file = a["file"] as? String, let script = a["script"] as? String,
                        let version = a["version"] as? String
                    else {
                        preconditionFailure("\(Self.file.path): a line is no approval: \(line)")
                    }
                    return key(file, script, version)
                }
                return Approvals(kept: Set(kept))
            }

            /// Whether the script `name` of the file at `file` may run as it is written now.
            public func allows(file: URL, script name: String, language: String, code: String)
                -> Bool
            {
                kept.contains(
                    Self.key(
                        Daemon.canonical(file), name,
                        Self.version(language: language, code: code)))
            }

            private static func key(_ file: String, _ script: String, _ version: String) -> String {
                "\(file)\u{1f}\(script)\u{1f}\(version)"
            }
        }
    }
#endif
