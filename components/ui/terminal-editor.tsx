"use client"

// A MODAL EDITOR IN THE TERMINAL, helix's look: line numbers, the code highlighted,
// the caret a block in normal mode and a bar in insert, the status line with the
// mode (NOR, INS), the file and whether it is unsaved, and the command line where
// `:w` is typed and answered. It plays an edit on the playhead (lib/code-session,
// the same steps `ide` plays as VS Code: open, type at a place, save), and draws
// inside `terminal`'s window as the full-screen program it is:
//
//   <Terminal alt="…"><TerminalEditor files={…} steps={…} /></Terminal>
//
// It wears the terminal's phosphor (`useTerminalPalette`) for everything but the
// code, which takes code's dark theme (lib/code-tokens): a terminal is dark whatever
// the page is. Display-only, like the terminal around it.

import * as React from "react"
import {
  type CodeFile,
  type CodeStep,
  codeFrameAt,
  codeTimeline,
  placeOf,
} from "@/lib/code-session"
import { langOf, runsOf } from "@/lib/code-tokens"
import { LINE_HEIGHT } from "@/lib/terminal-session"
import { usePlayhead } from "@/components/ui/playback"
import { useTerminalPalette } from "@/components/ui/terminal"
import { useCodeColours } from "@/components/ui/use-code-colours"

export interface TerminalEditorProps {
  /** The files the editor can open (`/` between folders). */
  files: CodeFile[]
  steps: CodeStep[]
  /** The instant drawn, 0..1 of the edit, for a still. Inside a driver (a Player)
   *  omit it: the driver provides it. */
  progress?: number
  /** The clock's length when the edit shares it with something longer, ms: the
   *  last frame holds to it (lib/code-session `codeTimeline`). */
  duration?: number
  /** Rows of text the screen shows, the status and command lines under them. The
   *  view follows the caret, a few rows short of the foot, as helix scrolls. */
  rows?: number
}

/** How the status line names each mode, helix's three letters. */
const MODES = { normal: "NOR", insert: "INS", command: "NOR" } as const
/** Rows helix keeps between the caret and the screen's foot. */
const SCROLL_OFF = 3

/** An edit played in the terminal: the frame at the instant drawn as helix. Its
 *  clip, for the driver: `codeTimeline(files, steps).clip`. */
export function TerminalEditor({
  files,
  steps,
  progress,
  duration,
  rows = 14,
}: TerminalEditorProps) {
  const palette = useTerminalPalette()
  const timeline = React.useMemo(
    () => codeTimeline(files, steps, duration),
    [files, steps, duration],
  )
  const ms = usePlayhead(progress).at * timeline.total
  const f = codeFrameAt(timeline, ms)
  const path = f.active ?? "[scratch]"
  const colours = useCodeColours(
    f.active ? (timeline.final[f.active] as string) : "",
    langOf(path),
  )
  const lines = runsOf(f.text, colours, f.origin)
  const place = placeOf(f.text, f.caret)
  const top = Math.max(0, place.line - rows + SCROLL_OFF)
  const digits = Math.max(3, String(lines.length).length)
  const insert = f.mode === "insert"
  // The command line: what is typed into it, else helix's answer to the last save
  // (`'index.html' written, 12L 380B`) while that file is in front.
  const message = f.command
    ? f.command
    : f.saved && f.saved.path === f.active
      ? `'${f.saved.path}' written, ${f.text.replace(/\n$/, "").split("\n").length}L ${new TextEncoder().encode(f.text).length}B`
      : ""
  return (
    <div
      data-slot="terminal-editor"
      className="whitespace-pre"
      style={{ color: palette.command }}
    >
      <div data-slot="terminal-editor-text" style={{ paddingTop: "0.5em" }}>
        {Array.from({ length: rows }, (_, r) => {
          const i = top + r
          const runs = lines[i]
          const here = i + 1 === place.line
          return (
            <div
              // biome-ignore lint/suspicious/noArrayIndexKey: a screen row
              key={r}
              data-slot="terminal-editor-line"
              className="flex overflow-hidden"
              style={{ height: `${LINE_HEIGHT}em` }}
            >
              <span
                data-slot="terminal-editor-number"
                className="shrink-0 pr-[1ch] text-right"
                style={{
                  width: `${digits + 2}ch`,
                  color: here ? palette.accent : palette.prompt,
                }}
              >
                {runs ? i + 1 : ""}
              </span>
              <span className="relative min-w-0 flex-1">
                {runs?.map((run, k) => (
                  <span
                    // biome-ignore lint/suspicious/noArrayIndexKey: runs are positional
                    key={k}
                    style={
                      run.ink
                        ? {
                            color: run.ink.dark,
                            fontStyle: run.ink.italic ? "italic" : undefined,
                            fontWeight: run.ink.bold ? 700 : undefined,
                          }
                        : undefined
                    }
                  >
                    {run.text}
                  </span>
                ))}
                {here && (
                  <span
                    data-slot="terminal-editor-caret"
                    className="absolute top-0 h-full"
                    style={{
                      left: `${place.column - 1}ch`,
                      width: insert ? "2px" : "1ch",
                      background: palette.accent,
                      mixBlendMode: insert ? undefined : "difference",
                    }}
                  />
                )}
              </span>
            </div>
          )
        })}
      </div>
      <div
        data-slot="terminal-editor-status"
        className="flex gap-[1ch]"
        style={{ background: palette.bar }}
      >
        <span
          data-slot="terminal-editor-mode"
          className="px-[1ch] font-bold"
          style={{
            background: insert ? palette.command : palette.accent,
            color: palette.bg,
          }}
        >
          {MODES[f.mode]}
        </span>
        <span className="min-w-0 flex-1 truncate">
          {path}
          {f.active && f.dirty.includes(f.active) ? " [+]" : ""}
        </span>
        <span className="pr-[1ch]" style={{ color: palette.muted }}>
          1 sel {place.line}:{place.column}
        </span>
      </div>
      <div
        data-slot="terminal-editor-command"
        className="px-[1ch]"
        style={{ color: palette.muted }}
      >
        {message || " "}
      </div>
    </div>
  )
}
