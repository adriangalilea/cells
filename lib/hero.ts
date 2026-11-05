// THE HERO, as data: ten seconds or so on a Mac, the format revealed a step at a time.
// accounts.cell is open first: a plain list. Then money.cell, which imports it, starts as
// a plain table; a type is typed into its header, then a reference to accounts, then a
// formula, each changing what the Cells window beside it reads. A movement to an account
// that does not exist is typed and named on its row; `:w`, and the terminal's check names
// it too. Back in accounts.cell the account is added, `:w`, and the check passes. One
// clock for all of it (`HERO.clip`), so a player, a playback and a fixed progress draw the
// same frames.

import type { Clip } from "@/lib/clip"
import { type CodeFile, type CodeStep, type CodeTimeline, codeTimeline, TYPE_MS } from "@/lib/code-session"
import { PRODUCT } from "@/lib/product"
import { parseSession, sessionTimeline } from "@/lib/terminal-session"

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
const FAILING = `${money} row 2, to: "${ACCOUNT}" is no key of accounts`
const FIRST = `$ ${PRODUCT.cli} check ${money}\n${FAILING}`
const SECOND = `$ ${PRODUCT.cli} check ${money} && echo ok\nok`

/** The lid opens over this, ms; the edit starts once it has. */
const OPENING = 1600
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
const STEPS: CodeStep[] = [
  { kind: "open", path: accounts },
  { kind: "open", path: money, delay: 900 },
  // The header, a part at a time: a type, a reference, a formula.
  { kind: "type", line: HEADER, column: "date,amount".length + 1, text: AMOUNT_TYPE, delay: 700 },
  { kind: "type", line: HEADER, text: TO_TYPE, delay: 700 },
  { kind: "type", line: HEADER, text: FORMULA, delay: 700 },
  // A movement to an account accounts.cell does not have.
  { kind: "type", line: 6, text: `\n${ROW}`, delay: 700 },
  { kind: "save", delay: 250 },
  // While the check runs and is read: then accounts.cell, and the account.
  { kind: "open", path: accounts, delay: TO_TERMINAL + firstCheck + READ_MS },
  { kind: "type", line: 4, text: `\n${ACCOUNT}`, delay: 300 },
  { kind: "save", delay: 250 },
  // money.cell back in front, whole: the file the page goes on with.
  { kind: "open", path: money, delay: 900 },
]

const edit = codeTimeline(FILES, STEPS)
const at = (step: number) => OPENING + (edit.ends[step] as number)
const savedTypo = at(6)
const savedFixed = at(9)

/** The terminal's session: the first check after the first save, the second after the
 *  second, its pause (`@ms`) keeping them apart on the one clock. */
const terminalFrom = savedTypo + TO_TERMINAL
const pause = savedFixed + TO_TERMINAL - (terminalFrom + firstCheck)
const SESSION = `${FIRST}\n@${Math.round(pause)} ${SECOND}`
const session = sessionTimeline(parseSession(SESSION))
const total = Math.max(terminalFrom + session.total, OPENING + edit.total) + HOLD_MS

/** A file's text at an instant of the edit (ms on the edit's own clock): the characters
 *  typed by then, whether or not the file is in front. */
export function textAt(timeline: CodeTimeline, path: string, ms: number): string {
  const chars = timeline.chars[path]
  if (!chars) throw new Error(`hero: no file ${path} in the edit`)
  let out = ""
  for (const c of chars) if (c.born <= ms) out += c.ch
  return out
}

const typo = MONEY.replace("date,amount,to", `date,amount${AMOUNT_TYPE},to${TO_TYPE}${FORMULA}`).replace(/\n$/, `\n${ROW}\n`)

export const HERO = {
  accounts,
  money,
  files: FILES,
  steps: STEPS,
  session: SESSION,
  opening: OPENING,
  /** The whole loop, ms. */
  total,
  terminal: { from: terminalFrom, duration: session.total },
  edit: { from: OPENING, duration: edit.total },
  clip: {
    duration: total,
    chapters: [
      { start: 0, title: "the lid" },
      { start: OPENING, title: `${accounts}, a list` },
      { start: at(1), title: `${money}, a plain table` },
      { start: at(2), title: "a type, a reference, a formula" },
      { start: at(5), title: "an account that does not exist" },
      { start: savedTypo, title: `${PRODUCT.cli} check names the row` },
      { start: at(8) - (ACCOUNT.length + 1) * TYPE_MS, title: "the account added" },
      { start: savedFixed, title: `${PRODUCT.cli} check passes` },
    ],
  } satisfies Clip,
  /** The files at three moments, and what the engine must say of money.cell read with
   *  accounts.cell (scripts/examples.ts asserts it). */
  checkpoints: {
    plain: { money: MONEY, accounts: ACCOUNTS, problems: [] as string[] },
    typo: { money: typo, accounts: ACCOUNTS, problems: [`row 2, to: "${ACCOUNT}" is no key of accounts`] },
    fixed: { money: typo, accounts: ACCOUNTS.replace(/\n$/, `\n${ACCOUNT}\n`), problems: [] as string[] },
  },
  /** What `cells check` prints for the typo, which the terminal shows. */
  failing: FAILING,
}
