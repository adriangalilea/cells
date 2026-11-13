// Every scope the TextMate grammar promises, read back through Shiki (the
// Oniguruma engine VS Code and GitHub use). Each expectation is a piece of
// text and a scope its token must carry, found in order through the file, so
// a scope landing on the wrong occurrence fails too. The inline grammar is held
// the same way over fragments, one at a time.
//
//   bun run test.ts

import { createHighlighter } from "shiki"
import inline from "./cells-inline.tmLanguage.json"
import grammar from "./cells.tmLanguage.json"

const FIXTURE = `---
import accounts from "../accounts.cells"   // shared
import { Kind } from "./kinds.cells" with { rev: "a1b2c3d" }
export type Category = travel|stay|'it''s fine'
fees = \`\`\`cells
kind: text!,pct: pct
card,1.4
\`\`\`
rates = run every day at 18:00 in Europe/Madrid
\`\`\`ts
export default async () => []
\`\`\`
"price, net": EUR? = cost * 2
type Mood = calm#green|loud#3fa34d|'C#'
mood: one of yay tint blue meh
look Mood: grouped
look total: negative red, gray when score < 1, scale red..green
look on: as 'D [de] MMM'
look row: gray when paid
---
item: text!,category: Category,cost: EUR>=0,paid: yes/no,on: date(DD/MM/YYYY),note: text?,score: 0..10,to: accounts.account,x: one of a b key,y: bool si no optional,z: num between 0 and 100,w: num min 1,total: EUR = ABS(cost) * (score - 1),fee: EUR(6),running: EUR = running[-1] + total,cum: EUR = SUM(total[FIRST:THIS]),low: num = round(cost),memo: text,"vat: EUR = ROUND(cost, 2)","my ""best"" item"
fuel,travel,32.25=96.75 / 3,yes,14/08/2026,"a ""b""",7,assets:bank,a,si,2026-08-14T10:00:00Z,https://example.com,84.20=,0.001200,84.20=,=SUM(TABLE.total),84=,'=> see invoice
maps,stay,   "2.10=ROUND(6.30 / 3, 2)"
a,"two
lines",b
`

const EXPECT: [string, string][] = [
  ["---", "punctuation.separator.prelude.cells"],
  ["import", "keyword.control.import.cells"],
  ["accounts", "entity.name.namespace.cells"],
  ["from", "keyword.control.import.cells"],
  ['"../accounts.cells"', "string.quoted.double.path.cells"],
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
  ["1.4", "meta.column.2.cells"],
  ["```", "punctuation.definition.fence.cells"],
  ["rates", "entity.name.function.table.cells"],
  ["run", "keyword.control.run.cells"],
  ["every", "keyword.other.schedule.cells"],
  ["18:00", "constant.numeric.cells"],
  ["Europe/Madrid", "string.unquoted.schedule.cells"],
  ["ts", "entity.name.section.language.cells"],
  ["export", "meta.embedded.block.typescript"],
  // a column declared: its name, then its type and formula as a header cell's
  ['"price, net"', "variable.other.member.column.declared.cells"],
  [":", "punctuation.separator.type.cells"],
  ["EUR", "support.type.currency.cells"],
  ["cost", "variable.other.property.cells"],
  // a choice's tint, after `#` or `tint`
  ["calm", "variable.other.enummember.choice.cells"],
  ["#", "punctuation.definition.constant.tint.cells"],
  ["green", "constant.other.color.tint.cells"],
  ["3fa34d", "constant.other.color.tint.cells"],
  ["'C#'", "variable.other.enummember.choice.cells"],
  ["yay", "variable.other.enummember.choice.cells"],
  ["tint", "keyword.other.one-of.cells"],
  ["blue", "constant.other.color.tint.cells"],
  ["meh", "variable.other.enummember.choice.cells"],
  // how values look: what it is for, then its rules
  ["look", "keyword.other.look.cells"],
  ["Mood", "entity.name.type.cells"],
  ["grouped", "keyword.other.rule.look.cells"],
  ["total", "variable.other.member.column.declared.cells"],
  ["negative", "keyword.other.rule.look.cells"],
  ["red", "constant.other.color.tint.cells"],
  ["gray", "constant.other.color.tint.cells"],
  ["when", "keyword.other.rule.look.cells"],
  ["score", "variable.other.property.cells"],
  ["<", "keyword.operator.arithmetic.cells"],
  ["scale", "keyword.other.rule.look.cells"],
  ["..", "keyword.operator.range.cells"],
  ["as", "keyword.other.rule.look.cells"],
  ["D", "constant.other.placeholder.format.cells"],
  ["[de]", "punctuation.separator.format.cells"],
  ["MMM", "constant.other.placeholder.format.cells"],
  ["row", "variable.language.cells"],
  ["paid", "variable.other.property.cells"],
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
  [":", "punctuation.separator.type.cells"],
  ["EUR", "support.type.currency.cells"],
  ["=", "keyword.operator.assignment.cells"],
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
  // a row's cell is its column's, whatever it spells: a table is read by its columns
  ["yes", "meta.column.4.cells"],
  ["14/08/2026", "meta.column.5.cells"],
  ['"', "punctuation.definition.string.begin.cells"],
  ['""', "constant.character.escape.cells"],
  ["7", "meta.column.1.cells"],
  ["assets:bank", "meta.column.2.cells"],
  ["2026-08-14T10:00:00Z", "meta.column.5.cells"],
  ["https://example.com", "meta.column.6.cells"],
  // a value its column's formula computed
  ["84.20", "meta.column.1.cells"],
  ["=", "punctuation.definition.formula.cells"],
  ["0.001200", "meta.column.2.cells"],
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

// One table in the three spellings a reader takes as the same: the standard form a
// writer writes (`name: type = formula`), the same with its columns aligned as `cells
// fmt` pads them, and tight. Each header cell's name, `:`, type, `=` and formula carry
// the same scopes in all three, the padding none of them.
const FORMS: [string, string][] = [
  [
    "spaced",
    `item: text!,cost: EUR>=0,total: EUR = cost * 2,"vat: EUR = ROUND(cost, 2)",plain
fuel,32.25,=,=,x
`,
  ],
  [
    "aligned",
    `item: text!,    cost: EUR>=0,   total: EUR = cost * 2,  "vat: EUR = ROUND(cost, 2)",    plain
fuel,           32.25,          =,                      =,                              x
`,
  ],
  [
    "tight",
    `item:text!,cost:EUR>=0,total:EUR=cost * 2,"vat:EUR=ROUND(cost, 2)",plain
fuel,32.25,=,=,x
`,
  ],
]

const FORM_EXPECT: [string, string][] = [
  ["item", "meta.column.1.cells"],
  [":", "punctuation.separator.type.cells"],
  ["text", "support.type.primitive.cells"],
  ["!", "storage.modifier.key.cells"],
  ["cost", "meta.column.2.cells"],
  [":", "punctuation.separator.type.cells"],
  ["EUR", "support.type.currency.cells"],
  [">=", "keyword.operator.comparison.cells"],
  ["0", "constant.numeric.cells"],
  ["total", "meta.column.3.cells"],
  [":", "punctuation.separator.type.cells"],
  ["EUR", "support.type.currency.cells"],
  ["=", "keyword.operator.assignment.cells"],
  ["cost", "variable.other.property.cells"],
  ["*", "keyword.operator.arithmetic.cells"],
  ["2", "constant.numeric.cells"],
  ['"', "punctuation.definition.string.begin.cells"],
  ["vat", "meta.column.4.cells"],
  [":", "punctuation.separator.type.cells"],
  ["EUR", "support.type.currency.cells"],
  ["=", "keyword.operator.assignment.cells"],
  ["ROUND", "support.function.formula.cells"],
  ['"', "punctuation.definition.string.end.cells"],
  ["plain", "meta.column.5.cells"],
  ["fuel", "meta.column.1.cells"],
]

// Fragments as prose quotes them in inline code, each read alone by cells-inline: the
// shape decides where it enters the grammar.
const INLINE: [string, [string, string][]][] = [
  [
    "cost: EUR>=0",
    [
      ["cost", "meta.column.1.cells"],
      [":", "punctuation.separator.type.cells"],
      ["EUR", "support.type.currency.cells"],
      [">=", "keyword.operator.comparison.cells"],
    ],
  ],
  [
    "total: EUR = cost * qty",
    [
      ["total", "meta.column.1.cells"],
      [":", "punctuation.separator.type.cells"],
      ["EUR", "support.type.currency.cells"],
      ["=", "keyword.operator.assignment.cells"],
      ["cost", "variable.other.property.cells"],
      ["*", "keyword.operator.arithmetic.cells"],
      ["qty", "variable.other.property.cells"],
    ],
  ],
  // the tight spelling a reader takes as the same
  [
    "cost:EUR>=0",
    [
      ["cost", "meta.column.1.cells"],
      ["EUR", "support.type.currency.cells"],
      [">=", "keyword.operator.comparison.cells"],
    ],
  ],
  [
    "=SUM(total[FIRST:THIS])",
    [
      ["=", "punctuation.definition.formula.cells"],
      ["SUM", "support.function.formula.cells"],
      ["FIRST", "variable.language.cells"],
    ],
  ],
  [
    "32.25=96.75 / 3",
    [
      ["32.25", "constant.numeric.cells"],
      ["=", "punctuation.definition.formula.cells"],
      ["/", "keyword.operator.arithmetic.cells"],
    ],
  ],
  ["84.20=", [["84.20", "constant.numeric.cells"], ["=", "punctuation.definition.formula.cells"]]],
  [
    "running[-1] + total",
    [
      ["running", "variable.other.property.cells"],
      ["-1", "constant.numeric.cells"],
      ["+", "keyword.operator.arithmetic.cells"],
    ],
  ],
  ["2026-08-14", [["2026-08-14", "constant.other.symbol.date.cells"]]],
  ["yes", [["yes", "constant.language.boolean.cells"]]],
  // a text opening with its escaped `=` is a value, not an expression
  ["'=> see", [["'=", "constant.character.escape.cells"]]],
]

const h = await createHighlighter({
  themes: ["github-dark"],
  langs: [grammar as never, inline as never, "typescript"],
})

function tokens(code: string, lang = "cells"): { text: string; scopes: string[] }[] {
  const out: { text: string; scopes: string[] }[] = []
  for (const line of h.codeToTokens(code, { lang, theme: "github-dark", includeExplanation: true }).tokens)
    for (const token of line)
      for (const e of token.explanation ?? [])
        out.push({ text: e.content, scopes: e.scopes.map((s) => s.scopeName) })
  return out
}

function check(label: string, code: string, expect = EXPECT, lang = "cells"): number {
  const got = tokens(code, lang)
  let at = 0
  let failures = 0
  for (const [text, scope] of expect) {
    const found = got.findIndex((t, i) => i >= at && t.text === text && t.scopes.includes(scope))
    if (found < 0) {
      const near = got.slice(at).find((t) => t.text === text)
      console.error(`${label}: "${text}" should carry ${scope}, the next "${text}" has ${near ? near.scopes.join(" > ") : "no token"}`)
      failures++
    } else at = found + 1
  }
  if (failures || lang === "cells") console.log(`${label}: ${expect.length - failures}/${expect.length}`)
  return failures
}

// The same file with CRLF line endings must scope the same.
let failures = check("lf", FIXTURE) + check("crlf", FIXTURE.replaceAll("\n", "\r\n"))
failures += FORMS.reduce((n, [label, code]) => n + check(label, code, FORM_EXPECT), 0)
const inlineFailures = INLINE.reduce((n, [code, expect]) => n + check(`inline ${code}`, code, expect, "cells-inline"), 0)
console.log(`inline: ${INLINE.length} fragments${inlineFailures ? `, ${inlineFailures} wrong` : ""}`)
failures += inlineFailures
process.exit(failures === 0 ? 0 : 1)
