"use client"

// A CHART AS THE APP'S CHART VIEW DRAWS IT, from the engine's summary of it (the wasm's
// `doc.summary`, the one `cells chart` and Cells.app draw from): its title, its legend
// (each series named in its shade with its kind's mark), its scale (the summary's, the
// one every view draws), gridlines at each step, its marks (stacked bars, a line, layers on one
// plot) and its places named under them. An SVG on the window's dark ground, sized by
// its box.

import { CELLS_COLUMNS } from "@/lib/cells-code"
import type { ChartSummary, Shade } from "@/lib/engine/cells.ts"

/** A series' shade as the dark window draws it: a tint in the tokens' colour, a hex as
 *  given, a column's own colour, the quiet gray. */
function colourOf(s: Shade): string {
  if ("tint" in s) return s.tint.startsWith("#") ? s.tint : `var(--tint-${s.tint})`
  if ("hex" in s) return s.hex
  if ("column" in s) return (CELLS_COLUMNS[s.column % CELLS_COLUMNS.length] as (typeof CELLS_COLUMNS)[number]).dark
  return "oklch(60% 0 0)"
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]

/** A place as its axis names it: a date's month and year, anything else as written. */
const placeLabel = (axis: ChartSummary["axis"], place: string) => {
  if (axis !== "date") return place
  const [y, m] = place.split("-")
  return `${MONTHS[Number(m) - 1] ?? m} ${y?.slice(2)}`
}

/** The view's own units: the SVG's box, and the room its parts take. */
const W = 480
const H = 230
const TITLE_Y = 16
const LEGEND_Y = 36
const PLOT = { top: 52, bottom: H - 22, left: 44, right: W - 8 }

export function ChartView({ summary, className }: { summary: ChartSummary; className?: string }) {
  const parts = summary.layers.length ? summary.layers : [summary]
  const places = parts[0]?.places ?? []
  if (!summary.scale) throw new Error(`chart view: ${summary.title} is a ${summary.kind} drawn alone, on no scale`)
  const { lo, hi, step } = summary.scale
  const y = (v: number) => PLOT.bottom - ((v - lo) / (hi - lo)) * (PLOT.bottom - PLOT.top)
  const band = (PLOT.right - PLOT.left) / Math.max(1, places.length)
  const mid = (i: number) => PLOT.left + band * (i + 0.5)
  const ticks: number[] = []
  for (let v = lo; v <= hi + step / 2; v += step) ticks.push(v)
  const legend = parts.flatMap((p) => p.series.map((s) => ({ kind: p.kind, name: s.name || p.title, colour: colourOf(s.shade) })))
  let at = 0
  return (
    <svg viewBox={`0 0 ${W} ${H}`} className={className} role="img" aria-label={summary.title}>
      <text x={0} y={TITLE_Y} fontSize={12} fontWeight={600} fill="rgb(255 255 255 / 0.85)">
        {summary.title}
      </text>
      {legend.length > 1 &&
        legend.map((l) => {
          const x = at
          at += 22 + l.name.length * 6.2
          return (
            <g key={`${l.kind}-${l.name}`} transform={`translate(${x} ${LEGEND_Y})`}>
              {l.kind === "line" ? (
                <line x1={0} x2={10} y1={-3.5} y2={-3.5} stroke={l.colour} strokeWidth={2} />
              ) : (
                <rect x={0} y={-8} width={8} height={8} rx={1.5} fill={l.colour} />
              )}
              <text x={14} y={0} fontSize={10} fill={l.colour}>
                {l.name}
              </text>
            </g>
          )
        })}
      {ticks.map((v) => (
        <g key={v}>
          <line x1={PLOT.left} x2={PLOT.right} y1={y(v)} y2={y(v)} stroke="rgb(255 255 255 / 0.08)" />
          <text x={PLOT.left - 6} y={y(v) + 3} fontSize={9} textAnchor="end" fill="rgb(255 255 255 / 0.45)">
            {v}
          </text>
        </g>
      ))}
      {parts.map((p, k) =>
        p.kind === "line" ? (
          <polyline
            key={k}
            fill="none"
            stroke={colourOf((p.series[0] as ChartSummary["series"][number]).shade)}
            strokeWidth={2}
            points={p.values.map((row, i) => `${mid(i)},${y(Number(row[0] ?? 0))}`).join(" ")}
          />
        ) : (
          <g key={k}>
            {p.values.map((row, i) => {
              let base = 0
              return row.map((v, j) => {
                const n = v === null ? 0 : Number(v)
                const top = base + n
                const rect = (
                  <rect
                    key={`${i}-${j}`}
                    x={mid(i) - band * 0.3}
                    width={band * 0.6}
                    y={y(top)}
                    height={Math.max(0, y(base) - y(top))}
                    fill={colourOf((p.series[j] as ChartSummary["series"][number]).shade)}
                  />
                )
                base = top
                return rect
              })
            })}
          </g>
        ),
      )}
      {places.map((place, i) => (
        <text key={place} x={mid(i)} y={H - 6} fontSize={9} textAnchor="middle" fill="rgb(255 255 255 / 0.45)">
          {placeLabel(summary.axis === "date" || parts[0]?.axis === "date" ? "date" : summary.axis, place)}
        </text>
      ))}
    </svg>
  )
}
