// THE STORY, as data: ONE file, built act by act. It starts as a short CSV; each act is one
// idea shown on the file as the act before left it, then told: an edit in Helix (each
// change typed where it goes, then written), the engine as its language server, and the
// Cells window reading each write, the column or cell the act is about lit once it has
// played. components/story.tsx tells it on ui's walkthrough, a step per act, pinned while
// the page scrolls.
//
// Every state of the file is the engine's: each `:w` writes what the language server's
// formatting writes (every computed value the edit reached stored beside its `=`), so the
// story is made with the engine (`story`), read by it when the page builds and again in
// the reader's browser; `bun scripts/examples.ts` times every step and asserts what each
// act claims (its problems, its computed values), so an act can never show something the
// engine does not do.

import type { CodeFile, CodeStep } from "@/lib/code-session"
import { PRODUCT } from "@/lib/product"
import { type Engine, savedAs } from "@/lib/view"

const ext = PRODUCT.extension

/** A file: its name and its text. */
export interface StoryFile {
  name: string
  text: string
}

/** A change to the file: after the first place `after` occurs in the text as it stands,
 *  `remove` characters taken away, then `insert` written. */
export interface StoryEdit {
  after: string
  insert: string
  remove?: number
}

/** What the Cells window lights: a column, or one cell of it. */
export interface StoryLit {
  column: string
  /** The data row, from 0; absent, the whole column. */
  row?: number
}

export interface StoryAct {
  id: string
  /** What is told once it has been shown: its lead the step's title, then the rest, only
   *  where it adds something. In both, what is written in a file (a name, a value, a
   *  file's name) stands between backticks, set as code where it is drawn. */
  lead: string
  rest?: string
  /** The file as the act finds it, and as it leaves it. */
  from: StoryFile
  end: StoryFile
  /** Each a change typed into the file as the save before wrote it. */
  edits: StoryEdit[]
  /** The file's text as each edit's save writes it, the engine's formatting: the last is
   *  `end`'s. */
  saves: string[]
  /** The files it imports, as they stand. */
  others: StoryFile[]
  light?: StoryLit
  /** What the engine must find in the file as the act leaves it: each problem's
   *  `row, column: reason`, and computed values; and `between`, the problems the file
   *  has as written after each edit but the last, an act that screams on the way
   *  (checked by scripts/examples.ts). */
  expect: { problems: string[]; values?: Record<string, (string | number | null)[]>; between?: string[][] }
}

/** `text` with `e` made. */
function edited(text: string, e: StoryEdit): string {
  const i = text.indexOf(e.after)
  if (i < 0) throw new Error(`story: "${e.after}" is not in the text it is written into`)
  const at = i + e.after.length
  return text.slice(0, at) + e.insert + text.slice(at + (e.remove ?? 0))
}

/** Told words as plain text, their backticks gone: a step's name to assistive tech. */
export const plain = (told: string) => told.replaceAll("`", "")

/** Where `index` falls in `text`: its line and column, both from 1. */
function placeOf(text: string, index: number): { line: number; column: number } {
  const before = text.slice(0, index)
  const lastLine = before.lastIndexOf("\n")
  return { line: before.split("\n").length, column: index - lastLine }
}

/** A pause after the file opens, ms, before the first key: the file is seen first. */
const LOOK_MS = 700

/** An act as an edit in an editor (ui's code-session): its file opened under the name it
 *  leaves with, on the text it finds, beside the files it imports; each change typed
 *  where it goes (what it takes away selected and typed over), then written (`:w`), the
 *  language server's formatting storing what it reached as the write lands, so what
 *  reads the file reads each change as the editor writes it. */
export function editOf(act: StoryAct): { files: CodeFile[]; steps: CodeStep[]; path: string } {
  const path = act.end.name
  let text = act.from.text
  const steps: CodeStep[] = [{ kind: "open", path }]
  for (const [k, e] of act.edits.entries()) {
    const i = text.indexOf(e.after)
    if (i < 0) throw new Error(`story: "${e.after}" is not in the text it is typed into`)
    const at = i + e.after.length
    const { line, column } = placeOf(text, at)
    const delay = k === 0 ? LOOK_MS : undefined
    const formatted = act.saves[k] as string
    steps.push(
      e.remove
        ? { kind: "replace", line, column, length: e.remove, text: e.insert, delay }
        : { kind: "type", line, column, text: e.insert, delay },
      { kind: "save", formatted },
    )
    text = formatted
  }
  if (text !== act.end.text) throw new Error(`story: ${act.id}'s edit does not leave the file it claims`)
  return { files: [{ path, text: act.from.text }, ...act.others.map((o) => ({ path: o.name, text: o.text }))], steps, path }
}

/** One act as written: what changes and what is said. The rest is the act before's. */
interface Script {
  id: string
  lead: string
  rest?: string
  /** The file's new name from this act on. */
  rename?: string
  /** Files imported from this act on. */
  imports?: StoryFile[]
  edits?: StoryEdit[]
  light?: StoryLit
  expect: StoryAct["expect"]
}

/** The scripts played in order on one file: each starts where the one before ended, and
 *  each edit is typed into the file as the save before wrote it. */
function chain(engine: Engine, name: string, text: string, scripts: Script[]): StoryAct[] {
  let from = { name, text }
  let others: StoryFile[] = []
  return scripts.map((s) => {
    others = [...others, ...(s.imports ?? [])]
    const edits = s.edits ?? []
    const named = s.rename ?? from.name
    const saves: string[] = []
    let saved = { name: named, text: from.text }
    for (const e of edits) {
      const now = { name: named, text: edited(saved.text, e) }
      saved = { name: named, text: savedAs(engine, saved, now, others) }
      saves.push(saved.text)
    }
    const act = { id: s.id, lead: s.lead, rest: s.rest, from, end: saved, edits, saves, others, light: s.light, expect: s.expect }
    from = saved
    return act
  })
}

const ACCOUNTS: StoryFile = {
  name: `accounts${ext}`,
  text: `account:text!,kind:needs|treats
car,needs
food,needs
cafe,treats
`,
}

const made = new WeakMap<Engine, StoryAct[]>()

/** The story, its files as the engine writes them: made once. */
export function story(engine: Engine): StoryAct[] {
  const had = made.get(engine)
  if (had) return had
  const acts = tell(engine)
  made.set(engine, acts)
  return acts
}

const tell = (engine: Engine): StoryAct[] => chain(
  engine,
  "groceries.csv",
  `item,on,cost,qty
fuel,2026-11-01,84.20,1
groceries,2026-11-02,96.75,2
coffee,2026-11-03,2.10,3
`,
  [
    {
      id: "plain",
      lead: "Behold a `.csv`",
      rest: "Values separated by commas, a row per line. The first row names the columns.",
      expect: { problems: [] },
    },
    {
      id: "csv",
      lead: "No types",
      rest: "So a column meant to hold numbers takes a typo, and nothing screams.",
      edits: [{ after: "2026-11-02,96.7", remove: 1, insert: "x" }],
      light: { column: "cost", row: 1 },
      expect: { problems: [] },
    },
    {
      id: "renamed",
      lead: `Now, let's \`${ext}\``,
      rename: `groceries${ext}`,
      light: { column: "cost", row: 1 },
      expect: { problems: [] },
    },
    {
      id: "types",
      lead: "Types live in the header",
      rest: "A value that breaks its column's type screams, so it can be fixed.",
      edits: [
        { after: "item,on,cost", insert: ":EUR>=0" },
        { after: "2026-11-02,96.7", remove: 1, insert: "5" },
      ],
      light: { column: "cost", row: 1 },
      expect: { problems: [], values: { cost: [84.2, 96.75, 2.1] }, between: [['row 2, cost: "96.7x" is not a number']] },
    },
    {
      id: "formula",
      lead: "Columns can compute",
      rest: "A formula in the header fills the column. Each value is written beside its `=`, so every reader sees it.",
      edits: [{ after: "qty", insert: ",total:EUR=cost * qty" }],
      light: { column: "total" },
      expect: { problems: [], values: { total: [84.2, 193.5, 6.3] } },
    },
    {
      id: "running",
      lead: "Formulas can see other rows",
      rest: "A row is named by where it is: `[-1]` is the one above.",
      edits: [{ after: "total:EUR=cost * qty", insert: ",running:EUR=running[-1] + total" }],
      light: { column: "running" },
      expect: { problems: [], values: { running: [84.2, 277.7, 284] } },
    },
    {
      id: "cell",
      lead: "A cell can hold its own formula",
      rest: "The file keeps the formula and its value.",
      edits: [{ after: "2026-11-03,", remove: 4, insert: "=6.30 / 3" }],
      light: { column: "cost", row: 2 },
      expect: { problems: [], values: { cost: [84.2, 96.75, 2.1] } },
    },
    {
      id: "references",
      lead: "Columns can point at another file",
      rest: "Each value must be a key there, checked across files.",
      imports: [ACCOUNTS],
      edits: [
        { after: "", insert: `---\nimport accounts from "./${ACCOUNTS.name}"\n---\n` },
        { after: "running[-1] + total", insert: ",to:accounts.account" },
        { after: "84.20=,84.20=", insert: ",car" },
        { after: "277.70=", insert: ",food" },
        { after: "284.00=", insert: ",cafe" },
      ],
      light: { column: "to" },
      expect: {
        problems: [],
        values: { to: ["car", "food", "cafe"] },
        between: [
          [],
          ["row 1, to: is missing", "row 2, to: is missing", "row 3, to: is missing"],
          ["row 2, to: is missing", "row 3, to: is missing"],
          ["row 3, to: is missing"],
        ],
      },
    },
    {
      id: "reads",
      lead: "Formulas follow those links",
      rest: "A column can read any field of the row it points at.",
      edits: [{ after: "to:accounts.account", insert: ",kind:text=to.kind" }],
      light: { column: "kind" },
      expect: { problems: [], values: { kind: ["needs", "needs", "treats"] } },
    },
    {
      id: "key",
      lead: "A broken link screams",
      rest: "Right where it is written, and what depends on it is left empty.",
      edits: [{ after: "277.70=,", remove: 4, insert: "fodo" }],
      light: { column: "to", row: 1 },
      expect: { problems: ['row 2, to: "fodo" is no key of accounts'], values: { kind: ["needs", null, "treats"] } },
    },
  ],
)
