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
  cells_fix_stale(h: number): void
  cells_keep_stale(h: number, row: number, column: number): number
  cells_stored_since(h: number, before: number): void
  cells_read_as(ptr: number, len: number, cap: number, sep: number): number
  cells_detect(ptr: number, len: number): number
  cells_token_kinds(): number
  cells_tokens(ptr: number, len: number, cap: number): number
  cells_layout(ptr: number, len: number, cap: number, fold: number): number
  cells_reshaped(h: number, ptr: number, len: number, cap: number): number
  cells_written_as(h: number, sep: number, ptr: number, len: number, cap: number): number
  cells_naming_why(h: number, i: number): number
  cells_spans(h: number): number
  cells_read_with(ptr: number, len: number, cap: number, files: number, filesLen: number, filesCap: number): number
  cells_offers(h: number, column: number): number
  cells_complete(h: number, ptr: number, len: number, cap: number, at: number): number
  cells_hover(h: number, ptr: number, len: number, cap: number, at: number): number
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
  /** What a cell of the column may hold, for a host offering it as the cell is typed
   *  (`Doc::offers`): a choice's words, a boolean's two, a reference's keys (every row of
   *  the column it names, in the table's order, each once; none for a table not read,
   *  so a file read alone offers no keys); empty for any other type. */
  offers(): string[] {
    const all = returned(x.cells_offers(this.doc.handle, this.index))
    return all === "" ? [] : all.slice(0, -1).split("\u001f")
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
   *  text's `=` written `'=`, so it never reads as a computed cell. */
  form(v: Value): string {
    if (v === null) return ""
    if (typeof v === "string" && v.includes("=")) return v.replaceAll("=", "'=")
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

  /** The cell's text as the file writes it, unquoted: `84.20=` where its column's formula
   *  computes it (blank before a write stores it), `32.25=96.75 / 3` where its own does. */
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
   *  two do; `stale`: a computed cell storing another value than its formula gives (its
   *  value the formula's; `fixStale` or `keepStale` resolve it). */
  kind: "type" | "missing" | "key" | "ragged" | "formula" | "reference" | "stale"
  /** How much it weighs, the same in every reader (`Kind::severity`): `error`, a value
   *  that is wrong (its type broken, a key repeated, a reference to no row, a formula
   *  that does not compute); `warning`, something absent or behind (a value not written,
   *  a stale value, a ragged row). */
  severity: Severity
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

/** What a problem weighs: what is wrong, or what is not there yet. A break of the grammar
 *  is always an error. */
export type Severity = "error" | "warning"

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

const KINDS = ["type", "missing", "key", "ragged", "formula", "reference", "stale"] as const
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
    const p = new Uint32Array(x.memory.buffer, this.meta.problems, 4 * n).slice()
    const out: Problem[] = []
    for (let k = 0; k < n; k++) {
      const row = p[4 * k] as number
      const at = p[4 * k + 1] as number
      const doc = this
      out.push({
        row,
        at,
        column: (this.columns[at] as Column).name,
        kind: KINDS[p[4 * k + 2] as number] as Problem["kind"],
        severity: p[4 * k + 3] === 1 ? "warning" : "error",
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

/** A `.cells` file's text (or its bytes), read; its imports followed through `files` when
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

/** What a span of a file is, as the engine names it (`cells::tokens::Kind::ALL`, in its
 *  order): checked against the engine's own list on the first `tokens`, so a kind the
 *  engine adds screams here instead of passing as a word no type knows. */
export const TOKEN_KINDS = [
  "fence",
  "comment",
  "keyword",
  "name",
  "type",
  "mark",
  "bound",
  "format",
  "choice",
  "punct",
  "number",
  "reference",
  "operator",
  "cellText",
  "cellNumber",
  "cellBool",
  "cellDate",
  "cellChoice",
  "cellLink",
  "cellId",
  "error",
  "string",
  "code",
  "table",
] as const

export type TokenKind = (typeof TOKEN_KINDS)[number]

/** A span of a file: `from` to `to` in UTF-16 units (a string's own indices), what it
 *  is, the column it stands in (a header cell's parts, a cell), and for a name a formula
 *  reads the column of its own table it names (`cost`, `TABLE['k7'].cost`); null where
 *  there is none. */
export interface Token {
  from: number
  to: number
  kind: TokenKind
  column: number | null
  reads: number | null
}

/** The engine's kinds by their number across the boundary, once checked. */
let kinds: readonly TokenKind[] | null = null

function tokenKinds(): readonly TokenKind[] {
  if (kinds) return kinds
  const engine = returned(x.cells_token_kinds()).split("\n")
  if (engine.join() !== TOKEN_KINDS.join())
    throw new Error(`cells: the engine's token kinds are ${engine.join(", ")}, the wrapper's ${TOKEN_KINDS.join(", ")}`)
  kinds = TOKEN_KINDS
  return kinds
}

/** What every part of a file is, read from its text alone and never refused (a broken
 *  file still has its spans): what a highlighter paints, the same reading a language
 *  server's semantic tokens carry. In order, none overlapping; what lies between (spaces,
 *  line breaks) is nothing. */
export function tokens(text: string): Token[] {
  const kinds = tokenKinds()
  const [p, len, cap] = put(text)
  const at = x.cells_tokens(p, len, cap)
  const raw = new Uint32Array(x.memory.buffer, at, x.cells_len() / 4).slice()
  const none = 0xffffffff
  const out: Token[] = []
  for (let i = 0; i < raw.length; i += 5) {
    const kind = kinds[raw[i + 2] as number]
    if (!kind) throw new Error(`cells: a token of kind ${raw[i + 2]}, which the engine does not name`)
    const column = raw[i + 3] as number
    const reads = raw[i + 4] as number
    out.push({
      from: raw[i] as number,
      to: raw[i + 1] as number,
      kind,
      column: column === none ? null : column,
      reads: reads === none ? null : reads,
    })
  }
  return out
}

/** A file's tables laid out as tables, by the rule `cells fmt` writes (SPEC.md,
 *  Formatting), for a view that aligns their columns without writing a byte: `pads`,
 *  spaces drawn at `at` (a string's index, a value's start) where the file has fewer
 *  than bring the value to its column's start, none for a file formatted; `cuts`, with a
 *  `fold`, the middle of each value wider than it, `from` to `to`, drawn folded until a
 *  reader opens it (a header cell's name kept whole). Widths in characters. */
export interface Layout {
  pads: { at: number; width: number }[]
  cuts: { from: number; to: number }[]
}

export function layout(text: string, fold?: number): Layout {
  if (fold !== undefined && !(Number.isInteger(fold) && fold >= 8))
    throw new Error(`cells: a fold is a whole number of characters, 8 or more, not ${fold}`)
  const [p, len, cap] = put(text)
  const at = x.cells_layout(p, len, cap, fold ?? 0)
  const raw = new Uint32Array(x.memory.buffer, at, x.cells_len() / 4).slice()
  const count = raw[0] as number
  const pads: Layout["pads"] = []
  for (let i = 0; i < count; i++) pads.push({ at: raw[1 + 2 * i] as number, width: raw[2 + 2 * i] as number })
  const cuts: Layout["cuts"] = []
  for (let i = 1 + 2 * count; i < raw.length; i += 2) cuts.push({ from: raw[i] as number, to: raw[i + 1] as number })
  return { pads, cuts }
}

/** What a completion is: a row's key or a yes or a no (`value`), a choice's word, a named
 *  type, a table's key column (`reference`, `accounts.account`), the language's or the
 *  prelude's word or a base type (`keyword`), a column, a table, a function. */
export type CompletionKind = "value" | "choice" | "type" | "reference" | "keyword" | "column" | "table" | "function"

/** What may be written at a place: `insert` in place of `from` to `to` of the text (the
 *  word being typed, a string's indices), shown as `label` with its `detail`. */
export interface Completion {
  label: string
  kind: CompletionKind
  detail: string
  insert: string
  from: number
  to: number
  /** What it is matched by, where that is not its label (`#3 card`). */
  filter?: string
  /** Where it sorts, where that is not by its label. */
  sort?: string
}

/** What may be written at `at` (a string's index) of `text`, the text `doc` was read from
 *  or one like it, a draft spliced in where it is typed (the core's `cells::complete`,
 *  what `cells lsp` completes): a cell's values (a reference's keys, a choice's words, a
 *  boolean's), a type's names and bases, a formula's columns, tables, rows, functions and
 *  words, the prelude's statements. The tables it names are those `doc` was read with. */
export function complete(doc: Doc, text: string, at: number): Completion[] {
  if (!(Number.isInteger(at) && at >= 0 && at <= text.length)) throw new CellsError(`at ${at}`, `is no place in a text of ${text.length}`)
  const [p, len, cap] = put(text)
  return JSON.parse(returned(x.cells_complete(doc.handle, p, len, cap, at))) as Completion[]
}

/** A part of what is said: plain words, words written as code (a header cell, a formula,
 *  a name), what matters most (a cell's value), or a paragraph's end. */
export type HoverPiece = { kind: "plain" | "code" | "strong"; text: string } | { kind: "break" }

/** What is said of a place: its pieces, and `from` to `to` of the text it is about. */
export interface Hover {
  pieces: HoverPiece[]
  from: number
  to: number
  /** In plain lines, a paragraph a line: what an app shows. */
  readonly text: string
}

/** What is said of `at` (a string's index) of `text`, as for `complete` (the core's
 *  `cells::hover`, what `cells lsp` says on hover): a header cell's column, a cell's
 *  value (its problem, the row its reference names), a formula's name or function, a
 *  named type or a table in the prelude; null where nothing is. */
export function hover(doc: Doc, text: string, at: number): Hover | null {
  if (!(Number.isInteger(at) && at >= 0 && at <= text.length)) throw new CellsError(`at ${at}`, `is no place in a text of ${text.length}`)
  const [p, len, cap] = put(text)
  const said = returned(x.cells_hover(doc.handle, p, len, cap, at))
  if (said === "") return null
  const raw = JSON.parse(said) as { pieces: [string, string?][]; from: number; to: number }
  const pieces = raw.pieces.map(([kind, text]): HoverPiece => (kind === "break" ? { kind } : { kind: kind as "plain", text: text as string }))
  return {
    pieces,
    from: raw.from,
    to: raw.to,
    get text() {
      return pieces.map((q) => (q.kind === "break" ? "\n" : q.text)).join("")
    },
  }
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

/** What a cell's text, as the file writes it, stores (SPEC.md, A cell), for a host that
 *  keeps its own rows and shows them: left of its first `=` with no `'` before it (the
 *  core's `read::split`), each `'=` read as `=`. `84.20=` and `84.20=cost * 2` show
 *  `84.20`, a lone `=` nothing yet, `a'=b` `a=b`. Pure. */
export function shown(text: string): string {
  let at = text.indexOf("=")
  while (at > 0 && text[at - 1] === "'") at = text.indexOf("=", at + 1)
  return (at < 0 ? text : text.slice(0, at)).replaceAll("'=", "=")
}

/** What a person typed into a cell, as the file writes it: a text starting with `=` is a
 *  formula (`=96.75 / 3` the cell's own, a lone `=` its column's, its value stored by the
 *  next write), any other a value, its `=` written `'=` (`a=b` stays the text `a=b`), a
 *  value with no `=` an override of a computed cell. Pure. */
export function typed(text: string): string {
  return text.startsWith("=") ? text : text.replaceAll("=", "'=")
}

/** One cell's text changed; every other byte stays as read but the computed values the
 *  change reaches, stored left of their `=` (SPEC.md, Writing). Returns `doc`, changed;
 *  text holding a control character throws `CellsError` at the row, `doc` untouched.
 *  `=96.75 / 3` is the cell's own formula (its value stored by this write); blank, or
 *  a lone `=`, in a column with a formula, the formula's; a value with no `=`, written. */
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

/** Every stale cell given its formula's value (`cells fix`); an override is never touched.
 *  Returns `doc`, changed. */
export function fixStale(doc: Doc): Doc {
  x.cells_fix_stale(doc.handle)
  doc.refresh()
  return doc
}

/** One stale cell's stored value kept as the row's override: its `=` and formula dropped.
 *  Returns `doc`, changed. */
export function keepStale(doc: Doc, row: number, column: string | number): Doc {
  const k = columnOf(doc, column)
  if (row < 0 || row >= doc.rowCount) throw new CellsError(`row ${row}`, "does not exist")
  if (x.cells_keep_stale(doc.handle, row, k) !== 0) throw thrown()
  doc.refresh()
  return doc
}

/** `doc` (the text a client holds now, read) as a write after `before` (the text as last
 *  written) leaves it: every computed cell the edits since reached stored, each row found
 *  in `before` by its written cells, and every one stored nowhere yet (a lone `=`, a blank
 *  field of a computed column); a stale cell they did not reach is left as read. What
 *  a client storing on save writes. Returns `doc`, changed. */
export function storedSince(doc: Doc, before: Doc): Doc {
  x.cells_stored_since(doc.handle, before.handle)
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
 *  every row's field as written; a `.cells` file whose columns no longer read (two of one
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
 *  every computed cell its value; a `.cells` file's header `header` when given (each
 *  column's cell with its type), its prelude kept. */
export function writtenAs(doc: Doc, dialect: Dialect, header?: readonly string[]): string {
  const [p, len, cap] = put(new Spec().list(header ?? []).bytes())
  return returned(x.cells_written_as(doc.handle, sepByte(dialect), p, len, cap))
}

/** A cell's text in a file's form: quoted when it holds the separator, `"` or a line
 *  break, in a Cells file when it starts or ends with a space or a tab (which would read
 *  as padding), or always in a plain file that quotes every field (the core's
 *  `scan::quote_in`). */
function quoter(dialect: Dialect, all: boolean): (text: string) => string {
  const separator = dialect === "cells" ? "," : dialect
  const padded = dialect === "cells"
  return (text) =>
    all || text.includes(separator) || /["\r\n]/.test(text) || (padded && /^[ \t]|[ \t]$/.test(text))
      ? `"${text.replaceAll('"', '""')}"`
      : text
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
    dialect: Dialect
    separator: Separator
    quotesAll: boolean
    unread: string
  },
  header: readonly string[],
  rows: readonly (readonly string[])[],
): string {
  // A header that did not read leaves nothing but what follows the prelude.
  if (header.length === 0 && like.unread !== "") return like.prelude + like.unread
  const quote = quoter(like.dialect, like.quotesAll)
  const parts: string[] = new Array(rows.length + 1)
  parts[0] = header.map(quote).join(like.separator)
  // A Cells file whose first line is `---` opens a prelude: a header that would be one is
  // quoted. A byte order mark alone is no prelude, and no part of the first line.
  const bom = String.fromCharCode(0xfeff)
  if (like.dialect === "cells" && (like.prelude === "" || like.prelude === bom) && parts[0] === "---") parts[0] = '"---"'
  for (let r = 0; r < rows.length; r++) parts[r + 1] = (rows[r] as readonly string[]).map(quote).join(like.separator)
  const end = like.unread !== "" ? like.newline + like.unread : like.trailing ? like.newline : ""
  return like.prelude + parts.join(like.newline) + end
}
