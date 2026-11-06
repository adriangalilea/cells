// The lightbox's actions (lib/actions): the single source for key dispatch, buttons,
// tooltips, aria-keyshortcuts, the live region and the `?` sheet (`sheet()` renders
// the table for one layer set). Every pointer verb dispatches an id from this table
// too. Its layers are the set's modes, innermost first; a binding with no mode answers
// in every layer. `resolve` hands a press to the innermost active layer that binds it
// (lib/actions' `match`). Framework-free.

import {
  type ActionPress,
  type ActionSpec,
  claims,
  declare,
  match,
} from "@/lib/actions"
import type { Platform, Press } from "@/lib/keys"

export type Layer =
  | "always"
  | "fit"
  | "zoomed"
  | "video"
  | "sheet"
  | "fullscreen"

/** The layers that are modes, innermost first: the order `resolve` and the Escape
 *  ladder walk. "always" is no mode: what it binds answers whatever is on. */
const MODES = ["sheet", "fullscreen", "zoomed", "fit", "video"] as const

export type Section = "navigate" | "zoom" | "video" | "view"
/** Table order: the `?` sheet groups rows by section in this order. */
export const SECTIONS: readonly Section[] = [
  "navigate",
  "zoom",
  "video",
  "view",
]

export interface Action extends ActionSpec {
  readonly section: Section
  readonly mode?: (typeof MODES)[number]
}

export const ACTIONS = [
  {
    id: "prev",
    keys: ["left"],
    mode: "fit",
    section: "navigate",
    label: "previous",
    repeat: false,
  },
  {
    id: "next",
    keys: ["right"],
    mode: "fit",
    section: "navigate",
    label: "next",
    repeat: false,
  },
  {
    id: "step.prev",
    keys: ["shift+left"],
    mode: "zoomed",
    section: "navigate",
    label: "previous",
  },
  {
    id: "step.next",
    keys: ["shift+right"],
    mode: "zoomed",
    section: "navigate",
    label: "next",
  },
  { id: "first", keys: ["home"], section: "navigate", label: "first" },
  { id: "last", keys: ["end"], section: "navigate", label: "last" },
  { id: "zoom.in", keys: ["+", "="], section: "zoom", label: "zoom in" },
  { id: "zoom.out", keys: ["-", "_"], section: "zoom", label: "zoom out" },
  {
    id: "zoom.fit",
    keys: ["esc", "0"],
    mode: "zoomed",
    section: "zoom",
    label: "fit",
  },
  {
    id: "pan.left",
    keys: ["left"],
    mode: "zoomed",
    section: "zoom",
    label: "pan",
  },
  {
    id: "pan.right",
    keys: ["right"],
    mode: "zoomed",
    section: "zoom",
    label: "pan",
  },
  { id: "pan.up", keys: ["up"], mode: "zoomed", section: "zoom", label: "pan" },
  {
    id: "pan.down",
    keys: ["down"],
    mode: "zoomed",
    section: "zoom",
    label: "pan",
  },
  {
    id: "play",
    keys: ["space", "k"],
    mode: "video",
    section: "video",
    label: "play / pause",
  },
  {
    id: "seek.back",
    keys: ["j"],
    mode: "video",
    section: "video",
    label: "-10s",
  },
  {
    id: "seek.fwd",
    keys: ["l"],
    mode: "video",
    section: "video",
    label: "+10s",
  },
  { id: "mute", keys: ["m"], mode: "video", section: "video", label: "mute" },
  { id: "rail", keys: ["i"], section: "view", label: "details" },
  { id: "strip", keys: ["t"], section: "view", label: "thumbnails" },
  { id: "chrome", keys: ["h"], section: "view", label: "hide chrome" },
  { id: "fullscreen", keys: ["f"], section: "view", label: "fullscreen" },
  { id: "open", keys: ["o"], section: "view", label: "open original" },
  { id: "sheet", keys: ["?"], section: "view", label: "keys" },
  {
    id: "sheet.close",
    keys: ["esc"],
    mode: "sheet",
    section: "view",
    label: "close keys",
  },
  { id: "close", keys: ["esc"], mode: "fit", section: "view", label: "close" },
] as const satisfies readonly Action[]

export type ActionId = (typeof ACTIONS)[number]["id"]

/** The set, checked at load: a key bound twice in one layer screams here. */
export const LIGHTBOX = declare<Action>({
  title: "lightbox",
  modes: MODES,
  actions: ACTIONS,
})

const layerOf = (a: Action): Layer => a.mode ?? "always"

export function action(id: ActionId): Action {
  const found = ACTIONS.find((a) => a.id === id)
  if (!found) throw new Error(`lightbox: unknown action ${id}`)
  return found
}

/** The innermost active layer's binding of `press`, or null. */
export function resolve(
  press: ActionPress,
  layers: ReadonlySet<Layer>,
  platform: Platform,
): Action | null {
  return match(LIGHTBOX, press, { platform, active: layers })?.action ?? null
}

/** Whether any layer binds the press: what the dialog swallows while open. */
export const bound = (press: Press, platform: Platform) =>
  claims(LIGHTBOX, press, platform)

/** The Escape ladder, one rung per press: sheet closes, fullscreen exits (the browser
 *  does it, so null: we never double), zoomed springs to fit, then close. */
export function escRung(layers: ReadonlySet<Layer>): ActionId | null {
  if (layers.has("sheet")) return "sheet.close"
  if (layers.has("fullscreen")) return null
  if (layers.has("zoomed")) return "zoom.fit"
  return "close"
}

export type SheetRow = { keys: readonly string[]; label: string }
export type SheetSection = { section: Section; rows: SheetRow[] }

/** The `?` sheet: only what the world BEHIND the sheet answers to, grouped by section
 *  in table order. `sheet` is dropped from the layers (the sheet lists what its close
 *  reveals); `unavailable` ids are gone, not dimmed; esc appears once, on its current
 *  rung; actions sharing a label collapse into one row (the four pans). A row left
 *  with no key is not a row. */
export function sheet(
  layers: ReadonlySet<Layer>,
  unavailable: ReadonlySet<string>,
): SheetSection[] {
  const behind = new Set(layers)
  behind.delete("sheet")
  const rung = escRung(behind)
  const out: SheetSection[] = []
  for (const a of ACTIONS as readonly Action[]) {
    if (!behind.has(layerOf(a)) || unavailable.has(a.id)) continue
    const keys = a.keys.filter((k) => k !== "esc" || a.id === rung)
    if (keys.length === 0) continue
    let section = out[out.length - 1]
    if (!section || section.section !== a.section) {
      section = { section: a.section, rows: [] }
      out.push(section)
    }
    const row = section.rows.find((r) => r.label === a.label)
    if (row) row.keys = [...row.keys, ...keys]
    else section.rows.push({ keys, label: a.label })
  }
  return out
}

// The sheet groups by section as it walks the table: a section split in two is a
// table out of order.
{
  const seen: Section[] = []
  for (const a of ACTIONS) {
    const last = seen[seen.length - 1]
    if (a.section === last) continue
    if (seen.includes(a.section))
      throw new Error(`lightbox: section "${a.section}" is split in the table`)
    seen.push(a.section)
  }
  if (seen.join() !== SECTIONS.join())
    throw new Error("lightbox: the table walks sections out of SECTIONS order")
}
