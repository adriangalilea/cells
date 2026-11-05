"use client"

// HELIX'S DIAGNOSTICS over ui's terminal-editor: the engine's problems at the frame drawn,
// placed on their cells as helix draws a language server's (a mark in the gutter, the
// span undercurled, the message at the line's end). ui's terminal-editor has no
// diagnostics layer (for ui: `diagnostics` on TerminalEditor, these three marks drawn in
// its own rows); this lies over it with the same metrics, the same frame and the same
// scroll, so a mark is exactly on its characters.

import { usePlayhead } from "@/components/ui/playback"
import { useTerminalPalette } from "@/components/ui/terminal"
import { useEngine } from "@/components/use-engine"
import { readFiles } from "@/lib/cells"
import { type CodeTimeline, codeFrameAt, placeOf } from "@/lib/code-session"
import { textAt } from "@/lib/hero"
import { placeBroken, placeProblems } from "@/lib/spans"
import { LINE_HEIGHT } from "@/lib/terminal-session"

/** terminal-editor's own: rows kept between the caret and the foot, the gutter's least
 *  digits, the text's top padding. */
const SCROLL_OFF = 3
const DIGITS = 3
const PAD_TOP = 0.5

/** `others`: the session's other files, read as the front one's imports (as they stand at
 *  the instant drawn, saved or not: what a language server reads). */
export function HelixDiagnostics({ timeline, rows, others = [] }: { timeline: CodeTimeline; rows: number; others?: string[] }) {
  const palette = useTerminalPalette()
  const engine = useEngine()
  const ms = usePlayhead(undefined).at * timeline.total
  const f = codeFrameAt(timeline, ms)
  if (!f.active) return null
  const imports = others.filter((p) => p !== f.active).map((p) => ({ name: p, text: textAt(timeline, p, ms) }))
  const read = readFiles(engine, [{ name: f.active, text: f.text }, ...imports])
  const placed = [...placeBroken(f.text, read.broken), ...placeProblems(f.text, read.problems)]
  const lines = f.text.split("\n")
  const top = Math.max(0, placeOf(f.text, f.caret).line - rows + SCROLL_OFF)
  const gutter = Math.max(DIGITS, String(lines.length).length) + 2
  return (
    <div data-slot="helix-diagnostics" aria-hidden className="pointer-events-none absolute inset-0 overflow-hidden whitespace-pre">
      {placed.map((p, i) => {
        const row = p.line - 1 - top
        if (row < 0 || row >= rows) return null
        const line = lines[p.line - 1] ?? ""
        const y = `${PAD_TOP + row * LINE_HEIGHT}em`
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: one frame's problems, in order
          <div key={i} className="absolute inset-x-0" style={{ top: y, height: `${LINE_HEIGHT}em` }}>
            <span className="absolute top-0 left-0" style={{ color: "var(--haze)" }}>
              ●
            </span>
            <span
              className="absolute top-0 text-transparent underline decoration-wavy decoration-1 underline-offset-[3px]"
              style={{ left: `${gutter + p.from}ch`, textDecorationColor: "var(--haze)" }}
            >
              {line.slice(p.from, p.to)}
            </span>
            <span
              className="absolute top-0"
              style={{ left: `${gutter + line.length + 2}ch`, color: "var(--haze)", opacity: 0.85 }}
            >
              <span style={{ color: palette.muted }}>■ </span>
              {p.message}
            </span>
          </div>
        )
      })}
    </div>
  )
}
