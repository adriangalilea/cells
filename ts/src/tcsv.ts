// TCSV, the reference implementation (SPEC.md beside this folder is the law).
//
//   const { doc, rows, columns, problems } = read(text)
//   const changed = setCell(doc, 0, "cost", "90.00")
//   write(changed)                      // the same bytes, but for that one cell
//
// No dependencies. `read` never throws on the data: a cell that breaks its type is a
// problem, its text kept and its value null. It throws on the file's grammar (an
// unknown type word, a row with more fields than the header, a malformed prelude),
// with a `TcsvError` naming where.

// ── Types ───────────────────────────────────────────────────────────────────────

export type Base =
  | "text"
  | "int"
  | "num"
  | "pct"
  | "bool"
  | "date"
  | "time"
  | "datetime"
  | "url"
  | "email"
  | "money"
  | "choice"

export interface Bound {
  value: number
  inclusive: boolean
}

/** A column's type, as read from its header cell (or inferred from its values). */
export interface ColumnType {
  base: Base
  /** `money`'s ISO 4217 code. */
  currency?: string
  /** `choice`'s words, in the order written. */
  choices?: string[]
  /** `bool`'s words for true and false, as the file writes them. */
  words?: [string, string]
  min?: Bound
  max?: Bound
  /** How the file writes a date or a time (`DD/MM/YYYY`). */
  format?: string
  key: boolean
  optional: boolean
  /** The named type the cell used (`Category`), when it used one. */
  named?: string
  /** Read from the values, the header said nothing. */
  inferred: boolean
}

export interface Column {
  name: string
  /** The type as the header cell spells it (after its `:`), "" when untyped. */
  spelled: string
  type: ColumnType
  /** A computed column's expression, from the prelude; its cells are never stored. */
  computed?: string
}

export type Value = string | number | boolean | null

export interface Problem {
  /** The data row, from 0. */
  row: number
  /** The column's name, and its place among the stored columns (names may repeat). */
  column: string
  at: number
  text: string
  why: string
}

/** A field as read: its text and how the file wrote it (quotes included). */
interface Field {
  text: string
  raw: string
}

/** Everything needed to write the file back as it was. */
export interface Doc {
  newline: "\n" | "\r\n"
  trailing: boolean
  prelude: string[]
  header: Field[]
  rows: Field[][]
  columns: Column[]
  named: Record<string, ColumnType>
  computed: { name: string; expression: string }[]
}

export class TcsvError extends Error {
  constructor(
    readonly where: string,
    why: string,
  ) {
    super(`tcsv: ${where}: ${why}`)
  }
}

/** ISO 4217's active codes: any of them is a type, an amount in that currency. */
export const CURRENCIES = new Set(
  (
    "AED AFN ALL AMD AOA ARS AUD AWG AZN BAM BBD BDT BGN BHD BIF BMD BND BOB BRL BSD " +
    "BTN BWP BYN BZD CAD CDF CHF CLP CNY COP CRC CUP CVE CZK DJF DKK DOP DZD EGP ERN " +
    "ETB EUR FJD FKP GBP GEL GHS GIP GMD GNF GTQ GYD HKD HNL HTG HUF IDR ILS INR IQD " +
    "IRR ISK JMD JOD JPY KES KGS KHR KMF KPW KRW KWD KYD KZT LAK LBP LKR LRD LSL LYD " +
    "MAD MDL MGA MKD MMK MNT MOP MRU MUR MVR MWK MXN MYR MZN NAD NGN NIO NOK NPR NZD " +
    "OMR PAB PEN PGK PHP PKR PLN PYG QAR RON RSD RUB RWF SAR SBD SCR SDG SEK SGD SHP " +
    "SLE SOS SRD SSP STN SVC SYP SZL THB TJS TMT TND TOP TRY TTD TWD TZS UAH UGX USD " +
    "UYU UZS VES VND VUV WST XAF XCD XCG XOF XPF YER ZAR ZMW ZWG"
  ).split(" "),
)

const BASES = new Set<string>([
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
])

/** The words a type's explicit form is made of; never a choice's bare word. */
const KEYWORDS = new Set([
  "key",
  "optional",
  "one",
  "of",
  "min",
  "max",
  "between",
  "and",
])

// ── The type grammar ────────────────────────────────────────────────────────────

type Token =
  | { k: "word"; v: string }
  | { k: "quoted"; v: string }
  | { k: "op"; v: "|" | "/" | "!" | "?" | ">=" | ">" | "<=" | "<" | ".." }
  | { k: "format"; v: string }

const OPS = [">=", "<=", "..", "|", "/", "!", "?", ">", "<"] as const
const RESERVED = new Set(["|", "/", "!", "?", "<", ">", "=", "(", ")", "'", " "])

function tokens(spelled: string, where: string): Token[] {
  const out: Token[] = []
  let i = 0
  while (i < spelled.length) {
    const c = spelled[i] as string
    if (c === " ") {
      i++
      continue
    }
    if (c === "'") {
      let v = ""
      i++
      for (;;) {
        if (i >= spelled.length) throw new TcsvError(where, "a quote is not closed")
        if (spelled[i] === "'") {
          if (spelled[i + 1] === "'") {
            v += "'"
            i += 2
            continue
          }
          i++
          break
        }
        v += spelled[i]
        i++
      }
      out.push({ k: "quoted", v })
      continue
    }
    if (c === "(") {
      const end = spelled.indexOf(")", i)
      if (end < 0) throw new TcsvError(where, "a format's ( is not closed")
      out.push({ k: "format", v: spelled.slice(i + 1, end) })
      i = end + 1
      continue
    }
    // An operator, `..` included; a single `.` inside a number (0.5) is a word's.
    const op = OPS.find((o) => spelled.startsWith(o, i))
    if (op) {
      out.push({ k: "op", v: op })
      i += op.length
      continue
    }
    let v = ""
    while (i < spelled.length) {
      const d = spelled[i] as string
      if (RESERVED.has(d) || spelled.startsWith("..", i)) break
      v += d
      i++
    }
    if (v === "") throw new TcsvError(where, `"${c}" cannot stand here`)
    out.push({ k: "word", v })
  }
  return out
}

const num = (t: Token | undefined, where: string): number => {
  const n = t && t.k === "word" ? Number(t.v) : Number.NaN
  if (!Number.isFinite(n))
    throw new TcsvError(where, `a number was expected, not ${t ? `"${t.v}"` : "the end"}`)
  return n
}

/** A header cell's type, from what follows its `:`. `named` are the prelude's types. */
export function parseType(
  spelled: string,
  where: string,
  named: Record<string, ColumnType> = {},
): ColumnType {
  const t = tokens(spelled, where)
  let type: ColumnType | null = null
  let key = false
  let optional = false
  let min: Bound | undefined
  let max: Bound | undefined
  let format: string | undefined
  const set = (next: ColumnType) => {
    if (type) throw new TcsvError(where, "two types in one cell")
    type = next
  }
  const plain = (base: Base): ColumnType => ({
    base,
    key: false,
    optional: false,
    inferred: false,
  })
  let i = 0
  const word = (j: number) => {
    const x = t[j]
    return x && (x.k === "word" || x.k === "quoted") ? x : null
  }
  while (i < t.length) {
    const x = t[i] as Token
    const next = t[i + 1]
    if (x.k === "op" && x.v === "!") {
      key = true
      i++
    } else if (x.k === "op" && x.v === "?") {
      optional = true
      i++
    } else if (x.k === "op" && (x.v === ">=" || x.v === ">" || x.v === "<=" || x.v === "<")) {
      const value = num(next, where)
      if (x.v === ">=" || x.v === ">") min = { value, inclusive: x.v === ">=" }
      else max = { value, inclusive: x.v === "<=" }
      i += 2
    } else if (x.k === "format") {
      format = x.v
      i++
    } else if (x.k === "word" && x.v === "key") {
      key = true
      i++
    } else if (x.k === "word" && x.v === "optional") {
      optional = true
      i++
    } else if (x.k === "word" && x.v === "min") {
      min = { value: num(next, where), inclusive: true }
      i += 2
    } else if (x.k === "word" && x.v === "max") {
      max = { value: num(next, where), inclusive: true }
      i += 2
    } else if (x.k === "word" && x.v === "between") {
      const and = t[i + 2]
      if (and?.k !== "word" || and.v !== "and")
        throw new TcsvError(where, "between takes two numbers and and: between 0 and 100")
      min = { value: num(next, where), inclusive: true }
      max = { value: num(t[i + 3], where), inclusive: true }
      i += 4
    } else if (x.k === "word" && x.v === "one" && next?.k === "word" && next.v === "of") {
      const choices: string[] = []
      i += 2
      for (let w = word(i); w && !(w.k === "word" && KEYWORDS.has(w.v)); w = word(i)) {
        choices.push(w.v)
        i++
      }
      if (choices.length === 0) throw new TcsvError(where, "one of names no choice")
      set({ ...plain("choice"), choices })
    } else if (x.k === "word" && x.v === "bool" && word(i + 1) && word(i + 2) &&
      !KEYWORDS.has((t[i + 1] as { v: string }).v)) {
      set({
        ...plain("bool"),
        words: [(t[i + 1] as { v: string }).v, (t[i + 2] as { v: string }).v],
      })
      i += 3
    } else if ((x.k === "word" || x.k === "quoted") && next?.k === "op" && next.v === "|") {
      const choices = [x.v]
      i++
      while (t[i]?.k === "op" && (t[i] as { v: string }).v === "|") {
        const w = word(i + 1)
        if (!w) throw new TcsvError(where, "a | is followed by no choice")
        choices.push(w.v)
        i += 2
      }
      set({ ...plain("choice"), choices })
    } else if ((x.k === "word" || x.k === "quoted") && next?.k === "op" && next.v === "/") {
      const no = word(i + 2)
      if (!no) throw new TcsvError(where, "a / is followed by no word for false")
      set({ ...plain("bool"), words: [x.v, no.v] })
      i += 3
    } else if (x.k === "word" && next?.k === "op" && next.v === "..") {
      min = { value: num(x, where), inclusive: true }
      max = { value: num(t[i + 2], where), inclusive: true }
      i += 3
    } else if (x.k === "quoted") {
      set({ ...plain("choice"), choices: [x.v] })
      i++
    } else if (x.k === "word" && BASES.has(x.v)) {
      set(plain(x.v as Base))
      i++
    } else if (x.k === "word" && CURRENCIES.has(x.v)) {
      set({ ...plain("money"), currency: x.v })
      i++
    } else if (x.k === "word" && named[x.v]) {
      set({ ...(named[x.v] as ColumnType), named: x.v })
      i++
    } else if (x.k === "word") {
      throw new TcsvError(where, `no type or currency "${x.v}"`)
    } else {
      throw new TcsvError(where, `"${x.v}" cannot stand here`)
    }
  }
  const resolved: ColumnType =
    type ?? (min || max ? plain("num") : (() => {
      throw new TcsvError(where, "no type")
    })())
  if ((min || max) && (resolved.base === "choice" || resolved.base === "bool"))
    throw new TcsvError(where, "a choice or a boolean takes no bounds")
  if (format && !["date", "time", "datetime"].includes(resolved.base))
    throw new TcsvError(where, "only a date or a time takes a format")
  return {
    ...resolved,
    key: key || resolved.key,
    optional: optional || resolved.optional,
    ...(min ? { min } : resolved.min ? { min: resolved.min } : {}),
    ...(max ? { max } : resolved.max ? { max: resolved.max } : {}),
    ...(format ? { format } : resolved.format ? { format: resolved.format } : {}),
  }
}

/** A header cell: its name before the first `:`, its type after it. */
export function parseHeader(
  cell: string,
  named: Record<string, ColumnType> = {},
): { name: string; spelled: string; type: ColumnType | null } {
  const at = cell.indexOf(":")
  if (at < 0) return { name: cell, spelled: "", type: null }
  const name = cell.slice(0, at)
  const spelled = cell.slice(at + 1)
  return { name, spelled, type: parseType(spelled, `column "${name}"`, named) }
}

// ── The file ────────────────────────────────────────────────────────────────────

/** RFC 4180 records, each field with its text and its raw spelling. */
function records(body: string, where: (n: number) => string): Field[][] {
  const out: Field[][] = []
  let record: Field[] = []
  let i = 0
  const n = body.length
  if (n === 0) return out
  for (;;) {
    let text = ""
    const start = i
    if (body[i] === '"') {
      i++
      for (;;) {
        if (i >= n) throw new TcsvError(where(out.length), "a quoted field is not closed")
        if (body[i] === '"') {
          if (body[i + 1] === '"') {
            text += '"'
            i += 2
            continue
          }
          i++
          break
        }
        text += body[i]
        i++
      }
    } else {
      while (i < n && body[i] !== "," && body[i] !== "\n" && body[i] !== "\r") {
        text += body[i]
        i++
      }
    }
    record.push({ text, raw: body.slice(start, i) })
    if (i >= n) {
      out.push(record)
      return out
    }
    if (body[i] === ",") {
      i++
      continue
    }
    // a line break ends the record
    i += body[i] === "\r" && body[i + 1] === "\n" ? 2 : 1
    out.push(record)
    record = []
    if (i >= n) return out
  }
}

/** Quoted only when CSV needs it. */
const rawOf = (text: string): string =>
  /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text

// ── Values ──────────────────────────────────────────────────────────────────────

const INT = /^[+-]?\d+$/
const NUM = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/
const ISO_DATETIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2}(\.\d+)?)?(Z|[+-]\d{2}:?\d{2})?$/
const EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/

/** A date or time read by a format (`DD/MM/YYYY`), as ISO, or null. */
function byFormat(text: string, format: string): string | null {
  const parts: Record<string, string> = {}
  let pattern = "^"
  const names: string[] = []
  for (let i = 0; i < format.length; ) {
    const tok = ["YYYY", "MM", "DD", "HH", "mm", "ss"].find((t) => format.startsWith(t, i))
    if (tok) {
      pattern += tok === "YYYY" ? "(\\d{4})" : "(\\d{2})"
      names.push(tok)
      i += tok.length
    } else {
      pattern += (format[i] as string).replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
      i++
    }
  }
  const m = new RegExp(`${pattern}$`).exec(text)
  if (!m) return null
  names.forEach((name, j) => {
    parts[name] = m[j + 1] as string
  })
  const date =
    parts.YYYY && parts.MM && parts.DD ? `${parts.YYYY}-${parts.MM}-${parts.DD}` : null
  const time = parts.HH && parts.mm ? `${parts.HH}:${parts.mm}:${parts.ss ?? "00"}` : null
  if (date) {
    const d = new Date(`${date}T00:00:00Z`)
    if (Number.isNaN(d.getTime()) || d.toISOString().slice(0, 10) !== date) return null
  }
  if (time && (Number(parts.HH) > 23 || Number(parts.mm) > 59 || Number(parts.ss ?? 0) > 59))
    return null
  return date && time ? `${date}T${time}` : (date ?? time)
}

/** A cell's value by its type, or why it cannot be. */
function decode(text: string, type: ColumnType): { value: Value } | { why: string } {
  if (text === "") return type.optional ? { value: null } : { why: "is missing" }
  const bounded = (n: number, what: string): { value: Value } | { why: string } => {
    if (type.min && (type.min.inclusive ? n < type.min.value : n <= type.min.value))
      return { why: `${what} is below ${type.min.inclusive ? "" : "or at "}${type.min.value}` }
    if (type.max && (type.max.inclusive ? n > type.max.value : n >= type.max.value))
      return { why: `${what} is above ${type.max.inclusive ? "" : "or at "}${type.max.value}` }
    return { value: n }
  }
  switch (type.base) {
    case "text": {
      const length = bounded([...text].length, "its length")
      return "why" in length ? length : { value: text }
    }
    case "int":
      return INT.test(text) ? bounded(Number(text), "it") : { why: "is not a whole number" }
    case "num":
    case "money":
      return NUM.test(text) ? bounded(Number(text), "it") : { why: "is not a number" }
    case "pct": {
      const bare = text.endsWith("%") ? text.slice(0, -1) : text
      return NUM.test(bare) ? bounded(Number(bare), "it") : { why: "is not a percentage" }
    }
    case "bool": {
      const [yes, no] = type.words ?? ["true", "false"]
      if (text === yes) return { value: true }
      if (text === no) return { value: false }
      return { why: `is not ${yes} or ${no}` }
    }
    case "choice":
      return type.choices?.includes(text)
        ? { value: text }
        : { why: `is not one of ${type.choices?.join(", ")}` }
    case "date": {
      const iso = byFormat(text, type.format ?? "YYYY-MM-DD")
      return iso ? { value: iso } : { why: `is not a date${type.format ? ` (${type.format})` : ""}` }
    }
    case "time": {
      const iso = byFormat(text, type.format ?? (text.length > 5 ? "HH:mm:ss" : "HH:mm"))
      return iso ? { value: iso } : { why: "is not a time" }
    }
    case "datetime": {
      if (type.format) {
        const iso = byFormat(text, type.format)
        return iso ? { value: iso } : { why: `is not a date and time (${type.format})` }
      }
      return ISO_DATETIME.test(text) && !Number.isNaN(Date.parse(text))
        ? { value: text }
        : { why: "is not a date and time" }
    }
    case "url":
      try {
        const u = new URL(text)
        return u.protocol ? { value: text } : { why: "is not a URL" }
      } catch {
        return { why: "is not a URL" }
      }
    case "email":
      return EMAIL.test(text) ? { value: text } : { why: "is not an email address" }
  }
}

/** A value in its column's form, the text the file writes. */
export function encode(type: ColumnType, value: Value): string {
  if (value === null) return ""
  if (type.base === "bool") {
    const [yes, no] = type.words ?? ["true", "false"]
    return value ? yes : no
  }
  if ((type.base === "date" || type.base === "time" || type.base === "datetime") && type.format) {
    const v = String(value)
    const m = /^(?:(\d{4})-(\d{2})-(\d{2}))?T?(?:(\d{2}):(\d{2})(?::(\d{2}))?)?/.exec(v)
    const parts: Record<string, string> = {
      YYYY: m?.[1] ?? "",
      MM: m?.[2] ?? "",
      DD: m?.[3] ?? "",
      HH: m?.[4] ?? "",
      mm: m?.[5] ?? "",
      ss: m?.[6] ?? "00",
    }
    return type.format.replace(/YYYY|MM|DD|HH|mm|ss/g, (t) => parts[t] as string)
  }
  return String(value)
}

/** A column's type read from its values: the plainest kind every value fits. */
export function infer(texts: readonly string[]): ColumnType {
  const seen = texts.filter((t) => t !== "")
  const base = (b: Base, extra: Partial<ColumnType> = {}): ColumnType => ({
    base: b,
    key: false,
    optional: true,
    inferred: true,
    ...extra,
  })
  if (seen.length === 0) return base("text")
  if (seen.every((t) => INT.test(t))) return base("int")
  if (seen.every((t) => NUM.test(t))) return base("num")
  for (const words of [
    ["true", "false"],
    ["yes", "no"],
  ] as const)
    if (seen.every((t) => t === words[0] || t === words[1]))
      return base("bool", { words: [words[0], words[1]] })
  if (seen.every((t) => byFormat(t, "YYYY-MM-DD") !== null)) return base("date")
  if (seen.every((t) => ISO_DATETIME.test(t))) return base("datetime")
  return base("text")
}

// ── Computed columns ────────────────────────────────────────────────────────────

type Expr =
  | { k: "num"; v: number }
  | { k: "col"; v: string }
  | { k: "op"; op: "+" | "-" | "*" | "/"; a: Expr; b: Expr }
  | { k: "neg"; a: Expr }

/** `cost * qty`: numbers, numeric columns' names, + - * /, parentheses. */
function parseExpression(source: string, where: string): Expr {
  const toks = source.match(/\d+(\.\d+)?|[A-Za-z_][A-Za-z0-9_]*|[-+*/()]|\S/g) ?? []
  let i = 0
  const fail = (why: string): never => {
    throw new TcsvError(where, why)
  }
  const atom = (): Expr => {
    const t = toks[i++]
    if (t === undefined) return fail("the expression ends early")
    if (t === "(") {
      const e = sum()
      if (toks[i++] !== ")") fail("a ( is not closed")
      return e
    }
    if (t === "-") return { k: "neg", a: atom() }
    if (/^\d/.test(t)) return { k: "num", v: Number(t) }
    if (/^[A-Za-z_]/.test(t)) return { k: "col", v: t }
    return fail(`"${t}" cannot stand in an expression`)
  }
  const product = (): Expr => {
    let a = atom()
    while (toks[i] === "*" || toks[i] === "/") {
      const op = toks[i++] as "*" | "/"
      a = { k: "op", op, a, b: atom() }
    }
    return a
  }
  const sum = (): Expr => {
    let a = product()
    while (toks[i] === "+" || toks[i] === "-") {
      const op = toks[i++] as "+" | "-"
      a = { k: "op", op, a, b: product() }
    }
    return a
  }
  const e = sum()
  if (i < toks.length) fail(`"${toks[i]}" is left over`)
  return e
}

function columnsOf(e: Expr): string[] {
  if (e.k === "col") return [e.v]
  if (e.k === "op") return [...columnsOf(e.a), ...columnsOf(e.b)]
  if (e.k === "neg") return columnsOf(e.a)
  return []
}

function evaluate(e: Expr, row: Record<string, Value>): number | null {
  switch (e.k) {
    case "num":
      return e.v
    case "col": {
      const v = row[e.v]
      return typeof v === "number" ? v : null
    }
    case "neg": {
      const a = evaluate(e.a, row)
      return a === null ? null : -a
    }
    case "op": {
      const a = evaluate(e.a, row)
      const b = evaluate(e.b, row)
      if (a === null || b === null) return null
      if (e.op === "/" && b === 0) return null
      return e.op === "+" ? a + b : e.op === "-" ? a - b : e.op === "*" ? a * b : a / b
    }
  }
}

// ── Reading and writing ─────────────────────────────────────────────────────────

const stripComment = (line: string): string => {
  let quoted = false
  for (let i = 0; i < line.length; i++) {
    if (line[i] === "'") quoted = !quoted
    if (!quoted && line.startsWith("//", i)) return line.slice(0, i)
  }
  return line
}

export interface Read {
  doc: Doc
  columns: Column[]
  /** Each row's values by column name, computed columns included. */
  rows: Record<string, Value>[]
  problems: Problem[]
}

/** A `.tcsv` file's text, read. */
export function read(text: string): Read {
  const newline = text.includes("\r\n") ? "\r\n" : "\n"
  const lines = text.split(newline)
  const prelude: string[] = []
  while (lines.length > 0 && (lines[0] as string).startsWith("#!"))
    prelude.push(lines.shift() as string)
  const body = lines.join(newline)
  const trailing = body.endsWith(newline)
  const all = records(trailing ? body.slice(0, -newline.length) : body, (n) =>
    n === 0 ? "the header" : `row ${n - 1}`,
  )
  const header = all[0] ?? []
  const rows = all.slice(1)
  const named: Record<string, ColumnType> = {}
  const computed: { name: string; expression: string }[] = []
  prelude.forEach((line, n) => {
    const where = `prelude line ${n + 1}`
    const said = stripComment(line.slice(2)).trim()
    if (said === "") return
    const t = /^type\s+([A-Z][A-Za-z0-9_]*)\s*=\s*(.+)$/.exec(said)
    if (t) {
      named[t[1] as string] = parseType(t[2] as string, where, named)
      return
    }
    const c = /^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$/.exec(said)
    if (c) {
      computed.push({ name: c[1] as string, expression: (c[2] as string).trim() })
      return
    }
    throw new TcsvError(where, "is neither `type Name = …` nor `name = expression`")
  })
  rows.forEach((r, n) => {
    if (r.length > header.length)
      throw new TcsvError(`row ${n}`, `${r.length} fields, the header has ${header.length}`)
  })
  const columns: Column[] = header.map((f, j) => {
    const { name, spelled, type } = parseHeader(f.text, named)
    return {
      name,
      spelled,
      type: type ?? infer(rows.map((r) => r[j]?.text ?? "")),
    }
  })
  for (const c of computed) {
    const expr = parseExpression(c.expression, `computed column "${c.name}"`)
    for (const used of columnsOf(expr))
      if (!columns.some((col) => col.name === used))
        throw new TcsvError(`computed column "${c.name}"`, `no column "${used}"`)
    columns.push({
      name: c.name,
      spelled: "",
      computed: c.expression,
      type: { base: "num", key: false, optional: true, inferred: false },
    })
  }
  const doc: Doc = { newline, trailing, prelude, header, rows, columns, named, computed }
  return { doc, ...valuesOf(doc) }
}

/** The rows' values and the problems, from a doc as it stands. */
export function valuesOf(doc: Doc): { columns: Column[]; rows: Record<string, Value>[]; problems: Problem[] } {
  const problems: Problem[] = []
  const stored = doc.columns.filter((c) => !c.computed)
  const keys = new Map<string, Map<string, number>>()
  const rows = doc.rows.map((r, n) => {
    const row: Record<string, Value> = {}
    stored.forEach((c, j) => {
      const text = r[j]?.text ?? ""
      const got = decode(text, c.type)
      if ("why" in got) {
        problems.push({ row: n, column: c.name, at: j, text, why: `"${text}" ${got.why}` })
        row[c.name] = null
      } else row[c.name] = got.value
      if (c.type.key && text !== "") {
        const seen = keys.get(c.name) ?? new Map<string, number>()
        keys.set(c.name, seen)
        const first = seen.get(text)
        if (first !== undefined)
          problems.push({
            row: n,
            column: c.name,
            at: j,
            text,
            why: `"${text}" is the key of row ${first} too`,
          })
        else seen.set(text, n)
      }
    })
    for (const c of doc.computed)
      row[c.name] = evaluate(parseExpression(c.expression, c.name), row)
    return row
  })
  return { columns: doc.columns, rows, problems }
}

/** The doc with one cell's text changed; every other byte stays as read. */
export function setCell(doc: Doc, row: number, column: string, text: string): Doc {
  const j = doc.columns.findIndex((c) => c.name === column && !c.computed)
  if (j < 0) throw new TcsvError(`column "${column}"`, "is not a stored column")
  if (row < 0 || row >= doc.rows.length) throw new TcsvError(`row ${row}`, "does not exist")
  const rows = doc.rows.map((r, n) => {
    if (n !== row) return r
    const next = [...r]
    while (next.length <= j) next.push({ text: "", raw: "" })
    next[j] = { text, raw: rawOf(text) }
    return next
  })
  return { ...doc, rows }
}

/** The doc with one column's type rewritten (in shorthand, as a writer changing a type
 *  does); its values are read again by it. */
export function setType(doc: Doc, column: string, spelled: string): Doc {
  const j = doc.columns.findIndex((c) => c.name === column && !c.computed)
  if (j < 0) throw new TcsvError(`column "${column}"`, "is not a stored column")
  const cell = spelled === "" ? column : `${column}:${spelled}`
  const type =
    spelled === ""
      ? infer(doc.rows.map((r) => r[j]?.text ?? ""))
      : parseType(spelled, `column "${column}"`, doc.named)
  return {
    ...doc,
    header: doc.header.map((f, k) => (k === j ? { text: cell, raw: rawOf(cell) } : f)),
    columns: doc.columns.map((c, k) => (k === j ? { ...c, spelled, type } : c)),
  }
}

/** A file written from cells, for a host that keeps its own rows (a table editing a
 *  document) and hands back the header line's cells and the rows' texts: the prelude,
 *  the line ending and the trailing break are `like`'s, every field quoted only where
 *  CSV needs it. */
export function compose(
  like: Pick<Doc, "prelude" | "newline" | "trailing">,
  header: readonly string[],
  rows: readonly (readonly string[])[],
): string {
  const line = (cells: readonly string[]) => cells.map(rawOf).join(",")
  const body = [line(header), ...rows.map(line)].join(like.newline)
  return [...like.prelude, body].join(like.newline) + (like.trailing ? like.newline : "")
}

/** The file's text: what was read, but for what was changed. */
export function write(doc: Doc): string {
  const line = (fields: Field[]) => fields.map((f) => f.raw).join(",")
  const body = [line(doc.header), ...doc.rows.map(line)].join(doc.newline)
  return [...doc.prelude, body].join(doc.newline) + (doc.trailing ? doc.newline : "")
}

// ── The row's type, from the header, at compile time ────────────────────────────

type Trim<S extends string> = S extends ` ${infer R}`
  ? Trim<R>
  : S extends `${infer R} `
    ? Trim<R>
    : S
type Split<S extends string, D extends string> = S extends `${infer A}${D}${infer B}`
  ? [A, ...Split<B, D>]
  : [S]
type Unquote<S extends string> = S extends `'${infer V}'` ? V : S
type Currency =
  | "AED" | "AUD" | "BRL" | "CAD" | "CHF" | "CNY" | "CZK" | "DKK" | "EUR" | "GBP"
  | "HKD" | "HUF" | "ILS" | "INR" | "JPY" | "KRW" | "MXN" | "NOK" | "NZD" | "PLN"
  | "RON" | "SEK" | "SGD" | "THB" | "TRY" | "TWD" | "USD" | "ZAR"

/** A type's value, from its spelling: the shorthand, and the explicit words that mark
 *  (`key`, `optional`, `one of`, `bool a b`). */
type ValueOf<T extends string> = Trim<T> extends `${infer B}?`
  ? ValueOf<B> | null
  : Trim<T> extends `${infer B} optional`
    ? ValueOf<B> | null
    : Trim<T> extends `${infer B}!`
      ? ValueOf<B>
      : Trim<T> extends `${infer B} key`
        ? ValueOf<B>
        : Trim<T> extends `${infer B}(${string})`
          ? ValueOf<B>
          : Trim<T> extends `one of ${infer R}`
            ? Unquote<Split<Trim<R>, " ">[number]>
            : Trim<T> extends `bool ${string}`
              ? boolean
              : Trim<T> extends `${string}|${string}`
                ? Unquote<Trim<Split<Trim<T>, "|">[number]>>
                : Trim<T> extends `${string}/${string}`
                  ? boolean
                  : Trim<T> extends `${infer B}>=${string}` | `${infer B}<=${string}` | `${infer B}>${string}` | `${infer B}<${string}`
                    ? B extends "" ? number : ValueOf<B>
                    : Trim<T> extends `${infer B} min ${string}` | `${infer B} max ${string}` | `${infer B} between ${string}`
                      ? ValueOf<B>
                      : Trim<T> extends `${string}..${string}`
                        ? number
                        : Trim<T> extends "text" | "url" | "email" | "date" | "time" | "datetime"
                          ? string
                          : Trim<T> extends "int" | "num" | "pct" | Currency
                            ? number
                            : Trim<T> extends "bool"
                              ? boolean
                              : unknown

type CellEntry<C extends string> = C extends `${infer N}:${infer T}`
  ? [N, ValueOf<T>]
  : [C, string | number | boolean | null]

type Entries<Cells extends string[]> = { [K in keyof Cells]: CellEntry<Cells[K] & string> }

/** The row type a header line declares: `Infer<"item:text!,cost:EUR>=0,paid:yes/no">`
 *  is `{ item: string; cost: number; paid: boolean }`. An untyped cell is whatever
 *  inference makes of it, a named type (from the prelude) and a currency outside the
 *  common list are `unknown`. */
export type Infer<Header extends string> = {
  [E in Entries<Split<Header, ",">>[number] as E[0]]: E[1]
}
