// The suite's own checker: every case is well-formed by README.md's contract.
// `bun tests/validate.ts`. No implementation is involved; exits non-zero on any fault.

import { readdirSync, readFileSync, statSync } from "node:fs"
import { join, relative } from "node:path"

const ROOT = import.meta.dir
const FOLDERS = [
  "records",
  "header",
  "types",
  "choices",
  "constraints",
  "formats",
  "marks",
  "explicit",
  "prelude",
  "formulas",
  "tables",
  "imports",
  "inference",
  "problems",
  "write",
  "plain",
]
const SEPARATORS = [",", ";", "\t", "|"]
const NAMINGS = ["blank", "twice"]
const ROOT_FILES = new Set(["README.md", "validate.ts", ".gitattributes"])
const TAGS = new Set([
  "text",
  "int",
  "num",
  "pct",
  "bool",
  "date",
  "time",
  "datetime",
  "url",
  "email",
  "id",
  "money",
  "choice",
  "ref",
])
const KINDS = ["type", "missing", "key", "formula", "reference"]
const FROM = ["formula", "cell"]
const AT = /^(the file|the header|the prelude|column [1-9]\d*|prelude line [1-9]\d*|row (0|[1-9]\d*))$/
const DECIMAL = /^-?(0|[1-9]\d*)(\.\d+)?$/
const INTEGER = /^-?(0|[1-9]\d*)$/
const DATE = /^\d{4}-\d{2}-\d{2}$/
const TIME = /^\d{2}:\d{2}:\d{2}$/
const DATETIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d*[1-9])?Z$/
const CURRENCY = /^[A-Z]{3}$/
const TYPE_NAME = /^[A-Z][A-Za-z0-9_]*$/

const faults: string[] = []
let cases = 0
const fault = (path: string, why: string) => faults.push(`${relative(ROOT, path)}: ${why}`)

type Json = null | boolean | number | string | Json[] | { [k: string]: Json }
const isObject = (v: Json | undefined): v is { [k: string]: Json } =>
  typeof v === "object" && v !== null && !Array.isArray(v)

function keysAre(path: string, what: string, o: { [k: string]: Json }, required: string[], optional: string[] = []) {
  for (const k of required) if (!(k in o)) fault(path, `${what} has no "${k}"`)
  for (const k of Object.keys(o))
    if (!required.includes(k) && !optional.includes(k)) fault(path, `${what} has an unknown key "${k}"`)
}

/** A tagged value, or null: its tag, or a fault. */
function value(path: string, what: string, v: Json | undefined): string | null {
  if (v === null) return null
  if (!isObject(v)) {
    fault(path, `${what} is neither null nor a tagged value`)
    return null
  }
  if ("from" in v && (typeof v.from !== "string" || !FROM.includes(v.from))) fault(path, `${what}: "from" is formula or cell`)
  const tags = Object.keys(v).filter((k) => k !== "currency" && k !== "from")
  // `{ "from": "formula" }`: a computed cell that gave nothing.
  if (tags.length === 0 && "from" in v) return null
  if (tags.length !== 1) {
    fault(path, `${what} has ${tags.length} tags, not one`)
    return null
  }
  const tag = tags[0] as string
  if (!TAGS.has(tag)) {
    fault(path, `${what} has an unknown tag "${tag}"`)
    return null
  }
  if (tag === "money") {
    if (typeof v.currency !== "string" || !CURRENCY.test(v.currency))
      fault(path, `${what} is money with no currency code`)
  } else if ("currency" in v) fault(path, `${what} is ${tag} with a currency`)
  const x = v[tag]
  const shaped: Record<string, (x: Json | undefined) => boolean> = {
    int: (x) => typeof x === "string" && INTEGER.test(x),
    num: (x) => typeof x === "string" && DECIMAL.test(x),
    pct: (x) => typeof x === "string" && DECIMAL.test(x),
    money: (x) => typeof x === "string" && DECIMAL.test(x),
    bool: (x) => typeof x === "boolean",
    date: (x) => typeof x === "string" && DATE.test(x),
    time: (x) => typeof x === "string" && TIME.test(x),
    datetime: (x) => typeof x === "string" && DATETIME.test(x),
    text: (x) => typeof x === "string",
    url: (x) => typeof x === "string",
    email: (x) => typeof x === "string",
    id: (x) => typeof x === "string" && /^[0-9a-hjkmnp-tv-z]{8}$/.test(x),
    choice: (x) => typeof x === "string",
    ref: (x) => typeof x === "string",
  }
  if (!(shaped[tag] as (x: Json | undefined) => boolean)(x))
    fault(path, `${what}: ${JSON.stringify(x)} is not how the suite writes a ${tag}`)
  return tag === "money" ? `money ${v.currency}` : tag
}

function readsCase(path: string, j: { [k: string]: Json }, plain: boolean) {
  if (plain) {
    keysAre(path, "the case", j, ["separator", "columns", "rows", "problems"], ["names"])
    if (typeof j.separator !== "string" || !SEPARATORS.includes(j.separator))
      fault(path, `"separator" is not one of , ; tab |`)
    const namings = Array.isArray(j.names) ? j.names : []
    if ("names" in j && (!Array.isArray(j.names) || j.names.length === 0)) fault(path, `"names" is not a list of header problems`)
    namings.forEach((n, i) => {
      const what = `header problem ${i + 1}`
      if (!isObject(n)) return fault(path, `${what} is not an object`)
      keysAre(path, what, n, ["column", "kind"])
      if (typeof n.column !== "number" || !Number.isInteger(n.column) || n.column < 0) fault(path, `${what} names no column`)
      if (typeof n.kind !== "string" || !NAMINGS.includes(n.kind)) fault(path, `${what} has no known kind`)
    })
  } else keysAre(path, "the case", j, ["columns", "rows", "problems"], ["types"])
  const twice = plain && Array.isArray(j.names) && j.names.some((n) => isObject(n) && n.kind === "twice")
  if ("types" in j) {
    if (!isObject(j.types) || Object.keys(j.types).length === 0) fault(path, `"types" is not a map of names`)
    else
      for (const [name, def] of Object.entries(j.types)) {
        if (!TYPE_NAME.test(name)) fault(path, `type name "${name}" is not a capitalized word`)
        if (typeof def !== "string" || def === "") fault(path, `type ${name} has no definition`)
      }
  }
  const columns = Array.isArray(j.columns) ? j.columns : []
  if (!Array.isArray(j.columns)) fault(path, `"columns" is not a list`)
  const names: string[] = []
  columns.forEach((c, i) => {
    const what = `column ${i + 1}`
    if (!isObject(c)) return fault(path, `${what} is not an object`)
    keysAre(path, what, c, ["name", "type"], plain ? ["inferred"] : ["inferred", "formula"])
    if (plain && c.inferred !== true) fault(path, `${what}: a plain file's columns are all inferred`)
    if (typeof c.name !== "string" || (c.name === "" && !plain)) fault(path, `${what} has no name`)
    else if (names.includes(c.name) && !twice) fault(path, `${what}: the name "${c.name}" is taken`)
    else names.push(c.name)
    if (typeof c.type !== "string" || c.type === "") fault(path, `${what} has no type`)
    if ("inferred" in c && c.inferred !== true) fault(path, `${what}: "inferred" is only ever true`)
    if (c.inferred === true && typeof c.type === "string" && !c.type.endsWith("?"))
      fault(path, `${what} is inferred, so optional, but its type is "${c.type}"`)
    if ("formula" in c) {
      if (typeof c.formula !== "string" || c.formula.trim() !== c.formula || c.formula === "")
        fault(path, `${what}: "formula" is not a formula`)
      if ("inferred" in c) fault(path, `${what} has a formula, so a type, and is no inferred column`)
    }
  })
  const stored = names
  const rows = Array.isArray(j.rows) ? j.rows : []
  if (!Array.isArray(j.rows)) fault(path, `"rows" is not a list`)
  if (twice && rows.length > 0) fault(path, "two columns of one name, so rows keyed by name cannot say it: no rows")
  const kinds = new Map<string, string>()
  rows.forEach((r, n) => {
    if (!isObject(r)) return fault(path, `row ${n} is not an object`)
    for (const name of names) if (!(name in r)) fault(path, `row ${n} has no "${name}"`)
    for (const [name, v] of Object.entries(r)) {
      if (!names.includes(name)) {
        fault(path, `row ${n} holds "${name}", no column`)
        continue
      }
      const kind = value(path, `row ${n}, ${name}`, v)
      if (kind === null) continue
      const before = kinds.get(name)
      if (before === undefined) kinds.set(name, kind)
      else if (before !== kind) fault(path, `column "${name}" holds ${before} and ${kind}`)
      const column = columns.find((c) => isObject(c) && c.name === name)
      if (isObject(v) && v.from === "formula" && isObject(column) && !("formula" in column))
        fault(path, `row ${n}, ${name}: computed by a formula its column does not have`)
    }
  })
  const problems = Array.isArray(j.problems) ? j.problems : []
  if (!Array.isArray(j.problems)) fault(path, `"problems" is not a list`)
  let last: [number, number] = [-1, -1]
  problems.forEach((p, i) => {
    const what = `problem ${i + 1}`
    if (!isObject(p)) return fault(path, `${what} is not an object`)
    keysAre(path, what, p, ["row", "column", "kind"])
    if (typeof p.row !== "number" || !Number.isInteger(p.row) || p.row < 0 || p.row >= rows.length)
      return fault(path, `${what} names no row`)
    if (typeof p.column !== "string" || !stored.includes(p.column))
      return fault(path, `${what} names no column`)
    if (typeof p.kind !== "string" || !(plain ? ["ragged", "type"] : KINDS).includes(p.kind))
      return fault(path, `${what} has no known kind${plain ? " (a plain file's problems are ragged rows and control characters)" : ""}`)
    const at: [number, number] = [p.row, stored.indexOf(p.column)]
    if (at[0] < last[0] || (at[0] === last[0] && at[1] <= last[1]))
      fault(path, `${what} is out of order, or a second problem on one cell`)
    last = at
    const cell = (rows[p.row] as { [k: string]: Json })[p.column]
    const v = isObject(cell) && Object.keys(cell).every((k) => k === "from") ? null : cell
    if (p.kind === "key" && v === null) fault(path, `${what} is a duplicate key on a null value`)
    if (p.kind === "reference" && v === null) fault(path, `${what} is a reference to no row on a null value`)
    if (!["key", "ragged", "reference"].includes(p.kind) && v !== null) fault(path, `${what} is ${p.kind}, but the value is not null`)
  })
}

function errorCase(path: string, j: { [k: string]: Json }) {
  keysAre(path, "the case", j, ["error"])
  if (!isObject(j.error)) return fault(path, `"error" is not an object`)
  keysAre(path, "the error", j.error, ["at"])
  if (typeof j.error.at !== "string" || !AT.test(j.error.at))
    fault(path, `"${String(j.error.at)}" is no place an error is reported`)
}

function edits(path: string, j: Json) {
  if (!Array.isArray(j) || j.length === 0) return fault(path, "is not a list of edits")
  const named = (what: string, v: Json | undefined) => {
    if (typeof v !== "string") fault(path, `${what} names no column`)
  }
  const place = (what: string, v: Json | undefined) => {
    if (typeof v !== "number" || !Number.isInteger(v) || v < 0) fault(path, `${what} names no place`)
  }
  j.forEach((e, i) => {
    const what = `edit ${i + 1}`
    if (!isObject(e)) return fault(path, `${what} is not an object`)
    if ("as" in e) {
      keysAre(path, what, e, ["as"], ["header"])
      if (typeof e.as !== "string" || ![...SEPARATORS, "cells"].includes(e.as))
        fault(path, `${what}: "as" is a separator or "cells"`)
      if ("header" in e && (!Array.isArray(e.header) || !e.header.every((h) => typeof h === "string")))
        fault(path, `${what}: "header" is not a list of header cells`)
      if (i !== j.length - 1) fault(path, `${what}: "as" writes the file, so it is the last edit`)
      return
    }
    const refused = () => {
      if ("refused" in e && (typeof e.refused !== "string" || !AT.test(e.refused)))
        fault(path, `"${String(e.refused)}" is no place a refusal is reported`)
    }
    if ("insert" in e) {
      keysAre(path, what, e, ["insert", "header"], ["cells", "refused"])
      place(what, e.insert)
      if (typeof e.header !== "string" || e.header === "") fault(path, `${what}: "header" is not a header cell`)
      if ("cells" in e && (!Array.isArray(e.cells) || !e.cells.every((c) => typeof c === "string")))
        fault(path, `${what}: "cells" is not a list of texts`)
      return refused()
    }
    if ("remove" in e) {
      keysAre(path, what, e, ["remove"])
      return named(what, e.remove)
    }
    if ("move" in e) {
      keysAre(path, what, e, ["move", "to"])
      named(what, e.move)
      return place(what, e.to)
    }
    if ("rename" in e) {
      keysAre(path, what, e, ["rename", "to"])
      named(what, e.rename)
      if (typeof e.to !== "string" || e.to === "") fault(path, `${what}: "to" is not a name`)
      return
    }
    if (typeof e.column !== "string" || e.column === "") fault(path, `${what} names no column`)
    if ("type" in e) {
      keysAre(path, what, e, ["column", "type"], ["refused"])
      if (typeof e.type !== "string") fault(path, `${what}: "type" is not a spelling`)
    } else if ("text" in e) {
      keysAre(path, what, e, ["row", "column", "text"], ["refused"])
      if (typeof e.text !== "string") fault(path, `${what}: "text" is not text`)
    } else if ("value" in e) {
      keysAre(path, what, e, ["row", "column", "value"], ["refused"])
      value(path, what, e.value)
    } else fault(path, `${what} sets no text, value or type, and moves no column`)
    refused()
    if (!("type" in e) && (typeof e.row !== "number" || !Number.isInteger(e.row) || e.row < 0))
      fault(path, `${what} names no row`)
  })
}

const utf8 = new TextDecoder("utf-8", { fatal: true })
function isUTF8(path: string): boolean {
  try {
    utf8.decode(readFileSync(path))
    return true
  } catch {
    return false
  }
}
function bytes(path: string) {
  if (!isUTF8(path)) fault(path, "is not UTF-8")
}

function json(path: string): Json | undefined {
  try {
    return JSON.parse(readFileSync(path, "utf8"))
  } catch (e) {
    fault(path, `does not parse: ${(e as Error).message}`)
    return undefined
  }
}

/** A file of `plain/`: a case (`.csv`, `.tsv`, `.txt`) with its `.json`, or with its
 *  `.edit.json` and its `.out.` file of the same extension. */
function plainFile(dir: string, file: string, has: Set<string>) {
  const path = join(dir, file)
  const m = /^([a-z0-9]+(?:-[a-z0-9]+)*)(\.edit\.json|\.json|\.out\.(?:csv|tsv|txt)|\.(?:csv|tsv|txt))$/.exec(file)
  if (!m) return fault(path, "is not named as a plain case (lowercase words joined by -, .csv .tsv or .txt)")
  const [, stem, ext] = m as unknown as [string, string, string]
  const cases_ = [".csv", ".tsv", ".txt"].map((e) => `${stem}${e}`).filter((f) => has.has(f))
  if (cases_.length !== 1) return fault(path, `${stem} has ${cases_.length} case files, not one`)
  const own = (cases_[0] as string).slice(stem.length)
  if ([".csv", ".tsv", ".txt"].includes(ext)) {
    cases++
    bytes(path)
    const edited = has.has(`${stem}.edit.json`)
    if (edited === has.has(`${stem}.json`)) fault(path, `has a ${stem}.json or a ${stem}.edit.json, one of them`)
    if (edited && !has.has(`${stem}.out${own}`)) fault(path, `has no ${stem}.out${own}`)
    if (stem.startsWith("error-")) fault(path, "a plain file is never refused")
    return
  }
  if (ext.startsWith(".out.")) {
    if (ext !== `.out${own}`) fault(path, `is not ${stem}.out${own}`)
    return bytes(path)
  }
  const j = json(path)
  if (j === undefined) return
  if (ext === ".edit.json") return edits(path, j)
  if (!isObject(j)) return fault(path, "is not an object")
  readsCase(path, j, true)
}

for (const entry of readdirSync(ROOT)) {
  const path = join(ROOT, entry)
  if (statSync(path).isDirectory()) {
    if (!FOLDERS.includes(entry)) fault(path, "is no folder of the suite")
  } else if (!ROOT_FILES.has(entry)) fault(path, "does not belong at the suite's root")
}

for (const folder of FOLDERS) {
  const dir = join(ROOT, folder)
  let files: string[]
  try {
    files = readdirSync(dir)
  } catch {
    fault(dir, "is missing")
    continue
  }
  const has = new Set(files)
  for (const file of files) {
    const path = join(dir, file)
    if (statSync(path).isDirectory()) {
      fault(path, "a folder holds cases, not folders")
      continue
    }
    if (folder === "plain") {
      plainFile(dir, file, has)
      continue
    }
    if (!/^[a-z0-9]+(-[a-z0-9]+)*(\.edit\.json|\.out\.cell|\.cell|\.json)$/.test(file)) {
      fault(path, "is not named as a case (lowercase words joined by -)")
      continue
    }
    const stem = file.replace(/(\.edit\.json|\.out\.cell|\.cell|\.json)$/, "")
    if (folder === "write") {
      if (file.endsWith(".json") && !file.endsWith(".edit.json")) fault(path, "a write case has no .json")
      else if (!has.has(`${stem}.cell`)) fault(path, `has no ${stem}.cell`)
      if (file === `${stem}.cell`) {
        cases++
        bytes(path)
        if (!has.has(`${stem}.edit.json`)) fault(path, `has no ${stem}.edit.json`)
        if (!has.has(`${stem}.out.cell`)) fault(path, `has no ${stem}.out.cell`)
        if (stem.startsWith("error-")) fault(path, "a write case reads")
      }
      if (file.endsWith(".out.cell")) bytes(path)
      if (file.endsWith(".edit.json")) {
        const j = json(path)
        if (j !== undefined) edits(path, j)
      }
      continue
    }
    if (file.endsWith(".edit.json") || file.endsWith(".out.cell")) {
      fault(path, "edits belong in write/")
      continue
    }
    if (file.endsWith(".cell")) {
      cases++
      if (!has.has(`${stem}.json`)) fault(path, `has no ${stem}.json`)
      else {
        const j = json(join(dir, `${stem}.json`))
        const notText = isObject(j) && isObject(j.error) && j.error.at === "the file"
        if (notText) {
          if (isUTF8(path)) fault(path, `is UTF-8, so it is no error at the file`)
        } else bytes(path)
      }
      continue
    }
    if (!has.has(`${stem}.cell`)) fault(path, `has no ${stem}.cell`)
    const j = json(path)
    if (j === undefined) continue
    if (!isObject(j)) {
      fault(path, "is not an object")
      continue
    }
    const error = "error" in j
    if (error !== stem.startsWith("error-"))
      fault(path, error ? "does not read, so its name opens with error-" : "reads, so its name does not open with error-")
    if (error) errorCase(path, j)
    else readsCase(path, j, false)
  }
}

if (faults.length > 0) {
  for (const f of faults) console.error(`✗ ${f}`)
  console.error(`${faults.length} faults in ${cases} cases`)
  process.exit(1)
}
console.log(`${cases} cases, well-formed`)
