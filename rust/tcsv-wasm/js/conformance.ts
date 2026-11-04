// The conformance suite (tests/README.md) through the wasm wrapper, under bun: every case
// read and compared, written back, and in write/ edited. Prints every case it fails and
// exits non-zero on any.
//
//   bun rust/tcsv-wasm/js/conformance.ts [suite folder]

import { readdirSync, readFileSync, existsSync, statSync } from "node:fs"
import { join, relative } from "node:path"
import { type Column, type Doc, read, setCell, setType, setValue, TcsvError, type Value, write } from "./tcsv.ts"

const root = process.argv[2] ?? join(import.meta.dir, "../../../tests")

function cases(dir: string): string[] {
  return readdirSync(dir)
    .sort()
    .flatMap((n) => {
      const p = join(dir, n)
      if (statSync(p).isDirectory()) return cases(p)
      return n.endsWith(".tcsv") && !n.endsWith(".out.tcsv") ? [p] : []
    })
}

/** A decimal string in one spelling. */
function normal(n: string): string {
  let neg = n.startsWith("-")
  let d = neg || n.startsWith("+") ? n.slice(1) : n
  let [int = "", frac = ""] = d.split(".")
  int = int.replace(/^0+/, "") || "0"
  frac = frac.replace(/0+$/, "")
  d = frac ? `${int}.${frac}` : int
  if (d === "0") neg = false
  return neg ? `-${d}` : d
}

type Tagged = [string, string, string | undefined] | null

function ours(c: Column, r: number): Tagged {
  if (c.isNull(r)) return null
  const v = c.kind === "decimal" ? normal(c.decimal(r) as string) : String(c.value(r))
  const tag = c.base
  return [tag, tag === "int" ? normal(v) : v, c.currency]
}

function theirs(v: unknown): Tagged {
  if (v === null) return null
  const o = v as Record<string, unknown>
  const tag = Object.keys(o).find((k) => k !== "currency") as string
  const raw = String(o[tag])
  return [tag, ["int", "num", "pct", "money"].includes(tag) ? normal(raw) : raw, o.currency as string | undefined]
}

interface Want {
  error?: { at: string }
  types?: Record<string, string>
  columns: { name: string; type: string; inferred?: boolean; computed?: string }[]
  rows: Record<string, unknown>[]
  problems: { row: number; column: string; kind: string }[]
}

function check(doc: Doc, want: Want): string[] {
  const f: string[] = []
  if (want.columns.length !== doc.columns.length) return [`${doc.columns.length} columns, the suite says ${want.columns.length}`]
  want.columns.forEach((w, k) => {
    const c = doc.columns[k] as Column
    if (c.name !== w.name || c.type !== w.type || c.inferred !== (w.inferred ?? false) || c.computed !== w.computed)
      f.push(`column ${k}: ${JSON.stringify([c.name, c.type, c.inferred, c.computed])}, the suite says ${JSON.stringify(w)}`)
  })
  for (const [name, def] of Object.entries(want.types ?? {})) if (doc.named[name] !== def) f.push(`type ${name}: ${doc.named[name]}, the suite says ${def}`)
  if (want.rows.length !== doc.rowCount) f.push(`${doc.rowCount} rows, the suite says ${want.rows.length}`)
  else
    want.rows.forEach((w, r) => {
      for (const c of doc.columns) {
        const a = JSON.stringify(ours(c, r))
        const b = JSON.stringify(theirs(w[c.name]))
        if (a !== b) f.push(`row ${r}, ${c.name}: ${a}, the suite says ${b}`)
      }
    })
  const a = JSON.stringify(doc.problems().map((p) => [p.row, p.column, p.kind]))
  const b = JSON.stringify(want.problems.map((p) => [p.row, p.column, p.kind]))
  if (a !== b) f.push(`problems ${a}, the suite says ${b}`)
  return f
}

/** A tagged suite value as the wrapper takes it. */
function value(v: unknown): Value {
  const t = theirs(v)
  if (t === null) return null
  return t[0] === "bool" ? t[1] === "true" : t[1]
}

const text = (b: Uint8Array) => new TextDecoder("utf-8", { ignoreBOM: true }).decode(b)

let passed = 0
let failed = 0
const all = cases(root)
for (const path of all) {
  const bytes = new Uint8Array(readFileSync(path))
  const stem = path.slice(0, -".tcsv".length)
  const fails: string[] = []
  let got: ReturnType<typeof read> | TcsvError
  try {
    got = read(bytes)
  } catch (e) {
    got = e as TcsvError
  }
  if (existsSync(`${stem}.edit.json`)) {
    if (got instanceof TcsvError) fails.push(`does not read: ${got.message}`)
    else {
      const doc = got.doc
      if (write(doc) !== text(bytes)) fails.push("does not round-trip")
      for (const e of JSON.parse(readFileSync(`${stem}.edit.json`, "utf8")) as Record<string, unknown>[]) {
        if ("type" in e) setType(doc, e.column as string, e.type as string)
        else if ("text" in e) setCell(doc, e.row as number, e.column as string, e.text as string)
        else setValue(doc, e.row as number, e.column as string, value(e.value))
      }
      const out = text(new Uint8Array(readFileSync(`${stem}.out.tcsv`)))
      if (write(doc) !== out) fails.push(`writes ${JSON.stringify(write(doc))}, the suite says ${JSON.stringify(out)}`)
    }
  } else {
    const want = JSON.parse(readFileSync(`${stem}.json`, "utf8")) as Want
    if (got instanceof TcsvError) {
      if (!want.error) fails.push(`does not read: ${got.message}`)
      else if (got.where !== want.error.at) fails.push(`refused at ${got.where}, the suite says ${want.error.at}`)
    } else if (want.error) fails.push(`reads, the suite says it breaks at ${want.error.at}`)
    else {
      fails.push(...check(got.doc, want))
      if (write(got.doc) !== text(bytes)) fails.push("does not round-trip")
    }
  }
  if (fails.length === 0) passed++
  else {
    failed++
    console.log(`FAIL ${relative(root, path)}`)
    for (const f of fails) console.log(`    ${f}`)
  }
}
console.log(`${passed} passed, ${failed} failed, of ${all.length}`)
process.exit(failed === 0 ? 0 : 1)
