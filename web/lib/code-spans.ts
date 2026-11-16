// Words that quote things: a message a program says to a person ("two columns are named
// `to`", "`8x.50` is not a number") marks what it quotes as Markdown's inline code, in
// backticks, so every reader can set it apart: a terminal prints the backticks, a page
// sets them as code. Only code spans are read: a `*` or an `_` in the words is never
// emphasis. The twin of swift-utils' `AttributedString(inkMarked:)`; framework-free.

/** A piece of a message: words as they are, or what it quotes (`code`). */
export interface CodeSpanPiece {
  code: boolean
  text: string
}

/** `text` quoted as a code span, as the engine writes one (`cells::say::code`): fenced by
 *  a run one longer than its longest run of backticks, one space inside each end when
 *  it begins or ends with a backtick or with a space (a reader strips them back). */
export function codeSpan(text: string): string {
  let longest = 0
  for (const run of text.match(/`+/g) ?? [])
    longest = Math.max(longest, run.length)
  const fence = "`".repeat(longest + 1)
  const pad = /^[` ]|[` ]$/.test(text) && text.trim() !== "" ? " " : ""
  return `${fence}${pad}${text}${pad}${fence}`
}

/** A message's pieces, its code spans read as CommonMark reads them: a run of
 *  backticks, the text, a run of exactly as many (`` `to` ``, ``` `` a`b `` ``` for one
 *  holding a backtick); one space each side is the fence's, not the span's, when both
 *  are there and the span is not spaces alone. A run never closed is words. */
export function codeSpans(text: string): CodeSpanPiece[] {
  const out: CodeSpanPiece[] = []
  let words = ""
  let at = 0
  while (at < text.length) {
    if (text[at] !== "`") {
      words += text[at]
      at++
      continue
    }
    let run = 0
    while (text[at + run] === "`") run++
    // the close: the next run of exactly as many backticks
    let close = -1
    let from = at + run
    while (from < text.length) {
      const next = text.indexOf("`", from)
      if (next === -1) break
      let length = 0
      while (text[next + length] === "`") length++
      if (length === run) {
        close = next
        break
      }
      from = next + length
    }
    if (close === -1) {
      words += "`".repeat(run)
      at += run
      continue
    }
    let quoted = text.slice(at + run, close)
    if (
      quoted.length > 1 &&
      quoted.startsWith(" ") &&
      quoted.endsWith(" ") &&
      quoted.trim() !== ""
    )
      quoted = quoted.slice(1, -1)
    if (words) out.push({ code: false, text: words })
    words = ""
    out.push({ code: true, text: quoted })
    at = close + run
  }
  if (words) out.push({ code: false, text: words })
  return out
}
