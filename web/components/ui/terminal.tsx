"use client"

// A phosphor terminal that draws a session script (lib/terminal-session) at a moment:
// commands type char by char, output lands whole, `@ms` pauses. It draws; something
// else moves time: inside `<Player playable={sessionTimeline(…).playable}>` it plays, inside
// a scroll stage it scrolls, with a fixed `progress` it is a still. The still renderer in the same lib draws
// the identical frame as SVG, so a CLI's media and its live demo cannot drift.
// Display-only: role="img", zero focusables; a program that takes input (`interactive`)
// makes it a group whose screen is read.
//
// A full-screen program (an editor, `terminal-editor`) is `children` instead of a
// session: the window draws it on its screen, and hands it its palette
// (`useTerminalPalette`), so the program wears the window's phosphor.

import * as React from "react"
import {
  ansiColor,
  DIM_OPACITY,
  LINE_HEIGHT,
  parseSession,
  type SessionLine,
  sessionTimeline,
  spansOf,
  type TerminalPalette,
  TYPE_MS,
  terminalPalette,
  WINDOW,
} from "@/lib/terminal-session"
import { usePlayhead } from "@/components/ui/playback"

export type TerminalProps = TerminalWindowProps &
  (
    | ScriptProps
    | {
        /** A program that takes the whole screen (an editor, `terminal-editor`),
         *  drawn in the window in place of a session: the alternate screen a
         *  full-screen program runs in. It draws its own rows from the top-left
         *  corner, in the window's type and its palette (`useTerminalPalette`). */
        children: React.ReactNode
        /** The program takes input (an editor with `onEdit`): the window is a group
         *  named by `alt` and its screen is read, never one picture. */
        interactive?: boolean
        session?: never
        progress?: never
        rows?: never
      }
  )

interface ScriptProps {
  /** A session script (`$ cmd` · `~ muted` · plain · `# comment` · `@ms` pause). */
  session: string
  children?: never
  /** The moment drawn, 0..1 of the session, for a still. Inside a driver (a Player)
   *  omit it: the driver provides the moment. */
  progress?: number
  /** Minimum rows the frame reserves, so a short session still gets a window that
   *  feels like one (a hero). The script's own length always wins when longer. */
  rows?: number
}

interface TerminalWindowProps {
  /** One accent drives the whole palette (default: phosphor green). */
  accent?: string
  /** The type size, any CSS length; the whole frame scales with it. A frame that
   *  must fill its box, as a still fills its image, passes a share of the box's width
   *  (`cqw`, lib/terminal-session `stillSize()`). */
  size?: string
  /** Accessible description (the terminal is one picture). */
  alt: string
  /** What the window's bar says, centred as a Mac's terminal says what runs in it (a
   *  full-screen program's command, `cells chart spending.cells 1 --watch`). None, the
   *  bar holds its dots alone. */
  title?: string
  className?: string
}

/** The chrome is drawn at the window's own type (lib/terminal-session `WINDOW`, which
 *  a still's window reads too) and kept in em, so it scales with `size`. */
const em = (px: number) => `${px / WINDOW.type}em`

/** The palette a program in the window draws with (`children`): the window's own,
 *  from its accent. */
const PaletteContext = React.createContext<TerminalPalette | null>(null)

/** The palette of the terminal around: what a full-screen program inside it
 *  (`children`) draws with, so it wears the window's phosphor. Screams outside one. */
export function useTerminalPalette(): TerminalPalette {
  const palette = React.useContext(PaletteContext)
  if (!palette)
    throw new Error(
      "useTerminalPalette: no terminal around; place the program inside <Terminal>",
    )
  return palette
}

export function Terminal(props: TerminalProps) {
  if (props.session === undefined) {
    const { children, interactive, accent, size, alt, title, className } = props
    return (
      <TerminalWindow
        accent={accent}
        size={size}
        alt={alt}
        title={title}
        interactive={interactive}
        className={className}
      >
        {children}
      </TerminalWindow>
    )
  }
  return <TerminalScript {...props} />
}

/** The window: its bar and its dots, the screen under them in the palette. */
function TerminalWindow({
  accent,
  size = "13px",
  alt,
  title,
  interactive = false,
  className,
  children,
}: TerminalWindowProps & {
  children: React.ReactNode
  interactive?: boolean
}) {
  const palette = React.useMemo(() => terminalPalette(accent), [accent])
  return (
    // biome-ignore lint/a11y/useAriaPropsSupportedByRole: img and group both take a name
    <div
      data-slot="terminal"
      className={className}
      role={interactive ? "group" : "img"}
      aria-label={alt}
      style={{
        // A column: the bar, then the screen taking what is left of a window given
        // a size (a fixed frame), or as much as it holds when it sizes itself.
        display: "flex",
        flexDirection: "column",
        background: palette.bg,
        borderRadius: em(WINDOW.radius),
        overflow: "hidden",
        // Parted from the page by a hairline in its own accent and a soft, low
        // shadow: a window resting on it, not a hole in it.
        border: `1px solid ${palette.edge}`,
        boxShadow: `0 ${em(1)} ${em(2)} rgb(0 0 0 / 0.25), 0 ${em(18)} ${em(40)} -${em(12)} rgb(0 0 0 / 0.55)`,
        fontFamily: "var(--font-mono, Menlo, Monaco, monospace)",
        fontSize: size,
        lineHeight: LINE_HEIGHT,
      }}
    >
      <div
        aria-hidden={interactive ? undefined : true}
        style={{
          display: "flex",
          flexDirection: "column",
          flex: 1,
          minHeight: 0,
        }}
      >
        <div
          data-slot="terminal-bar"
          aria-hidden={interactive || undefined}
          style={{
            display: "flex",
            gap: em(WINDOW.gap),
            alignItems: "center",
            background: palette.bar,
            padding: `${em(WINDOW.padY)} ${em(WINDOW.padX)}`,
            position: "relative",
          }}
        >
          {[0, 1, 2].map((d) => (
            <span
              key={d}
              style={{
                width: em(WINDOW.dot),
                height: em(WINDOW.dot),
                borderRadius: "50%",
                background: palette.dot,
              }}
            />
          ))}
          {title && (
            // Centred on the window, never on what is left beside the dots, so it
            // stands where a Mac's terminal puts it.
            <span
              style={{
                position: "absolute",
                insetInline: 0,
                textAlign: "center",
                color: palette.muted,
                pointerEvents: "none",
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
              }}
            >
              {title}
            </span>
          )}
        </div>
        <PaletteContext.Provider value={palette}>
          <div
            data-slot="terminal-screen"
            // The grid inset so its first column's centre stands under the first dot's
            // (an editor's diagnostic mark centred below it), and as far from the right
            // edge, so no line ends on the frame. `--terminal-inset` is it, for a row
            // that runs edge to edge (an editor's bars) to reach back through it, as a
            // terminal extends a row's ground into its padding.
            style={
              {
                flex: 1,
                minHeight: 0,
                overflow: "hidden",
                "--terminal-inset": `calc(${em(WINDOW.padX + WINDOW.dot / 2)} - 0.5ch)`,
                paddingInline: "var(--terminal-inset)",
              } as React.CSSProperties
            }
          >
            {children}
          </div>
        </PaletteContext.Provider>
      </div>
    </div>
  )
}

function TerminalScript({
  session,
  progress,
  accent,
  rows = 0,
  size,
  alt,
  title,
  className,
}: TerminalWindowProps & ScriptProps) {
  const timeline = React.useMemo(
    () => sessionTimeline(parseSession(session)),
    [session],
  )
  const palette = React.useMemo(() => terminalPalette(accent), [accent])
  const at = usePlayhead(progress).at * timeline.total
  let full = 0
  while (full < timeline.lines.length && (timeline.ends[full] as number) <= at)
    full++
  const current =
    full < timeline.lines.length ? timeline.lines[full] : undefined
  const start =
    full < timeline.lines.length ? (timeline.starts[full] as number) : 0
  const typedChars =
    current && current.kind === "command" && at > start
      ? Math.max(0, Math.floor((at - start) / TYPE_MS))
      : 0
  const done = full >= timeline.lines.length

  // A session taller than its window scrolls as a terminal does: the newest line
  // stays at the screen's foot and the oldest leave at its top. Measured whenever
  // the live lines or the screen change size, never per frame; a window sized by
  // the whole session never overflows, so it never moves.
  const sheet = React.useRef<HTMLDivElement>(null)
  const live = React.useRef<HTMLDivElement>(null)
  React.useLayoutEffect(() => {
    const page = sheet.current
    const layer = live.current
    const screen = page?.parentElement
    if (!page || !layer || !screen)
      throw new Error("terminal: the screen rendered without its sheet")
    const follow = () => {
      const below = Number.parseFloat(getComputedStyle(page).paddingBottom)
      const bottom =
        layer.getBoundingClientRect().bottom -
        page.getBoundingClientRect().top +
        below
      screen.scrollTop = Math.max(0, bottom - screen.clientHeight)
    }
    follow()
    const observer = new ResizeObserver(follow)
    observer.observe(layer)
    observer.observe(screen)
    return () => observer.disconnect()
  }, [])

  const prompt = (key: string, cursor: boolean) => (
    <div key={key}>
      <span style={{ color: palette.prompt }}>$ </span>
      {cursor && <span style={{ color: palette.dot }}>█</span>}
    </div>
  )

  const renderLine = (line: SessionLine, key: number, chars?: number) => {
    if (line.kind === "blank") return <div key={key}>&nbsp;</div>
    if (line.kind === "command")
      return (
        <div key={key}>
          <span style={{ color: palette.prompt }}>$ </span>
          <span style={{ color: palette.command }}>
            {chars === undefined ? line.text : line.text.slice(0, chars)}
          </span>
        </div>
      )
    return (
      <div
        key={key}
        style={{
          color: line.kind === "muted" ? palette.muted : palette.accent,
        }}
      >
        {spansOf(line.text).map((s) =>
          s.color === null && !s.bold && !s.dim ? (
            s.text
          ) : (
            <span
              key={s.at}
              style={{
                color: s.color === null ? undefined : ansiColor(s.color),
                fontWeight: s.bold ? 700 : undefined,
                opacity: s.dim ? DIM_OPACITY : undefined,
              }}
            >
              {s.text}
            </span>
          ),
        )}
      </div>
    )
  }

  return (
    <TerminalWindow
      accent={accent}
      size={size}
      alt={alt}
      title={title}
      className={className}
    >
      {/* The box is sized by the FINISHED session (an invisible full copy in the
          same cell), so playback never grows the page: text streams into a fixed
          frame. Both layers share width, so wrapping is identical. */}
      <div
        ref={sheet}
        style={{
          display: "grid",
          padding: `${em(20)} ${em(24)} ${em(24)}`,
          whiteSpace: "pre-wrap",
          overflowWrap: "anywhere",
        }}
      >
        <div
          style={{
            gridArea: "1 / 1",
            visibility: "hidden",
            minHeight: `${rows * LINE_HEIGHT}em`,
          }}
        >
          {timeline.lines.map((l, i) => renderLine(l, i))}
          {prompt("end", true)}
        </div>
        {/* Its own height, never the cell's (which the finished copy sets): the
            screen follows the lines that have landed. */}
        <div ref={live} style={{ gridArea: "1 / 1", alignSelf: "start" }}>
          {timeline.lines.slice(0, full).map((l, i) => renderLine(l, i))}
          {current &&
            current.kind === "command" &&
            typedChars > 0 &&
            renderLine(current, full, typedChars)}
          {prompt("live", done)}
        </div>
      </div>
    </TerminalWindow>
  )
}
