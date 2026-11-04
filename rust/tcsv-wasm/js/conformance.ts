// The conformance suite (tests/README.md) through the wasm wrapper, under bun: every case
// read and compared, written back, and in write/ edited. Prints every case it fails and
// exits non-zero on any. `--lib` runs it through another copy of the wrapper (one a
// project vendored), whose imports resolve from where it lies.
//
//   bun rust/tcsv-wasm/js/conformance.ts [suite folder] [--lib path/to/tcsv.ts]

import { readdirSync, readFileSync, existsSync, statSync } from "node:fs"
import { join, relative, resolve } from "node:path"
import type { Column, Doc, Value } from "./tcsv.ts"

const args = process.argv.slice(2)
const libAt = args.indexOf("--lib")
const lib = libAt < 0 ? "./tcsv.ts" : resolve(args.splice(libAt, 2)[1] as string)
const { compose, detect, readAs, reshape, setCell, setType, setValue, TcsvError, write, writtenAs } = (await import(
  lib
)) as typeof import("./tcsv.ts")
type TcsvError = InstanceType<typeof TcsvError>
type Dialect = import("./tcsv.ts").Dialect
type ColumnAt = import("./tcsv.ts").ColumnAt
const root = args[0] ?? join(import.meta.dir, "../../../tests")

/** A case's file: a `.tcsv`, or under `plain/` a `.csv`, `.tsv` or `.txt`; never an
 *  edit's `.out.` file. */
function cases(dir: string): string[] {
  const plain = dir.endsWith("/plain")
  return readdirSync(dir)
    .sort()
    .flatMap((n) => {
      const p = join(dir, n)
      if (statSync(p).isDirectory()) return cases(p)
      if (n.includes(".out.")) return []
      return (plain ? /\.(csv|tsv|txt)$/ : /\.tcsv$/).test(n) ? [p] : []
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
  separator?: string
  names?: { column: number; kind: string }[]
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
  for (const p of doc.problems())
    if (p.why !== `row ${p.row}, ${p.column}: ${p.reason}`) f.push(`problem ${JSON.stringify(p.why)} is not its reason ${JSON.stringify(p.reason)} at its place`)
  if (doc.dialect !== "tcsv") {
    if (doc.dialect !== want.separator) f.push(`separator ${JSON.stringify(doc.dialect)}, the suite says ${JSON.stringify(want.separator)}`)
    const ours = JSON.stringify(doc.names().map((n) => [n.column, n.kind]))
    const theirs = JSON.stringify((want.names ?? []).map((n) => [n.column, n.kind]))
    if (ours !== theirs) f.push(`names ${ours}, the suite says ${theirs}`)
    for (const n of doc.names()) if (n.reason === "") f.push(`header problem ${n.column} has no words`)
  }
  f.push(...hosted(doc))
  return f
}

/** What a host that keeps its own rows gets: `cells` is every cell's `text`, and the
 *  file `compose` writes from them reads back to the same cells and types. */
function hosted(doc: Doc): string[] {
  const f: string[] = []
  const cells = doc.cells()
  const stored = doc.columns.slice(0, doc.stored)
  cells.forEach((row, r) =>
    stored.forEach((c, k) => {
      if (row[k] !== c.text(r)) f.push(`cells()[${r}][${k}] ${JSON.stringify(row[k])}, text says ${JSON.stringify(c.text(r))}`)
    }),
  )
  const header = stored.map((c) => (c.spelled ? `${c.name}:${c.spelled}` : c.name))
  if (header.length === 0) return f
  const again = readAs(compose(doc, header, cells), doc.dialect).doc
  if (JSON.stringify(again.cells()) !== JSON.stringify(cells)) f.push("compose of its cells reads back other cells")
  if (JSON.stringify(again.columns.map((c) => c.type)) !== JSON.stringify(doc.columns.map((c) => c.type)))
    f.push("compose of its cells reads back other types")
  again.free()
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
/** The edits applied in order, and the file's text after them: as written, or after a
 *  last `as` edit the file written in that dialect. */
function edited(start: Doc, edits: Record<string, unknown>[]): string {
  let doc = start
  const at = (name: unknown) => doc.columns.findIndex((c) => c.name === name)
  const kept = (): ColumnAt[] => doc.columns.slice(0, doc.stored).map((_, k) => ({ kept: k }))
  for (const e of edits) {
    if ("as" in e) return writtenAs(doc, e.as as Dialect, e.header as string[] | undefined)
    let order: ColumnAt[] | null = null
    if ("insert" in e) {
      order = kept()
      order.splice(e.insert as number, 0, { header: e.header as string, cells: e.cells as string[] | undefined })
    } else if ("remove" in e) {
      order = kept()
      order.splice(at(e.remove), 1)
    } else if ("move" in e) {
      order = kept()
      const [moved] = order.splice(at(e.move), 1)
      order.splice(e.to as number, 0, moved as ColumnAt)
    } else if ("rename" in e) {
      order = kept()
      order[at(e.rename)] = { kept: at(e.rename), name: e.to as string }
    }
    if (order) doc = reshape(doc, order).doc
    else if ("type" in e) setType(doc, e.column as string, e.type as string)
    else if ("text" in e) setCell(doc, e.row as number, e.column as string, e.text as string)
    else setValue(doc, e.row as number, e.column as string, value(e.value))
  }
  return write(doc)
}

for (const path of all) {
  const bytes = new Uint8Array(readFileSync(path))
  const ext = path.slice(path.lastIndexOf("."))
  const stem = path.slice(0, -ext.length)
  const dialect: Dialect = ext === ".tcsv" ? "tcsv" : detect(bytes)
  const fails: string[] = []
  let got: ReturnType<typeof readAs> | TcsvError
  try {
    got = readAs(bytes, dialect)
  } catch (e) {
    got = e as TcsvError
  }
  if (existsSync(`${stem}.edit.json`)) {
    if (got instanceof TcsvError) fails.push(`does not read: ${got.message}`)
    else {
      if (write(got.doc) !== text(bytes)) fails.push("does not round-trip")
      try {
        const wrote = edited(got.doc, JSON.parse(readFileSync(`${stem}.edit.json`, "utf8")) as Record<string, unknown>[])
        const out = text(new Uint8Array(readFileSync(`${stem}.out${ext}`)))
        if (wrote !== out) fails.push(`writes ${JSON.stringify(wrote)}, the suite says ${JSON.stringify(out)}`)
      } catch (e) {
        fails.push(`an edit fails: ${(e as Error).message}`)
      }
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
