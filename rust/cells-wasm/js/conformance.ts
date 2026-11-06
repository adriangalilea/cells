// The conformance suite (tests/README.md) through the wasm wrapper, under bun: every case
// read and compared, written back, and in write/ edited. Prints every case it fails and
// exits non-zero on any. `--lib` runs it through another copy of the wrapper (one a
// project vendored), whose imports resolve from where it lies.
//
//   bun rust/cells-wasm/js/conformance.ts [suite folder] [--lib path/to/cells.ts]

import { readdirSync, readFileSync, existsSync, statSync } from "node:fs"
import { join, relative, resolve } from "node:path"
import type { Column, Doc, Value } from "./cells.ts"

const args = process.argv.slice(2)
const libAt = args.indexOf("--lib")
const lib = libAt < 0 ? "./cells.ts" : resolve(args.splice(libAt, 2)[1] as string)
const { compose, detect, fixStale, read, readAs, reshape, setCell, setType, setValue, CellsError, write, writtenAs } = (await import(
  lib
)) as typeof import("./cells.ts")
type CellsError = InstanceType<typeof CellsError>
type Dialect = import("./cells.ts").Dialect
type ColumnAt = import("./cells.ts").ColumnAt
const root = args[0] ?? join(import.meta.dir, "../../../tests")

/** A case's file: a `.cell`, or under `plain/` a `.csv`, `.tsv` or `.txt`; never an
 *  edit's `.out.` file. */
function cases(dir: string): string[] {
  const plain = dir.endsWith("/plain")
  return readdirSync(dir)
    .sort()
    .flatMap((n) => {
      const p = join(dir, n)
      if (statSync(p).isDirectory()) return cases(p)
      if (n.includes(".out.")) return []
      return (plain ? /\.(csv|tsv|txt)$/ : /\.cell$/).test(n) ? [p] : []
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
  // `{ "from": "formula" }`: a computed cell that gave nothing.
  const tag = Object.keys(o).find((k) => k !== "currency" && k !== "from")
  if (tag === undefined) return null
  const raw = String(o[tag])
  return [tag, ["int", "num", "pct", "money"].includes(tag) ? normal(raw) : raw, o.currency as string | undefined]
}

interface Want {
  error?: { at: string }
  types?: Record<string, string>
  separator?: string
  names?: { column: number; kind: string }[]
  columns: { name: string; type: string; inferred?: boolean; formula?: string }[]
  rows: Record<string, unknown>[]
  problems: { row: number; column: string; kind: string; span: [number, number] }[]
  broken?: { at: string; span: [number, number] }[]
}

function check(doc: Doc, want: Want): string[] {
  const f: string[] = []
  if (want.columns.length !== doc.columns.length) return [`${doc.columns.length} columns, the suite says ${want.columns.length}`]
  want.columns.forEach((w, k) => {
    const c = doc.columns[k] as Column
    if (c.name !== w.name || c.type !== w.type || c.inferred !== (w.inferred ?? false) || c.formula !== w.formula)
      f.push(`column ${k}: ${JSON.stringify([c.name, c.type, c.inferred, c.formula])}, the suite says ${JSON.stringify(w)}`)
  })
  for (const [name, def] of Object.entries(want.types ?? {})) if (doc.named[name] !== def) f.push(`type ${name}: ${doc.named[name]}, the suite says ${def}`)
  if (want.rows.length !== doc.rowCount) f.push(`${doc.rowCount} rows, the suite says ${want.rows.length}`)
  else
    want.rows.forEach((w, r) => {
      for (const c of doc.columns) {
        const a = JSON.stringify(ours(c, r))
        const b = JSON.stringify(theirs(w[c.name]))
        if (a !== b) f.push(`row ${r}, ${c.name}: ${a}, the suite says ${b}`)
        const from = (w[c.name] as { from?: string } | null)?.from ?? "written"
        if (c.origin(r) !== from) f.push(`row ${r}, ${c.name}: from ${c.origin(r)}, the suite says ${from}`)
      }
    })
  const a = JSON.stringify(doc.problems().map((p) => [p.row, p.column, p.kind]))
  const b = JSON.stringify(want.problems.map((p) => [p.row, p.column, p.kind]))
  if (a !== b) f.push(`problems ${a}, the suite says ${b}`)
  for (const p of doc.problems())
    if (p.why !== `row ${p.row + 1}, ${p.column}: ${p.reason}`) f.push(`problem ${JSON.stringify(p.why)} is not its reason ${JSON.stringify(p.reason)} at its place`)
  const broken = JSON.stringify(doc.broken.map((b) => b.at))
  const wanted = JSON.stringify((want.broken ?? []).map((b) => b.at))
  if (broken !== wanted) f.push(`broken at ${broken}, the suite says ${wanted} (${doc.broken.map((b) => b.why).join("; ")})`)
  for (const b of doc.broken) if (b.why === "") f.push(`the break at ${b.at} has no words`)
  if (doc.dialect !== "cells") {
    if (doc.dialect !== want.separator) f.push(`separator ${JSON.stringify(doc.dialect)}, the suite says ${JSON.stringify(want.separator)}`)
    const ours = JSON.stringify(doc.names().map((n) => [n.column, n.kind]))
    const theirs = JSON.stringify((want.names ?? []).map((n) => [n.column, n.kind]))
    if (ours !== theirs) f.push(`names ${ours}, the suite says ${theirs}`)
    for (const n of doc.names()) if (n.reason === "") f.push(`header problem ${n.column} has no words`)
  }
  f.push(...hosted(doc))
  return f
}

/** Every problem's and break's span, the suite's in bytes of the file, ours in UTF-16
 *  units of its text. */
function spanned(doc: Doc, want: Want, bytes: Uint8Array): string[] {
  const units = (b: number) => text(bytes.subarray(0, b)).length
  const of = ([s, l]: [number, number]) => [units(s), units(s + l) - units(s)]
  const a = JSON.stringify([...doc.problems().map((p) => p.span), ...doc.broken.map((b) => b.span)])
  const b = JSON.stringify([...want.problems, ...(want.broken ?? [])].map((p) => of(p.span)))
  return a === b ? [] : [`spans ${a}, the suite says ${b}`]
}

/** What a host that keeps its own rows gets: `cells` is every cell's `text`, and the
 *  file `compose` writes from them reads back to the same cells and types. */
function hosted(doc: Doc): string[] {
  const f: string[] = []
  const cells = doc.cells()
  const stored = doc.columns
  cells.forEach((row, r) =>
    stored.forEach((c, k) => {
      if (row[k] !== c.text(r)) f.push(`cells()[${r}][${k}] ${JSON.stringify(row[k])}, text says ${JSON.stringify(c.text(r))}`)
    }),
  )
  const header = stored.map((c) => (c.spelled ? `${c.name}:${c.spelled}` : c.name))
  const composed = compose(doc, header, cells)
  // What did not read is written back as it was, after the rows.
  if (doc.unread !== "" && !composed.endsWith(doc.unread)) f.push("compose loses what the file did not read")
  if (header.length === 0) return f
  const again = readAs(composed, doc.dialect).doc
  if (again.unread !== doc.unread) f.push(`compose of its cells leaves ${JSON.stringify(again.unread)} unread, the file ${JSON.stringify(doc.unread)}`)
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
/** Every `.cell` file by its path in the suite, for a case's imports to follow. */
const files: Record<string, string> = {}
for (const p of all) if (p.endsWith(".cell")) files[relative(root, p)] = readFileSync(p, "utf8")
/** The edits applied in order, and the file's text after them: as written, or after a
 *  last `as` edit the file written in that dialect. */
function edited(start: Doc, edits: Record<string, unknown>[]): string {
  let doc = start
  const at = (name: unknown) => doc.columns.findIndex((c) => c.name === name)
  const kept = (): ColumnAt[] => doc.columns.map((_, k) => ({ kept: k }))
  for (const e of edits) {
    if ("as" in e) return writtenAs(doc, e.as as Dialect, e.header as string[] | undefined)
    if ("fix" in e) {
      fixStale(doc)
      continue
    }
    /** The edit made, against what the suite says of it: taken, or refused at `refused`. */
    const taken = (edit: () => void) => {
      try {
        edit()
      } catch (err) {
        if (!(err instanceof CellsError)) throw err
        if (err.where !== e.refused) throw new Error(`refused at ${err.where} (${err.why}), the suite says ${String(e.refused)}`)
        return
      }
      if (e.refused) throw new Error(`taken, the suite says it is refused at ${e.refused}`)
    }
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
    const o = order
    if (o)
      taken(() => {
        doc = reshape(doc, o).doc
      })
    else if ("type" in e) taken(() => setType(doc, e.column as string, e.type as string))
    else if ("text" in e) taken(() => setCell(doc, e.row as number, e.column as string, e.text as string))
    else taken(() => setValue(doc, e.row as number, e.column as string, value(e.value)))
  }
  return write(doc)
}

for (const path of all) {
  const bytes = new Uint8Array(readFileSync(path))
  const ext = path.slice(path.lastIndexOf("."))
  const stem = path.slice(0, -ext.length)
  const dialect: Dialect = ext === ".cell" ? "cells" : detect(bytes)
  const fails: string[] = []
  let got: ReturnType<typeof readAs> | CellsError
  try {
    got = dialect === "cells" ? read(bytes, { at: relative(root, path), files }) : readAs(bytes, dialect)
  } catch (e) {
    got = e as CellsError
  }
  if (existsSync(`${stem}.edit.json`)) {
    if (got instanceof CellsError) fails.push(`does not read: ${got.message}`)
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
    if (got instanceof CellsError) {
      if (!want.error) fails.push(`does not read: ${got.message}`)
      else if (got.where !== want.error.at) fails.push(`refused at ${got.where}, the suite says ${want.error.at}`)
    } else if (want.error) fails.push(`reads, the suite says it breaks at ${want.error.at}`)
    else {
      fails.push(...check(got.doc, want), ...spanned(got.doc, want, bytes))
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
