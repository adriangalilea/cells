// THE HERO, as data: ten seconds on a Mac. Helix types a movement into money.cell; the
// Cells window beside it reads the file at every keystroke (vat computed as the amount
// lands, the new account named on its row while it does not exist); the account is added
// to the file's own table and the row is whole; `:w`; the terminal runs the check, once
// failing on the typed file, once clean. One clock for all of it (`HERO.clip`), so a
// player, a playback and a fixed progress draw the same frames.

import type { Clip } from "@/lib/clip"
import { type CodeFile, type CodeStep, codeTimeline } from "@/lib/code-session"
import { PRODUCT } from "@/lib/product"

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

const FILES: CodeFile[] = [{ path: file, text: BEFORE }]
const STEPS: CodeStep[] = [
  { kind: "open", path: file },
  // At the end of the last row: a new one.
  { kind: "type", line: 11, text: `\n${ROW}`, delay: 500 },
  // The account, at the end of the inline table's last row.
  { kind: "type", line: 6, text: `\n${ACCOUNT}`, delay: 900 },
  { kind: "save", delay: 400 },
]

const edit = codeTimeline(FILES, STEPS)
/** When the row is typed whole, and when the account has been added. */
const typed = edit.ends[1] as number
const fixed = edit.ends[2] as number
const saved = edit.ends[3] as number

/** The terminal's two checks: the file as it stood with the row and no account, then
 *  after `:w`. The engine's own words (`cells check` prints nothing when nothing is
 *  wrong, and exits 0). */
const CHECK_FAILING = `${file} row 2, to: "${ACCOUNT}" is no key of accounts`
const SESSION = `$ ${PRODUCT.cli} check ${file}
${CHECK_FAILING}
$ ${PRODUCT.cli} check ${file} && echo ok
ok`

const OPENING = 1600
const total = OPENING + saved + 2600

export const HERO = {
  file,
  files: FILES,
  steps: STEPS,
  session: SESSION,
  /** The lid opens over the first `OPENING` ms; the edit starts once it has. */
  opening: OPENING,
  /** The whole loop, ms. */
  total,
  /** When the terminal starts its session, and how long it takes. */
  terminal: { from: OPENING + saved + 200, to: total - 400 },
  /** The editor's own clock, from when it starts. */
  edit: { from: OPENING, duration: edit.total },
  clip: {
    duration: total,
    chapters: [
      { start: 0, title: "the lid" },
      { start: OPENING, title: "a movement typed" },
      { start: OPENING + typed, title: "an account that does not exist" },
      { start: OPENING + fixed, title: "the account added" },
      { start: OPENING + saved, title: `${PRODUCT.cli} check` },
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
}
