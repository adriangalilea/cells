"use client"

// THE HERO'S TERMINAL: `cells chart spending.cells 1 4 --watch` as the real program drew
// it for the file as it stands (lib/hero-screens, `pnpm charts`). ui's terminal window
// with the screen as its program (`children`): each cell of the program's screen in its
// place, a glyph the page's mono face may lack held to one cell's width so no column
// moves, and the type a share of the box's width, so its 80 columns always fit it.

import type * as React from "react"
import { Terminal, useTerminalPalette } from "@/components/ui/terminal"
import { type ScreenColor, type ScreenFrames, SCREEN } from "@/lib/chart-screen"
import { WATCH_COMMAND } from "@/lib/hero"
import { SCREENS } from "@/lib/hero-screens"
import { ansiColor } from "@/lib/terminal-session"

/** A screen colour as the page draws it: the named sixteen and the palette in the site's
 *  terminal colours, the program's true colour as it is. */
const colourOf = (c: ScreenColor) => (typeof c === "number" ? ansiColor(c) : c)

/** The screen's rows at a pitch where a block meets the block under it, as a terminal
 *  draws them: a bar is one bar, never stripes. */
const PITCH = 1.18

/** Printable ASCII: what every mono face holds at its one width. */
const ASCII = /^[\x20-\x7e]$/

/** A block element as the share of its cell it fills, from its top and from its foot, as a
 *  terminal draws it: geometry, never the font's glyph, so a bar's cells meet with no seam. */
function blockOf(ch: string): { top: number; foot: number } | null {
  const c = ch.codePointAt(0) as number
  if (c === 0x2580) return { top: 1 / 2, foot: 0 }
  if (c >= 0x2581 && c <= 0x2588) return { top: 0, foot: (c - 0x2580) / 8 }
  if (c === 0x2594) return { top: 1 / 8, foot: 0 }
  return null
}

/** A run's text as the screen holds it: ASCII as text; a run of one block element as one
 *  box of its cells, filled; any other glyph held to its cell. */
function Glyphs({ text, ink, ground }: { text: string; ink: string; ground: string }) {
  const chars = [...text]
  const parts: React.ReactNode[] = []
  for (let i = 0; i < chars.length; ) {
    const ch = chars[i] as string
    let j = i + 1
    while (j < chars.length && chars[j] === ch && !ASCII.test(ch)) j++
    const block = blockOf(ch)
    if (ASCII.test(ch)) parts.push(ch)
    else if (block) {
      const from = block.top ? `${ink} 0 ${block.top * 100}%, ${ground} ${block.top * 100}%` : `${ground} 0 ${(1 - block.foot) * 100}%, ${ink} ${(1 - block.foot) * 100}%`
      parts.push(<span key={i} style={{ display: "inline-block", verticalAlign: "top", width: `${j - i}ch`, height: `${PITCH}em`, background: `linear-gradient(${from})` }} />)
    } else
      for (let k = i; k < j; k++)
        parts.push(
          <span key={k} style={{ display: "inline-block", verticalAlign: "top", width: "1ch", height: `${PITCH}em`, textAlign: "center", background: ground }}>
            {ch}
          </span>,
        )
    i = j
  }
  return parts
}

/** One frame of the program's screen. */
function Screen({ frame }: { frame: ScreenFrames["frames"][number] }) {
  const palette = useTerminalPalette()
  return (
    <div aria-hidden style={{ lineHeight: PITCH, paddingBlock: "0.6em", whiteSpace: "pre" }}>
      {frame.rows.map((row, r) => (
        <div key={r} style={{ height: `${PITCH}em` }}>
          {row.map(([text, k], i) => {
            const s = SCREENS.styles[k]
            if (!s) throw new Error(`charts: no style ${k} in the screens`)
            const ink = s.fg === undefined ? palette.accent : colourOf(s.fg)
            const ground = s.bg === undefined ? "transparent" : colourOf(s.bg)
            return (
              <span key={i} style={{ color: ink, background: s.bg === undefined ? undefined : ground, fontWeight: s.bold ? 700 : undefined }}>
                <Glyphs text={text} ink={ink} ground={ground} />
              </span>
            )
          })}
        </div>
      ))}
    </div>
  )
}

/** The terminal holding the charts of the hero's `index`th text (lib/hero `texts`, in the
 *  order the screens were drawn from them), in a box of any width. */
export function ChartTerminal({
  index,
  accent,
  className,
  style,
}: {
  index: number
  accent: string
  className?: string
  style?: React.CSSProperties
}) {
  const frame = SCREENS.frames[index]
  if (!frame) throw new Error("chart-terminal: the hero writes a text no screen was drawn from: pnpm charts")
  return (
    <div className={`[container-type:inline-size] ${className ?? ""}`} style={style}>
      <Terminal
        alt={`A terminal running ${WATCH_COMMAND}: spending by category and spent against budget, by month, drawn again each time the file changes.`}
        accent={accent}
        size={`calc(100cqw / ${SCREEN.cols * 0.6 + 4})`}
      >
        <Screen frame={frame} />
      </Terminal>
    </div>
  )
}
