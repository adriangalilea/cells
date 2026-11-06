"use client"

// THE HERO: a MacBook at night, open on the format revealed a step at a time.
// Helix, the full height of the screen on the left, opens accounts.cell then money.cell;
// the Cells window and the terminal stack on the right. One clock (lib/hero's `playable`)
// drives it all: `Playback` plays it in view and round again; every window reads its own
// moment from the hero's through a `Playhead`, so each ui piece keeps its clock's meaning.
//
// As the reader scrolls past it, the editor's window lifts out of the screen toward them
// and lands as the page's first block (`Landing`), in one move, while the lid folds shut
// behind it (the Mac's own lid, its timeline's last step, driven by the scroll). The
// windows keep one layout throughout; only their contents change. The Cells window shows
// the file in front in the editor, as last saved. Reduced motion: the Mac shut and the
// window already in place. A phone stacks the windows and nothing flies.

import * as React from "react"
import { createPortal } from "react-dom"
import { CellsWindow } from "@/components/cells-window"
import { Macos } from "@/components/ui/macos"
import { Playback, Playhead, usePlayhead } from "@/components/ui/playback"
import { Terminal } from "@/components/ui/terminal"
import { TerminalEditor } from "@/components/ui/terminal-editor"
import { useCellsDiagnose } from "@/components/use-cells-diagnose"
import { useEngine } from "@/components/use-engine"
import { type CodeStep, codeFrameAt } from "@/lib/code-session"
import { hero, saveAfter } from "@/lib/hero"
import { savedAt } from "@/lib/written"
import "@/lib/cells-code"
import { type Art, LID_MS, sceneClock, type Timeline } from "@/lib/macos-session"
import { PRODUCT } from "@/lib/product"
import { LINE_HEIGHT } from "@/lib/terminal-session"

/** The Mac's own story: open from the first frame, the same layout for the whole hero (its
 *  windows change only their contents), and shut only by the scroll. */
const WORLD = { battery: 82, charging: false, lid: "open" as const, asleep: false, heat: false }
const MAC: Timeline = {
  app: "Terminal",
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

/** The hero, its saves as the engine writes them. */
const useHero = () => hero(useEngine())

/** The editor's accent, the terminal's phosphor: the site's cold edge light. */
export const ACCENT = "#7fdcf0"
/** The editor's window on the Mac's stage (960 by 600, ui macos): 36 from the top (under
 *  the menu bar), 12 from the foot, the three windows' one inset. */
const INSET = 12
const TOP = 36
const WINDOW_W = 540
const WINDOW_H = 600 - TOP - INSET
/** Helix's rows: the screen's full height. */
const ROWS = 26
/** The window's height in its own type, from ui's terminal and terminal-editor: the bar
 *  (30/13 em), the bufferline, the text's top padding (0.5 em), the rows, the status and
 *  command lines; plus its 1px border above and below. */
const EDITOR_EM = 30 / 13 + 0.5 + (ROWS + 3) * LINE_HEIGHT
/** The type size that makes ROWS fill the window to its foot, status line last. */
const EDITOR_SIZE = `${(WINDOW_H - 2) / EDITOR_EM}px`

const share = (ms: number, from: number, length: number) => Math.min(1, Math.max(0, (ms - from) / length))

/** What the Cells window shows at the hero's instant: the file in front in the editor (it
 *  follows the editor, never ahead of it), as last written (an app watching the folder
 *  re-reads a file when the editor writes it, never while it is being typed), read with
 *  the other as written; and the editor's own moment. */
function useFiles(ms: number) {
  const h = useHero()
  const editAt = share(ms, h.edit.from, h.edit.duration)
  const editMs = editAt * h.timeline.total
  const front = codeFrameAt(h.timeline, editMs).active ?? h.accounts
  const other = front === h.money ? h.accounts : h.money
  return {
    editAt,
    name: front,
    text: savedAt(h.timeline, front, editMs),
    others: [{ name: other, text: savedAt(h.timeline, other, editMs) }],
  }
}

/** Helix in its window: ui's terminal-editor with its bufferline (always there, so the
 *  rows never move) and the engine as its language server, every file's text at the
 *  instant read together (components/use-cells-diagnose). `rows` counts the bufferline's. */
function Editor({
  size,
  rows = ROWS + 1,
  steps,
  onOpen,
  onEdit,
  className,
}: {
  size: string
  rows?: number
  steps?: CodeStep[]
  onOpen?: (path: string) => void
  onEdit?: (step: CodeStep) => void
  className?: string
}) {
  const h = useHero()
  const diagnose = useCellsDiagnose()
  return (
    <Terminal
      alt={onEdit ? "Helix on the hero's files: type into it" : ""}
      interactive={onEdit !== undefined}
      accent={ACCENT}
      size={size}
      className={className}
    >
      <TerminalEditor files={h.files} steps={steps ?? h.steps} rows={rows} bufferline="always" diagnose={diagnose} onOpen={onOpen} onEdit={onEdit} />
    </Terminal>
  )
}

/** The editor window the Mac's screen lets go of, as the page keeps it: the very window
 *  (its stage size, its type, its rows), scaled to `width` as a whole, so the flight
 *  between the two is a move and a scale and nothing else. money.cell as the edit left it,
 *  then what the reader did to it (`added`: a tab pressed, a key typed), each the edit's
 *  next step, handed back through `onStep`, so the frame drawn is still the edit's. */
export function EditorWindow({
  width,
  added = NO_STEPS,
  onStep,
}: {
  width: number
  added?: CodeStep[]
  onStep?: (step: CodeStep) => void
}) {
  const h = useHero()
  const steps = React.useMemo(() => [...h.steps, ...added], [h, added])
  const open = React.useMemo(() => onStep && ((path: string) => onStep({ kind: "open", path })), [onStep])
  return (
    <div style={{ width, height: (width * WINDOW_H) / WINDOW_W }}>
      <div style={{ width: WINDOW_W, height: WINDOW_H, transform: `scale(${width / WINDOW_W})`, transformOrigin: "0 0" }}>
        <Playhead progress={1}>
          <Editor size={EDITOR_SIZE} steps={steps} onOpen={open} onEdit={onStep} className="h-full" />
        </Playhead>
      </div>
    </div>
  )
}

const NO_STEPS: CodeStep[] = []

/** The landing's width on the page: the window a little larger than on the Mac. */
const LANDING_W = 600

/** Where the window the Mac lets go of lands: the page's first block. On a phone the hero
 *  already shows the editor, stacked, and nothing flies, so there is none. */
export function Landing() {
  const narrow = useNarrow()
  const engine = useEngine()
  const h = useHero()
  const [added, setAdded] = React.useState<CodeStep[]>(NO_STEPS)
  // An edit is written as it lands, as an editor formatting on save writes it: every
  // computed value it reaches stored beside its `=`, so `vat` follows the amount typed.
  const step = React.useCallback(
    (s: CodeStep) =>
      setAdded((a) => {
        const next = [...a, s]
        return s.kind === "type" || s.kind === "replace" ? [...next, saveAfter(engine, [...h.steps, ...next])] : next
      }),
    [engine, h],
  )
  if (narrow) return null
  return (
    // Before it knows the width (the server's HTML) a phone collapses it rather than
    // `display: none`, so the editor in it can still measure its columns.
    <div data-hero-landing className="overflow-hidden rounded-[10px] max-md:invisible max-md:h-0" style={{ width: LANDING_W }}>
      <EditorWindow width={LANDING_W} added={added} onStep={step} />
    </div>
  )
}

function Scene({ lid }: { lid: number }) {
  const h = useHero()
  const ms = usePlayhead(undefined).at * h.total
  const files = useFiles(ms)
  const macMs = SHUT_FROM + lid * LID_MS
  return (
    <Playhead progress={macMs / MAC_CLOCK.total}>
      <Macos
        timeline={MAC}
        art={NO_ART}
        device="macbook"
        captions={false}
        front={PRODUCT.name}
        alt={`A MacBook at night. In Helix, ${h.accounts} lists three accounts; ${h.money} imports it, starts as a plain table, and is given a type, a reference to accounts and a formula, the ${PRODUCT.name} window beside it reading each change. A movement to coffee, which accounts does not have, is named on its row, and ${PRODUCT.cli} check names it too; coffee is added to ${h.accounts} and the check passes.`}
      >
        {/* Night over the desktop picture: the windows are the brightest thing. */}
        <div aria-hidden className="absolute inset-0 bg-[radial-gradient(120%_90%_at_70%_100%,rgb(8_12_20/0.2),rgb(4_6_10/0.6))]" />
        <Playhead progress={files.editAt}>
          {/* Gone from the screen while it flies (components/hero useDeparture): one window,
              never two. */}
          <div
            data-slot="hero-editor-window"
            className="absolute"
            style={{ top: TOP, left: INSET, width: WINDOW_W, height: WINDOW_H, visibility: lid > 0 ? "hidden" : undefined }}
          >
            <Editor size={EDITOR_SIZE} className="h-full" />
          </div>
        </Playhead>
        <CellsWindow
          name={files.name}
          text={files.text}
          others={files.others}
          className="absolute top-[36px] left-[562px] h-[300px] w-[386px]"
        />
        <Playhead progress={share(ms, h.terminal.from, h.terminal.duration)}>
          {/* Down to the editor's foot: the two columns end on one line. */}
          <div className="absolute top-[346px] bottom-[12px] left-[562px] w-[386px] [&>*]:h-full">
            <Terminal alt="" accent={ACCENT} size="9.5px" session={h.session} rows={5} className="h-full" />
          </div>
        </Playhead>
      </Macos>
    </Playhead>
  )
}

/** The same moments on a phone's width, where a Mac's screen would be too small to read:
 *  no machine, the windows stacked, the same clock. */
function Stacked() {
  const h = useHero()
  const ms = usePlayhead(undefined).at * h.total
  const files = useFiles(ms)
  return (
    <div
      role="img"
      aria-label={`In Helix, ${h.money} imports ${h.accounts} and is given its types and a formula; the ${PRODUCT.name} table names coffee as no key of accounts until it is added, and ${PRODUCT.cli} check passes.`}
      className="space-y-3"
    >
      <Playhead progress={files.editAt}>
        {/* The longest file is 8 lines: the phone's editor is no taller than it needs. */}
        <Editor size="clamp(7px,2.35vw,10px)" rows={11} />
      </Playhead>
      <CellsWindow name={files.name} text={files.text} others={files.others} className="h-[188px]" />
      <Playhead progress={share(ms, h.terminal.from, h.terminal.duration)}>
        <Terminal alt="" accent={ACCENT} size="clamp(7px,2.35vw,10px)" session={h.session} rows={4} />
      </Playhead>
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
          // Hidden by visibility, never display: laid out while it waits, so the editor in it
          // has measured its columns before it is first seen.
          <div ref={flight} aria-hidden className="pointer-events-none fixed top-0 left-0 z-30 origin-top-left rounded-[10px] will-change-transform" style={{ visibility: "hidden" }}>
            <EditorWindow width={WINDOW_W} />
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
 *  shut (0..1, over the sticky stage's extra scroll). The editor's window lifts out of the
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
      const window = hero.querySelector<HTMLElement>("[data-slot=hero-editor-window]")
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
