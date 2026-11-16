// How a message the binding words itself quotes the file: the engine's rule (`cells::say::code`
// in rust/cells/src/say.rs), for the few messages composed here rather than in the engine.

/// `text` as a message quotes it, Markdown's inline code: `` `to` ``, `` `` a`b `` ``; blank
/// text, which no span can show, is the word `blank`.
func code(_ text: String) -> String {
    guard !text.isEmpty else { return "blank" }
    var longest = 0
    var run = 0
    for c in text {
        run = c == "`" ? run + 1 : 0
        longest = max(longest, run)
    }
    let fence = String(repeating: "`", count: longest + 1)
    let padded =
        longest > 0 || (text.hasPrefix(" ") && text.hasSuffix(" ") && text.contains { $0 != " " })
    return padded ? "\(fence) \(text) \(fence)" : "\(fence)\(text)\(fence)"
}
