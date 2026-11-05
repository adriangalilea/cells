// AN ACT, timed: what it draws at an instant of its show. The beats run one after another
// (lib/story `StoryBeat`), each leaving its effect for the next: the text typed stays
// typed, the table once arrived stays, the light once lit stays lit, a mark stays until
// another takes its place. Nothing is marked before the beat that marks it. After the
// last beat, a settle, then the told line. A pure function of the instant, so a played
// act, a scrubbed one and a still are the same frames. Framework-free.

import type { StoryAct, StoryBeat, StoryEdit, StoryLit } from "@/lib/story"

/** The pacing, one table: an act's time is decided here and nowhere else. */
const TYPE_MS = 55
const REMOVE_MS = 45
/** A mark held, to be seen, after keys or on its own. */
const HOLD_MS = 900
const TABLE_MS = 600
const LIGHT_MS = 900
const SETTLE_MS = 400
const TELL_MS = 600

export interface ActTimeline {
  act: StoryAct
  /** Each beat's start, ms. */
  starts: number[]
  /** When the words begin to arrive, and the whole. */
  tell: number
  total: number
}

export interface ActFrame {
  name: string
  /** The marked characters of the name, [from, to); null when it is not marked. */
  nameBand: [number, number] | null
  text: string
  /** The marked characters, [from, to) offsets into `text`; null when nothing is. */
  band: [number, number] | null
  /** Where keys are landing: in the name or the text. */
  caret: { in: "name" | "text"; at: number } | null
  /** The table, there 0..1. */
  table: number
  /** What the table lights, and how far (0..1). */
  lit: { at: StoryLit; amount: number } | null
  /** The told line, 0..1. */
  tell: number
}

/** A rename as keys: the old name's end taken away after what the two share, the new
 *  name's end typed. */
function renameEdit(from: string, to: string): StoryEdit {
  let p = 0
  while (p < from.length && from[p] === to[p]) p++
  return { after: from.slice(0, p), remove: from.length - p, insert: to.slice(p) }
}

const keysMs = (edits: readonly StoryEdit[]) => edits.reduce((n, e) => n + (e.remove ?? 0) * REMOVE_MS + e.insert.length * TYPE_MS, 0)

/** The names the act's file has, beat by beat: what each rename starts from. */
function namesOf(act: StoryAct): string[] {
  let name = act.name
  return act.beats.map((b) => {
    const was = name
    if ("rename" in b) name = b.rename
    return was
  })
}

function beatMs(b: StoryBeat, name: string): number {
  if ("see" in b) return b.see
  if ("rename" in b) return keysMs([renameEdit(name, b.rename)])
  if ("type" in b) return keysMs(b.type) + HOLD_MS
  if ("table" in b) return TABLE_MS
  if ("light" in b) return LIGHT_MS
  return HOLD_MS
}

export function actTimeline(act: StoryAct): ActTimeline {
  const names = namesOf(act)
  const starts: number[] = []
  let at = 0
  for (const [i, b] of act.beats.entries()) {
    starts.push(at)
    at += beatMs(b, names[i] as string)
  }
  const tell = at + SETTLE_MS
  return { act, starts, tell, total: tell + TELL_MS }
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

export function actFrameAt(t: ActTimeline, ms: number): ActFrame {
  const act = t.act
  const f: ActFrame = { name: act.name, nameBand: null, text: act.text, band: null, caret: null, table: act.table ? 1 : 0, lit: null, tell: ease((ms - t.tell) / TELL_MS) }
  for (const [i, b] of act.beats.entries()) {
    const start = t.starts[i] as number
    if (ms < start) break
    const into = ms - start
    if ("rename" in b) {
      const edit = renameEdit(f.name, b.rename)
      const typed = typing(f.name, [edit], into)
      if (typed) {
        f.name = typed.text
        f.nameBand = typed.band
        f.band = null
        f.caret = { in: "name", at: typed.caret }
      } else {
        f.name = b.rename
        f.nameBand = null
        f.caret = null
      }
    } else if ("type" in b) {
      const typed = typing(f.text, b.type, into)
      f.nameBand = null
      if (typed) {
        f.text = typed.text
        f.band = typed.band
        f.caret = { in: "text", at: typed.caret }
      } else {
        // The same edits whole: what `typing` ends on.
        let text = f.text
        for (const e of b.type) {
          const at = text.indexOf(e.after) + e.after.length
          text = text.slice(0, at) + e.insert + text.slice(at + (e.remove ?? 0))
        }
        f.text = text
        f.band = rangeOf(text, b.mark)
        f.caret = null
      }
    } else if ("mark" in b && !("light" in b)) {
      f.band = rangeOf(f.text, b.mark)
    } else if ("table" in b) {
      f.table = ease(into / TABLE_MS)
    } else if ("light" in b) {
      f.lit = { at: b.light, amount: ease(into / 500) }
      if (b.mark) f.band = rangeOf(f.text, b.mark)
    }
  }
  return f
}
