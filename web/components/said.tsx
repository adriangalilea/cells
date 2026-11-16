// The engine's words as the page shows them: every part of the file a message names is
// Markdown's inline code (rust/README.md, Messages), set here as <code>, the prose around
// it plain. Only code spans are read, so a `*` or a `_` in the words is never emphasis.

import * as React from "react"

/** A message's pieces: plain words, and the file's own, each as its span holds it. */
export function piecesOf(text: string): { code: boolean; text: string }[] {
  const out: { code: boolean; text: string }[] = []
  let plain = ""
  let at = 0
  while (at < text.length) {
    if (text[at] !== "`") {
      plain += text[at]
      at++
      continue
    }
    let run = 0
    while (text[at + run] === "`") run++
    const fence = "`".repeat(run)
    // the span closes at the next run of exactly as many backticks
    let close = text.indexOf(fence, at + run)
    while (close !== -1 && (text[close + run] === "`" || text[close - 1] === "`")) {
      let past = close
      while (text[past] === "`") past++
      close = text.indexOf(fence, past)
    }
    if (close === -1) {
      plain += fence
      at += run
      continue
    }
    let inside = text.slice(at + run, close)
    if (inside.startsWith(" ") && inside.endsWith(" ") && inside.trim() !== "") inside = inside.slice(1, -1)
    if (plain) out.push({ code: false, text: plain })
    plain = ""
    out.push({ code: true, text: inside })
    at = close + run
  }
  if (plain) out.push({ code: false, text: plain })
  return out
}

/** A message of the engine's, its quoted parts as code. */
export function Said({ text }: { text: string }) {
  return (
    <>
      {piecesOf(text).map((p, i) =>
        p.code ? (
          <code key={i} className="font-mono text-white/85">
            {p.text}
          </code>
        ) : (
          <React.Fragment key={i}>{p.text}</React.Fragment>
        ),
      )}
    </>
  )
}
