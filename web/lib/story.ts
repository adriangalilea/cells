// THE STORY, as data: ONE file, built act by act, a trip four friends share. It starts as a
// CSV opened in Cells; each act is one thing the act before could not do, shown on the
// file as it left it, then told. The app comes first (`view: "app"`, the Cells window
// alone, reading each write, the column or cell the act is about lit once it has played);
// only once the reader knows what it does is the file under it shown (`view: "text"`):
// Helix beside the app, the edit typed where it goes and written, the engine as its
// language server. components/story.tsx tells it on ui's walkthrough, a step per act,
// pinned while the page scrolls; each act links to the docs' section it shows (`doc`).
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

/** How an act is shown: the Cells window alone, or the file's text in Helix beside it. */
export type StoryView = "app" | "text"

export interface StoryAct {
  id: string
  view: StoryView
  /** The docs' page and heading the act is told in full by (`/docs/spec/formulas`). */
  doc?: string
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
  view: StoryView
  doc?: string
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
    const act = { id: s.id, view: s.view, doc: s.doc, lead: s.lead, rest: s.rest, from, end: saved, edits, saves, others, light: s.light, expect: s.expect }
    from = saved
    return act
  })
}

/** Who is on the trip, the file a cost's `paid` names a row of. */
const PEOPLE: StoryFile = {
  name: `people${ext}`,
  text: `name:text!,email:email
ana,ana@example.com
ben,ben@example.com
carla,carla@example.com
dan,dan@example.com
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
  "lisbon.csv",
  `item,paid,cost
flights,ana,412.00
apartment,ben,540.00
dinner,carla,96.75
taxi,dan,18.40
`,
  [
    // ── the app: what Cells does, before what it is ──
    {
      id: "csv",
      view: "app",
      doc: "/docs/spec/the-file",
      lead: "A CSV, opened in Cells",
      rest: "Four friends, one trip. It opens fine, but every value is text: nothing knows `cost` is money.",
      expect: { problems: [] },
    },
    {
      id: "typo",
      view: "app",
      lead: "So a typo slips in",
      rest: "`54O.00`, with a letter O. The CSV takes it, and any sum over the column is quietly wrong.",
      edits: [{ after: "ben,54", remove: 1, insert: "O" }],
      light: { column: "cost", row: 1 },
      expect: { problems: [] },
    },
    {
      id: "renamed",
      view: "app",
      doc: "/docs",
      lead: `Rename it \`lisbon${ext}\``,
      rest: "That is the whole migration. From here on, the file can say what each column holds.",
      rename: `lisbon${ext}`,
      light: { column: "cost", row: 1 },
      expect: { problems: [] },
    },
    {
      id: "types",
      view: "app",
      doc: "/docs/spec/types",
      lead: "Say what a column holds",
      rest: "`cost` is euros, never below zero. The typo is flagged where it is written; fixed, the column is money.",
      edits: [
        { after: "item,paid,cost", insert: ":EUR>=0" },
        { after: "ben,54", remove: 1, insert: "0" },
      ],
      light: { column: "cost", row: 1 },
      expect: {
        problems: [],
        values: { cost: [412, 540, 96.75, 18.4] },
        between: [['row 2, cost: "54O.00" is not a number']],
      },
    },
    {
      id: "formula",
      view: "app",
      doc: "/docs/spec/formulas#a-columns-formula",
      lead: "Columns compute",
      rest: "Who owes what: a quarter of each cost, on every row. Each value is stored in the file beside its `=`, so it reads without running anything.",
      edits: [{ after: "cost:EUR>=0", insert: ",share:EUR=cost / 4" }],
      light: { column: "share" },
      expect: { problems: [], values: { share: [103, 135, 24.19, 4.6] } },
    },
    {
      id: "cell",
      view: "app",
      doc: "/docs/spec/formulas#a-formula-in-a-cell",
      lead: "A cell keeps its own math",
      rest: "Dinner was three bottles at 28.50 and a tip. The cell holds the sum and how it was made.",
      edits: [{ after: "dinner,carla,", remove: 5, insert: "=3 * 28.50 + 11.25" }],
      light: { column: "cost", row: 2 },
      expect: { problems: [], values: { cost: [412, 540, 96.75, 18.4] } },
    },
    {
      id: "running",
      view: "app",
      doc: "/docs/spec/formulas#rows-by-where-they-are",
      lead: "Formulas see other rows",
      rest: "How much so far: each row adds its cost to the row above's, `[-1]`.",
      edits: [{ after: "share:EUR=cost / 4", insert: ",spent:EUR=spent[-1] + cost" }],
      light: { column: "spent" },
      expect: { problems: [], values: { spent: [412, 952, 1048.75, 1067.15] } },
    },
    {
      id: "references",
      view: "app",
      doc: "/docs/spec/types#references",
      lead: "Who paid is one of the four",
      rest: `\`paid\` names a row of \`people${ext}\`, checked across the two files, so \`karla\` is flagged.`,
      imports: [PEOPLE],
      edits: [
        { after: "", insert: `---\nimport people from "./${PEOPLE.name}"\n---\n` },
        { after: "item,paid", insert: ":people.name" },
        { after: "dinner,", remove: 5, insert: "karla" },
      ],
      light: { column: "paid", row: 2 },
      expect: { problems: ['row 3, paid: "karla" is no key of people'], between: [[], []] },
    },
    // ── the file: what it is, now the reader knows what it does ──
    {
      id: "text",
      view: "text",
      doc: "/docs/spec/the-file",
      lead: "Underneath, it is plain text",
      rest: "Everything you just did is a few characters: the types in the header, each computed value beside its `=`, the other file imported between the `---` lines. Open it in Vim, Helix or any editor.",
      expect: { problems: ['row 3, paid: "karla" is no key of people'] },
    },
    {
      id: "editor",
      view: "text",
      doc: "/docs/editors",
      lead: "Your editor knows it too",
      rest: `\`${PRODUCT.cli} lsp\` names a broken value as you type, completes a key from the other file, and stores every computed value when you save. Helix and Neovim today.`,
      edits: [{ after: "dinner,", remove: 5, insert: "carla" }],
      light: { column: "paid", row: 2 },
      expect: { problems: [] },
    },
  ],
)
