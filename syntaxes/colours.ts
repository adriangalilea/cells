// Which colours each showcase theme gives the Cells scopes: every example
// tokenized with Shiki under each theme, every token filed under the scope it
// stands in, and the colours each scope got. A rainbow column that shares a
// colour with another column, or with a value's kind, is reported; so is a
// header scope that shares one with another.
//
//   bun run colours.ts

import { readdirSync, readFileSync, statSync } from "node:fs"
import { join } from "node:path"
import { createHighlighter } from "shiki"
import grammar from "./cells.tmLanguage.json"
import { PAIRS } from "./themes.ts"

const examples = join(import.meta.dir, "../tree-sitter/examples")
const files = (dir: string): string[] =>
  readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    return statSync(path).isDirectory() ? files(path) : name.endsWith(".cells") ? [path] : []
  })

// The innermost scope a token stands in, named by what it is in Cells.
const KINDS: [string, string][] = [
  ["support.type.primitive.cells", "base type"],
  ["support.type.currency.cells", "currency"],
  ["entity.name.type.cells", "named type"],
  ["variable.other.enummember.choice.cells", "choice"],
  ["storage.modifier.key.cells", "key !"],
  ["keyword.operator.optional.cells", "optional ?"],
  ["constant.numeric.cells", "number"],
  ["constant.other.symbol.date.cells", "date"],
  ["string.quoted.double.cells", "quoted"],
  ["constant.language.boolean.cells", "boolean"],
  ["markup.underline.link.cells", "link"],
]
const TYPES = ["base type", "currency", "named type", "choice", "key !", "optional ?"]
const VALUES = ["number", "date", "quoted", "boolean", "link"]
const COLUMNS = [1, 2, 3, 4, 5, 6].map((n) => `column ${n}`)

function kind(scopes: string[]): string | undefined {
  const inner = scopes.at(-1)!
  const column = scopes.findLast((s) => /^meta\.column\.\d\.cells$/.test(s))
  const columnOwn =
    inner === column || inner.endsWith(".column.cells") || inner === "string.quoted.double.name.cells"
  if (column && columnOwn) return `column ${column.split(".")[2]}`
  return KINDS.find(([scope]) => inner === scope)?.[1]
}

const themeNames = PAIRS.flatMap((p) => [p.dark, p.light])
const highlighter = await createHighlighter({ themes: themeNames, langs: [grammar as never, "typescript"] })
const codes = files(examples).map((path) => readFileSync(path, "utf8"))

const report: { theme: string; colours: Map<string, Set<string>> }[] = themeNames.map((theme) => {
  const colours = new Map<string, Set<string>>()
  for (const code of codes)
    for (const line of highlighter.codeToTokens(code, { lang: "cells", theme, includeExplanation: true }).tokens)
      for (const token of line)
        for (const piece of token.explanation ?? []) {
          if (!piece.content.trim()) continue
          const k = kind(piece.scopes.map((s) => s.scopeName))
          if (!k) continue
          const colour = (token.color ?? "").toUpperCase()
          colours.set(k, (colours.get(k) ?? new Set()).add(colour))
        }
  return { theme, colours }
})

for (const { theme, colours } of report) {
  const of = (k: string) => [...(colours.get(k) ?? [])]
  const unseen = [...COLUMNS, ...TYPES, ...VALUES].filter((k) => !colours.has(k))
  if (unseen.length) throw new Error(`${theme}: the examples never reach ${unseen.join(", ")}`)
  const clashes = (a: string[], b: string[]) =>
    a.flatMap((x, i) => b.filter((y, j) => (a === b ? j > i : true) && of(x).some((c) => of(y).includes(c))).map((y) => `${x}=${y}`))

  // A value painted in more than one colour has none of its own: the theme
  // leaves its scope bare and it wears its column's.
  const bare = VALUES.filter((k) => of(k).length > 1)
  const own = VALUES.filter((k) => !bare.includes(k))

  const distinctColumns = new Set(COLUMNS.flatMap(of)).size
  console.log(`\n${theme}: ${distinctColumns} colours across the six columns`)
  for (const k of [...COLUMNS, ...TYPES, ...own]) console.log(`  ${k.padEnd(11)} ${of(k).join(" ")}`)
  console.log(`  columns sharing: ${clashes(COLUMNS, COLUMNS).join(", ") || "none"}`)
  console.log(`  column = value:  ${clashes(COLUMNS, own).join(", ") || "none"}`)
  console.log(`  column's colour: ${bare.join(", ") || "none"}`)
  console.log(`  type sharing:    ${clashes(TYPES, TYPES).join(", ") || "none"}`)
}
