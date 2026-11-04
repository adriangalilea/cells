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
const dir = mkdtempSync(join(tmpdir(), "tcsv-highlights-"))

const FIXTURE = `---
use ../accounts.tcsv   // provisional
type Category = travel|stay|'it''s fine'
total = cost * (qty - 1)
---
item:text!,category:Category,cost:EUR>=0,paid:yes/no,on:date(DD/MM/YYYY),note:text?,score:0..10,to:accounts.account,x:one of a b key,y:bool si no optional,z:num between 0 and 100,w:num min 1
fuel,travel,84.20,yes,14/08/2026,"a ""b""",7,assets:bank,a,si,2026-08-14T10:00:00Z,https://example.com
`

// [text, canonical capture, Helix scope]
const EXPECT: [string, string, string][] = [
  ["---", "punctuation.special", "punctuation.special"],
  ["use", "keyword.import", "keyword.control.import"],
  ["../accounts.tcsv", "string.special.path", "string.special.path"],
  ["// provisional", "comment", "comment.line"],
  ["type", "keyword.type", "keyword.storage.type"],
  ["Category", "type.definition", "type"],
  ["=", "operator", "operator"],
  ["travel", "constant", "type.enum.variant"],
  ["|", "punctuation.delimiter", "punctuation.delimiter"],
  ["'it''s fine'", "constant", "type.enum.variant"],
  ["''", "string.escape", "constant.character.escape"],
  ["total", "function", "function"],
  ["cost", "variable.member", "variable.other.member"],
  ["*", "operator", "operator"],
  ["(", "punctuation.bracket", "punctuation.bracket"],
  ["-", "operator", "operator"],
  ["1", "number", "constant.numeric"],
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
  // a row: text in its column's colour, everything else by its spelling
  ["fuel", "variable", "variable"],
  ["travel", "variable.parameter", "variable.parameter"],
  ["84.20", "number", "constant.numeric"],
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

const file = join(dir, "fixture.tcsv")
writeFileSync(file, FIXTURE)

// The same file with CRLF line endings must parse without an error.
const crlf = join(dir, "crlf.tcsv")
writeFileSync(crlf, FIXTURE.replaceAll("\n", "\r\n"))
const parsed = Bun.spawnSync(["tree-sitter", "parse", "--quiet", crlf], { cwd: root })
const crlfFailures = parsed.exitCode === 0 ? 0 : 1
console.log(`crlf: ${crlfFailures === 0 ? "parses" : `ERROR\n${parsed.stdout}`}`)

const failures =
  check("canonical", "queries/highlights.scm", file, 1) +
  check("helix", "queries/helix/highlights.scm", file, 2) +
  crlfFailures
process.exit(failures === 0 ? 0 : 1)
