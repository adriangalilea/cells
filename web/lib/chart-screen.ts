// A terminal's screen as `cells chart --watch` leaves it (scripts/charts.ts reads it, cell
// by cell, from the program's own bytes): its rows as runs of one style each. A colour is
// an xterm index (0..15 the named sixteen, drawn in the site's terminal palette) or six hex
// digits, the program's true colour.

export type ScreenColor = number | string

export interface ScreenStyle {
  fg?: ScreenColor
  bg?: ScreenColor
  bold?: true
  dim?: true
}

/** A screen's size, in a terminal's cells. */
export interface ScreenSize {
  cols: number
  rows: number
}

/** One frame: the first 16 hex digits of the sha256 of the text it was drawn from, and its
 *  rows, each `[text, style]` runs. */
export interface ScreenFrame {
  source: string
  rows: [string, number][][]
}

export interface ScreenFrames<Size extends string = string> {
  /** Every style a run wears, by its place. */
  styles: ScreenStyle[]
  /** For each terminal size, a frame per text of the scene, in its order. */
  sizes: Record<Size, ScreenSize & { frames: ScreenFrame[] }>
}

/** How the page sets a screen, in em of the terminal's type: a character's advance (Geist
 *  Mono's), the window's insets either side of the columns, a row's pitch (where a block
 *  meets the block under it, so a bar is one bar), and the room above and below the rows. */
export const SCREEN_EM = {
  advance: 0.6,
  insets: 4,
  pitch: 1.18,
  pad: 0.6,
} as const

/** A screen's width in em of its type: its columns and the window's insets. */
export const wideOf = (cols: number) => cols * SCREEN_EM.advance + SCREEN_EM.insets

/** The rows a terminal window `width` by `height` (px) shows at the type that fits `cols`
 *  columns to its width: its height less its bar, its hairlines and the room around the
 *  rows, in rows, whole. */
export function screenRows(cols: number, width: number, height: number, barEm: number): number {
  const type = width / wideOf(cols)
  const hairlines = 2
  return Math.floor(((height - hairlines) / type - barEm - 2 * SCREEN_EM.pad) / SCREEN_EM.pitch)
}
