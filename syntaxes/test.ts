// Every scope the TextMate grammar promises, read back through Shiki (the
// Oniguruma engine VS Code and GitHub use). Each expectation is a piece of
// text and a scope its token must carry, found in order through the file, so
// a scope landing on the wrong occurrence fails too.
//
//   bun run test.ts

import { createHighlighter } from "shiki"
import grammar from "./cells.tmLanguage.json"

const FIXTURE = `---
import accounts from "../accounts.cell"   // shared
import { Kind } from "./kinds.cell" with { rev: "a1b2c3d" }
export type Category = travel|stay|'it''s fine'
fees = \`\`\`cells
kind:text!,pct:pct
card,1.4
\`\`\`
rates = run every day at 18:00 in Europe/Madrid
\`\`\`ts
export default async () => []
\`\`\`
---
item:text!,category:Category,cost:EUR>=0,paid:yes/no,on:date(DD/MM/YYYY),note:text?,score:0..10,to:accounts.account,x:one of a b key,y:bool si no optional,z:num between 0 and 100,w:num min 1,total:EUR=ABS(cost) * (score - 1),fee:EUR(6),running:EUR=running[-1] + total,cum:EUR=SUM(total[FIRST:THIS]),low:num=round(cost),memo:text,"vat:EUR=ROUND(cost, 2)","my ""best"" item"
fuel,travel,32.25=96.75 / 3,yes,14/08/2026,"a ""b""",7,assets:bank,a,si,2026-08-14T10:00:00Z,https://example.com,84.20=,0.001200,84.20=,=SUM(TABLE.total),84=,'=> see invoice
maps,stay,"2.10=ROUND(6.30 / 3, 2)"
a,"two
lines",b
`

const EXPECT: [string, string][] = [
  ["---", "punctuation.separator.prelude.cells"],
  ["import", "keyword.control.import.cells"],
  ["accounts", "entity.name.namespace.cells"],
  ["from", "keyword.control.import.cells"],
  ['"../accounts.cell"', "string.quoted.double.path.cells"],
  ["//", "punctuation.definition.comment.cells"],
  [" shared", "comment.line.double-slash.cells"],
  ["{", "punctuation.section.braces.cells"],
  ["Kind", "entity.name.namespace.cells"],
  ["with", "keyword.control.import.cells"],
  ["rev", "entity.other.attribute-name.import.cells"],
  ['"a1b2c3d"', "string.quoted.double.path.cells"],
  ["export", "storage.modifier.export.cells"],
  ["type", "storage.type.cells"],
  ["Category", "entity.name.type.cells"],
  ["=", "keyword.operator.assignment.cells"],
  ["travel", "variable.other.enummember.choice.cells"],
  ["|", "punctuation.separator.choice.cells"],
  ["''", "constant.character.escape.cells"],
  ["fees", "entity.name.function.table.cells"],
  ["```", "punctuation.definition.fence.cells"],
  ["cells", "entity.name.section.language.cells"],
  ["kind", "meta.column.1.cells"],
  ["text", "support.type.primitive.cells"],
  ["1.4", "constant.numeric.cells"],
  ["```", "punctuation.definition.fence.cells"],
  ["rates", "entity.name.function.table.cells"],
  ["run", "keyword.control.run.cells"],
  ["every", "keyword.other.schedule.cells"],
  ["18:00", "constant.numeric.cells"],
  ["Europe/Madrid", "string.unquoted.schedule.cells"],
  ["ts", "entity.name.section.language.cells"],
  ["export", "meta.embedded.block.typescript"],
  ["---", "punctuation.separator.prelude.cells"],
  // the header: each name in its column's colour, then its type's parts
  ["item", "meta.column.1.cells"],
  [":", "punctuation.separator.type.cells"],
  ["text", "support.type.primitive.cells"],
  ["!", "storage.modifier.key.cells"],
  [",", "punctuation.separator.comma.cells"],
  ["category", "meta.column.2.cells"],
  ["Category", "entity.name.type.cells"],
  ["cost", "meta.column.3.cells"],
  ["EUR", "support.type.currency.cells"],
  [">=", "keyword.operator.comparison.cells"],
  ["0", "constant.numeric.cells"],
  ["paid", "meta.column.4.cells"],
  ["yes", "constant.language.boolean.cells"],
  ["/", "punctuation.separator.boolean.cells"],
  ["no", "constant.language.boolean.cells"],
  ["on", "meta.column.5.cells"],
  ["date", "support.type.primitive.cells"],
  ["(", "punctuation.section.parens.begin.cells"],
  ["DD", "constant.other.placeholder.format.cells"],
  ["/", "punctuation.separator.format.cells"],
  ["YYYY", "constant.other.placeholder.format.cells"],
  [")", "punctuation.section.parens.end.cells"],
  ["note", "meta.column.6.cells"],
  ["?", "keyword.operator.optional.cells"],
  ["score", "meta.column.1.cells"],
  ["..", "keyword.operator.range.cells"],
  ["to", "meta.column.2.cells"],
  ["accounts", "entity.name.namespace.cells"],
  [".", "punctuation.accessor.cells"],
  ["account", "variable.other.property.cells"],
  ["x", "meta.column.3.cells"],
  ["one", "keyword.other.one-of.cells"],
  ["of", "keyword.other.one-of.cells"],
  ["a", "variable.other.enummember.choice.cells"],
  ["key", "storage.modifier.key.cells"],
  ["bool", "support.type.primitive.cells"],
  ["si", "constant.language.boolean.cells"],
  ["optional", "keyword.operator.optional.cells"],
  ["between", "keyword.operator.word.cells"],
  ["and", "keyword.operator.word.cells"],
  ["min", "keyword.operator.word.cells"],
  // a column's formula, after its type's `=`
  ["total", "meta.column.1.cells"],
  ["EUR", "support.type.currency.cells"],
  ["=", "keyword.operator.assignment.cells"],
  ["ABS", "support.function.formula.cells"],
  ["cost", "variable.other.property.cells"],
  ["*", "keyword.operator.arithmetic.cells"],
  ["score", "variable.other.property.cells"],
  ["1", "constant.numeric.cells"],
  // a currency's scale
  ["fee", "meta.column.2.cells"],
  ["(", "punctuation.section.parens.begin.cells"],
  ["6", "constant.numeric.cells"],
  [")", "punctuation.section.parens.end.cells"],
  // rows by where they are: the line above, and a range
  ["running", "meta.column.3.cells"],
  ["running", "variable.other.property.cells"],
  ["[", "punctuation.section.brackets.cells"],
  ["-1", "constant.numeric.cells"],
  ["]", "punctuation.section.brackets.cells"],
  ["total", "variable.other.property.cells"],
  ["SUM", "support.function.formula.cells"],
  ["FIRST", "variable.language.cells"],
  [":", "keyword.operator.range.cells"],
  ["THIS", "variable.language.cells"],
  // a word in lower case is a name, never a function
  ["round", "variable.other.property.cells"],
  // a quoted header cell reads as a bare one inside its quotes: its name, type, formula
  ['"', "punctuation.definition.string.begin.cells"],
  ["vat", "meta.column.1.cells"],
  ["EUR", "support.type.currency.cells"],
  ["ROUND", "support.function.formula.cells"],
  ["cost", "variable.other.property.cells"],
  ["2", "constant.numeric.cells"],
  ['"', "punctuation.definition.string.end.cells"],
  ['my ""best"" item', "meta.column.2.cells"],
  // a row: every cell in its column's colour, typed spellings scoped inside it
  ["fuel", "meta.column.1.cells"],
  ["travel", "meta.column.2.cells"],
  // a value and its own formula: what the file stores, `=`, how it is computed
  ["32.25", "meta.stored.cells"],
  ["=", "punctuation.definition.formula.cells"],
  ["96.75", "constant.numeric.cells"],
  ["/", "keyword.operator.arithmetic.cells"],
  ["yes", "constant.language.boolean.cells"],
  ["14/08/2026", "meta.column.5.cells"],
  ['"', "punctuation.definition.string.begin.cells"],
  ['""', "constant.character.escape.cells"],
  ["7", "constant.numeric.cells"],
  ["assets:bank", "meta.column.2.cells"],
  ["2026-08-14T10:00:00Z", "constant.other.symbol.date.cells"],
  ["https://example.com", "markup.underline.link.cells"],
  // a value its column's formula computed
  ["84.20", "constant.numeric.cells"],
  ["=", "punctuation.definition.formula.cells"],
  ["0.001200", "constant.numeric.cells"],
  // a formula not computed yet
  ["=", "punctuation.definition.formula.cells"],
  ["SUM", "support.function.formula.cells"],
  ["TABLE", "variable.language.cells"],
  // a text's `=`, written `'=`
  ["'=", "constant.character.escape.cells"],
  // a quoted cell holding its own formula: the same parts inside its quotes
  ['"', "punctuation.definition.string.begin.cells"],
  ["2.10", "meta.stored.cells"],
  ["=", "punctuation.definition.formula.cells"],
  ["ROUND", "support.function.formula.cells"],
  ["6.30", "constant.numeric.cells"],
  ['"', "punctuation.definition.string.end.cells"],
  // a quoted field across a line break stays one string
  ["two", "string.quoted.double.cells"],
  ["lines", "string.quoted.double.cells"],
  ['"', "punctuation.definition.string.end.cells"],
]

const h = await createHighlighter({ themes: ["github-dark"], langs: [grammar as never, "typescript"] })

function tokens(code: string): { text: string; scopes: string[] }[] {
  const out: { text: string; scopes: string[] }[] = []
  for (const line of h.codeToTokens(code, { lang: "cells", theme: "github-dark", includeExplanation: true }).tokens)
    for (const token of line)
      for (const e of token.explanation ?? [])
        out.push({ text: e.content, scopes: e.scopes.map((s) => s.scopeName) })
  return out
}

function check(label: string, code: string): number {
  const got = tokens(code)
  let at = 0
  let failures = 0
  for (const [text, scope] of EXPECT) {
    const found = got.findIndex((t, i) => i >= at && t.text === text && t.scopes.includes(scope))
    if (found < 0) {
      const near = got.slice(at).find((t) => t.text === text)
      console.error(`${label}: "${text}" should carry ${scope}, the next "${text}" has ${near ? near.scopes.join(" > ") : "no token"}`)
      failures++
    } else at = found + 1
  }
  console.log(`${label}: ${EXPECT.length - failures}/${EXPECT.length}`)
  return failures
}

// The same file with CRLF line endings must scope the same.
const failures = check("lf", FIXTURE) + check("crlf", FIXTURE.replaceAll("\n", "\r\n"))
process.exit(failures === 0 ? 0 : 1)
