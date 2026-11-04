// Every scope the TextMate grammar promises, read back through Shiki (the
// Oniguruma engine VS Code and GitHub use). Each expectation is a piece of
// text and a scope its token must carry, found in order through the file, so
// a scope landing on the wrong occurrence fails too.
//
//   bun run test.ts

import { createHighlighter } from "shiki"
import grammar from "./tcsv.tmLanguage.json"

const FIXTURE = `---
use ../accounts.tcsv   // provisional
type Category = travel|stay|'it''s fine'
total = cost * (qty - 1)
---
item:text!,category:Category,cost:EUR>=0,paid:yes/no,on:date(DD/MM/YYYY),note:text?,score:0..10,to:accounts.account,x:one of a b key,y:bool si no optional,z:num between 0 and 100,w:num min 1
fuel,travel,84.20,yes,14/08/2026,"a ""b""",7,assets:bank,a,si,2026-08-14T10:00:00Z,https://example.com
a,"two
lines",b
`

const EXPECT: [string, string][] = [
  ["---", "punctuation.separator.prelude.tcsv"],
  ["use", "keyword.control.import.tcsv"],
  ["../accounts.tcsv", "string.unquoted.path.tcsv"],
  ["//", "punctuation.definition.comment.tcsv"],
  [" provisional", "comment.line.double-slash.tcsv"],
  ["type", "storage.type.tcsv"],
  ["Category", "entity.name.type.tcsv"],
  ["=", "keyword.operator.assignment.tcsv"],
  ["travel", "variable.other.enummember.choice.tcsv"],
  ["|", "punctuation.separator.choice.tcsv"],
  ["''", "constant.character.escape.tcsv"],
  ["total", "entity.name.function.computed.tcsv"],
  ["cost", "variable.other.property.tcsv"],
  ["*", "keyword.operator.arithmetic.tcsv"],
  ["(", "punctuation.section.parens.tcsv"],
  ["-", "keyword.operator.arithmetic.tcsv"],
  ["1", "constant.numeric.tcsv"],
  ["---", "punctuation.separator.prelude.tcsv"],
  // the header: each name in its column's colour, then its type's parts
  ["item", "meta.column.1.tcsv"],
  [":", "punctuation.separator.type.tcsv"],
  ["text", "support.type.primitive.tcsv"],
  ["!", "storage.modifier.key.tcsv"],
  [",", "punctuation.separator.comma.tcsv"],
  ["category", "meta.column.2.tcsv"],
  ["Category", "entity.name.type.tcsv"],
  ["cost", "meta.column.3.tcsv"],
  ["EUR", "support.type.currency.tcsv"],
  [">=", "keyword.operator.comparison.tcsv"],
  ["0", "constant.numeric.tcsv"],
  ["paid", "meta.column.4.tcsv"],
  ["yes", "constant.language.boolean.tcsv"],
  ["/", "punctuation.separator.boolean.tcsv"],
  ["no", "constant.language.boolean.tcsv"],
  ["on", "meta.column.5.tcsv"],
  ["date", "support.type.primitive.tcsv"],
  ["(", "punctuation.section.parens.begin.tcsv"],
  ["DD", "constant.other.placeholder.format.tcsv"],
  ["/", "punctuation.separator.format.tcsv"],
  ["YYYY", "constant.other.placeholder.format.tcsv"],
  [")", "punctuation.section.parens.end.tcsv"],
  ["note", "meta.column.6.tcsv"],
  ["?", "keyword.operator.optional.tcsv"],
  ["score", "meta.column.1.tcsv"],
  ["..", "keyword.operator.range.tcsv"],
  ["to", "meta.column.2.tcsv"],
  ["accounts", "entity.name.namespace.tcsv"],
  [".", "punctuation.accessor.tcsv"],
  ["account", "variable.other.property.tcsv"],
  ["x", "meta.column.3.tcsv"],
  ["one", "keyword.other.one-of.tcsv"],
  ["of", "keyword.other.one-of.tcsv"],
  ["a", "variable.other.enummember.choice.tcsv"],
  ["key", "storage.modifier.key.tcsv"],
  ["bool", "support.type.primitive.tcsv"],
  ["si", "constant.language.boolean.tcsv"],
  ["optional", "keyword.operator.optional.tcsv"],
  ["between", "keyword.operator.word.tcsv"],
  ["and", "keyword.operator.word.tcsv"],
  ["min", "keyword.operator.word.tcsv"],
  // a row: every cell in its column's colour, typed spellings scoped inside it
  ["fuel", "meta.column.1.tcsv"],
  ["travel", "meta.column.2.tcsv"],
  ["84.20", "constant.numeric.tcsv"],
  ["yes", "constant.language.boolean.tcsv"],
  ["14/08/2026", "meta.column.5.tcsv"],
  ['"', "punctuation.definition.string.begin.tcsv"],
  ['""', "constant.character.escape.tcsv"],
  ["7", "constant.numeric.tcsv"],
  ["assets:bank", "meta.column.2.tcsv"],
  ["2026-08-14T10:00:00Z", "constant.other.symbol.date.tcsv"],
  ["https://example.com", "markup.underline.link.tcsv"],
  // a quoted field across a line break stays one string
  ["two", "string.quoted.double.tcsv"],
  ["lines", "string.quoted.double.tcsv"],
  ['"', "punctuation.definition.string.end.tcsv"],
]

const h = await createHighlighter({ themes: ["github-dark"], langs: [grammar as never] })

function tokens(code: string): { text: string; scopes: string[] }[] {
  const out: { text: string; scopes: string[] }[] = []
  for (const line of h.codeToTokens(code, { lang: "tcsv", theme: "github-dark", includeExplanation: true }).tokens)
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
