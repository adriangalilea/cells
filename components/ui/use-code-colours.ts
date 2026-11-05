"use client"

import * as React from "react"
import {
  CODE_THEMES,
  type CodeColours,
  type CodeThemes,
  codeColours,
} from "@/lib/code-tokens"

/** `text`'s colours in `lang` (lib/code-tokens), read in the browser once it
 *  renders: null until Shiki has loaded, so a view draws the text plain at once and
 *  coloured a moment later, never blank. `given`, colours already read (on the
 *  server, for a still), are used as they are, and the browser loads no Shiki. */
export function useCodeColours(
  text: string,
  lang: string,
  themes: CodeThemes = CODE_THEMES,
  given?: CodeColours,
): CodeColours | null {
  const [colours, setColours] = React.useState<{
    key: string
    colours: CodeColours
  } | null>(null)
  const { light, dark } = themes
  const key = `${light} ${dark} ${lang}\n${text}`
  React.useEffect(() => {
    if (given) return
    let live = true
    void codeColours(text, lang, { light, dark }).then((c) => {
      if (live) setColours({ key, colours: c })
    })
    return () => {
      live = false
    }
  }, [given, key, text, lang, light, dark])
  if (given) return given
  return colours?.key === key ? colours.colours : null
}

const NONE: readonly (CodeColours | null)[] = []

/** Many texts' colours at once (every state an edit passes through), read once in
 *  the browser, so a view moving from one to the next never waits for one: none
 *  until all have loaded, then each at its index. `texts` keeps its identity while
 *  it holds the same texts. */
export function useCodeColoursOf(
  texts: readonly { text: string; lang: string }[],
  themes: CodeThemes = CODE_THEMES,
): readonly (CodeColours | null)[] {
  const [read, setRead] = React.useState<{
    texts: readonly { text: string; lang: string }[]
    colours: CodeColours[]
  } | null>(null)
  const { light, dark } = themes
  React.useEffect(() => {
    let live = true
    void Promise.all(
      texts.map((t) => codeColours(t.text, t.lang, { light, dark })),
    ).then((colours) => {
      if (live) setRead({ texts, colours })
    })
    return () => {
      live = false
    }
  }, [texts, light, dark])
  return read?.texts === texts ? read.colours : NONE
}
