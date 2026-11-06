// HELIX'S SCREEN AS DATA: what terminal-editor shows at an instant of an edit, decided
// once, and drawn by two emitters that cannot disagree: the live editor in the DOM
// (ui/terminal-editor) and the SVG still (`renderEditorSvg`, for a build script, an og
// card, a feature's media). Framework-free by contract: nothing here may import React
// or touch the DOM.
//
//   const screen = await editorStill({ files, steps, progress: 1, columns: 64, rows: 12 })
//   const svg = renderEditorSvg(screen, { fontSize: 16 })
//
// The model decides everything that follows from the text and the screen's size, in
// the terminal's own units, columns and rows: which rows show (the scroll that keeps
// the caret in view), how a long line soft-wraps, each run's ink and marks (the
// undercurl, the selection), the gutter's mark and the message at a line's end, the
// caret, the bufferline, the status line's parts, the command line's words, where a
// popup opens. An emitter only places those cells: the DOM in `ch` and `em`, the SVG
// in px from the face's measured advance.

import {
  type CodeDiagnose,
  type CodeDiagnostic,
  type CodeFile,
  type CodeFrame,
  type CodePopup,
  type CodeSeverity,
  type CodeStep,
  codeFrameAt,
  codeTimeline,
  placeOf,
} from "@/lib/code-session"
import {
  type CodeColours,
  type CodePaint,
  codeColours,
  langOf,
  paintRuns,
  runsOf,
} from "@/lib/code-tokens"
import {
  ansiColor,
  baselineOf,
  escapeSvg,
  GEIST_MONO,
  LINE_HEIGHT,
  type MonoFace,
  type TerminalPalette,
  terminalPalette,
  terminalWindowSvg,
} from "@/lib/terminal-session"

/** How the status line names each mode, helix's three letters: a selection made to be
 *  changed is normal mode's. */
const MODES = {
  normal: "NOR",
  insert: "INS",
  select: "NOR",
  command: "NOR",
} as const
/** Rows helix keeps between the caret and the screen's foot. */
const SCROLL_OFF = 3
/** Helix's soft-wrap: the indicator a wrapped row opens with, and the longest word it
 *  moves whole to the next row rather than breaking (`max-wrap`). */
export const WRAP_INDICATOR = "↪ "
const MAX_WRAP = 20
/** The terminal's own colour for each severity (its red, yellow, blue, grey), and how
 *  helix's pickers name them. */
export const SEVERITY: Record<CodeSeverity, { ink: string; name: string }> = {
  error: { ink: ansiColor(1), name: "ERROR" },
  warning: { ink: ansiColor(3), name: "WARN" },
  info: { ink: ansiColor(4), name: "INFO" },
  hint: { ink: ansiColor(8), name: "HINT" },
}
export const WORST: CodeSeverity[] = ["error", "warning", "info", "hint"]
/** The rows helix's completion menu and picker show before they scroll. */
export const MENU_ROWS = 8
/** The message at a line's end: four spaces after the line, a little faint. */
export const EOL_GAP = "    "
export const EOL_OPACITY = 0.8
/** The selection: the accent laid over the text at this share. */
export const SELECTION_ALPHA = 0.3

/** A stretch of a screen row in one ink: `color` null is the screen's foreground. */
export interface EditorRun {
  text: string
  color: string | null
  italic: boolean
  bold: boolean
  selected: boolean
  /** The worst diagnostic over it, undercurled in its colour. */
  undercurl: CodeSeverity | null
}

/** A row of the text: a piece of one line of the file, from column `from`. */
export interface EditorRow {
  /** The file's line, from 0. */
  line: number
  from: number
  /** The line's first row (numbered, the gutter's mark) and its last (the message). */
  first: boolean
  last: boolean
  /** The line's length, where a press past its end lands. */
  length: number
  runs: EditorRun[]
  /** The caret's line: its number lit. */
  current: boolean
  /** The line's worst diagnostic: the gutter's mark on its first row, its message
   *  after its last. */
  worst: CodeDiagnostic | null
}

/** One file across the bufferline. */
export interface EditorBuffer {
  path: string
  /** The name shown, `[+]` on unsaved keys. */
  label: string
  front: boolean
}

/** The editor's screen at an instant, in columns and rows. */
export interface EditorScreen {
  palette: TerminalPalette
  /** The screen's width in columns; Infinity until a live editor has measured it. */
  columns: number
  /** The text's rows (the screen's, less the bufferline's), null past the file's end. */
  rows: (EditorRow | null)[]
  /** The line numbers' width in digits; the text starts at `gutter`, past the
   *  diagnostics' column, the numbers and their padding. */
  digits: number
  gutter: number
  /** Null while the bufferline is not shown. */
  buffers: EditorBuffer[] | null
  /** The caret: its row of `rows`, its column in the text (past `gutter`), a bar in
   *  insert and a block otherwise. */
  caret: { row: number; column: number; insert: boolean } | null
  /** A popup open, anchored at a row of `rows` and a column of the text; `below` the
   *  anchor's row (completion, or no room above) or above it. */
  popup: {
    popup: CodePopup
    row: number
    column: number
    below: boolean
  } | null
  /** `space d`'s picker open: the front file's diagnostics, worst first. */
  picker: {
    name: string
    text: string
    diagnostics: CodeDiagnostic[]
  } | null
  status: {
    mode: "NOR" | "INS"
    insert: boolean
    /** The file in front, ` [+]` while it is unsaved. */
    file: string
    /** The errors and the warnings, only those there are. */
    counts: { severity: CodeSeverity; n: number }[]
    /** `1 sel 4:26`. */
    place: string
  }
  /** The command line: what is typed into it, a prompt (its bar after), helix's
   *  answer to the last save, or nothing. */
  command: { text: string; prompt: boolean }
}

export interface EditorScreenOptions {
  palette: TerminalPalette
  /** The front file's colours (lib/code-tokens `codeColours` of the frame's state),
   *  null to draw it plain. */
  colours: CodeColours | null
  /** Rows of the screen above the status and command lines, the bufferline's
   *  included when it shows. */
  rows: number
  /** The screen's whole width in columns, the gutter's included (a soft-wrapped line
   *  wraps in what is left past it); Infinity wraps nothing. */
  columns: number
  bufferline: "always" | "multiple" | "never"
  softWrap: boolean
  /** A text box taking typing: insert, whatever mode the edit is in. */
  insert?: boolean
}

const nameOf = (path: string) => path.split("/").at(-1) as string

type Mark = CodeSeverity | "selection"
type Painted = CodePaint<Mark>[]

/** `runs` from column `a` to `b`. */
function cut(runs: Painted, a: number, b: number): Painted {
  const out: Painted = []
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

/** A line's pieces as helix soft-wraps it in `width` columns: at the last space that
 *  fits (one just past the edge too), unless the word left over is longer than
 *  MAX_WRAP, then at the edge; every row after the first `WRAP_INDICATOR` narrower. */
function wrap(
  runs: Painted,
  width: number,
): { from: number; to: number; runs: Painted }[] {
  const text = runs.map((r) => r.text).join("")
  const pieces: { from: number; to: number; runs: Painted }[] = []
  let from = 0
  do {
    const room = Math.max(
      1,
      pieces.length ? width - WRAP_INDICATOR.length : width,
    )
    let to = Math.min(text.length, from + room)
    if (to < text.length) {
      const space = text.lastIndexOf(" ", to)
      if (space >= from && to - space - 1 <= MAX_WRAP) to = space + 1
    }
    pieces.push({ from, to, runs: cut(runs, from, to) })
    from = to
  } while (from < text.length)
  return pieces
}

/** The screen of `f`, the edit's frame at an instant (lib/code-session
 *  `codeFrameAt`). */
export function editorScreen(
  f: CodeFrame,
  {
    palette,
    colours,
    rows,
    columns,
    bufferline,
    softWrap,
    insert: typing = false,
  }: EditorScreenOptions,
): EditorScreen {
  if (!(columns > 0))
    throw new Error(`terminal-editor-screen: ${columns} columns`)
  const path = f.active ?? "[scratch]"
  const lines = runsOf(f.text, colours, f.origin)
  const place = placeOf(f.text, f.caret)
  const digits = Math.max(3, String(lines.length).length)
  const gutter = digits + 3
  const shown =
    bufferline === "always" || (bufferline === "multiple" && f.open.length > 1)
  const textRows = rows - (shown ? 1 : 0)
  if (textRows < 1)
    throw new Error(`terminal-editor-screen: ${rows} rows hold no text`)
  const ranges: { from: number; to: number; mark: Mark }[] = [
    ...f.diagnostics.map((d) => ({ from: d.from, to: d.to, mark: d.severity })),
    ...(f.selection ? [{ ...f.selection, mark: "selection" as const }] : []),
  ]
  const starts: number[] = []
  let at = 0
  for (const runs of lines) {
    starts.push(at)
    at += runs.reduce((n, r) => n + r.text.length, 0) + 1
  }
  const lineOf = (offset: number) => starts.findLastIndex((s) => s <= offset)
  const worstOf = (i: number) =>
    f.diagnostics
      .filter((d) => lineOf(d.from) === i)
      .sort(
        (a, b) => WORST.indexOf(a.severity) - WORST.indexOf(b.severity),
      )[0] ?? null
  // Every row of the file: each line once, or soft-wrapped across as many as it needs.
  const all: EditorRow[] = lines.flatMap((runs, i) => {
    const painted = paintRuns(runs, starts[i] as number, ranges)
    const length = runs.reduce((n, r) => n + r.text.length, 0)
    const pieces =
      softWrap && Number.isFinite(columns)
        ? wrap(painted, columns - gutter)
        : [{ from: 0, to: length, runs: painted }]
    const worst = worstOf(i)
    return pieces.map((p, k) => ({
      line: i,
      from: p.from,
      first: k === 0,
      last: k === pieces.length - 1,
      length,
      current: i === place.line - 1,
      worst,
      runs: p.runs.map((r) => ({
        text: r.text,
        color: r.ink?.dark ?? null,
        italic: r.ink?.italic ?? false,
        bold: r.ink?.bold ?? false,
        selected: r.marks.includes("selection"),
        undercurl: WORST.find((s) => r.marks.includes(s)) ?? null,
      })),
    }))
  })
  // The row a place of the text is on: the last piece of its line starting at or
  // before its column.
  const rowOf = (p: { line: number; column: number }) =>
    all.findLastIndex((s) => s.line === p.line - 1 && s.from <= p.column - 1)
  const caretRow = rowOf(place)
  const top = Math.max(0, caretRow - textRows + SCROLL_OFF)
  const columnIn = (r: number, p: { column: number }) => {
    const s = all[r]
    return (
      (s && !s.first ? WRAP_INDICATOR.length : 0) +
      p.column -
      1 -
      (s?.from ?? 0)
    )
  }
  const mode = typing ? "insert" : f.mode
  const insert = mode === "insert"
  /** A popup at its anchor's row and column: below it for the completion menu, or
   *  when the rows above are too few. */
  const placed = (popup: CodePopup) => {
    const p = placeOf(f.text, popup.at)
    const r = rowOf(p)
    const row = (r < 0 ? caretRow : r) - top
    return {
      popup,
      row,
      column: columnIn(r, p),
      below: popup.kind === "completion" || row < 4,
    }
  }
  const command = f.prompt
    ? `${f.prompt.label}: ${f.prompt.text}`
    : f.command
      ? f.command
      : f.saved && f.saved.path === f.active
        ? `'${f.saved.path}' written, ${f.text.replace(/\n$/, "").split("\n").length}L ${new TextEncoder().encode(f.text).length}B`
        : ""
  return {
    palette,
    columns,
    rows: Array.from({ length: textRows }, (_, r) => all[top + r] ?? null),
    digits,
    gutter,
    buffers: shown
      ? f.open.map((p) => ({
          path: p,
          label: `${nameOf(p)}${f.dirty.includes(p) ? "[+]" : ""}`,
          front: p === f.active,
        }))
      : null,
    caret:
      caretRow < 0
        ? null
        : { row: caretRow - top, column: columnIn(caretRow, place), insert },
    popup: f.popup ? placed(f.popup) : null,
    picker:
      f.panel === "problems"
        ? {
            name: nameOf(path),
            text: f.text,
            diagnostics: [...f.diagnostics].sort(
              (a, b) => WORST.indexOf(a.severity) - WORST.indexOf(b.severity),
            ),
          }
        : null,
    status: {
      mode: MODES[mode],
      insert,
      file: `${path}${f.active && f.dirty.includes(f.active) ? " [+]" : ""}`,
      counts: WORST.slice(0, 2)
        .map((severity) => ({
          severity,
          n: f.problems.reduce(
            (k, p) =>
              k + p.diagnostics.filter((d) => d.severity === severity).length,
            0,
          ),
        }))
        .filter((c) => c.n > 0),
      place: `1 sel ${place.line}:${place.column}`,
    },
    command: { text: command, prompt: f.prompt !== null },
  }
}

// ── the still ──

export interface EditorStillOptions
  extends Omit<
    EditorScreenOptions,
    "palette" | "colours" | "bufferline" | "softWrap"
  > {
  files: CodeFile[]
  steps: CodeStep[]
  /** The instant drawn, 0..1 of the edit: the live editor's `progress`. */
  progress: number
  diagnose?: CodeDiagnose
  duration?: number
  /** The terminal's accent (lib/terminal-session `terminalPalette`). */
  accent?: string
  bufferline?: EditorScreenOptions["bufferline"]
  softWrap?: boolean
}

/** The screen at `progress` of an edit, coloured: what the live editor draws at that
 *  `progress`, for `renderEditorSvg`. Async only for the colours (Shiki, or the
 *  language's own tokens, loaded on the first call). */
export async function editorStill({
  files,
  steps,
  progress,
  diagnose,
  duration,
  accent,
  bufferline = "multiple",
  softWrap = false,
  ...screen
}: EditorStillOptions): Promise<EditorScreen> {
  if (!(progress >= 0 && progress <= 1))
    throw new Error(`terminal-editor-screen: progress ${progress} is not 0..1`)
  if (!Number.isFinite(screen.columns))
    throw new Error("terminal-editor-screen: a still is given its columns")
  const timeline = codeTimeline(files, steps, { duration, diagnose })
  const f = codeFrameAt(timeline, progress * timeline.total)
  const state = timeline.states[f.state]
  const colours = state
    ? await codeColours(state.text, langOf(state.path))
    : null
  return editorScreen(f, {
    ...screen,
    palette: terminalPalette(accent),
    colours,
    bufferline,
    softWrap,
  })
}

/** The screen's size in em of its type: the bufferline, the text's top padding, its
 *  rows, the status and command lines. The window adds its bar (terminal-session
 *  `BAR_EM`) and its hairline. */
export const screenEm = (screen: EditorScreen) =>
  (screen.buffers ? LINE_HEIGHT : 0) +
  TEXT_TOP +
  (screen.rows.length + 2) * LINE_HEIGHT
/** The padding above the text's first row, em. */
export const TEXT_TOP = 0.5

export interface EditorSvgOptions {
  /** The type, px; the whole window scales with it. */
  fontSize?: number
  face?: MonoFace
}

/** The screen as the terminal's window in SVG, at its own size: `columns` wide in the
 *  face's advance, `screenEm` tall plus the window's bar. Throws on what it does not
 *  draw yet (a popup, the picker) and on a screen with no width: never a still that
 *  quietly differs from the live one. */
export function renderEditorSvg(
  screen: EditorScreen,
  { fontSize = 16, face = GEIST_MONO }: EditorSvgOptions = {},
): string {
  const { palette, columns } = screen
  if (!Number.isFinite(columns))
    throw new Error("renderEditorSvg: the screen has no width; give it columns")
  if (screen.popup)
    throw new Error(
      `renderEditorSvg: a ${screen.popup.popup.kind} popup is not drawn as a still yet`,
    )
  if (screen.picker)
    throw new Error(
      "renderEditorSvg: the diagnostics picker is not drawn as a still yet",
    )
  const ch = face.ch * fontSize
  const lh = LINE_HEIGHT * fontSize
  const base = baselineOf(face) * fontSize
  const out: string[] = []
  /** Text from column `col` of the row whose top is `y`, cut at the screen's edge. */
  // Every word is placed at its own column, so the grid holds whatever a renderer does
  // with spaces (a browser collapses them in an inlined SVG, `white-space` or not) and
  // whatever its face's advance.
  const put = (col: number, y: number, text: string, attrs: string): number => {
    const shown = [...text].slice(0, Math.max(0, columns - col))
    const s = shown.join("")
    for (const m of s.matchAll(/\S+/g)) {
      const at = col + [...s.slice(0, m.index)].length
      out.push(
        `<text x="${at * ch}" y="${y + base}" ${attrs}>${escapeSvg(m[0])}</text>`,
      )
    }
    return col + shown.length
  }
  const fill = (
    col: number,
    y: number,
    width: number,
    colour: string,
    extra = "",
  ) => {
    const w = Math.min(width, columns - col)
    if (w > 0)
      out.push(
        `<rect x="${col * ch}" y="${y}" width="${w * ch}" height="${lh}" fill="${colour}"${extra}/>`,
      )
  }
  let y = 0
  if (screen.buffers) {
    fill(0, y, columns, palette.bar)
    let col = 0
    for (const b of screen.buffers) {
      const width = b.label.length + 2
      if (b.front) fill(col, y, width, palette.dot)
      put(
        col + 1,
        y,
        b.label,
        `fill="${b.front ? palette.command : palette.muted}"`,
      )
      col += width
    }
    y += lh
  }
  y += TEXT_TOP * fontSize
  screen.rows.forEach((row, r) => {
    const top = y + r * lh
    if (!row) return
    if (row.first && row.worst)
      put(0, top, "●", `fill="${SEVERITY[row.worst.severity].ink}"`)
    if (row.first) {
      const n = String(row.line + 1)
      put(
        screen.digits + 2 - n.length,
        top,
        n,
        `fill="${row.current ? palette.accent : palette.prompt}"`,
      )
    }
    let col = screen.gutter
    if (!row.first)
      col = put(col, top, WRAP_INDICATOR, `fill="${palette.prompt}"`)
    for (const run of row.runs) {
      const width = [...run.text].length
      if (run.selected)
        fill(
          col,
          top,
          width,
          palette.accent,
          ` fill-opacity="${SELECTION_ALPHA}"`,
        )
      const attrs = [
        `fill="${run.color ?? palette.command}"`,
        run.italic && `font-style="italic"`,
        run.bold && `font-weight="700"`,
      ]
        .filter(Boolean)
        .join(" ")
      const end = put(col, top, run.text, attrs)
      if (run.undercurl && end > col)
        out.push(
          undercurl(
            col * ch,
            end * ch,
            top + base,
            fontSize,
            SEVERITY[run.undercurl].ink,
          ),
        )
      col += width
    }
    if (row.last && row.worst)
      put(
        col,
        top,
        `${EOL_GAP}${row.worst.message}`,
        `fill="${SEVERITY[row.worst.severity].ink}" fill-opacity="${EOL_OPACITY}"`,
      )
  })
  if (screen.caret) {
    const top = y + screen.caret.row * lh
    const x = (screen.gutter + screen.caret.column) * ch
    out.push(
      screen.caret.insert
        ? `<rect x="${x}" y="${top}" width="${(2 / 13) * fontSize}" height="${lh}" fill="${palette.accent}"/>`
        : `<rect x="${x}" y="${top}" width="${ch}" height="${lh}" fill="${palette.accent}" style="mix-blend-mode: difference"/>`,
    )
  }
  y += screen.rows.length * lh
  // The status line: the mode, the file, then at the right the counts and the place,
  // a column between each, as the live one lays them in a row.
  const { status } = screen
  fill(0, y, columns, palette.bar)
  fill(
    0,
    y,
    status.mode.length + 2,
    status.insert ? palette.command : palette.accent,
  )
  put(1, y, status.mode, `fill="${palette.bg}" font-weight="700"`)
  const right = [
    ...status.counts.map((c) => ({
      text: `● ${c.n}`,
      ink: SEVERITY[c.severity].ink,
    })),
    { text: status.place, ink: palette.muted },
  ]
  let end = columns - 1
  const placed = right.reverse().map((s) => {
    const col = end - s.text.length
    end = col - 1
    return { ...s, col }
  })
  const fileAt = status.mode.length + 3
  const room = end - fileAt
  const file =
    status.file.length <= room
      ? status.file
      : `${status.file.slice(0, Math.max(0, room - 1))}…`
  put(fileAt, y, file, `fill="${palette.command}"`)
  for (const s of placed) put(s.col, y, s.text, `fill="${s.ink}"`)
  y += lh
  const typed = put(1, y, screen.command.text, `fill="${palette.muted}"`)
  if (screen.command.prompt) fill(typed, y, 1, palette.accent)
  y += lh
  return terminalWindowSvg(
    palette,
    { width: columns * ch, height: y, fontSize, face },
    out.join(""),
  )
}

/** A wavy line under a run, as Chrome draws CSS's `underline wavy` in Geist Mono: at
 *  the face's underline (0.1 em below the baseline, 0.05 em thick, from its `post`
 *  table, a pixel at the least), a wave half a wavelength of 0.27 em, its curve's
 *  control 0.23 em off the line (measured against Chrome's at 13px: 3px peak to
 *  peak). */
function undercurl(
  x0: number,
  x1: number,
  baseline: number,
  fontSize: number,
  ink: string,
): string {
  const amp = fontSize * 0.23
  const y = baseline + fontSize * 0.1 + amp / 2
  const half = fontSize * 0.27
  let d = `M${x0} ${y}`
  for (let x = x0, k = 0; x < x1; x += half, k++)
    d += `Q${Math.min(x1, x + half / 2)} ${y + (k % 2 ? amp : -amp)} ${Math.min(x1, x + half)} ${y}`
  return `<path d="${d}" fill="none" stroke="${ink}" stroke-width="${Math.max(1, fontSize * 0.05)}"/>`
}
