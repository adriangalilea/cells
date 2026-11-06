// Code as `code` draws it, as markup: Shiki with the two themes every code view here
// wears (CODE_THEMES, both in one render, `--shiki-light` and `--shiki-dark` on each
// token), the `[!code …]` notations read where asked, lines marked, and every token
// in the block's own colours unwrapped. One function, so a block on the server
// (`Code`), one prepared at build (markdown-prepare) and one coloured in the browser
// (markdown-code) are the same markup. Framework-free; Shiki loads on the first call.

import type { BundledLanguage, ShikiTransformer } from "shiki"
import {
  CODE_THEMES,
  codeColours,
  codeHighlighter,
  languageOf,
  linesOf,
  readerOf,
  registeredLanguage,
  runsOf,
} from "@/lib/code-tokens"

/** A language as code is told it: a Shiki id or alias, a registered language's id or
 *  alias (lib/code-tokens `registerLanguage`: `cells` once `cells-code` is imported), or
 *  `text`. */
export type CodeLang = BundledLanguage | "text" | (string & {})

export type CodeHtmlOptions = {
  /** `text` when it is not code, which skips the grammar. One neither Shiki nor
   *  `registerLanguage` knows throws (`knownLanguage` asks first). */
  lang: CodeLang
  /** Lines to mark, `"2"` or `"1,4-6"`. */
  highlight?: string
  /** Read `[!code …]` comments: off where the source is shown as itself. */
  notations?: boolean
}

/** One of the two inks a Shiki style names (`--shiki-light`, `--shiki-dark`). */
const inkOf = (style: string, name: string) =>
  new RegExp(`(?:^|;)${name}:([^;]+)`).exec(style)?.[1]?.toLowerCase()

/** A token in the block's own colours says nothing its line does not: the line wears
 *  them already (the `<pre>`'s `--shiki-light` and `--shiki-dark`, inherited, which
 *  code.css colours every span with), so its span is markup alone, two in five of a
 *  page's tokens. It goes, its text left in the line. Run last, once every notation
 *  has read the spans: a word highlight counts characters through them. */
const unwrapPlainTokens: ShikiTransformer = {
  name: "ag:unwrap-plain-tokens",
  root(root) {
    const pre = root.children.find(
      (n) => n.type === "element" && n.tagName === "pre",
    )
    if (pre?.type !== "element") throw new Error("code: Shiki drew no <pre>")
    const own = String(pre.properties.style ?? "")
    const light = inkOf(own, "--shiki-light")
    const dark = inkOf(own, "--shiki-dark")
    if (!light || !dark)
      throw new Error(`code: the <pre> names no inks of its own (${own})`)
    const plain = `--shiki-light:${light};--shiki-dark:${dark}`
    const code = pre.children.find(
      (n) => n.type === "element" && n.tagName === "code",
    )
    if (code?.type !== "element") throw new Error("code: Shiki drew no <code>")
    for (const line of code.children) {
      if (line.type !== "element") continue
      line.children = line.children.flatMap((token) =>
        token.type === "element" &&
        token.tagName === "span" &&
        Object.keys(token.properties).length === 1 &&
        String(token.properties.style).toLowerCase() === plain &&
        token.children.every((c) => c.type === "text")
          ? token.children
          : [token],
      )
    }
  },
}

/** Whether code knows `name` (Shiki's, a registered one, or `text`): a block in any
 *  other is drawn plain. */
export async function knownLanguage(name: string): Promise<boolean> {
  const lang = languageOf(name)
  if (lang === "text" || registeredLanguage(lang)) return true
  const { bundledLanguages } = await import("shiki")
  return lang in bundledLanguages
}

/** A reader language's colours as Shiki's tokens, a line each, so its markup is Shiki's
 *  own and every transformer reads it as any other's. */
function readTokens(
  source: string,
  colours: import("@/lib/code-tokens").CodeColours,
): ShikiTransformer {
  return {
    name: "ag:reader-tokens",
    tokens() {
      let offset = 0
      return runsOf(source, colours).map((line) => {
        const tokens = line.map((run) => {
          const ink = run.ink as NonNullable<typeof run.ink>
          const token = {
            content: run.text,
            offset,
            htmlStyle: {
              "--shiki-light": ink.light,
              "--shiki-dark": ink.dark,
              ...(ink.bold ? { "font-weight": "bold" } : {}),
              ...(ink.italic ? { "font-style": "italic" } : {}),
            },
          }
          offset += run.text.length
          return token
        })
        offset += 1
        return tokens
      })
    },
  }
}

/** `source` as Shiki's markup, in code's two themes, from the one highlighter every
 *  code view colours with (lib/code-tokens): a grammar language by Shiki, a reader
 *  language by its reader, the same markup either way. */
export async function codeHtml(
  source: string,
  { lang: name, highlight, notations = false }: CodeHtmlOptions,
): Promise<string> {
  const lang = languageOf(name)
  const read = readerOf(lang)
  const h = await codeHighlighter(read ? "text" : lang, CODE_THEMES)
  const reader = read
    ? [readTokens(source, await codeColours(source, lang, CODE_THEMES))]
    : []
  const marked = highlight ? linesOf(highlight) : null
  const notes = notations
    ? await import("@shikijs/transformers").then((t) => [
        t.transformerNotationDiff({ matchAlgorithm: "v3" }),
        t.transformerNotationHighlight({ matchAlgorithm: "v3" }),
        t.transformerNotationWordHighlight({ matchAlgorithm: "v3" }),
        t.transformerNotationFocus({ matchAlgorithm: "v3" }),
        t.transformerNotationErrorLevel({ matchAlgorithm: "v3" }),
      ])
    : []
  return h.codeToHtml(source, {
    lang: read ? "text" : lang,
    themes: CODE_THEMES,
    defaultColor: false,
    transformers: [
      ...reader,
      // The notations travel INSIDE the source, so they survive an edit that moves a
      // line; Shiki strips the comment from the markup, the clipboard keeps it.
      ...notes,
      // ...and `highlight` for source nobody may write into.
      {
        name: "ag:highlight-prop",
        line(node, line) {
          if (marked?.has(line)) this.addClassToHast(node, "highlighted")
        },
      },
      unwrapPlainTokens,
    ],
  })
}
