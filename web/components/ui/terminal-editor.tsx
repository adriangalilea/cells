"use client"

// A MODAL EDITOR IN THE TERMINAL, helix's look: the bufferline of open files across
// the top, the diagnostics gutter and line numbers, the code highlighted, the caret a
// block in normal mode and a bar in insert, what the language server says (the range
// undercurled, the message at the line's end as helix's end-of-line diagnostics, the
// hover and the completion menu as helix's bordered popups, signature help, `space d`'s
// diagnostics picker), the status line with the mode (NOR, INS), the file, whether it
// is unsaved and the error and warning counts, and the command line where `:w` and
// `rename-to:` are typed and answered. It plays an edit on the playhead
// (lib/code-session, the same steps `ide` plays as VS Code), and draws inside
// `terminal`'s window as the full-screen program it is:
//
//   <Terminal alt="…"><TerminalEditor files={…} steps={…} /></Terminal>
//
// It wears the terminal's phosphor (`useTerminalPalette`) for everything but the
// code, which takes code's dark theme (lib/code-tokens), and the severities, which
// take the terminal's own red, yellow, blue and grey: a terminal is dark whatever
// the page is. Display-only, like the terminal around it, but for the bufferline's
// files when `onOpen` makes them pressable.

import * as React from "react"
import {
  type CodeDiagnose,
  type CodeFile,
  type CodePopup,
  type CodeSeverity,
  type CodeStep,
  codeFrameAt,
  codeTimeline,
  placeOf,
} from "@/lib/code-session"
import {
  type CodePaint,
  langOf,
  paintRuns,
  runsOf,
} from "@/lib/code-tokens"
import {
  ansiColor,
  LINE_HEIGHT,
} from "@/lib/terminal-session"
import { usePlayhead } from "@/components/ui/playback"
import { useTerminalPalette } from "@/components/ui/terminal"
import { useCodeColoursOf } from "@/components/ui/use-code-colours"

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
  /** The language server's diagnostics, run on each file with every file's text at
   *  the instant (lib/code-session `CodeDiagnose`): a problem one file causes in
   *  another shows, and goes when it is fixed there. */
  diagnose?: CodeDiagnose
  /** Rows of the screen above the status and command lines: the text's, and the
   *  bufferline's when it shows. The view follows the caret, a few rows short of
   *  the foot, as helix scrolls. */
  rows?: number
  /** Helix's `bufferline`: the open files across the top `always`, only while
   *  more than one is open (`multiple`, helix's choice when set), or `never`. */
  bufferline?: "always" | "multiple" | "never"
  /** Helix's `soft-wrap`: a line longer than the screen is drawn on as many rows as
   *  it needs, each row after the first behind helix's `↪ `, numbered once. The file
   *  is never changed: a line that must stay one line (a header) stays one. */
  softWrap?: boolean
  /** The bufferline's files pressable: a press asks for that file in front, and the
   *  page answers with the edit's next step (`{ kind: "open", path }` appended to
   *  `steps`), so what is drawn stays the edit's frame. Absent, display-only. */
  onOpen?: (path: string) => void
}

/** How the status line names each mode, helix's three letters: a selection made
 *  to be changed is normal mode's. */
const MODES = {
  normal: "NOR",
  insert: "INS",
  select: "NOR",
  command: "NOR",
} as const
/** Rows helix keeps between the caret and the screen's foot. */
const SCROLL_OFF = 3
/** Helix's soft-wrap: the indicator a wrapped row opens with, and the longest
 *  word it moves whole to the next row rather than breaking (`max-wrap`). */
const WRAP_INDICATOR = "↪ "
const MAX_WRAP = 20
/** The terminal's own colour for each severity (its red, yellow, blue, grey), and
 *  how helix's pickers name them. */
const SEVERITY: Record<CodeSeverity, { ink: string; name: string }> = {
  error: { ink: ansiColor(1), name: "ERROR" },
  warning: { ink: ansiColor(3), name: "WARN" },
  info: { ink: ansiColor(4), name: "INFO" },
  hint: { ink: ansiColor(8), name: "HINT" },
}
const WORST: CodeSeverity[] = ["error", "warning", "info", "hint"]
/** The rows helix's completion menu and picker show before they scroll. */
const MENU_ROWS = 8

/** What lies over a piece of a line: a diagnostic's severity, or the selection. */
type Mark = CodeSeverity | "selection"

/** A row of the screen: a piece of one line of the file, from column `from`. */
interface ScreenRow {
  line: number
  from: number
  runs: CodePaint<Mark>[]
  first: boolean
  last: boolean
}

/** `runs` from column `a` to `b`. */
function cut(
  runs: readonly CodePaint<Mark>[],
  a: number,
  b: number,
): CodePaint<Mark>[] {
  const out: CodePaint<Mark>[] = []
  let at = 0
  for (const r of runs) {
    const end = at + r.text.length
    const from = Math.max(a, at)
    const to = Math.min(b, end)
    if (from < to) out.push({ ...r, text: r.text.slice(from - at, to - at) })
    at = end
  }
  return out
}

/** A line on the screen's rows, as helix soft-wraps it in `width` columns: at the
 *  last space that fits (one just past the edge too), unless the word left over is
 *  longer than MAX_WRAP, then at the edge; every row after the first
 *  `WRAP_INDICATOR` narrower. */
function wrap(
  line: number,
  runs: readonly CodePaint<Mark>[],
  width: number,
): ScreenRow[] {
  const text = runs.map((r) => r.text).join("")
  const rows: ScreenRow[] = []
  let from = 0
  do {
    const room = Math.max(
      1,
      rows.length ? width - WRAP_INDICATOR.length : width,
    )
    let to = Math.min(text.length, from + room)
    if (to < text.length) {
      const space = text.lastIndexOf(" ", to)
      if (space >= from && to - space - 1 <= MAX_WRAP) to = space + 1
    }
    rows.push({
      line,
      from,
      runs: cut(runs, from, to),
      first: from === 0,
      last: to >= text.length,
    })
    from = to
  } while (from < text.length)
  return rows
}

/** The screen's width in columns: the text's room over one character's width, as
 *  the page draws them. Unbounded until it is read. */
function useColumns(): [React.RefObject<HTMLSpanElement | null>, number] {
  const probe = React.useRef<HTMLSpanElement>(null)
  const [columns, setColumns] = React.useState(Number.POSITIVE_INFINITY)
  React.useLayoutEffect(() => {
    const el = probe.current
    const room = el?.parentElement
    if (!el || !room) return
    const read = () => {
      // Not laid out (inside `display: none`, a layout kept for a narrow window): no
      // width to count yet. The observer reads again once it is.
      if (el.getClientRects().length === 0) return
      const ch = el.getBoundingClientRect().width / 10
      if (!(ch > 0))
        throw new Error("terminal-editor: a character with no width")
      setColumns(Math.floor(room.clientWidth / ch))
    }
    read()
    const sizes = new ResizeObserver(read)
    sizes.observe(room)
    return () => sizes.disconnect()
  }, [])
  return [probe, columns]
}

const nameOf = (path: string) => path.split("/").at(-1) as string

/** An edit played in the terminal: the frame at the instant drawn as helix. Its
 *  playable, for the driver: `codeTimeline(files, steps).playable`. */
export function TerminalEditor({
  files,
  steps,
  progress,
  duration,
  diagnose,
  rows = 14,
  bufferline = "multiple",
  softWrap = false,
  onOpen,
}: TerminalEditorProps) {
  const palette = useTerminalPalette()
  const timeline = React.useMemo(
    () => codeTimeline(files, steps, { duration, diagnose }),
    [files, steps, duration, diagnose],
  )
  const ms = usePlayhead(progress).at * timeline.total
  const f = codeFrameAt(timeline, ms)
  const path = f.active ?? "[scratch]"
  const texts = React.useMemo(
    () => timeline.states.map((s) => ({ text: s.text, lang: langOf(s.path) })),
    [timeline],
  )
  const colours = useCodeColoursOf(texts)[f.state] ?? null
  const [probe, columns] = useColumns()
  const lines = runsOf(f.text, colours, f.origin)
  const place = placeOf(f.text, f.caret)
  const buffers =
    bufferline === "always" || (bufferline === "multiple" && f.open.length > 1)
  const textRows = rows - (buffers ? 1 : 0)
  const ranges: { from: number; to: number; mark: Mark }[] = [
    ...f.diagnostics.map((d) => ({ from: d.from, to: d.to, mark: d.severity })),
    ...(f.selection ? [{ ...f.selection, mark: "selection" as const }] : []),
  ]
  // Each line's diagnostics, worst first: its gutter's mark, its end's message.
  const starts: number[] = []
  let at = 0
  for (const runs of lines) {
    starts.push(at)
    at += runs.reduce((n, r) => n + r.text.length, 0) + 1
  }
  const lineOf = (offset: number) => starts.findLastIndex((s) => s <= offset)
  const said = lines.map((_, i) =>
    f.diagnostics
      .filter((d) => lineOf(d.from) === i)
      .sort((a, b) => WORST.indexOf(a.severity) - WORST.indexOf(b.severity)),
  )
  // The screen's rows: each line once, or soft-wrapped across as many as it needs.
  const screen: ScreenRow[] = lines.flatMap((runs, i) => {
    const painted = paintRuns(runs, starts[i] as number, ranges)
    return softWrap && Number.isFinite(columns)
      ? wrap(i, painted, columns)
      : [{ line: i, from: 0, runs: painted, first: true, last: true }]
  })
  // The caret's row: the last piece of its line starting at or before its column.
  const column = place.column - 1
  const caretRow = screen.findLastIndex(
    (s) => s.line === place.line - 1 && s.from <= column,
  )
  const caret = screen[caretRow] as ScreenRow
  const top = Math.max(0, caretRow - textRows + SCROLL_OFF)
  const digits = Math.max(3, String(lines.length).length)
  const insert = f.mode === "insert"
  /** Where the text starts, past the diagnostics gutter and the numbers. */
  const gutter = digits + 3
  // The command line: what is typed into it, a prompt, else helix's answer to the
  // last save (`'index.html' written, 12L 380B`) while that file is in front.
  const message = f.prompt
    ? `${f.prompt.label}: ${f.prompt.text}`
    : f.command
      ? f.command
      : f.saved && f.saved.path === f.active
        ? `'${f.saved.path}' written, ${f.text.replace(/\n$/, "").split("\n").length}L ${new TextEncoder().encode(f.text).length}B`
        : ""
  const counts = WORST.slice(0, 2).map((s) => ({
    s,
    n: f.problems.reduce(
      (k, p) => k + p.diagnostics.filter((d) => d.severity === s).length,
      0,
    ),
  }))
  const caretAt = (row: number, offset: number) => {
    const p = placeOf(f.text, offset)
    const r = screen.findLastIndex(
      (s) => s.line === p.line - 1 && s.from <= p.column - 1,
    )
    const s = screen[r] as ScreenRow | undefined
    return {
      row: (r < 0 ? row : r) - top,
      column:
        (s && !s.first ? WRAP_INDICATOR.length : 0) +
        p.column -
        1 -
        (s?.from ?? 0),
    }
  }
  return (
    <div
      data-slot="terminal-editor"
      className="whitespace-pre"
      style={{ color: palette.command }}
    >
      {buffers && (
        <div
          data-slot="terminal-editor-bufferline"
          className="flex overflow-hidden"
          style={{ background: palette.bar, height: `${LINE_HEIGHT}em` }}
        >
          {f.open.map((p) => {
            const front = p === f.active
            const name = `${nameOf(p)}${f.dirty.includes(p) ? "[+]" : ""}`
            const look = {
              className: "shrink-0 px-[1ch]",
              style: front
                ? { background: palette.dot, color: palette.command }
                : { color: palette.muted },
            }
            return onOpen ? (
              <button
                key={p}
                type="button"
                data-slot="terminal-editor-buffer"
                data-active={front || undefined}
                // The terminal is one picture to assistive tech (role img, its
                // screen hidden): a tab is for the pointer, never a focus stop.
                tabIndex={-1}
                onClick={() => front || onOpen(p)}
                {...look}
                className={`${look.className} cursor-pointer whitespace-pre transition-colors hover:brightness-150`}
              >
                {name}
              </button>
            ) : (
              <span
                key={p}
                data-slot="terminal-editor-buffer"
                data-active={front || undefined}
                {...look}
              >
                {name}
              </span>
            )
          })}
        </div>
      )}
      <div
        data-slot="terminal-editor-text"
        className="relative"
        style={{ paddingTop: "0.5em" }}
      >
        {Array.from({ length: textRows }, (_, r) => {
          const row = screen[top + r]
          const here = row !== undefined && top + r === caretRow
          const worst = row ? said[row.line]?.[0] : undefined
          return (
            <div
              // biome-ignore lint/suspicious/noArrayIndexKey: a screen row
              key={r}
              data-slot="terminal-editor-line"
              className="flex overflow-hidden"
              style={{ height: `${LINE_HEIGHT}em` }}
            >
              <span
                data-slot="terminal-editor-gutter"
                className="w-[1ch] shrink-0"
                style={{
                  color: worst ? SEVERITY[worst.severity].ink : undefined,
                }}
              >
                {row?.first && worst ? "●" : ""}
              </span>
              <span
                data-slot="terminal-editor-number"
                className="shrink-0 pr-[1ch] text-right"
                style={{
                  width: `${digits + 2}ch`,
                  color:
                    row && row.line === place.line - 1
                      ? palette.accent
                      : palette.prompt,
                }}
              >
                {row?.first ? row.line + 1 : ""}
              </span>
              <span className="relative min-w-0 flex-1">
                {r === 0 && (
                  // One character's width ten times over: what the screen's columns
                  // are counted in.
                  <span ref={probe} aria-hidden className="invisible absolute">
                    0000000000
                  </span>
                )}
                {row && !row.first && (
                  <span
                    data-slot="terminal-editor-wrap"
                    style={{ color: palette.prompt }}
                  >
                    {WRAP_INDICATOR}
                  </span>
                )}
                {row?.runs.map((run, k) => {
                  const severity = WORST.find((s) => run.marks.includes(s))
                  return (
                    <span
                      // biome-ignore lint/suspicious/noArrayIndexKey: runs are positional
                      key={k}
                      data-slot={
                        severity ? "terminal-editor-undercurl" : undefined
                      }
                      style={{
                        color: run.ink?.dark,
                        fontStyle: run.ink?.italic ? "italic" : undefined,
                        fontWeight: run.ink?.bold ? 700 : undefined,
                        background: run.marks.includes("selection")
                          ? `color-mix(in oklab, ${palette.accent} 30%, transparent)`
                          : undefined,
                        textDecoration: severity
                          ? `underline wavy ${SEVERITY[severity].ink}`
                          : undefined,
                      }}
                    >
                      {run.text}
                    </span>
                  )
                })}
                {row?.last && worst && (
                  // Helix's end-of-line diagnostic: the worst one's message, after
                  // the line, in its severity's colour.
                  <span
                    data-slot="terminal-editor-eol"
                    style={{
                      color: SEVERITY[worst.severity].ink,
                      opacity: 0.8,
                    }}
                  >
                    {"    "}
                    {worst.message}
                  </span>
                )}
                {here && (
                  <span
                    data-slot="terminal-editor-caret"
                    className="absolute top-0 h-full"
                    style={{
                      left: `${(caret.first ? 0 : WRAP_INDICATOR.length) + column - caret.from}ch`,
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
        {f.popup && (
          <Popup
            popup={f.popup}
            at={caretAt(caretRow, f.popup.at)}
            gutter={gutter}
            rows={textRows}
          />
        )}
        {f.panel === "problems" && (
          <Picker path={path} diagnostics={f.diagnostics} text={f.text} />
        )}
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
        {counts.map(
          ({ s, n }) =>
            n > 0 && (
              <span
                key={s}
                data-slot="terminal-editor-count"
                style={{ color: SEVERITY[s].ink }}
              >
                ● {n}
              </span>
            ),
        )}
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
        {f.prompt && (
          <span style={{ background: palette.accent, color: palette.bg }}>
            {" "}
          </span>
        )}
      </div>
    </div>
  )
}

/** A helix popup at a place of the screen, drawn in a box-drawing border: the
 *  hover's docs and signature help above the caret's row when they fit there, the
 *  completion menu below it. */
function Popup({
  popup,
  at,
  gutter,
  rows,
}: {
  popup: CodePopup
  at: { row: number; column: number }
  gutter: number
  rows: number
}) {
  const palette = useTerminalPalette()
  const below = popup.kind === "completion" || at.row < 4
  const box: React.CSSProperties = {
    position: "absolute",
    zIndex: 10,
    left: `${gutter + at.column}ch`,
    border: `1px solid ${palette.prompt}`,
    background: palette.bar,
    color: palette.command,
    maxWidth: `calc(100% - ${gutter + at.column}ch)`,
    ...(below
      ? { top: `calc(0.5em + ${(at.row + 1) * LINE_HEIGHT}em)` }
      : { bottom: `calc(100% - 0.5em - ${at.row * LINE_HEIGHT}em)` }),
  }
  if (popup.kind === "completion") {
    const first = Math.max(0, popup.selected - MENU_ROWS + 1)
    const shown = popup.items.slice(first, first + Math.min(MENU_ROWS, rows))
    const width = Math.max(...shown.map((i) => i.label.length))
    return (
      <div data-slot="terminal-editor-completion" style={box}>
        {shown.map((item, k) => {
          const chosen = first + k === popup.selected
          return (
            <div
              key={item.label}
              data-selected={chosen || undefined}
              className="px-[1ch]"
              style={
                chosen
                  ? { background: palette.accent, color: palette.bg }
                  : undefined
              }
            >
              {item.label.padEnd(width + 2)}
              <span style={{ color: chosen ? palette.bg : palette.muted }}>
                {item.kind ?? "text"}
              </span>
            </div>
          )
        })}
      </div>
    )
  }
  if (popup.kind === "signature")
    return (
      <div
        data-slot="terminal-editor-signature"
        className="px-[1ch]"
        style={box}
      >
        {popup.active ? (
          <>
            {popup.label.slice(0, popup.active.from)}
            <span style={{ color: palette.accent, fontWeight: 700 }}>
              {popup.label.slice(popup.active.from, popup.active.to)}
            </span>
            {popup.label.slice(popup.active.to)}
          </>
        ) : (
          popup.label
        )}
      </div>
    )
  return (
    <div
      data-slot="terminal-editor-hover"
      className="whitespace-pre-wrap px-[1ch]"
      style={box}
    >
      {popup.text}
    </div>
  )
}

/** `space d`: helix's diagnostics picker over the editor, its prompt, the count,
 *  and one row per diagnostic of the file (its severity, source, message, place),
 *  the first selected. */
function Picker({
  path,
  diagnostics,
  text,
}: {
  path: string
  diagnostics: readonly {
    from: number
    severity: CodeSeverity
    message: string
    source?: string
  }[]
  text: string
}) {
  const palette = useTerminalPalette()
  const list = [...diagnostics].sort(
    (a, b) => WORST.indexOf(a.severity) - WORST.indexOf(b.severity),
  )
  return (
    <div
      data-slot="terminal-editor-picker"
      className="absolute inset-x-[4ch] top-[1em] z-20 overflow-hidden"
      style={{
        border: `1px solid ${palette.prompt}`,
        background: palette.bg,
        color: palette.command,
      }}
    >
      <div className="flex px-[1ch]">
        <span style={{ color: palette.accent }}>{"> "}</span>
        <span
          style={{ background: palette.accent, width: "1ch" }}
          aria-hidden
        />
        <span className="flex-1" />
        <span style={{ color: palette.muted }}>
          {list.length}/{list.length}
        </span>
      </div>
      <div style={{ borderTop: `1px solid ${palette.prompt}` }}>
        {list.length === 0 && (
          <div className="px-[1ch]" style={{ color: palette.muted }}>
            no diagnostics in {nameOf(path)}
          </div>
        )}
        {list.slice(0, MENU_ROWS).map((d, i) => {
          const p = placeOf(text, d.from)
          return (
            <div
              key={`${d.from}:${d.message}`}
              data-slot="terminal-editor-picker-row"
              className="flex gap-[2ch] overflow-hidden px-[1ch]"
              style={i === 0 ? { background: palette.dot } : undefined}
            >
              <span
                className="w-[5ch] shrink-0"
                style={{ color: SEVERITY[d.severity].ink }}
              >
                {SEVERITY[d.severity].name}
              </span>
              <span className="shrink-0" style={{ color: palette.muted }}>
                {d.source ?? ""}
              </span>
              <span className="min-w-0 flex-1 truncate">{d.message}</span>
              <span className="shrink-0" style={{ color: palette.muted }}>
                {nameOf(path)}:{p.line}:{p.column}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}
