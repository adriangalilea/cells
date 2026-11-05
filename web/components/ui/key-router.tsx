"use client"

// WHO A KEY BELONGS TO, page-wide. Every item that answers keys on the page (a player's
// Space and arrows, a gallery's quick look, a list's letters, a lightbox) registers
// here instead of listening on the document itself, and ONE listener hands each press
// to one of them:
//
//   1. Nothing at all when the press belongs to what it landed in (`ownsKeys`, lib/keys):
//      already taken, typed into a field, or a key a widget answers itself (Space on a
//      switch, an arrow in tabs). A shortcut never also does what the focused control
//      just did.
//   2. A modal (an open lightbox) takes every press; nothing behind it hears one.
//   3. Else the instances in order: the one holding focus (the deepest first), the
//      one under the pointer, the one last pressed or focused in, then the rest, newest
//      first. Each says whether the press was its (`onKey` returns true) and the first
//      that takes it ends the press: a gallery and a player on one page never both
//      answer Space. A part IN HAND (focus, pointer or last used) may also answer
//      "idle": the key is its, with nothing to do now (⌘Z with nothing to undo). That
//      ends the press too, untaken, so the key never falls to another part the person
//      is not in. A part reached only as the newest has no such claim.
//   4. Never an instance or a layer whose root is inside an `inert` subtree: inert is
//      the platform saying nothing in there takes interaction (a picture of a control,
//      a page under a modal of its own), and a key is interaction.
//   5. An instance inside `[data-keys="held"]` only while the hand holds it: focus is
//      in it, or it was the last thing pressed or focused in. Never for the pointer
//      resting over it, never page-wide. For a page of live examples (a gallery of
//      cards, each a real player or list): every example works under the hand, and
//      the page's own keys stay the page's.
//
// THE ESCAPE LADDER is the same listener: the layers open now (a window-filling
// player, a full view, a sheet of keys) in the order they opened, and Esc closes the
// innermost one only, and only when nothing inside it took the Esc first (a menu open
// over the full view closes itself, not the view). Base UI's own popups and dialogs
// close themselves and are seen here as roles that own Escape.
//
// It installs itself with its first instance and leaves with its last, so a page with no
// keyboard items carries no listener at all.
//
// It listens on the document, after the page's own handlers, so what stops a press on
// its way up never reaches it: Base UI's dialog popups stop the arrows, Home and End.
// An item answering those inside a Base UI dialog (the lightbox) answers on its own
// element, asking `ownedHere` first, and registers here too (as a modal) so nothing
// behind it hears a key.

import * as React from "react"
import {
  type Action,
  type ActionSet,
  type ActionSpec,
  browserChord,
  declare,
  match,
  remapped,
} from "@/lib/actions"
import {
  type KeyPress,
  type KeyTarget,
  ownsKeys,
  type Platform,
  type Press,
  platformOf,
} from "@/lib/keys"

/** Input types a person types text into. */
const TEXT_INPUTS = new Set([
  "text",
  "search",
  "email",
  "url",
  "tel",
  "password",
  "number",
  "date",
  "datetime-local",
  "month",
  "time",
  "week",
])

/** An element's role, written or implied by its tag (`<button>` is a button, a range
 *  input a slider). */
function roleOf(el: Element): string | null {
  const written = el.getAttribute("role")?.trim().split(/\s+/)[0]
  if (written) return written
  switch (el.tagName) {
    case "BUTTON":
    case "SUMMARY":
      return "button"
    case "A":
      return el.hasAttribute("href") ? "link" : null
    case "SELECT":
      return "listbox"
    case "INPUT": {
      const type = (el as HTMLInputElement).type
      if (type === "checkbox") return "checkbox"
      if (type === "radio") return "radio"
      if (type === "range") return "slider"
      if (
        ["button", "submit", "reset", "image", "color", "file"].includes(type)
      )
        return "button"
      return null
    }
    default:
      return null
  }
}

function editable(el: Element): boolean {
  if (el instanceof HTMLTextAreaElement) return !el.readOnly
  if (el instanceof HTMLInputElement)
    return TEXT_INPUTS.has(el.type) && !el.readOnly
  return el instanceof HTMLElement && el.isContentEditable
}

/** What a key landed in, as `ownsKeys` reads it: whether it takes typing, and the roles
 *  from it up to `root` (not included), nearest first. `own` are the roles of the
 *  listener's own parts, left out: a gallery moving focus over its options, a timeline
 *  over its ruler, answers those keys itself. */
export function keyTargetOf(
  target: EventTarget | null,
  root: Element | null = null,
  own: readonly string[] = [],
): KeyTarget {
  const start = target instanceof Element ? target : null
  const roles: string[] = []
  let typing = false
  for (let el = start; el && el !== root; el = el.parentElement) {
    if (el === document.body || el === document.documentElement) break
    if (el === start && editable(el)) typing = true
    const role = roleOf(el)
    if (role && !own.includes(role)) roles.push(role)
  }
  return { editable: typing, roles }
}

/** Whether `event` belongs to what it landed in (lib/keys' `ownsKeys`), read from the
 *  page, with roles counted up to `root` (an item's own element, whose own role does
 *  not count against it) and the listener's `own` roles left out. Every keyboard
 *  handler in the registry asks this before it answers a key. */
export function ownedHere(
  event: KeyboardEvent,
  root: Element | null = null,
  own: readonly string[] = [],
): boolean {
  return ownsKeys(event as KeyPress, keyTargetOf(event.target, root, own))
}

/** Whether something typed lands in a field (a text field, a textarea, an editable
 *  element): what a clipboard handler or a grid's own keys leave alone. */
export const typingIn = (target: EventTarget | null): boolean =>
  keyTargetOf(target, target instanceof Element ? target.parentElement : null)
    .editable

/** Where an instance lives: its element, read when a key comes (it may mount later). */
type RootOf = () => Element | null

/** What an instance says of a press: taken (`true`), not its (`false`), or its with
 *  nothing to do now (`"idle"`, heard only from a part in hand: rule 3). */
export type KeyAnswer = boolean | "idle"

/** How the press reached an instance: whether its part is in hand (holds the focus,
 *  is under the pointer or was used last), as against reached only as the newest. */
export interface KeyReach {
  inHand: boolean
  /** Holds the focus itself (the nearest of in hand). */
  focused: boolean
}

interface Instance {
  root: RootOf
  /** Answer the press: true when it was this instance's (the router then takes it from
   *  the page: `preventDefault`). */
  onKey: (event: KeyboardEvent, reach: KeyReach) => KeyAnswer
  modal: boolean
  /** The roles of its own focusable parts (a gallery's options), which do not count
   *  as a control owning the press. */
  own: readonly string[]
  /** Mount order: newer instances come first among equals. */
  born: number
}

interface Layer {
  root: RootOf
  onEscape: () => void
  /** When it opened: the latest open is the innermost. */
  opened: number
}

const instances = new Set<Instance>()
const layers = new Set<Layer>()
let born = 0
let installed = false
/** The element under the pointer, and the one last pressed or focused into. */
let under: Element | null = null
let touched: Element | null = null

const holds = (root: Element | null, el: Element | null) =>
  !!root && !!el && root.contains(el)

/** Inside an `inert` subtree: what the platform says takes no interaction (a picture
 *  of a control, a page under a modal of its own), so no press is offered to it and
 *  none of its layers closes on Esc. */
const asleep = (root: Element | null) => !!root?.closest("[inert]")

/** Inside a region whose instances take a press only while held (rule 5). */
const heldOnly = (root: Element | null) => !!root?.closest('[data-keys="held"]')

/** How near the hand a root is: 0 holding the focus, 1 under the pointer, 2 used
 *  last, 3 none of them (reached only as the newest). */
function rankOf(r: Element | null): number {
  if (holds(r, document.activeElement)) return 0
  if (holds(r, under)) return 1
  if (holds(r, touched)) return 2
  return 3
}

/** Roots in the order a press is offered to them: nearest the hand first, an inner
 *  root before the one holding it, then the newest. */
function byReach<T extends { root: RootOf; born: number }>(list: T[]): T[] {
  return list.sort((a, b) => {
    const d = rankOf(a.root()) - rankOf(b.root())
    if (d !== 0) return d
    // Nested roots holding the same focus: the inner one first.
    const ra = a.root()
    const rb = b.root()
    if (ra && rb && ra !== rb) {
      if (ra.contains(rb)) return 1
      if (rb.contains(ra)) return -1
    }
    return b.born - a.born
  })
}

/** The instances a press is offered to, in order (see the header). */
function ranked(): Instance[] {
  const focus = document.activeElement
  const awake = [...instances].filter((i) => {
    const r = i.root()
    if (asleep(r)) return false
    return !heldOnly(r) || holds(r, focus) || holds(r, touched)
  })
  const modal = awake.filter((i) => i.modal)
  const pool = modal.length
    ? awake.filter((i) =>
        modal.some((m) => m === i || holds(m.root(), i.root())),
      )
    : awake
  return byReach(pool)
}

/** Offers a press to `list` in order; true once one took it or kept it idle. */
function offer(event: KeyboardEvent, list: Instance[]): boolean {
  for (const i of list) {
    if (ownedHere(event, i.root(), i.own)) continue
    const rank = rankOf(i.root())
    const inHand = rank < 3
    const answer = i.onKey(event, { inHand, focused: rank === 0 })
    if (answer === true) {
      event.preventDefault()
      return true
    }
    if (answer === "idle" && inHand) return true
  }
  return false
}

/** The presses just before this one, oldest first, for a binding that is a sequence
 *  ("g g"): kept while they follow each other within `SEQUENCE_MS`. */
let recent: { press: Press; at: number }[] = []
const SEQUENCE_MS = 900
const before = (): readonly Press[] => recent.map((r) => r.press)

function remember(event: KeyboardEvent) {
  if (["Meta", "Control", "Alt", "Shift"].includes(event.key)) return
  const at = event.timeStamp
  const last = recent.at(-1)
  if (last && at - last.at > SEQUENCE_MS) recent = []
  recent = [
    ...recent.slice(-3),
    {
      press: {
        key: event.key,
        code: event.code,
        metaKey: event.metaKey,
        ctrlKey: event.ctrlKey,
        altKey: event.altKey,
        shiftKey: event.shiftKey,
      },
      at,
    },
  ]
}

function onKeyDown(event: KeyboardEvent) {
  if (event.isComposing) return
  // What came before this press is read before it joins them.
  const prior = before()
  pressing = prior
  try {
    dispatchKey(event)
  } finally {
    pressing = []
    remember(event)
  }
}
/** The presses before the one being offered, for the instances answering it. */
let pressing: readonly Press[] = []
/** The presses before the press being answered now (a sequence's earlier keys). */
export const previousPresses = (): readonly Press[] => pressing

function dispatchKey(event: KeyboardEvent) {
  const order = ranked()
  const open = [...layers].filter((l) => !asleep(l.root()))
  // Esc: a modal first (it is above every layer), then the innermost layer, and only
  // then an instance that answers it itself.
  if (event.key === "Escape" && open.length > 0) {
    if (event.defaultPrevented) return
    if (
      offer(
        event,
        order.filter((i) => i.modal),
      )
    )
      return
    const top = open.sort((a, b) => b.opened - a.opened)[0] as Layer
    if (ownedHere(event, top.root())) return
    event.preventDefault()
    top.onEscape()
    return
  }
  offer(event, order)
}
const onPointerOver = (event: PointerEvent) => {
  under = event.target instanceof Element ? event.target : null
}
const onTouch = (event: Event) => {
  if (event.target instanceof Element) touched = event.target
}
const onPointerLeavePage = () => {
  under = null
}

function install() {
  if (installed || typeof document === "undefined") return
  installed = true
  document.addEventListener("keydown", onKeyDown)
  document.addEventListener("pointerover", onPointerOver, { passive: true })
  document.addEventListener("pointerdown", onTouch, { passive: true })
  document.addEventListener("focusin", onTouch)
  document.documentElement.addEventListener("pointerleave", onPointerLeavePage)
}
function uninstall() {
  if (!installed || instances.size > 0 || layers.size > 0) return
  installed = false
  document.removeEventListener("keydown", onKeyDown)
  document.removeEventListener("pointerover", onPointerOver)
  document.removeEventListener("pointerdown", onTouch)
  document.removeEventListener("focusin", onTouch)
  document.documentElement.removeEventListener(
    "pointerleave",
    onPointerLeavePage,
  )
  under = null
  touched = null
}

/** Puts an instance on the router from an effect (a binder that sets up its own
 *  listeners); returns the way off. `useKeys` is the hook form. */
export function registerKeys(instance: {
  root: RootOf
  onKey: (event: KeyboardEvent, reach: KeyReach) => KeyAnswer
  modal?: boolean
  own?: readonly string[]
}): () => void {
  const i: Instance = { modal: false, own: [], ...instance, born: ++born }
  instances.add(i)
  install()
  return () => {
    instances.delete(i)
    uninstall()
  }
}

/** Opens a layer Escape closes (the innermost open one first); returns the way off,
 *  which the layer calls when it closes by any other means. `useEscape` is the hook. */
export function openLayer(layer: {
  root: RootOf
  onEscape: () => void
}): () => void {
  const l: Layer = { ...layer, opened: ++born }
  layers.add(l)
  install()
  return () => {
    layers.delete(l)
    uninstall()
  }
}

const rootOf = (
  root: React.RefObject<Element | null> | RootOf | undefined,
): RootOf =>
  typeof root === "function" ? root : () => (root ? root.current : null)

/** An item's keys, on the router: `onKey` answers a press and returns true when it was
 *  the item's (the press is then taken from the page), or "idle" for its key with
 *  nothing to do now. `root` is the item's element: focus, the pointer or a last press
 *  inside it puts it first; without one it answers only what nothing nearer takes.
 *  Read at press time, so it always sees the latest props. For a part's named actions,
 *  `useActions`; this stays for a widget's own keys. */
export function useKeys(
  onKey: (event: KeyboardEvent, reach: KeyReach) => KeyAnswer,
  {
    root,
    enabled = true,
    modal = false,
    own,
  }: {
    root?: React.RefObject<Element | null> | RootOf
    /** Off, it is not on the router at all (a hidden or inert instance). */
    enabled?: boolean
    /** Takes every press while mounted; nothing outside its root hears one. */
    modal?: boolean
    /** The roles of its own focusable parts (a gallery's "option"), which it answers
     *  for itself. */
    own?: readonly string[]
  } = {},
): void {
  const answer = React.useEffectEvent(onKey)
  const where = React.useRef(root)
  where.current = root
  const ownKey = own?.join(" ") ?? ""
  React.useEffect(() => {
    if (!enabled) return
    return registerKeys({
      root: () => rootOf(where.current)(),
      onKey: (event, reach) => answer(event, reach),
      modal,
      own: ownKey ? ownKey.split(" ") : [],
    })
  }, [enabled, modal, ownKey])
}

/** A layer Escape closes while `active`: the innermost open layer, and only when
 *  nothing inside it took the Esc first. */
export function useEscape(
  onEscape: () => void,
  {
    active,
    root,
  }: { active: boolean; root?: React.RefObject<Element | null> | RootOf },
): void {
  const close = React.useEffectEvent(onEscape)
  const where = React.useRef(root)
  where.current = root
  React.useEffect(() => {
    if (!active) return
    return openLayer({
      root: () => rootOf(where.current)(),
      onEscape: () => close(),
    })
  }, [active])
}

// ── ACTIONS: a part's named keys (lib/actions), declared once and answered from the
// declaration, so what a menu, a tooltip, `aria-keyshortcuts` and the page's sheet
// say is what the keys do. A part on the router through `useActions` is an OWNER: the
// sheet lists the owners nearest the hand first, which-key reads the keys they draw.

/** The platform keys are read on (`mod` is ⌘ on a Mac, Ctrl elsewhere). */
export const platformNow = (): Platform =>
  typeof navigator === "undefined" ? "mac" : platformOf(navigator.platform)

interface Owner {
  title: string
  root: RootOf
  /** Its set as it answers now (remapped where it allows it). */
  set: () => ActionSet<Action>
  /** Its set as declared, what a remapping is checked against. */
  declared: () => ActionSet<Action>
  active: () => ReadonlySet<string>
  remappable: boolean
  born: number
}

/** An owner as the page's sheet reads it. */
export interface OwnerView {
  title: string
  set: ActionSet<Action>
  active: ReadonlySet<string>
  remappable: boolean
  /** Holds the focus, is under the pointer or was used last. */
  inHand: boolean
}

const owners = new Set<Owner>()
const watchers = new Set<() => void>()
let version = 0
const changed = () => {
  version += 1
  for (const w of watchers) w()
}
const watch = (w: () => void) => {
  watchers.add(w)
  return () => {
    watchers.delete(w)
  }
}

/** The owners on the page now, nearest the hand first, none asleep (inert). */
export function ownersNow(): OwnerView[] {
  return byReach([...owners].filter((o) => !asleep(o.root()))).map((o) => ({
    title: o.title,
    set: o.set(),
    active: o.active(),
    remappable: o.remappable,
    inHand: rankOf(o.root()) < 3,
  }))
}

/** Re-renders when owners come, go or are remapped: what the sheet and which-key
 *  read. The list itself is `ownersNow()`, read when drawn (the hand moves without
 *  telling anyone). */
export function useOwnersVersion(): number {
  return React.useSyncExternalStore(
    watch,
    () => version,
    () => 0,
  )
}

// ── REMAPPING: a person's own keys for a part that allows it (`remappable`), kept
// per site in localStorage by the owner's title and the action's id, and read by
// everything that reads keys, so a remapped key is the one menus and tips teach.

const REMAPS = "ag.keys.remapped"
type Remaps = Record<string, Record<string, string[]>>
let remaps: Remaps | null = null
const loaded = (): Remaps => {
  if (remaps) return remaps
  if (typeof localStorage === "undefined") return {}
  const text = localStorage.getItem(REMAPS)
  remaps = text ? (JSON.parse(text) as Remaps) : {}
  return remaps
}

/** An owner's remapped keys, by action id. */
export const remapsOf = (title: string): ReadonlyMap<string, string[]> =>
  new Map(Object.entries(loaded()[title] ?? {}))

/** Gives an action new keys (or its own back, with null). A key the owner already
 *  binds elsewhere in that mode throws, naming the action that holds it, and nothing
 *  is kept. */
export function remap(
  title: string,
  id: string,
  keys: readonly string[] | null,
): void {
  const owner = [...owners].find((o) => o.title === title)
  if (!owner) throw new Error(`remap: no part "${title}" on the page`)
  if (!owner.remappable)
    throw new Error(`remap: "${title}" does not let its keys be changed`)
  const mine = { ...(loaded()[title] ?? {}) }
  if (keys) mine[id] = [...keys]
  else delete mine[id]
  // Checked against the declaration before it is kept: a conflict names its holder.
  remapped(owner.declared(), new Map(Object.entries(mine)))
  remaps = { ...loaded(), [title]: mine }
  localStorage.setItem(REMAPS, JSON.stringify(remaps))
  changed()
}

/** A part's named actions on the router (lib/actions): the set declared and checked
 *  at once, its keys answered from it, the part listed on the page's sheet under
 *  `set.title`. A key whose action is unavailable now (`when`) is kept, idle, while
 *  the part is in hand. A chord the browser answers (⌘F, ⌘G) is answered only while
 *  the part is in hand, unless its binding `takesBrowserChord`. `active` names the
 *  set's modes on now. `remappable`: a person may change its keys (the sheet offers
 *  it). Returns the set as it answers now, for the part's own menus and tips. */
export function useActions(
  set: ActionSet<Action>,
  {
    root,
    enabled = true,
    modal = false,
    own,
    active,
    remappable = false,
  }: {
    root?: React.RefObject<Element | null> | RootOf
    enabled?: boolean
    modal?: boolean
    own?: readonly string[]
    active?: () => ReadonlySet<string>
    remappable?: boolean
  } = {},
): ActionSet<Action> {
  declare(set)
  useOwnersVersion()
  const latest = React.useRef(set)
  latest.current = set
  const modes = React.useRef(active)
  modes.current = active
  const where = React.useRef(root)
  where.current = root
  const now = () =>
    remappable
      ? remapped(latest.current, remapsOf(latest.current.title))
      : latest.current
  const current = React.useRef(now)
  current.current = now
  const answer = React.useEffectEvent(
    (event: KeyboardEvent, reach: KeyReach): KeyAnswer => {
      const m = match(now(), event, {
        platform: platformNow(),
        active: modes.current?.() ?? new Set(),
        before: previousPresses(),
      })
      if (!m) return false
      if (m.action.needsFocus && !reach.focused) return false
      if (browserChord(m.key) && !reach.inHand && !m.action.takesBrowserChord)
        return false
      if (!m.available) return m.action.passWhenIdle ? false : "idle"
      m.action.run()
      return true
    },
  )
  const ownKey = own?.join(" ") ?? ""
  const title = set.title
  React.useEffect(() => {
    if (!enabled) return
    const rootNow = () => rootOf(where.current)()
    const off = registerKeys({
      root: rootNow,
      onKey: (event, reach) => answer(event, reach),
      modal,
      own: ownKey ? ownKey.split(" ") : [],
    })
    const leave = enter({
      title,
      root: rootNow,
      set: () => current.current(),
      declared: () => latest.current,
      active: () => modes.current?.() ?? new Set(),
      remappable,
      born: ++born,
    })
    return () => {
      off()
      leave()
    }
  }, [enabled, modal, ownKey, title, remappable])
  return now()
}

const enter = (owner: Owner) => {
  owners.add(owner)
  changed()
  return () => {
    owners.delete(owner)
    changed()
  }
}

/** A part that answers its own keys on its own element, ahead of the router (a
 *  timeline inside a player, whose J K L must never reach the player's), listed on
 *  the page's sheet as an owner all the same: its set is declared, and read, from the
 *  one place its handler resolves through. */
export function useKeyOwner(
  set: ActionSet<ActionSpec>,
  {
    root,
    enabled = true,
    active,
  }: {
    root: React.RefObject<Element | null> | RootOf
    enabled?: boolean
    active?: () => ReadonlySet<string>
  },
): void {
  const latest = React.useRef(set)
  latest.current = set
  const modes = React.useRef(active)
  modes.current = active
  const where = React.useRef(root)
  where.current = root
  const title = set.title
  React.useEffect(() => {
    if (!enabled) return
    // Listed, never run from the sheet: its handler is its own.
    const listed = () => ({
      ...latest.current,
      actions: latest.current.actions.map((a) => ({ ...a, run: () => {} })),
    })
    return enter({
      title,
      root: () => rootOf(where.current)(),
      set: listed,
      declared: listed,
      active: () => modes.current?.() ?? new Set(),
      remappable: false,
      born: ++born,
    })
  }, [enabled, title])
}

/** The actions a part's own tips and menus name by id (`<Tip action="add-row">`):
 *  the set `useActions` returned, given to everything inside the part. */
const Scope = React.createContext<ActionSet<Action> | null>(null)

export function ActionScope({
  actions,
  children,
}: {
  actions: ActionSet<Action>
  children: React.ReactNode
}) {
  return <Scope.Provider value={actions}>{children}</Scope.Provider>
}

/** An action of the enclosing `ActionScope`, by id. Named outside one, or an id the
 *  scope does not have, throws: a tooltip teaching a key that answers nothing. */
export function useAction(id: string): Action {
  const set = React.useContext(Scope)
  if (!set)
    throw new Error(`useAction: "${id}" is named outside an ActionScope`)
  const found = set.actions.find((a) => a.id === id)
  if (!found) throw new Error(`useAction: ${set.title} has no action "${id}"`)
  return found
}
