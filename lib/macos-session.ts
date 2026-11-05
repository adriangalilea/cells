// The macos-session timeline: a menu bar app's story as steps, compiled by the APP from
// a scene script played through its real engine (awake's `awake-scene` is the model),
// and played here. The app owns every word, glyph and banner; this file owns pacing
// and nothing else. Framework-free by contract: `frameAt(timeline, ms)` is a pure fold,
// so scrubbing, stills and autoplay are the same function at different times.

import {
  type AgentEntry,
  type AgentFinished,
  type AgentWork,
  entryTokens,
  requestTokens,
} from "@/lib/agent-session"
import type { Chapter, Clip } from "@/lib/clip"
import { TYPE_MS } from "@/lib/terminal-session"
import {
  DEPTH_MS,
  type ThermalDepth,
  type ThermalKey,
  type ThermalReading,
  thermalShown,
} from "@/lib/thermal"

/** A menu row. A separator says only that; an item says only what is true of it. */
export interface MenuRow {
  title?: string
  separator?: true
  disabled?: true
  checked?: true
  /** The key equivalent drawn at the row's end (the app's toggle chord). */
  chord?: string
  submenu?: MenuRow[]
}

/** The Mac as the stage shows it. */
export interface WorldState {
  /** The time, "HH:MM", when the story needs one: a clock that jumps is time passing
   *  (a time-lapse behind the shut lid, an agent's hours). A story that never sets it
   *  shows the viewer's own time, and its story time never jumps. Set in some steps
   *  and not others is a broken scene. */
  clock?: string
  battery: number
  charging: boolean
  lid: "open" | "closed"
  asleep: boolean
  heat: boolean
  /** The lock screen is up: the Mac awake, nobody at it. */
  locked?: true
  /** The day the lock screen spells above the time, when the story needs one;
   *  absent, the viewer's own. */
  date?: string
}

export type StepKind =
  | "world"
  | "glyph"
  | "command"
  | "output"
  | "muted"
  | "menu"
  | "close"
  | "hover"
  | "press"
  | "key"
  | "right-click"
  | "banner"
  | "caption"
  | "poster" // the frame a still of the story shows; takes no time
  // The app's own surface (a popover) hanging from its glyph: `text` names it in
  // `Art.surfaces`, `arg` is the second of its clip it opens at. `close` shuts it.
  | "surface"
  // A coding agent in the terminal (drawn by a skin the page supplies):
  | "agent" // opens it: `text` its name, `arg` the working directory
  | "history" // a prompt already sent when the story starts
  | "prompt" // a prompt typed now
  | "say" // the agent's reply
  | "tool" // a tool call: `text` the tool, `arg` its argument, `lines` the result
  | "done" // it stops working and says `text`
  // The machine's heat (a MacBook only): `thermal` a reading the app's sensors gave,
  // eased into from the last; `depth` how far the view shows it (lib/thermal).
  | "thermal"

export interface Step {
  kind: StepKind
  /** The script did this (a command, a click, the lid, a caption); absent, the app
   *  did it in answer. The player waits for the viewer before an author step. */
  author?: true
  /** The author's pause before this step, ms. Absent: the kind's default. */
  delay?: number
  text?: string
  rows?: MenuRow[]
  /** Indices into the open menu, then its submenu. */
  path?: number[]
  glyph?: string
  tooltip?: string
  keys?: string
  world?: WorldState
  arg?: string
  lines?: string[]
  thermal?: ThermalReading
  depth?: ThermalDepth
}

export interface Timeline {
  app: string
  chord: string
  steps: Step[]
}

/** The app's own pixels, drawn by its own code: one PNG per glyph state per menu bar
 *  appearance, the icon its banners wear, and the surfaces it opens (a popover the app
 *  rendered as a clip, played in step with the story). */
export interface Art {
  icon: string
  glyphs: Record<string, { light: string; dark: string }>
  surfaces?: Record<string, Surface>
}

/** A surface's clip: its video, a still of its first frame, its size in points (the
 *  stage's units, so it hangs at its true size under the glyph), and the colour it is
 *  drawn on, which the arrow and the border around it wear too. */
export interface Surface {
  src: string
  poster: string
  width: number
  height: number
  background: string
}

/** A still of a surface's clip at one second: the picture, and the tiny picture drawn
 *  blurred under it from the first paint (ui/prepare-media's `blurDataURL`), when it
 *  was prepared ahead. A still cut on demand has no blur. */
export interface Still {
  src: string
  blurDataURL?: string
}

/** How long the pointer takes to travel to what it is about to click. */
export const POINTER_MS = 520
/** How long the camera takes to move between two shots. */
export const ZOOM_MS = 750
/** Zoom first, then the input: a chord or a right-click that flips the glyph waits
 *  for the camera to arrive on it, so the change happens in the close-up. */
const LEAD_MS = ZOOM_MS + 250

// ── pacing: the one place a step's time is decided ──
//
// People do not read in an instant, and a story that moves on before its detail lands
// has not shown it. So every step says what it asks of the viewer once it is on screen
// (LOOK: a line to read, a menu to scan, the lid to watch close), and before the next
// AUTHOR step (the script's own act: a command, a click, the lid, a caption) the
// player waits until all of it has been taken in. A reaction (output, a banner, the
// glyph) follows its cause at once: the app answers, it does not deliberate. PAUSE is
// only the breath between steps; an author's `@ms` replaces it.

const PAUSE: Record<StepKind, number> = {
  world: 400,
  glyph: 120,
  command: 400,
  output: 0,
  muted: 0,
  // A step the pointer performs gives it time to get there first.
  menu: POINTER_MS + 150,
  close: 300,
  hover: POINTER_MS + 100,
  press: 350,
  key: LEAD_MS,
  "right-click": Math.max(POINTER_MS + 150, LEAD_MS),
  banner: 300,
  caption: 250,
  poster: 0,
  surface: 300,
  agent: 0,
  history: 0,
  prompt: 400,
  say: 500,
  tool: 400,
  done: 400,
  // A reading is the machine answering; time between readings is the author's `@ms`.
  thermal: 0,
}

/** How long a tool call shows as running before its result comes back. */
export const TOOL_MS = 700

/** How long a pressed row flashes before the menu closes, as NSMenu does. */
export const PRESS_MS = 220
/** How long a keycap chip stays up. */
export const KEY_MS = 1600
/** How long the lid takes to fold, and the panel to go dark with it. */
export const LID_MS = 1100
/** How long the lock screen takes to come up, or to go. */
export const LOCK_MS = 700
/** How long the camera holds on the glyph after it changes. */
export const GLYPH_FOCUS_MS = 2400
/** Output lands a beat after the previous line. */
const LAND_MS = 140
/** The opening frame, held before the story's first act. */
const ESTABLISH_MS = 1600

/** Time to notice a change and read `text` at a steady adult pace (about 210 words a
 *  minute), in ms. */
export function readMs(text: string): number {
  const words = text.trim().split(/\s+/).filter(Boolean).length
  return 700 + words * 285
}

/** How long a banner stays: long enough to read, and a beat. A story never
 *  holds still longer than its words need. */
export function bannerStay(text: string): number {
  return readMs(text) + 600
}

const hold = (s: Step) =>
  s.kind === "command" || s.kind === "prompt"
    ? (s.text?.length ?? 0) * TYPE_MS + LAND_MS
    : s.kind === "output" || s.kind === "muted"
      ? LAND_MS
      : s.kind === "press"
        ? PRESS_MS
        : s.kind === "tool"
          ? TOOL_MS
          : 0

/** What a step asks of the viewer once it has landed, ms. */
function look(s: Step, before: WorldState, glyph: string): number {
  const text = s.text ?? ""
  switch (s.kind) {
    case "world": {
      const w = s.world as WorldState
      if (w.lid !== before.lid) return LID_MS + 1200
      if (w.clock !== before.clock || w.battery !== before.battery) return 1800
      if (w.asleep !== before.asleep) return 1400
      if (w.locked !== before.locked) return LOCK_MS + 1400
      return 0
    }
    case "glyph":
      // Behind the lock screen the menu bar is not there to look at.
      return s.glyph === glyph || before.locked ? 0 : GLYPH_FOCUS_MS
    case "command":
      return 300
    case "output":
      return readMs(text)
    case "muted":
      return readMs(text) / 2
    case "menu":
      // The camera closes in, then the eye walks the rows.
      return 1400 + (s.rows?.filter((r) => !r.separator).length ?? 0) * 110
    case "hover":
      return 900
    case "key":
      return KEY_MS
    case "right-click":
      return 700
    case "banner":
      return readMs(text) + 600
    case "caption":
      return readMs(text)
    case "say":
    case "done":
      return readMs(text)
    case "tool":
      return readMs(`${s.arg ?? ""} ${(s.lines ?? []).join(" ")}`) / 2
    case "thermal":
      // A view going deeper or back is watched; a reading is taken in at a glance.
      return s.depth === undefined ? 0 : DEPTH_MS + 600
    default:
      return 0
  }
}

const READ_IN_TURN = new Set<StepKind>([
  "output",
  "muted",
  "banner",
  "caption",
  "say",
  "tool",
  "done",
])

export interface SceneClock {
  /** When each step starts (after its pause), ms. */
  starts: number[]
  /** When each step is complete. */
  ends: number[]
  /** When the viewer turns to each step: once it has landed and whatever was being
   *  read before it has been read. The camera goes to a glyph then, not before. */
  looks: number[]
  /** What happened behind a shut lid is seen when it opens, in order: first the
   *  work the agent did in the dark is read, then each banner that arrived
   *  meanwhile, from `deferred[banner step]`. */
  deferred: Record<number, number>
  /** How many steps the opening frame is: the first `opening` steps land together,
   *  as one picture, at 0. */
  opening: number
  /** The whole story, until the last thing on screen has been taken in. */
  total: number
  /** The story as a clip, for any driver: its length, and one chapter per caption
   *  titled with its words. The caption line shows the chapter the moment it
   *  begins, so a player's timeline and the stage always name the same chapter. */
  clip: Clip
}

/** An agent's words: read in turn, or, behind a shut lid, when it opens. */
const AGENT_WORDS = new Set<StepKind>(["say", "tool", "done"])

/** The opening frame: the world, the glyph, an agent already at work, a surface
 *  already open. It is on screen from the first instant, then held before the
 *  story's first act. */
const OPENING = new Set<StepKind>([
  "world",
  "glyph",
  "agent",
  "history",
  "surface",
  "thermal",
])

/** Where the story's `poster` step lands, as the stage's `progress` (0..1): the one
 *  frame a still shows (a link card, a shelf). The scene chooses it; a story
 *  without one screams rather than posing an arbitrary frame. */
export function posterAt(timeline: Timeline): number {
  const i = timeline.steps.findIndex((s) => s.kind === "poster")
  if (i < 0) throw new Error("posterAt: the scene marks no `poster` frame")
  const clock = sceneClock(timeline)
  return (clock.starts[i] as number) / clock.total
}

/** A surface drawn this close to its clip's start stands on the clip's own poster (its
 *  first frame), seconds: within the tolerance ui/video allows a followed instant. */
export const CLIP_START = 0.15

/** The second of a surface's clip a popover stands on when it is first drawn at scene
 *  time `ms`: where it opened plus the scene time since, to the hundredth. The one
 *  rounding the stage and a still's preparation share, so a prepared still is found
 *  under the very second the stage asks for. */
export function surfaceSecond(
  open: { from: number; since: number },
  ms: number,
): number {
  return Math.round((open.from + (ms - open.since) / 1000) * 100) / 100
}

/** The stills a stage resting on its poster frame asks for (a studio card, a link
 *  card): each surface open there, at the second it stands on, past its clip's start
 *  (that one is the clip's poster). Known before any page is drawn, so a preparer
 *  makes them ahead (ui/prepare-media's `posterTime`) and the stage is handed them
 *  through `still` instead of a picture cut on demand. */
export function restingStills(
  timeline: Timeline,
  art: Art,
): { src: string; second: number }[] {
  const clock = sceneClock(timeline)
  const ms = posterAt(timeline) * clock.total
  const open = frameAt(timeline, clock, ms).surface
  if (!open) return []
  const surface = art.surfaces?.[open.name]
  if (!surface)
    throw new Error(`restingStills: no surface "${open.name}" in art.surfaces`)
  const second = surfaceSecond(open, ms)
  return second <= CLIP_START ? [] : [{ src: surface.src, second }]
}

export function sceneClock(timeline: Timeline): SceneClock {
  let at = 0
  // When the viewer is done with everything on screen so far.
  let ready = 0
  // The same, but for the camera's close-up on a changed glyph: a caption sits under
  // the picture, not in it, so it can be read while the camera holds.
  let readyButGlyph = 0
  // The opening world is where the story starts, not a change to watch.
  const first = timeline.steps[0]
  let world = first?.kind === "world" ? (first.world as WorldState) : NIGHT
  // What happens behind a shut lid is owed to the viewer when it opens.
  let dark = { agent: 0, banners: [] as [number, number][] }
  const deferred: Record<number, number> = {}
  let glyph = "off"
  let opening = true
  let openingSteps = timeline.steps.length
  // Where the current chapter began: the step after the last caption.
  let chapterFrom = 0
  let chapterNext = false
  const starts: number[] = []
  const ends: number[] = []
  const looks: number[] = []
  for (const [i, s] of timeline.steps.entries()) {
    // The opening frame lands at once and is taken in before anything happens:
    // where we are, what is on screen.
    // A step the author paused before is not part of the opening frame, whatever
    // its kind: a glyph that flips a second in happens a second in.
    if (opening && (!OPENING.has(s.kind) || s.delay !== undefined)) {
      opening = false
      openingSteps = i
      ready = Math.max(ready, ESTABLISH_MS)
      readyButGlyph = ready
    }
    // What the agent does behind a shut lid takes no screen time: nobody sees it
    // happen, it is read when the lid opens.
    const inDark = world.lid === "closed" && AGENT_WORDS.has(s.kind)
    if (!opening && !inDark) {
      at += s.delay ?? PAUSE[s.kind]
      // An input that flips the glyph also waits for the camera, which only
      // leaves once the last beat has been taken in.
      if (s.author)
        at = Math.max(
          at,
          s.kind === "caption"
            ? readyButGlyph
            : ready +
                (s.kind === "key" || s.kind === "right-click" ? LEAD_MS : 0),
        )
    }
    starts.push(at)
    if (chapterNext) {
      chapterFrom = at
      chapterNext = false
    }
    const landed = inDark ? at : at + hold(s)
    ends.push(landed)
    // A caption is on screen from its chapter's start (frameAt), so all it is
    // owed is the reading its chapter has not already given it.
    const seen = opening
      ? 0
      : s.kind === "caption"
        ? Math.max(0, look(s, world, glyph) - (landed - chapterFrom))
        : look(s, world, glyph)
    if (s.kind === "caption") chapterNext = true
    if (s.kind === "glyph") glyph = s.glyph as string
    // Words are read one line after another, and a changed glyph is turned to once
    // they are read; a world change is taken in at a glance, alongside.
    const inTurn = READ_IN_TURN.has(s.kind) || s.kind === "glyph"
    looks.push(inTurn ? Math.max(ready, landed) : landed)
    const shut = world.lid === "closed"
    if (shut && s.kind === "banner") dark.banners.push([i, seen])
    else if (shut && AGENT_WORDS.has(s.kind)) dark.agent += seen
    else {
      const after = (r: number) =>
        inTurn ? Math.max(r, landed) + seen : Math.max(r, landed + seen)
      if (s.kind === "caption") {
        // Read under the picture while the camera holds its close-up, not after.
        readyButGlyph = after(readyButGlyph)
        ready = Math.max(ready, readyButGlyph)
      } else {
        ready = after(ready)
        if (s.kind !== "glyph") readyButGlyph = after(readyButGlyph)
      }
    }
    if (s.kind === "world") {
      const w = s.world as WorldState
      if (w.lid === "open" && shut) {
        // The lid swings open, the work done in the dark is read, then the
        // banners that arrived meanwhile, one after another.
        let t = landed + LID_MS + dark.agent
        for (const [b, look] of dark.banners) {
          deferred[b] = t
          t += look
        }
        ready = Math.max(ready, t)
        readyButGlyph = Math.max(readyButGlyph, t)
        dark = { agent: 0, banners: [] }
      }
      world = w
    }
    at = landed
  }
  const total = Math.max(1, at, ready) + 800
  // Each caption titles one chapter, which starts where the thing it tells begins:
  // the step after the previous caption (the first at 0).
  const chapters: Chapter[] = []
  let from = 0
  timeline.steps.forEach((s, i) => {
    if (s.kind !== "caption" || !s.text) return
    chapters.push({ start: from, title: s.text })
    from = starts[i + 1] ?? total
  })
  return {
    starts,
    ends,
    looks,
    deferred,
    opening: openingSteps,
    total,
    clip: { duration: total, chapters },
  }
}

/** How far the lid has travelled toward where it is going, 0..1, eased in and out
 *  like a hand closing it: every motion on stage is a function of scene time, so a
 *  scrubbed frame, a played one and a captured one are the same frame. */
export function lidTravel(frame: Frame, ms: number): number {
  const t = Math.min(1, Math.max(0, (ms - frame.lidSince) / LID_MS))
  return t < 0.5 ? 4 * t ** 3 : 1 - (-2 * t + 2) ** 3 / 2
}

// ── the fold: a timeline at one instant ──

export interface TerminalLine {
  kind: "command" | "output" | "muted"
  text: string
}

export interface Frame {
  world: WorldState
  /** When the lid last moved (ms): its fold and the panel's fade run from here. */
  lidSince: number
  /** When the lock screen last came up or went (ms): its fade runs from here. */
  lockSince: number
  /** The app's surface hanging from its glyph: which, the second of its clip it
   *  opened at, and when (ms); the clip plays on from there with scene time. */
  surface: { name: string; from: number; since: number } | null
  glyph: string
  tooltip: string
  terminal: TerminalLine[]
  /** The command being typed right now, and how much of it is typed. */
  typing: { text: string; chars: number } | null
  menu: {
    /** The step that opened it: a row's pointer target is scoped to its menu. */
    step: number
    rows: MenuRow[]
    hover: number[] | null
    pressed: number[] | null
  } | null
  /** The chord on screen as keycaps, and since when (ms). */
  key: { keys: string; since: number } | null
  /** The last mouse click (left opens the menu and picks a row, right on the glyph
   *  is the toggle), and since when: shown as the pressed button while KEY_MS. */
  click: { button: "left" | "right"; since: number } | null
  banner: { text: string; since: number } | null
  caption: string | null
  /** Index of the last step that has started. */
  step: number
  /** When the glyph last changed (ms): the camera holds on it for GLYPH_FOCUS_MS. */
  glyphSince: number
  /** When the current caption appeared: it fades in from here. */
  captionSince: number
  /** A coding agent in the terminal, when the story opened one: its transcript so
   *  far, what is being typed, and its work (the elapsed time is the world's). */
  agent: {
    name: string
    cwd: string
    entries: AgentEntry[]
    draft: string
    work: AgentWork | null
    finished: AgentFinished | null
  } | null
  /** The last jump of the world's clock or battery, and when (ms): shown as a
   *  time-lapse (`lapseAt`), because time passing is the story behind a shut lid. */
  lapse: { from: WorldState; to: WorldState; since: number } | null
  /** Critical heat, once raised: when it shows (ms, after the time-lapse it came
   *  with) and the world's clock then. An alert, so it stays while the lid it was
   *  raised under stays shut: the Mac cools, the warning it gave does not unsay
   *  itself, and it is why a banner waits when the lid opens. Opening the lid is
   *  the person meeting it; the next close is a new stretch in the dark, and a
   *  later sleep (the battery floor) must not wear the old heat as its reason. */
  heatAlert: { since: number; clock: string | undefined } | null
}

/** How long a jump of the world's clock takes to run on screen. */
export const LAPSE_MS = 1600

/** The world's clock and battery as a time-lapse shows them at `ms`: running from
 *  the last jump's start to its end, and how much time it covered. A story without
 *  a clock has no time to lapse: `clock` is absent and only the battery runs. */
export function lapseAt(
  frame: Frame,
  ms: number,
): { clock: string | undefined; battery: number; gained: string | null } {
  const l = frame.lapse
  if (!l)
    return {
      clock: frame.world.clock,
      battery: frame.world.battery,
      gained: null,
    }
  const p = Math.min(1, Math.max(0, (ms - l.since) / LAPSE_MS))
  const e = p < 0.5 ? 4 * p ** 3 : 1 - (-2 * p + 2) ** 3 / 2
  const battery = Math.round(
    l.from.battery + (l.to.battery - l.from.battery) * e,
  )
  const span = minutesBetween(l.from.clock, l.to.clock)
  if (l.from.clock === undefined)
    return { clock: undefined, battery, gained: null }
  const [h, m] = l.from.clock.split(":").map(Number)
  // The time covered so far counts up with the clock.
  const run = Math.round(span * e)
  const now = ((h ?? 0) * 60 + (m ?? 0) + run) % 1440
  const gained =
    span === 0
      ? null
      : `+${run >= 60 ? `${Math.floor(run / 60)}h ` : ""}${run % 60}m`
  return {
    clock: `${String(Math.floor(now / 60)).padStart(2, "0")}:${String(now % 60).padStart(2, "0")}`,
    battery,
    gained,
  }
}

const NIGHT: WorldState = {
  battery: 80,
  charging: false,
  lid: "open",
  asleep: false,
  heat: false,
}

const clicked = (button: "left" | "right", since: number, ms: number) =>
  ms < since + KEY_MS ? { button, since } : null

/** The whole stage at `ms`. Pure: no clock of its own, no memory between calls. */
export function frameAt(
  timeline: Timeline,
  clock: SceneClock,
  ms: number,
): Frame {
  const f: Frame = {
    world: NIGHT,
    lidSince: Number.NEGATIVE_INFINITY,
    lockSince: Number.NEGATIVE_INFINITY,
    surface: null,
    glyph: "off",
    tooltip: "",
    terminal: [],
    typing: null,
    menu: null,
    key: null,
    click: null,
    banner: null,
    caption: null,
    step: -1,
    glyphSince: Number.NEGATIVE_INFINITY,
    captionSince: Number.NEGATIVE_INFINITY,
    agent: null,
    lapse: null,
    heatAlert: null,
  }
  // The agent's work, and when it began in scene time and on the world's clock: its
  // elapsed time is the world's, so two hours behind a shut lid read as two hours.
  let work = null as { since: number; clock: string | undefined } | null
  // The request in flight: the model is asked again once each reply or tool result
  // lands, and the new request starts by thinking.
  let request = null as { since: number; clock: string | undefined } | null
  const add = (e: AgentEntry, landed: number) => {
    if (!f.agent) return
    f.agent.entries.push(e)
    request = { since: landed, clock: f.world.clock }
  }
  timeline.steps.forEach((s, i) => {
    const start = clock.starts[i] as number
    if (start > ms) return
    const end = clock.ends[i] as number
    f.step = i
    switch (s.kind) {
      case "world": {
        const w = s.world as WorldState
        // The opening world is where the story starts, not a jump from anything.
        if (
          i > 0 &&
          (w.clock !== f.world.clock || w.battery !== f.world.battery)
        )
          f.lapse = { from: f.world, to: w, since: start }
        if (w.lid === "closed" && f.world.lid === "open") f.heatAlert = null
        if (w.heat && !f.heatAlert)
          f.heatAlert = {
            since: f.lapse?.since === start ? start + LAPSE_MS : start,
            clock: w.clock,
          }
        if (w.lid !== f.world.lid) f.lidSince = start
        if (w.locked !== f.world.locked) f.lockSince = start
        // Locking closes whatever the app had open, as macOS does.
        if (w.locked) {
          f.menu = null
          f.surface = null
        }
        f.world = w
        break
      }
      case "surface":
        f.surface = {
          name: s.text ?? "",
          from: Number(s.arg ?? 0),
          since: start,
        }
        // Opened by the author is a click on the glyph; open from the first
        // frame, it was already up.
        if (s.author) f.click = clicked("left", start, ms)
        break
      case "glyph":
        if (s.glyph !== f.glyph) f.glyphSince = start
        f.glyph = s.glyph as string
        f.tooltip = s.tooltip ?? ""
        break
      case "command":
        if (ms < end) {
          f.typing = {
            text: s.text ?? "",
            chars: Math.floor((ms - start) / TYPE_MS),
          }
        } else {
          f.typing = null
          f.terminal.push({ kind: "command", text: s.text ?? "" })
        }
        break
      case "output":
      case "muted":
        f.terminal.push({ kind: s.kind, text: s.text ?? "" })
        break
      case "menu":
        f.menu = { step: i, rows: s.rows ?? [], hover: null, pressed: null }
        f.click = clicked("left", start, ms)
        break
      case "hover":
        if (f.menu) f.menu.hover = s.path ?? null
        break
      case "press":
        if (f.menu) {
          f.menu.hover = s.path ?? null
          f.menu.pressed = s.path ?? null
        }
        f.click = clicked("left", start, ms)
        if (ms >= end) f.menu = null
        break
      case "close":
        f.menu = null
        f.surface = null
        break
      case "key":
        f.key =
          ms < start + KEY_MS ? { keys: s.keys ?? "", since: start } : null
        break
      case "right-click":
        f.click = clicked("right", start, ms)
        break
      case "banner": {
        // A banner that arrived behind the shut lid shows once the lid is open and
        // the work done in the dark has been read (clock.deferred); one that
        // arrived with the lid open shows at once.
        const text = s.text ?? ""
        const since =
          clock.deferred[i] ?? (f.world.lid === "open" ? start : null)
        if (since !== null && ms >= since && ms < since + bannerStay(text))
          f.banner = { text, since }
        break
      }
      case "caption":
        // Shown from its chapter's start, below.
        break
      case "agent":
        f.agent = {
          name: s.text ?? "",
          cwd: s.arg ?? "~",
          entries: [],
          draft: "",
          work: null,
          finished: null,
        }
        work = null
        break
      // A prompt sent is the agent at work, until it is done: the CLI shows its
      // working line from that instant, across every reply and tool call.
      case "history":
        if (!f.agent) break
        f.agent.entries.push({ kind: "prompt", text: s.text ?? "" })
        f.agent.finished = null
        work = { since: start, clock: f.world.clock }
        request = work
        break
      case "prompt":
        if (!f.agent) break
        if (ms < end)
          f.agent.draft = (s.text ?? "").slice(
            0,
            Math.floor((ms - start) / TYPE_MS),
          )
        else {
          f.agent.draft = ""
          f.agent.entries.push({ kind: "prompt", text: s.text ?? "" })
          f.agent.finished = null
          work = { since: end, clock: f.world.clock }
          request = work
        }
        break
      case "say":
        add({ kind: "say", text: s.text ?? "" }, start)
        break
      case "tool":
        // The next request starts once the result is back, not while the tool runs.
        add(
          {
            kind: "tool",
            name: s.text ?? "",
            arg: s.arg ?? "",
            result: ms < end ? [] : (s.lines ?? []),
          },
          Math.min(ms, end),
        )
        break
      case "done":
        add({ kind: "say", text: s.text ?? "" }, start)
        if (f.agent && work)
          f.agent.finished = { seconds: worked(work, f.world, start) }
        work = null
        request = null
        break
    }
  })
  // The caption is the chapter the story is in, from the moment it begins.
  const chapter = clock.clip.chapters.findLast((c) => ms >= c.start)
  if (chapter) {
    f.caption = chapter.title
    f.captionSince = chapter.start
  }
  if (f.agent && work && request) {
    const entries = f.agent.entries
    const turn = entries.slice(
      entries.findLastIndex((e) => e.kind === "prompt") + 1,
    )
    const inFlight = worked(request, f.world, ms)
    f.agent.work = {
      seconds: worked(work, f.world, ms),
      request: inFlight,
      tokens:
        turn.reduce((n, e) => n + entryTokens(e), 0) + requestTokens(inFlight),
      ms: ms - work.since,
    }
  }
  return f
}

/** How long the agent has worked at scene time `ms`, in the world's seconds: two
 *  hours behind a shut lid count as two hours. */
function worked(
  work: { since: number; clock: string | undefined },
  world: WorldState,
  ms: number,
): number {
  return minutesBetween(work.clock, world.clock) * 60 + (ms - work.since) / 1000
}

/** Minutes from one "HH:MM" to the next, across midnight. A story without a clock
 *  has none passing; a clock in one step and not the other is a broken scene. */
function minutesBetween(
  from: string | undefined,
  to: string | undefined,
): number {
  if (from === undefined && to === undefined) return 0
  if (from === undefined || to === undefined)
    throw new Error(
      `macos-session: a clock in one step and not another (${from} → ${to}): set it in every world step or in none`,
    )
  const m = (t: string) => {
    const [h, mm] = t.split(":").map(Number)
    return (h ?? 0) * 60 + (mm ?? 0)
  }
  return (m(to) - m(from) + 1440) % 1440
}

/** The story's thermal track (lib/thermal): each `thermal` step keyed where it
 *  starts. A reading that lands with a jump of the world's clock (time passing
 *  behind a shut lid) runs with its time-lapse, as the clock and the battery do:
 *  held where it was as the lapse starts, there as it ends. Empty for a story
 *  that never shows heat. */
export function thermalKeys(
  timeline: Timeline,
  clock: SceneClock,
): ThermalKey[] {
  const keys: ThermalKey[] = []
  let world: WorldState | null = null
  let lapsed: number | null = null
  let last: ThermalReading | undefined
  timeline.steps.forEach((s, i) => {
    const at = clock.starts[i] as number
    if (s.kind === "world") {
      const w = s.world as WorldState
      if (world && (w.clock !== world.clock || w.battery !== world.battery))
        lapsed = at
      world = w
      return
    }
    if (s.kind !== "thermal") return
    if (s.thermal && last && lapsed === at) {
      keys.push({ at, reading: last, depth: s.depth })
      keys.push({ at: at + LAPSE_MS, reading: s.thermal })
    } else keys.push({ at, reading: s.thermal, depth: s.depth })
    if (s.thermal) last = s.thermal
  })
  // A lapse's landing can fall after a step that follows it.
  return keys.sort((a, b) => a.at - b.at)
}

// ── focus: what the camera is about, over time ──
//
// A menu bar app lives in a 24pt strip, so the camera closes in on it while there is
// something there to see: an open menu (a hovered submenu is its own focus, it
// changes the menu's size), a banner, a glyph that just changed. Spans cover the
// whole story in order, "none" between, so the camera is a pure function of time:
// every frame can be drawn alone, in any order, by any renderer.

export type FocusKind = "menu" | "banner" | "glyph"

export interface FocusSpan {
  /** Stable per thing in focus; "none" is the wide shot. */
  key: string
  kind: FocusKind | null
  start: number
  end: number
}

const RANK: Record<FocusKind, number> = { menu: 3, banner: 2, glyph: 1 }

export function focusSpans(timeline: Timeline, clock: SceneClock): FocusSpan[] {
  const wants: { key: string; kind: FocusKind; start: number; end: number }[] =
    []
  let menu: { i: number; hover: string; since: number } | null = null
  const endMenu = (at: number) => {
    if (menu)
      wants.push({
        key: `menu:${menu.i}:${menu.hover}`,
        kind: "menu",
        start: menu.since,
        end: at,
      })
    menu = null
  }
  let lid: WorldState["lid"] = "open"
  let glyph = "off"
  // An open surface is the thing to see, and it hangs below the glyph: a close-up
  // on the glyph would crop it, so the camera stays wide while one is open. Behind
  // the lock screen there is no glyph to close in on. The opening frame is one
  // picture, so a surface open in it is open from its first instant, before any
  // glyph that frame also sets.
  let surface = timeline.steps
    .slice(0, clock.opening)
    .some((s) => s.kind === "surface")
  let locked = false
  // The chord or right-click whose answer is the next glyph: the camera leaves for
  // the glyph LEAD_MS before it lands, so the change happens in the close-up.
  let input: number | null = null
  timeline.steps.forEach((s, i) => {
    const start = clock.starts[i] as number
    const end = clock.ends[i] as number
    if (s.kind === "key" || s.kind === "right-click") input = start
    else if (s.author) input = null
    switch (s.kind) {
      case "menu":
        endMenu(start)
        menu = { i, hover: "", since: start }
        break
      case "hover":
        if (menu) {
          const m = menu
          endMenu(start)
          menu = { i: m.i, hover: (s.path ?? []).join("."), since: start }
        }
        break
      case "press":
        endMenu(end)
        break
      case "close":
        endMenu(start)
        surface = false
        break
      case "surface":
        surface = true
        break
      case "glyph":
        if (s.glyph !== glyph && !surface && !locked)
          wants.push({
            key: `glyph:${i}`,
            kind: "glyph",
            // Zoom first, then the input; otherwise when the viewer turns to it,
            // once the lines that caused it have been read.
            start:
              input === null
                ? (clock.looks[i] as number)
                : Math.max(0, input - LEAD_MS),
            end:
              (input === null ? (clock.looks[i] as number) : start) +
              GLYPH_FOCUS_MS,
          })
        glyph = s.glyph as string
        input = null
        break
      case "banner": {
        // When the frame shows it: at once, or (behind a shut lid) once it opens
        // and the work done in the dark has been read.
        const since = clock.deferred[i] ?? (lid === "open" ? start : null)
        if (since !== null)
          wants.push({
            key: `banner:${i}`,
            kind: "banner",
            start: since,
            end: since + bannerStay(s.text ?? ""),
          })
        break
      }
      case "world":
        lid = (s.world as WorldState).lid
        locked = (s.world as WorldState).locked === true
        if (locked) surface = false
        break
    }
  })
  endMenu(clock.total)
  // The thermal view is drawn in the machine's base: while it shows, the camera
  // holds the whole machine, pulling out as the view comes in and closing in
  // again only as it goes.
  const wide = thermalShown(thermalKeys(timeline, clock))
  // Between every pair of edges, the highest-ranked want (the latest, on a tie).
  const edges = [
    ...new Set([
      0,
      clock.total,
      ...wants.flatMap((w) => [w.start, w.end]),
      ...wide.flat(),
    ]),
  ]
    .filter((t) => t >= 0 && t <= clock.total)
    .sort((a, b) => a - b)
  const spans: FocusSpan[] = []
  for (let k = 0; k + 1 < edges.length; k++) {
    const a = edges[k] as number
    const b = edges[k + 1] as number
    let best: (typeof wants)[number] | undefined
    const held = wide.some(([from, to]) => from <= a && to > a)
    for (const w of held ? [] : wants)
      if (w.start <= a && w.end > a)
        if (
          !best ||
          RANK[w.kind] > RANK[best.kind] ||
          (RANK[w.kind] === RANK[best.kind] && w.start >= best.start)
        )
          best = w
    const key = best?.key ?? "none"
    const last = spans.at(-1)
    if (last && last.key === key) last.end = b
    else spans.push({ key, kind: best?.kind ?? null, start: a, end: b })
  }
  return spans
}

/** The camera's zoom at `ms`, given each span's target (`target(key)`, 1 for the
 *  wide shot): eased from where the previous span left it, so a span shorter than
 *  a move hands on its unfinished position, never a jump. */
export function zoomAt(
  spans: FocusSpan[],
  ms: number,
  target: (key: string) => number,
): number {
  const at = (i: number, t: number): number => {
    const span = spans[i]
    if (!span) return 1
    const from = i === 0 ? 1 : at(i - 1, span.start)
    const to = span.key === "none" ? 1 : target(span.key)
    const p = Math.min(1, Math.max(0, (t - span.start) / ZOOM_MS))
    const e = p < 0.5 ? 4 * p ** 3 : 1 - (-2 * p + 2) ** 3 / 2
    return from + (to - from) * e
  }
  let i = spans.findIndex((s) => ms >= s.start && ms < s.end)
  if (i < 0) i = spans.length - 1
  return at(i, ms)
}

// ── the pointer: where the mouse is going, over time ──
//
// Every click the story makes is made by a visible pointer that travels there first:
// to the glyph for the menu and the right-click toggle, to a row for a hover or a
// pick. Targets are named, not placed: "glyph", or "row:<menu step>:<path>"; the
// renderer measures where a name is on its own stage.

export interface PointerTarget {
  key: string
  /** When the pointer is there (the click, or the hover). It sets off POINTER_MS
   *  earlier. */
  arrive: number
}

export function pointerTargets(
  timeline: Timeline,
  clock: SceneClock,
): PointerTarget[] {
  const targets: PointerTarget[] = []
  let menu = -1
  timeline.steps.forEach((s, i) => {
    const arrive = clock.starts[i] as number
    const key =
      s.kind === "menu" ||
      s.kind === "right-click" ||
      (s.kind === "surface" && s.author)
        ? "glyph"
        : (s.kind === "hover" || s.kind === "press") && menu >= 0
          ? `row:${menu}:${(s.path ?? []).join(".")}`
          : null
    if (s.kind === "menu") menu = i
    if (key && targets.at(-1)?.key !== key) targets.push({ key, arrive })
  })
  return targets
}

/** The pointer's position at `ms`, from each target's measured position (`at`) and
 *  where it rests before its first trip: eased from the previous target, arriving
 *  exactly when the click lands. */
export function pointerAt(
  targets: PointerTarget[],
  ms: number,
  at: (key: string) => { x: number; y: number } | undefined,
  rest: { x: number; y: number },
): { x: number; y: number } {
  let i = -1
  while (
    i + 1 < targets.length &&
    ms >= (targets[i + 1] as PointerTarget).arrive - POINTER_MS
  )
    i++
  if (i < 0) return rest
  const t = targets[i] as PointerTarget
  const from =
    i === 0 ? rest : (at((targets[i - 1] as PointerTarget).key) ?? rest)
  const to = at(t.key) ?? from
  const p = Math.min(
    1,
    Math.max(0, (ms - (t.arrive - POINTER_MS)) / POINTER_MS),
  )
  const e = p < 0.5 ? 4 * p ** 3 : 1 - (-2 * p + 2) ** 3 / 2
  return { x: from.x + (to.x - from.x) * e, y: from.y + (to.y - from.y) * e }
}

/** The row a path points at in an open menu. */
export function rowAt(rows: MenuRow[], path: number[]): MenuRow | undefined {
  const top = rows[path[0] as number]
  return path.length === 1 ? top : top?.submenu?.[path[1] as number]
}

// Runnable example: scripts/examples/macos-session.ts (the lib carries no runtime).
