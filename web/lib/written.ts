// A FILE AS AN EDIT LEAVES IT, at an instant of ui's code-session: as typed, and as last
// written. The hero's Cells window and the story's read the second: an app watching the
// folder re-reads a file when the editor writes it, never while it is being typed.

import type { CodeTimeline } from "@/lib/code-session"

/** A file's text at an instant of the edit (ms on the edit's own clock): the characters
 *  standing then (typed, and not yet typed over), whether or not the file is in front. */
export function textAt(timeline: CodeTimeline, path: string, ms: number): string {
  const chars = timeline.chars[path]
  if (!chars) throw new Error(`written: no file ${path} in the edit`)
  let out = ""
  for (const c of chars) if (c.born <= ms && ms < c.died) out += c.ch
  return out
}

/** A file as it was last written by an instant of the edit (ms on its own clock). Before
 *  any save, the file as the edit found it. */
export function savedAt(timeline: CodeTimeline, path: string, ms: number): string {
  let front: string | null = null
  let written = -1
  for (const [i, s] of timeline.steps.entries()) {
    if (s.kind === "open") front = s.path
    if (s.kind === "save" && front === path && (timeline.ends[i] as number) <= ms) written = timeline.ends[i] as number
  }
  const initial = timeline.files.find((f) => f.path === path)
  if (!initial) throw new Error(`written: no file ${path} in the edit`)
  return written < 0 ? initial.text : textAt(timeline, path, written)
}
