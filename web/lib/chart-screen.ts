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

export interface ScreenFrames {
  /** Every style a run wears, by its place. */
  styles: ScreenStyle[]
  /** A frame per text of the scene, in its order: the first 16 hex digits of the text's
   *  sha256, and its rows, each `[text, style]` runs. */
  frames: { source: string; rows: [string, number][][] }[]
}

/** The screen's columns: the width `cells chart` falls back to. */
export const SCREEN_COLS = 80

/** How the page sets a screen, in em of the terminal's type: a character's advance (Geist
 *  Mono's), the screen's width with the window's insets either side, a row's pitch (where a
 *  block meets the block under it, so a bar is one bar), and the room above and below the
 *  rows. */
export const SCREEN_EM = {
  advance: 0.6,
  wide: SCREEN_COLS * 0.6 + 4,
  pitch: 1.18,
  pad: 0.6,
} as const

/** The rows a terminal window `width` by `height` (px) shows at the type that fits its
 *  columns to its width: its height less its bar, its hairlines and the room around the
 *  rows, in rows, whole. */
export function screenRows(width: number, height: number, barEm: number): number {
  const type = width / SCREEN_EM.wide
  const hairlines = 2
  return Math.floor(((height - hairlines) / type - barEm - 2 * SCREEN_EM.pad) / SCREEN_EM.pitch)
}
