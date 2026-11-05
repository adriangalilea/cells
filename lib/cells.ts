// The engine as the site reads files with it: a story step's files read together, the
// first one through its imports (the others), a plain `.csv` by its separator. Every
// doc is freed once what the view needs is taken from it. The engine is given, never
// imported here: it is reached through lib/engine-module (`loadEngine`), once its module
// has streamed in.

import type { Column, Problem, Value } from "@/lib/engine/cells.ts"
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
  problems: { row: number; at: number; column: string; kind: Problem["kind"]; why: string; reason: string }[]
  /** Where the file's grammar breaks (a header cell half typed, a prelude statement):
   *  only that part degrades, the table always reads. `why` names what broke; `span` is
   *  its first byte and length in UTF-8 bytes of the text. */
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
  if (!first) throw new Error("cells: a step with no file")
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
      }),
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
    const problems = r.problems.map((p) => ({ row: p.row, at: p.at, column: p.column, kind: p.kind, why: p.why, reason: p.reason }))
    const broken = r.broken.map((b) => ({ at: b.at, why: b.why, ...(b.span ? { span: b.span } : {}) }))
    return { columns, rows, values, problems, broken }
  } finally {
    r.doc.free()
  }
}
