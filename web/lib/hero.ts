// THE HERO, as data: someone at work on a Mac, one file open twice. spending.cells, six
// months of a household's spending, its four charts and the month's budgets declared in its
// prelude, is open in Cells on the left, and charted in a terminal on the right by `cells
// chart spending.cells 1 4 --watch`, which draws the file again each time it changes. A
// dinner is typed into the app with a zero too many, and the terminal draws May over its
// budget, in both charts; typed again as it was, May is back under. One clock (ui's
// table-session, every commit the engine's, `playable`), so a player, a playback and a
// fixed progress draw the same frames: the app's window draws the file at the moment, the
// terminal the screen the real `cells chart` drew for that file (lib/hero-screens, made by
// `pnpm charts` from these texts, never drawn by hand). The Cells window is the one that
// leaves the screen for the page (components/hero). scripts/examples.ts asserts what it
// claims.

import type { Playable } from "@/lib/playable"
import { PRODUCT } from "@/lib/product"
import { type TableFile, type TableStep, type TableTimeline, tableFrameAt, tableTimeline } from "@/lib/table-session"
import { type Engine, writeCell } from "@/lib/view"

export const SPENDING_PATH = `spending${PRODUCT.extension}`

/** The charts the terminal holds: the first the file declares and the last. */
export const WATCHED_CHARTS = [1, 4] as const
export const WATCH_COMMAND = `${PRODUCT.cli} chart ${SPENDING_PATH} ${WATCHED_CHARTS.join(" ")} --watch`

/** The file: its types and its charts first in the prelude, where the app's prelude panel
 *  shows them, then the budgets the last chart reads. */
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

/** The dinner: its row (a data row, from 0), what it holds, and what the hand types. */
export const DINNER = { row: 4, was: "96.00", typo: "960.00" } as const

/** The windows as they are found, ms, before the hand moves: a beat to see both. */
const OPENING = 1200
/** The redrawn charts looked at before the cell is chosen again. */
const READ_MS = 2600
/** The last frame held before the loop starts again. */
const HOLD_MS = 2400

const STEPS: TableStep[] = [
  { kind: "select", row: DINNER.row, column: "amount", delay: OPENING },
  { kind: "type", text: DINNER.typo },
  { kind: "select", row: DINNER.row, column: "amount", delay: READ_MS },
  { kind: "type", text: DINNER.was },
]

export interface Hero {
  timeline: TableTimeline
  /** The whole loop, ms: the hand's session, then the last frame held. */
  total: number
  playable: Playable
  /** Every text the file has on the clock, in order, each once: the terminal's frames. */
  texts: string[]
  /** The file as found, with the typo, and typed back: what scripts/examples.ts asserts. */
  checkpoints: Record<"found" | "typo" | "fixed", string>
}

const made = new WeakMap<Engine, Hero>()

/** The hero, every commit written by the engine: made once. */
export function hero(engine: Engine): Hero {
  const had = made.get(engine)
  if (had) return had
  const files: TableFile[] = [{ path: SPENDING_PATH, text: SPENDING_TEXT }]
  const timeline = tableTimeline(files, STEPS, (file, cell, value, others) => writeCell(engine, file, cell, value, others))
  const after = (step: number) => tableFrameAt(timeline, (timeline.plans[step]?.end as number) + 1).texts[SPENDING_PATH] as string
  const committed = timeline.plans.flatMap((p) => (p.texts ? [p.texts[SPENDING_PATH] as string] : []))
  const h: Hero = {
    timeline,
    total: timeline.total + HOLD_MS,
    playable: { duration: timeline.total + HOLD_MS, chapters: timeline.playable.chapters },
    texts: [...new Set([SPENDING_TEXT, ...committed])],
    checkpoints: { found: SPENDING_TEXT, typo: after(1), fixed: after(3) },
  }
  made.set(engine, h)
  return h
}
