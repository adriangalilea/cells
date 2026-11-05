"use client"

// CARRY, bound to the page: an item taken up under a press, a finger or the keys,
// and put down elsewhere, in one lane (a list, a strip) or across lanes (a board).
// lib/carry-core holds the numbers and the math, Ink's; this is the hand and the DOM.
//
// The pointer: a mouse takes a card up once it moves 3 px (a press that never moves
// opens it); a finger once held 280 ms, so a stroke that starts at once still scrolls.
// The card is then drawn over everything where the press is, grown 3.5% from under it
// with a little overshoot, two shadows, leaning into its sideways speed (3° at 1400
// px/s, righting once still 60 ms); its slot holds open and the others glide apart
// where it would land, on one critically damped spring that carries its velocity
// through a retarget; the gap crosses a neighbour only 20% past that neighbour's
// middle. Near a lane's ends (and a board's sides) the edge drive scrolls under it.
// Let go, it lands in the gap on a spring; over a place `refuses` says no to, no gap
// opens anywhere and it flies home.
//
// The keys: space takes the focused card up, the arrows move it (along its lane, or
// to the next lane), space or return puts it down, esc puts it back; a refused step
// leans toward where it was asked to go and springs back. Without a card up, the
// arrows walk the focus and return opens. Every step is said to a screen reader.
//
// CONTROLLED. The board is the consumer's: a move is handed over (`onMove`) and the
// consumer's lanes say where things are. The card lands where the next render puts
// it; a move the consumer does not apply flies the card home.
//
// What a carry changes is split by how often it changes, as Ink's DragBoard splits it:
// where the card is moves every frame and is written to the elements by one loop,
// never rendered; where it would land (`target`) changes only when it crosses a
// neighbour and is React state; the geometry is read once at the lift, and nothing is
// read back mid-carry but the lanes' own boxes, which move with whatever scrolls them.

import { CircleAlert } from "lucide-react"
import * as React from "react"
import { createPortal } from "react-dom"
import { cn } from "@/lib/utils"
import {
  assert,
  CARRY,
  type CarryState,
  EdgeDriver,
  extraOf,
  footShiftOf,
  gapOf,
  type LaneLayout,
  placeAt,
  type Slot,
  Spring,
  sameSlot,
  shiftsOf,
  slotStart,
  stepFrom,
  tiltFor,
} from "@/lib/carry-core"
import { openLayer } from "@/components/ui/key-router"

export type { Slot } from "@/lib/carry-core"

/** Which way a lane's items stand: down a column (`y`, lanes side by side) or along
 *  a strip (`x`, a row of columns). */
export type CarryAxis = "y" | "x"

/** What a lane says while a card is carried: nothing, that the card would land in it,
 *  or that it refuses the card. */
export type CarryLight = "none" | "receiving" | "refusing"

export type CarryOptions<ID> = {
  /** The items' order now, lane by lane (one lane for a list). */
  lanes: readonly (readonly ID[])[]
  axis?: CarryAxis
  /** A move the board does not allow (a full lane, a rule): no gap opens there, the
   *  lane says no, and a card let go there flies home. A string is the refusal said
   *  ("sorted by cost: clear the sort to reorder"). */
  refuses?: (id: ID, from: Slot, to: Slot) => boolean | string
  /** Where a card from another lane lands in `lane` when the lane decides it rather
   *  than the hand (a sorted lane: its sorted place), as an index counted without the
   *  card; `undefined` leaves it to the hand. Its gap opens there while the card is
   *  over the lane, and the keys' steps along that lane stay put. */
  placeIn?: (id: ID, lane: number) => number | undefined
  /** Items the lanes put in a new order on their own (a sort, the data changing)
   *  glide from where they stood to their new places, as the gap's glide. */
  glides?: boolean
  /** A move made: the consumer applies it to its lanes. With a `group`, `id` is the
   *  pressed item and `snapshot().group` all that move, landing together at `to`.
   *  Words returned are said after the landing ("now in manual order"). */
  // biome-ignore lint/suspicious/noConfusingVoidType: a handler with nothing to say returns nothing, as every existing one does
  onMove: (id: ID, from: Slot, to: Slot) => string | void
  /** A carry not allowed at all now, and why ("sorted by cost: clear the sort to
   *  reorder"): a press that travels takes nothing up, its item leans the way it was
   *  pulled and springs back, felt and said; a press that does not travel still opens. */
  declines?: (id: ID) => string | false
  /** What a press on `id` carries: it and what moves with it (a selection of rows), in
   *  one lane; `[id]` unless said. One card floats (the pressed item's), the gap is
   *  all of theirs, and they land together in their order. */
  group?: (id: ID) => readonly ID[]
  /** An item's extent along the lanes, when the lane draws its items at extents it
   *  knows (windowed rows of one height): nothing is measured, and items that are not
   *  mounted take part all the same. */
  extent?: (id: ID) => number
  /** Every frame an item's shift changes, with the px it is moved by: for what moves
   *  with the item but is not it (a column's cells with its head). */
  onShift?: (id: ID, px: number) => void
  /** What an item is called, said to a screen reader as it moves. */
  name: (id: ID) => string
  /** What a lane is called ("done"), said as a card enters it. */
  laneName?: (lane: number) => string
  /** A press that never moved, or return: the item opened. */
  onOpen?: (id: ID) => void
  /** Delete or backspace on the focused item. */
  onDelete?: (id: ID) => void
}

/** What React draws from: changes only when a card is taken up, crosses a neighbour,
 *  is refused or lands. */
export type CarrySnapshot<ID> = {
  carried: ID | null
  /** What is carried with it, itself included, in the lane's order. */
  group: readonly ID[]
  mode: "pointer" | "keys"
  source: Slot
  target: Slot
  refusing: number | null
  /** Why, while the carried card is over a place that refuses it with words: what
   *  the floating card says beside it. */
  reason: string | null
  settling: boolean
  /** Going home: every item back in its own place. */
  home: boolean
  /** Counts each refusal, and the lane that refused last: a count leans each time. */
  refusals: number
  refusedLane: number | null
  /** What is said to a screen reader now. */
  said: string
}

const IDLE: CarrySnapshot<never> = {
  carried: null,
  group: [],
  mode: "pointer",
  source: { lane: 0, index: 0 },
  target: { lane: 0, index: 0 },
  refusing: null,
  reason: null,
  settling: false,
  home: false,
  refusals: 0,
  refusedLane: null,
  said: "",
}

/** A lifted card's light, by the ground it stands on: a tight shadow where it nearly
 *  touches and a soft one well below it, more of both on a dark ground, which swallows
 *  shadow (Ink's DragLiftShadow). */
export const LIFTED =
  "shadow-[0_1px_3px_rgb(0_0_0/0.12),0_16px_44px_rgb(0_0_0/0.2)] dark:shadow-[0_1px_3px_rgb(0_0_0/0.35),0_16px_44px_rgb(0_0_0/0.5)]"

/** A carried card's look while the keys hold it up, in its gap: the same light. */
export const KEYED_LIFT =
  "data-[lifted]:shadow-[0_1px_3px_rgb(0_0_0/0.12),0_16px_44px_rgb(0_0_0/0.2)] dark:data-[lifted]:shadow-[0_1px_3px_rgb(0_0_0/0.35),0_16px_44px_rgb(0_0_0/0.5)]"

/** What an item wears so a finger can hold it: no callout, no text selection, no
 *  double-tap zoom (a scroll still passes). */
export const HOLDABLE =
  "touch-manipulation select-none [-webkit-touch-callout:none]"

/** Controls inside an item that take their own press (a card's own buttons). */
const OWN_PRESS =
  "button, a[href], input, textarea, select, [contenteditable], [data-carry-ignore]"

/** The press's feel on a touch screen, where a phone can (navigator.vibrate): a lift,
 *  a place crossed, a landing, and a refusal as two short pulses 70 ms apart (Ink's
 *  refusal on a Mac's trackpad). */
const FEEL = { lift: 8, tick: 4, confirm: 12, refuse: [8, 70, 8] } as const

type Press<ID> = {
  id: ID
  pointerId: number
  touch: boolean
  x: number
  y: number
  hold: number
}

/** The nearest element that scrolls `el` along `axis`, or the page. */
function scrollerOf(el: Element, axis: CarryAxis): HTMLElement {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const s = getComputedStyle(p)
    const o = axis === "y" ? s.overflowY : s.overflowX
    if (o === "auto" || o === "scroll") return p
  }
  return (document.scrollingElement ?? document.documentElement) as HTMLElement
}

const isPage = (el: HTMLElement) =>
  el === document.scrollingElement || el === document.documentElement

/** A box along the lanes' axis (`main`) and across it (`cross`). */
function along(r: DOMRect, axis: CarryAxis) {
  return axis === "y"
    ? { start: r.top, size: r.height, cross: r.left, crossSize: r.width }
    : { start: r.left, size: r.width, cross: r.top, crossSize: r.height }
}

/** The engine of one board. Imperative: it holds the elements, the springs and the
 *  frame loop; React reads its snapshot. */
export class Carrier<ID> {
  opts: CarryOptions<ID>
  private snap: CarrySnapshot<ID> = IDLE as CarrySnapshot<ID>
  private listeners = new Set<() => void>()

  private items = new Map<ID, HTMLElement>()
  private laneEls: (HTMLElement | null)[] = []
  private laneScrollers: (HTMLElement | null)[] = []
  private feet: (HTMLElement | null)[] = []
  private rooms: (HTMLElement | null)[] = []
  private board: HTMLElement | null = null
  private floatEl: HTMLElement | null = null

  private layout: LaneLayout<ID>[] = []
  private shifts = new Map<ID, Spring>()
  private footShift: Spring[] = []
  private roomSize: Spring[] = []
  private float = {
    x: new Spring(0, CARRY.settle, 0.1),
    y: new Spring(0, CARRY.settle, 0.1),
    scale: new Spring(1, CARRY.lift, 0.0005),
    tilt: new Spring(0, CARRY.follow, 0.01),
    width: 0,
    anchor: { x: 0, y: 0 },
  }
  /** The card carried by keys: its offset to its slot, and its lift. */
  private keyed = {
    main: new Spring(0, CARRY.glide, 0.1),
    cross: new Spring(0, CARRY.glide, 0.1),
    scale: new Spring(1, CARRY.lift, 0.0005),
  }
  private frame = 0
  private press: Press<ID> | null = null
  private grab = { x: 0, y: 0 }
  private pointer = { x: 0, y: 0 }
  private speed = 0
  private moved = 0
  private righting = 0
  private hovered = 0
  private asked: { slot: Slot; refused: boolean | string } | null = null
  /** The refusal the gap is held by, said on the drop. */
  private reason: string | null = null
  /** The way the refused place lies from home, (dx, dy) in lane and index steps:
   *  a refused drop lands home and leans that way (Ink's `inkRefusal(.toward)`). */
  private refusedToward: [number, number] | null = null
  private leanOnLanding: [number, number] | null = null
  private touch = false
  /** A move handed to the consumer, waiting for its lanes to say so: `first` is the
   *  group's first item, which lands at `to`. */
  private committing: {
    id: ID
    first: ID
    to: Slot
    waited: number
  } | null = null
  private main = new EdgeDriver((px) => this.driveMain(px))
  private side = new EdgeDriver((px) => this.driveSide(px))
  private undoPage: (() => void) | null = null

  constructor(opts: CarryOptions<ID>) {
    this.opts = opts
  }

  get axis(): CarryAxis {
    return this.opts.axis ?? "y"
  }

  // ── React's side ──────────────────────────────────────────────────────────────

  subscribe = (f: () => void) => {
    this.listeners.add(f)
    return () => this.listeners.delete(f)
  }

  snapshot = () => this.snap

  private set(next: Partial<CarrySnapshot<ID>>): void {
    this.snap = { ...this.snap, ...next }
    for (const f of this.listeners) f()
  }

  item(id: ID, el: HTMLElement | null): void {
    if (!el) {
      if (this.items.get(id)) this.items.delete(id)
      return
    }
    this.items.set(id, el)
    // Mounted mid-carry (a windowed row the edge drive brought in): it stands where
    // the carry puts it at once, and hides if it is carried.
    const s = this.snap
    if (s.carried === null) return
    if (s.group.includes(id) && s.mode === "pointer") el.style.opacity = "0"
    this.paintItem(id)
  }

  lane(i: number, el: HTMLElement | null): void {
    this.laneEls[i] = el
  }

  /** A lane's own scroller; without one, the nearest that scrolls it (or the page). */
  laneScroller(i: number, el: HTMLElement | null): void {
    this.laneScrollers[i] = el
  }

  foot(i: number, el: HTMLElement | null): void {
    this.feet[i] = el
  }

  room(i: number, el: HTMLElement | null): void {
    this.rooms[i] = el
  }

  /** What scrolls a board of lanes across them (a kanban's sideways scroll). */
  across(el: HTMLElement | null): void {
    this.board = el
  }

  floatAt(el: HTMLElement | null): void {
    this.floatEl = el
    if (el) this.paint()
  }

  /** The lanes the consumer now draws: a move handed over has landed in them. */
  lanesChanged(): void {
    const c = this.committing
    if (!c) {
      this.reflow()
      return
    }
    // Only lanes that hold the move: a render of the board's own (its snapshot) is
    // not the consumer's answer.
    const at = this.locate(c.first)
    if (!at || !sameSlot(at, c.to)) return
    this.committing = null
    // The DOM is in the new order: every shift is now nothing, at once (a glide
    // would move items that the new layout already put in place).
    for (const s of this.shifts.values()) s.jump(0)
    for (const s of this.footShift) s.jump(0)
    for (const s of this.roomSize) s.jump(0)
    if (this.snap.mode === "keys") {
      this.keyed.main.jump(0)
      this.keyed.cross.jump(0)
      this.paint()
      this.finish(c.id)
      return
    }
    this.paint()
    this.landOn(c.id)
  }

  /** Where each item stood along its lane when the lanes last changed, as laid out
   *  (its shift taken off): what a new order glides from (`glides`). */
  private placed = new Map<ID, { lane: number; at: number }>()

  /** Every mounted item's place along its lane, as laid out. */
  private positions(): Map<ID, { lane: number; at: number }> {
    const out = new Map<ID, { lane: number; at: number }>()
    for (const [lane, ids] of this.opts.lanes.entries()) {
      const box = this.laneEls[lane]
      if (!box) continue
      const start = along(box.getBoundingClientRect(), this.axis).start
      for (const id of ids) {
        const el = this.items.get(id)
        if (!el) continue
        const at = along(el.getBoundingClientRect(), this.axis).start - start
        out.set(id, { lane, at: at - (this.shifts.get(id)?.value ?? 0) })
      }
    }
    return out
  }

  /** The lanes put their items in a new order with nothing carried (a sort, a vote
   *  under one): each item that moved along its lane stays where it stood on screen
   *  and glides to its new place, as the gap's glide, before the frame is painted. */
  private reflow(): void {
    if (!this.opts.glides || this.snap.carried !== null) return
    const was = this.placed
    this.placed = this.positions()
    let moved = false
    for (const [id, now] of this.placed) {
      const before = was.get(id)
      if (!before || before.lane !== now.lane) continue
      if (Math.abs(before.at - now.at) < 0.5) continue
      let spring = this.shifts.get(id)
      if (!spring) {
        spring = new Spring(0, CARRY.glide, 0.1)
        this.shifts.set(id, spring)
      }
      // On screen it stood at its old place plus the shift it wore; it wears the
      // same place now, from its new one, and glides home from there.
      spring.value += before.at - now.at
      spring.to(0, CARRY.glide)
      moved = true
    }
    if (!moved) return
    this.paint()
    this.kick()
  }

  // ── The pointer ───────────────────────────────────────────────────────────────

  pointerDown = (id: ID, e: React.PointerEvent<HTMLElement>): void => {
    if (e.button !== 0 || this.snap.carried !== null || this.press) return
    const own = (e.target as Element).closest(OWN_PRESS)
    if (own && own !== e.currentTarget) return
    const touch = e.pointerType === "touch"
    this.touch = touch
    this.press = {
      id,
      pointerId: e.pointerId,
      touch,
      x: e.clientX,
      y: e.clientY,
      hold: touch ? window.setTimeout(() => this.held(), CARRY.holdMs) : 0,
    }
    window.addEventListener("pointermove", this.pointerMove)
    window.addEventListener("pointerup", this.pointerUp)
    window.addEventListener("pointercancel", this.pointerCancel)
  }

  /** A finger held still long enough: the card comes up under it. */
  private held(): void {
    const p = this.press
    if (!p || this.snap.carried !== null) return
    p.hold = 0
    if (this.declined(p.id, 0, 0)) return
    this.lift(p.id, p.x, p.y)
  }

  /** A carry `declines` refuses: the press ends taking nothing up, the item leans the
   *  way it was pulled (along the lane, by `dx`, `dy`) and springs back, felt and
   *  said. */
  private declined(id: ID, dx: number, dy: number): boolean {
    const why = this.opts.declines?.(id)
    if (!why) return false
    this.release()
    const along = this.axis === "y" ? dy : dx
    const sign = along < 0 ? -1 : 1
    refuse(
      this.items.get(id),
      this.axis === "x" ? sign : 0,
      this.axis === "y" ? sign : 0,
      CARRY.leanStep,
    )
    this.feel("refuse")
    this.set({ refusals: this.snap.refusals + 1, said: why })
    return true
  }

  private pointerMove = (e: PointerEvent): void => {
    const p = this.press
    if (!p || e.pointerId !== p.pointerId) return
    if (this.snap.carried === null) {
      const d = Math.hypot(e.clientX - p.x, e.clientY - p.y)
      if (p.touch) {
        // Moving before the hold is a scroll: the press is the page's.
        if (d > CARRY.holdSlop) this.release()
        return
      }
      if (d < CARRY.travel) return
      if (this.declined(p.id, e.clientX - p.x, e.clientY - p.y)) return
      this.lift(p.id, p.x, p.y)
    }
    if (this.snap.carried === p.id && this.snap.mode === "pointer")
      this.move(e.clientX, e.clientY)
  }

  private pointerUp = (e: PointerEvent): void => {
    const p = this.press
    if (!p || e.pointerId !== p.pointerId) return
    const lifted = this.snap.carried === p.id && this.snap.mode === "pointer"
    this.release()
    if (lifted) this.drop()
    // Never taken up: a press that opens.
    else this.opts.onOpen?.(p.id)
  }

  private pointerCancel = (e: PointerEvent): void => {
    const p = this.press
    if (!p || e.pointerId !== p.pointerId) return
    const lifted = this.snap.carried === p.id && this.snap.mode === "pointer"
    this.release()
    if (lifted) this.cancel()
  }

  private release(): void {
    if (this.press?.hold) window.clearTimeout(this.press.hold)
    this.press = null
    window.removeEventListener("pointermove", this.pointerMove)
    window.removeEventListener("pointerup", this.pointerUp)
    window.removeEventListener("pointercancel", this.pointerCancel)
  }

  /** While a press carries: no text is selected, the hand is closed, a finger's
   *  stroke never scrolls the page, and a scroll under the card moves its gap. */
  private holdPage(): void {
    const root = document.documentElement
    const was = { select: root.style.userSelect, cursor: root.style.cursor }
    root.style.userSelect = "none"
    root.style.cursor = "grabbing"
    const still = (e: TouchEvent) => e.preventDefault()
    const scrolled = () => this.retarget()
    // esc mid-drag puts the card back, the rest of that press carrying nothing: the
    // carry is a rung of the page's Escape ladder (key-router), the innermost while
    // it lasts, so the Esc is its alone.
    const closeLayer = openLayer({
      root: () => this.laneEls[this.snap.source.lane] ?? null,
      onEscape: () => {
        this.release()
        this.cancel()
      },
    })
    document.addEventListener("touchmove", still, { passive: false })
    window.addEventListener("scroll", scrolled, {
      capture: true,
      passive: true,
    })
    this.undoPage = () => {
      root.style.userSelect = was.select
      root.style.cursor = was.cursor
      document.removeEventListener("touchmove", still)
      window.removeEventListener("scroll", scrolled, { capture: true })
      closeLayer()
    }
  }

  private freePage(): void {
    this.undoPage?.()
    this.undoPage = null
  }

  // ── The layout, read once ─────────────────────────────────────────────────────

  private measure(): void {
    const axis = this.axis
    this.layout = this.opts.lanes.map((ids, i) => {
      const content = this.laneEls[i]
      assert(content, `lane ${i} has no element (lane(${i}) unset)`)
      const box = along(content.getBoundingClientRect(), axis)
      const extent = new Map<ID, number>()
      const starts: number[] = []
      const given = this.opts.extent
      for (const id of ids) {
        // Extents the lane knows (windowed rows): an item need not be mounted.
        if (given) {
          extent.set(id, given(id))
          continue
        }
        const el = this.items.get(id)
        assert(el, `item ${String(id)} of lane ${i} is not mounted`)
        const r = along(el.getBoundingClientRect(), axis)
        extent.set(id, r.size)
        starts.push(r.start - box.start)
      }
      const cs = getComputedStyle(content)
      // Where the first item stands and the room between two, as drawn; the CSS
      // padding and gap only for a lane with too few items to read them from.
      const pad =
        starts[0] ??
        Number.parseFloat(axis === "y" ? cs.paddingTop : cs.paddingLeft)
      const spacing =
        starts.length > 1
          ? (starts[1] as number) -
            (starts[0] as number) -
            (extent.get(ids[0] as ID) ?? 0)
          : Number.parseFloat(axis === "y" ? cs.rowGap : cs.columnGap) || 0
      return { ids, extent, pad, spacing }
    })
    this.footShift = this.opts.lanes.map(
      (_, i) => this.footShift[i] ?? new Spring(0, CARRY.glide, 0.1),
    )
    this.roomSize = this.opts.lanes.map(
      (_, i) => this.roomSize[i] ?? new Spring(0, CARRY.glide, 0.1),
    )
  }

  private locate(id: ID): Slot | null {
    for (const [lane, ids] of this.opts.lanes.entries()) {
      const index = ids.indexOf(id)
      if (index >= 0) return { lane, index }
    }
    return null
  }

  private state(): CarryState<ID> {
    const s = this.snap
    assert(s.carried !== null, "nothing is carried")
    const lane = this.layout[s.source.lane]
    assert(lane, `no lane ${s.source.lane}`)
    return {
      carried: s.carried,
      group: new Set(s.group),
      source: s.source,
      target: s.target,
      gap: gapOf(lane, s.group),
      refusing: s.refusing,
      settling: s.settling,
      home: s.home,
    }
  }

  /** A lane's box along the axis, where it stands now. */
  private laneBox(lane: number) {
    const el = this.laneEls[lane]
    assert(el, `lane ${lane} has no element`)
    return along(el.getBoundingClientRect(), this.axis)
  }

  /** The lane across the lanes at `cross`: the one it is in, else the nearest. */
  private laneAt(cross: number): number {
    let best = 0
    let far = Number.POSITIVE_INFINITY
    for (let i = 0; i < this.laneEls.length; i++) {
      if (!this.laneEls[i]) continue
      const b = this.laneBox(i)
      if (cross >= b.cross && cross <= b.cross + b.crossSize) return i
      const d = Math.abs(b.cross + b.crossSize / 2 - cross)
      if (d < far) {
        far = d
        best = i
      }
    }
    return best
  }

  // ── A press carrying ──────────────────────────────────────────────────────────

  /** What a press on `id` carries, in its lane's order, and where that group stands
   *  among the items that stay. */
  private groupOf(id: ID): { group: ID[]; source: Slot } | null {
    const at = this.locate(id)
    if (!at) return null
    const ids = this.opts.lanes[at.lane] ?? []
    const asked = new Set(this.opts.group?.(id) ?? [id])
    assert(asked.has(id), "a carried group holds the pressed item")
    const group = ids.filter((x) => asked.has(x))
    assert(
      group.length === asked.size,
      "a carried group is all in the pressed item's lane",
    )
    // Its place: the first of the group, counted among the items that stay.
    const first = ids.indexOf(group[0] as ID)
    return { group, source: { lane: at.lane, index: first } }
  }

  private lift(id: ID, x: number, y: number): void {
    const found = this.groupOf(id)
    const el = this.items.get(id)
    if (!found || !el || this.snap.settling) return
    const { group, source: slot } = found
    this.measure()
    const r = el.getBoundingClientRect()
    this.grab = { x: x - r.left, y: y - r.top }
    this.pointer = { x, y }
    this.float.width = r.width
    this.float.anchor = { x: this.grab.x, y: this.grab.y }
    this.float.x.jump(r.left)
    this.float.y.jump(r.top)
    this.float.tilt.jump(0)
    this.float.scale.jump(1)
    this.float.scale.to(CARRY.scale, CARRY.lift)
    this.speed = 0
    this.moved = performance.now()
    this.asked = null
    this.hovered = slot.lane
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches
    this.main.reduced = reduced
    this.side.reduced = reduced
    for (const g of group) {
      const it = this.items.get(g)
      if (it) it.style.opacity = "0"
    }
    this.reason = null
    this.refusedToward = null
    this.leanOnLanding = null
    // A mouse moving 3 px may have begun a selection before the card came up.
    window.getSelection()?.removeAllRanges()
    this.holdPage()
    this.set({
      carried: id,
      group,
      mode: "pointer",
      source: slot,
      target: slot,
      refusing: null,
      reason: null,
      settling: false,
      home: false,
      said:
        group.length > 1
          ? `${group.length} picked up`
          : `${this.opts.name(id)}, picked up`,
    })
    this.feel("lift")
    this.kick()
  }

  /** The press moved: the card follows it exactly, the gap goes where the card's
   *  middle is, the edges drive. */
  private move(x: number, y: number): void {
    if (this.snap.settling) return
    this.lean(x)
    this.pointer = { x, y }
    this.float.x.jump(x - this.grab.x)
    this.float.y.jump(y - this.grab.y)
    this.retarget()
    const axis = this.axis
    const cross = axis === "y" ? x : y
    const lane = this.laneAt(cross)
    if (lane !== this.hovered) {
      this.main.end()
      this.hovered = lane
    }
    const scroller = this.scrollerOf(lane)
    const box = isPage(scroller)
      ? axis === "y"
        ? { lo: 0, hi: window.innerHeight }
        : { lo: 0, hi: window.innerWidth }
      : (() => {
          const b = along(scroller.getBoundingClientRect(), axis)
          return { lo: b.start, hi: b.start + b.size }
        })()
    this.main.follow(axis === "y" ? y : x, box)
    if (this.board) {
      const b = along(this.board.getBoundingClientRect(), axis)
      this.side.follow(cross, { lo: b.cross, hi: b.cross + b.crossSize })
    }
    this.kick()
  }

  private scrollerOf(lane: number): HTMLElement {
    const own = this.laneScrollers[lane]
    if (own) return own
    const el = this.laneEls[lane]
    assert(el, `lane ${lane} has no element`)
    return scrollerOf(el, this.axis)
  }

  /** The card leans into its sideways travel by the press's speed, smoothed over
   *  about three events so one jittery event never kicks it, and rights itself once
   *  the press stops. */
  private lean(x: number): void {
    const now = performance.now()
    const dt = (now - this.moved) / 1000
    this.moved = now
    if (dt > 0.001)
      this.speed += ((x - this.pointer.x) / dt - this.speed) * 0.35
    this.float.tilt.to(tiltFor(this.speed), CARRY.follow)
    window.clearTimeout(this.righting)
    this.righting = window.setTimeout(() => {
      if (this.snap.carried === null || this.snap.settling) return
      this.speed = 0
      this.float.tilt.to(0, CARRY.upright)
      this.kick()
    }, CARRY.stillMs)
  }

  /** Where the card's middle is the gap opens, unless that place refuses it: then
   *  the gap goes home and the lane says no. */
  private retarget(): void {
    const s = this.snap
    if (s.carried === null || s.mode !== "pointer" || s.settling) return
    const axis = this.axis
    const st = this.state()
    const lane = this.laneAt(axis === "y" ? this.pointer.x : this.pointer.y)
    const box = this.laneBox(lane)
    const origin = axis === "y" ? this.float.y.value : this.float.x.value
    // The floating card's middle: the pressed item's, which is the card drawn.
    const card = this.layout[s.source.lane]?.extent.get(s.carried) ?? st.gap
    const middle = origin + card / 2 - box.start
    const place = this.pinned(
      s.carried,
      s.source,
      placeAt(this.layout, st, lane, middle),
    )
    let refused: boolean | string
    if (this.asked && sameSlot(this.asked.slot, place)) {
      refused = this.asked.refused
    } else {
      refused =
        !sameSlot(place, s.source) &&
        (this.opts.refuses?.(s.carried, s.source, place) ?? false)
      this.asked = { slot: place, refused }
    }
    this.reason = typeof refused === "string" ? refused : null
    const next = refused ? s.source : place
    const refusing = refused ? lane : null
    if (
      sameSlot(next, s.target) &&
      refusing === s.refusing &&
      this.reason === s.reason
    )
      return
    const crossed = !sameSlot(next, s.target)
    // Where it was asked to go, for the lean a refused drop ends in.
    this.refusedToward = refused ? towardOf(s.source, place, this.axis) : null
    this.set({ target: next, refusing, reason: this.reason })
    this.reshift()
    if (crossed) this.feel("tick")
  }

  /** A place in another lane that decides where a card lands there (`placeIn`): that
   *  place, whatever the hand asked. */
  private pinned(id: ID, source: Slot, place: Slot): Slot {
    if (place.lane === source.lane) return place
    const index = this.opts.placeIn?.(id, place.lane)
    return index === undefined ? place : { lane: place.lane, index }
  }

  /** Every item's spring toward where the carry puts it now. */
  private reshift(spec = CARRY.glide): void {
    const s = this.snap
    if (s.carried === null) return
    const st = this.state()
    for (const [lane, ids] of this.opts.lanes.entries()) {
      const by = shiftsOf(this.layout, st, lane)
      for (const [index, id] of ids.entries()) {
        const v = by[index] ?? 0
        let spring = this.shifts.get(id)
        // A spring only for what moves: a lane of thousands holds a handful.
        if (!spring) {
          if (v === 0) continue
          spring = new Spring(0, spec, 0.1)
          this.shifts.set(id, spring)
        }
        spring.to(v, spec)
      }
      this.footShift[lane]?.to(footShiftOf(this.layout, st, lane), spec)
      this.roomSize[lane]?.to(extraOf(this.layout, st, lane), spec)
    }
    this.kick()
  }

  /** The press lifted: the move is handed over and the card lands where the next
   *  render puts it; over a lane that refuses it, home. */
  private drop(): void {
    const s = this.snap
    if (s.carried === null || s.settling) return
    this.endDrives()
    if (s.refusing !== null) {
      this.set({
        refusals: s.refusals + 1,
        refusedLane: s.refusing,
        said: this.reason ?? this.refusedSaid(s.refusing, s.carried),
      })
      this.feel("refuse")
      this.leanOnLanding = this.refusedToward
      this.goHome()
      return
    }
    this.feel("confirm")
    if (sameSlot(s.source, s.target)) this.goHome()
    else this.commit(s.carried, s.source, s.target)
  }

  /** The move to the consumer; its next render lands it (`lanesChanged`). A move it
   *  does not apply within two frames flies the card home. */
  private commit(id: ID, from: Slot, to: Slot): void {
    const group = this.snap.group
    const what = group.length > 1 ? `${group.length}` : this.opts.name(id)
    this.set({
      source: to,
      target: to,
      settling: true,
      said: `${what} put down, ${this.placeSaid(to)}`,
    })
    this.committing = { id, first: group[0] ?? id, to, waited: 0 }
    const more = this.opts.onMove(id, from, to)
    if (typeof more === "string")
      this.set({ said: `${this.snap.said}, ${more}` })
    const wait = () => {
      const c = this.committing
      if (!c || c.id !== id) return
      if (++c.waited < 3) return void requestAnimationFrame(wait)
      this.committing = null
      this.set({ source: from, target: from })
      this.goHome()
    }
    requestAnimationFrame(wait)
  }

  /** esc, or the press taken away: the card goes back where it was. */
  cancel(): void {
    const s = this.snap
    if (s.carried === null || s.settling) return
    this.endDrives()
    this.set({ said: `${this.opts.name(s.carried)} put back` })
    this.goHome()
  }

  private goHome(): void {
    const s = this.snap
    if (s.carried === null) return
    const id = s.carried
    this.set({
      target: s.source,
      refusing: null,
      settling: true,
      home: true,
    })
    this.reshift()
    if (s.mode === "keys") {
      this.keyed.main.to(0, CARRY.settle)
      this.keyed.cross.to(0, CARRY.settle)
      this.keyed.scale.to(1, CARRY.settle)
      this.when(
        () =>
          this.keyed.main.settled &&
          this.keyed.cross.settled &&
          this.keyed.scale.settled,
        () => this.finish(id),
      )
      return
    }
    this.landOn(id)
  }

  /** The floating card lands on the item's own place, its scale, lean and shadow
   *  settling with it, then the item shows again. */
  private landOn(id: ID): void {
    const el = this.items.get(id)
    if (!el) {
      this.finish(id)
      return
    }
    const r = el.getBoundingClientRect()
    this.float.x.to(r.left, CARRY.settle)
    this.float.y.to(r.top, CARRY.settle)
    this.float.scale.to(1, CARRY.settle)
    this.float.tilt.to(0, CARRY.settle)
    window.clearTimeout(this.righting)
    this.kick()
    this.when(
      () =>
        this.float.x.settled &&
        this.float.y.settled &&
        this.float.scale.settled,
      () => this.finish(id),
    )
  }

  private finish(id: ID): void {
    for (const g of new Set([id, ...this.snap.group])) {
      const el = this.items.get(g)
      if (el) el.style.opacity = ""
    }
    this.freePage()
    this.set({
      carried: null,
      group: [],
      settling: false,
      refusing: null,
      reason: null,
      home: false,
    })
    this.paint()
    // Everything stands in its place, unshifted: what the next new order glides from.
    if (this.opts.glides) this.placed = this.positions()
    // A refused drop, home now: the card leans toward where it was asked to go and
    // springs back, as a refused step by keys does.
    const toward = this.leanOnLanding
    this.leanOnLanding = null
    if (toward) refuse(this.items.get(id), toward[0], toward[1], CARRY.leanStep)
  }

  private endDrives(): void {
    this.main.end()
    this.side.end()
  }

  private driveMain(px: number): boolean {
    const scroller = this.scrollerOf(this.hovered)
    const key = this.axis === "y" ? "scrollTop" : "scrollLeft"
    const was = scroller[key]
    scroller[key] = was + px
    if (Math.abs(scroller[key] - was) < 0.01) return false
    this.retarget()
    return true
  }

  private driveSide(px: number): boolean {
    const b = this.board
    if (!b) return false
    const key = this.axis === "y" ? "scrollLeft" : "scrollTop"
    const was = b[key]
    b[key] = was + px
    if (Math.abs(b[key] - was) < 0.01) return false
    this.retarget()
    return true
  }

  // ── The keys ──────────────────────────────────────────────────────────────────

  keyDown = (id: ID, e: React.KeyboardEvent<HTMLElement>): void => {
    if (e.target !== e.currentTarget) return
    const s = this.snap
    const carrying = s.carried === id && s.mode === "keys"
    const arrow = this.arrow(e.key)
    if (e.key === " ") {
      e.preventDefault()
      if (carrying) this.put()
      else if (s.carried === null) this.pick(id)
      return
    }
    if (e.key === "Enter") {
      e.preventDefault()
      if (carrying) this.put()
      else if (s.carried === null) this.opts.onOpen?.(id)
      return
    }
    if (e.key === "Escape" && s.carried === id) {
      e.preventDefault()
      this.cancel()
      return
    }
    if ((e.key === "Delete" || e.key === "Backspace") && s.carried === null) {
      if (!this.opts.onDelete) return
      e.preventDefault()
      const next = this.successor(id)
      this.opts.onDelete(id)
      if (next !== null)
        requestAnimationFrame(() => this.items.get(next)?.focus())
      return
    }
    if (!arrow || e.metaKey || e.ctrlKey || e.altKey) return
    e.preventDefault()
    if (carrying) {
      this.step(arrow.lane, arrow.index)
      return
    }
    if (s.carried !== null) return
    const next = this.neighbour(id, arrow.lane, arrow.index)
    if (next !== null) this.items.get(next)?.focus()
  }

  /** An arrow as a step along the lane (`index`) or to the next lane (`lane`). */
  private arrow(key: string): { lane: number; index: number } | null {
    const y = this.axis === "y"
    switch (key) {
      case "ArrowUp":
        return y ? { lane: 0, index: -1 } : { lane: -1, index: 0 }
      case "ArrowDown":
        return y ? { lane: 0, index: 1 } : { lane: 1, index: 0 }
      case "ArrowLeft":
        return y ? { lane: -1, index: 0 } : { lane: 0, index: -1 }
      case "ArrowRight":
        return y ? { lane: 1, index: 0 } : { lane: 0, index: 1 }
      default:
        return null
    }
  }

  private neighbour(id: ID, dLane: number, dIndex: number): ID | null {
    const at = this.locate(id)
    if (!at) return null
    const all = this.opts.lanes
    if (dLane !== 0) {
      let lane = at.lane + dLane
      while (all[lane] && all[lane]?.length === 0) lane += dLane
      const ids = all[lane]
      if (!ids || ids.length === 0) return null
      return ids[Math.min(at.index, ids.length - 1)] ?? null
    }
    return all[at.lane]?.[at.index + dIndex] ?? null
  }

  private successor(id: ID): ID | null {
    const at = this.locate(id)
    if (!at) return null
    const ids = this.opts.lanes[at.lane] ?? []
    return ids[at.index + 1] ?? ids[at.index - 1] ?? null
  }

  /** Space on a focused item: it rises in its slot, carried by keys. */
  pick(id: ID): void {
    const slot = this.locate(id)
    if (!slot || this.snap.carried !== null) return
    this.measure()
    this.keyed.main.jump(0)
    this.keyed.cross.jump(0)
    this.keyed.scale.jump(1)
    this.keyed.scale.to(CARRY.scale, CARRY.lift)
    this.items.get(id)?.setAttribute("data-lifted", "")
    this.set({
      carried: id,
      group: [id],
      mode: "keys",
      source: slot,
      target: slot,
      refusing: null,
      settling: false,
      home: false,
      said: `${this.opts.name(id)}, picked up, ${this.placeSaid(slot, id)}. Arrows move it, space puts it down, escape puts it back`,
    })
    this.feel("lift")
    this.kick()
  }

  /** An arrow while carrying by keys: a place along the lane or the next lane; a
   *  place that refuses it is leaned toward and sprung back from. */
  private step(dLane: number, dIndex: number): void {
    const s = this.snap
    if (s.carried === null || s.settling) return
    const st = this.state()
    const next = this.pinned(
      s.carried,
      s.source,
      stepFrom(this.layout, st, dLane, dIndex),
    )
    if (sameSlot(next, s.target)) return
    const refused =
      !sameSlot(next, s.source) &&
      (this.opts.refuses?.(s.carried, s.source, next) ?? false)
    if (refused) {
      this.set({
        refusals: s.refusals + 1,
        refusedLane: next.lane,
        said:
          typeof refused === "string"
            ? refused
            : this.refusedSaid(next.lane, s.carried),
      })
      this.feel("refuse")
      refuse(
        this.items.get(s.carried),
        this.axis === "y" ? dLane : dIndex,
        this.axis === "y" ? dIndex : dLane,
        dLane !== 0 ? CARRY.leanCard : CARRY.leanStep,
      )
      return
    }
    this.set({ target: next, said: this.placeSaid(next) })
    this.reshift()
    // The card carried by keys stands lifted in its gap.
    const from = this.laneBox(s.source.lane)
    const to = this.laneBox(next.lane)
    // Its own place is its home slot's start: the items before it are the same.
    const group = new Set(s.group)
    const home = from.start + slotStart(this.layout, group, s.source)
    const there = to.start + slotStart(this.layout, group, next)
    this.keyed.main.to(there - home, CARRY.glide)
    this.keyed.cross.to(to.cross - from.cross, CARRY.glide)
    this.feel("tick")
    this.kick()
    requestAnimationFrame(() =>
      this.items
        .get(s.carried as ID)
        ?.scrollIntoView({ block: "nearest", inline: "nearest" }),
    )
  }

  /** Space or return while carrying by keys: it lands where it stands. */
  private put(): void {
    const s = this.snap
    if (s.carried === null || s.settling) return
    this.items.get(s.carried)?.removeAttribute("data-lifted")
    this.keyed.scale.to(1, CARRY.settle)
    this.feel("confirm")
    if (sameSlot(s.source, s.target)) {
      const id = s.carried
      this.set({
        said: `${this.opts.name(id)} put down, ${this.placeSaid(s.target)}`,
      })
      this.goHome()
      return
    }
    this.commit(s.carried, s.source, s.target)
  }

  // ── Said and felt ─────────────────────────────────────────────────────────────

  private laneSaid(lane: number): string {
    return this.opts.laneName?.(lane) ?? `lane ${lane + 1}`
  }

  /** A refusal, said: on a board the lane that says no, in one lane the place. */
  private refusedSaid(lane: number, id: ID): string {
    return this.opts.lanes.length > 1
      ? `${this.laneSaid(lane)} does not take ${this.opts.name(id)}`
      : `${this.opts.name(id)} cannot go there`
  }

  /** Where `carried` stands at `slot`, as said: its place among the lane's items with
   *  it among them. */
  private placeSaid(slot: Slot, carried = this.snap.carried): string {
    const ids = this.opts.lanes[slot.lane] ?? []
    const count = ids.filter((id) => id !== carried).length + 1
    const where = `${slot.index + 1} of ${count}`
    return this.opts.lanes.length > 1
      ? `${where} in ${this.laneSaid(slot.lane)}`
      : where
  }

  private feel(kind: keyof typeof FEEL): void {
    if (!this.touch || !("vibrate" in navigator)) return
    navigator.vibrate(FEEL[kind] as number | number[])
  }

  // ── The frame loop ────────────────────────────────────────────────────────────

  private waits: { done: () => boolean; then: () => void }[] = []
  private last = 0

  /** Runs `then` once `done` holds, checked each frame. */
  private when(done: () => boolean, then: () => void): void {
    this.waits.push({ done, then })
    this.kick()
  }

  private kick(): void {
    if (this.frame !== 0) return
    this.last = performance.now()
    this.frame = requestAnimationFrame(this.tick)
  }

  private tick = (now: number): void => {
    this.frame = 0
    const dt = Math.min((now - this.last) / 1000, 1 / 30)
    this.last = now
    let moving = false
    for (const s of this.springs()) if (s.step(dt)) moving = true
    this.paint()
    const ready = this.waits.filter((w) => w.done())
    this.waits = this.waits.filter((w) => !ready.includes(w))
    for (const w of ready) w.then()
    if (moving || this.waits.length > 0)
      this.frame = requestAnimationFrame(this.tick)
  }

  private *springs(): Iterable<Spring> {
    yield* this.shifts.values()
    yield* this.footShift
    yield* this.roomSize
    yield this.float.x
    yield this.float.y
    yield this.float.scale
    yield this.float.tilt
    yield this.keyed.main
    yield this.keyed.cross
    yield this.keyed.scale
  }

  /** Writes one item where the carry puts it: the card carried by keys lifted in its
   *  gap, any other by its shift (and `onShift`, for what moves with it). */
  private paintItem(id: ID): void {
    const y = this.axis === "y"
    const s = this.snap
    const el = this.items.get(id)
    if (s.carried === id && s.mode === "keys") {
      if (!el) return
      const main = this.keyed.main.value
      const cross = this.keyed.cross.value
      el.style.transform = `translate(${y ? cross : main}px, ${y ? main : cross}px) scale(${this.keyed.scale.value})`
      el.style.zIndex = "10"
      el.style.position ||= "relative"
      return
    }
    const v = this.shifts.get(id)?.value ?? 0
    this.opts.onShift?.(id, v)
    if (!el) return
    el.style.transform =
      v === 0 ? "" : y ? `translateY(${v}px)` : `translateX(${v}px)`
    if (el.style.zIndex === "10") el.style.zIndex = ""
  }

  /** Writes every spring to its element. */
  private paint(): void {
    const y = this.axis === "y"
    const s = this.snap
    for (const id of this.shifts.keys()) this.paintItem(id)
    if (s.carried !== null && !this.shifts.has(s.carried))
      this.paintItem(s.carried)
    for (const [i, spring] of this.footShift.entries()) {
      const el = this.feet[i]
      if (el)
        el.style.transform =
          spring.value === 0
            ? ""
            : y
              ? `translateY(${spring.value}px)`
              : `translateX(${spring.value}px)`
    }
    for (const [i, spring] of this.roomSize.entries()) {
      const el = this.rooms[i]
      if (el)
        el.style[y ? "height" : "width"] = `${Math.max(0, spring.value)}px`
    }
    const f = this.floatEl
    if (f) {
      const fl = this.float
      f.style.width = `${fl.width}px`
      f.style.transformOrigin = `${fl.anchor.x}px ${fl.anchor.y}px`
      f.style.transform = `translate(${fl.x.value}px, ${fl.y.value}px) rotate(${fl.tilt.value}deg) scale(${fl.scale.value})`
    }
  }
}

/** The way `to` lies from `from` on screen, one step each way at most: a lane is a
 *  step across the lanes, a place a step along one. */
function towardOf(from: Slot, to: Slot, axis: CarryAxis): [number, number] {
  const across = Math.sign(to.lane - from.lane)
  const along = across !== 0 ? 0 : Math.sign(to.index - from.index)
  return axis === "y" ? [across, along] : [along, across]
}

/** "Not now", seen: the element leans `points` toward (dx, dy), out in 0.09 s, home on
 *  a spring that overshoots once (Ink's InkRefusal), on its own `translate` so the
 *  carry's `transform` is untouched. */
export function refuse(
  el: Element | null | undefined,
  dx: number,
  dy: number,
  points: number,
): void {
  if (!el || matchMedia("(prefers-reduced-motion: reduce)").matches) return
  const at = (k: number) => `${dx * points * k}px ${dy * points * k}px`
  el.animate(
    [
      { translate: "0 0", offset: 0, easing: "cubic-bezier(0.2, 0, 0.4, 1)" },
      {
        translate: at(1),
        offset: 0.13,
        easing: "cubic-bezier(0.3, 1.6, 0.5, 1)",
      },
      { translate: "0 0", offset: 1 },
    ],
    // Added to the element's own translate (a table's row is placed by it), never in
    // place of it.
    { duration: 690, composite: "add" },
  )
}

export type Carry<ID> = {
  engine: Carrier<ID>
  state: CarrySnapshot<ID>
  /** The id of the text that tells a screen reader how to carry. */
  hint: string
  /** Every item's props: its element, the press, the keys, one Tab stop per board. */
  item: (id: ID) => {
    ref: (el: HTMLElement | null) => void
    tabIndex: number
    onPointerDown: (e: React.PointerEvent<HTMLElement>) => void
    onKeyDown: (e: React.KeyboardEvent<HTMLElement>) => void
    onFocus: () => void
    "aria-describedby": string
    "data-carried"?: ""
  }
  /** A lane's content: the element its items stand in, in order. */
  lane: (i: number) => (el: HTMLElement | null) => void
  /** A lane's own scroller, when it has one the lane is not inside of. */
  laneScroller: (i: number) => (el: HTMLElement | null) => void
  /** What follows a lane's last item (an add row): moved as the carried slot closes. */
  foot: (i: number) => (el: HTMLElement | null) => void
  /** A spacer after a lane's items: the room it gains as a card from another comes. */
  room: (i: number) => (el: HTMLElement | null) => void
  /** What scrolls a board across its lanes. */
  across: (el: HTMLElement | null) => void
  light: (lane: number) => CarryLight
}

/** A board's carry: one engine, its snapshot, and the props its parts wear. Draw a
 *  `CarryLayer` with it. */
export function useCarry<ID extends string | number>(
  opts: CarryOptions<ID>,
): Carry<ID> {
  const engine = React.useRef<Carrier<ID> | null>(null)
  engine.current ??= new Carrier(opts)
  const e = engine.current
  e.opts = opts
  const state = React.useSyncExternalStore(e.subscribe, e.snapshot, e.snapshot)
  const hint = React.useId()
  // The lanes the consumer draws after a move: the move landed. Run for every new
  // lanes, which the engine reads through `opts`, set above.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the lanes are the trigger
  React.useLayoutEffect(() => {
    e.lanesChanged()
  }, [opts.lanes, e])
  const [focus, setFocus] = React.useState<ID | null>(null)
  const first = opts.lanes.find((l) => l.length > 0)?.[0] ?? null
  const stop =
    focus !== null && opts.lanes.some((l) => l.includes(focus)) ? focus : first
  return {
    engine: e,
    state,
    hint,
    item: (id) => ({
      ref: (el) => e.item(id, el),
      tabIndex: id === stop ? 0 : -1,
      onPointerDown: (ev) => e.pointerDown(id, ev),
      onKeyDown: (ev) => e.keyDown(id, ev),
      onFocus: () => setFocus(id),
      "aria-describedby": hint,
      ...(state.carried === id ? { "data-carried": "" as const } : {}),
    }),
    lane: (i) => (el) => e.lane(i, el),
    laneScroller: (i) => (el) => e.laneScroller(i, el),
    foot: (i) => (el) => e.foot(i, el),
    room: (i) => (el) => e.room(i, el),
    across: (el) => e.across(el),
    light: (lane) => {
      if (state.carried === null || state.settling) return "none"
      if (state.refusing === lane) return "refusing"
      return state.refusing === null && state.target.lane === lane
        ? "receiving"
        : "none"
    },
  }
}

/** The carried card drawn over everything where the press is, what is said to a
 *  screen reader, and how to carry. `render` draws the carried item's face (the
 *  item's own, as it stands in its lane); `className` its floating box. */
export function CarryLayer<ID extends string | number>({
  carry,
  render,
  className,
}: {
  carry: Carry<ID>
  render: (id: ID) => React.ReactNode
  className?: string
}) {
  const { state, engine, hint } = carry
  const [host, setHost] = React.useState<HTMLElement | null>(null)
  React.useEffect(() => setHost(document.body), [])
  const floating = state.carried !== null && state.mode === "pointer"
  return (
    <>
      <span id={hint} className="sr-only">
        space picks it up, the arrows move it, space puts it down, escape puts
        it back
      </span>
      <span aria-live="polite" className="sr-only">
        {state.said}
      </span>
      {host &&
        floating &&
        createPortal(
          <div
            ref={(el) => engine.floatAt(el)}
            aria-hidden="true"
            data-slot="carry-float"
            data-refusing={state.refusing === null ? undefined : ""}
            className={cn(
              "pointer-events-none fixed top-0 left-0 z-[100] will-change-transform",
              LIFTED,
              className,
            )}
          >
            {render(state.carried as ID)}
            {/* Over a place that refuses it, the card says so where the eye is: a
                wash of the bad tone over it (Ink's refusal, a state and never the
                card's own colour) and, when the refusal has words, them under it. */}
            {state.refusing !== null && (
              <>
                <span
                  data-slot="carry-refusing"
                  className="absolute inset-0 rounded-[inherit] bg-tone-bad/14 ring-1 ring-tone-bad/50 ring-inset"
                />
                {state.reason && (
                  <span
                    data-slot="carry-reason"
                    className="absolute top-full left-0 mt-2 flex max-w-full items-center gap-1.5 rounded-md border border-border bg-popover px-2 py-1 text-popover-foreground text-xs shadow-md"
                  >
                    <CircleAlert
                      aria-hidden
                      className="size-3.5 shrink-0 text-tone-bad"
                    />
                    {state.reason}
                  </span>
                )}
              </>
            )}
          </div>,
          host,
        )}
    </>
  )
}
