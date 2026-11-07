// Every scope the queries promise, checked against the tree-sitter CLI's own
// captures, for both the canonical names and Helix's. Each expectation is a
// piece of text and the capture it must wear, found in order through the
// file, so a capture landing on the wrong occurrence fails too.
//
//   bun run test/highlights.ts

import { mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const root = join(import.meta.dir, "..")
const dir = mkdtempSync(join(tmpdir(), "cells-highlights-"))

const FIXTURE = `---
import accounts from "../accounts.cells"   // shared
import { Kind } from "./kinds.cells" with { rev: "a1b2c3d" }
export type Category = travel|stay|'it''s fine'
fees = \`\`\`cells
kind:text!,pct:pct
card,1.4
\`\`\`
rates = run every day at 18:00 in Europe/Madrid
\`\`\`ts
export default async () => []
\`\`\`
split dinner: EUR? = cost * 2
---
item:text!,category:Category,cost:EUR>=0,paid:yes/no,on:date(DD/MM/YYYY),note:text?,score:0..10,to:accounts.account,x:one of a b key,y:bool si no optional,z:num between 0 and 100,w:num min 1,total:EUR=ABS(cost) * (score - 1),fee:EUR(6),running:EUR=running[-1] + total,cum:EUR=SUM(total[FIRST:THIS]),low:num=round(cost)
fuel,travel,32.25=96.75 / 3,yes,14/08/2026,"a ""b""",7,assets:bank,a,si,2026-08-14T10:00:00Z,https://example.com,84.20=,0.001200,84.20=,=SUM(TABLE.total),84=
maps,stay,"2.10=ROUND(6.30 / 3, 2)"
`

// [text, canonical capture, Helix scope]
const EXPECT: [string, string, string][] = [
  ["---", "punctuation.special", "punctuation.special"],
  ["import", "keyword.import", "keyword.control.import"],
  ["accounts", "module", "namespace"],
  ["from", "keyword.import", "keyword.control.import"],
  ['"../accounts.cells"', "string.special.path", "string.special.path"],
  ["// shared", "comment", "comment.line"],
  ["{", "punctuation.bracket", "punctuation.bracket"],
  ["Kind", "module", "namespace"],
  ["with", "keyword.import", "keyword.control.import"],
  ["rev", "attribute", "attribute"],
  ['"a1b2c3d"', "string.special.path", "string.special.path"],
  ["export", "keyword.modifier", "keyword.storage.modifier"],
  ["type", "keyword.type", "keyword.storage.type"],
  ["Category", "type.definition", "type"],
  ["=", "operator", "operator"],
  ["travel", "constant", "type.enum.variant"],
  ["|", "punctuation.delimiter", "punctuation.delimiter"],
  ["'it''s fine'", "constant", "type.enum.variant"],
  ["''", "string.escape", "constant.character.escape"],
  ["fees", "function", "function"],
  ["```", "punctuation.special", "punctuation.special"],
  ["cells", "label", "label"],
  ["rates", "function", "function"],
  ["run", "keyword", "keyword"],
  ["every", "keyword", "keyword"],
  ["18:00", "number", "constant.numeric"],
  ["Europe/Madrid", "string.special", "string.special"],
  ["ts", "label", "label"],
  // a column declared: its name, its type, its formula
  ["split dinner", "variable.member", "variable.other.member"],
  ["EUR", "type.builtin", "type.builtin"],
  ["---", "punctuation.special", "punctuation.special"],
  // the header: each name in its column's colour, then its type's parts
  ["item", "variable", "variable"],
  [":", "punctuation.delimiter", "punctuation.delimiter"],
  ["text", "type.builtin", "type.builtin"],
  ["!", "attribute", "attribute"],
  [",", "punctuation.delimiter", "punctuation.delimiter"],
  ["category", "variable.parameter", "variable.parameter"],
  ["Category", "type", "type"],
  ["cost", "tag", "tag"],
  ["EUR", "type.builtin", "type.builtin"],
  [">=", "operator", "operator"],
  ["0", "number", "constant.numeric"],
  ["paid", "function", "function"],
  ["yes", "boolean", "constant.builtin.boolean"],
  ["/", "punctuation.delimiter", "punctuation.delimiter"],
  ["no", "boolean", "constant.builtin.boolean"],
  ["on", "variable.member", "variable.other.member"],
  ["date", "type.builtin", "type.builtin"],
  ["(", "punctuation.bracket", "punctuation.bracket"],
  ["DD", "string.special", "string.special"],
  ["/", "punctuation.special", "punctuation.special"],
  ["YYYY", "string.special", "string.special"],
  [")", "punctuation.bracket", "punctuation.bracket"],
  ["note", "label", "label"],
  ["?", "keyword.modifier", "keyword.storage.modifier"],
  ["score", "variable", "variable"],
  ["..", "operator", "operator"],
  ["to", "variable.parameter", "variable.parameter"],
  ["accounts", "module", "namespace"],
  [".", "punctuation.delimiter", "punctuation.delimiter"],
  ["account", "variable.member", "variable.other.member"],
  ["x", "tag", "tag"],
  ["one", "keyword", "keyword"],
  ["of", "keyword", "keyword"],
  ["a", "constant", "type.enum.variant"],
  ["key", "attribute", "attribute"],
  ["bool", "type.builtin", "type.builtin"],
  ["si", "boolean", "constant.builtin.boolean"],
  ["optional", "keyword.modifier", "keyword.storage.modifier"],
  ["between", "keyword.operator", "keyword.operator"],
  ["and", "keyword.operator", "keyword.operator"],
  ["min", "keyword.operator", "keyword.operator"],
  // a column's formula, after its type's `=`
  ["total", "variable", "variable"],
  ["EUR", "type.builtin", "type.builtin"],
  ["=", "operator", "operator"],
  ["ABS", "function", "function"],
  ["cost", "variable.member", "variable.other.member"],
  ["*", "operator", "operator"],
  ["(", "punctuation.bracket", "punctuation.bracket"],
  ["score", "variable.member", "variable.other.member"],
  ["-", "operator", "operator"],
  ["1", "number", "constant.numeric"],
  // a currency's scale
  ["fee", "variable.parameter", "variable.parameter"],
  ["(", "punctuation.bracket", "punctuation.bracket"],
  ["6", "number", "constant.numeric"],
  [")", "punctuation.bracket", "punctuation.bracket"],
  // rows by where they are: the line above, and a range
  ["running", "tag", "tag"],
  ["running", "variable.member", "variable.other.member"],
  ["[", "punctuation.bracket", "punctuation.bracket"],
  ["-1", "number", "constant.numeric"],
  ["]", "punctuation.bracket", "punctuation.bracket"],
  ["total", "variable.member", "variable.other.member"],
  ["cum", "function", "function"],
  ["SUM", "function", "function"],
  ["FIRST", "variable.builtin", "variable.builtin"],
  [":", "operator", "operator"],
  ["THIS", "variable.builtin", "variable.builtin"],
  // a word in lower case is a name, called or not
  ["low", "variable.member", "variable.other.member"],
  ["round", "variable.member", "variable.other.member"],
  // a row: text in its column's colour, everything else by its spelling
  ["fuel", "variable", "variable"],
  ["travel", "variable.parameter", "variable.parameter"],
  // a value and its own formula: what the file stores, `=`, how it is computed
  ["32.25", "constant.builtin.computed", "constant.builtin.computed"],
  ["=", "punctuation.special", "punctuation.special"],
  ["96.75", "number", "constant.numeric"],
  ["/", "operator", "operator"],
  ["yes", "boolean", "constant.builtin.boolean"],
  // a date in the column's own format is text to the grammar; the language
  // server, which reads the header, colours it as the date it is
  ["14/08/2026", "variable.member", "variable.other.member"],
  ['"a ""b"""', "string", "string"],
  ['""', "string.escape", "constant.character.escape"],
  ["7", "number", "constant.numeric"],
  ["assets:bank", "variable.parameter", "variable.parameter"],
  ["2026-08-14T10:00:00Z", "string.special", "string.special"],
  ["https://example.com", "string.special.url", "string.special.url"],
  // a value its column's formula computed, and a formula not computed yet
  ["84.20", "constant.builtin.computed", "constant.builtin.computed"],
  ["=", "punctuation.special", "punctuation.special"],
  ["0.001200", "number", "constant.numeric"],
  ["=", "punctuation.special", "punctuation.special"],
  ["SUM", "function", "function"],
  ["TABLE", "variable.builtin", "variable.builtin"],
  // a quoted cell holding its own formula: the formula coloured inside the quotes
  ["2.10", "constant.builtin.computed", "constant.builtin.computed"],
  ["=", "punctuation.special", "punctuation.special"],
  ["ROUND", "function", "function"],
  ["6.30", "number", "constant.numeric"],
  ["/", "operator", "operator"],
]

type Capture = { name: string; text: string }

function captures(query: string, file: string): Capture[] {
  const run = Bun.spawnSync(["tree-sitter", "query", query, file, "--captures"], { cwd: root })
  if (run.exitCode !== 0) throw new Error(`tree-sitter query failed: ${run.stderr.toString()}`)
  const out: Capture[] = []
  for (const line of run.stdout.toString().split("\n")) {
    const m = /capture: \d+ - ([\w.]+), start: .*?, text: `([\s\S]*)`$/.exec(line)
    if (m) out.push({ name: m[1] as string, text: m[2] as string })
  }
  return out
}

function check(label: string, query: string, file: string, column: 1 | 2): number {
  const got = captures(query, file)
  let at = 0
  let failures = 0
  for (const e of EXPECT) {
    const [text, want] = [e[0], e[column]]
    const found = got.findIndex((c, i) => i >= at && c.text === text && c.name === want)
    if (found < 0) {
      const near = got.slice(at).find((c) => c.text === text)
      console.error(`${label}: "${text}" should be @${want}, the next "${text}" is ${near ? `@${near.name}` : "not captured"}`)
      failures++
    } else at = found + 1
  }
  console.log(`${label}: ${EXPECT.length - failures}/${EXPECT.length}`)
  return failures
}

const file = join(dir, "fixture.cells")
writeFileSync(file, FIXTURE)

// The same file with CRLF line endings must parse without an error.
const crlf = join(dir, "crlf.cells")
writeFileSync(crlf, FIXTURE.replaceAll("\n", "\r\n"))
const parsed = Bun.spawnSync(["tree-sitter", "parse", "--quiet", crlf], { cwd: root })
const crlfFailures = parsed.exitCode === 0 ? 0 : 1
console.log(`crlf: ${crlfFailures === 0 ? "parses" : `ERROR\n${parsed.stdout}`}`)

const failures =
  check("canonical", "queries/highlights.scm", file, 1) +
  check("helix", "queries/helix/highlights.scm", file, 2) +
  crlfFailures
process.exit(failures === 0 ? 0 : 1)
