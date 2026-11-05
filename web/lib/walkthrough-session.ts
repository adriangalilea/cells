// A WALKTHROUGH, scripted: a file told step by step, each step its words and what the
// file does at it (lines marked, another file brought forward, the file at a new
// state, keys typed into it), timed into a playable any driver moves through and folded
// into the frame a view draws at an instant: which file, the lines it shows and how
// open each is, the bands marking lines and where they stand mid-travel, the line to
// keep in view. A player plays it, a scroll stage cues it step by step, a fixed
// progress is a still: the frame is a pure function of the instant, so a scrubbed
// walkthrough and a played one are the same frames. Framework-free.
//
// A step changes the file in one of two ways, each the existing mechanism for it:
// `text`, the file's whole state from that step, compared line by line with the one
// before (the lines that go fold away, those that come unfold, and `diff` keeps both
// signed as `code`'s diff draws them); `type`, keys typed into it, code-session's own
// `type` steps, paced by code-session's table.
//
// A step's time, in order: the change (the file brought forward, or its lines
// folding), the keys, the marks travelling to their new lines, then reading. The
// pacing is the one table below.

import {
  type CodeStep,
  type CodeTimeline,
  codeFrameAt,
  codeTimeline,
  placeOf,
} from "@/lib/code-session"
import { langOf, linesOf } from "@/lib/code-tokens"
import type { Chapter, Playable } from "@/lib/playable"

/** The change: a file brought forward, or its lines folding and unfolding. */
const CHANGE_MS = 700
/** The marks travelling from the lines they stood on to the step's own. */
const MARK_MS = 650
/** A step held for reading once it has played, unless it says (`hold`). */
export const READ_MS = 2600
/** The step's words and view arriving. */
const ARRIVE_MS = 450
/** What a step with nothing moving in its file plays over (a view's progress). */
const PLAY_MS = 1200
/** The view easing onto the caret before the first key lands. */
const FOLLOW_MS = 300
/** The most cells a line diff may weigh: a file past it screams rather than stall. */
const DIFF_CELLS = 16_000_000

/** Keys typed into the file in front: code-session's `type` step. Where: `line` and
 *  `column` (from 1; no column, the line's end) in the text as it is when the keys
 *  start, else where the last keys left the caret (the file's start before any).
 *  With `replace`, that many characters from the place are selected and typed over,
 *  as an editor changes a value (code-session's `replace`; an empty `text` deletes
 *  them): a change that takes something away is typed too, never drawn as a diff. */
export interface WalkthroughKeys {
  text: string
  line?: number
  column?: number
  replace?: number
}

/** One step of the walkthrough. Every field but the title is optional: a step that
 *  names nothing keeps the file, its text and its marks as the last step left them,
 *  but its marks, which a step names or clears. */
export interface WalkthroughStep<Words = unknown, View = unknown> {
  /** Its name: the chapter's title, in a player's bar and to assistive tech. */
  title: string
  /** The title as drawn over its words, when it needs more than text (a file's name
   *  set as code); absent, the title. Carried, never read here. */
  heading?: Words
  /** The file shown, a path: the language is read from its extension unless `lang`
   *  says. Absent, the last step's. A file shown before keeps its text. */
  file?: string
  /** The file's language (a Shiki id, or a registered one: lib/code-tokens
   *  `registerLanguage`). */
  lang?: string
  /** The file's whole text from this step. Its change from the text before is drawn
   *  line by line. A file shown for the first time needs it (or keys). */
  text?: string
  /** Keys typed into it after the change. */
  type?: readonly WalkthroughKeys[]
  /** Lines to mark, `"2"` or `"1,4-6"`, counted from 1 in the file as the step
   *  leaves it. Absent: no marks. */
  marks?: string
  /** Draw the change as a diff: the lines that came signed `+`, the ones that went
   *  kept and signed `-`, until the next step folds them away. */
  diff?: boolean
  /** Reading time once it has played, ms. */
  hold?: number
  /** What its view plays over, ms, when nothing moves in its file: a view with a
   *  clock of its own (an editor playing an edit, its length the edit's). 1.2 s
   *  unless it says. */
  play?: number
  /** Its words, whatever the view draws them with (a React node in `walkthrough`). */
  words?: Words
  /** What it shows beside the file, live (a function of its progress in
   *  `walkthrough`). Carried, never read here. */
  view?: View
}

/** A text the walkthrough shows, as each line of it is coloured: one per state a file
 *  passes through. */
export interface WalkthroughState {
  path: string
  lang: string
  text: string
}

/** A line as drawn: its text, and where its characters are in the state they are
 *  coloured by (`state`, an index into the timeline's `states`): the line's start, or
 *  one offset per character for a line being typed. */
export interface WalkthroughLine {
  text: string
  state: number
  at: number | readonly number[]
}

/** What a line is in a diff (`code`'s two signs), or nothing. */
export type WalkthroughSign = "add" | "remove" | null

/** A line in the frame: how open it is (0 folded away, 1 a whole line) and its sign. */
export interface WalkthroughRow {
  line: WalkthroughLine
  open: number
  sign: WalkthroughSign
}

/** A band marking lines: where it starts and how many lines it covers, both in lines
 *  from the top, fractional while it travels or while the lines under it fold. */
export interface WalkthroughBand {
  from: number
  size: number
}

/** The file at an instant. */
export interface WalkthroughPane {
  path: string
  lang: string
  rows: WalkthroughRow[]
  bands: WalkthroughBand[]
  /** The line to keep in view, in lines from the top (fractional: the view travels). */
  focus: number
  /** The caret while keys are typed: its row and column, both from 0. */
  caret: { row: number; column: number } | null
  /** What keys are about to type over, a piece per row it covers: its row and its
   *  columns (from 0, `to` excluded). Empty when nothing is selected. */
  selection: { row: number; from: number; to: number }[]
  /** 0..1: a file fading out for the next, and the next fading in. */
  opacity: number
  /** The file as the step leaves it, as the clipboard gets it. */
  source: string
}

/** Everything a view draws at an instant. */
export interface WalkthroughFrame {
  /** The step on, from 0. */
  step: number
  /** Its progress, 0 as it arrives to 1 once it has played; reading holds it at 1. */
  p: number
  /** 0..1, eased: the step's words and view arriving over the last ones. */
  arrive: number
  /** The file, or null before any step names one. */
  pane: WalkthroughPane | null
  /** The file as last written at this instant (its path and text): what a view that
   *  reads the file shows, as an app reads a file when the editor writes it, never
   *  mid-keys. Null before any step names a file. */
  written: { path: string; text: string } | null
}

/** A row at rest, with what it was: a real line of the text, or a line kept signed
 *  after it went (a diff's `-`). */
interface Rest {
  line: WalkthroughLine
  sign: WalkthroughSign
  ghost: boolean
}

/** A file at rest: its text, its rows, and the row each line of the text is. */
interface Settled {
  text: string
  rows: Rest[]
  real: number[]
}

/** A row through a change: open from `from` to `to`, its sign from one to the other
 *  at the change's middle. */
interface Folding {
  line: WalkthroughLine
  from: number
  to: number
  signFrom: WalkthroughSign
  signTo: WalkthroughSign
}

/** A band as lines of the rows it lies on, `first` through `first + size - 1`: zero
 *  lines is a band shrunk to a point at `first`. */
interface Span {
  first: number
  size: number
}

interface Pane {
  path: string
  lang: string
  settled: Settled
  bands: WalkthroughBand[]
  focus: number
}

/** One step, planned: what each of its phases draws. */
export interface WalkthroughPlan {
  path: string | null
  lang: string
  /** The file it leaves, faded out before this one fades in. */
  leaving: Pane | null
  /** Its lines folding: the merged rows, the last step's bands on them, the line the
   *  view held, and the rows that change. */
  change: {
    rows: Folding[]
    bands: Span[]
    anchor: number
    held: number
    changed: { first: number; last: number } | null
  } | null
  /** Its keys: code-session's timeline over the file, from its first key. */
  keys: {
    timeline: CodeTimeline
    path: string
    from: number
    /** Each of code-session's states, as one of the walkthrough's. */
    states: number[]
    /** Each offset of the text before the keys, where it is in the typed file. */
    base: number[]
    bands: Span[]
    /** Where the view stands as the keys begin. */
    focus: number
  } | null
  /** The file as written, from when (ms into the step): its text once the change is
   *  made, then as each keys entry is done. */
  written: { at: number; text: string }[]
  /** Where the bands stand once the change and the keys are done. */
  before: WalkthroughBand[]
  bands: WalkthroughBand[]
  /** The view's line once the keys are done, and once the marks have arrived. */
  focusBefore: number
  focus: number
  settled: Settled
  /** The phases' lengths, ms: `play` the view's own when nothing moves in the file
   *  (0 when something does). */
  phase: {
    change: number
    keys: number
    marks: number
    play: number
    read: number
  }
}

export interface WalkthroughTimeline<Words = unknown, View = unknown> {
  steps: readonly WalkthroughStep<Words, View>[]
  plans: WalkthroughPlan[]
  /** Each step's start and end, ms. */
  starts: number[]
  ends: number[]
  total: number
  /** Every text shown, for its colours (lib/code-tokens). */
  states: WalkthroughState[]
  /** The most lines the file shows at any step: the view's height in lines, when it
   *  fits. */
  lines: number
  /** The walkthrough as a playable: one chapter per step, titled with it. */
  playable: Playable
}

const ease = (t: number) =>
  t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2
const clamp01 = (t: number) => Math.min(1, Math.max(0, t))
const lerp = (a: number, b: number, t: number) => a + (b - a) * t

/** `text`'s lines, each with where it starts. */
function linesWithStarts(text: string): { text: string; at: number }[] {
  const out: { text: string; at: number }[] = []
  let at = 0
  for (const line of text.split("\n")) {
    out.push({ text: line, at })
    at += line.length + 1
  }
  return out
}

type Op =
  | { kind: "keep"; a: number; b: number }
  | { kind: "remove"; a: number }
  | { kind: "add"; b: number }

/** The lines of `a` against those of `b`, the longest common run kept: what went
 *  comes before what came, as a diff reads. */
function diffLines(a: readonly string[], b: readonly string[]): Op[] {
  const n = a.length
  const m = b.length
  if ((n + 1) * (m + 1) > DIFF_CELLS)
    throw new Error(
      `walkthrough-session: ${n} lines against ${m} is past the diff's ${DIFF_CELLS} cells`,
    )
  const w = m + 1
  const lcs = new Uint32Array((n + 1) * w)
  for (let i = n - 1; i >= 0; i--)
    for (let j = m - 1; j >= 0; j--)
      lcs[i * w + j] =
        a[i] === b[j]
          ? (lcs[(i + 1) * w + j + 1] as number) + 1
          : Math.max(
              lcs[(i + 1) * w + j] as number,
              lcs[i * w + j + 1] as number,
            )
  const ops: Op[] = []
  let i = 0
  let j = 0
  while (i < n || j < m) {
    if (i < n && j < m && a[i] === b[j])
      ops.push({ kind: "keep", a: i++, b: j++ })
    else if (
      i < n &&
      (j >= m ||
        (lcs[(i + 1) * w + j] as number) >= (lcs[i * w + j + 1] as number))
    )
      ops.push({ kind: "remove", a: i++ })
    else ops.push({ kind: "add", b: j++ })
  }
  return ops
}

/** A text at rest, every line real and unsigned. */
function settle(text: string, state: number): Settled {
  const rows = linesWithStarts(text).map((l) => ({
    line: { text: l.text, state, at: l.at },
    sign: null,
    ghost: false,
  }))
  return { text, rows, real: rows.map((_, i) => i) }
}

/** The rows of `before` folding into `text`: lines kept, gone, come, and the rows a
 *  last diff kept signed folding away. Returns the rows and where each row of
 *  `before` is among them. */
function fold(
  before: Settled,
  text: string,
  state: number,
  diff: boolean,
): { rows: Folding[]; at: number[] } {
  const next = linesWithStarts(text)
  const ops = diffLines(
    before.real.map((r) => (before.rows[r] as Rest).line.text),
    next.map((l) => l.text),
  )
  const line = (b: number): WalkthroughLine => {
    const l = next[b] as { text: string; at: number }
    return { text: l.text, state, at: l.at }
  }
  const rows: Folding[] = []
  const at: number[] = []
  let k = 0
  const come = () => {
    while (ops[k]?.kind === "add") {
      const op = ops[k++] as { kind: "add"; b: number }
      const sign = diff ? "add" : null
      rows.push({
        line: line(op.b),
        from: 0,
        to: 1,
        signFrom: sign,
        signTo: sign,
      })
    }
  }
  let real = 0
  for (const row of before.rows) {
    if (row.ghost) {
      at.push(rows.length)
      rows.push({
        line: row.line,
        from: 1,
        to: 0,
        signFrom: row.sign,
        signTo: row.sign,
      })
      continue
    }
    come()
    const op = ops[k++]
    if (!op || op.kind === "add" || op.a !== real)
      throw new Error("walkthrough-session: the diff lost its place")
    real++
    at.push(rows.length)
    if (op.kind === "keep")
      rows.push({
        line: line(op.b),
        from: 1,
        to: 1,
        signFrom: row.sign,
        signTo: null,
      })
    else
      rows.push({
        line: row.line,
        from: 1,
        to: diff ? 1 : 0,
        signFrom: row.sign,
        signTo: diff ? "remove" : row.sign,
      })
  }
  come()
  if (k !== ops.length)
    throw new Error("walkthrough-session: the diff ran over")
  return { rows, at }
}

/** The rows a fold leaves: those still open, a `-` kept as a ghost. */
function folded(rows: readonly Folding[], text: string): Settled {
  const rest: Rest[] = rows
    .filter((r) => r.to > 0)
    .map((r) => ({
      line: r.line,
      sign: r.signTo,
      ghost: r.signTo === "remove",
    }))
  return {
    text,
    rows: rest,
    real: rest.flatMap((r, i) => (r.ghost ? [] : [i])),
  }
}

/** The bands `marks` names, on the rows of `settled`. Screams at a line it lacks. */
function bandsOf(
  marks: string | undefined,
  settled: Settled,
  title: string,
): WalkthroughBand[] {
  if (!marks) return []
  const lines = [...linesOf(marks)].sort((a, b) => a - b)
  const bands: WalkthroughBand[] = []
  for (const n of lines) {
    const row = settled.real[n - 1]
    if (row === undefined)
      throw new Error(
        `walkthrough-session: "${title}" marks line ${n}, and its file has ${settled.real.length}`,
      )
    const last = bands.at(-1)
    if (last && last.from + last.size === row) last.size++
    else bands.push({ from: row, size: 1 })
  }
  return bands
}

/** A band as the rows it lies on, through `at` (each old row's place among new). */
function spanOf(band: WalkthroughBand, at: readonly number[]): Span {
  const first = Math.round(band.from)
  const size = Math.round(band.size)
  const a = at[first]
  if (a === undefined)
    throw new Error(
      `walkthrough-session: a band on row ${first}, past the file`,
    )
  if (size === 0) return { first: a, size: 0 }
  const b = at[first + size - 1] as number
  return { first: a, size: b - a + 1 }
}

/** The line in the middle of the bands: what the view keeps in sight. */
const middle = (bands: readonly WalkthroughBand[]) => {
  const a = bands[0] as WalkthroughBand
  const z = bands.at(-1) as WalkthroughBand
  return (a.from + z.from + z.size) / 2
}

/** Rows folding at `e` (0..1): each one's openness, and its sign. */
function rowsAt(rows: readonly Folding[], e: number): WalkthroughRow[] {
  return rows.map((r) => ({
    line: r.line,
    open: lerp(r.from, r.to, e),
    sign: e < 0.5 ? r.signFrom : r.signTo,
  }))
}

/** Where a band on folding rows stands: the open lines before it, and its own. */
function bandOn(rows: readonly WalkthroughRow[], span: Span): WalkthroughBand {
  let from = 0
  for (let i = 0; i < span.first; i++) from += (rows[i] as WalkthroughRow).open
  let size = 0
  for (let i = span.first; i < span.first + span.size; i++)
    size += (rows[i] as WalkthroughRow).open
  return { from, size }
}

/** The bands of a change at `e`, with the line the view holds. */
function changeAt(
  change: NonNullable<WalkthroughPlan["change"]>,
  e: number,
): { rows: WalkthroughRow[]; bands: WalkthroughBand[]; focus: number } {
  const rows = rowsAt(change.rows, e)
  const bands = change.bands.map((s) => bandOn(rows, s))
  // The view stays on the line it held while lines fold above it, and eases to the
  // lines that change.
  const held =
    bandOn(rows, { first: change.anchor, size: 0 }).from + change.held
  const focus = change.changed
    ? lerp(
        held,
        (() => {
          const c = change.changed
          const span = bandOn(rows, {
            first: c.first,
            size: c.last - c.first + 1,
          })
          return span.from + span.size / 2
        })(),
        e,
      )
    : held
  return { rows, bands, focus }
}

/** The keys' frame at `ms` from the first key. */
function keysAt(
  keys: NonNullable<WalkthroughPlan["keys"]>,
  ms: number,
): {
  rows: WalkthroughRow[]
  bands: WalkthroughBand[]
  focus: number
  caret: { row: number; column: number } | null
  selection: WalkthroughPane["selection"]
} {
  const t = keys.timeline
  const f = codeFrameAt(t, (t.starts[1] as number) + ms)
  const doc = t.chars[keys.path] as { ch: string; born: number; died: number }[]
  const now = (t.starts[1] as number) + ms
  const rows: WalkthroughRow[] = []
  const state = keys.states[f.state]
  if (state === undefined)
    throw new Error("walkthrough-session: keys coloured by no state")
  let start = 0
  for (const line of f.text.split("\n")) {
    rows.push({
      line: {
        text: line,
        state,
        at: f.origin.slice(start, start + line.length),
      },
      open: 1,
      sign: null,
    })
    start += line.length + 1
  }
  // A row of the typed file: the lines standing before a place in it (landed, and
  // not yet typed over).
  const rowAt = (place: number) => {
    let row = 0
    for (let i = 0; i < place; i++) {
      const c = doc[i] as { ch: string; born: number; died: number }
      if (c.ch === "\n" && c.born <= now && now < c.died) row++
    }
    return row
  }
  const before = keys.base
  const lineStarts = linesWithStarts(
    doc
      .filter((c) => c.born < 0)
      .map((c) => c.ch)
      .join(""),
  )
  const bands = keys.bands.map((s) => {
    const a = lineStarts[s.first]
    if (!a) throw new Error("walkthrough-session: a band past the typed file")
    const from = rowAt(before[a.at] as number)
    if (s.size === 0) return { from, size: 0 }
    const z = lineStarts[s.first + s.size - 1] as { text: string; at: number }
    return {
      from,
      size: rowAt(before[z.at + z.text.length] as number) - from + 1,
    }
  })
  const caret = placeOf(f.text, f.caret)
  const focus = lerp(
    keys.focus,
    caret.line - 0.5,
    ease(clamp01(ms / FOLLOW_MS)),
  )
  return {
    rows,
    bands,
    focus,
    // None at the keys' first instant: that frame is also the step before's rest
    // (steps meet at their edges), and a step at rest shows no caret.
    caret: ms > 0 ? { row: caret.line - 1, column: caret.column - 1 } : null,
    selection: f.selection ? piecesOf(f.text, f.selection) : [],
  }
}

/** A range of `text` (offsets, `to` excluded) as a piece per row it covers. */
function piecesOf(
  text: string,
  range: { from: number; to: number },
): WalkthroughPane["selection"] {
  const a = placeOf(text, range.from)
  const b = placeOf(text, range.to)
  const lines = text.split("\n")
  const out: WalkthroughPane["selection"] = []
  for (let line = a.line; line <= b.line; line++)
    out.push({
      row: line - 1,
      from: line === a.line ? a.column - 1 : 0,
      to: line === b.line ? b.column - 1 : (lines[line - 1] as string).length,
    })
  return out
}

/** The bands travelling from `a` to `b` at `e`: paired in order, one without a pair
 *  growing from (or shrinking to) the middle of its own lines. */
function travel(
  a: readonly WalkthroughBand[],
  b: readonly WalkthroughBand[],
  e: number,
): WalkthroughBand[] {
  const out: WalkthroughBand[] = []
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const x = a[i] ?? b[i]
    const y = b[i] ?? a[i]
    if (!x || !y) continue
    const from = a[i] ?? { from: y.from + y.size / 2, size: 0 }
    const to = b[i] ?? { from: x.from + x.size / 2, size: 0 }
    out.push({
      from: lerp(from.from, to.from, e),
      size: lerp(from.size, to.size, e),
    })
  }
  return out
}

const sameBands = (
  a: readonly WalkthroughBand[],
  b: readonly WalkthroughBand[],
) =>
  a.length === b.length &&
  a.every((x, i) => x.from === b[i]?.from && x.size === b[i]?.size)

const rested = (s: Settled): WalkthroughRow[] =>
  s.rows.map((r) => ({ line: r.line, open: 1, sign: r.sign }))

/** Times a walkthrough. THROWS on a step that cannot happen: a first step with no
 *  file to show its marks on, a file shown first with no text, marks past the file,
 *  keys with `diff`, a hold below zero, and every throw of code-session's keys. */
export function walkthroughTimeline<Words, View>(
  steps: readonly WalkthroughStep<Words, View>[],
): WalkthroughTimeline<Words, View> {
  if (!steps.length) throw new Error("walkthrough-session: no steps")
  const states: WalkthroughState[] = []
  const known = new Map<string, number>()
  const stateOf = (path: string, lang: string, text: string) => {
    const key = `${path}\n${lang}\n${text}`
    let n = known.get(key)
    if (n === undefined) {
      n = states.length
      states.push({ path, lang, text })
      known.set(key, n)
    }
    return n
  }
  const files = new Map<string, { lang: string; settled: Settled }>()
  let pane = null as Pane | null
  const plans: WalkthroughPlan[] = []
  const starts: number[] = []
  const ends: number[] = []
  let lines = 0
  let at = 0
  for (const step of steps) {
    if ((step.hold ?? 0) < 0)
      throw new Error(
        `walkthrough-session: "${step.title}" holds ${step.hold} ms`,
      )
    const read = step.hold ?? READ_MS
    if ((step.play ?? 0) < 0)
      throw new Error(
        `walkthrough-session: "${step.title}" plays ${step.play} ms`,
      )
    const play = step.play ?? PLAY_MS
    const path: string | null = step.file ?? pane?.path ?? null
    starts.push(at)
    if (path === null) {
      if (step.text !== undefined || step.type || step.marks || step.diff)
        throw new Error(
          `walkthrough-session: "${step.title}" changes a file, and no step has named one`,
        )
      plans.push({
        path: null,
        lang: "text",
        leaving: null,
        change: null,
        keys: null,
        written: [],
        before: [],
        bands: [],
        focusBefore: 0,
        focus: 0,
        settled: { text: "", rows: [], real: [] },
        phase: { change: 0, keys: 0, marks: 0, play, read },
      })
      at += play + read
      ends.push(at)
      continue
    }
    if (step.type && step.diff)
      throw new Error(
        `walkthrough-session: "${step.title}" types keys and draws a diff; a diff is a state's change (text), keys are typed`,
      )
    const last = files.get(path)
    const lang: string = step.lang ?? last?.lang ?? langOf(path)
    if (!last && step.text === undefined && !step.type)
      throw new Error(
        `walkthrough-session: "${step.title}" shows ${path} for the first time, with no text`,
      )
    const text = step.text ?? last?.settled.text ?? ""
    const state = stateOf(path, lang, text)
    const swap = pane !== null && pane.path !== path
    const here = swap || pane === null ? null : pane
    // The change: the rows on screen folding into the step's text.
    let change: WalkthroughPlan["change"] = null
    let settled: Settled
    let bands: WalkthroughBand[]
    let focus: number
    if (here) {
      const f = fold(here.settled, text, state, step.diff ?? false)
      const moves = f.rows.some(
        (r) => r.from !== r.to || r.signFrom !== r.signTo,
      )
      settled = folded(f.rows, text)
      if (moves) {
        const anchorRow = Math.min(
          here.settled.rows.length - 1,
          Math.max(0, Math.floor(here.focus)),
        )
        const moving = f.rows.flatMap((r, i) =>
          r.from !== r.to || r.signTo !== r.signFrom ? [i] : [],
        )
        change = {
          rows: f.rows,
          bands: here.bands.map((b) => spanOf(b, f.at)),
          anchor: f.at[anchorRow] as number,
          held: here.focus - anchorRow,
          changed: moving.length
            ? { first: moving[0] as number, last: moving.at(-1) as number }
            : null,
        }
        const end = changeAt(change, 1)
        bands = end.bands
        focus = end.focus
      } else {
        bands = here.bands
        focus = here.focus
      }
    } else {
      // Shown fresh: the last step's file fades out and this one in. A diff
      // against the text it last showed is drawn at rest.
      settled =
        last && step.diff
          ? folded(fold(last.settled, text, state, true).rows, text)
          : settle(text, state)
      bands = []
      focus = 0
    }
    lines = Math.max(lines, settled.rows.length)
    // The keys, typed into the step's text.
    let keys: WalkthroughPlan["keys"] = null
    if (step.type?.length) {
      const timeline = codeTimeline(
        [{ path, text }],
        [
          { kind: "open", path },
          ...step.type.map((k): CodeStep => {
            if (k.replace === undefined) return { kind: "type", ...k }
            if (k.line === undefined || k.column === undefined)
              throw new Error(
                `walkthrough-session: keys replacing ${k.replace} characters need their line and column`,
              )
            return {
              kind: "replace",
              line: k.line,
              column: k.column,
              length: k.replace,
              text: k.text,
            }
          }),
        ],
      )
      const doc = timeline.chars[path] as { born: number }[]
      const base: number[] = []
      doc.forEach((c, i) => {
        if (c.born < 0) base.push(i)
      })
      base.push(doc.length)
      const typed = timeline.final[path] as string
      const s = stateOf(path, lang, typed)
      keys = {
        timeline,
        path,
        from: timeline.starts[1] as number,
        states: timeline.states.map((x) => stateOf(path, lang, x.text)),
        base,
        bands: bands.map((b) => spanOf(b, settled.real)),
        focus,
      }
      const done = (timeline.ends.at(-1) as number) - keys.from
      const end = keysAt(keys, done)
      bands = end.bands
      focus = end.focus
      settled = settle(typed, s)
      lines = Math.max(lines, settled.rows.length)
    }
    const target = bandsOf(step.marks, settled, step.title)
    // A file shown fresh opens on its marks, unless keys have moved the view.
    if (!here && !keys && target.length) focus = middle(target)
    const before = bands
    const focusBefore = focus
    const marks = sameBands(before, target) ? 0 : MARK_MS
    const keysMs = keys ? (keys.timeline.ends.at(-1) as number) - keys.from : 0
    const changeMs = swap || change ? CHANGE_MS : 0
    const leaving = swap ? pane : null
    pane = {
      path,
      lang,
      settled,
      bands: target,
      focus: target.length ? middle(target) : focus,
    }
    files.set(path, { lang, settled })
    // The file as written: its text once the change is made, then again as each keys
    // entry is done, as an editor writes a file after each change.
    const written = [{ at: 0, text }]
    if (keys) {
      const t = keys.timeline
      const doc = t.chars[path] as { ch: string; born: number; died: number }[]
      for (let g = 1; g < t.ends.length; g++) {
        const end = t.ends[g] as number
        written.push({
          at: changeMs + end - keys.from,
          text: doc
            .filter((c) => c.born <= end && end < c.died)
            .map((c) => c.ch)
            .join(""),
        })
      }
    }
    plans.push({
      path,
      lang,
      leaving,
      change,
      keys,
      written,
      before,
      bands: target,
      focusBefore,
      focus: pane.focus,
      settled,
      phase: {
        change: changeMs,
        keys: keysMs,
        marks,
        // Nothing moving in the file: the view plays on its own clock, for `play`.
        play: changeMs + keysMs + marks > 0 ? 0 : play,
        read,
      },
    })
    at +=
      changeMs +
      keysMs +
      marks +
      (changeMs + keysMs + marks > 0 ? 0 : play) +
      read
    ends.push(at)
  }
  const chapters: Chapter[] = steps.map((s, i) => ({
    start: starts[i] as number,
    title: s.title,
  }))
  return {
    steps,
    plans,
    starts,
    ends,
    total: at,
    states,
    lines,
    playable: { duration: at, chapters },
  }
}

/** The step on at `ms`. */
function stepAt(t: WalkthroughTimeline, ms: number): number {
  let i = 0
  while (i + 1 < t.starts.length && (t.starts[i + 1] as number) <= ms) i++
  return i
}

/** The frame at `ms`: the step on, its file as each phase leaves it so far. */
export function walkthroughFrameAt(
  t: WalkthroughTimeline,
  ms: number,
): WalkthroughFrame {
  const step = stepAt(t, Math.min(t.total, Math.max(0, ms)))
  const plan = t.plans[step] as WalkthroughPlan
  const local = Math.max(0, ms - (t.starts[step] as number))
  const { change, keys, marks } = plan.phase
  const play = change + keys + marks + plan.phase.play
  const p = clamp01(local / play)
  const arrive = ease(clamp01(local / ARRIVE_MS))
  if (plan.path === null) return { step, p, arrive, pane: null, written: null }
  const path = plan.path
  const last = plan.written.findLast((w) => w.at <= local) ?? plan.written[0]
  if (!last) throw new Error("walkthrough-session: a step with no file written")
  const written = { path, text: last.text }
  const frame = (pane: WalkthroughPane): WalkthroughFrame => ({
    step,
    p,
    arrive,
    pane,
    written,
  })
  const pane = (
    rows: WalkthroughRow[],
    bands: WalkthroughBand[],
    focus: number,
    caret: WalkthroughPane["caret"] = null,
    opacity = 1,
    selection: WalkthroughPane["selection"] = [],
  ): WalkthroughPane => ({
    path,
    lang: plan.lang,
    rows,
    bands,
    focus,
    caret,
    selection,
    opacity,
    source: plan.settled.text,
  })
  if (local < change) {
    const e = ease(local / change)
    if (plan.leaving) {
      // Out, then in: the first half fades the last file, the second this one.
      if (e < 0.5) {
        const l = plan.leaving
        return frame({
          path: l.path,
          lang: l.lang,
          rows: rested(l.settled),
          bands: l.bands,
          focus: l.focus,
          caret: null,
          selection: [],
          opacity: 1 - 2 * e,
          source: l.settled.text,
        })
      }
      const first = plan.keys
        ? keysAt(plan.keys, 0)
        : { rows: rested(plan.settled), bands: [], focus: plan.focusBefore }
      return frame(pane(first.rows, first.bands, first.focus, null, 2 * e - 1))
    }
    if (!plan.change)
      throw new Error("walkthrough-session: a change with nothing to fold")
    const c = changeAt(plan.change, e)
    return frame(pane(c.rows, c.bands, c.focus))
  }
  if (plan.keys && local < change + keys) {
    const k = keysAt(plan.keys, local - change)
    return frame(pane(k.rows, k.bands, k.focus, k.caret, 1, k.selection))
  }
  const rows = rested(plan.settled)
  if (local < change + keys + marks) {
    const e = ease((local - change - keys) / marks)
    const bands = travel(plan.before, plan.bands, e)
    return frame(pane(rows, bands, lerp(plan.focusBefore, plan.focus, e)))
  }
  return frame(pane(rows, plan.bands, plan.focus))
}

/** The progress (0..1) at which step `index` rests, played: its last frame, what a
 *  still of that step draws. */
export function walkthroughRest(t: WalkthroughTimeline, index: number): number {
  const end = t.ends[index]
  if (end === undefined)
    throw new Error(
      `walkthroughRest: step ${index} outside 0..${t.ends.length - 1}`,
    )
  return Math.max(0, end - 1) / t.total
}
