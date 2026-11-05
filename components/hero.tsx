"use client"

// THE HERO: a MacBook at night, the lid opening onto the format revealed a step at a time.
// Helix, the full height of the screen on the left, opens accounts.cell then money.cell;
// the Cells window and the terminal stack on the right. One clock (lib/hero `HERO.clip`)
// drives it all: `Playback` plays it in view and round again; every window reads its own
// moment from the hero's through a `Playhead`, so each ui piece keeps its clock's meaning.
//
// As the reader scrolls past it, the file leaves the computer: money.cell's text lifts off
// the screen and travels into the page, landing on the story's first block (the same
// characters, a copy of that block moved onto them), while the lid folds shut behind it,
// folded by the Mac's own lid (its timeline's last step, driven by the scroll). Reduced
// motion: the Mac shut and the text already in place. A phone stacks the windows and
// keeps the text where it is.

import * as React from "react"
import { CellsWindow } from "@/components/cells-window"
import { HelixDiagnostics } from "@/components/helix-diagnostics"
import { Macos } from "@/components/ui/macos"
import { Playback, Playhead, usePlayhead } from "@/components/ui/playback"
import { Terminal, useTerminalPalette } from "@/components/ui/terminal"
import { TerminalEditor } from "@/components/ui/terminal-editor"
import { useEngine } from "@/components/use-engine"
import { readFiles } from "@/lib/cells"
import { codeFrameAt, codeTimeline, TYPE_MS } from "@/lib/code-session"
import { HERO, textAt } from "@/lib/hero"
import { type Art, LID_MS, sceneClock, type Timeline } from "@/lib/macos-session"
import { PRODUCT } from "@/lib/product"

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
/** Helix's rows: the screen's full height. */
const ROWS = 27

const share = (ms: number, from: number, length: number) => Math.min(1, Math.max(0, (ms - from) / length))

/** Both files as they stand at the hero's instant, the editor's own moment, and money.cell
 *  as it last read (`held`, for the Cells window while a header cell is half typed): found
 *  by stepping back a keystroke at a time, so a still knows it as a played frame does. */
function useFiles(ms: number) {
  const engine = useEngine()
  const editAt = share(ms, HERO.edit.from, HERO.edit.duration)
  const editMs = editAt * EDIT.total
  const money = textAt(EDIT, HERO.money, editMs)
  const accounts = textAt(EDIT, HERO.accounts, editMs)
  const reads = (t: string) => !readFiles(engine, [{ name: HERO.money, text: t }, { name: HERO.accounts, text: accounts }]).error
  let held = money
  for (let back = editMs; !reads(held) && back > 0; back -= TYPE_MS) held = textAt(EDIT, HERO.money, back)
  return { editAt, money, accounts, held }
}

/** Helix's bufferline: the open files as tabs, the one in front lit. A site-side stand-in
 *  until ui's terminal-editor draws its own (for ui: `bufferline` on TerminalEditor). */
function Bufferline() {
  const palette = useTerminalPalette()
  const ms = usePlayhead(undefined).at * EDIT.total
  const f = codeFrameAt(EDIT, ms)
  return (
    <div className="flex whitespace-pre" style={{ background: palette.bar, color: palette.muted }}>
      {f.open.map((path) => (
        <span
          key={path}
          className="px-[1ch]"
          style={path === f.active ? { background: palette.bg, color: palette.command } : undefined}
        >
          {path}
          {f.dirty.includes(path) ? " [+]" : ""}
        </span>
      ))}
      {f.open.length === 0 && <span className="px-[1ch]"> </span>}
    </div>
  )
}

function Editor({ size, rows = ROWS }: { size: string; rows?: number }) {
  return (
    <Terminal alt="" accent={ACCENT} size={size}>
      <Bufferline />
      <div className="relative" data-slot="hero-editor">
        <TerminalEditor files={HERO.files} steps={HERO.steps} rows={rows} />
        <HelixDiagnostics timeline={EDIT} rows={rows} others={HERO.files.map((f) => f.path)} />
      </div>
    </Terminal>
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
          <div className="absolute top-[36px] bottom-[12px] left-[12px] w-[540px]">
            <Editor size="10.5px" />
          </div>
        </Playhead>
        <CellsWindow
          name={HERO.money}
          text={files.money}
          others={[{ name: HERO.accounts, text: files.accounts }]}
          held={files.held}
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
      <CellsWindow name={HERO.money} text={files.money} others={[{ name: HERO.accounts, text: files.accounts }]} held={files.held} className="h-[188px]" />
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
  const lid = useDeparture(root, !narrow && !reduced)
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
    </div>
  )
}

const ease = (t: number) => t * t * (3 - 2 * t)
const clamp01 = (t: number) => Math.min(1, Math.max(0, t))
const lerp = (a: number, b: number, t: number) => a + (b - a) * t

/** THE DEPARTURE: the scroll past the hero, read every frame. Returns how far the lid has
 *  shut (0..1, over the sticky stage's extra scroll), and moves a copy of the landing
 *  block from the editor's text onto the block itself as the block comes up the page.
 *  Styles are written straight to the elements: a frame of scroll re-renders only the Mac. */
function useDeparture(root: React.RefObject<HTMLDivElement | null>, on: boolean): number {
  const [lid, setLid] = React.useState(0)
  React.useEffect(() => {
    const hero = root.current
    if (!on || !hero) return
    const section = hero.closest<HTMLElement>("[data-hero-section]")
    const landing = document.querySelector<HTMLElement>("[data-hero-landing]")
    if (!section || !landing) throw new Error("hero: the page has no [data-hero-section] or [data-hero-landing]")
    const pre = landing.querySelector<HTMLElement>("pre")
    if (!pre) throw new Error("hero: the landing block holds no <pre>")
    // The travelling copy: the landing block's own characters, inside code's own frame
    // class so they keep its colours and metrics, fixed over the page.
    const flight = document.createElement("div")
    flight.setAttribute("aria-hidden", "true")
    flight.className = "ag-code pointer-events-none fixed top-0 left-0 z-30 origin-top-left will-change-transform"
    flight.append(pre.cloneNode(true))
    flight.style.display = "none"
    document.body.append(flight)
    // Where the editor's first line stood in the window the moment the file lifted off:
    // the lid then folds the editor away, and its live rect would fold with it.
    let lifted: { left: number; top: number; line: number } | null = null
    let frame = 0
    let last = -1
    const tick = () => {
      frame = 0
      const vh = innerHeight
      const s = section.getBoundingClientRect()
      const shut = clamp01(-s.top / Math.max(1, s.height - vh))
      if (Math.abs(shut - last) > 0.001) {
        last = shut
        setLid(shut)
      }
      const editor = hero.querySelector<HTMLElement>("[data-slot=hero-editor]")
      const leaving = shut > 0.02
      if (!leaving) lifted = null
      else if (!lifted) {
        const text = hero.querySelector<HTMLElement>("[data-slot=hero-editor] [data-slot=terminal-editor-line] > span + span")
        if (!text) return
        const r = text.getBoundingClientRect()
        lifted = { left: r.left, top: r.top, line: r.height }
      }
      editor?.style.setProperty("opacity", leaving ? "0" : "1")
      const p = pre.getBoundingClientRect()
      // The flight starts as the stage lets go (the lid shut) and ends as the block reaches a
      // third of the way down the window: the scroll between the two is the whole journey.
      const released = Math.max(0, vh - s.bottom)
      const from = p.top + released
      const travel = ease(clamp01(released / Math.max(1, from - vh * 0.3)))
      const away = leaving && travel < 1
      flight.style.display = away ? "block" : "none"
      pre.style.visibility = away ? "hidden" : ""
      if (!away || !lifted) return
      const target = pre.querySelector<HTMLElement>(".line") ?? pre
      const tgt = target.getBoundingClientRect()
      const k0 = lifted.line / (tgt.height || 1)
      const k = lerp(k0, 1, travel)
      // Where the copy's top-left stands so its first line sits where the editor's stood,
      // lifted a little as the lid shuts under it: held there in the window (the Mac
      // scrolls away under it), then carried down onto the block.
      const dx = tgt.left - p.left
      const dy = tgt.top - p.top
      const x = lerp(lifted.left - dx * k0, p.left, travel)
      const y = lerp(lifted.top - dy * k0 - ease(shut) * 18, p.top, travel)
      flight.style.width = `${p.width}px`
      flight.style.transform = `translate3d(${x}px, ${y}px, 0) scale(${k})`
      flight.style.filter = `drop-shadow(0 0 ${14 * (1 - travel)}px color-mix(in oklch, var(--edge) 45%, transparent))`
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
      flight.remove()
      pre.style.visibility = ""
    }
  }, [root, on])
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
