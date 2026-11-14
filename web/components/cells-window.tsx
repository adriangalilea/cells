"use client"

// THE CELLS APP'S WINDOW on the hero's Mac: a file read by the engine at the moment drawn,
// as the app shows it (ui's table: typed columns, computed cells lit by the edge light,
// every broken cell marked with the engine's reason), in ui's AppWindow.

import * as React from "react"
import { AppWindow } from "@/components/ui/app-window"
import { Table, type TableColumn, type TableFace, type TableProblem } from "@/components/ui/table"
import type { Tinted } from "@/lib/engine/cells.ts"

/** A tint as a CSS colour: one of the ten names in the tokens' colour for it, a `#rrggbb`
 *  as the file gives it, a scale's point mixed between its two ends. */
const tintOf = (t: Tinted): string =>
  typeof t === "string" ? (t.startsWith("#") ? t : `var(--tint-${t})`) : `color-mix(in oklab, ${tintOf(t.to)} ${t.at / 10}%, ${tintOf(t.from)})`
import { useEngine } from "@/components/use-engine"
import { readFiles, type ViewCell, type ViewTable } from "@/lib/view"
import { useCodeColours } from "@/components/ui/use-code-colours"
import { runsOf } from "@/lib/code-tokens"
import { CELLS_COLUMNS } from "@/lib/cells-code"

/** Column `k`'s colour (`CELLS_COLUMNS`), each scheme its own. */
const inkOf = (k: number) => {
  const c = CELLS_COLUMNS[k % CELLS_COLUMNS.length] as (typeof CELLS_COLUMNS)[number]
  return `light-dark(${c.light}, ${c.dark})`
}
import { PRODUCT } from "@/lib/product"
import { revealX } from "@/lib/reveal"
import { cn } from "@/lib/utils"

type Row = { id: string; cells: ViewCell[] }

const NONE: { name: string; text: string }[] = []

/** A file read as it stands. It always has its table: a break in its grammar (a header
 *  cell half typed) degrades only what it breaks, that column read as text. */
function useRead(name: string, text: string, others: { name: string; text: string }[]): ViewTable {
  const engine = useEngine()
  return React.useMemo(() => readFiles(engine, [{ name, text }, ...others]), [engine, name, text, others])
}

/** What a scene shows in the table: a column by its name, or one cell of it (the data
 *  row, from 0), lit as ui's table lights what a column's menu acts on. */
export interface TableLight {
  column: string
  row?: number
}

/** A hand in a window: its cursor (a data row from 0, a column by name) and the text it
 *  is typing into that cell, none between keys. */
export interface CellsScene {
  cursor: { row: number; column: string } | null
  draft: string | null
}

/** The lines of a file's prelude (between its first two `---` lines), by their place in
 *  the file's lines; nothing for a file without one. */
function preludeLines(text: string): { from: number; to: number } | null {
  const lines = text.split("\n")
  if (lines[0] !== "---") return null
  const end = lines.indexOf("---", 1)
  return end > 1 ? { from: 1, to: end } : null
}

/** The prelude's panel along the window's foot, as the app opens it for a Cells file: its
 *  name, then its lines as written, coloured as the engine reads the whole file (a declared
 *  column in its column's colour, a script in its language). As tall as its lines, up to
 *  two fifths of the window, so the table keeps the most of it. */
function Prelude({ text }: { text: string }) {
  const range = preludeLines(text)
  const colours = useCodeColours(text, "cells")
  if (!range) return null
  const lines = runsOf(text, colours).slice(range.from, range.to)
  return (
    <div data-slot="cells-window-prelude" aria-label="prelude" className="flex max-h-[40%] shrink-0 flex-col border-t border-white/8 p-2">
      {/* its text as code, in a well sunk in the window, as the app shows a script's */}
      <pre className="min-h-0 overflow-hidden rounded-md bg-black/30 px-2.5 py-1.5 font-mono text-[10px] leading-[1.6] text-white/85">
        {lines.map((line, i) => (
          <div key={i} className="truncate">
            {line.length === 0
              ? " "
              : line.map((run, j) => (
                  <span key={j} style={run.ink ? { color: run.ink.dark, fontWeight: run.ink.bold ? 600 : undefined } : undefined}>
                    {run.text}
                  </span>
                ))}
          </div>
        ))}
      </pre>
    </div>
  )
}

/** A file as a table: its columns, its rows, its problems by cell. */
export function CellsTable({
  name,
  text,
  others = NONE,
  label,
  light,
  scene,
  className,
}: {
  name: string
  text: string
  others?: { name: string; text: string }[]
  label: string
  light?: TableLight
  /** A hand in the window (ui's table-session `sceneOf`): its cursor, a data row and a
   *  column by name, and what it is typing. */
  scene?: CellsScene | null
  className?: string
}) {
  const read = useRead(name, text, others)
  // The hand on the row after the last is on a new row, shown empty until its first
  // cell is written, as an app's new row is.
  const adding = scene?.cursor?.row === read.rows.length
  const rows = React.useMemo<Row[]>(
    () => [...read.rows.map((cells, i) => ({ id: String(i), cells })), ...(adding ? [{ id: String(read.rows.length), cells: [] }] : [])],
    [read, adding],
  )
  const at = scene?.cursor ? read.columns.findIndex((c) => c.name === scene.cursor?.column) : -1
  if (scene?.cursor && at < 0) throw new Error(`cells-window: no column ${scene.cursor.column} for the hand`)
  const tableScene = scene ? { cursor: scene.cursor ? { row: String(scene.cursor.row), column: String(at) } : null, draft: scene.draft } : undefined
  // The engine's looks: a cell's words in place of its text, its tint a colour.
  const face = React.useCallback(
    (r: Row, column: string): TableFace | undefined => {
      const f = read.faces[read.columns[Number(column)]?.name ?? ""]?.[Number(r.id)]
      if (!f) return undefined
      return { ...(f.drawn ? { drawn: f.drawn } : {}), ...(f.tint ? { tint: tintOf(f.tint) } : {}) }
    },
    [read],
  )
  const problems = React.useMemo(() => {
    const m = new Map<string, TableProblem>()
    for (const p of read.problems) m.set(`${p.row}:${p.at}`, { why: p.reason, severity: p.severity })
    return m
  }, [read])
  const columns = React.useMemo<TableColumn<Row>[]>(
    () =>
      read.columns.map((c, k) => ({
        id: String(k),
        label: c.name,
        // its name in its column's colour, as the app's head and the file's text wear it
        tint: inkOf(k),
        text: (r) => r.cells[k]?.shown ?? "",
        kind: r0Numeric(read.rows, k) ? "number" : "text",
        // a choice is the table's badge, each word in its tint as the app draws it
        ...(c.choices ? { options: c.choices } : {}),
        render: read.faces[c.name] ? undefined : (r) => {
          const cell = r.cells[k]
          if (!cell) return null
          const computed = cell.origin !== "written"
          return (
            <span
              className={cn(
                "truncate font-mono slashed-zero",
                computed && "text-edge [text-shadow:0_0_12px_color-mix(in_oklch,var(--edge)_45%,transparent)]",
              )}
              title={computed ? (cell.origin === "formula" ? `${c.name} = ${c.formula}` : cell.text) : undefined}
            >
              {cell.shown}
            </span>
          )
        },
      })),
    [read],
  )
  // The table's own light, its columns named by their place; a cell with a problem is
  // already marked (the table's own mark), so a light on it alone is none.
  const k = light ? read.columns.findIndex((c) => c.name === light.column) : -1
  if (light && k < 0) throw new Error(`cells-window: no column ${light.column} to light`)
  const marked = light?.row !== undefined && problems.has(`${light.row}:${k}`)
  const lit = light && !marked ? { column: String(k), row: light.row === undefined ? undefined : String(light.row) } : undefined
  // The lit column kept in sight when the table is wider than its box.
  const root = React.useRef<HTMLDivElement>(null)
  React.useLayoutEffect(() => {
    if (lit) revealX(root.current?.querySelector<HTMLElement>(`[data-slot=table-cell][data-column="${lit.column}"]`) ?? null)
  }, [lit?.column, lit?.row, lit])
  return (
    <div ref={root} className="contents">
      <Table
        rows={rows}
        rowId={(r) => r.id}
        columns={columns}
        label={label}
        problem={(r, column) => problems.get(`${r.id}:${column}`)}
        face={face}
        density="compact"
        lines="rows"
        interactive={false}
        lit={lit}
        scene={tableScene}
        className={className}
      />
    </div>
  )
}

const r0Numeric = (rows: ViewCell[][], k: number) => rows.some((r) => r[k]?.numeric)

/** A file's name, its extension lit (an extension alone, all of it): what tells a CSV
 *  from a Cells file, ours in the edge's blue, any other in the haze's amber. */
export function FileName({ name }: { name: string }) {
  const dot = name.lastIndexOf(".")
  if (dot < 0) return name
  const ext = name.slice(dot)
  return (
    <>
      {name.slice(0, dot)}
      <span className={ext === PRODUCT.extension ? "text-edge" : "text-haze"}>{ext}</span>
    </>
  )
}

/** The app's window: a title bar with the file's name, the table under it, and a strip of
 *  a fixed height at its foot with the file's problems (their count and the engine's own
 *  words), so a problem appearing or going never moves anything. */
export function CellsWindow({
  name,
  text,
  others = NONE,
  light,
  scene,
  prelude = true,
  className,
}: {
  name: string
  text: string
  /** Its prelude's panel along its foot, as the app opens it; false, the table alone. */
  prelude?: boolean
  /** The files it imports, as they stand. */
  others?: { name: string; text: string }[]
  /** A column or a cell lit, the rest dimmed. */
  light?: TableLight
  /** A hand working in it (ui's table-session `sceneOf`). */
  scene?: CellsScene | null
  className?: string
}) {
  const now = useRead(name, text, others)
  // A break in the grammar first (it names its column), then the cells' problems.
  const said = [...now.broken.map((b) => b.why), ...now.problems.map((p) => p.why)]
  // The count in the tone of the worst: a break or a wrong value, else only what is absent.
  const wrong = now.broken.length > 0 || now.problems.some((p) => p.severity === "error")
  return (
    <AppWindow
      data-slot="cells-window"
      title={
        <>
          <FileName name={name} /> · {PRODUCT.name}
        </>
      }
      scheme="dark"
      className={cn("rounded-[10px] [--ag-table-ground:var(--ag-app-window-ground)]", className)}
    >
      <div className="flex h-full flex-col">
        <div className="min-h-0 flex-1 overflow-hidden">
          {/* The table's own type size: its columns are counted in its characters. */}
          <CellsTable name={name} text={text} others={others} label={name} light={light} scene={scene} className="h-full text-[11px]" />
        </div>
        {prelude && <Prelude text={text} />}
        <div
          data-slot="cells-window-problems"
          className="flex h-6 shrink-0 items-center gap-2 overflow-hidden border-t border-white/8 px-3 font-mono text-[10px] whitespace-nowrap"
        >
          {/* The count in a slot of its own width, whatever it says: nothing beside it moves. */}
          <span className={cn("w-[10ch] shrink-0 tabular-nums", !said.length ? "text-edge/80" : wrong ? "text-haze" : "text-tone-warn")}>
            {said.length ? `${said.length} ${said.length === 1 ? "problem" : "problems"}` : "no problems"}
          </span>
          <span className="min-w-0 truncate text-white/60">{said.join(" · ")}</span>
        </div>
      </div>
    </AppWindow>
  )
}
