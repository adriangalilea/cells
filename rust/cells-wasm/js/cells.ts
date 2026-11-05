// Cells in a page or under bun: the Rust core compiled to WebAssembly, instantiated
// synchronously from the bytes `build.ts` embeds, so `read` needs no `await init()`.
//
//   const { doc, columns, rows, problems } = read(text)
//   setCell(doc, 0, "cost", "90.00")
//   write(doc)                        // the same bytes, but for that one cell
//
// A column's values stay in the module's memory as typed arrays; `rows` builds objects only
// when it is read, and `column.value(i)` reads one cell. A broken cell is a problem, its
// value null; a broken grammar takes out only what it breaks, each break in `broken` (a
// header cell that does not read is a column of text, a prelude statement left out, a
// column formula's cells blank, the rows ending at a record that does not read). Only
// bytes that are not UTF-8 throw `CellsError`. Unlike ts/src/cells.ts, an
// edit changes the doc it is given (and returns it): the file is held once, in the module.

import { wasm } from "./wasm.ts"

interface Exports {
  memory: WebAssembly.Memory
  cells_len(): number
  cells_alloc(cap: number): number
  cells_free(ptr: number, cap: number): void
  cells_read(ptr: number, len: number, cap: number): number
  cells_error(): number
  cells_drop(h: number): void
  cells_meta(h: number): number
  cells_write(h: number): number
  cells_text(h: number, row: number, column: number): number
  cells_describe(h: number, i: number): number
  cells_why(h: number, i: number): number
  cells_cells(h: number): number
  cells_cell_ends(h: number): number
  cells_set_cell(h: number, row: number, column: number, ptr: number, len: number, cap: number): number
  cells_set_type(h: number, column: number, ptr: number, len: number, cap: number): number
  cells_read_as(ptr: number, len: number, cap: number, sep: number): number
  cells_detect(ptr: number, len: number): number
  cells_reshaped(h: number, ptr: number, len: number, cap: number): number
  cells_written_as(h: number, sep: number, ptr: number, len: number, cap: number): number
  cells_naming_why(h: number, i: number): number
  cells_spans(h: number): number
  cells_read_with(ptr: number, len: number, cap: number, files: number, filesLen: number, filesCap: number): number
}

function decodeBase64(s: string): Uint8Array<ArrayBuffer> {
  const bin = atob(s)
  const out = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i)
  return out
}

const x = new WebAssembly.Instance(new WebAssembly.Module(decodeBase64(wasm)), {}).exports as unknown as Exports
// A byte order mark is the file's: kept, never swallowed.
const decoder = new TextDecoder("utf-8", { ignoreBOM: true })
const encoder = new TextEncoder()

/** The bytes the last call returned, as text. */
function returned(ptr: number): string {
  return decoder.decode(new Uint8Array(x.memory.buffer, ptr, x.cells_len()))
}

/** Bytes copied into room the module owns: [address, length, capacity]. */
function put(input: string | Uint8Array): [number, number, number] {
  const b = typeof input === "string" ? encoder.encode(input) : input
  const cap = Math.max(b.length, 1)
  const p = x.cells_alloc(cap)
  new Uint8Array(x.memory.buffer, p, b.length).set(b)
  return [p, b.length, cap]
}

export class CellsError extends Error {
  constructor(
    readonly where: string,
    readonly why: string,
  ) {
    super(`cells: ${where}: ${why}`)
  }
}

function thrown(): CellsError {
  const e = JSON.parse(returned(x.cells_error())) as { at: string; why: string }
  return new CellsError(e.at, e.why)
}

export type Value = string | number | boolean | null

interface ColumnMeta {
  name: string
  type: string
  spelled?: string
  base: string
  inferred: boolean
  key: boolean
  optional: boolean
  formula?: string
  origin?: number
  targets?: string[]
  currency?: string
  choices?: string[]
  words?: [string, string]
  format?: string
  nulls: number
  valid: number
  kind: "text" | "int" | "decimal" | "bool" | "date" | "time" | "datetime" | "choice"
  values?: number
  offsets?: number
  bytes?: number
  bytesLen?: number
  scale?: number
  unit?: number
}

/** A plain file's separator. */
export type Separator = "," | ";" | "\t" | "|"
/** How a file is read and written: Cells, or a plain file with its separator. */
export type Dialect = "cells" | Separator

interface Meta {
  rows: number
  newline: "\n" | "\r\n" | "\r"
  dialect: Dialect
  quotesAll: boolean
  names: { column: number; kind: "blank" | "twice" }[]
  broken: { at: string; why: string }[]
  prelude: string
  unread: string
  trailing: boolean
  problems: number
  problemCount: number
  named: Record<string, string>
  columns: ColumnMeta[]
}

const TWO32 = 2 ** 32
// BigInts by call, never by literal: a project vendoring this may compile for ES2017,
// Next.js's default, where `0n` does not parse.
const ZERO = BigInt(0)
const ONE = BigInt(1)
const TEN = BigInt(10)
const WORD = BigInt(32)
const pad = (n: number, w: number) => String(n).padStart(w, "0")

/** `2026-08-14` from days since 1970-01-01 (Hinnant's civil_from_days). */
export function isoDate(days: number): string {
  const z = days + 719468
  const era = Math.floor(z / 146097)
  const doe = z - era * 146097
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365)
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100))
  const mp = Math.floor((5 * doy + 2) / 153)
  const d = doy - Math.floor((153 * mp + 2) / 5) + 1
  const m = mp < 10 ? mp + 3 : mp - 9
  const y = yoe + era * 400 + (m <= 2 ? 1 : 0)
  return `${pad(y, 4)}-${pad(m, 2)}-${pad(d, 2)}`
}

/** `14:30:05` from seconds since midnight. */
export function isoTime(s: number): string {
  return `${pad(Math.floor(s / 3600), 2)}:${pad(Math.floor(s / 60) % 60, 2)}:${pad(s % 60, 2)}`
}

/** One column, read where it lies in the module's memory. */
export class Column {
  private buffer: ArrayBuffer | null = null
  private i32: Int32Array = new Int32Array()
  private i16: Int16Array = new Int16Array()
  private u8: Uint8Array = new Uint8Array()
  private valid: Uint8Array = new Uint8Array()

  constructor(
    private readonly doc: Doc,
    private readonly m: ColumnMeta,
    readonly index: number,
  ) {}

  get name(): string {
    return this.m.name
  }
  /** The type in canonical shorthand: `EUR>=0`, `text!`, `yes/no?`. */
  get type(): string {
    return this.m.type
  }
  /** What the header cell spells after its `:`, its formula included; `""` for a plain
   *  column. */
  get spelled(): string {
    return this.m.spelled ?? ""
  }
  /** What the column holds: `text`, `int`, `num`, `pct`, `money`, `bool`, `date`, ... */
  get base(): string {
    return this.m.base
  }
  get inferred(): boolean {
    return this.m.inferred
  }
  /** The column's formula, as its header cell writes it after the type's `=`. */
  get formula(): string | undefined {
    return this.m.formula
  }
  /** The tables a reference column names, `accounts.account`. */
  get targets(): string[] | undefined {
    return this.m.targets
  }
  /** Where the cell's value comes from: its text, its column's formula, or its own. */
  origin(i: number): Origin {
    if (this.m.origin === undefined) return "written"
    return ORIGINS[new Uint8Array(x.memory.buffer, this.m.origin, this.doc.rowCount)[i] as number] as Origin
  }
  get currency(): string | undefined {
    return this.m.currency
  }
  get choices(): string[] | undefined {
    return this.m.choices
  }
  get length(): number {
    return this.doc.rowCount
  }
  get nulls(): number {
    return this.m.nulls
  }
  /** How the values are stored: `int` i64, `decimal` i128 at `scale`, `date` days, ... */
  get kind(): ColumnMeta["kind"] {
    return this.m.kind
  }
  get scale(): number {
    return this.m.scale ?? 0
  }

  private views(): void {
    const b = x.memory.buffer
    if (this.buffer === b) return
    this.buffer = b
    this.i32 = new Int32Array(b)
    this.i16 = new Int16Array(b)
    this.u8 = new Uint8Array(b)
    this.valid = new Uint8Array(b, this.m.valid, Math.ceil(this.doc.rowCount / 8))
  }

  isNull(i: number): boolean {
    this.views()
    return ((this.valid[i >> 3] as number) >> (i & 7) & 1) === 0
  }

  /** A whole number or a decimal's mantissa as a JS number when exact, else a BigInt. */
  private wide(at: number, words: number): number | bigint {
    const w = this.i32
    const lo = (w[at] as number) >>> 0
    const hi = w[at + 1] as number
    const sign = hi >> 31
    let small = Math.abs(hi) < 0x200000
    for (let k = 2; k < words; k++) small &&= w[at + k] === sign
    if (small) return hi * TWO32 + lo
    let v = ZERO
    for (let k = words - 1; k >= 0; k--) v = (v << WORD) | BigInt((w[at + k] as number) >>> 0)
    return BigInt.asIntN(32 * words, v)
  }

  /** The cell's value, as ts/src/cells.ts gives it: text, a number, a boolean, an ISO date
   *  or time, or null for a blank or broken cell. */
  value(i: number): Value {
    if (this.isNull(i)) return null
    const m = this.m
    const v = m.values as number
    switch (m.kind) {
      case "text": {
        const o = (m.offsets as number) >> 2
        const a = this.i32[o + i] as number
        const b = this.i32[o + i + 1] as number
        return decoder.decode(this.u8.subarray((m.bytes as number) + a, (m.bytes as number) + b))
      }
      case "int":
        return Number(this.wide((v >> 2) + 2 * i, 2))
      case "decimal":
        return Number(this.wide((v >> 2) + 4 * i, 4)) / 10 ** this.scale
      case "bool":
        return ((this.u8[v + (i >> 3)] as number) >> (i & 7) & 1) === 1
      case "date":
        return isoDate(this.i32[(v >> 2) + i] as number)
      case "time":
        return isoTime(this.i32[(v >> 2) + i] as number)
      case "choice":
        return (m.choices as string[])[this.i16[(v >> 1) + i] as number] as string
      case "datetime": {
        const t = BigInt(this.wide((v >> 2) + 2 * i, 2))
        const per = TEN ** BigInt(m.unit as number)
        const secs = t / per - (t % per < ZERO ? ONE : ZERO)
        const frac = ((t % per) + per) % per
        const s = Number(secs)
        const days = Math.floor(s / 86400)
        let out = `${isoDate(days)}T${isoTime(s - days * 86400)}`
        if (frac !== ZERO) out += `.${frac.toString().padStart(m.unit as number, "0").replace(/0+$/, "")}`
        return `${out}Z`
      }
    }
  }

  /** A decimal cell exactly, `84.20`, at the column's scale; null when blank or broken. */
  decimal(i: number): string | null {
    if (this.m.kind !== "decimal") throw new CellsError(`column "${this.name}"`, "holds no decimals")
    if (this.isNull(i)) return null
    const m = this.wide(((this.m.values as number) >> 2) + 4 * i, 4)
    const neg = m < 0
    const digits = (neg ? -BigInt(m) : BigInt(m)).toString().padStart(this.scale + 1, "0")
    const whole = this.scale ? `${digits.slice(0, -this.scale)}.${digits.slice(-this.scale)}` : digits
    return neg ? `-${whole}` : whole
  }

  /** A value as this column writes it: its pair's words, its format, ISO otherwise, and
   *  text that would read as a formula (`` `=x` ``) with its first backtick doubled. */
  form(v: Value): string {
    if (v === null) return ""
    if (typeof v === "string" && /^`+=[\s\S]*`$/.test(v) && v.length >= 3) return `\`${v}`
    if (typeof v === "boolean") {
      const [yes, no] = this.m.words ?? ["true", "false"]
      return v ? yes : no
    }
    const f = this.m.format
    if (f && typeof v === "string" && (this.m.kind === "date" || this.m.kind === "time")) {
      const [y, mo, d] = this.m.kind === "date" ? v.split("-") : []
      const [h, mi, s = "00"] = this.m.kind === "time" ? v.split(":") : []
      const parts: Record<string, string | undefined> = { YYYY: y, MM: mo, DD: d, HH: h, mm: mi, ss: s }
      return f.replace(/YYYY|MM|DD|HH|mm|ss/g, (p) => parts[p] ?? "")
    }
    return String(v)
  }

  /** The cell's text as the file writes it, unquoted: blank where its column's formula
   *  computes it, `` `=…` `` where its own does. */
  text(i: number): string {
    return returned(x.cells_text(this.doc.handle, i, this.index))
  }
}

/** Where a cell's value comes from. */
export type Origin = "written" | "formula" | "cell"
const ORIGINS = ["written", "formula", "cell"] as const

export interface Problem {
  /** The data row, from 0. */
  row: number
  /** The column's name, and its place among the columns. */
  column: string
  at: number
  /** `ragged`: a row longer than its header (in a plain file, shorter too); `formula`: a cell's
   *  own formula that does not compute; `reference`: a key no table it names holds, or
   *  two do. */
  kind: "type" | "missing" | "key" | "ragged" | "formula" | "reference"
  /** The cell's text as written. */
  readonly text: string
  /** The problem in words: `row 3, cost: "8x.50" is not a number`. */
  readonly why: string
  /** The same without its place, for a view that shows it on the cell:
   *  `"8x.50" is not a number`. */
  readonly reason: string
  /** Where it is in the text `write` gives now: the cell's text inside its quotes, the
   *  control character it holds, the part of its own formula that does not read, an
   *  empty span where a missing cell stands. */
  readonly span: Span
}

/** Where a file's grammar breaks, the rest read. */
export interface Broken {
  /** `column 3`, `prelude line 2`, `row 4`, `the header`, `the prelude`. */
  at: string
  /** In words naming what broke: `to: accounts has no column "a" (its key is account)`. */
  why: string
  /** Where it is in the text `write` gives now, its start and length in UTF-16 units (a
   *  string's indices); null for the file as a whole. */
  readonly span: Span | null
}

/** Where something is in a file's text: its start and its length, in UTF-16 units. */
export type Span = [start: number, length: number]

/** A plain file's header cell that names its column badly: blank, or another's name too. */
export interface Naming {
  /** The column, from 0. */
  column: number
  kind: "blank" | "twice"
  /** In words without its place: `has no name`, `"a" names column 1 too`. */
  readonly reason: string
}

const KINDS = ["type", "missing", "key", "ragged", "formula", "reference"] as const
const live = new FinalizationRegistry<number>((h) => x.cells_drop(h))

/** A file read, held by the module. */
export class Doc {
  meta: Meta
  columns: Column[]

  constructor(readonly handle: number) {
    this.meta = JSON.parse(returned(x.cells_meta(handle))) as Meta
    this.columns = this.meta.columns.map((m, k) => new Column(this, m, k))
    live.register(this, handle, this)
  }

  /** Read again after an edit: an edit may move a column's buffers. */
  refresh(): void {
    this.meta = JSON.parse(returned(x.cells_meta(this.handle))) as Meta
    this.columns = this.meta.columns.map((m, k) => new Column(this, m, k))
    this.spanned = null
  }

  private spanned: Uint32Array | null = null
  /** Every problem's span, then every break's, as pairs: asked of the module once per
   *  version of the doc, when a span is first read. */
  private spans(): Uint32Array {
    if (this.spanned === null) {
      const n = 2 * (this.meta.problemCount + this.meta.broken.length)
      this.spanned = new Uint32Array(x.memory.buffer, x.cells_spans(this.handle), n).slice()
    }
    return this.spanned
  }
  /** Span `k` of `spans`. */
  span(k: number): Span | null {
    const s = this.spans()
    const start = s[2 * k] as number
    return start === 0xffffffff ? null : [start, s[2 * k + 1] as number]
  }

  get rowCount(): number {
    return this.meta.rows
  }
  get named(): Record<string, string> {
    return this.meta.named
  }
  /** The text before the header, the byte order mark and the prelude, as written. */
  get prelude(): string {
    return this.meta.prelude
  }
  /** What follows the rows read, from a record that does not read on (all of it after the
   *  prelude when the header does not), kept as written: `compose` writes it after the
   *  rows. Empty for a file that reads whole. */
  get unread(): string {
    return this.meta.unread
  }
  /** The header's line break, the one new lines take (a plain file's may be a lone `\r`). */
  get newline(): "\n" | "\r\n" | "\r" {
    return this.meta.newline
  }
  /** How the file was read: `cells`, or a plain file's separator. */
  get dialect(): Dialect {
    return this.meta.dialect
  }
  /** The text between two fields. */
  get separator(): Separator {
    return this.meta.dialect === "cells" ? "," : this.meta.dialect
  }
  /** A plain file that quotes every field: an edited cell and a new row are quoted so too. */
  get quotesAll(): boolean {
    return this.meta.quotesAll
  }
  /** A plain file's header cells that name their columns badly. */
  names(): Naming[] {
    const doc = this
    return this.meta.names.map((n, i) => ({
      ...n,
      get reason() {
        return returned(x.cells_naming_why(doc.handle, i))
      },
    }))
  }
  /** Where the grammar breaks, in the file's order; empty for a file that reads whole. */
  get broken(): Broken[] {
    const doc = this
    const after = this.meta.problemCount
    return this.meta.broken.map((b, i) => ({
      ...b,
      get span() {
        return doc.span(after + i)
      },
    }))
  }
  /** Whether the file ends in a line break. */
  get trailing(): boolean {
    return this.meta.trailing
  }

  /** Every cell's text as written, unquoted, row by row: what a host that keeps its own
   *  rows takes (and gives back to `compose`). One decode for the whole file. */
  cells(): string[][] {
    const at = x.cells_cells(this.handle)
    const all = decoder.decode(new Uint8Array(x.memory.buffer, at, x.cells_len()))
    const n = this.columns.length
    const ends = new Uint32Array(x.memory.buffer, x.cells_cell_ends(this.handle), this.rowCount * n)
    const out: string[][] = new Array(this.rowCount)
    let from = 0
    for (let r = 0; r < this.rowCount; r++) {
      const row: string[] = new Array(n)
      for (let k = 0; k < n; k++) {
        const to = ends[r * n + k] as number
        row[k] = all.slice(from, to)
        from = to
      }
      out[r] = row
    }
    return out
  }

  column(name: string): Column {
    const c = this.columns.find((c) => c.name === name)
    if (!c) throw new CellsError(`column "${name}"`, "does not exist")
    return c
  }

  problems(): Problem[] {
    const n = this.meta.problemCount
    const p = new Uint32Array(x.memory.buffer, this.meta.problems, 3 * n).slice()
    const out: Problem[] = []
    for (let k = 0; k < n; k++) {
      const row = p[3 * k] as number
      const at = p[3 * k + 1] as number
      const doc = this
      out.push({
        row,
        at,
        column: (this.columns[at] as Column).name,
        kind: KINDS[p[3 * k + 2] as number] as Problem["kind"],
        get text() {
          return (doc.columns[at] as Column).text(row)
        },
        get why() {
          return returned(x.cells_describe(doc.handle, k))
        },
        get reason() {
          return returned(x.cells_why(doc.handle, k))
        },
        get span() {
          return doc.span(k) as Span
        },
      })
    }
    return out
  }

  /** Lets the module free the file now rather than when this is collected. */
  free(): void {
    live.unregister(this)
    x.cells_drop(this.handle)
  }
}

export interface Read {
  doc: Doc
  columns: Column[]
  /** Each row's values by column name, computed cells included; built when first read. */
  readonly rows: Record<string, Value>[]
  problems: Problem[]
  broken: Broken[]
}

/** Files a page holds, for a file's imports: `at` the file's own path, `files` every other
 *  file's text by its path. An import of a file not among them reads as unreachable. */
export interface Files {
  at: string
  files: Record<string, string>
}

/** A `.cell` file's text (or its bytes), read; its imports followed through `files` when
 *  given, else read alone (what it imports unchecked). */
export function read(input: string | Uint8Array, files?: Files): Read {
  if (!files) return readAs(input, "cells")
  const [p, len, cap] = put(input)
  const list = [files.at, ...Object.entries(files.files).flat()]
  const [fp, flen, fcap] = put(new Spec().list(list).bytes())
  return opened(x.cells_read_with(p, len, cap, fp, flen, fcap))
}

const sepByte = (d: Dialect): number => (d === "cells" ? 0 : d.charCodeAt(0))

/** The separator the first lines of a plain file agree on (the one splitting each into as
 *  many fields as the first, the most fields winning, a tie to `,` then tab, `;`, `|`). */
export function detect(input: string | Uint8Array): Separator {
  const [p, len, cap] = put(input)
  const sep = String.fromCharCode(x.cells_detect(p, len)) as Separator
  x.cells_free(p, cap)
  return sep
}

/** A file read in a dialect: `cells`, or a plain file (a `.csv`, a `.tsv`, plain text) by
 *  its separator, never refused but for bytes that are not UTF-8 (`detect` finds it). */
export function readAs(input: string | Uint8Array, dialect: Dialect): Read {
  const [p, len, cap] = put(input)
  return opened(x.cells_read_as(p, len, cap, sepByte(dialect)))
}

function opened(h: number): Read {
  if (h === 0) throw thrown()
  const doc = new Doc(h)
  let rows: Record<string, Value>[] | null = null
  return {
    doc,
    columns: doc.columns,
    problems: doc.problems(),
    broken: doc.broken,
    get rows() {
      if (rows === null) {
        rows = []
        for (let r = 0; r < doc.rowCount; r++) {
          const row: Record<string, Value> = {}
          for (const c of doc.columns) row[c.name] = c.value(r)
          rows.push(row)
        }
      }
      return rows
    },
  }
}

function columnOf(doc: Doc, column: string | number): number {
  const k = typeof column === "number" ? column : doc.columns.findIndex((c) => c.name === column)
  if (k < 0 || k >= doc.columns.length) throw new CellsError(`column "${column}"`, "does not exist")
  return k
}

/** One cell's text changed; every other byte stays as read. Returns `doc`, changed; text
 *  holding a control character throws `CellsError` at the row, `doc` untouched. Text of
 *  `` `=…` `` is the cell's own formula; blank in a column with a formula, the formula's. */
export function setCell(doc: Doc, row: number, column: string | number, text: string): Doc {
  const k = columnOf(doc, column)
  if (row < 0 || row >= doc.rowCount) throw new CellsError(`row ${row}`, "does not exist")
  const [p, len, cap] = put(text)
  if (x.cells_set_cell(doc.handle, row, k, p, len, cap) !== 0) throw thrown()
  doc.refresh()
  return doc
}

/** One cell set to a value, written in its column's form (`true` as `yes` in a `yes/no`
 *  column, `2026-09-01` as `01/09/2026` in a `date(DD/MM/YYYY)` one). */
export function setValue(doc: Doc, row: number, column: string | number, value: Value): Doc {
  const k = columnOf(doc, column)
  return setCell(doc, row, k, (doc.columns[k] as Column).form(value))
}

/** One column's header cell rewritten after its `:`: its type in the shorthand a writer
 *  writes and its formula when given (`EUR=cost*qty`); `""` makes it plain. */
export function setType(doc: Doc, column: string | number, spelled: string): Doc {
  const k = columnOf(doc, column)
  const [p, len, cap] = put(spelled)
  if (x.cells_set_type(doc.handle, k, p, len, cap) !== 0) throw thrown()
  doc.refresh()
  return doc
}

/** The file's text: what was read, but for what was changed. */
export function write(doc: Doc): string {
  return returned(x.cells_write(doc.handle))
}

/** Strings for the module, one after another, each a little-endian `u32` length and its
 *  bytes; a list is its count and its strings. */
class Spec {
  private parts: Uint8Array[] = []
  byte(b: number): this {
    this.parts.push(Uint8Array.of(b))
    return this
  }
  u32(n: number): this {
    const b = new Uint8Array(4)
    new DataView(b.buffer).setUint32(0, n, true)
    this.parts.push(b)
    return this
  }
  str(s: string): this {
    const b = encoder.encode(s)
    this.u32(b.length)
    this.parts.push(b)
    return this
  }
  list(xs: readonly string[]): this {
    this.u32(xs.length)
    for (const s of xs) this.str(s)
    return this
  }
  bytes(): Uint8Array {
    const out = new Uint8Array(this.parts.reduce((n, p) => n + p.length, 0))
    let at = 0
    for (const p of this.parts) {
      out.set(p, at)
      at += p.length
    }
    return out
  }
}

/** A column of a file reshaped: a stored one by its place (renamed when `name` is given),
 *  or a new one, its header cell and a cell a row (none: blanks). */
export type ColumnAt = { kept: number; name?: string } | { header: string; cells?: readonly string[] }

/** The file with exactly `columns`, in their order, read again as a new doc (`doc` is
 *  untouched): inserting, removing, moving and renaming columns. Each kept column keeps
 *  every row's field as written; a `.cell` file whose columns no longer read (two of one
 *  name, a formula reading one taken out) throws. */
export function reshape(doc: Doc, columns: readonly ColumnAt[]): Read {
  const spec = new Spec()
  for (const c of columns) {
    if ("header" in c) spec.byte(2).str(c.header).list(c.cells ?? [])
    else if (c.name !== undefined) spec.byte(1).u32(c.kept).str(c.name)
    else spec.byte(0).u32(c.kept)
  }
  const [p, len, cap] = put(spec.bytes())
  return opened(x.cells_reshaped(doc.handle, p, len, cap))
}

/** The file written in another dialect: a plain file's header the columns' names and
 *  every computed cell its value; a `.cell` file's header `header` when given (each
 *  column's cell with its type), its prelude kept. */
export function writtenAs(doc: Doc, dialect: Dialect, header?: readonly string[]): string {
  const [p, len, cap] = put(new Spec().list(header ?? []).bytes())
  return returned(x.cells_written_as(doc.handle, sepByte(dialect), p, len, cap))
}

/** A cell's text in a file's form: quoted when it holds the separator, `"` or a line
 *  break, or always in a plain file that quotes every field (the core's `scan::quote_in`). */
function quoter(separator: Separator, all: boolean): (text: string) => string {
  return (text) => (all || text.includes(separator) || /["\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text)
}

/** A file written from cells, for a host that keeps its own rows (a table editing the
 *  file) and hands back the header's cells and the rows' texts: `like`'s byte order
 *  mark and prelude, its separator, its line break and its final break, every field quoted
 *  only where the file needs it, and what it did not read after the rows, as written (the
 *  core's `cells::compose`, here so the rows never cross into the module). Pure: nothing
 *  is read or changed. */
export function compose(
  like: {
    prelude: string
    newline: "\n" | "\r\n" | "\r"
    trailing: boolean
    separator: Separator
    quotesAll: boolean
    unread: string
  },
  header: readonly string[],
  rows: readonly (readonly string[])[],
): string {
  // A header that did not read leaves nothing but what follows the prelude.
  if (header.length === 0 && like.unread !== "") return like.prelude + like.unread
  const quote = quoter(like.separator, like.quotesAll)
  const parts: string[] = new Array(rows.length + 1)
  parts[0] = header.map(quote).join(like.separator)
  for (let r = 0; r < rows.length; r++) parts[r + 1] = (rows[r] as readonly string[]).map(quote).join(like.separator)
  const end = like.unread !== "" ? like.newline + like.unread : like.trailing ? like.newline : ""
  return like.prelude + parts.join(like.newline) + end
}
