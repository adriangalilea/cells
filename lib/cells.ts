// The engine as the site reads files with it: a story step's files read together, the
// first one through its imports (the others), a plain `.csv` by its separator. Every
// doc is freed once what the view needs is taken from it.

import { CellsError, type Column, detect, type Problem, read, readAs, type Value } from "@/lib/engine/cells.ts"
import type { StoryFile } from "@/lib/story"

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
  /** The file does not read at all (its grammar): where and why. */
  error?: string
}

const NUMERIC = new Set(["int", "num", "pct", "money"])

/** A value as the table shows it: money at its scale, everything else as written. */
function shown(c: Column, r: number, v: Value): string {
  if (v === null) return ""
  if (c.kind === "decimal") return c.decimal(r) ?? ""
  return String(v)
}

/** The first file of `files`, read as the reader would open it. */
export function readFiles(files: readonly StoryFile[]): ViewTable {
  const [first, ...others] = files
  if (!first) throw new Error("cells: a step with no file")
  try {
    const r = first.name.endsWith(".csv")
      ? readAs(first.text, detect(first.text))
      : read(first.text, { at: first.name, files: Object.fromEntries(others.map((f) => [f.name, f.text])) })
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
      return { columns, rows, values, problems }
    } finally {
      r.doc.free()
    }
  } catch (e) {
    // A file whose grammar does not read (being typed, mid-edit) is shown as that, never
    // as a crash; anything else is a bug and screams.
    if (e instanceof CellsError) return { columns: [], rows: [], values: {}, problems: [], error: `${e.where}: ${e.why}` }
    throw e
  }
}
