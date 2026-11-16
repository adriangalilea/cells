// The engine's words as the page shows them: every part of the file a message names is
// Markdown's inline code (rust/README.md, Messages), read by ui's code-spans (only code
// spans, so a `*` or a `_` in the words is never emphasis) and set here as <code>, the
// prose around it plain.

import * as React from "react"
import { codeSpans } from "@/lib/code-spans"

/** A message of the engine's, its quoted parts as code. */
export function Said({ text }: { text: string }) {
  return (
    <>
      {codeSpans(text).map((p, i) =>
        p.code ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: the pieces of one fixed string, in order
          <code key={i} className="font-mono text-white/85">
            {p.text}
          </code>
        ) : (
          // biome-ignore lint/suspicious/noArrayIndexKey: the pieces of one fixed string, in order
          <React.Fragment key={i}>{p.text}</React.Fragment>
        ),
      )}
    </>
  )
}
