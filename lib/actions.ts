// ACTIONS: what a part of a page does when a key asks, declared once as data, so the
// keys that answer, the menus and tooltips that teach them, `aria-keyshortcuts` and
// the page's sheet of keys are all read from one place and can never disagree.
// Framework-free; key-router binds a set to the page (`useActions`), ui/key-sheet
// lists every set on the page. The twin of swift-utils' Keymap (ActionSet, ActionSpec).
//
// A BINDING is a verb (`id`), what it is called (`label`), its keys in lib/keys
// notation ("mod+z", "shift+left", a sequence "g g"; several are alternatives), and
// optionally the MODE it answers in. A set lists its modes innermost first (a
// lightbox's sheet over its zoom over its fit; a player studying a paused frame over
// watching): at a press, the innermost active mode that binds the key wins, and a
// binding with no mode answers in every mode, last. One verb may be bound in several
// modes (a player's frame-back on ← while studying, on , whenever frames are known).
//
// DECLARED, CHECKED: `declare` throws at once, naming what is wrong, for a key that is
// not keys, a chord the browser keeps for itself (⌘W ⌘T ⌘N ⌘Q ⌘⇧T ⌘⇧N ⌘⇧W, Ctrl
// elsewhere: no page can take them), a mode the set does not list, and two bindings of
// one key in one mode (a dispatch nobody could predict). A chord the browser answers
// but lets a focused page take (⌘F, ⌘G, ⌘S…) is answered only while its part holds
// the focus, the pointer or was used last, unless the binding says
// `takesBrowserChord`, which the sheet marks.

import {
  ariaKeyshortcuts,
  combo,
  type Platform,
  type Press,
  pressed,
  keys as sequence,
} from "@/lib/keys"

export interface ActionSpec {
  /** The verb: what a menu, a tooltip or a dispatcher names it by. */
  readonly id: string
  /** What it does, in a few words ("delete the row"). */
  readonly label: string
  /** Its keys, alternatives, each one combination ("mod+z") or a sequence ("g g").
   *  None: a verb with no key (a menu's alone). */
  readonly keys: readonly string[]
  /** Where the sheet lists it ("rows", "edit"). */
  readonly section?: string
  /** The mode it answers in; none, every mode. */
  readonly mode?: string
  /** A key held down repeats it; false: a held key is one press. */
  readonly repeat?: boolean
  /** How a legend writes the keys where listing them all would not help ("1-9"). */
  readonly shown?: string
  readonly destructive?: boolean
  /** Answers its browser chord page-wide, not only while its part is the one in hand. */
  readonly takesBrowserChord?: boolean
  /** Answered only while its part holds the focus, never for the pointer resting over
   *  it or its being used last: a grid's arrows move its cursor only from the grid. */
  readonly needsFocus?: boolean
  /** Unavailable, the press goes on to the next part instead of being kept idle: a
   *  draft's own undo, under the page's history, which takes ⌘Z when there is no
   *  draft. */
  readonly passWhenIdle?: boolean
}

/** A binding a page runs: the spec, what it does, and whether it can now. */
export interface Action<Icon = unknown> extends ActionSpec {
  run: () => void
  /** Available now (something to undo, a row selected). An unavailable action keeps
   *  its key: the part it belongs to answers the press by doing nothing, never by
   *  letting it fall to another part. */
  when?: () => boolean
  icon?: Icon
}

export interface ActionSet<A extends ActionSpec = ActionSpec> {
  /** What the part is called on the page's sheet ("budget.csv", "player"). */
  readonly title: string
  /** Its modes, innermost first. */
  readonly modes?: readonly string[]
  readonly actions: readonly A[]
}

/** The chords a browser never lets a page take, as written for every platform: Chrome
 *  and Safari never send them to the page at all (Chromium since Chrome 4), so a page
 *  cannot keep a person from leaving it by the keyboard. The tab switches are Ctrl on
 *  every platform. Firefox lets more through, never these. */
export const RESERVED: readonly string[] = [
  "mod+w",
  "mod+t",
  "mod+n",
  "mod+q",
  "mod+shift+t",
  "mod+shift+n",
  "mod+shift+w",
  "ctrl+tab",
  "ctrl+shift+tab",
  "ctrl+pageup",
  "ctrl+pagedown",
]

/** The chords a browser answers but lets a focused page take: find, find next, the
 *  bookmark, the address bar, save, print, reload, the downloads, history, the source. */
export const BROWSER_CHORDS: readonly string[] = [
  "mod+f",
  "mod+g",
  "mod+shift+g",
  "mod+d",
  "mod+l",
  "mod+s",
  "mod+p",
  "mod+r",
  "mod+e",
  "mod+j",
  "mod+y",
  "mod+u",
  "mod+o",
]

const PLATFORMS: readonly Platform[] = ["mac", "other"]

/** A key as one comparable string on a platform ("⌘Z", "G G"), or null when not keys. */
function signature(text: string, platform: Platform): string | null {
  const seq = sequence(text, platform)
  return seq ? seq.map((c) => c.join("")).join(" ") : null
}

const among = (text: string, list: readonly string[]) =>
  PLATFORMS.some((p) => {
    const s = signature(text, p)
    return s !== null && list.some((r) => signature(r, p) === s)
  })

/** A chord no page can take. */
export const reserved = (text: string) => among(text, RESERVED)
/** A chord the browser answers unless a focused page takes it. */
export const browserChord = (text: string) => among(text, BROWSER_CHORDS)

/** A set checked (see the header), returned as it was given. */
export function declare<A extends ActionSpec>(set: ActionSet<A>): ActionSet<A> {
  const modes = new Set(set.modes ?? [])
  const seen = new Map<string, A>()
  for (const a of set.actions) {
    if (a.mode !== undefined && !modes.has(a.mode))
      throw new Error(
        `actions: ${set.title}'s ${a.id} answers in mode "${a.mode}", which the set does not list`,
      )
    for (const k of a.keys) {
      for (const p of PLATFORMS) {
        const s = signature(k, p)
        if (s === null)
          throw new Error(`actions: ${set.title}'s ${a.id}: "${k}" is not keys`)
        const at = `${a.mode ?? ""}\u0000${p}\u0000${s}`
        const other = seen.get(at)
        if (other && other.id !== a.id)
          throw new Error(
            `actions: ${set.title} binds "${k}" twice${a.mode ? ` in mode "${a.mode}"` : ""}: ${other.id} and ${a.id}`,
          )
        seen.set(at, a)
      }
      if (reserved(k))
        throw new Error(
          `actions: ${set.title}'s ${a.id} takes "${k}", which the browser keeps for itself`,
        )
    }
  }
  return set
}

/** A set with some bindings' keys replaced (a person's remapping), checked again. */
export function remapped<A extends ActionSpec>(
  set: ActionSet<A>,
  keysOf: ReadonlyMap<string, readonly string[]>,
): ActionSet<A> {
  if (keysOf.size === 0) return set
  return declare({
    ...set,
    actions: set.actions.map((a) => {
      const k = keysOf.get(a.id)
      return k ? { ...a, keys: k } : a
    }),
  })
}

/** The binding that already holds `key` where the binding `id` answers (its mode), on
 *  either platform, or null: who a new key for `id` would collide with, named before
 *  anything is kept. */
export function holderOf<A extends ActionSpec>(
  set: ActionSet<A>,
  id: string,
  key: string,
): A | null {
  const mine = set.actions.find((a) => a.id === id)
  if (!mine) throw new Error(`actions: ${set.title} has no action "${id}"`)
  return (
    set.actions.find(
      (a) =>
        a.id !== id &&
        a.mode === mine.mode &&
        a.keys.some((k) =>
          PLATFORMS.some((p) => {
            const s = signature(k, p)
            return s !== null && s === signature(key, p)
          }),
        ),
    ) ?? null
  )
}

/** What a press asks of a set: the binding, and whether it can answer now. */
export interface Match<A extends ActionSpec> {
  action: A
  /** Which of its keys the press is. */
  key: string
  /** False: the binding is the set's, but has nothing to do (unavailable, or a held
   *  key on a binding that does not repeat). The part keeps the press anyway. */
  available: boolean
}

/** A press as it arrives: a key press, and whether it is a held key's repeat. */
export interface ActionPress extends Press {
  repeat?: boolean
}

/** The modes of a set in the order a press tries them: the active ones, innermost
 *  first, then no mode. */
function order<A extends ActionSpec>(
  set: ActionSet<A>,
  active: ReadonlySet<string>,
): (string | undefined)[] {
  return [...(set.modes ?? []).filter((m) => active.has(m)), undefined]
}

/** Whether a binding's key `text` is this press, a sequence's last combination
 *  completing the presses before it (`before`, oldest first). */
function hits(
  text: string,
  press: ActionPress,
  before: readonly Press[],
  platform: Platform,
): boolean {
  const parts = text.trim().split(/\s+/)
  if (parts.length === 1) return pressed(text, press, platform)
  if (!pressed(parts.at(-1) as string, press, platform)) return false
  const earlier = parts.slice(0, -1)
  if (before.length < earlier.length) return false
  const tail = before.slice(before.length - earlier.length)
  return earlier.every((p, i) => pressed(p, tail[i] as Press, platform))
}

/** The binding a press asks for, the innermost active mode first, or null. */
export function match<A extends ActionSpec>(
  set: ActionSet<A>,
  press: ActionPress,
  {
    platform,
    active = new Set(),
    before = [],
  }: {
    platform: Platform
    /** The modes on now. */
    active?: ReadonlySet<string>
    /** The presses just before this one, oldest first, for sequences. */
    before?: readonly Press[]
  },
): Match<A> | null {
  for (const mode of order(set, active)) {
    for (const a of set.actions) {
      if (a.mode !== mode) continue
      const key = a.keys.find((k) => hits(k, press, before, platform))
      if (key === undefined) continue
      const when = (a as Partial<Action>).when
      const able = when ? when() : true
      return {
        action: a,
        key,
        available: able && !(press.repeat && a.repeat === false),
      }
    }
  }
  return null
}

/** Whether any binding of the set, in any mode, is this press: what a part that
 *  swallows its own keys (a modal) holds back from the page. */
export const claims = <A extends ActionSpec>(
  set: ActionSet<A>,
  press: Press,
  platform: Platform,
) => set.actions.some((a) => a.keys.some((k) => pressed(k, press, platform)))

/** A binding's keys as `aria-keyshortcuts` names them (its single combinations: a
 *  sequence is no attribute's to say), or undefined when it has none. */
export function ariaOf(
  action: ActionSpec,
  platform: Platform,
): string | undefined {
  const single = action.keys.filter((k) => !/\s/.test(k.trim()))
  if (single.length === 0) return undefined
  const named = single.map((k) =>
    // `mod` as the platform's own, so a screen reader hears Meta on a Mac and Control
    // elsewhere.
    k.replace(/\bmod\b/gi, platform === "mac" ? "cmd" : "ctrl"),
  )
  return ariaKeyshortcuts(named)
}

/** A row of a sheet: one label, every key that does it, the bindings behind it. */
export interface SheetRow {
  label: string
  keys: string[]
  ids: string[]
  /** How a legend writes the keys, when a binding says so. */
  shown?: string
  /** A binding taking a browser chord page-wide. */
  takesBrowserChord: boolean
}
export interface SheetSection {
  section: string
  rows: SheetRow[]
}

/** A set as its sheet lists it: what answers now (the active modes and no mode; an
 *  unavailable binding is gone, not dimmed), grouped by section in the order the set
 *  first names them, bindings that share a label joined into one row (the four pans).
 *  A binding with no key is no row. */
export function sheetOf<A extends ActionSpec>(
  set: ActionSet<A>,
  {
    active = new Set(),
    available = () => true,
  }: {
    active?: ReadonlySet<string>
    available?: (a: A) => boolean
  } = {},
): SheetSection[] {
  const out: SheetSection[] = []
  for (const a of set.actions) {
    if (a.mode !== undefined && !active.has(a.mode)) continue
    if (a.keys.length === 0 || !available(a)) continue
    const name = a.section ?? ""
    let section = out.find((s) => s.section === name)
    if (!section) {
      section = { section: name, rows: [] }
      out.push(section)
    }
    const row = section.rows.find((r) => r.label === a.label)
    if (row) {
      for (const k of a.keys) if (!row.keys.includes(k)) row.keys.push(k)
      if (!row.ids.includes(a.id)) row.ids.push(a.id)
      row.takesBrowserChord ||= !!a.takesBrowserChord
    } else
      section.rows.push({
        label: a.label,
        keys: [...a.keys],
        ids: [a.id],
        shown: a.shown,
        takesBrowserChord: !!a.takesBrowserChord,
      })
  }
  return out
}

/** A press written as one combination in lib/keys notation ("mod+shift+k", "?"), for a
 *  person recording a new key: null when the press is a modifier alone. */
export function written(press: Press, platform: Platform): string | null {
  if (["Meta", "Control", "Alt", "Shift"].includes(press.key)) return null
  const mods = [
    press.ctrlKey && (platform === "mac" ? "ctrl" : "mod"),
    press.altKey && "alt",
    press.shiftKey && "shift",
    press.metaKey && (platform === "mac" ? "mod" : "cmd"),
  ].filter(Boolean)
  const NAMES: Record<string, string> = {
    " ": "space",
    Enter: "enter",
    Escape: "esc",
    Backspace: "backspace",
    Delete: "delete",
    Tab: "tab",
    ArrowUp: "up",
    ArrowDown: "down",
    ArrowLeft: "left",
    ArrowRight: "right",
    Home: "home",
    End: "end",
  }
  const physical = /^(?:Key([A-Z])|Digit([0-9]))$/.exec(press.code ?? "")
  const key =
    NAMES[press.key] ??
    (/^[a-z0-9]$/i.test(press.key)
      ? press.key.toLowerCase()
      : physical
        ? (physical[1] ?? physical[2] ?? "").toLowerCase()
        : press.key)
  // A symbol is typed with shift as the layout types it (? is ⇧/): shift is part of
  // the character, as `pressed` reads it, never written beside it.
  const symbol = [...key].length === 1 && !/[a-z0-9]/i.test(key)
  const text = [...mods.filter((m) => !(symbol && m === "shift")), key].join(
    "+",
  )
  return combo(text, platform) ? text : null
}
