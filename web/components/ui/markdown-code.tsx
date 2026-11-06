"use client"

// A CODE BLOCK OF MARKDOWN: `code`'s own block (its frame, `CodeFrame`: the bar with
// the language and the copy, the lines numbered where asked), coloured by `code`'s
// own highlighter (lib/code-highlight). Coloured ahead where the text was prepared
// (markdown-prepare), so no highlighter loads; else in the browser once it shows,
// plain until then, so the page around it renders anywhere, a server included.

import * as React from "react"
import {
  codeHtml,
  knownLanguage,
} from "@/lib/code-highlight"
import { CodeFrame } from "@/components/ui/code-frame"

export interface HighlightedProps {
  text: string
  lang: string
  /** Shiki's markup of it, coloured ahead (markdown-prepare); null colours it here. */
  html?: string | null
  /** Number the lines (a fence's `lines`). */
  lines?: boolean
  /** Lines to mark, `"2"` or `"1,4-6"` (a fence's `{2,4-6}`). */
  highlight?: string
  /** A path or a name for the bar, in place of the language. */
  filename?: string
  /** The copy control; off for text that is not the whole of what it shows. */
  copy?: boolean
  className?: string
}

/** A code block, in `code`'s frame and colours. */
export function Highlighted({
  text,
  lang,
  html: ahead = null,
  lines,
  highlight,
  filename,
  copy,
  className,
}: HighlightedProps) {
  const [html, setHtml] = React.useState<string | null>(ahead)
  React.useEffect(() => {
    if (ahead !== null) return
    let live = true
    void (async () => {
      if (!(await knownLanguage(lang))) return
      const out = await codeHtml(text, { lang, highlight })
      if (live) setHtml(out)
    })()
    return () => {
      live = false
    }
  }, [text, lang, highlight, ahead])
  return (
    <CodeFrame
      source={text}
      html={html}
      lang={lang}
      lines={lines}
      filename={filename}
      copy={copy}
      className={className}
    />
  )
}
