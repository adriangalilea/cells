// THE HERO, as data: ten seconds on a Mac. Helix types a movement into money.cell; the
// Cells window beside it reads the file at every keystroke (vat computed as the amount
// lands, the new account named on its row while it does not exist). `:w`, and the
// terminal's check fails on the saved file, naming the row. The account is added to the
// file's own table, the row is whole, `:w`, and the check passes. One clock for all of it
// (`HERO.clip`), so a player, a playback and a fixed progress draw the same frames.

import type { Clip } from "@/lib/clip"
import { type CodeFile, type CodeStep, codeTimeline, TYPE_MS } from "@/lib/code-session"
import { PRODUCT } from "@/lib/product"
import { parseSession, sessionTimeline } from "@/lib/terminal-session"

const file = `money${PRODUCT.extension}`

const BEFORE = `---
accounts = \`\`\`cells
account:text!
bank
rent
food
\`\`\`
---
date:date,amount:EUR>0,from:accounts.account,to:accounts.account,vat:EUR=amount * 21 / 121
2026-11-01,950.00,bank,rent,
2026-11-03,42.50,bank,food,
`

/** The movement typed at the file's end, and the account that makes it whole. */
const ROW = "2026-11-05,3.80,bank,coffee,"
const ACCOUNT = "coffee"
/** What the engine says of the saved file with the row and no account (`cells check`'s
 *  own words; it prints nothing when nothing is wrong, and exits 0). */
const FAILING = `${file} row 2, to: "${ACCOUNT}" is no key of accounts`
const FIRST = `$ ${PRODUCT.cli} check ${file}\n${FAILING}`
const SECOND = `$ ${PRODUCT.cli} check ${file} && echo ok\nok`

/** The lid opens over this, ms; the edit starts once it has. */
const OPENING = 1600
/** From a save to the terminal's command starting to type. */
const TO_TERMINAL = 150
/** The failure on screen, read, before the hand goes back to the file. */
const READ_MS = 1000
/** The last frame held before the loop starts again. */
const HOLD_MS = 2200

const firstCheck = sessionTimeline(parseSession(FIRST)).total

const FILES: CodeFile[] = [{ path: file, text: BEFORE }]
const STEPS: CodeStep[] = [
  { kind: "open", path: file },
  // At the end of the last row: a new one.
  { kind: "type", line: 11, text: `\n${ROW}`, delay: 400 },
  { kind: "save", delay: 250 },
  // While the check runs and is read: then the account, at the end of the inline
  // table's last row.
  { kind: "type", line: 6, text: `\n${ACCOUNT}`, delay: TO_TERMINAL + firstCheck + READ_MS },
  { kind: "save", delay: 250 },
]

const edit = codeTimeline(FILES, STEPS)
const at = (step: number) => OPENING + (edit.ends[step] as number)
const savedTypo = at(2)
const savedFixed = at(4)

/** The terminal's session: the first check after the first save, the second after the
 *  second, its pause (`@ms`) keeping them apart on the one clock. */
const terminalFrom = savedTypo + TO_TERMINAL
const pause = savedFixed + TO_TERMINAL - (terminalFrom + firstCheck)
const SESSION = `${FIRST}\n@${Math.round(pause)} ${SECOND}`
const session = sessionTimeline(parseSession(SESSION))
const total = terminalFrom + session.total + HOLD_MS

export const HERO = {
  file,
  files: FILES,
  steps: STEPS,
  session: SESSION,
  opening: OPENING,
  /** The whole loop, ms. */
  total,
  /** When the terminal's session starts, and how long it lasts. */
  terminal: { from: terminalFrom, duration: session.total },
  /** The editor's own clock, from when it starts. */
  edit: { from: OPENING, duration: edit.total },
  clip: {
    duration: total,
    chapters: [
      { start: 0, title: "the lid" },
      { start: OPENING, title: "a movement typed" },
      { start: savedTypo, title: `${PRODUCT.cli} check names the row` },
      { start: at(3) - (ACCOUNT.length + 1) * TYPE_MS, title: "the account added" },
      { start: savedFixed, title: `${PRODUCT.cli} check passes` },
    ],
  } satisfies Clip,
  /** The file at three moments, and what the engine must say of each
   *  (scripts/examples.ts asserts it). */
  checkpoints: {
    before: { text: BEFORE, problems: [] as string[] },
    typo: {
      text: BEFORE.replace(/\n$/, `\n${ROW}\n`),
      problems: [`row 2, to: "${ACCOUNT}" is no key of accounts`],
    },
    fixed: {
      text: BEFORE.replace("food\n```", `food\n${ACCOUNT}\n\`\`\``).replace(/\n$/, `\n${ROW}\n`),
      problems: [] as string[],
    },
  },
  /** What `cells check` prints for the typo, which the terminal shows. */
  failing: FAILING,
}
