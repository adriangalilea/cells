"use client"

// THE HERO: a MacBook at night, the lid opening onto the format revealed a step at a time.
// Helix, the full height of the screen on the left, opens accounts.cell then money.cell;
// the Cells window and the terminal stack on the right. One clock (lib/hero `HERO.clip`)
// drives it all: `Playback` plays it in view and round again; every window reads its own
// moment from the hero's through a `Playhead`, so each ui piece keeps its clock's meaning.
//
// As the reader scrolls past it, the editor's window leaves the computer: on the first
// frame of scroll it comes off the screen as one object and grows, continuously, into the
// page's first block, while the lid folds shut behind it (the Mac's own lid, its
// timeline's last step, driven by the scroll). The Cells window shows the file in front in
// the editor, as last saved. Reduced motion: the Mac shut and the window already in place.
// A phone stacks the windows and nothing flies.

import * as React from "react"
import { createPortal } from "react-dom"
import { CellsWindow } from "@/components/cells-window"
import { HelixDiagnostics } from "@/components/helix-diagnostics"
import { Macos } from "@/components/ui/macos"
import { Playback, Playhead, usePlayhead } from "@/components/ui/playback"
import { Terminal, useTerminalPalette } from "@/components/ui/terminal"
import { TerminalEditor } from "@/components/ui/terminal-editor"
import { codeFrameAt, codeTimeline } from "@/lib/code-session"
import { HERO, savedAt } from "@/lib/hero"
import { type Art, LID_MS, sceneClock, type Timeline } from "@/lib/macos-session"
import { PRODUCT } from "@/lib/product"
import { LINE_HEIGHT } from "@/lib/terminal-session"

/** The Mac's own story: closed, the lid opens, and (driven by the scroll) shuts again. */
const WORLD = { battery: 82, charging: false, lid: "closed" as const, asleep: false, heat: false }
const MAC: Timeline = {
  app: "Terminal",
  chord: "",
  steps: [
    { kind: "world", world: WORLD },
    { kind: "world", world: { ...WORLD, lid: "open" }, delay: 200 },
    { kind: "world", world: WORLD, delay: 4000 },
  ],
}
const MAC_CLOCK = sceneClock(MAC)
const SHUT_FROM = MAC_CLOCK.starts[2] as number
if (MAC_CLOCK.total < SHUT_FROM + LID_MS) throw new Error("hero: the Mac's story ends before its lid has shut")
const NO_ART: Art = { icon: "", glyphs: {} }

const EDIT = codeTimeline(HERO.files, HERO.steps)

/** The editor's accent, the terminal's phosphor: the site's cold edge light. */
const ACCENT = "#7fdcf0"
/** The editor's window on the Mac's stage (960 by 600, ui macos): 36 from the top (under
 *  the menu bar), 12 from the foot, the three windows' one inset. */
const INSET = 12
const TOP = 36
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
  const editAt = share(ms, HERO.edit.from, HERO.edit.duration)
  const editMs = editAt * EDIT.total
  const front = codeFrameAt(EDIT, editMs).active ?? HERO.accounts
  const other = front === HERO.money ? HERO.accounts : HERO.money
  return {
    editAt,
    name: front,
    text: savedAt(EDIT, front, editMs),
    others: [{ name: other, text: savedAt(EDIT, other, editMs) }],
  }
}

/** Helix's bufferline: the open files as tabs, the one in front lit; with `choose`, a press
 *  on a tab brings its file to the front. A site-side stand-in until ui's terminal-editor
 *  draws its own (for ui: `bufferline` on TerminalEditor). */
function Bufferline({ choose }: { choose?: (path: string) => void }) {
  const palette = useTerminalPalette()
  const ms = usePlayhead(undefined).at * EDIT.total
  const f = codeFrameAt(EDIT, ms)
  return (
    <div className="flex whitespace-pre" style={{ background: palette.bar, color: palette.muted }}>
      {f.open.map((path) => {
        const front = path === f.active
        const style = front ? { background: palette.bg, color: palette.command } : undefined
        const label = `${path}${f.dirty.includes(path) ? " [+]" : ""}`
        return choose ? (
          <button key={path} type="button" aria-pressed={front} onClick={() => choose(path)} className="cursor-pointer px-[1ch] hover:text-white" style={style}>
            {label}
          </button>
        ) : (
          <span key={path} className="px-[1ch]" style={style}>
            {label}
          </span>
        )
      })}
      {f.open.length === 0 && <span className="px-[1ch]"> </span>}
    </div>
  )
}

function Editor({ size, rows = ROWS, choose, className }: { size: string; rows?: number; choose?: (path: string) => void; className?: string }) {
  return (
    <Terminal alt="" accent={ACCENT} size={size} className={className}>
      <Bufferline choose={choose} />
      <div className="relative" data-slot="hero-editor">
        <TerminalEditor files={HERO.files} steps={HERO.steps} rows={rows} />
        <HelixDiagnostics timeline={EDIT} rows={rows} others={HERO.files.map((f) => f.path)} />
      </div>
    </Terminal>
  )
}

/** The editor window as the page keeps it, after the hero: both files as the edit left
 *  them, its tabs pressable. The same window the Mac's screen let go of, larger. */
/** Rows: money.cell's 7 lines and the caret's last, plus helix's 3 rows kept below the
 *  caret (its scrolloff), so the whole file shows from its first line. */
const LANDING = { size: "clamp(11px, 1.15vw, 15px)", rows: 10 }
/** The edit's instant each file is in front and whole: money.cell at the end, accounts.cell
 *  just after its save. */
const FRONT: Record<string, number> = {
  [HERO.money]: 1,
  [HERO.accounts]: ((EDIT.ends[12] as number) + 1) / EDIT.total,
}

export function EditorWindow({ choose = true, rows = LANDING.rows }: { choose?: boolean; rows?: number }) {
  const [front, setFront] = React.useState(HERO.money)
  return (
    <Playhead progress={FRONT[front] as number}>
      <Editor size={LANDING.size} rows={rows} choose={choose ? setFront : undefined} />
    </Playhead>
  )
}

function Scene({ lid }: { lid: number }) {
  const ms = usePlayhead(undefined).at * HERO.total
  const files = useFiles(ms)
  const opening = Math.min(ms, SHUT_FROM - 1)
  const macMs = lid > 0 ? SHUT_FROM + lid * LID_MS : opening
  return (
    <Playhead progress={macMs / MAC_CLOCK.total}>
      <Macos
        timeline={MAC}
        art={NO_ART}
        device="macbook"
        captions={false}
        front={PRODUCT.name}
        alt={`A MacBook opening at night. In Helix, ${HERO.accounts} lists three accounts; ${HERO.money} imports it, starts as a plain table, and is given a type, a reference to accounts and a formula, the ${PRODUCT.name} window beside it reading each change. A movement to coffee, which accounts does not have, is named on its row, and ${PRODUCT.cli} check names it too; coffee is added to ${HERO.accounts} and the check passes.`}
      >
        {/* Night over the desktop picture: the windows are the brightest thing. */}
        <div aria-hidden className="absolute inset-0 bg-[radial-gradient(120%_90%_at_70%_100%,rgb(8_12_20/0.2),rgb(4_6_10/0.6))]" />
        <Playhead progress={files.editAt}>
          {/* Gone from the screen while it flies (components/hero useDeparture): one window,
              never two. */}
          <div
            data-slot="hero-editor-window"
            className="absolute w-[540px]"
            style={{ top: TOP, bottom: INSET, left: INSET, visibility: lid > 0 ? "hidden" : undefined }}
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
        <Playhead progress={share(ms, HERO.terminal.from, HERO.terminal.duration)}>
          {/* Down to the editor's foot: the two columns end on one line. */}
          <div className="absolute top-[346px] bottom-[12px] left-[562px] w-[386px] [&>*]:h-full">
            <Terminal alt="" accent={ACCENT} size="9.5px" session={HERO.session} rows={5} className="h-full" />
          </div>
        </Playhead>
      </Macos>
    </Playhead>
  )
}

/** The same moments on a phone's width, where a Mac's screen would be too small to read:
 *  no machine, the windows stacked, the same clock. */
function Stacked() {
  const ms = usePlayhead(undefined).at * HERO.total
  const files = useFiles(ms)
  return (
    <div
      role="img"
      aria-label={`In Helix, ${HERO.money} imports ${HERO.accounts} and is given its types and a formula; the ${PRODUCT.name} table names coffee as no key of accounts until it is added, and ${PRODUCT.cli} check passes.`}
      className="space-y-3"
    >
      <Playhead progress={files.editAt}>
        {/* The longest file is 8 lines: the phone's editor is no taller than it needs. */}
        <Editor size="clamp(7px,2.35vw,10px)" rows={10} />
      </Playhead>
      <CellsWindow name={files.name} text={files.text} others={files.others} className="h-[188px]" />
      <Playhead progress={share(ms, HERO.terminal.from, HERO.terminal.duration)}>
        <Terminal alt="" accent={ACCENT} size="clamp(7px,2.35vw,10px)" session={HERO.session} rows={4} />
      </Playhead>
    </div>
  )
}

/** The hero: played in view and round again; any frame still with `?still=0.62` (a
 *  README's picture, one moment looked at); the Mac shut under reduced motion. */
export function Hero() {
  const reduced = useReducedMotion()
  const still = useStill()
  const narrow = useNarrow()
  const root = React.useRef<HTMLDivElement>(null)
  const flight = React.useRef<HTMLDivElement>(null)
  const departs = !narrow && !reduced
  const mounted = React.useSyncExternalStore(NO_CHANGE, () => true, () => false)
  const { lid, rows } = useDeparture(root, flight, departs && mounted)
  if (narrow) {
    if (still !== null || reduced) return <Playhead progress={still ?? 1}>{<Stacked />}</Playhead>
    return (
      <Playback clip={HERO.clip} playing>
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
        <Playback clip={HERO.clip} playing>
          <Scene lid={0} />
        </Playback>
      )}
      {/* The window that comes out of the screen: the landing's twin, over the whole page. */}
      {departs &&
        mounted &&
        createPortal(
          <div ref={flight} aria-hidden className="pointer-events-none fixed top-0 left-0 z-30 origin-top-left overflow-hidden rounded-[10px] will-change-transform" style={{ display: "none" }}>
            <EditorWindow choose={false} rows={rows} />
          </div>,
          document.body,
        )}
    </div>
  )
}

const clamp01 = (t: number) => Math.min(1, Math.max(0, t))
const lerp = (a: number, b: number, t: number) => a + (b - a) * t
/** Fast at once, still at the end: the window starts growing on the first frame of scroll
 *  and is at rest when the landing slides under it. */
const easeOut = (t: number) => 1 - (1 - t) ** 3

/** Where the window lands, as a share of the viewport's height from its top. */
const LAND_AT = 0.28

/** THE DEPARTURE: the scroll past the hero, read every frame. Returns how far the lid has
 *  shut (0..1, over the sticky stage's extra scroll) and the flying window's rows. The
 *  editor's window comes out of the screen as ONE object: on the first frame of scroll the
 *  Mac's window is gone from the screen and the page's own editor window (`flight`, laid
 *  over everything) stands exactly where it stood (its columns, its rows, its type at the
 *  Mac's scale); from there it grows continuously, while the lid folds behind it, to the
 *  page column's width and the landing's rows, at rest where the landing slot is when the
 *  scroll brings it there. Then the slot shows and the flight goes. Styles are written
 *  straight to the elements; React renders only the lid and a change of rows. */
function useDeparture(
  root: React.RefObject<HTMLDivElement | null>,
  flightRef: React.RefObject<HTMLDivElement | null>,
  on: boolean,
): { lid: number; rows: number } {
  const [lid, setLid] = React.useState(0)
  const [rows, setRows] = React.useState(ROWS)
  React.useEffect(() => {
    const hero = root.current
    const flight = flightRef.current
    if (!on || !hero || !flight) return
    const section = hero.closest<HTMLElement>("[data-hero-section]")
    const landing = document.querySelector<HTMLElement>("[data-hero-landing]")
    if (!section || !landing) throw new Error("hero: the page has no [data-hero-section] or [data-hero-landing]")
    const fontOf = (el: Element | null) => {
      const term = el?.querySelector('[data-slot="terminal"]')
      if (!term) throw new Error("hero: an editor window with no terminal in it")
      return Number.parseFloat(getComputedStyle(term).fontSize)
    }
    // The Mac's window as it stood before anything moved: its rect, its scale on the
    // screen, its type. Once the lid folds, all three fold with it.
    let from: { rect: DOMRect; width: number; scale: number; font: number } | null = null
    let frame = 0
    let lastLid = -1
    let lastRows = ROWS
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
      if (p === 0) {
        const rect = window.getBoundingClientRect()
        from = { rect, width: window.offsetWidth, scale: rect.width / window.offsetWidth, font: fontOf(window) }
      }
      const away = p > 0 && p < 1 && from !== null
      flight.style.display = away ? "block" : "none"
      landing.style.visibility = away ? "hidden" : ""
      if (!away || !from) return
      const e = easeOut(p)
      // The flight is laid out in the landing's type and drawn scaled: at the start its
      // layout is the Mac window's columns in that type, its scale the Mac's type size.
      const font = fontOf(landing)
      const width0 = (from.width * from.font) / font
      const scale = lerp((from.scale * from.font) / font, 1, e)
      const width = lerp(width0, l.width, e)
      const n = Math.round(lerp(ROWS, LANDING.rows, e))
      if (n !== lastRows) {
        lastRows = n
        setRows(n)
      }
      const x = lerp(from.rect.left, l.left, e)
      const y = lerp(from.rect.top, vh * LAND_AT, e)
      // Lifted off the screen: the shadow of something nearer the reader, gone once it sits
      // on the page.
      const lift = Math.sin(Math.PI * Math.min(1, p * 1.6))
      flight.style.width = `${width}px`
      flight.style.transform = `translate3d(${x}px, ${y}px, 0) scale(${scale})`
      flight.style.boxShadow = `0 ${28 * lift}px ${80 * lift}px -18px rgb(0 0 0 / 0.8)`
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
  return { lid, rows }
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
