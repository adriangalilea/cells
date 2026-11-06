// A TABLE WORKED IN BY HAND, as data: what a person does in an app's tables (a window
// brought to the front, a cell chosen, keys typed into it and Enter), on one clock, and
// what every window shows at any instant of it. table's `scene` draws the hand (its
// cursor, the text being typed), and each file's text is the one its last commit
// wrote. A commit is written by the consumer's own engine (`write`: a Cells file through
// its engine, which stores what the commit reaches), worked out once when the timeline
// is made, so a frame is a lookup and a still at any `progress` is the live frame.
//
//   const t = tableTimeline(files, steps, write)
//   const f = tableFrameAt(t, ms)   // f.texts[path], f.active, f.scene(path)
//
// Framework-free.

import type { Chapter, Playable } from "@/lib/playable"

/** A file a window shows: its path and its text. */
export interface TableFile {
  path: string
  text: string
}

/** A cell of a file's table: its data row (from 0, the first under the header; one past
 *  the last is a new row) and its column by name. */
export interface TableCell {
  row: number
  column: string
}

/** What the hand does, in order, each after `delay` ms (or none). */
export type TableStep =
  /** A window brought to the front. */
  | { kind: "open"; path: string; delay?: number }
  /** The cursor to a cell of the window in front. */
  | { kind: "select"; row: number; column: string; delay?: number }
  /** Keys typed into the cursor's cell, then Enter: the cell is committed (`write`). */
  | { kind: "type"; text: string; delay?: number }

/** A commit, as the consumer writes it: `file`'s text with `value` in `cell`, read with
 *  the other files as they stand (a reference to one of them checked against it). */
export type TableWrite = (
  file: TableFile,
  cell: TableCell,
  value: string,
  others: readonly TableFile[],
) => string

/** How long the hand takes, ms: a window brought up, the cursor moved, a key, the
 *  Enter that commits. */
export const TABLE_MS = {
  open: 350,
  select: 300,
  key: 70,
  commit: 220,
} as const

/** Each step, planned: when it starts and ends, and what it leaves. */
interface Planned {
  step: TableStep
  start: number
  end: number
  /** A select's cell, in the file in front. */
  cell?: TableCell & { path: string }
  /** A type's text written, at `end`: every file's text after the commit. */
  texts?: Record<string, string>
}

export interface TableTimeline {
  files: readonly TableFile[]
  plans: Planned[]
  /** ms */
  total: number
  /** The session as a playable: one chapter per thing done. */
  playable: Playable
}

/** The hand's steps on `files`, timed, each commit written by `write`. The first file
 *  is in front until a step opens another. A step on a file no window shows, a type
 *  with no cell chosen, throws. */
export function tableTimeline(
  files: readonly TableFile[],
  steps: readonly TableStep[],
  write: TableWrite,
): TableTimeline {
  const first = files[0]
  if (!first) throw new Error("table-session: no file to work in")
  const texts: Record<string, string> = Object.fromEntries(
    files.map((f) => [f.path, f.text]),
  )
  let front = first.path
  let cell: (TableCell & { path: string }) | null = null
  let at = 0
  const plans: Planned[] = []
  const chapters: Chapter[] = []
  for (const step of steps) {
    const start = at + (step.delay ?? 0)
    let end = start
    const plan: Planned = { step, start, end }
    switch (step.kind) {
      case "open":
        if (!(step.path in texts))
          throw new Error(
            `table-session: ${step.path} is no file of the session`,
          )
        front = step.path
        cell = null
        end = start + TABLE_MS.open
        chapters.push({ start, title: step.path })
        break
      case "select":
        cell = { path: front, row: step.row, column: step.column }
        plan.cell = cell
        end = start + TABLE_MS.select
        break
      case "type": {
        if (!cell)
          throw new Error("table-session: keys typed with no cell chosen")
        const path = cell.path
        const others = files
          .filter((f) => f.path !== path)
          .map((f) => ({ path: f.path, text: texts[f.path] as string }))
        texts[path] = write(
          { path, text: texts[path] as string },
          cell,
          step.text,
          others,
        )
        plan.texts = { ...texts }
        end = start + [...step.text].length * TABLE_MS.key + TABLE_MS.commit
        chapters.push({ start, title: step.text, code: true })
        break
      }
    }
    plan.end = end
    plans.push(plan)
    at = end
  }
  return { files, plans, total: at, playable: { duration: at, chapters } }
}

/** What the windows show at `ms`: the file in front, every file's text as its last
 *  commit wrote it, and the hand in the front window (its cursor, and the keys typed so
 *  far into a cell not yet committed). */
export interface TableFrame {
  active: string
  texts: Record<string, string>
  hand: { path: string; cell: TableCell; draft: string | null } | null
}

export function tableFrameAt(t: TableTimeline, ms: number): TableFrame {
  const texts: Record<string, string> = Object.fromEntries(
    t.files.map((f) => [f.path, f.text]),
  )
  let active = (t.files[0] as TableFile).path
  let hand: TableFrame["hand"] = null
  for (const p of t.plans) {
    if (ms < p.start) break
    const s = p.step
    if (s.kind === "open") {
      active = s.path
      hand = null
    } else if (s.kind === "select" && p.cell) {
      const { path, ...cell } = p.cell
      hand = { path, cell, draft: null }
    } else if (s.kind === "type" && hand) {
      const at: NonNullable<TableFrame["hand"]> = hand
      if (ms >= p.end - TABLE_MS.commit && p.texts) {
        Object.assign(texts, p.texts)
        hand = { ...at, draft: null }
      } else {
        const keys = Math.floor((ms - p.start) / TABLE_MS.key) + 1
        hand = { ...at, draft: [...s.text].slice(0, keys).join("") }
      }
    }
  }
  return { active, texts, hand }
}

/** The hand as `path`'s window draws it (table's `scene`), or none where it is not. */
export function sceneOf(
  frame: TableFrame,
  path: string,
): { cursor: TableCell | null; draft: string | null } | null {
  return frame.hand && frame.hand.path === path
    ? { cursor: frame.hand.cell, draft: frame.hand.draft }
    : null
}
