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
  "inference",
  "problems",
  "write",
]
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
])
const KINDS = ["type", "missing", "key"]
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
  const tags = Object.keys(v).filter((k) => k !== "currency")
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
  }
  if (!(shaped[tag] as (x: Json | undefined) => boolean)(x))
    fault(path, `${what}: ${JSON.stringify(x)} is not how the suite writes a ${tag}`)
  return tag === "money" ? `money ${v.currency}` : tag
}

function readsCase(path: string, j: { [k: string]: Json }) {
  keysAre(path, "the case", j, ["columns", "rows", "problems"], ["types"])
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
  const stored: string[] = []
  let computedSeen = false
  columns.forEach((c, i) => {
    const what = `column ${i + 1}`
    if (!isObject(c)) return fault(path, `${what} is not an object`)
    keysAre(path, what, c, ["name", "type"], ["inferred", "computed"])
    if (typeof c.name !== "string" || c.name === "") fault(path, `${what} has no name`)
    else if (names.includes(c.name)) fault(path, `${what}: the name "${c.name}" is taken`)
    else names.push(c.name)
    if (typeof c.type !== "string" || c.type === "") fault(path, `${what} has no type`)
    if ("inferred" in c && c.inferred !== true) fault(path, `${what}: "inferred" is only ever true`)
    if (c.inferred === true && typeof c.type === "string" && !c.type.endsWith("?"))
      fault(path, `${what} is inferred, so optional, but its type is "${c.type}"`)
    if ("computed" in c) {
      if (typeof c.computed !== "string" || c.computed.trim() !== c.computed || c.computed === "")
        fault(path, `${what}: "computed" is not an expression`)
      if (c.type !== "num?") fault(path, `${what} is computed, so num?, not "${c.type}"`)
      if ("inferred" in c) fault(path, `${what} is computed and inferred`)
      computedSeen = true
    } else {
      if (computedSeen) fault(path, `${what} is stored, after a computed column`)
      if (typeof c.name === "string") stored.push(c.name)
    }
  })
  const rows = Array.isArray(j.rows) ? j.rows : []
  if (!Array.isArray(j.rows)) fault(path, `"rows" is not a list`)
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
      if (isObject(column) && "computed" in column && kind !== "num")
        fault(path, `row ${n}, ${name}: a computed value is num, not ${kind}`)
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
      return fault(path, `${what} names no stored column`)
    if (typeof p.kind !== "string" || !KINDS.includes(p.kind)) return fault(path, `${what} has no known kind`)
    const at: [number, number] = [p.row, stored.indexOf(p.column)]
    if (at[0] < last[0] || (at[0] === last[0] && at[1] <= last[1]))
      fault(path, `${what} is out of order, or a second problem on one cell`)
    last = at
    const v = (rows[p.row] as { [k: string]: Json })[p.column]
    if (p.kind === "key" && v === null) fault(path, `${what} is a duplicate key on a null value`)
    if (p.kind !== "key" && v !== null) fault(path, `${what} is ${p.kind}, but the value is not null`)
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
  j.forEach((e, i) => {
    const what = `edit ${i + 1}`
    if (!isObject(e)) return fault(path, `${what} is not an object`)
    if (typeof e.column !== "string" || e.column === "") fault(path, `${what} names no column`)
    if ("type" in e) {
      keysAre(path, what, e, ["column", "type"])
      if (typeof e.type !== "string") fault(path, `${what}: "type" is not a spelling`)
    } else if ("text" in e) {
      keysAre(path, what, e, ["row", "column", "text"])
      if (typeof e.text !== "string") fault(path, `${what}: "text" is not text`)
    } else if ("value" in e) {
      keysAre(path, what, e, ["row", "column", "value"])
      value(path, what, e.value)
    } else fault(path, `${what} sets no text, value or type`)
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
    if (!/^[a-z0-9]+(-[a-z0-9]+)*(\.edit\.json|\.out\.tcsv|\.tcsv|\.json)$/.test(file)) {
      fault(path, "is not named as a case (lowercase words joined by -)")
      continue
    }
    const stem = file.replace(/(\.edit\.json|\.out\.tcsv|\.tcsv|\.json)$/, "")
    if (folder === "write") {
      if (file.endsWith(".json") && !file.endsWith(".edit.json")) fault(path, "a write case has no .json")
      else if (!has.has(`${stem}.tcsv`)) fault(path, `has no ${stem}.tcsv`)
      if (file === `${stem}.tcsv`) {
        cases++
        bytes(path)
        if (!has.has(`${stem}.edit.json`)) fault(path, `has no ${stem}.edit.json`)
        if (!has.has(`${stem}.out.tcsv`)) fault(path, `has no ${stem}.out.tcsv`)
        if (stem.startsWith("error-")) fault(path, "a write case reads")
      }
      if (file.endsWith(".out.tcsv")) bytes(path)
      if (file.endsWith(".edit.json")) {
        const j = json(path)
        if (j !== undefined) edits(path, j)
      }
      continue
    }
    if (file.endsWith(".edit.json") || file.endsWith(".out.tcsv")) {
      fault(path, "edits belong in write/")
      continue
    }
    if (file.endsWith(".tcsv")) {
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
    if (!has.has(`${stem}.tcsv`)) fault(path, `has no ${stem}.tcsv`)
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
    else readsCase(path, j)
  }
}

if (faults.length > 0) {
  for (const f of faults) console.error(`✗ ${f}`)
  console.error(`${faults.length} faults in ${cases} cases`)
  process.exit(1)
}
console.log(`${cases} cases, well-formed`)
