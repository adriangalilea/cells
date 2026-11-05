"use client"

// THE STORY: one file built act by act (lib/story), each act one idea played as one show
// (lib/scene) once it is in view (ui's Playback), the file large and first, the table
// under it once it has arrived, and the words only after the show. Afterwards the file is
// the reader's to edit, read by the engine at every keystroke. Reduced motion: each act at
// rest, every part shown.

import * as React from "react"
import { CellEditor } from "@/components/cell-editor"
import { CellsTable, type TableLight } from "@/components/cells-window"
import { Playback, usePlayhead } from "@/components/ui/playback"
import { useEngine } from "@/components/use-engine"
import { PRODUCT } from "@/lib/product"
import { type ActFrame, type ActTimeline, actFrameAt, actTimeline } from "@/lib/scene"
import { STORY, type StoryAct, type StoryFile } from "@/lib/story"
import { cn } from "@/lib/utils"
import { readFiles, type ViewTable } from "@/lib/view"

export function Story() {
  return (
    <div className="divide-y divide-border/40">
      {STORY.map((act, i) => (
        <Act key={act.id} act={act} index={i} />
      ))}
    </div>
  )
}

/** Where an act starts: once its top has come above this share of the viewport's height,
 *  where a reader reads, never while it only peeks in under the act being read. */
const READ_LINE = 0.55

/** Whether `el` has reached the reading line, for good. (For ui: a start line on
 *  Playback, `startAt`, so an act begins when it reaches where it is read rather than when
 *  a third of it shows; this stands in for it through Playback's own `cue`.) */
function useReached(el: HTMLElement | null): boolean {
  const [reached, setReached] = React.useState(false)
  React.useEffect(() => {
    if (!el || reached) return
    const io = new IntersectionObserver(([e]) => e?.isIntersecting && setReached(true), {
      rootMargin: `0px 0px -${Math.round((1 - READ_LINE) * 100)}% 0px`,
    })
    io.observe(el)
    return () => io.disconnect()
  }, [el, reached])
  return reached
}

const useReducedMotion = () =>
  React.useSyncExternalStore(
    (changed) => {
      const q = matchMedia("(prefers-reduced-motion: reduce)")
      q.addEventListener("change", changed)
      return () => q.removeEventListener("change", changed)
    },
    () => matchMedia("(prefers-reduced-motion: reduce)").matches,
    () => false,
  )

function Act({ act, index }: { act: StoryAct; index: number }) {
  const timeline = React.useMemo(() => actTimeline(act), [act])
  const playable = React.useMemo(() => ({ duration: timeline.total, chapters: [] }), [timeline])
  const [el, setEl] = React.useState<HTMLElement | null>(null)
  const reached = useReached(el)
  // Held on its first frame until it reaches the reading line, then played to its end
  // (reduced motion: at its end at once, the item's own rule).
  const reduced = useReducedMotion()
  const cue = React.useMemo(() => ({ from: 0, to: reached || reduced ? timeline.total : 0 }), [reached, reduced, timeline])
  return (
    <section ref={setEl} id={act.id} aria-labelledby={`${act.id}-told`} className="mx-auto max-w-6xl px-6 py-14 md:py-20">
      <Playback playable={playable} cue={cue}>
        <Played act={act} index={index} timeline={timeline} />
      </Playback>
    </section>
  )
}

function Played({ act, index, timeline }: { act: StoryAct; index: number; timeline: ActTimeline }) {
  const at = usePlayhead(undefined).at
  const frame = actFrameAt(timeline, at * timeline.total)
  const played = at >= 1
  // Once played, the files are the reader's: edits land here and the act's marks go.
  const [edited, setEdited] = React.useState<StoryFile[] | null>(null)
  const [open, setOpen] = React.useState(0)
  // The files as the frame stands, one identity per text, so the engine reads a file once
  // per change, never once per frame.
  const shown = React.useMemo(() => [{ name: frame.name, text: frame.text }, ...act.others], [act, frame.name, frame.text])
  const files = edited ?? shown
  const file = files[open] as StoryFile
  const engine = useEngine()
  const read = React.useMemo(() => readFiles(engine, files), [engine, files])
  const judged = React.useMemo(() => (open === 0 ? read : readFiles(engine, [file])), [engine, open, read, file])
  const marking = edited === null && open === 0
  const edit = (text: string) => setEdited(files.map((f, k) => (k === open ? { ...f, text } : f)))
  // The table's room is kept from the act's start when it arrives in it, so nothing below
  // moves when it does.
  const table = act.table || act.beats.some((b) => "table" in b)
  const light: TableLight | undefined = marking && frame.lit ? { ...frame.lit.at, amount: frame.lit.amount, dim: frame.lit.amount } : undefined
  const marked = marking && (frame.band !== null || frame.nameBand !== null)
  // The type fits the act's longest line as the act leaves it, so the whole file shows and
  // its size never changes while the act plays.
  const fit = React.useMemo(() => Math.max(...[act.end, ...act.others].flatMap((f) => f.text.split("\n").map((l) => l.length))), [act])
  return (
    <div>
      <p className="font-mono text-[11px] tracking-[0.18em] text-muted-foreground/70">{String(index + 1).padStart(2, "0")}</p>
      <Pane className="mt-4">
        <Tabs files={files} open={open} choose={played ? setOpen : undefined} frame={marking ? frame : null} />
        <CellEditor
          name={file.name}
          value={file.text}
          onChange={played ? edit : undefined}
          problems={judged.problems}
          broken={judged.broken}
          band={marking ? frame.band : null}
          dim={marked ? 1 : 0}
          caret={marking && frame.caret?.in === "text" ? frame.caret.at : null}
          fit={fit}
          label={`edit ${file.name}`}
        />
      </Pane>
      {table && (
        <div style={{ opacity: frame.table, transform: `translateY(${(1 - frame.table) * 10}px)` }} className="mt-3">
          <Pane>
            <div className="border-b border-border px-4 py-2 font-mono text-[11.5px] text-muted-foreground">
              {files[0]?.name} · {PRODUCT.name}
            </div>
            <div className="text-[13px] md:text-[15px]">
              <CellsTable name={files[0]?.name ?? ""} text={files[0]?.text ?? ""} others={files.slice(1)} label={`${files[0]?.name}, as a table`} light={light} />
            </div>
            <div className="border-t border-border">
              <Problems read={read} />
            </div>
          </Pane>
        </div>
      )}
      <Told act={act} frame={frame} />
    </div>
  )
}

/** The file's name and its imports', a tab each; the first's name marked and retyped by a
 *  rename while the act plays. */
function Tabs({ files, open, choose, frame }: { files: StoryFile[]; open: number; choose?: (k: number) => void; frame: ActFrame | null }) {
  return (
    <div role="tablist" aria-label="files" className="flex gap-1 border-b border-border px-2 pt-2">
      {files.map((f, k) => (
        <button
          key={k === 0 ? "file" : f.name}
          role="tab"
          type="button"
          aria-selected={k === open}
          disabled={!choose}
          onClick={() => choose?.(k)}
          className={cn(
            "relative rounded-t-md px-3 py-1.5 font-mono text-[13px] whitespace-pre transition-colors md:text-[14px]",
            k === open ? "bg-foreground/6 text-foreground" : "text-muted-foreground enabled:hover:text-foreground",
          )}
        >
          {k === 0 && frame ? <Named name={f.name} frame={frame} /> : f.name}
        </button>
      ))}
    </div>
  )
}

function Named({ name, frame }: { name: string; frame: ActFrame }) {
  const band = frame.nameBand
  const caret = frame.caret?.in === "name" ? frame.caret.at : null
  if (!band) return name
  return (
    <>
      {name.slice(0, band[0])}
      <span className="rounded-[3px] bg-edge/16 shadow-[0_0_0_1px_color-mix(in_oklch,var(--edge)_50%,transparent)]">{name.slice(band[0], band[1])}</span>
      {name.slice(band[1])}
      {caret !== null && <span className="absolute top-[0.45em] bottom-[0.45em] w-[2px] bg-edge" style={{ left: `calc(0.75rem + ${caret}ch)` }} />}
    </>
  )
}

/** The line told once the act has shown it. */
function Told({ act, frame }: { act: StoryAct; frame: ActFrame }) {
  return (
    <h2
      id={`${act.id}-told`}
      style={{ opacity: frame.tell, transform: `translateY(${(1 - frame.tell) * 8}px)` }}
      className="mt-8 max-w-3xl text-balance text-[clamp(1.375rem,2.4vw,2rem)] leading-[1.2] font-medium tracking-[-0.02em]"
    >
      {act.lead} <span className="text-muted-foreground">{act.rest}</span>
    </h2>
  )
}

function Pane({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn("overflow-hidden rounded-xl border border-border bg-card/70 shadow-[0_1px_0_0_rgb(255_255_255/0.04)_inset] backdrop-blur-sm", className)}>
      {children}
    </div>
  )
}

/** What breaks the file's grammar, then what breaks its cells: each in the engine's words. */
function Problems({ read }: { read: ViewTable }) {
  const said = [...read.broken.map((b) => b.why), ...read.problems.map((p) => p.why)]
  if (said.length === 0) return <p className="px-4 py-3 font-mono text-[12px] text-edge/80">no problems</p>
  return (
    <ul className="space-y-1 px-4 py-3 font-mono text-[12px] text-haze">
      {said.map((s) => (
        <li key={s}>{s}</li>
      ))}
    </ul>
  )
}
