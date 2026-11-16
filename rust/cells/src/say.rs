//! How a message quotes the file: every part it names from it (a column, a table, a type as
//! spelled, a value, a formula or a part of one, a key, a word, a path) as Markdown's inline
//! code, so each reader can tell what the file wrote from the words around it. The Mac app
//! styles each span, the page renders it as code, a terminal and an editor print it as it is.
//!
//! A span is CommonMark's: a run of backticks, the text, the same run. Text holding a backtick
//! is fenced by a run longer than any inside it, with a space at each end (which a reader
//! strips); text with a space at both ends gets one more on each, which a reader strips back.

use alloc::format;
use alloc::string::String;

/// `text` as a message quotes it: `` `to` ``, `` `` a`b `` ``; blank text, which no span can
/// show, is the word `blank`.
pub fn code(text: &str) -> String {
    if text.is_empty() {
        return String::from("blank");
    }
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = "`".repeat(longest + 1);
    let padded = longest > 0 || (text.starts_with(' ') && text.ends_with(' ') && text.bytes().any(|b| b != b' '));
    match padded {
        true => format!("{fence} {text} {fence}"),
        false => format!("{fence}{text}{fence}"),
    }
}

#[cfg(test)]
mod tests {
    use super::code;

    #[test]
    fn spans() {
        assert_eq!(code("to"), "`to`");
        assert_eq!(code("8x.50"), "`8x.50`");
        assert_eq!(code("a`b"), "`` a`b ``");
        assert_eq!(code("a``b"), "``` a``b ```");
        assert_eq!(code("`"), "`` ` ``");
        assert_eq!(code(" a "), "`  a  `");
        assert_eq!(code("  "), "`  `");
        assert_eq!(code(" a"), "` a`");
        assert_eq!(code(""), "blank");
    }
}
