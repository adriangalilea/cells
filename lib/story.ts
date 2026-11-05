// THE STORY, as data: ONE file, built act by act. It starts as a short CSV; each act is one
// idea, played as one continuous show on the file as the act before left it, and only
// then told in one line. An act is a script of beats: a pause to read, the name retyped,
// keys typed, a mark moved onto what the act is about, the table arriving, a column or a
// cell of it lit. Nothing is marked before it happens. components/story.tsx plays each
// act once in view; afterwards the file is the reader's to edit.
//
// Every state of the file is read by the real engine when the page builds and again in the
// reader's browser; `bun scripts/examples.ts` plays every act and asserts what it claims
// (its problems, its computed values), so an act can never show something the engine
// does not do.

import { PRODUCT } from "@/lib/product"

const ext = PRODUCT.extension

/** A file: its name and its text. */
export interface StoryFile {
  name: string
  text: string
}

/** Keys typed into the file: after the first place `after` occurs in the text as it
 *  stands, `remove` characters taken away, then `insert` typed. */
export interface StoryEdit {
  after: string
  insert: string
  remove?: number
}

/** What the table lights: a column, or one cell of it. */
export interface StoryLit {
  column: string
  /** The data row, from 0; absent, the whole column. */
  row?: number
}

/** One beat of an act's show. */
export type StoryBeat =
  /** The file as it stands, looked at, ms. */
  | { see: number }
  /** The file's name retyped. */
  | { rename: string }
  /** Keys typed, then `mark` (in the text they leave) marked. */
  | { type: StoryEdit[]; mark: string }
  /** `mark` marked where it stands. */
  | { mark: string }
  /** The table arrives beside the file. */
  | { table: true }
  /** The table lights a column or a cell, and `mark` in the file with it. */
  | { light: StoryLit; mark?: string }

export interface StoryAct {
  id: string
  /** What is told once it has been shown: one line, its lead bright, its rest quieter. */
  lead: string
  rest: string
  /** The file as the act finds it. */
  name: string
  text: string
  /** The files it imports, each a tab beside it; `imports` the ones this act brings. */
  others: StoryFile[]
  /** Whether the table is beside the file as the act starts. */
  table: boolean
  beats: StoryBeat[]
  /** The file as the act leaves it. */
  end: StoryFile
  /** What the engine must find in the file as the act leaves it: each problem's
   *  `row, column: reason`, and computed values (checked by scripts/examples.ts). */
  expect: { problems: string[]; values?: Record<string, (string | number | null)[]> }
}

/** `text` with `edits` typed, whole. */
export function edited(text: string, edits: readonly StoryEdit[]): string {
  let out = text
  for (const e of edits) {
    const i = out.indexOf(e.after)
    if (i < 0) throw new Error(`story: "${e.after}" is not in the text it is typed into`)
    const at = i + e.after.length
    out = out.slice(0, at) + e.insert + out.slice(at + (e.remove ?? 0))
  }
  return out
}

/** One act as written: its beats and what is said. The rest is the act before's. */
interface Script {
  id: string
  lead: string
  rest: string
  /** Files imported from this act on. */
  imports?: StoryFile[]
  beats: StoryBeat[]
  expect: StoryAct["expect"]
}

/** The scripts played in order on one file: each starts where the one before ended. */
function chain(name: string, text: string, scripts: Script[]): StoryAct[] {
  const out: StoryAct[] = []
  let file = { name, text }
  let others: StoryFile[] = []
  let table = false
  for (const s of scripts) {
    others = [...others, ...(s.imports ?? [])]
    let end = file
    let tableAfter: boolean = table
    for (const b of s.beats) {
      if ("rename" in b) end = { ...end, name: b.rename }
      if ("type" in b) end = { ...end, text: edited(end.text, b.type) }
      if ("table" in b) tableAfter = true
    }
    out.push({ id: s.id, lead: s.lead, rest: s.rest, name: file.name, text: file.text, others, table, beats: s.beats, end, expect: s.expect })
    file = end
    table = tableAfter
  }
  return out
}

const ACCOUNTS: StoryFile = {
  name: `accounts${ext}`,
  text: `account:text!,kind:needs|treats
car,needs
food,needs
cafe,treats
`,
}

export const STORY: StoryAct[] = chain(
  "groceries.csv",
  `item,on,cost,qty
fuel,2026-11-01,84.20,1
groceries,2026-11-02,96.75,2
coffee,2026-11-03,2.10,3
`,
  [
    {
      id: "csv",
      lead: "Nothing objected.",
      rest: "A CSV has no types, so 96.7x is as good as 96.75.",
      beats: [{ see: 1400 }, { type: [{ after: "2026-11-02,96.7", remove: 1, insert: "x" }], mark: "96.7x" }, { see: 1000 }],
      expect: { problems: [] },
    },
    {
      id: "types",
      lead: "Typed as euros, it objects.",
      rest: "The type is written in the header, so it travels with the file.",
      beats: [
        { see: 1000 },
        { rename: `groceries${ext}` },
        { type: [{ after: "item,on,cost", insert: ":EUR>=0" }], mark: "cost:EUR>=0" },
        { mark: "96.7x" },
      ],
      expect: { problems: ['row 2, cost: "96.7x" is not a number'] },
    },
    {
      id: "table",
      lead: "The table is the same file.",
      rest: `Nothing is imported or synced: fix the file and ${PRODUCT.name} follows.`,
      beats: [
        { see: 1000 },
        { table: true },
        { light: { column: "cost", row: 1 }, mark: "96.7x" },
        { type: [{ after: "2026-11-02,96.7", remove: 1, insert: "5" }], mark: "96.75" },
      ],
      expect: { problems: [], values: { cost: [84.2, 96.75, 2.1] } },
    },
    {
      id: "formula",
      lead: "total is never stored.",
      rest: "The header computes it for every row.",
      beats: [{ see: 1000 }, { type: [{ after: "qty", insert: ",total:EUR=cost * qty" }], mark: ",total:EUR=cost * qty" }, { light: { column: "total" } }],
      expect: { problems: [], values: { total: [84.2, 193.5, 6.3] } },
    },
    {
      id: "running",
      lead: "running follows the dates.",
      rest: "Shuffle the rows and each running total stays the same.",
      beats: [
        { see: 1000 },
        { type: [{ after: "total:EUR=cost * qty", insert: ",running:EUR=sum(total) over (order by on)" }], mark: ",running:EUR=sum(total) over (order by on)" },
        { light: { column: "running" } },
      ],
      expect: { problems: [], values: { running: [84.2, 277.7, 284] } },
    },
    {
      id: "cell",
      lead: "A cell can hold its own formula.",
      rest: "The file keeps 6.30 / 3. Every reader shows 2.10.",
      beats: [{ see: 1000 }, { type: [{ after: "2026-11-03,", remove: 4, insert: "`=6.30 / 3`" }], mark: "`=6.30 / 3`" }, { light: { column: "cost", row: 2 } }],
      expect: { problems: [], values: { cost: [84.2, 96.75, 2.1] } },
    },
    {
      id: "references",
      lead: "to must name an account.",
      rest: `Each value is checked against the keys of ${ACCOUNTS.name}, which the file imports.`,
      imports: [ACCOUNTS],
      beats: [
        { see: 1000 },
        {
          type: [
            { after: "", insert: `---\nimport accounts from "./${ACCOUNTS.name}"\n---\n` },
            { after: "qty", insert: ",to:accounts.account" },
            { after: "84.20,1", insert: ",car" },
            { after: "96.75,2", insert: ",food" },
            { after: "3`,3", insert: ",cafe" },
          ],
          mark: "to:accounts.account",
        },
        { light: { column: "to" } },
      ],
      expect: { problems: [], values: { to: ["car", "food", "cafe"] } },
    },
    {
      id: "reads",
      lead: `kind is read from ${ACCOUNTS.name}.`,
      rest: "The formula follows the reference to the row it names.",
      beats: [{ see: 1000 }, { type: [{ after: "over (order by on)", insert: ",kind:text=to.kind" }], mark: ",kind:text=to.kind" }, { light: { column: "kind" } }],
      expect: { problems: [], values: { kind: ["needs", "needs", "treats"] } },
    },
    {
      id: "key",
      lead: "fodo is no account.",
      rest: "Caught as it is typed, and its kind is left empty.",
      beats: [{ see: 1000 }, { type: [{ after: "96.75,2,", remove: 4, insert: "fodo" }], mark: "fodo" }, { light: { column: "to", row: 1 } }],
      expect: { problems: ['row 2, to: "fodo" is no key of accounts'], values: { kind: ["needs", null, "treats"] } },
    },
  ],
)
