// THE HERO, as data: ten seconds or so on a Mac, the format revealed a step at a time.
// accounts.cells is open first: a plain list. Then money.cells, which imports it, starts as
// a plain table; a type is typed into its header, then a reference to accounts, then a
// formula, each changing what the Cells window beside it reads. A movement to an account
// that does not exist is typed and named on its row; `:w`, and the terminal's check names
// it too. Back in accounts.cells the account is added, `:w`, and the check passes. Each
// `:w` writes what the language server's formatting writes (the formula's values stored
// beside their `=`), so the hero is made with the engine (`hero`). One clock for all of
// it (`playable`), so a player, a playback and a fixed progress draw the same frames.

import type { Playable } from "@/lib/playable"
import { type CodeFile, type CodeStep, codeFrameAt, codeTimeline, TYPE_MS } from "@/lib/code-session"
import { PRODUCT } from "@/lib/product"
import { parseSession, sessionTimeline } from "@/lib/terminal-session"
import { type Engine, savedAs } from "@/lib/view"
import { savedAt, textAt } from "@/lib/written"

const accounts = `accounts${PRODUCT.extension}`
const money = `money${PRODUCT.extension}`

const ACCOUNTS = `account:text!
bank
rent
food
`
const MONEY = `---
import accounts from "./${accounts}"
---
date,amount,to
2026-11-01,950.00,rent
2026-11-03,42.50,food
`
/** The header's line, and where each part is typed into it. */
const HEADER = 4
const AMOUNT_TYPE = ":EUR>0"
const TO_TYPE = ":accounts.account"
const FORMULA = ",vat:EUR=amount * 21 / 121"
/** The movement typed at the file's end, and the account that makes it whole. */
const ROW = "2026-11-05,3.80,coffee"
const ACCOUNT = "coffee"

/** What the engine says of the saved files with the row and no account (`cells check`'s
 *  own words; it prints nothing when nothing is wrong, and exits 0). */
const FAILING = `${money} row 3, to: "${ACCOUNT}" is no key of accounts`
const FIRST = `$ ${PRODUCT.cli} check ${money}\n${FAILING}`
const SECOND = `$ ${PRODUCT.cli} check ${money} && echo ok\nok`

/** The screen as it is found, ms, before the edit starts: a beat to see the three windows. */
const OPENING = 600
/** From a save to the terminal's command starting to type. */
const TO_TERMINAL = 150
/** The failure on screen, read, before the hand goes back to the files. */
const READ_MS = 1100
/** The last frame held before the loop starts again. */
const HOLD_MS = 2600

const firstCheck = sessionTimeline(parseSession(FIRST)).total

const FILES: CodeFile[] = [
  { path: accounts, text: ACCOUNTS },
  { path: money, text: MONEY },
]
/** How long a saved change holds before the next is typed: the Cells window reads the file
 *  when it is written, and the change is looked at. */
const LOOK_MS = 900

const STEPS: CodeStep[] = [
  { kind: "open", path: accounts },
  { kind: "open", path: money, delay: 900 },
  // The header, a part at a time, each saved: a type, a reference, a formula.
  { kind: "type", line: HEADER, column: "date,amount".length + 1, text: AMOUNT_TYPE, delay: 700 },
  { kind: "save", delay: 200 },
  { kind: "type", line: HEADER, text: TO_TYPE, delay: LOOK_MS },
  { kind: "save", delay: 200 },
  { kind: "type", line: HEADER, text: FORMULA, delay: LOOK_MS },
  { kind: "save", delay: 200 },
  // A movement to an account accounts.cells does not have.
  { kind: "type", line: 6, text: `\n${ROW}`, delay: LOOK_MS },
  { kind: "save", delay: 200 },
  // While the check runs and is read: then accounts.cells, and the account.
  { kind: "open", path: accounts, delay: TO_TERMINAL + firstCheck + READ_MS },
  { kind: "type", line: 4, text: `\n${ACCOUNT}`, delay: 300 },
  { kind: "save", delay: 250 },
  // money.cells back in front, whole: the file the page goes on with.
  { kind: "open", path: money, delay: 900 },
]

/** The save `steps` end on, written as the language server's formatting writes it: the
 *  file in front as Helix holds it then, against the file as it last saved it, read with
 *  the other as it stands. */
export function saveAfter(engine: Engine, steps: readonly CodeStep[]): CodeStep {
  const t = codeTimeline(FILES, [...steps])
  const path = codeFrameAt(t, t.total).active as string
  const other = path === money ? accounts : money
  const was = { name: path, text: savedAt(t, path, t.total) }
  const now = { name: path, text: textAt(t, path, t.total) }
  return { kind: "save", formatted: savedAs(engine, was, now, [{ name: other, text: textAt(t, other, t.total) }]) }
}

/** STEPS with each save written as the language server's formatting writes it. */
function formatted(engine: Engine): CodeStep[] {
  const steps: CodeStep[] = []
  for (const s of STEPS) steps.push(s.kind === "save" ? { ...saveAfter(engine, steps), delay: s.delay } : s)
  return steps
}

function tell(engine: Engine) {
  const steps = formatted(engine)
  const edit = codeTimeline(FILES, steps)
  const at = (step: number) => OPENING + (edit.ends[step] as number)
  const savedTypo = at(9)
  const savedFixed = at(12)

  /** The terminal's session: the first check after the first save, the second after the
   *  second, its pause (`@ms`) keeping them apart on the one clock. */
  const terminalFrom = savedTypo + TO_TERMINAL
  const pause = savedFixed + TO_TERMINAL - (terminalFrom + firstCheck)
  const session = `${FIRST}\n@${Math.round(pause)} ${SECOND}`
  const played = sessionTimeline(parseSession(session))
  const total = Math.max(terminalFrom + played.total, OPENING + edit.total) + HOLD_MS

  const filesAt = (ms: number) => ({ money: savedAt(edit, money, ms), accounts: savedAt(edit, accounts, ms) })
  return {
    accounts,
    money,
    files: FILES,
    steps,
    /** The edit, timed. */
    timeline: edit,
    session,
    opening: OPENING,
    /** The whole loop, ms. */
    total,
    terminal: { from: terminalFrom, duration: played.total },
    edit: { from: OPENING, duration: edit.total },
    playable: {
      duration: total,
      chapters: [
        { start: 0, title: "the screen" },
        { start: OPENING, title: `${accounts}, a list` },
        { start: at(1), title: `${money}, a plain table` },
        { start: at(1), title: "a type, a reference, a formula" },
        { start: at(7), title: "an account that does not exist" },
        { start: savedTypo, title: `${PRODUCT.cli} check names the row` },
        { start: at(11) - (ACCOUNT.length + 1) * TYPE_MS, title: "the account added" },
        { start: savedFixed, title: `${PRODUCT.cli} check passes` },
      ],
    } satisfies Playable,
    /** The files as written at three moments, and what the engine must say of money.cells
     *  read with accounts.cells (scripts/examples.ts asserts it). */
    checkpoints: {
      plain: { money: MONEY, accounts: ACCOUNTS, problems: [] as string[] },
      typo: { ...filesAt(savedTypo - OPENING), problems: [`row 3, to: "${ACCOUNT}" is no key of accounts`] },
      fixed: { ...filesAt(edit.total), problems: [] as string[] },
    },
    /** What `cells check` prints for the typo, which the terminal shows. */
    failing: FAILING,
  }
}

export type Hero = ReturnType<typeof tell>

const made = new WeakMap<Engine, Hero>()

/** The hero, its saves as the engine writes them: made once. */
export function hero(engine: Engine): Hero {
  const had = made.get(engine)
  if (had) return had
  const h = tell(engine)
  made.set(engine, h)
  return h
}
