// A STORY STEP, timed: what it draws at an instant of its arrival. In order: the step
// arrives; the spot it is about is marked, everything else dimmed; a rename is typed into
// the file's name; the keys land in the text (a character at a time, taken away then
// typed); the mark moves onto what changed; the table arrives if this is its step; the
// table lights what it did; the told line arrives. A pure function of the instant, so a
// played step, a scrubbed one and a still are the same frames. Framework-free.

import type { StoryEdit, StoryStep } from "@/lib/story"

/** The pacing, one table: a step's time is decided here and nowhere else. */
const ARRIVE_MS = 400
const SPOT_MS = 900
const TYPE_MS = 55
const REMOVE_MS = 45
const SETTLE_MS = 350
const TABLE_MS = 600
const LIGHT_MS = 700
const TELL_MS = 600

export interface SceneTimeline {
  step: StoryStep
  /** The rename's keys, then the text's, ms. */
  rename: { from: number; to: number }
  keys: { from: number; to: number }
  /** The table arriving (its own step only), the light, the words, ms. */
  table: number
  light: number
  tell: number
  total: number
}

export interface SceneFrame {
  name: string
  /** The marked characters of the name, [from, to); null when it is not marked. */
  nameBand: [number, number] | null
  /** The text at the instant. */
  text: string
  /** The marked characters, [from, to) offsets into `text`; null when nothing is marked. */
  band: [number, number] | null
  /** Everything but the band dimmed, 0..1. */
  dim: number
  /** The table, there 0..1 (1 once it has arrived). */
  table: number
  /** The table's lit column or cell, 0..1. */
  lit: number
  /** The told line, 0..1. */
  tell: number
  /** Where keys are landing: in the name or the text. */
  caret: { in: "name" | "text"; at: number } | null
}

/** A rename as keys: the old name's end taken away after what the two share, the new
 *  name's end typed. */
function renameEdit(step: StoryStep): StoryEdit | null {
  const { before, after } = step.name
  if (before === after) return null
  let p = 0
  while (p < before.length && before[p] === after[p]) p++
  return { after: before.slice(0, p), remove: before.length - p, insert: after.slice(p) }
}

const keysMs = (edits: readonly StoryEdit[]) => edits.reduce((n, e) => n + (e.remove ?? 0) * REMOVE_MS + e.insert.length * TYPE_MS, 0)

export function sceneTimeline(step: StoryStep): SceneTimeline {
  const rename = renameEdit(step)
  const renameFrom = ARRIVE_MS + SPOT_MS
  const renameTo = renameFrom + (rename ? keysMs([rename]) : 0)
  // After a rename the spot in the text is marked for its own look before the keys.
  const from = rename ? renameTo + SPOT_MS : renameFrom
  const to = from + keysMs(step.edits)
  const settled = step.edits.length || rename ? to + SETTLE_MS : to
  const table = settled
  const light = step.table === "arrives" ? table + TABLE_MS : settled
  const tell = light + LIGHT_MS
  return { step, rename: { from: renameFrom, to: renameTo }, keys: { from, to }, table, light, tell, total: tell + TELL_MS }
}

const ease = (t: number) => {
  const c = Math.min(1, Math.max(0, t))
  return c * c * (3 - 2 * c)
}

/** The range of `needle` in `text`, screaming when it is not there. */
function rangeOf(text: string, needle: string): [number, number] {
  const i = text.indexOf(needle)
  if (i < 0) throw new Error(`scene: "${needle}" is not in the text it marks`)
  return [i, i + needle.length]
}

/** `edits` typed into `text` for `ms`: the text so far, and the edit under way (what is
 *  left to take away, then what has been typed of it), or null once all are typed. */
function typing(text: string, edits: readonly StoryEdit[], ms: number): { text: string; band: [number, number]; caret: number } | null {
  let out = text
  let left = ms
  for (const e of edits) {
    const at = out.indexOf(e.after) + e.after.length
    const removing = e.remove ?? 0
    const removed = Math.min(removing, Math.floor(left / REMOVE_MS))
    left -= removed * REMOVE_MS
    const typed = removed < removing ? 0 : Math.min(e.insert.length, Math.floor(left / TYPE_MS))
    left -= typed * TYPE_MS
    out = out.slice(0, at) + e.insert.slice(0, typed) + out.slice(at + removed)
    if (removed < removing) return { text: out, band: [at, at + removing - removed], caret: at }
    if (typed < e.insert.length) return { text: out, band: [at, at + typed], caret: at + typed }
  }
  return null
}

export function sceneFrameAt(t: SceneTimeline, ms: number): SceneFrame {
  const s = t.step
  const dim = ease((ms - ARRIVE_MS) / 400)
  const table = s.table === "none" ? 0 : s.table === "shown" ? 1 : ease((ms - t.table) / TABLE_MS)
  const lit = ease((ms - t.light) / 500)
  const tell = ease((ms - t.tell) / TELL_MS)
  const arrived = ms >= ARRIVE_MS
  const base = { dim, table, lit, tell }
  const done = { name: s.name.after, nameBand: null, text: s.text.after, band: rangeOf(s.text.after, s.mark), caret: null }
  if (!s.spot) return { ...base, ...done, band: arrived ? done.band : null }
  const rename = renameEdit(s)
  // The name first, while the text waits dimmed: its old end marked, then retyped.
  if (rename && ms < t.rename.to) {
    const typed = ms < t.rename.from ? null : typing(s.name.before, [rename], ms - t.rename.from)
    return {
      ...base,
      name: typed?.text ?? s.name.before,
      nameBand: arrived ? (typed?.band ?? [rename.after.length, s.name.before.length]) : null,
      text: s.text.before,
      band: null,
      caret: typed ? { in: "name", at: typed.caret } : null,
    }
  }
  if (ms < t.keys.from)
    return { ...base, name: s.name.after, nameBand: null, text: s.text.before, band: arrived ? rangeOf(s.text.before, s.spot) : null, caret: null }
  const typed = typing(s.text.before, s.edits, ms - t.keys.from)
  if (!typed) return { ...base, ...done }
  return { ...base, name: s.name.after, nameBand: null, text: typed.text, band: typed.band, caret: { in: "text", at: typed.caret } }
}
