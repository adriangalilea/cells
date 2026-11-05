"use client"

// THE CELLS APP'S WINDOW on the hero's Mac: a file read by the engine at the moment drawn,
// as the app shows it (ui's table: typed columns, computed cells lit by the edge light,
// every broken cell marked with the engine's reason). The window frame is the site's:
// ui draws menu-bar apps and editors, no plain app window (for ui: a `window` item, a
// title bar and traffic lights around any content, would replace this).

import * as React from "react"
import { Table, type TableColumn } from "@/components/ui/table"
import { useEngine } from "@/components/use-engine"
import { readFiles, type ViewCell, type ViewTable } from "@/lib/cells"
import { PRODUCT } from "@/lib/product"
import { cn } from "@/lib/utils"

type Row = { id: string; cells: ViewCell[] }

const NONE: { name: string; text: string }[] = []

/** A file read as it stands (`now`), and the table to show (`shown`): the file's own while
 *  it reads; while its grammar does not (a header cell half typed), `held`, the text as it
 *  last read (a still has no past to remember, so the caller says it), else the last
 *  table this view drew. A table never gives way to an error, and nothing moves. */
function useRead(
  name: string,
  text: string,
  others: { name: string; text: string }[],
  held?: string,
): { now: ViewTable; shown: ViewTable } {
  const engine = useEngine()
  const now = React.useMemo(() => readFiles(engine, [{ name, text }, ...others]), [engine, name, text, others])
  const kept = React.useMemo(
    () => (now.error && held !== undefined ? readFiles(engine, [{ name, text: held }, ...others]) : null),
    [engine, now, name, held, others],
  )
  const good = React.useRef<ViewTable | null>(null)
  if (!now.error) good.current = now
  else if (kept && !kept.error) good.current = kept
  const shown = now.error && good.current ? { ...good.current, problems: [] } : now
  return { now, shown }
}

/** A file as a table: its columns, its rows, its problems by cell. */
export function CellsTable({
  name,
  text,
  others = NONE,
  held,
  label,
  className,
}: {
  name: string
  text: string
  others?: { name: string; text: string }[]
  /** The text as it last read, shown while `text` does not. */
  held?: string
  label: string
  className?: string
}) {
  const read = useRead(name, text, others, held).shown
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
  // A file that has never read (nothing to keep on screen): its columns' place, empty.
  if (read.error) return <div className={className} />
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

/** The app's window: a title bar with the file's name, the table under it, and a strip of
 *  a fixed height at its foot with the file's problems (their count and the engine's own
 *  words), so a problem appearing or going never moves anything. */
export function CellsWindow({
  name,
  text,
  others = NONE,
  held,
  className,
}: {
  name: string
  text: string
  /** The files it imports, as they stand. */
  others?: { name: string; text: string }[]
  /** The text as it last read, shown while `text` does not. */
  held?: string
  className?: string
}) {
  const { now } = useRead(name, text, others)
  const said = now.error ? [now.error] : now.problems.map((p) => p.why)
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
        <CellsTable name={name} text={text} others={others} held={held} label={name} className="h-full" />
      </div>
      <div
        data-slot="cells-window-problems"
        className="flex h-[38px] shrink-0 items-start gap-2 overflow-hidden border-t border-white/8 px-3 py-1.5 font-mono text-[10px] leading-[1.45]"
      >
        <span className={cn("shrink-0 tabular-nums", said.length ? "text-haze" : "text-edge/80")}>
          {said.length ? `${said.length} ${said.length === 1 ? "problem" : "problems"}` : "no problems"}
        </span>
        <span className="min-w-0 text-white/60">
          {said.slice(0, 2).map((s) => (
            <span key={s} className="block truncate">
              {s}
            </span>
          ))}
        </span>
      </div>
    </div>
  )
}
