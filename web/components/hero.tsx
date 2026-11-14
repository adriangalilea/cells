"use client"

// THE HERO: a MacBook at night, someone at work on one file, spending.cells, open twice:
// in Cells on the left, and on the right in a terminal running `cells chart spending.cells
// 1 4 --watch`. A dinner is typed into the app with a zero too many and the terminal draws
// May over its budget; typed back, May is under again (lib/hero, ui's table-session; the
// terminal's screens the real program's, lib/hero-screens). One clock: `Playback` plays it
// in view and round again; the app's window draws the session's frame at the hero's
// moment, the hand (its cursor, what it types) in it, and the terminal the screen drawn
// for the file that frame holds.
//
// As the reader scrolls past it, the Cells window lifts out of the screen toward them and
// lands as the page's first block (`Landing`), in one move, while the lid folds shut
// behind it (the Mac's own lid, its timeline's last step, driven by the scroll). Reduced
// motion: the Mac shut and the window already in place. A phone stacks the two windows and
// nothing flies.

import * as React from "react"
import { createPortal } from "react-dom"
import { CellsWindow } from "@/components/cells-window"
import { ChartTerminal } from "@/components/chart-terminal"
import { Macos } from "@/components/ui/macos"
import { Playback, Playhead, usePlayhead } from "@/components/ui/playback"
import { useEngine } from "@/components/use-engine"
import { DINNER, hero, SPENDING_PATH, WATCH_COMMAND } from "@/lib/hero"
import { type Art, LID_MS, sceneClock, type Timeline } from "@/lib/macos-session"
import { PRODUCT } from "@/lib/product"
import { sceneOf, tableFrameAt } from "@/lib/table-session"

/** The Mac's own story: open from the first frame, the same layout for the whole hero (its
 *  windows change only their contents), and shut only by the scroll. */
const WORLD = { battery: 82, charging: false, lid: "open" as const, asleep: false, heat: false }
const MAC: Timeline = {
  app: PRODUCT.name,
  chord: "",
  steps: [
    { kind: "world", world: WORLD },
    { kind: "world", world: { ...WORLD, lid: "closed" }, delay: 1000 },
  ],
}
const MAC_CLOCK = sceneClock(MAC)
const SHUT_FROM = MAC_CLOCK.starts[1] as number
if (MAC_CLOCK.total < SHUT_FROM + LID_MS) throw new Error("hero: the Mac's story ends before its lid has shut")
const NO_ART: Art = { icon: "", glyphs: {} }

/** The hero, its commits as the engine writes them. */
const useHero = () => hero(useEngine())

/** The site's cold edge light: the accent of every terminal and editor on the page. */
export const ACCENT = "#7fdcf0"
/** The windows on the Mac's stage (960 by 600, ui macos): 36 from the top (under the menu
 *  bar), 12 from the foot and the sides, 12 between them, the screen split in two halves of
 *  the full height. The Cells window, the one that flies, on the left; the terminal on the
 *  right, as tall as its screen. */
const INSET = 12
const TOP = 36
const WINDOW_W = (960 - 3 * INSET) / 2
const WINDOW_H = 600 - TOP - INSET
const SIDE = { left: INSET + WINDOW_W + INSET, width: WINDOW_W }

/** The file, the hand and the terminal's screen at the hero's instant. */
function useFrame(ms: number) {
  const h = useHero()
  const f = tableFrameAt(h.timeline, Math.min(ms, h.timeline.total))
  const text = f.texts[SPENDING_PATH] as string
  const screen = h.texts.indexOf(text)
  if (screen < 0) throw new Error("hero: a frame holds a text the hero never writes")
  return { text, screen, scene: sceneOf(f, SPENDING_PATH) }
}

/** The Cells window's height on the page, at its stage width: shorter than the Mac's half
 *  of the screen, as tall as some rows, its prelude and some air. */
const LANDED_H = 360

/** The Cells window as the page keeps it once the hand is done: the window that leaves
 *  the Mac, at its stage width and `height` (its stage height while it flies, from the
 *  Mac's to the page's, is `--stage-h`), scaled to `width` as a whole, so the flight is a
 *  move, a scale and the window's own height easing to the page's. */
function SpendingWindow({ width, height }: { width: number; height: number }) {
  const h = useHero()
  return (
    <div style={{ width, height: `calc(var(--stage-h, ${height}px) * ${width / WINDOW_W})` }}>
      <div style={{ width: WINDOW_W, height: `var(--stage-h, ${height}px)`, transform: `scale(${width / WINDOW_W})`, transformOrigin: "0 0" }}>
        <CellsWindow name={SPENDING_PATH} text={h.checkpoints.fixed} className="h-full" />
      </div>
    </div>
  )
}

/** What the hero shows, said: for assistive tech, the same on the Mac and stacked. */
const ALT = `A MacBook at night: ${SPENDING_PATH} open in ${PRODUCT.name} on the left, six months of spending by category, and on the right a terminal running ${WATCH_COMMAND}, charting spending by category and spent against budget by month. A dinner of ${DINNER.was} is typed as ${DINNER.typo} in ${PRODUCT.name}, and the terminal redraws May over its budget; typed back, May is under again.`

/** The landing's width on the page: the window a little larger than on the Mac. */
const LANDING_W = 520

/** Where the window the Mac lets go of lands: the page's first block. On a phone the hero
 *  already shows the windows, stacked, and nothing flies, so there is none. */
export function Landing() {
  const narrow = useNarrow()
  if (narrow) return null
  return (
    // Before it knows the width (the server's HTML) a phone collapses it rather than
    // `display: none`.
    <div data-hero-landing className="overflow-hidden rounded-[10px] max-md:invisible max-md:h-0" style={{ width: LANDING_W }}>
      <SpendingWindow width={LANDING_W} height={LANDED_H} />
    </div>
  )
}

function Scene({ lid }: { lid: number }) {
  const h = useHero()
  const ms = usePlayhead(undefined).at * h.total
  const f = useFrame(ms)
  const macMs = SHUT_FROM + lid * LID_MS
  return (
    <Playhead progress={macMs / MAC_CLOCK.total}>
      <Macos timeline={MAC} art={NO_ART} device="macbook" captions={false} front={PRODUCT.name} alt={ALT}>
        {/* Night over the desktop picture: the windows are the brightest thing. */}
        <div aria-hidden className="absolute inset-0 bg-[radial-gradient(120%_90%_at_70%_100%,rgb(8_12_20/0.2),rgb(4_6_10/0.6))]" />
        {/* Gone from the screen while it flies (useDeparture): one window, never two. */}
        <div
          data-slot="hero-app-window"
          className="absolute"
          style={{ top: TOP, left: INSET, width: WINDOW_W, height: WINDOW_H, visibility: lid > 0 ? "hidden" : undefined }}
        >
          <CellsWindow name={SPENDING_PATH} text={f.text} scene={f.scene} className="h-full" />
        </div>
        <ChartTerminal index={f.screen} accent={ACCENT} className="absolute" style={{ top: TOP, left: SIDE.left, width: SIDE.width }} />
      </Macos>
    </Playhead>
  )
}

/** The Cells window on a phone: its height, and the scale it is drawn at. */
const PHONE_H = 420
const PHONE_SCALE = 0.8

/** The same moments on a phone's width, where a Mac's screen would be too small to read:
 *  no machine, the two windows stacked, the same clock. */
function Stacked() {
  const h = useHero()
  const ms = usePlayhead(undefined).at * h.total
  const f = useFrame(ms)
  return (
    <div role="img" aria-label={ALT} className="space-y-3">
      {/* At a phone's width the table drawn a little smaller, so its four columns, the
          amount the hand types into among them, all fit. */}
      <div style={{ height: PHONE_H * PHONE_SCALE }}>
        <div style={{ width: `${100 / PHONE_SCALE}%`, height: PHONE_H, transform: `scale(${PHONE_SCALE})`, transformOrigin: "0 0" }}>
          <CellsWindow name={SPENDING_PATH} text={f.text} scene={f.scene} className="h-full" />
        </div>
      </div>
      <ChartTerminal index={f.screen} accent={ACCENT} />
    </div>
  )
}

/** The hero: played in view and round again; any frame still with `?still=0.62` (a
 *  README's picture, one moment looked at); the Mac shut under reduced motion. */
export function Hero() {
  const h = useHero()
  const reduced = useReducedMotion()
  const still = useStill()
  const narrow = useNarrow()
  const root = React.useRef<HTMLDivElement>(null)
  const flight = React.useRef<HTMLDivElement>(null)
  const departs = !narrow && !reduced
  const mounted = React.useSyncExternalStore(NO_CHANGE, () => true, () => false)
  const lid = useDeparture(root, flight, departs && mounted)
  if (narrow) {
    if (still !== null || reduced) return <Playhead progress={still ?? 1}>{<Stacked />}</Playhead>
    return (
      <Playback playable={h.playable} playing>
        <Stacked />
      </Playback>
    )
  }
  // Scrolled into, the loop holds its last frame: the file the page goes on with.
  const held = reduced ? 1 : lid > 0 ? 1 : still
  return (
    <div ref={root} data-slot="hero">
      {held !== null ? (
        <Playhead progress={held}>
          <Scene lid={reduced ? 1 : lid} />
        </Playhead>
      ) : (
        <Playback playable={h.playable} playing>
          <Scene lid={0} />
        </Playback>
      )}
      {/* The window that comes out of the screen: the landing's twin, over the whole page. */}
      {departs &&
        mounted &&
        createPortal(
          // Hidden by visibility, never display: laid out while it waits, so its table has
          // measured its columns before it is first seen.
          <div ref={flight} aria-hidden className="pointer-events-none fixed top-0 left-0 z-30 origin-top-left rounded-[10px] will-change-transform" style={{ visibility: "hidden" }}>
            <SpendingWindow width={WINDOW_W} height={WINDOW_H} />
          </div>,
          document.body,
        )}
    </div>
  )
}

const clamp01 = (t: number) => Math.min(1, Math.max(0, t))
const lerp = (a: number, b: number, t: number) => a + (b - a) * t
/** Slow out, slow in: one move, from rest to rest. */
const ease = (t: number) => t * t * (3 - 2 * t)

/** Where the window lands, as a share of the viewport's height from its top. */
const LAND_AT = 0.12
/** How much nearer the reader the window comes at the middle of its flight. */
const LIFT = 0.06

/** THE DEPARTURE: the scroll past the hero, read every frame. Returns how far the lid has
 *  shut (0..1, over the sticky stage's extra scroll). The money window lifts out of the
 *  screen toward the reader and lands on the page, in ONE continuous move over one
 *  progress: from the first pixel of scroll to the landing slot reaching LAND_AT. The
 *  flying window (`flight`) is the landing's own component at the Mac window's stage
 *  size, so the whole journey is a translate and a uniform scale: from the Mac window's
 *  rect to the slot's, a little larger at the middle (it comes nearer, its shadow with
 *  it). The Mac's window is hidden for the whole flight and the slot until it lands. */
function useDeparture(
  root: React.RefObject<HTMLDivElement | null>,
  flightRef: React.RefObject<HTMLDivElement | null>,
  on: boolean,
): number {
  const [lid, setLid] = React.useState(0)
  React.useEffect(() => {
    const hero = root.current
    const flight = flightRef.current
    if (!on || !hero || !flight) return
    const section = hero.closest<HTMLElement>("[data-hero-section]")
    const landing = document.querySelector<HTMLElement>("[data-hero-landing]")
    if (!section || !landing) throw new Error("hero: the page has no [data-hero-section] or [data-hero-landing]")
    // The Mac's window as it stood before anything moved: once the lid folds, its rect
    // folds too.
    let from: DOMRect | null = null
    let frame = 0
    let lastLid = -1
    const tick = () => {
      frame = 0
      const vh = innerHeight
      const s = section.getBoundingClientRect()
      const shut = clamp01(-s.top / Math.max(1, s.height - vh))
      if (Math.abs(shut - lastLid) > 0.001) {
        lastLid = shut
        setLid(shut)
      }
      const window = hero.querySelector<HTMLElement>("[data-slot=hero-app-window]")
      if (!window) return
      const l = landing.getBoundingClientRect()
      // The journey is the scroll from the stage starting to move to the slot reaching
      // LAND_AT: one progress for the lift, the growth and the travel.
      const p = clamp01(-s.top / Math.max(1, l.top - s.top - vh * LAND_AT))
      if (p === 0) from = window.getBoundingClientRect()
      const away = p > 0 && p < 1 && from !== null
      flight.style.visibility = away ? "visible" : "hidden"
      landing.style.visibility = away ? "hidden" : ""
      if (!away || !from) return
      const e = ease(p)
      const near = Math.sin(Math.PI * p)
      // The rect it is at, then that rect a little larger about its own centre.
      const w = lerp(from.width, l.width, e)
      const h = lerp(from.height, l.height, e)
      const k = (w / WINDOW_W) * (1 + LIFT * near)
      const x = lerp(from.left, l.left, e) - (w * LIFT * near) / 2
      const y = lerp(from.top, vh * LAND_AT, e) - (h * LIFT * near) / 2
      flight.style.transform = `translate3d(${x}px, ${y}px, 0) scale(${k})`
      // its own height eases from the Mac's half of the screen to the page's, with the rest
      flight.style.setProperty("--stage-h", `${lerp(WINDOW_H, LANDED_H, e)}px`)
      flight.style.boxShadow = `0 ${24 * near}px ${70 * near}px -16px rgb(0 0 0 / ${0.75 * near})`
    }
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(tick)
    }
    schedule()
    addEventListener("scroll", schedule, { passive: true })
    addEventListener("resize", schedule)
    return () => {
      cancelAnimationFrame(frame)
      removeEventListener("scroll", schedule)
      removeEventListener("resize", schedule)
      landing.style.visibility = ""
    }
  }, [root, flightRef, on])
  return lid
}

const NO_CHANGE = () => () => {}

/** `?still=<0..1>` in the address: the hero held at that moment. */
function useStill(): number | null {
  const raw = React.useSyncExternalStore(
    NO_CHANGE,
    () => new URLSearchParams(location.search).get("still"),
    () => null,
  )
  if (raw === null) return null
  const p = Number(raw)
  if (!(p >= 0 && p <= 1)) throw new Error(`hero: ?still=${raw} is no moment between 0 and 1`)
  return p
}

function useMedia(query: string): boolean {
  return React.useSyncExternalStore(
    (changed) => {
      const q = matchMedia(query)
      q.addEventListener("change", changed)
      return () => q.removeEventListener("change", changed)
    },
    () => matchMedia(query).matches,
    () => false,
  )
}

/** Narrower than a Mac's screen can be read at: a phone. */
const useNarrow = () => useMedia("(max-width: 720px)")
const useReducedMotion = () => useMedia("(prefers-reduced-motion: reduce)")
