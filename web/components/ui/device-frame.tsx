"use client"

import type { ComponentProps, ReactNode } from "react"
import * as React from "react"
import { cn } from "@/lib/utils"
import {
  fanTurnRate,
  type ThermalDepth,
  type ThermalPart,
  type ThermalReading,
} from "@/lib/thermal"
import { DeviceAirflow } from "@/components/ui/device-airflow"
import "./device-frame.css"

type DesktopFrameProps = ComponentProps<"div"> & { screenClassName?: string }

/** Where a MacBook's lid is. `"open"` / `"closed"`: the frame moves the lid itself,
 *  on its own curve, whenever the value changes. A number (0 open .. 1 shut): the
 *  lid sits exactly there and nothing animates, for a caller that owns time (a
 *  scrubbed or filmed timeline, where every frame must be exact). */
export type MacbookLid = "open" | "closed" | number

/** How hot the machine runs, for the frame to show where it is: a reading
 *  (lib/thermal, what the machine reports; the frame maps it, the caller never
 *  picks a colour) and how to show it. A driver with a track hands over
 *  `thermalAt`'s moment as is. */
export interface MacbookThermal extends ThermalReading {
  /** Seconds, for a caller that owns time (a filmed or scrubbed timeline): the
   *  fans and the air are drawn at that instant. It is the film's clock, the
   *  one the viewer watches, never a story clock running faster than it: a
   *  story that plays a minute of heat in six seconds still turns its fans at
   *  their own speed. Omitted, they move on their own. */
  t?: number
  /** How many turns each fan has made by `t`, for a held timeline whose fans
   *  change speed. A fan's position (its slats, the air it has thrown) is
   *  everything it has turned so far, never its speed now times `t`, which
   *  jumps whenever the speed does (`thermalAt` adds up fanTurnRate over its
   *  track). Omitted, each fan is taken as having always turned at its speed
   *  at `t` (exact while the fans hold steady). */
  turns?: number[]
  /** How deep the view goes (ThermalDepth), so a story can move through it
   *  mid-scene: 0 the plain machine, 1 its skin (the case's heat, what a palm
   *  feels: cool reads ice, hot reads ember), 2 the insides (parts, fans, the
   *  air), the skin receding behind them and the body's outer details with it.
   *  Live (no `t`), a change fades over a moment; held (`t` given), the number
   *  is exact. Default 2. */
  depth?: ThermalDepth
}

const clamp01 = (v: number) => Math.min(1, Math.max(0, v))

/** Each fan's share, left and right; one reported fan drives both. */
const fanShares = (thermal: MacbookThermal) =>
  [0, 1].map((i) => clamp01(thermal.fans[i] ?? thermal.fans[0] ?? 0))

/** The range each part is painted over, °C: from where it shows to its limit. */
const RANGE: Record<ThermalPart, [number, number]> = {
  cpu: [45, 100],
  gpu: [45, 100],
  memory: [45, 95],
  ssd: [35, 70],
  battery: [30, 50],
}

const heat = (part: ThermalPart, c: number | undefined) => {
  if (c === undefined) return 0
  const [from, to] = RANGE[part]
  return Math.min(1, Math.max(0, (c - from) / (to - from)))
}

/** chill's palette: ice cool, ember warm, red at the limit. */
const ICE = [140, 196, 255]
const EMBER = [255, 140, 64]
const RED = [255, 64, 52]
function tone(h: number): string {
  const [a, b, k] =
    h < 0.7 ? [ICE, EMBER, h / 0.7] : [EMBER, RED, (h - 0.7) / 0.3]
  return `rgb(${a.map((v, i) => Math.round(v + ((b[i] as number) - v) * k)).join(" ")})`
}

/** The lid as the camera sees it, in cqw: its border box W by H (84% of the
 *  frame, a 16:10 display in its bezel), its top corners' radius R, its
 *  thickness T, and the eye at the hinge's height at distance P, where the lid
 *  shut spans the base's width exactly at its widest, just behind its rounded
 *  top corners (W P / (P - (H - R)) = 100, the base drawn at its own widest).
 *  device-frame.css turns the face with the same perspective. */
const LID = { W: 84, H: 55.1125, R: 2.7, T: 1, P: 327.578 }
/** How far above the frame's foot the hinge is: the base's height less the
 *  overlap that hides the lid's foot (device-frame.css `.device-base`). */
const HINGE = 3.55
/** That overlap: open the base's lip covers the lid's foot by it; closing, the
 *  hinge rises by it (times sin of the angle), so shut the lid rests on the
 *  base's top instead of sinking into it. device-frame.css lifts the face alike. */
const LIP = 0.15
/** The edge's drawing box, in cqw above the hinge: the lid's height and the
 *  most its top rises while it folds. */
const RISE = 60

/** The lid's outline in its own plane, x from its centre, y up from the hinge,
 *  drawn `inset` in from the edge: the top corners rounded, the sides straight
 *  down to the hinge. */
function outline(inset: number): [number, number][] {
  const W = LID.W - 2 * inset
  const H = LID.H - inset
  const R = LID.R - inset
  const n = 16
  const pts: [number, number][] = [[-W / 2, 0]]
  for (let i = 0; i <= n; i++) {
    const a = Math.PI - (i / n) * (Math.PI / 2)
    pts.push([-W / 2 + R + R * Math.cos(a), H - R + R * Math.sin(a)])
  }
  for (let i = 0; i <= n; i++) {
    const a = Math.PI / 2 - (i / n) * (Math.PI / 2)
    pts.push([W / 2 - R + R * Math.cos(a), H - R + R * Math.sin(a)])
  }
  pts.push([W / 2, 0])
  return pts
}
/** The edge's fillet where it meets the lid's back, the rounding a shut lid's
 *  ends show from the front. */
const FILLET = 0.45
/** The solid as outlines at depths behind the face: the face's own, then the
 *  back's, rounded over the fillet's quarter circle (each a little further back
 *  and a little smaller), so the hull of them all is the edge with its round. */
const LAYERS: { depth: number; pts: [number, number][] }[] = [
  { depth: 0, pts: outline(0) },
  ...[0, 1, 2, 3, 4].map((i) => {
    const phi = (i / 4) * (Math.PI / 2)
    return {
      depth: -(LID.T - FILLET) - FILLET * Math.sin(phi),
      pts: outline(FILLET * (1 - Math.cos(phi))),
    }
  }),
]

/** The lid's silhouette at `shut` (0 open .. 1 shut), as an SVG path in the edge's
 *  box (x 0..100 across the frame, y down from RISE above the hinge). The lid is a
 *  rounded rectangle pushed back by its thickness, a convex solid, so what any
 *  eye sees of it is exactly the convex hull of its face's outline and its back's,
 *  each turned about the hinge and projected. Drawn behind the face, what shows
 *  past the face is the true edge: thin open, round its corners, the closed
 *  laptop's front shut. */
function lidHull(shut: number): { d: string; top: number; face: number } {
  const { P } = LID
  const a = (shut * Math.PI) / 2
  const cos = Math.cos(a)
  const sin = Math.sin(a)
  const pts: [number, number][] = []
  for (const { depth, pts: layer } of LAYERS) {
    for (const [x, y] of layer) {
      const up = y * cos - depth * sin
      const near = y * sin + depth * cos
      const s = P / (P - near)
      pts.push([50 + x * s, RISE - up * s - LIP * sin])
    }
  }
  pts.sort((p, q) => p[0] - q[0] || p[1] - q[1])
  type Point = [number, number]
  const cross = (o: Point, p: Point, q: Point) =>
    (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0])
  const half = (list: Point[]) => {
    const h: Point[] = []
    for (const p of list) {
      while (
        h.length >= 2 &&
        cross(h[h.length - 2] as Point, h[h.length - 1] as Point, p) <= 0
      )
        h.pop()
      h.push(p)
    }
    h.pop()
    return h
  }
  const hull = [...half(pts), ...half([...pts].reverse())]
  // The face's top edge at its middle, where the edge's visible height ends.
  const faceUp = LID.H * cos
  const face = RISE - (faceUp * P) / (P - LID.H * sin) - LIP * sin
  return {
    d: `M ${hull.map(([x, y]) => `${x.toFixed(3)} ${y.toFixed(3)}`).join(" L ")} Z`,
    top: Math.min(...hull.map(([, y]) => y)),
    face,
  }
}

/** CSS's cubic-bezier(x1, y1, x2, y2) as a function of progress, solved by
 *  bisection on x. */
function bezier(x1: number, y1: number, x2: number, y2: number) {
  const at = (p1: number, p2: number, t: number) =>
    3 * p1 * t * (1 - t) ** 2 + 3 * p2 * t ** 2 * (1 - t) + t ** 3
  return (x: number) => {
    let lo = 0
    let hi = 1
    for (let i = 0; i < 24; i++) {
      const mid = (lo + hi) / 2
      if (at(x1, x2, mid) < x) lo = mid
      else hi = mid
    }
    return at(y1, y2, (lo + hi) / 2)
  }
}
/** The frame's own lid motion: 1.1 s on one easing, the angle known every frame. */
const LID_MS = 1100
const LID_EASE = bezier(0.65, 0, 0.35, 1)

/** Where the lid is drawn: a number is the caller's instant, exact; "open" and
 *  "closed" move it on the frame's curve whenever they change (at once under
 *  reduced motion). */
function useLidAt(lid: MacbookLid | undefined): number | undefined {
  const target =
    lid === undefined
      ? undefined
      : typeof lid === "number"
        ? Math.min(1, Math.max(0, lid))
        : lid === "closed"
          ? 1
          : 0
  const owned = typeof lid === "string"
  const [moving, setMoving] = React.useState(target)
  const shown = React.useRef(target)
  React.useEffect(() => {
    if (!owned || target === undefined) return
    const from = shown.current ?? target
    if (
      from === target ||
      matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      shown.current = target
      setMoving(target)
      return
    }
    const start = performance.now()
    let raf = 0
    const step = (now: number) => {
      const k = Math.min(1, (now - start) / LID_MS)
      const at = from + (target - from) * LID_EASE(k)
      shown.current = at
      setMoving(at)
      if (k < 1) raf = requestAnimationFrame(step)
    }
    raf = requestAnimationFrame(step)
    return () => cancelAnimationFrame(raf)
  }, [owned, target])
  return owned ? moving : target
}

/** The lid's edge: its silhouette (lidHull) drawn in the base's aluminium behind
 *  the face, lit along its top, and the case's heat over it when shown. */
function LidEdge({
  shut,
  thermal,
}: {
  shut: number
  thermal?: MacbookThermal
}) {
  const id = React.useId()
  const hull = lidHull(shut)
  const stops =
    thermal?.surface === undefined
      ? undefined
      : skinStops(thermal.surface, fanShares(thermal))
  return (
    <svg
      aria-hidden
      className="device-lid-edge"
      viewBox={`0 0 100 ${RISE}`}
      preserveAspectRatio="none"
      style={{ bottom: `${HINGE}cqw`, height: `${RISE}cqw` }}
    >
      <defs>
        {/* The base's own aluminium (device-frame.css `.device-base`): lit along
            the top and darkening down the visible edge, from its top to the
            face's top edge, so shut the lid's front and the base read as one. */}
        <linearGradient
          id={`${id}-metal`}
          gradientUnits="userSpaceOnUse"
          x1="0"
          y1={hull.top}
          x2="0"
          y2={Math.max(hull.face, hull.top + 0.1)}
        >
          <stop offset="0" stopColor="#5f5f63" />
          <stop offset="0.04" stopColor="#424245" />
          <stop offset="0.18" stopColor="#27272b" />
          <stop offset="0.62" stopColor="#1c1c20" />
          <stop offset="0.84" stopColor="#101013" />
          <stop offset="0.93" stopColor="#39393c" />
          <stop offset="1" stopColor="#131315" />
        </linearGradient>
        {/* And the base's end shading, across the frame's width, the same at
            every angle as on the base below it. */}
        <linearGradient
          id={`${id}-ends`}
          gradientUnits="userSpaceOnUse"
          x1="0"
          y1="0"
          x2="100"
          y2="0"
        >
          <stop offset="0" stopColor="#141416" />
          <stop offset="0.006" stopColor="#505054" />
          <stop offset="0.02" stopColor="#202023" />
          <stop offset="0.07" stopColor="#202023" stopOpacity="0" />
          <stop offset="0.93" stopColor="#202023" stopOpacity="0" />
          <stop offset="0.98" stopColor="#202023" />
          <stop offset="0.994" stopColor="#505054" />
          <stop offset="1" stopColor="#141416" />
        </linearGradient>
        {stops && (
          <linearGradient id={`${id}-skin`} x1="0" y1="0" x2="1" y2="0">
            {stops.map((s) => (
              <stop
                key={s.offset}
                offset={s.offset}
                stopColor={tone(s.heat)}
                stopOpacity={s.alpha * 0.3}
              />
            ))}
          </linearGradient>
        )}
      </defs>
      <path d={hull.d} fill={`url(#${id}-metal)`} />
      <path d={hull.d} fill={`url(#${id}-ends)`} />
      {stops && (
        <path
          className="device-lid-skin"
          d={hull.d}
          fill={`url(#${id}-skin)`}
        />
      )}
    </svg>
  )
}

/** A 16:10 laptop display with a shallow base. `lid` puts it on a hinge (see
 *  MacbookLid): the face turns in true perspective (device-frame.css) and the
 *  lid's edge is its exact silhouette under the same camera (lidHull), so shut it
 *  is the closed laptop's front, resting on the base. Without `lid` the frame is
 *  flat 2D.
 *  `thermal` shows the machine's insides flat in the base's face, where they
 *  sit (MacbookThermal, MacbookInternals). */
export function MacbookFrame({
  children,
  className,
  screenClassName,
  notch = true,
  lid,
  thermal,
  style,
  ...props
}: DesktopFrameProps & {
  notch?: boolean
  lid?: MacbookLid
  thermal?: MacbookThermal
}) {
  const shut = useLidAt(lid)
  const vars: Record<string, number> = {}
  if (shut !== undefined) vars["--device-lid"] = shut
  if (thermal) {
    const depth = Math.min(2, Math.max(0, thermal.depth ?? 2))
    const inside = clamp01(depth - 1)
    vars["--device-skin"] = Math.min(1, depth) * (1 - inside * 0.6)
    vars["--device-xray"] = inside
  }
  return (
    <div
      {...props}
      data-device="macbook"
      data-hinged={lid === undefined ? undefined : ""}
      data-thermal={
        thermal ? (thermal.t === undefined ? "live" : "held") : undefined
      }
      className={cn("device-desktop", className)}
      style={{ ...style, ...vars } as React.CSSProperties}
    >
      <div className="device-lid">
        <div className={cn("device-display", screenClassName)}>{children}</div>
        {notch && <span aria-hidden className="device-notch" />}
        <span aria-hidden className="device-camera" />
      </div>
      {shut !== undefined && <LidEdge shut={shut} thermal={thermal} />}
      <div aria-hidden className="device-base">
        {thermal?.surface !== undefined && (
          <MacbookSkin surface={thermal.surface} fans={fanShares(thermal)} />
        )}
        {thermal && <MacbookInternals thermal={thermal} />}
      </div>
      <span aria-hidden className="device-rubber left" />
      <span aria-hidden className="device-rubber right" />
    </div>
  )
}

/** The base's outline in its own units: corners 1.2 on top, 25 below (its
 *  border-radius, 0.12 and 2.5 cqw). */
const SHELL =
  "M 1.7 0.5 H 998.3 Q 999.5 0.5 999.5 1.7 V 12 A 25 24.5 0 0 1 974.5 36.5 H 25.5 A 25 24.5 0 0 1 0.5 12 V 1.7 Q 0.5 0.5 1.7 0.5 Z"

/** The skin's heat across the machine, left to right (0..1 of its width): from
 *  a cool room's 26 °C to hot at 46, so every degree a case gains or sheds
 *  shows, down to the last of its cooling; hottest over the chip and the
 *  battery under it, coolest where the fans move air. One reading for every
 *  face that shows it: the base's, and a shut lid's, which a bag heats as
 *  much. */
function skinStops(surface: number, fans: number[]) {
  const skin = clamp01((surface - 26) / 20)
  const airy = (side: number) => 1 - (fans[side] as number) * 0.45
  const weights: [number, number][] = [
    [0, airy(0) * 0.6],
    [0.16, airy(0) * 0.75],
    [0.356, 0.85],
    [0.5, 1],
    [0.644, 0.85],
    [0.84, airy(1) * 0.75],
    [1, airy(1) * 0.6],
  ]
  return weights.map(([offset, weight]) => {
    const heat = skin * weight
    return { offset, heat, alpha: 0.3 + heat * 0.7 }
  })
}

/** The skin (MacbookThermal.surface), the first level of the view: the
 *  base's face as a thermal camera sees the case, ice where it is cool and
 *  ember as it warms, hottest over the chip and the battery under it,
 *  coolest where the fans move air. Always drawn when shown, so a cooled case
 *  reads cool rather than blank. */
function MacbookSkin({ surface, fans }: { surface: number; fans: number[] }) {
  const stops = skinStops(surface, fans)
  const id = `device-skin-${stops.map((s) => Math.round(s.heat * 100)).join("-")}`
  return (
    <svg aria-hidden className="device-skin" viewBox="0 0 1000 37">
      <defs>
        <linearGradient
          id={id}
          gradientUnits="userSpaceOnUse"
          x1="0"
          y1="0"
          x2="1000"
          y2="0"
        >
          {stops.map((s) => (
            <stop
              key={`skin-${s.offset}`}
              offset={s.offset}
              stopColor={tone(s.heat)}
              stopOpacity={s.alpha}
            />
          ))}
        </linearGradient>
      </defs>
      <path
        d={SHELL}
        fill={`url(#${id})`}
        fillOpacity={0.3}
        stroke={`url(#${id})`}
      />
    </svg>
  )
}

/** The machine's insides as the front view sees them: a side elevation
 *  through the base's face (a 14″ MacBook Pro). It is a thin slab, so every
 *  part sits in one band and parts at different depths overlap where they
 *  share an x: the board and the chip at the back, the fans at the ends, the
 *  battery's cells translucent in front. Each is a hairline in the frame's own
 *  neutral so the aluminium keeps its tone.
 *
 *  The picture is how it cools, told plainly rather than to the millimetre.
 *  Each fan is a blower seen edge-on (its housing, its impeller drum with
 *  slats sweeping across on a cosine, near ones bright, far ones faint). The
 *  air is a fine, subtle fluid: each fan pushes it cool along the base toward
 *  the middle, over the components; it takes their heat (its colour warms
 *  over the battery, most over the chip) and leaves along the whole top at
 *  the hinge. Denser and quicker as a fan turns faster, still when it stops.
 *  The parts' own heat is a faint glow, never the subject. Drawn in the
 *  base's own units (1000 × 37, its 100 × 3.7 cqw), so nothing is stretched. */
function MacbookInternals({ thermal }: { thermal: MacbookThermal }) {
  const p = thermal.parts
  const h = {
    cpu: heat("cpu", p.cpu),
    gpu: heat("gpu", p.gpu),
    memory: heat("memory", p.memory),
    ssd: heat("ssd", p.ssd),
    battery: heat("battery", p.battery),
  }
  const chip = Math.max(h.cpu, h.gpu)
  const fans = fanShares(thermal)
  const rates = fans.map(fanTurnRate)
  const t = thermal.t
  const turns =
    t === undefined
      ? undefined
      : rates.map((rate, i) => thermal.turns?.[i] ?? rate * t)
  // Ids named by what they hold: frames on one page share a definition when
  // it is the same and never collide when it is not, with no hook (the frame
  // stays a server component).
  const id = `device-${tone(chip).replace(/\W/g, "")}`
  // The band every part shares, clear of the lip notch at the top centre
  // (its foot is at 13): the board along its foot, the chip on the board.
  const top = 15
  const bottom = 31
  const board = 29
  const glow = (cx: number, cy: number, rx: number, ry: number, v: number) =>
    v > 0 && (
      <ellipse
        key={`g${cx}-${cy}`}
        cx={cx}
        cy={cy}
        rx={rx}
        ry={ry}
        fill={tone(v)}
        opacity={0.04 + v * 0.26}
        filter={`url(#${id}-heat)`}
      />
    )
  // Inset from the corners, as a 14″'s are.
  const FAN = [160, 840]
  const SLATS = 18
  return (
    <span
      className="device-internals"
      data-held={t === undefined ? undefined : ""}
    >
      <svg aria-hidden className="device-internals-art" viewBox="0 0 1000 37">
        <defs>
          <filter id={`${id}-heat`} x="-1" y="-2" width="3" height="5">
            <feGaussianBlur stdDeviation="6" />
          </filter>
        </defs>

        {/* heat, under everything drawn */}
        {glow(500, 25, 60, 10, chip)}
        {glow(445, 26, 12, 8, h.memory)}
        {glow(555, 26, 12, 8, h.memory)}
        {glow(595, 27, 18, 7, h.ssd)}
        {glow(356, 23, 110, 7, h.battery)}
        {glow(644, 23, 110, 7, h.battery)}

        {/* the back: the chip (cpu | gpu), memory beside it, the ssd */}
        <g className="device-parts">
          <rect x={455} y={board - 8} width={90} height={8} rx={1} />
          <line x1={500} y1={board - 8} x2={500} y2={board} />
          <rect x={438} y={board - 6} width={14} height={6} rx={0.8} />
          <rect x={548} y={board - 6} width={14} height={6} rx={0.8} />
          <rect x={580} y={board - 3.5} width={30} height={3.5} rx={0.8} />
        </g>
        <g className="device-labels">
          <text x={477.5} y={board - 4}>
            cpu
          </text>
          <text x={522.5} y={board - 4}>
            gpu
          </text>
        </g>

        {/* each fan, a blower edge-on, its slats turning (the two mirror) */}
        {fans.map((fan, side) => {
          const cx = FAN[side] as number
          const dir = side === 0 ? 1 : -1
          // A still fan keeps a rate of 1 only so its pace stays defined; its
          // slats are paused.
          const motion = {
            "--rate": rates[side] || 1,
            "--turns": turns?.[side] ?? 0,
            "--dir": dir,
          } as React.CSSProperties
          return (
            <g key={`fan-${cx}`} style={motion}>
              <g className="device-parts">
                {/* the housing */}
                <rect x={cx - 72} y={17} width={144} height={14} rx={3} />
                {/* the drum's plates and its slats */}
                <line x1={cx - 58} y1={19} x2={cx + 58} y2={19} />
                <line x1={cx - 58} y1={29} x2={cx + 58} y2={29} />
                <g
                  className="device-fan"
                  data-still={fan === 0 ? "" : undefined}
                >
                  {Array.from({ length: SLATS }, (_, k) => k).map((k) => (
                    <line
                      key={`slat-${cx}-${k}`}
                      className="device-blade"
                      x1={cx}
                      y1={19}
                      x2={cx}
                      y2={29}
                      style={{ "--k": k / SLATS } as React.CSSProperties}
                    />
                  ))}
                </g>
              </g>
            </g>
          )
        })}

        {/* the front: the battery's cells, translucent over what is behind */}
        <g className="device-parts">
          {[245, 357, 531, 643].map((x) => (
            <rect
              key={`cell-${x}`}
              className="device-cell"
              x={x}
              y={top + 1}
              width={110}
              height={bottom - top - 1}
              rx={1.5}
            />
          ))}
        </g>
      </svg>
      {/* the air (device-airflow): particles carried in at the sides,
          through the fans, over the parts and out at the hinge */}
      <DeviceAirflow
        fans={fans}
        rates={rates}
        turns={turns}
        warmth={chip}
        t={t}
      />
    </span>
  )
}

/** A 16:9 desktop display with its own stand, independent of app content. */
export function StudioDisplayFrame({
  children,
  className,
  screenClassName,
  ...props
}: DesktopFrameProps) {
  return (
    <div
      {...props}
      data-device="studio"
      className={cn("device-desktop", className)}
    >
      <div className="device-lid">
        <div className={cn("device-display", screenClassName)}>{children}</div>
        <span aria-hidden className="device-camera" />
      </div>
      <div aria-hidden className="device-stand" />
      <div aria-hidden className="device-foot" />
    </div>
  )
}

/** Hardware only. App navigation and content belong to the child. */
export function IphoneFrame({
  children,
  bare = false,
  screenClassName,
  className,
  ...props
}: ComponentProps<"div"> & {
  bare?: boolean
  screenClassName?: string
  children?: ReactNode
}) {
  return (
    <div
      {...props}
      data-device={bare ? undefined : "iphone"}
      className={cn("device-frame", className)}
    >
      {!bare && (
        <>
          <span className="device-button action" />
          <span className="device-button vol-up" />
          <span className="device-button vol-down" />
          <span className="device-button power" />
          <span aria-hidden className="device-earpiece" />
          <span aria-hidden className="device-antenna left top" />
          <span aria-hidden className="device-antenna right top" />
          <span aria-hidden className="device-antenna left bottom" />
          <span aria-hidden className="device-antenna right bottom" />
        </>
      )}
      <div className={cn("device-screen", screenClassName)}>
        {!bare && <IphoneStatusBar />}
        {children}
      </div>
      {!bare && <span aria-hidden className="device-glass-edge" />}
    </div>
  )
}

function IphoneStatusBar() {
  return (
    <div aria-hidden="true">
      <div className="device-island">
        <span className="device-lens" />
      </div>
      <div className="device-status">
        <span>9:41</span>
        <span className="radios">
          <svg aria-hidden="true" viewBox="0 0 17 11" width="17" height="11">
            <g fill="currentColor">
              <rect x="0" y="7" width="3" height="4" rx="1" />
              <rect x="4.5" y="5" width="3" height="6" rx="1" />
              <rect x="9" y="2.5" width="3" height="8.5" rx="1" />
              <rect x="13.5" y="0" width="3" height="11" rx="1" />
            </g>
          </svg>
          <svg aria-hidden="true" viewBox="0 0 26 11" width="26" height="11">
            <rect
              x="0.6"
              y="0.6"
              width="21"
              height="9.8"
              rx="2.8"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.2"
              opacity="0.5"
            />
            <rect
              x="23.4"
              y="3.6"
              width="2"
              height="3.8"
              rx="1"
              fill="currentColor"
              opacity="0.5"
            />
            <rect
              x="2.2"
              y="2.2"
              width="14"
              height="6.6"
              rx="1.6"
              fill="currentColor"
            />
          </svg>
        </span>
      </div>
    </div>
  )
}
