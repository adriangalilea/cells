"use client"

// THE HERO: a MacBook at night, the lid opening onto ten seconds of work. Helix on the
// left types into money.cell; the Cells window on the right reads the file at every
// keystroke; the terminal checks it. One clock (lib/hero `HERO.clip`) drives all of it:
// `Playback` plays it once in view and round again, a fixed progress draws a still
// (reduced motion: the last frame). Every window reads its own moment from the hero's
// through a `Playhead`, so each ui piece keeps its own clock's meaning.

import * as React from "react"
import { CellsWindow } from "@/components/cells-window"
import { HelixDiagnostics } from "@/components/helix-diagnostics"
import { Macos } from "@/components/ui/macos"
import { Playback, Playhead, usePlayhead } from "@/components/ui/playback"
import { Terminal } from "@/components/ui/terminal"
import { TerminalEditor } from "@/components/ui/terminal-editor"
import { codeFrameAt, codeTimeline } from "@/lib/code-session"
import { HERO } from "@/lib/hero"
import { type Art, sceneClock, type Timeline } from "@/lib/macos-session"
import { PRODUCT } from "@/lib/product"

/** The Mac's own story: closed, then the lid opens. Everything else is a window on it. */
const WORLD = { battery: 82, charging: false, lid: "closed" as const, asleep: false, heat: false }
const MAC: Timeline = {
  app: "Terminal",
  chord: "",
  steps: [
    { kind: "world", world: WORLD },
    { kind: "world", world: { ...WORLD, lid: "open" }, delay: 200 },
  ],
}
const MAC_TOTAL = sceneClock(MAC).total
const NO_ART: Art = { icon: "", glyphs: {} }

const EDIT = codeTimeline(HERO.files, HERO.steps)

/** The editor's accent, the terminal's phosphor: the site's cold edge light. */
const ACCENT = "#7fdcf0"
/** Helix's rows: the whole file fits, so the view never scrolls. */
const ROWS = 15

const share = (ms: number, from: number, length: number) => Math.min(1, Math.max(0, (ms - from) / length))

function Scene() {
  const ms = usePlayhead(undefined).at * HERO.total
  const editAt = share(ms, HERO.edit.from, HERO.edit.duration)
  const text = codeFrameAt(EDIT, editAt * EDIT.total).text
  return (
    <Playhead progress={Math.min(1, ms / MAC_TOTAL)}>
      <Macos
        timeline={MAC}
        art={NO_ART}
        device="macbook"
        captions={false}
        front="Cells"
        alt={`A MacBook opening at night. In Helix, a movement to an account named coffee is typed into ${HERO.file}; the ${PRODUCT.name} window beside it computes the row's VAT as the amount lands and names coffee as no key of accounts, in the editor and on the table's row. Saved, and ${PRODUCT.cli} check in the terminal names the row. The account is added to the file's own table, the row is whole, saved again, and the check passes.`}
      >
        {/* Night over the desktop picture: the windows are the brightest thing. */}
        <div aria-hidden className="absolute inset-0 bg-[radial-gradient(120%_90%_at_70%_100%,rgb(8_12_20/0.15),rgb(4_6_10/0.55))]" />
        <Playhead progress={editAt}>
          <div className="absolute top-[38px] left-[14px] w-[492px]">
            <Terminal alt="" accent={ACCENT} size="10.5px">
              <div className="relative">
                <TerminalEditor files={HERO.files} steps={HERO.steps} rows={ROWS} />
                <HelixDiagnostics timeline={EDIT} rows={ROWS} />
              </div>
            </Terminal>
          </div>
        </Playhead>
        <CellsWindow
          name={HERO.file}
          text={text}
          className="absolute top-[38px] left-[516px] h-[330px] w-[430px]"
        />
        <Playhead progress={share(ms, HERO.terminal.from, HERO.terminal.duration)}>
          <div className="absolute top-[378px] left-[516px] w-[430px]">
            <Terminal alt="" accent={ACCENT} size="10px" session={HERO.session} rows={4} />
          </div>
        </Playhead>
      </Macos>
    </Playhead>
  )
}

/** The same ten seconds on a phone's width, where a Mac's screen would be too small to
 *  read: no machine, the three windows stacked, the same clock. */
function Stacked() {
  const ms = usePlayhead(undefined).at * HERO.total
  const editAt = share(ms, HERO.edit.from, HERO.edit.duration)
  const text = codeFrameAt(EDIT, editAt * EDIT.total).text
  return (
    <div role="img" aria-label={`In Helix, a movement to an account named coffee is typed into ${HERO.file}; the ${PRODUCT.name} table names coffee as no key of accounts until the account is added, and ${PRODUCT.cli} check passes.`} className="space-y-3">
      <Playhead progress={editAt}>
        <Terminal alt="" accent={ACCENT} size="clamp(7px,2.35vw,10px)">
          <div className="relative">
            <TerminalEditor files={HERO.files} steps={HERO.steps} rows={ROWS} />
            <HelixDiagnostics timeline={EDIT} rows={ROWS} />
          </div>
        </Terminal>
      </Playhead>
      <CellsWindow name={HERO.file} text={text} className="h-[150px]" />
      <Playhead progress={share(ms, HERO.terminal.from, HERO.terminal.duration)}>
        <Terminal alt="" accent={ACCENT} size="clamp(7px,2.35vw,10px)" session={HERO.session} rows={4} />
      </Playhead>
    </div>
  )
}

/** The hero: played once in view and round again; its last frame still under reduced
 *  motion; any frame still with `?still=0.62` (a README's picture, a look at one moment).
 *  A Mac on a wide screen, the windows stacked on a phone's. */
export function Hero() {
  const reduced = useReducedMotion()
  const still = useStill()
  const narrow = useNarrow()
  const scene = narrow ? <Stacked /> : <Scene />
  if (still !== null || reduced) return <Playhead progress={still ?? 1}>{scene}</Playhead>
  return (
    <Playback clip={HERO.clip} playing>
      {scene}
    </Playback>
  )
}

/** Narrower than a Mac's screen can be read at: a phone. */
function useNarrow(): boolean {
  return React.useSyncExternalStore(
    (changed) => {
      const q = matchMedia("(max-width: 720px)")
      q.addEventListener("change", changed)
      return () => q.removeEventListener("change", changed)
    },
    () => matchMedia("(max-width: 720px)").matches,
    () => false,
  )
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

function useReducedMotion(): boolean {
  return React.useSyncExternalStore(
    (changed) => {
      const q = matchMedia("(prefers-reduced-motion: reduce)")
      q.addEventListener("change", changed)
      return () => q.removeEventListener("change", changed)
    },
    () => matchMedia("(prefers-reduced-motion: reduce)").matches,
    () => false,
  )
}
