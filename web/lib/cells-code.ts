// CELLS IN CODE: `cells` (a `.cells` file, a ```` ```cells ```` fence) registered with
// code, read by the Cells engine (`tokens`, the wasm build), never by a pattern. The
// engine knows what every part of a file is, so a file is coloured the way a table is
// read, by its columns, as Rainbow CSV colours one: a column's name in the header,
// every cell under it whatever it spells, and every name a formula reads (`cost` in
// `=cost * qty`) share that column's colour. In a table colour means a column, so nothing else wears a hue: what is read
// is the text's own colour, what describes (types, bounds, punctuation) is muted, and
// only an error and a comment take the theme's colours; a script in its fence is its
// own language's. Import the module for its effect once in each
// place code runs and Cells is shown (a server layout for `Code`, the client module
// that draws an editor): code itself holds no language.
//
// The same reading reaches an editor as the language server's semantic tokens (`cells
// lsp`): one engine, every surface.

import type { Token, TokenKind } from "@/lib/cells"
import {
  type CodeSpan,
  registerLanguage,
} from "@/lib/code-tokens"

/** A column's colour, six in turn as the grammars cycle them: one hue each, apart from
 *  its neighbours and from red, which only an error wears, and at one lightness per
 *  scheme so no column shouts. Each clears 4.5:1 on code's light surface and 8.5:1 on
 *  its dark one (OKLCH L 0.50 C 0.15 light, L 0.80 C 0.12 dark, hues 200 295 60 145
 *  335 95). */
export const CELLS_COLUMNS = [
  { light: "#007a85", dark: "#43d5dc" },
  { light: "#6b4cae", dark: "#c3aeff" },
  { light: "#9e4700", dark: "#f6ab6b" },
  { light: "#107823", dark: "#8bd28d" },
  { light: "#943b84", dark: "#eea0dc" },
  { light: "#7e5f00", dark: "#d6bd5c" },
] as const

/** The ink of what describes a column rather than holds it (its type, a bound, a mark,
 *  the punctuation between cells): the text's own colour, quieter, so the names and the
 *  values lead. Over 5:1 on code's surface in either scheme. */
export const CELLS_MUTED = { light: "#57606a", dark: "#8b949e" } as const

/** What a part wears when no column colours it. In a table colour means a column, so
 *  the rest is ink, never a hue of the theme's that a column could be mistaken for: what
 *  is read (a formula's operators, numbers, functions and the words in capitals, a
 *  table's name, the prelude's statements) in the text's own colour, what describes in
 *  `CELLS_MUTED`, a comment in the theme's comment colour, and only an error in the
 *  theme's colour for one. */
const INKS: Record<
  Exclude<TokenKind, "code">,
  "plain" | "muted" | { scope: string }
> = {
  fence: "muted",
  comment: { scope: "comment.line" },
  keyword: "plain",
  name: "plain",
  type: "muted",
  mark: "muted",
  bound: "muted",
  format: "muted",
  choice: "muted",
  // A word's tint (`income#green`, a look's `negative red`) describes how it shows, so
  // it is ink as a type is: in its own hue it would read as a column's colour.
  tint: "muted",
  punct: "muted",
  number: "plain",
  reference: "plain",
  operator: "plain",
  cellText: "plain",
  cellNumber: "plain",
  cellBool: "plain",
  cellDate: "plain",
  cellChoice: "plain",
  cellLink: "plain",
  cellId: "plain",
  error: { scope: "invalid.illegal" },
  string: "plain",
  table: "plain",
}

/** The column a part is coloured by: a header's name and a cell by the column they
 *  stand in, a formula's name by the one it reads; none for the rest. */
function columnOf(t: Token): number | null {
  if (t.kind === "reference") return t.reads
  if (t.kind === "name" || t.kind.startsWith("cell")) return t.column
  return null
}

/** A fence's language, from its opening backticks (`` ```ts ``). */
const fenceLang = (spelled: string) =>
  /^`{3,}[ \t]*([A-Za-z][\w+-]*)/.exec(spelled)?.[1]

/** A file's tokens (the engine's `tokens(text)`) as code's spans: each part by its
 *  column or by what it is, and a script's lines (each a `code` token) joined back
 *  into one span in the language its fence names, so a string across them reads as
 *  one. */
export function cellsSpans(text: string, read: readonly Token[]): CodeSpan[] {
  const out: CodeSpan[] = []
  let lang: string | undefined
  let script: { from: number; to: number } | null = null
  const close = () => {
    if (script && lang) out.push({ ...script, lang })
    script = null
  }
  for (const t of read) {
    if (t.kind === "code") {
      if (!lang)
        throw new Error(
          `cells-code: a script at ${t.from} with no fence before it`,
        )
      script = script
        ? { from: script.from, to: t.to }
        : { from: t.from, to: t.to }
      continue
    }
    close()
    if (t.kind === "fence") lang = fenceLang(text.slice(t.from, t.to))
    const column = columnOf(t)
    if (column !== null)
      out.push({
        from: t.from,
        to: t.to,
        ink: CELLS_COLUMNS[
          column % CELLS_COLUMNS.length
        ] as (typeof CELLS_COLUMNS)[number],
        bold: t.kind === "name",
      })
    else {
      // Plain is what no span covers: the text's own colour.
      const ink = INKS[t.kind]
      if (ink === "muted")
        out.push({ from: t.from, to: t.to, ink: CELLS_MUTED })
      else if (ink !== "plain")
        out.push({ from: t.from, to: t.to, scope: ink.scope })
    }
  }
  close()
  return out
}

// The engine (its WebAssembly, half a megabyte as a module) loads when a Cells file is
// first coloured, never with the page that registers it.
registerLanguage("cells", {
  read: () =>
    import("@/lib/cells").then(
      ({ tokens }) =>
        (text: string) =>
          cellsSpans(text, tokens(text)),
    ),
})
