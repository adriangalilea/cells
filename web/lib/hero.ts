// THE HERO, as data: someone at work in Cells on a Mac, two windows of the app open:
// money.cells, the month's movements, and accounts.cells, the accounts they go to. A
// movement is typed into a new row, cell by cell, each cell written by the engine as the
// app writes it: the VAT is there as the amount lands. It is paid to coffee, an account
// accounts.cells does not have, and the cell is flagged where it is. In accounts.cells
// coffee is added, and the flag in money.cells is gone: the two files are one database.
// One clock (`playable`), ui's table-session, so a player, a playback and a fixed
// progress draw the same frames; the money window is the one that leaves the screen for
// the page (components/hero).

import type { Playable } from "@/lib/playable"
import { PRODUCT } from "@/lib/product"
import { type TableFile, type TableStep, type TableTimeline, tableFrameAt, tableTimeline } from "@/lib/table-session"
import { type Engine, writeCell } from "@/lib/view"

const accounts = `accounts${PRODUCT.extension}`
const money = `money${PRODUCT.extension}`

const ACCOUNTS = `account: text!
bank
rent
food
`
const MONEY = `---
import accounts from "./${accounts}"
---
date,amount: EUR>0,to: accounts.account?,vat: EUR = amount * 21 / 121
2026-11-01,950.00,rent,164.88=
2026-11-03,42.50,food,7.38=
`
/** The movement typed, a cell at a time, into the row after the last; and the account it
 *  names, which accounts.cells is given. */
const ROW = 2
const MOVEMENT = { date: "2026-11-05", amount: "3.80", to: "coffee" } as const
const ACCOUNT_ROW = 3

/** The windows as they are found, ms, before the hand moves: a beat to see both. */
const OPENING = 900
/** A cell written, looked at before the next is chosen. */
const LOOK_MS = 700
/** The flag read before the hand goes to the other window. */
const READ_MS = 1300
/** The last frame held before the loop starts again. */
const HOLD_MS = 2600

const FILES: TableFile[] = [
  { path: money, text: MONEY },
  { path: accounts, text: ACCOUNTS },
]

// The amount first: the row it makes has its VAT at once, and every other cell may wait.
const STEPS: TableStep[] = [
  { kind: "select", row: ROW, column: "amount", delay: OPENING },
  { kind: "type", text: MOVEMENT.amount },
  { kind: "select", row: ROW, column: "date", delay: LOOK_MS },
  { kind: "type", text: MOVEMENT.date },
  { kind: "select", row: ROW, column: "to", delay: 200 },
  { kind: "type", text: MOVEMENT.to },
  { kind: "open", path: accounts, delay: READ_MS },
  { kind: "select", row: ACCOUNT_ROW, column: "account", delay: 200 },
  { kind: "type", text: MOVEMENT.to },
  { kind: "open", path: money, delay: LOOK_MS },
]

export interface Hero {
  money: string
  accounts: string
  files: TableFile[]
  timeline: TableTimeline
  /** The whole loop, ms: the hand's session, then the last frame held. */
  total: number
  playable: Playable
  /** The files at the moments the hero claims something of, for scripts/examples.ts. */
  checkpoints: Record<"vat" | "flagged" | "fixed", { money: string; accounts: string; problems: string[] }>
}

const made = new WeakMap<Engine, Hero>()

/** The hero, every commit written by the engine: made once. */
export function hero(engine: Engine): Hero {
  const had = made.get(engine)
  if (had) return had
  const timeline = tableTimeline(FILES, STEPS, (file, cell, value, others) => writeCell(engine, file, cell, value, others))
  // Each checkpoint the instant its step's commit has landed.
  const after = (step: number) => {
    const f = tableFrameAt(timeline, (timeline.plans[step]?.end as number) + 1)
    return { money: f.texts[money] as string, accounts: f.texts[accounts] as string }
  }
  const h: Hero = {
    money,
    accounts,
    files: FILES,
    timeline,
    total: timeline.total + HOLD_MS,
    playable: { duration: timeline.total + HOLD_MS, chapters: timeline.playable.chapters },
    checkpoints: {
      vat: { ...after(1), problems: [] },
      flagged: { ...after(5), problems: [`row 3, to: "${MOVEMENT.to}" is no key of accounts`] },
      fixed: { ...after(8), problems: [] },
    },
  }
  made.set(engine, h)
  return h
}
