"use client"

// THE CELLS APP'S WINDOW on the hero's Mac: a file read by the engine at the moment drawn,
// as the app shows it (ui's table: typed columns, computed cells lit by the edge light,
// every broken cell marked with the engine's reason). The window frame is the site's:
// ui draws menu-bar apps and editors, no plain app window (for ui: a `window` item, a
// title bar and traffic lights around any content, would replace this).

import * as React from "react"
import { Table, type TableColumn } from "@/components/ui/table"
import { useEngine } from "@/components/use-engine"
import { readFiles, type ViewCell } from "@/lib/cells"
import { PRODUCT } from "@/lib/product"
import { cn } from "@/lib/utils"

type Row = { id: string; cells: ViewCell[] }

/** A file as a table: its columns, its rows, its problems by cell. */
export function CellsTable({
  name,
  text,
  others = [],
  label,
  className,
}: {
  name: string
  text: string
  others?: { name: string; text: string }[]
  label: string
  className?: string
}) {
  const engine = useEngine()
  const read = React.useMemo(() => readFiles(engine, [{ name, text }, ...others]), [engine, name, text, others])
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
          return (
            <span
              className={cn("truncate font-mono slashed-zero", computed && "text-edge [text-shadow:0_0_12px_color-mix(in_oklch,var(--edge)_45%,transparent)]")}
              title={computed ? (cell.origin === "formula" ? `${c.name} = ${c.formula}` : cell.text) : undefined}
            >
              {cell.shown}
            </span>
          )
        },
      })),
    [read],
  )
  if (read.error)
    return <p className={cn("p-3 font-mono text-[11px] text-haze", className)}>{read.error}</p>
  return (
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
  )
}

const r0Numeric = (rows: ViewCell[][], k: number) => rows.some((r) => r[k]?.numeric)

/** The app's window: a title bar with the file's name, the table under it. */
export function CellsWindow({ name, text, className }: { name: string; text: string; className?: string }) {
  return (
    <div
      data-slot="cells-window"
      className={cn(
        "flex flex-col overflow-hidden rounded-[10px] border border-white/10 bg-[#101318] text-white shadow-[0_24px_60px_-12px_rgb(0_0_0/0.7)] [--ag-table-ground:#101318]",
        className,
      )}
    >
      <div className="relative flex h-7 shrink-0 items-center gap-1.5 border-b border-white/8 px-3">
        <span className="size-[11px] rounded-full bg-[#ff5f57]" />
        <span className="size-[11px] rounded-full bg-[#febc2e]" />
        <span className="size-[11px] rounded-full bg-[#28c840]" />
        <span className="absolute inset-x-0 text-center font-sans text-[11px] text-white/55">
          {name} · {PRODUCT.name}
        </span>
      </div>
      <div className="dark min-h-0 flex-1 overflow-hidden text-[11px]">
        <CellsTable name={name} text={text} label={name} className="h-full" />
      </div>
    </div>
  )
}
