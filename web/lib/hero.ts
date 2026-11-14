// THE HERO, as data: two screens on one desk, one file. spending.cells, six months of a
// household's spending, its four charts and the month's budgets declared in its prelude,
// is open in Cells on a MacBook, its table and its chart view side by side; on a Studio
// Display beside it, a terminal, where `cells chart spending.cells 1 4 --watch` is typed
// and draws the file full screen, again each time it changes. Then small edits in the app,
// a note filled in, an amount nudged, a coffee added at the end of October, each put back
// so the loop ends on the file it began with, and both screens redraw at every one. One
// clock (ui's table-session, every commit the engine's, `playable`), so a player, a
// playback and a fixed progress draw the same frames: the app draws the file at the
// moment, the terminal the screen the real `cells chart` drew for that file at its size
// (lib/hero-screens, made by `pnpm charts` from these texts, never drawn by hand).
// scripts/examples.ts asserts what it claims, every chart's scale the same at every moment
// included.

import { screenRows } from "@/lib/chart-screen"
import type { Playable } from "@/lib/playable"
import { PRODUCT } from "@/lib/product"
import { BAR_EM, TYPE_MS } from "@/lib/terminal-session"
import { type TableFile, type TableStep, type TableTimeline, tableFrameAt, tableTimeline } from "@/lib/table-session"
import { type Engine, writeCell } from "@/lib/view"

export const SPENDING_PATH = `spending${PRODUCT.extension}`

/** The charts the terminal holds and the app's chart view shows: the first the file
 *  declares and the last. */
export const WATCHED_CHARTS = [1, 4] as const
export const WATCH_COMMAND = `${PRODUCT.cli} chart ${SPENDING_PATH} ${WATCHED_CHARTS.join(" ")} --watch`

/** The Cells window on the MacBook's stage (960 by 600, ui macos): 36 from the top (under
 *  the menu bar), 12 from the foot and the sides. */
const INSET = 12
const TOP = 36
export const STAGE = { inset: INSET, top: TOP, window: { width: 960 - 2 * INSET, height: 600 - TOP - INSET } } as const

/** The Studio Display's screen, in the same design pixels (960 wide, 16:9): the terminal
 *  fills it. */
export const DISPLAY = { width: 960, height: 540 } as const

/** The terminals the frames are drawn for: the display's, its columns many enough for a
 *  wide chart and its rows the ones its screen holds at the type that fits them; and a
 *  phone's, where the terminal stands alone under the app, at `cells chart`'s own width. */
const DISPLAY_COLS = 140
export const SCREENS_AT = {
  display: { cols: DISPLAY_COLS, rows: screenRows(DISPLAY_COLS, DISPLAY.width, DISPLAY.height, BAR_EM) },
  phone: { cols: 80, rows: 44 },
} as const
export type ScreenAt = keyof typeof SCREENS_AT

/** The command typed at the terminal's prompt, ms: a key at the terminal's pace, then a
 *  beat before Enter. */
export const TYPED_MS = WATCH_COMMAND.length * TYPE_MS + 400

/** The file: its types and its charts first in its prelude, then the budgets the last
 *  chart reads. */
export const SPENDING_TEXT = `---
type Category = rent#blue|groceries#green|transport#orange|dining#red|fun#purple

\`\`\`chart bars
title: Spending by category
x:     DATE_TRUNC('month', date)
y:     SUM(amount)
color: category
\`\`\`

spent = \`\`\`chart bars
title: Spent
x:     DATE_TRUNC('month', date)
y:     SUM(amount)
\`\`\`
budget = \`\`\`chart line
title: Budget
from:  plan
x:     month
y:     SUM(budget)
\`\`\`
\`\`\`chart
title:  Spent against budget
layers: spent, budget
\`\`\`

plan = \`\`\`cells
month: date,budget: EUR
2026-05-01,2000.00
2026-06-01,2000.00
2026-07-01,2100.00
2026-08-01,2100.00
2026-09-01,2000.00
2026-10-01,2000.00
\`\`\`
---
date: date,category: Category,note: text?,amount: EUR
2026-05-01,rent,May rent,1200.00
2026-05-04,groceries,market,86.40
2026-05-11,groceries,supermarket,142.15
2026-05-14,transport,metro card,54.00
2026-05-18,dining,birthday dinner,96.00
2026-05-25,fun,concert,65.00
2026-06-01,rent,June rent,1200.00
2026-06-03,groceries,market,74.20
2026-06-12,groceries,supermarket,158.90
2026-06-15,transport,train to Valencia,88.50
2026-06-21,dining,,42.30
2026-06-28,fun,books,38.00
2026-07-01,rent,July rent,1200.00
2026-07-06,groceries,supermarket,131.75
2026-07-14,transport,metro card,54.00
2026-07-17,dining,terrace,61.80
2026-07-22,fun,festival,180.00
2026-07-29,groceries,market,92.10
2026-08-01,rent,August rent,1200.00
2026-08-05,groceries,supermarket,119.30
2026-08-09,transport,flight,245.00
2026-08-16,dining,beach bar,73.40
2026-08-20,fun,surf lesson,90.00
2026-08-27,groceries,market,68.60
2026-09-01,rent,September rent,1200.00
2026-09-07,groceries,supermarket,165.25
2026-09-13,transport,metro card,54.00
2026-09-19,dining,,38.90
2026-09-26,groceries,market,81.30
2026-10-01,rent,October rent,1200.00
2026-10-04,groceries,supermarket,149.80
2026-10-10,transport,metro card,54.00
2026-10-15,dining,friends,112.50
2026-10-24,fun,cinema,24.00
2026-10-30,groceries,market,77.45
`

/** The file as the page shows it under the story: its prelude whole, its header and its
 *  first rows, where its charts are seen declared. */
export const SPENDING_SHOWN = (() => {
  const lines = SPENDING_TEXT.split("\n")
  const header = lines.indexOf("---", 1) + 1
  return `${lines.slice(0, header + 4).join("\n")}\n`
})()

/** The month's small edits, in data rows from 0: a note filled in, an amount nudged, a
 *  coffee added at the end; then the coffee deleted, the amount and the note put back, so
 *  the loop ends on the file it began with. Small enough that no chart's scale moves
 *  (scripts/examples.ts checks every frame's axis against the first). */
const TAPAS = { row: 27, note: "tapas" } as const
const FRIENDS = { row: 32, was: "112.50", now: "118.50" } as const
const NEW_ROW = 35
export const COFFEE = { date: "2026-10-31", category: "dining", note: "coffee", amount: "3.80" } as const

/** The command typed and the charts drawn, then a beat to see both screens, ms, before
 *  the hand moves. */
const OPENING = TYPED_MS + 1600
/** Between one edit and the next, ms: the redrawn charts looked at. */
const BEAT = 1800
/** Between the cells of the new row, ms. */
const NEXT_CELL = 350
/** The last frame held before the loop starts again. */
const HOLD_MS = 1600

/** The edits up to the coffee's last cell: the file they leave is the one the deletion
 *  starts from. */
const ADDING: TableStep[] = [
  { kind: "select", row: TAPAS.row, column: "note", delay: OPENING },
  { kind: "type", text: TAPAS.note },
  { kind: "select", row: FRIENDS.row, column: "amount", delay: BEAT },
  { kind: "type", text: FRIENDS.now },
  { kind: "select", row: NEW_ROW, column: "date", delay: BEAT },
  { kind: "type", text: COFFEE.date },
  { kind: "select", row: NEW_ROW, column: "category", delay: NEXT_CELL },
  { kind: "type", text: COFFEE.category },
  { kind: "select", row: NEW_ROW, column: "note", delay: NEXT_CELL },
  { kind: "type", text: COFFEE.note },
  { kind: "select", row: NEW_ROW, column: "amount", delay: NEXT_CELL },
  { kind: "type", text: COFFEE.amount },
]

/** The coffee's row deleted: the file without its last line, as the app writes a row
 *  deleted; then the amount and the note put back. */
const putBack = (withCoffee: string): TableStep[] => {
  const lines = withCoffee.split("\n")
  const last = lines.length - 2
  if (!lines[last]?.startsWith(COFFEE.date)) throw new Error("hero: the coffee is not the file's last row")
  return [
    { kind: "written", path: SPENDING_PATH, text: [...lines.slice(0, last), ""].join("\n"), delay: BEAT + 600 },
    { kind: "select", row: FRIENDS.row, column: "amount", delay: BEAT },
    { kind: "type", text: FRIENDS.was },
    { kind: "select", row: TAPAS.row, column: "note", delay: BEAT },
    { kind: "type", text: "" },
  ]
}

export interface Hero {
  timeline: TableTimeline
  /** The whole loop, ms: the hand's session, then the last frame held. */
  total: number
  playable: Playable
  /** Every text the file has on the clock, in order, each once: the terminal's frames. The
   *  first is the file as found, and the clock ends on it again. */
  texts: string[]
  /** The file as each edit leaves it, in order: what scripts/examples.ts reads. */
  states: string[]
}

const made = new WeakMap<Engine, Hero>()

/** The hero, every commit written by the engine: made once. */
export function hero(engine: Engine): Hero {
  const had = made.get(engine)
  if (had) return had
  const files: TableFile[] = [{ path: SPENDING_PATH, text: SPENDING_TEXT }]
  const write = (file: TableFile, cell: { row: number; column: string }, value: string, others: readonly TableFile[]) =>
    writeCell(engine, file, cell, value, others)
  const adding = tableTimeline(files, ADDING, write)
  const withCoffee = tableFrameAt(adding, adding.total).texts[SPENDING_PATH] as string
  const timeline = tableTimeline(files, [...ADDING, ...putBack(withCoffee)], write)
  const states = timeline.plans.flatMap((p) => (p.texts ? [p.texts[SPENDING_PATH] as string] : []))
  if (states.at(-1) !== SPENDING_TEXT) throw new Error("hero: the loop does not end on the file it began with")
  const h: Hero = {
    timeline,
    total: timeline.total + HOLD_MS,
    playable: { duration: timeline.total + HOLD_MS, chapters: timeline.playable.chapters },
    texts: [...new Set([SPENDING_TEXT, ...states])],
    states,
  }
  made.set(engine, h)
  return h
}
