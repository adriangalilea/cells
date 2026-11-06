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
// What is on the screen is decided by lib/terminal-editor-screen (`editorScreen`), the
// same model its SVG still draws (`renderEditorSvg`), so a still and this view cannot
// disagree; this file places the model's cells in `ch` and `em` and adds what only a
// page has: the caret's blink, the presses and the keys.
//
// It wears the terminal's phosphor (`useTerminalPalette`) for everything but the
// code, which takes code's dark theme (lib/code-tokens), and the severities, which
// take the terminal's own red, yellow, blue and grey: a terminal is dark whatever
// the page is. Display-only, like the terminal around it, but for the bufferline's
// files when `onOpen` makes them pressable, and the text when `onEdit` makes it a text
// box: each press and key handed to the page as the step it makes, the frame still the
// edit's.

import * as React from "react"
import {
  type CodeDiagnose,
  type CodeFile,
  type CodePopup,
  type CodeStep,
  codeFrameAt,
  codeTimeline,
  keyStep,
  placeOf,
} from "@/lib/code-session"
import { langOf } from "@/lib/code-tokens"
import {
  type EditorRow,
  type EditorScreen,
  EOL_GAP,
  EOL_OPACITY,
  editorScreen,
  MENU_ROWS,
  SELECTION_ALPHA,
  SEVERITY,
  TEXT_TOP,
  WRAP_INDICATOR,
} from "@/lib/terminal-editor-screen"
import { LINE_HEIGHT } from "@/lib/terminal-session"
import { usePlayhead } from "@/components/ui/playback"
import { useTerminalPalette } from "@/components/ui/terminal"
import { useCodeColoursOf } from "@/components/ui/use-code-colours"

export interface TerminalEditorProps {
  /** The files the editor can open (`/` between folders). */
  files: CodeFile[]
  steps: CodeStep[]
  /** The instant drawn, 0..1 of the edit, for a still. Inside a driver (a Player)
   *  omit it: the driver provides it. The same instant as an SVG still:
   *  lib/terminal-editor-screen `editorStill` and `renderEditorSvg`. */
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
  /** The editor takes typing as a text box does, with no modes: focused, it is in
   *  insert, a click places the caret, a key types at it, the arrows, Home and End
   *  move it, Backspace and Delete take a character. Each is handed over as the
   *  step it makes (`keyStep`), and the page answers by appending it to `steps`,
   *  as `onOpen`. Inside `<Terminal interactive>`, so the box is not hidden in one
   *  picture. Absent, display-only. */
  onEdit?: (step: CodeStep) => void
}

/** How long the caret has been still, ms, before it blinks, and a blink's whole
 *  period: a terminal's. */
const STILL_MS = 500
const BLINK_MS = 1060

/** The caret: solid while keys land, blinking once it has been still (`still`), on the
 *  page's clock, as a terminal's does; solid always under reduced motion. */
function Caret({
  still,
  style,
}: {
  still: boolean
  style: React.CSSProperties
}) {
  const ref = React.useRef<HTMLSpanElement>(null)
  React.useEffect(() => {
    const el = ref.current
    if (!still || !el || matchMedia("(prefers-reduced-motion: reduce)").matches)
      return
    const blink = el.animate([{ opacity: 1 }, { opacity: 0 }], {
      duration: BLINK_MS,
      iterations: Number.POSITIVE_INFINITY,
      easing: "steps(2, jump-none)",
    })
    return () => blink.cancel()
  }, [still])
  return (
    <span
      ref={ref}
      data-slot="terminal-editor-caret"
      className="absolute top-0 h-full"
      style={style}
    />
  )
}

/** The screen's whole width in columns (the gutter's included, as the model counts
 *  them) over one character's width, as the page draws them. Unbounded until it is
 *  read. */
function useColumns(): [React.RefObject<HTMLSpanElement | null>, number] {
  const probe = React.useRef<HTMLSpanElement>(null)
  const [columns, setColumns] = React.useState(Number.POSITIVE_INFINITY)
  React.useLayoutEffect(() => {
    const el = probe.current
    const room = el?.closest<HTMLElement>('[data-slot="terminal-editor"]')
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
  onEdit,
}: TerminalEditorProps) {
  const palette = useTerminalPalette()
  const [focused, setFocused] = React.useState(false)
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
  // A text box has no modes: taking typing, the editor is in insert.
  const screen = editorScreen(f, {
    palette,
    colours,
    rows,
    columns,
    bufferline,
    softWrap,
    insert: onEdit !== undefined && focused,
  })
  const { status, caret } = screen
  /** A key, as the step it makes. */
  const key = (e: React.KeyboardEvent) => {
    if (!onEdit) return
    // Select all is the box's, never the page's: the box has no selection, so nothing.
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "a")
      return e.preventDefault()
    if (e.metaKey || e.ctrlKey || e.altKey) return
    const step = keyStep(e.key, f.text, f.caret)
    if (!step) return
    e.preventDefault()
    onEdit(step)
  }
  /** A press on a screen row, as the caret placed where it landed. */
  const press = (row: EditorRow, e: React.PointerEvent<HTMLElement>) => {
    const ch = (probe.current?.getBoundingClientRect().width ?? 0) / 10
    if (!onEdit || !(ch > 0)) return
    const x = e.clientX - e.currentTarget.getBoundingClientRect().left
    const indent = row.first ? 0 : WRAP_INDICATOR.length
    const at = Math.round(x / ch) - indent + row.from
    onEdit({
      kind: "place",
      line: row.line + 1,
      column: Math.min(Math.max(at, row.from), row.length) + 1,
    })
  }
  return (
    <div
      data-slot="terminal-editor"
      // Taking typing, a press places the caret and never starts the page's selection.
      className={`whitespace-pre outline-none${onEdit ? " select-none" : ""}`}
      style={{ color: palette.command }}
      {...(onEdit && {
        role: "textbox",
        "aria-multiline": true,
        "aria-label": `${nameOf(path)}, editable`,
        tabIndex: 0,
        onKeyDown: key,
        onFocus: () => setFocused(true),
        onBlur: () => setFocused(false),
      })}
    >
      {screen.buffers && (
        <div
          data-slot="terminal-editor-bufferline"
          className="flex overflow-hidden"
          style={{ background: palette.bar, height: `${LINE_HEIGHT}em` }}
        >
          {screen.buffers.map((b) => {
            const look = {
              className: "shrink-0 px-[1ch]",
              style: b.front
                ? { background: palette.dot, color: palette.command }
                : { color: palette.muted },
            }
            return onOpen ? (
              <button
                key={b.path}
                type="button"
                data-slot="terminal-editor-buffer"
                data-active={b.front || undefined}
                // The terminal is one picture to assistive tech (role img, its
                // screen hidden): a tab is for the pointer, never a focus stop.
                tabIndex={-1}
                onClick={() => b.front || onOpen(b.path)}
                {...look}
                className={`${look.className} cursor-pointer whitespace-pre transition-colors hover:brightness-150`}
              >
                {b.label}
              </button>
            ) : (
              <span
                key={b.path}
                data-slot="terminal-editor-buffer"
                data-active={b.front || undefined}
                {...look}
              >
                {b.label}
              </span>
            )
          })}
        </div>
      )}
      <div
        data-slot="terminal-editor-text"
        className="relative"
        style={{ paddingTop: `${TEXT_TOP}em` }}
      >
        {screen.rows.map((row, r) => {
          const worst = row?.worst ?? null
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
                  width: `${screen.digits + 2}ch`,
                  color: row?.current ? palette.accent : palette.prompt,
                }}
              >
                {row?.first ? row.line + 1 : ""}
              </span>
              <span
                className={`relative min-w-0 flex-1${onEdit ? " cursor-text" : ""}`}
                onPointerDown={onEdit && row ? (e) => press(row, e) : undefined}
              >
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
                {row?.runs.map((run, k) => (
                  <span
                    // biome-ignore lint/suspicious/noArrayIndexKey: runs are positional
                    key={k}
                    data-slot={
                      run.undercurl ? "terminal-editor-undercurl" : undefined
                    }
                    style={{
                      color: run.color ?? undefined,
                      fontStyle: run.italic ? "italic" : undefined,
                      fontWeight: run.bold ? 700 : undefined,
                      background: run.selected
                        ? `color-mix(in oklab, ${palette.accent} ${Math.round(SELECTION_ALPHA * 100)}%, transparent)`
                        : undefined,
                      textDecoration: run.undercurl
                        ? `underline wavy ${SEVERITY[run.undercurl].ink}`
                        : undefined,
                    }}
                  >
                    {run.text}
                  </span>
                ))}
                {row?.last && worst && (
                  // Helix's end-of-line diagnostic: the worst one's message, after
                  // the line, in its severity's colour.
                  <span
                    data-slot="terminal-editor-eol"
                    style={{
                      color: SEVERITY[worst.severity].ink,
                      opacity: EOL_OPACITY,
                    }}
                  >
                    {EOL_GAP}
                    {worst.message}
                  </span>
                )}
                {caret?.row === r && (
                  <Caret
                    still={f.keyed < 0 || ms - f.keyed >= STILL_MS}
                    style={{
                      left: `${caret.column}ch`,
                      width: caret.insert ? "2px" : "1ch",
                      background: palette.accent,
                      mixBlendMode: caret.insert ? undefined : "difference",
                    }}
                  />
                )}
              </span>
            </div>
          )
        })}
        {screen.popup && (
          <Popup
            popup={screen.popup.popup}
            at={screen.popup}
            gutter={screen.gutter}
            rows={screen.rows.length}
          />
        )}
        {screen.picker && <Picker picker={screen.picker} />}
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
            background: status.insert ? palette.command : palette.accent,
            color: palette.bg,
          }}
        >
          {status.mode}
        </span>
        <span className="min-w-0 flex-1 truncate">{status.file}</span>
        {status.counts.map(({ severity, n }) => (
          <span
            key={severity}
            data-slot="terminal-editor-count"
            style={{ color: SEVERITY[severity].ink }}
          >
            ● {n}
          </span>
        ))}
        <span className="pr-[1ch]" style={{ color: palette.muted }}>
          {status.place}
        </span>
      </div>
      <div
        data-slot="terminal-editor-command"
        className="px-[1ch]"
        style={{ color: palette.muted }}
      >
        {screen.command.text || " "}
        {screen.command.prompt && (
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
  at: { row: number; column: number; below: boolean }
  gutter: number
  rows: number
}) {
  const palette = useTerminalPalette()
  const box: React.CSSProperties = {
    position: "absolute",
    zIndex: 10,
    left: `${gutter + at.column}ch`,
    border: `1px solid ${palette.prompt}`,
    background: palette.bar,
    color: palette.command,
    maxWidth: `calc(100% - ${gutter + at.column}ch)`,
    ...(at.below
      ? { top: `calc(${TEXT_TOP}em + ${(at.row + 1) * LINE_HEIGHT}em)` }
      : { bottom: `calc(100% - ${TEXT_TOP}em - ${at.row * LINE_HEIGHT}em)` }),
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
function Picker({ picker }: { picker: NonNullable<EditorScreen["picker"]> }) {
  const palette = useTerminalPalette()
  const list = picker.diagnostics
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
            no diagnostics in {picker.name}
          </div>
        )}
        {list.slice(0, MENU_ROWS).map((d, i) => {
          const p = placeOf(picker.text, d.from)
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
                {picker.name}:{p.line}:{p.column}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}
