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

/** The terminal the frames are drawn in: the width `cells chart` falls back to, and tall
 *  enough for the two charts' cards, each given half. */
export const SCREEN = { cols: 80, rows: 44 } as const
