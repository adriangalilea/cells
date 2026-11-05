"use client"

// THE STORY: one file built a step at a time (lib/story), each step its own screen. A step
// plays once it is in view (ui's Playback): the spot it is about is marked and everything
// else dimmed, the change happens in the file (keys typed, a name retyped), the table
// lights what it did, and only then the told line arrives. The table is not there until
// the step it arrives in. Afterwards the file is the reader's to edit, read by the engine
// at every keystroke. Reduced motion: each step at rest, every part shown.

import * as React from "react"
import { CellEditor } from "@/components/cell-editor"
import { CellsTable, type TableLight } from "@/components/cells-window"
import { Playback, usePlayhead } from "@/components/ui/playback"
import { useEngine } from "@/components/use-engine"
import { readFiles, type ViewTable } from "@/lib/cells"
import { PRODUCT } from "@/lib/product"
import { type SceneFrame, type SceneTimeline, sceneFrameAt, sceneTimeline } from "@/lib/scene"
import { STORY, type StoryFile, type StoryStep } from "@/lib/story"
import { cn } from "@/lib/utils"

/** The characters a line of the editor shows in half the story's row (max-w-6xl, 13px mono,
 *  its padding taken off). */
const HALF_ROW_CHARS = 60

export function Story() {
  return (
    <div>
      {STORY.map((step, i) => (
        <Scene key={step.id} step={step} index={i} />
      ))}
    </div>
  )
}

function Scene({ step, index }: { step: StoryStep; index: number }) {
  const timeline = React.useMemo(() => sceneTimeline(step), [step])
  const clip = React.useMemo(() => ({ duration: timeline.total, chapters: [] }), [timeline])
  // A screen to itself, the step in its middle: one is in view at a time, so it plays
  // when the reader reaches it, never while they read the one before.
  return (
    <section id={step.id} aria-labelledby={`${step.id}-told`} className="mx-auto flex min-h-svh max-w-6xl flex-col justify-center px-6 py-16">
      <Playback clip={clip} delay={250}>
        <Played step={step} index={index} timeline={timeline} />
      </Playback>
    </section>
  )
}

function Played({ step, index, timeline }: { step: StoryStep; index: number; timeline: SceneTimeline }) {
  const at = usePlayhead(undefined).at
  const frame = sceneFrameAt(timeline, at * timeline.total)
  const played = at >= 1
  // Once played, the files are the reader's: edits land here and the step's marks go.
  const [edited, setEdited] = React.useState<StoryFile[] | null>(null)
  const [open, setOpen] = React.useState(0)
  // The files as the frame stands, one identity per text, so the engine reads a file once
  // per change, never once per frame.
  const shown = React.useMemo(() => [{ name: frame.name, text: frame.text }, ...step.others], [step, frame.name, frame.text])
  const files = edited ?? shown
  const file = files[open] as StoryFile
  const engine = useEngine()
  const read = React.useMemo(() => readFiles(engine, files), [engine, files])
  const judged = React.useMemo(() => (open === 0 ? read : readFiles(engine, [file])), [engine, open, read, file])
  const marking = edited === null && open === 0
  const edit = (text: string) => setEdited(files.map((f, k) => (k === open ? { ...f, text } : f)))
  // Side by side only when the file's longest line fits half the row; else the file spans
  // it, with the table under it.
  const wide = React.useMemo(() => Math.max(...step.text.after.split("\n").map((l) => l.length)) > HALF_ROW_CHARS, [step])
  const table = step.table !== "none"
  const light: TableLight | undefined = marking && step.lit ? { ...step.lit, amount: frame.lit, dim: frame.dim } : undefined
  return (
    <div>
      <p className="font-mono text-[11px] tracking-[0.18em] text-muted-foreground/70">{String(index + 1).padStart(2, "0")}</p>
      <div className={cn("mt-5 grid gap-4", table ? !wide && "lg:grid-cols-2 lg:gap-5" : "max-w-2xl")}>
        <Pane className="self-start">
          <Tabs files={files} open={open} choose={played ? setOpen : undefined} frame={marking ? frame : null} />
          <CellEditor
            name={file.name}
            value={file.text}
            onChange={played ? edit : undefined}
            problems={judged.problems}
            broken={judged.broken}
            band={marking ? frame.band : null}
            dim={marking ? frame.dim : 0}
            caret={marking && frame.caret?.in === "text" ? frame.caret.at : null}
            label={`edit ${file.name}`}
          />
        </Pane>
        {table && (
          <div style={{ opacity: frame.table, transform: `translateY(${(1 - frame.table) * 12}px)` }} className="self-start">
            <Pane>
              <div className="border-b border-border px-4 py-2 font-mono text-[11.5px] text-muted-foreground">
                {files[0]?.name} · {PRODUCT.name}
              </div>
              <CellsTable name={files[0]?.name ?? ""} text={files[0]?.text ?? ""} others={files.slice(1)} label={`${files[0]?.name}, as a table`} light={light} className="max-h-72" />
              <div className="border-t border-border">
                <Problems read={read} />
              </div>
            </Pane>
          </div>
        )}
      </div>
      <Told step={step} frame={frame} />
    </div>
  )
}

/** The file's name and its imports', a tab each; the first's name marked and retyped by a
 *  rename while the step plays. */
function Tabs({
  files,
  open,
  choose,
  frame,
}: {
  files: StoryFile[]
  open: number
  choose?: (k: number) => void
  frame: SceneFrame | null
}) {
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
            "relative rounded-t-md px-3 py-1.5 font-mono text-[11.5px] whitespace-pre transition-colors",
            k === open ? "bg-foreground/6 text-foreground" : "text-muted-foreground enabled:hover:text-foreground",
          )}
        >
          {k === 0 && frame ? <Named name={f.name} frame={frame} /> : f.name}
        </button>
      ))}
    </div>
  )
}

function Named({ name, frame }: { name: string; frame: SceneFrame }) {
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

/** The line told once the step has shown it, and the forms of the idea under it. */
function Told({ step, frame }: { step: StoryStep; frame: SceneFrame }) {
  const style = { opacity: frame.tell, transform: `translateY(${(1 - frame.tell) * 8}px)` }
  return (
    <div style={style} className="mt-10 max-w-3xl">
      <h2 id={`${step.id}-told`} className="text-balance text-[clamp(1.5rem,2.6vw,2.125rem)] leading-[1.15] font-medium tracking-[-0.02em]">
        {step.lead} <span className="text-muted-foreground">{step.rest}</span>
      </h2>
      {step.forms && (
        <dl className="mt-6 grid max-w-2xl gap-x-6 gap-y-2 text-[14px] sm:grid-cols-[auto_1fr]">
          {step.forms.map((f) => (
            <React.Fragment key={f.formula}>
              <dt className="font-mono text-[12.5px] text-foreground/90">{f.formula}</dt>
              <dd className="text-muted-foreground">{f.says}</dd>
            </React.Fragment>
          ))}
        </dl>
      )}
    </div>
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
