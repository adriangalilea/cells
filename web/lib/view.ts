// The engine as the site reads files with it: a story step's files read together, the
// first one through its imports (the others), a plain `.csv` by its separator. Every
// doc is freed once what the view needs is taken from it. The engine is given, never
// imported here: it is reached through lib/engine-module (`loadEngine`), once its module
// has streamed in. (lib/cells is ui's: the same engine with its bytes embedded, which
// ui's table codec imports; the site streams its own copy, lib/engine, instead.)

import type { Column, Problem, Tinted, Value } from "@/lib/engine/cells.ts"
import type { StoryFile } from "@/lib/story"

/** The engine, as `loadEngine()` gives it. */
export type Engine = typeof import("@/lib/engine/cells.ts")

/** A column as a view draws it. */
export interface ViewColumn {
  name: string
  /** The header cell after its `:` (type and formula), `""` for a plain column. */
  spelled: string
  base: string
  formula?: string
  key: boolean
  inferred: boolean
  /** A choice's words, in the order its type names them. */
  choices?: string[]
}

/** How a cell looks, as the engine's looks say (a choice's word in its tint, a value as a
 *  look draws it): `drawn`, the words shown in place of its text; `tint`, a tint's name, a
 *  `#rrggbb`, or a scale's point between two. */
export interface ViewFace {
  drawn?: string
  tint?: Tinted
}

/** A cell as a view draws it: its value as text, where it came from, and its problem. */
export interface ViewCell {
  text: string
  shown: string
  origin: "written" | "formula" | "cell"
  numeric: boolean
}

export interface ViewTable {
  columns: ViewColumn[]
  rows: ViewCell[][]
  /** Each value by column name, for a step that asks questions of it. */
  values: Record<string, Value[]>
  /** Each cell's face, by column name, for the columns the engine's looks draw: null for a
   *  cell drawn as its text. */
  faces: Record<string, (ViewFace | null)[]>
  /** Each cell's problem, weighed as the engine weighs it (`severity`: `error` wrong,
   *  `warning` absent or behind). */
  problems: { row: number; at: number; column: string; kind: Problem["kind"]; severity: Problem["severity"]; why: string; reason: string }[]
  /** Where the file's grammar breaks (a header cell half typed, a prelude statement):
   *  only that part degrades, the table always reads. `why` names what broke; `span` is
   *  its start and length in UTF-16 units of the text, a string's indices. */
  broken: { at: string; why: string; span?: [number, number] }[]
}

const NUMERIC = new Set(["int", "num", "pct", "money"])

/** A value as the table shows it: a decimal at its scale, a boolean in the file's own
 *  words, a broken cell as it is written (its problem says why), the rest as the column
 *  writes it. */
function shown(c: Column, r: number, v: Value): string {
  if (v === null) return c.origin(r) === "written" ? c.text(r) : ""
  if (c.kind === "decimal") return c.decimal(r) ?? ""
  return c.form(v)
}

/** The first file of `files`, read as the reader would open it. A file always opens with
 *  its table; the engine throws only for bytes that are not UTF-8, which a string never
 *  holds, so a throw here is a bug and screams. */
export function readFiles(engine: Engine, files: readonly StoryFile[]): ViewTable {
  const [first, ...others] = files
  if (!first) throw new Error("view: a step with no file")
  const r = first.name.endsWith(".csv")
    ? engine.readAs(first.text, engine.detect(first.text))
    : engine.read(first.text, { at: first.name, files: Object.fromEntries(others.map((f) => [f.name, f.text])) })
  try {
    const columns = r.columns.map(
      (c): ViewColumn => ({
        name: c.name,
        spelled: c.spelled,
        base: c.base,
        ...(c.formula ? { formula: c.formula } : {}),
        key: c.type.includes("!"),
        inferred: c.inferred,
        ...(c.choices ? { choices: c.choices } : {}),
      }),
    )
    const looks = r.doc.looks()
    const faces: Record<string, (ViewFace | null)[]> = Object.fromEntries(
      Object.entries(looks.columns).map(([at, c]) => [
        r.columns[Number(at)]?.name as string,
        c.drawn.map((drawn, row) => {
          const tint = c.tint[row] ?? null
          if (drawn === "" && tint === null) return null
          return { ...(drawn === "" ? {} : { drawn }), ...(tint === null ? {} : { tint }) }
        }),
      ]),
    )
    const rows: ViewCell[][] = []
    const values: Record<string, Value[]> = Object.fromEntries(r.columns.map((c) => [c.name, []]))
    for (let i = 0; i < r.doc.rowCount; i++)
      rows.push(
        r.columns.map((c) => {
          const v = c.value(i)
          values[c.name]?.push(v)
          return { text: c.text(i), shown: shown(c, i, v), origin: c.origin(i), numeric: NUMERIC.has(c.base) }
        }),
      )
    const problems = r.problems.map((p) => ({ row: p.row, at: p.at, column: p.column, kind: p.kind, severity: p.severity, why: p.why, reason: p.reason }))
    const broken = r.broken.map((b) => ({ at: b.at, why: b.why, ...(b.span ? { span: b.span } : {}) }))
    return { columns, rows, values, faces, problems, broken }
  } finally {
    r.doc.free()
  }
}

/** A `.cells` file read through its imports (`others`). */
const readCell = (engine: Engine, file: StoryFile, others: readonly StoryFile[]) =>
  engine.read(file.text, { at: file.name, files: Object.fromEntries(others.map((f) => [f.name, f.text])) })

/** `now` as the language server's formatting writes it on save, `saved` the file as last
 *  written (`cells lsp`, SPEC.md Writing): every computed value the edits since reached,
 *  and every one stored nowhere yet, stored beside its `=`; a stale cell they did not
 *  reach left as it is. A plain `.csv` is written as typed: nothing in it computes. */
export function savedAs(engine: Engine, saved: StoryFile, now: StoryFile, others: readonly StoryFile[]): string {
  if (now.name.endsWith(".csv")) return now.text
  const before = readCell(engine, saved, others)
  const after = readCell(engine, now, others)
  try {
    return engine.write(engine.storedSince(after.doc, before.doc))
  } finally {
    before.doc.free()
    after.doc.free()
  }
}

/** A cell written by hand in the app (ui's table-session `write`): `value` set in the
 *  cell, a row one past the last first added as a blank record, as an app's new row is,
 *  and the file written, every computed value the change reaches stored beside its `=`. */
export function writeCell(
  engine: Engine,
  file: { path: string; text: string },
  cell: { row: number; column: string },
  value: string,
  others: readonly { path: string; text: string }[],
): string {
  const files = others.map((o) => ({ name: o.path, text: o.text }))
  let text = file.text
  const probe = readCell(engine, { name: file.path, text }, files)
  const rows = probe.rows.length
  const width = probe.columns.length
  probe.doc.free()
  if (cell.row > rows) throw new Error(`view: row ${cell.row} is past the new row ${rows}`)
  if (cell.row === rows) text = `${text.endsWith("\n") || text === "" ? text : `${text}\n`}${",".repeat(width - 1)}\n`
  const r = readCell(engine, { name: file.path, text }, files)
  try {
    engine.setCell(r.doc, cell.row, cell.column, value)
    return engine.write(r.doc)
  } finally {
    r.doc.free()
  }
}
