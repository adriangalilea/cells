// CARRY: a thing taken up under a press and put down elsewhere (a card in a list, a
// card across a board's lanes, a column along a strip), the math of it with no DOM
// and no framework: the numbers, one spring, the edge drive, and where the gap
// opens. swift-utils' Ink carries the same way (InkDragList, InkKanban, DragBoard,
// InkEdgeDrive); every number here is Ink's, measured there, said in the unit it is
// felt in. ui/carry binds it to the pointer, the keys and the DOM.
//
// A board is lanes of items (one lane for a list or a strip). An item has an extent
// along the lanes' axis (its height in a column, its width in a strip). While one is
// carried, its slot closes and a gap of its extent opens where it would land: the
// others are moved, never laid out again, so every position follows from the extents
// read once at the lift and the one layout the lanes draw.

export function assert(cond: unknown, msg: string): asserts cond {
  if (!cond) throw new Error(`carry: ${msg}`)
}

/** A spring as SwiftUI states one: how long it takes, and how much it overshoots (0
 *  critically damped, never past its target). */
export type SpringSpec = { duration: number; bounce: number }

/** Ink's carry, number for number (DragMetric, InkRefusal, Press). */
export const CARRY = {
  /** A carried card is the hand's, no longer a container under it: it grows a touch,
   *  from under the press. */
  scale: 1.035,
  /** It leans into its sideways travel by the press's speed: the full lean at
   *  `fullLean` px a second. */
  maxTilt: 3,
  fullLean: 1400,
  /** How far past a neighbour's middle the carried card's middle goes before the gap
   *  crosses it, as a share of that neighbour's extent, either way: a card held on
   *  the line never flickers the gap. */
  hysteresis: 0.2,
  /** Taken up: a little overshoot in the scale, the life of a lift. */
  lift: { duration: 0.3, bounce: 0.3 },
  /** The others gliding out of the way: critically damped, so a gap retargeted
   *  mid-glide carries its velocity on and never wobbles. */
  glide: { duration: 0.3, bounce: 0 },
  /** Let go: landing in the gap, or flying home. 0.1 bounce overshoots under a
   *  thousandth of the distance, so it lands exactly. */
  settle: { duration: 0.32, bounce: 0.1 },
  /** The lean following the travel, and righting itself once the press stops. */
  follow: { duration: 0.18, bounce: 0 },
  upright: { duration: 0.45, bounce: 0.35 },
  /** No movement for this long is a press slowing to a stop: the lean rights. */
  stillMs: 60,
  /** A finger takes a card up once held this long, so a stroke that starts at once
   *  still scrolls (Ink's Press.holdSeconds). */
  holdMs: 280,
  /** A finger that travels this far before the hold is a scroll, never a carry. */
  holdSlop: 10,
  /** A pointer takes a card up once it moves this far: a press that never moves is a
   *  click. */
  travel: 3,
  /** "Not now": a refused step leans toward where it was asked to go, out in 0.09 s
   *  and home on a spring that overshoots once, by what moves: a carried card 14, a
   *  step by keys 10, a count 8. */
  leanCard: 14,
  leanStep: 10,
  leanCount: 8,
} as const satisfies Record<string, number | SpringSpec>

// ── One spring, interruptible ─────────────────────────────────────────────────────

/** A value on a spring: retargeted at any moment, it carries its velocity on, so a
 *  gap moved again mid-glide never jumps. Stiffness and damping are SwiftUI's from a
 *  duration and a bounce (k = (2π/d)², c = 4π(1 - bounce)/d, mass 1), integrated in
 *  small fixed steps. */
export class Spring {
  value: number
  velocity = 0
  target: number
  private k = 0
  private c = 0
  /** Settled once both are under these, in the value's own unit. */
  private readonly rest: number

  constructor(value: number, spec: SpringSpec, rest = 0.01) {
    this.value = value
    this.target = value
    this.rest = rest
    this.tune(spec)
  }

  tune(spec: SpringSpec): void {
    assert(spec.duration > 0, "a spring takes some time")
    assert(spec.bounce >= 0 && spec.bounce < 1, "a bounce is a share below one")
    this.k = ((2 * Math.PI) / spec.duration) ** 2
    this.c = (4 * Math.PI * (1 - spec.bounce)) / spec.duration
  }

  /** Toward `target`, on `spec` when given, its velocity kept. */
  to(target: number, spec?: SpringSpec): void {
    if (spec) this.tune(spec)
    this.target = target
  }

  /** At `value` at once, still. */
  jump(value: number): void {
    this.value = value
    this.target = value
    this.velocity = 0
  }

  get settled(): boolean {
    return (
      Math.abs(this.value - this.target) < this.rest &&
      Math.abs(this.velocity) < this.rest * 10
    )
  }

  /** `dt` seconds on: false once settled (and then exactly at its target). */
  step(dt: number): boolean {
    if (this.settled) {
      this.value = this.target
      this.velocity = 0
      return false
    }
    const h = 1 / 480
    for (let t = 0; t < dt; t += h) {
      const s = Math.min(h, dt - t)
      const a = -this.k * (this.value - this.target) - this.c * this.velocity
      this.velocity += a * s
      this.value += this.velocity * s
    }
    return true
  }
}

// ── The edge drive ────────────────────────────────────────────────────────────────

/** Where a held press starts scrolling a region and how fast (Ink's InkEdgeDrive):
 *  a zone inside each edge, a share of the region bounded in px so a fingertip always
 *  has room; the speed in region lengths a second, rising as the square of the depth
 *  into the zone (the first px creep, where precision lives), full at `reach` zones
 *  deep (half a zone past the edge, where a pointer can go), eased in over `ramp`
 *  seconds once it engages and stopped at once when the press leaves the zone. A press
 *  that lands inside a zone drives that edge only once it travels `arming` px toward
 *  it. Under reduced motion the top speed is a third. */
export const EDGE = {
  zone: 0.15,
  zonePx: { min: 28, max: 80 },
  reach: 1.5,
  top: 1.5,
  ramp: 0.3,
  arming: 8,
} as const

/** The zone's depth for a region `length` px long. */
export function zoneDepth(length: number): number {
  return Math.min(
    Math.max(length * EDGE.zone, EDGE.zonePx.min),
    Math.min(EDGE.zonePx.max, length / 3),
  )
}

/** The share of top speed a press `inside` px from an edge asks (negative past it): 0
 *  outside the zone, the square of the depth, 1 at `reach` zones deep. */
export function edgeShare(inside: number, depth: number): number {
  if (depth <= 0 || inside >= depth) return 0
  const u = Math.min((depth - inside) / depth / EDGE.reach, 1)
  return u * u
}

/** The signed speed, px a second, for a press at `p` in `lo..hi` (negative toward
 *  `lo`), before the ramp. */
export function edgeSpeed(
  p: number,
  lo: number,
  hi: number,
  reduced: boolean,
): number {
  const length = hi - lo
  if (length <= 0) return 0
  const depth = zoneDepth(length)
  const toLow = edgeShare(p - lo, depth)
  const toHigh = edgeShare(hi - p, depth)
  return (toHigh - toLow) * EDGE.top * (reduced ? 1 / 3 : 1) * length
}

/** How much of the speed the ramp lets through `t` seconds after engaging: a
 *  smoothstep, never a jump. */
export function ramped(t: number): number {
  const x = Math.min(Math.max(t / EDGE.ramp, 0), 1)
  return x * x * (3 - 2 * x)
}

/** A region along one axis: where it starts and ends, px. */
export type Span = { lo: number; hi: number }

/** Runs the edge drive while a press is held: told where the press is over a region
 *  (`follow`), it steps the region once a frame while the press is in a zone, then
 *  sleeps. `step` moves the region by the px it is given and says whether it could;
 *  at the region's end the drive waits for the press to move again. */
export class EdgeDriver {
  reduced = false
  private point: number | null = null
  private span: Span = { lo: 0, hi: 0 }
  private origin: number | null = null
  private armed = { low: false, high: false }
  private engaged: number | null = null
  private last: number | null = null
  private frame = 0

  constructor(private readonly step: (px: number) => boolean) {}

  get driving(): boolean {
    return this.engaged !== null
  }

  /** The press is at `p` over `span`; the first call of a press is where it began. */
  follow(p: number, span: Span): void {
    this.span = span
    this.point = p
    if (this.origin === null) {
      this.origin = p
      const depth = zoneDepth(span.hi - span.lo)
      this.armed = { low: p - span.lo >= depth, high: span.hi - p >= depth }
    }
    if (p <= this.origin - EDGE.arming) this.armed.low = true
    if (p >= this.origin + EDGE.arming) this.armed.high = true
    if (this.velocity() === 0) {
      this.pause()
      return
    }
    if (this.frame !== 0) return
    this.engaged ??= performance.now()
    this.last = null
    this.frame = requestAnimationFrame(this.tick)
  }

  /** The press lifted or was taken away: the region stops at once. */
  end(): void {
    this.pause()
    this.point = null
    this.origin = null
    this.armed = { low: false, high: false }
  }

  private velocity(): number {
    if (this.point === null) return 0
    const s = edgeSpeed(this.point, this.span.lo, this.span.hi, this.reduced)
    return s < 0 ? (this.armed.low ? s : 0) : this.armed.high ? s : 0
  }

  private pause(): void {
    if (this.frame !== 0) cancelAnimationFrame(this.frame)
    this.frame = 0
    this.engaged = null
    this.last = null
  }

  private tick = (now: number): void => {
    this.frame = 0
    const v = this.velocity()
    if (v === 0 || this.engaged === null) {
      this.pause()
      return
    }
    const dt = Math.min((now - (this.last ?? now - 1000 / 120)) / 1000, 1 / 30)
    this.last = now
    const px = v * ramped((now - this.engaged) / 1000) * dt
    // At the region's end: nothing to drive until the press moves again.
    if (!this.step(px)) return
    this.frame = requestAnimationFrame(this.tick)
  }
}

// ── Where the gap opens ───────────────────────────────────────────────────────────

/** A place in a board: a lane, and an index in it counted without the carried item,
 *  which is also its index once it lands there. */
export type Slot = { lane: number; index: number }

export const sameSlot = (a: Slot, b: Slot) =>
  a.lane === b.lane && a.index === b.index

/** A lane as read at the lift: its items in order, each one's extent, the padding
 *  before the first and the spacing between them (the lane's own CSS gap). */
export type LaneLayout<ID> = {
  ids: readonly ID[]
  extent: ReadonlyMap<ID, number>
  pad: number
  spacing: number
}

/** A carry in progress, as the layout reads it. One item is carried (the pressed
 *  one), with the items carried together with it (`group`, itself included: a
 *  selection of rows), in one lane. */
export type CarryState<ID> = {
  carried: ID
  group: ReadonlySet<ID>
  /** Where the group was, counted among the items that stay (the first of the
   *  group's place). */
  source: Slot
  target: Slot
  /** The group's extent together (and the spacing between its items): the gap it
   *  leaves and opens. */
  gap: number
  /** The lane under the card that refuses it: no gap opens anywhere. */
  refusing: number | null
  /** Let go and travelling into its slot: its gap is already the layout. */
  settling: boolean
  /** Going home: every item back in its own place, the group's included. */
  home: boolean
}

/** A lane's items without the carried ones. */
export function others<ID>(lane: LaneLayout<ID>, group: ReadonlySet<ID>): ID[] {
  return lane.ids.filter((id) => !group.has(id))
}

/** The gap a group leaves in its lane: its items' extents and the spacing between. */
export function gapOf<ID>(lane: LaneLayout<ID>, group: readonly ID[]): number {
  let gap = 0
  for (const id of group) gap += lane.extent.get(id) ?? 0
  return gap + lane.spacing * Math.max(0, group.length - 1)
}

/** Where item `index` of `ids` begins along its lane. */
export function startOf<ID>(
  lane: LaneLayout<ID>,
  ids: readonly ID[],
  index: number,
): number {
  let at = lane.pad
  for (let i = 0; i < index; i++)
    at += (lane.extent.get(ids[i] as ID) ?? 0) + lane.spacing
  return at
}

/** Where a slot's gap begins along its lane. */
export function slotStart<ID>(
  lanes: readonly LaneLayout<ID>[],
  group: ReadonlySet<ID>,
  slot: Slot,
): number {
  const lane = lanes[slot.lane]
  assert(lane, `no lane ${slot.lane}`)
  return startOf(lane, others(lane, group), slot.index)
}

/** How far each item of `lane` (in the lane's own order) moves out of the carried
 *  group's way: back by the slots of the group before it, which close, and on by the
 *  gap where it opens. Over a lane that refuses it no gap opens, not even at home,
 *  until it is let go; going home, nothing is moved. */
export function shiftsOf<ID>(
  lanes: readonly LaneLayout<ID>[],
  s: CarryState<ID>,
  lane: number,
): number[] {
  const l = lanes[lane]
  if (!l) return []
  if (s.home) return l.ids.map(() => 0)
  const opens = lane === s.target.lane && (s.refusing === null || s.settling)
  // One pass: the group's slots closed so far, and the place among those that stay.
  let closed = 0
  let at = 0
  return l.ids.map((id) => {
    if (s.group.has(id)) {
      if (lane === s.source.lane) closed += (l.extent.get(id) ?? 0) + l.spacing
      return 0
    }
    const by = -closed + (opens && at >= s.target.index ? s.gap + l.spacing : 0)
    at += 1
    return by
  })
}

/** Room a lane gains at its foot while a card from another lane would land in it. */
export function extraOf<ID>(
  lanes: readonly LaneLayout<ID>[],
  s: CarryState<ID>,
  lane: number,
): number {
  return !s.home && lane === s.target.lane && lane !== s.source.lane
    ? s.gap + (lanes[lane]?.spacing ?? 0)
    : 0
}

/** How far what follows a lane's last item moves back once the carried card's slot
 *  has closed (its gap open in another lane, or none anywhere over a refusing lane). */
export function footShiftOf<ID>(
  lanes: readonly LaneLayout<ID>[],
  s: CarryState<ID>,
  lane: number,
): number {
  if (s.home || lane !== s.source.lane) return 0
  const closed =
    s.target.lane !== s.source.lane || (s.refusing !== null && !s.settling)
  return closed ? -(s.gap + (lanes[lane]?.spacing ?? 0)) : 0
}

/** Where the carried card's middle is (`middle`, along `lane`), the gap opens: the
 *  index it would take there, crossing a neighbour only once the middle is
 *  `hysteresis` of that neighbour past its middle, either way from where the gap is. */
export function placeAt<ID>(
  lanes: readonly LaneLayout<ID>[],
  s: CarryState<ID>,
  lane: number,
  middle: number,
): Slot {
  const l = lanes[lane]
  assert(l, `no lane ${lane}`)
  const ids = others(l, s.group)
  const open =
    s.refusing === null && s.target.lane === lane ? s.target.index : null
  let index = 0
  while (index < ids.length) {
    const h = l.extent.get(ids[index] as ID) ?? 0
    let line = startOf(l, ids, index) + h / 2
    if (open !== null) line += (index >= open ? 1 : -1) * h * CARRY.hysteresis
    if (middle <= line) break
    index += 1
  }
  return { lane, index }
}

/** A step by keys from `target`: a place along its lane, or the next lane at the same
 *  index (clamped to what that lane holds). */
export function stepFrom<ID>(
  lanes: readonly LaneLayout<ID>[],
  s: CarryState<ID>,
  dLane: number,
  dIndex: number,
): Slot {
  if (dLane !== 0) {
    const lane = Math.min(Math.max(s.target.lane + dLane, 0), lanes.length - 1)
    const l = lanes[lane] as LaneLayout<ID>
    return {
      lane,
      index: Math.min(s.target.index, others(l, s.group).length),
    }
  }
  const l = lanes[s.target.lane] as LaneLayout<ID>
  return {
    lane: s.target.lane,
    index: Math.min(
      Math.max(s.target.index + dIndex, 0),
      others(l, s.group).length,
    ),
  }
}

/** The lean for a sideways speed, degrees. */
export function tiltFor(speed: number): number {
  return Math.max(-1, Math.min(1, speed / CARRY.fullLean)) * CARRY.maxTilt
}
