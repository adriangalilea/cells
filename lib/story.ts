// THE STORY, as data: ONE file, built a step at a time. It starts as a short CSV; each
// step makes one change to it as the step before left it (a value typed, the extension
// renamed, a type, a formula, a reference), and the table that reads it arrives only once
// the file has shown it needs one. A step SHOWS its change (the spot marked first,
// everything else dimmed, the keys typed), LIGHTS what it did in the table, and only then
// TELLS: one short line saying what was just seen. components/story.tsx plays each once in
// view; afterwards the file is the reader's to edit.
//
// Every state of the file is read by the real engine when the page builds and again in the
// reader's browser; `bun scripts/examples.ts` reads each one and asserts what the step
// claims (its problems, its computed values), so a step can never show something the
// engine does not do.

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

/** What the table lights once the change has happened: a column, or one cell of it. */
export interface StoryLit {
  column: string
  /** The data row, from 0; absent, the whole column. */
  row?: number
}

export interface StoryStep {
  id: string
  /** What is told once it has been shown: one line, its lead bright, its rest quieter. */
  lead: string
  rest: string
  /** The file's name before the step and after it (a rename types the difference). */
  name: { before: string; after: string }
  /** The file's text before the step and after it. */
  text: { before: string; after: string }
  edits: StoryEdit[]
  /** Marked before the keys: where the change will happen. Null when nothing is typed. */
  spot: string | null
  /** Marked once the change is made: what the step is about. */
  mark: string
  /** The files it imports, each a tab beside it. */
  others: StoryFile[]
  /** The table beside the file: not yet, arriving in this step, or there. */
  table: "none" | "arrives" | "shown"
  lit?: StoryLit
  /** The forms of the idea, listed under the told line, each in the file. */
  forms?: { formula: string; says: string }[]
  /** What the engine must find in the file as the step leaves it: each problem's
   *  `row, column: reason`, and computed values (checked by scripts/examples.ts). */
  expect: { problems: string[]; values?: Record<string, (string | number | null)[]> }
}

/** The file and its imports as a step leaves them. */
export const filesOf = (step: StoryStep): StoryFile[] => [{ name: step.name.after, text: step.text.after }, ...step.others]

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

/** One step as written: what changes and what is said. The rest is the step before's. */
interface Beat {
  id: string
  lead: string
  rest: string
  rename?: string
  edits?: StoryEdit[]
  spot?: string
  mark: string
  /** Files imported from this step on. */
  imports?: StoryFile[]
  /** The table arrives here. */
  table?: true
  lit?: StoryLit
  forms?: StoryStep["forms"]
  expect: StoryStep["expect"]
}

/** The beats run in order on one file: each starts where the one before ended. */
function chain(name: string, text: string, beats: Beat[]): StoryStep[] {
  const out: StoryStep[] = []
  let file = { name, text }
  let others: StoryFile[] = []
  let table = false
  for (const b of beats) {
    if (b.edits?.length && !b.spot) throw new Error(`story: ${b.id} types with no spot marked first`)
    const after = { name: b.rename ?? file.name, text: edited(file.text, b.edits ?? []) }
    others = [...others, ...(b.imports ?? [])]
    out.push({
      id: b.id,
      lead: b.lead,
      rest: b.rest,
      name: { before: file.name, after: after.name },
      text: { before: file.text, after: after.text },
      edits: b.edits ?? [],
      spot: b.spot ?? null,
      mark: b.mark,
      others,
      table: b.table ? "arrives" : table ? "shown" : "none",
      lit: b.lit,
      forms: b.forms,
      expect: b.expect,
    })
    table ||= !!b.table
    file = after
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

export const STORY: StoryStep[] = chain(
  "groceries.csv",
  `item,on,cost,qty
fuel,2026-11-01,84.20,1
groceries,2026-11-02,96.75,2
coffee,2026-11-03,2.10,3
`,
  [
    {
      id: "csv",
      lead: "A CSV: a header, then a row a line.",
      rest: "Any editor opens it, any program reads it.",
      mark: "item,on,cost,qty",
      expect: { problems: [] },
    },
    {
      id: "typo",
      lead: "96.7x went in, and nothing objected.",
      rest: "A CSV has no types: every cell is text.",
      edits: [{ after: "2026-11-02,96.7", remove: 1, insert: "x" }],
      spot: "96.75",
      mark: "96.7x",
      expect: { problems: [] },
    },
    {
      id: "types",
      lead: `Renamed ${ext} and typed as euros, 96.7x is named.`,
      rest: "The type is in the header, so the file carries it wherever it goes.",
      rename: `groceries${ext}`,
      edits: [{ after: "item,on,cost", insert: ":EUR>=0" }],
      spot: "cost",
      mark: "cost:EUR>=0",
      expect: { problems: ['row 2, cost: "96.7x" is not a number'] },
    },
    {
      id: "table",
      lead: `${PRODUCT.name} shows the same file as a table.`,
      rest: "96.7x keeps its text, marked with the reason.",
      mark: "96.7x",
      table: true,
      lit: { column: "cost", row: 1 },
      expect: { problems: ['row 2, cost: "96.7x" is not a number'] },
    },
    {
      id: "fixed",
      lead: "96.75 typed back, and the problem is gone.",
      rest: "The editor and the table read one file, so they never disagree.",
      edits: [{ after: "2026-11-02,96.7", remove: 1, insert: "5" }],
      spot: "96.7x",
      mark: "96.75",
      lit: { column: "cost", row: 1 },
      expect: { problems: [], values: { cost: [84.2, 96.75, 2.1] } },
    },
    {
      id: "formula",
      lead: "total is computed on every row.",
      rest: "Its formula is in the header, and the rows store nothing for it.",
      edits: [{ after: "qty", insert: ",total:EUR=cost * qty" }],
      spot: "qty",
      mark: ",total:EUR=cost * qty",
      lit: { column: "total" },
      expect: { problems: [], values: { total: [84.2, 193.5, 6.3] } },
    },
    {
      id: "running",
      lead: "running adds up the rows before it.",
      rest: "Before by the order of on, a column, never by the order of the lines.",
      edits: [{ after: "total:EUR=cost * qty", insert: ",running:EUR=sum(total) over (order by on)" }],
      spot: "total:EUR=cost * qty",
      mark: ",running:EUR=sum(total) over (order by on)",
      lit: { column: "running" },
      expect: { problems: [], values: { running: [84.2, 277.7, 284] } },
    },
    {
      id: "cell",
      lead: "One cell holds its own formula.",
      rest: "Between backticks: the file keeps 6.30 / 3, the table shows 2.10.",
      edits: [{ after: "2026-11-03,", remove: 4, insert: "`=6.30 / 3`" }],
      spot: "2.10",
      mark: "`=6.30 / 3`",
      lit: { column: "cost", row: 2 },
      expect: { problems: [], values: { cost: [84.2, 96.75, 2.1] } },
    },
    {
      id: "references",
      lead: `to names a row of ${ACCOUNTS.name}, the file it imports.`,
      rest: "Its keys are car, food and cafe: open its tab.",
      imports: [ACCOUNTS],
      edits: [
        { after: "", insert: `---\nimport accounts from "./${ACCOUNTS.name}"\n---\n` },
        { after: "qty", insert: ",to:accounts.account" },
        { after: "84.20,1", insert: ",car" },
        { after: "96.75,2", insert: ",food" },
        { after: "3`,3", insert: ",cafe" },
      ],
      spot: "qty",
      mark: "to:accounts.account",
      lit: { column: "to" },
      expect: { problems: [], values: { to: ["car", "food", "cafe"] } },
    },
    {
      id: "reads",
      lead: "to.kind follows the reference into accounts.",
      rest: "A formula reads its own row, the rows before it, or a row another file holds.",
      edits: [{ after: "over (order by on)", insert: ",kind:text=to.kind" }],
      spot: "over (order by on)",
      mark: ",kind:text=to.kind",
      lit: { column: "kind" },
      forms: [
        { formula: "cost * qty", says: "its own row" },
        { formula: "sum(total) over (order by on)", says: "the rows before it, by a column's order" },
        { formula: "to.kind", says: "the row of accounts that to names" },
      ],
      expect: { problems: [], values: { kind: ["needs", "needs", "treats"] } },
    },
    {
      id: "key",
      lead: "fodo is caught as it is typed.",
      rest: `No row of ${ACCOUNTS.name} is fodo, and the kind it would read is left empty.`,
      edits: [{ after: "96.75,2,", remove: 4, insert: "fodo" }],
      spot: "food",
      mark: "fodo",
      lit: { column: "to", row: 1 },
      expect: { problems: ['row 2, to: "fodo" is no key of accounts'], values: { kind: ["needs", null, "treats"] } },
    },
  ],
)
