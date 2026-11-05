"use client"

// THE CELLS APP'S WINDOW on the hero's Mac: a file read by the engine at the moment drawn,
// as the app shows it (ui's table: typed columns, computed cells lit by the edge light,
// every broken cell marked with the engine's reason), in ui's AppWindow.

import * as React from "react"
import { AppWindow } from "@/components/ui/app-window"
import { Table, type TableColumn } from "@/components/ui/table"
import { useEngine } from "@/components/use-engine"
import { readFiles, type ViewCell, type ViewTable } from "@/lib/view"
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

/** What a scene lights in the table: a column, or one cell of it, and how far (0..1);
 *  everything else is dimmed as far as `dim`. */
export interface TableLight {
  column: string
  row?: number
  amount: number
  dim: number
}

/** A file as a table: its columns, its rows, its problems by cell. */
export function CellsTable({
  name,
  text,
  others = NONE,
  label,
  light,
  className,
}: {
  name: string
  text: string
  others?: { name: string; text: string }[]
  label: string
  light?: TableLight
  className?: string
}) {
  const read = useRead(name, text, others)
  const rows = React.useMemo<Row[]>(() => read.rows.map((cells, i) => ({ id: String(i), cells })), [read])
  const problems = React.useMemo(() => {
    const m = new Map<string, string>()
    for (const p of read.problems) m.set(`${p.row}:${p.at}`, p.reason)
    return m
  }, [read])
  const columns = React.useMemo<TableColumn<Row>[]>(
    () =>
      read.columns.map((c, k) => ({
        id: String(k),
        label: c.name,
        text: (r) => r.cells[k]?.shown ?? "",
        kind: r0Numeric(read.rows, k) ? "number" : "text",
        render: (r) => {
          const cell = r.cells[k]
          if (!cell) return null
          const computed = cell.origin !== "written"
          const target = light && c.name === light.column && (light.row === undefined || String(light.row) === r.id)
          const broken = problems.has(`${r.id}:${k}`)
          return (
            <span
              data-lit-cell={target || undefined}
              className={cn(
                "truncate rounded-[3px] px-1 -mx-1 font-mono slashed-zero",
                computed && "text-edge [text-shadow:0_0_12px_color-mix(in_oklch,var(--edge)_45%,transparent)]",
              )}
              style={
                light
                  ? target
                    ? {
                        background: `color-mix(in oklch, var(${broken ? "--haze" : "--edge"}) ${Math.round(22 * light.amount)}%, transparent)`,
                        boxShadow: `0 0 0 1px color-mix(in oklch, var(${broken ? "--haze" : "--edge"}) ${Math.round(55 * light.amount)}%, transparent)`,
                      }
                    : { opacity: 1 - 0.6 * light.dim }
                  : undefined
              }
              title={computed ? (cell.origin === "formula" ? `${c.name} = ${c.formula}` : cell.text) : undefined}
            >
              {cell.shown}
            </span>
          )
        },
      })),
    [read, light, problems],
  )
  // The lit column kept in sight when the table is wider than its box.
  const root = React.useRef<HTMLDivElement>(null)
  const lit = light && light.amount > 0
  React.useLayoutEffect(() => {
    if (lit) revealX(root.current?.querySelector<HTMLElement>("[data-lit-cell]") ?? null)
  }, [lit])
  return (
    <div ref={root} className="contents">
    <Table
      rows={rows}
      rowId={(r) => r.id}
      columns={columns}
      label={label}
      problem={(r, column) => problems.get(`${r.id}:${column}`)}
      density="compact"
      lines="rows"
      interactive={false}
      className={className}
    />
    </div>
  )
}

const r0Numeric = (rows: ViewCell[][], k: number) => rows.some((r) => r[k]?.numeric)

/** The app's window: a title bar with the file's name, the table under it, and a strip of
 *  a fixed height at its foot with the file's problems (their count and the engine's own
 *  words), so a problem appearing or going never moves anything. */
export function CellsWindow({
  name,
  text,
  others = NONE,
  className,
}: {
  name: string
  text: string
  /** The files it imports, as they stand. */
  others?: { name: string; text: string }[]
  className?: string
}) {
  const now = useRead(name, text, others)
  // A break in the grammar first (it names its column), then the cells' problems.
  const said = [...now.broken.map((b) => b.why), ...now.problems.map((p) => p.why)]
  return (
    <AppWindow
      data-slot="cells-window"
      title={`${name} · ${PRODUCT.name}`}
      scheme="dark"
      className={cn("rounded-[10px] [--ag-table-ground:var(--ag-app-window-ground)]", className)}
    >
      <div className="flex h-full flex-col">
        <div className="min-h-0 flex-1 overflow-hidden text-[11px]">
          <CellsTable name={name} text={text} others={others} label={name} className="h-full" />
        </div>
        <div
          data-slot="cells-window-problems"
          className="flex h-6 shrink-0 items-center gap-2 overflow-hidden border-t border-white/8 px-3 font-mono text-[10px] whitespace-nowrap"
        >
          {/* The count in a slot of its own width, whatever it says: nothing beside it moves. */}
          <span className={cn("w-[10ch] shrink-0 tabular-nums", said.length ? "text-haze" : "text-edge/80")}>
            {said.length ? `${said.length} ${said.length === 1 ? "problem" : "problems"}` : "no problems"}
          </span>
          <span className="min-w-0 truncate text-white/60">{said.join(" · ")}</span>
        </div>
      </div>
    </AppWindow>
  )
}
