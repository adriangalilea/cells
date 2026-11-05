// THE STORY, as data: each step its words, the files it shows, and what the reader is
// shown them becoming. The page draws it (components/story.tsx); a walkthrough player
// can draw the same list, so the words and files never live in a component.
//
// Every file here is read by the real engine (lib/engine) when the page builds and again
// in the reader's browser as they edit; `bun scripts/examples.ts` reads each one and
// asserts what the step claims (its problems, its computed values), so a step can never
// say something the engine does not do.

import { PRODUCT } from "@/lib/product"

const ext = PRODUCT.extension

/** A file a step shows: its name and its text. The first is the one the reader edits. */
export interface StoryFile {
  name: string
  text: string
}

/** What a step's files become, on the right of its text. */
export type StoryView =
  /** The first file as a table: its values typed, its broken cells named. */
  | { kind: "table" }
  /** The first file read with the others as its imports: the table, its references
   *  checked across files. */
  | { kind: "linked" }
  /** A table a script fills: the rows its last run stored and its runs, recorded. */
  | { kind: "run" }
  /** A question asked of every file at once, answered from their typed columns. */
  | { kind: "query" }

export interface StoryStep {
  id: string
  /** The headline: its lead bright, its rest quieter. */
  lead: string
  rest: string
  /** Why it matters, one sentence. */
  why: string
  files: StoryFile[]
  view: StoryView
  /** What the reader can try, said as an action. */
  try?: string
  /** What the engine must find in the first file as the page ships it: each problem's
   *  `row, column: reason` (checked by scripts/examples.ts). */
  expect: { problems: string[]; values?: Record<string, (string | number | null)[]> }
}

const ACCOUNTS = `account:text!,kind:assets|expenses|income
bank,assets
salary,income
rent,expenses
food,expenses
`

export const STORY: StoryStep[] = [
  {
    id: "csv",
    lead: "A CSV knows nothing.",
    rest: "Every value is a string that hopes.",
    why: "96.7x reads as fine, because nothing in the file says what cost is.",
    files: [
      {
        name: "groceries.csv",
        text: `item,cost,qty,paid
fuel,84.20,1,yes
groceries,96.7x,2,no
fuel,12.00,0,maybe
`,
      },
    ],
    view: { kind: "table" },
    expect: { problems: [] },
  },
  {
    id: "types",
    lead: "Say what each column is.",
    rest: "In the header, where it cannot drift.",
    why: "The schema is the first line of the file, so every reader checks the same rules and names what breaks them.",
    try: "Fix the cells: 96.75, 1, no.",
    files: [
      {
        name: `groceries${ext}`,
        text: `item:text!,cost:EUR>=0,qty:int>=1,paid:yes/no
fuel,84.20,1,yes
groceries,96.7x,2,no
fuel,12.00,0,maybe
`,
      },
    ],
    view: { kind: "table" },
    expect: {
      problems: [
        'row 0, item: "fuel" is the key of row 2 too',
        'row 1, cost: "96.7x" is not a number',
        'row 2, item: "fuel" is the key of row 0 too',
        'row 2, qty: "0" is below 1',
        'row 2, paid: "maybe" is not yes or no',
      ],
    },
  },
  {
    id: "formula",
    lead: "Columns that compute.",
    rest: "The rule beside the column it fills.",
    why: "The field stays empty and the header holds the formula, so a value is never stored twice and never stale.",
    try: "Change a cost and watch total follow.",
    files: [
      {
        name: `groceries${ext}`,
        text: `item:text!,cost:EUR>=0,qty:int>=1,total:EUR=cost * qty
fuel,84.20,1,
groceries,96.75,2,
coffee,2.10,3,
`,
      },
    ],
    view: { kind: "table" },
    expect: { problems: [], values: { total: [84.2, 193.5, 6.3] } },
  },
  {
    id: "cell",
    lead: "A cell can hold its own.",
    rest: "By name, never by coordinate.",
    why: "There is no B7 to point at the wrong row after a line is inserted by hand: a formula names what it reads.",
    try: "Write `=96.75 / 4` instead.",
    files: [
      {
        name: `groceries${ext}`,
        text: `item:text!,cost:EUR>=0,qty:int>=1,total:EUR=cost * qty
fuel,84.20,1,
dinner for three,\`=96.75 / 3\`,3,
`,
      },
    ],
    view: { kind: "table" },
    expect: { problems: [], values: { cost: [84.2, 32.25], total: [84.2, 96.75] } },
  },
  {
    id: "references",
    lead: "Files that know each other.",
    rest: "A folder of them is a database.",
    why: "A column typed by another file's key must name a row that exists there, so a typo is caught the moment it is written.",
    try: "Fix fodo, or add it to accounts.",
    files: [
      {
        name: `money${ext}`,
        text: `---
import accounts from "./accounts${ext}"
---
date:date,amount:EUR>0,from:accounts.account,to:accounts.account,vat:EUR=amount * 21 / 121
2026-11-01,950.00,bank,rent,
2026-11-03,42.50,bank,fodo,
2026-11-04,2400.00,salary,bank,
`,
      },
      { name: `accounts${ext}`, text: ACCOUNTS },
    ],
    view: { kind: "linked" },
    expect: {
      problems: ['row 1, to: "fodo" is no key of accounts'],
      values: { vat: [164.88, 7.38, 416.53] },
    },
  },
  {
    id: "run",
    lead: "Data that fetches itself.",
    rest: "A script fills a table, and the file keeps what it got.",
    why: "Opening a file never runs anything: you see the rows its last run stored, and the run is yours to start.",
    files: [
      {
        name: `rates${ext}`,
        text: `---
eurusd = run every day at 18:00 in Europe/Madrid
\`\`\`ts
export default async function ({ last }) {
  const res = await fetch(\`https://api.exchange.example/usdeur?since=\${last.date}\`)
  return (await res.json()).map(d => ({ date: d.day, rate: String(d.close) }))
}
\`\`\`
\`\`\`cells
date:date!,rate:num>0,run:int?
2026-11-03,0.9287,9
2026-11-04,0.9312,9
2026-11-05,0.9306,12
\`\`\`
\`\`\`cells runs
run:int!,at:datetime,outcome:ok|failed,error:text?
9,2026-11-04T17:00:03Z,ok,
12,2026-11-05T17:00:04Z,ok,
\`\`\`
---
date:date,usd:USD>0,note:text?,eur:EUR=usd * eurusd[date].rate
2026-11-03,180.00,two nights,
2026-11-05,42.50,dinner,
`,
      },
    ],
    view: { kind: "run" },
    expect: { problems: [], values: { eur: [167.17, 39.55] } },
  },
  {
    id: "query",
    lead: "Ask the whole folder.",
    rest: "Typed tables answer questions.",
    why: "Every file is a table whose columns are already numbers, dates and references, so a question is a query, never a parse.",
    files: [
      {
        name: `home/money${ext}`,
        text: `---
import accounts from "../accounts${ext}"
---
date:date,amount:EUR>0,from:accounts.account,to:accounts.account
2026-11-01,950.00,bank,rent
2026-11-12,64.30,bank,food
`,
      },
      {
        name: `travel/money${ext}`,
        text: `---
import accounts from "../accounts${ext}"
---
date:date,amount:EUR>0,from:accounts.account,to:accounts.account
2026-11-03,180.00,bank,rent
2026-11-04,42.50,bank,food
2026-11-28,2400.00,salary,bank
`,
      },
      { name: `accounts${ext}`, text: ACCOUNTS },
    ],
    view: { kind: "query" },
    expect: { problems: [] },
  },
]
