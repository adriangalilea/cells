"use client"

// A WALKTHROUGH drawn at a moment: a file told step by step (lib/walkthrough-session),
// its lines in `code`'s own frame and colours, the marked lines under a band that
// travels from step to step, the lines that change folding and unfolding, keys typed
// with their caret, another file brought forward; beside it what the step shows live
// (a view, a function of its progress, as scroll-story's chapters draw), under it the
// step's words. It draws; something else moves time: inside `<Player
// playable={timeline.playable}>` it plays and scrubs, inside a `Playback` it plays once in
// view, `WalkthroughScroll` cues it step by step from a scroll stage with the words
// beside it, and a fixed `progress` is a still. No clock of its own.
//
//   const TOUR = walkthroughTimeline([
//     { title: "the file", file: "greet.ts", text: V1, marks: "1", words: <p>…</p> },
//     { title: "its answer", marks: "2", words: <p>…</p> },
//     { title: "say the name", text: V2, marks: "2", diff: true },
//   ])
//   <Player playable={TOUR.playable} label="greet"><Walkthrough timeline={TOUR} /></Player>
//
// A long file keeps the band in sight: the frame shows `rows` lines and scrolls to
// the band's middle as it travels, reading its own line height and its own height
// from the page, the facts the scroll is decided by.

import * as React from "react"
import { cn } from "@/lib/utils"
import { type CodeColours, runsOf } from "@/lib/code-tokens"
import { chapterSpan } from "@/lib/playable"
import {
  type WalkthroughFrame,
  type WalkthroughLine,
  type WalkthroughPane,
  type WalkthroughSign,
  type WalkthroughStep,
  type WalkthroughTimeline,
  walkthroughFrameAt,
  walkthroughRest,
} from "@/lib/walkthrough-session"
import { Card } from "@/components/ui/card"
import { CodeFrame } from "@/components/ui/code-frame"
import { Playback, usePlayhead } from "@/components/ui/playback"
import {
  Act,
  ScrollStage,
  useAct,
  useScrollStage,
} from "@/components/ui/scroll-stage"
import type { StoryMoment } from "@/components/ui/scroll-story"
import {
  ToggleGroup,
  ToggleGroupItem,
} from "@/components/ui/toggle-group"
import { useCodeColoursOf } from "@/components/ui/use-code-colours"

/** A step's moment as its view is given it: scroll-story's (`p` its progress,
 *  `active` while it is the step on), and the file as last written (`written`, its
 *  path and text: after the change, then after each keys entry, never mid-keys), so
 *  a view reading the file (a table, a preview) follows the edit as an app follows an
 *  editor's writes. */
export type WalkthroughMoment = StoryMoment & {
  written: { path: string; text: string } | null
}

/** What a step shows live beside its file, drawn at the step's moment. */
export type WalkthroughView = (moment: WalkthroughMoment) => React.ReactNode

/** Steps as `walkthrough` draws them: their words React nodes, their views live.
 *  Typed so, `walkthroughTimeline(STEPS)` reads every view's moment. */
export type WalkthroughSteps = readonly WalkthroughStep<
  React.ReactNode,
  WalkthroughView
>[]

/** Steps timed for this view: `walkthroughTimeline(steps)`. */
export type WalkthroughTimed = WalkthroughTimeline<
  React.ReactNode,
  WalkthroughView
>

/** The lines shown before the file scrolls, unless `rows` says. */
const ROWS = 16

/** A line's sign easing in and out, the one motion not drawn by the instant: a
 *  diff's tint (code.css) fading as the next step folds it, never cut. */
const SIGN_MOTION =
  "transition-[background-color,border-color] duration-300 motion-reduce:transition-none"

/** The step's words: its title, then what it says. */
const TITLE = "font-semibold text-lg tracking-tight"
const WORDS = "text-[0.9375rem] text-foreground/70 leading-relaxed"
/** Told beside a pinned stage (`WalkthroughScroll`), the words are the page's own
 *  text, not a caption: a heading's size and a reading size, set to the screen. */
const TOLD_TITLE =
  "font-medium text-[clamp(1.5rem,2.4vw,2.25rem)] leading-[1.1] tracking-[-0.025em] text-balance"
const TOLD_WORDS =
  "text-[clamp(1.0625rem,1.3vw,1.25rem)] text-foreground/70 leading-relaxed text-pretty"

export interface WalkthroughProps {
  /** The walkthrough, timed: `walkthroughTimeline(steps)`, whose `playable` the
   *  driver around it plays. */
  timeline: WalkthroughTimed
  /** The progress drawn, 0..1, for a still. Inside a driver omit it. */
  progress?: number
  /** The lines the file shows before it scrolls. The frame is as tall as the
   *  longest state when it is shorter. */
  rows?: number
  /** The step's title and words under the frame. Off where the page sets them
   *  itself (`WalkthroughScroll` beside it). */
  words?: boolean
  /** Its face: the page's frame, or the one glass material. */
  material?: "plain" | "glass"
  /** The views in a card of their own (default); false where each view brings its
   *  own windows (an editor, an app), which a card would only frame twice. */
  framed?: boolean
  className?: string
}

export function Walkthrough({
  timeline,
  progress,
  rows = ROWS,
  words = true,
  material = "plain",
  framed = true,
  className,
}: WalkthroughProps) {
  const { at, driven } = usePlayhead(progress)
  const frame = walkthroughFrameAt(timeline, at * timeline.total)
  // Every state's colours, read once in the browser: plain until they load.
  const colours = useCodeColoursOf(timeline.states)
  const code = timeline.plans.some((p) => p.path !== null)
  const views = timeline.steps.some((s) => s.view !== undefined)
  return (
    <div
      data-slot="walkthrough"
      className={cn("@container flex min-w-0 flex-col gap-4", className)}
    >
      <div
        className={cn(
          "grid min-w-0 gap-4",
          code && views && "@3xl:grid-cols-2",
        )}
      >
        {code && (
          <Lines
            pane={frame.pane}
            colours={colours}
            rows={Math.min(rows, timeline.lines)}
            material={material}
          />
        )}
        {views && (
          <Views
            timeline={timeline}
            frame={frame}
            driven={driven}
            material={material}
            framed={framed}
          />
        )}
      </div>
      {words && <Words timeline={timeline} frame={frame} />}
    </div>
  )
}

/** The file: `code`'s frame (its bar, copy, face), the lines drawn here. */
function Lines({
  pane,
  colours,
  rows,
  material,
}: {
  pane: WalkthroughPane | null
  colours: readonly (CodeColours | null)[]
  rows: number
  material: "plain" | "glass"
}) {
  const scroller = React.useRef<HTMLDivElement>(null)
  const sheet = React.useRef<HTMLElement>(null)
  // The band's middle kept in the middle of the frame: from the line height and
  // the frame's height as the page draws them. The instant decides what is in
  // view, so the wheel never scrolls it (`overflow-y-hidden` below): over a scroll
  // stage the wheel is the page's, and the page's scroll is what moves the steps.
  const focus = pane?.focus ?? 0
  const path = pane?.path
  // biome-ignore lint/correctness/useExhaustiveDependencies: a new file is scrolled even when its line is the last one's
  React.useLayoutEffect(() => {
    const box = scroller.current
    const code = sheet.current
    if (!box || !code)
      throw new Error("walkthrough: the frame drew without its lines")
    const line = Number.parseFloat(getComputedStyle(code).lineHeight)
    if (!(line > 0))
      throw new Error(`walkthrough: a line ${line} px tall is no line`)
    const top =
      code.getBoundingClientRect().top -
      box.getBoundingClientRect().top +
      box.scrollTop
    box.scrollTop = top + focus * line - box.clientHeight / 2
  }, [focus, path])
  const caret = pane?.caret ?? null
  const selection = pane?.selection ?? []
  return (
    <CodeFrame
      source={pane?.source ?? ""}
      html={null}
      lang={pane?.lang ?? "text"}
      filename={pane?.path ?? ""}
      copy={pane !== null}
      material={material}
      scroller={scroller}
      className="min-w-0 [&_.ag-code-scroll]:overflow-y-hidden"
    >
      <pre style={{ opacity: pane?.opacity ?? 0 }}>
        <code
          ref={sheet}
          data-slot="walkthrough-lines"
          className="relative h-(--ag-walkthrough-rows)"
          style={
            {
              "--ag-walkthrough-rows": `${rows}lh`,
            } as React.CSSProperties
          }
        >
          {pane?.bands.map((b, i) => (
            <span
              // The bands are paired by place, step to step: the first travels
              // to the next first.
              // biome-ignore lint/suspicious/noArrayIndexKey: a band's identity is its order
              key={i}
              aria-hidden
              data-slot="walkthrough-band"
              className="line highlighted pointer-events-none absolute top-0 left-0 h-lh origin-top"
              style={{ transform: `translateY(${b.from}lh) scaleY(${b.size})` }}
            />
          ))}
          {pane?.rows.map((r, i) => (
            <Row
              // biome-ignore lint/suspicious/noArrayIndexKey: a row is its place; what it holds is drawn from the instant
              key={i}
              line={r.line}
              colours={colours[r.line.state] ?? null}
              open={r.open}
              sign={r.sign}
              caret={caret?.row === i ? caret.column : null}
              selected={selection.find((s) => s.row === i) ?? null}
            />
          ))}
        </code>
      </pre>
    </CodeFrame>
  )
}

/** Each character's place in its state, for a line that starts at one. */
const placesOf = (line: WalkthroughLine) =>
  typeof line.at === "number"
    ? Array.from(line.text, (_, i) => (line.at as number) + i)
    : line.at

/** One line: `code`'s `.line` (its padding, its gutter, a diff's sign and tint),
 *  as tall as it is open. */
const Row = React.memo(function Row({
  line,
  colours,
  open,
  sign,
  caret,
  selected,
}: {
  line: WalkthroughLine
  colours: CodeColours | null
  open: number
  sign: WalkthroughSign
  caret: number | null
  /** The columns selected on this line (from 0, end excluded): keys about to type
   *  over them. */
  selected: { from: number; to: number } | null
}) {
  const runs = React.useMemo(
    () => runsOf(line.text, colours, placesOf(line))[0] ?? [],
    [line, colours],
  )
  const parts: React.ReactNode[] = []
  let at = 0
  const ink = (text: string, run: (typeof runs)[number], key: string) =>
    run.ink ? (
      <span
        key={key}
        style={
          {
            "--shiki-light": run.ink.light,
            "--shiki-dark": run.ink.dark,
            fontStyle: run.ink.italic ? "italic" : undefined,
            fontWeight: run.ink.bold ? 700 : undefined,
          } as React.CSSProperties
        }
      >
        {text}
      </span>
    ) : (
      text
    )
  const bar = (
    <span
      key="caret"
      aria-hidden
      data-slot="walkthrough-caret"
      className="-mr-0.5 inline-block h-lh w-0.5 bg-current align-top"
    />
  )
  // Each run cut where the caret stands and where the selection starts and ends; a
  // selected piece under the editor's selection wash.
  const cuts = [caret, selected?.from, selected?.to].filter(
    (c): c is number => c !== null && c !== undefined,
  )
  for (const [n, run] of runs.entries()) {
    const end = at + run.text.length
    const here = [
      ...new Set([at, ...cuts.filter((c) => c > at && c < end), end]),
    ].sort((a, b) => a - b)
    for (let k = 0; k + 1 < here.length; k++) {
      const a = here[k] as number
      const b = here[k + 1] as number
      if (caret === a) parts.push(bar)
      const piece = ink(run.text.slice(a - at, b - at), run, `${n}.${k}`)
      parts.push(
        selected && a >= selected.from && b <= selected.to ? (
          <span
            key={`${n}.${k}s`}
            data-slot="walkthrough-selection"
            className="bg-[color-mix(in_oklab,currentColor_22%,transparent)]"
          >
            {piece}
          </span>
        ) : (
          piece
        ),
      )
    }
    at = end
  }
  if (caret !== null && caret >= at) parts.push(bar)
  return (
    <span
      data-slot="walkthrough-line"
      className={cn(
        "line relative block overflow-hidden",
        sign && `diff ${sign}`,
        SIGN_MOTION,
      )}
      style={{ height: `${open}lh`, opacity: open < 1 ? open : undefined }}
    >
      {parts}
    </span>
  )
})

/** The step's view: each step's own, kept by the steps after it that name none,
 *  crossfading as a step brings another. All laid in one cell, so the card is as
 *  tall as the tallest and never changes height. */
function Views({
  timeline,
  frame,
  driven,
  material,
  framed,
}: {
  timeline: WalkthroughTimed
  frame: WalkthroughFrame
  driven: boolean
  material: "plain" | "glass"
  framed: boolean
}) {
  const steps = timeline.steps
  const owner = (i: number) => {
    for (let j = i; j >= 0; j--) if (steps[j]?.view) return j
    return -1
  }
  const now = owner(frame.step)
  const was = frame.step > 0 ? owner(frame.step - 1) : now
  const ambient = frame.step + frame.p
  const layers = steps.map((s, j) => {
    if (!s.view) return null
    const shown =
      j === now
        ? now === was
          ? 1
          : frame.arrive
        : j === was
          ? 1 - frame.arrive
          : 0
    // A step behind the one on shows the file as it left it; the one on, as
    // last written now.
    const left = timeline.plans[j]?.written.at(-1)
    const path = timeline.plans[j]?.path
    const moment: WalkthroughMoment = {
      p: j === frame.step ? frame.p : j < frame.step ? 1 : 0,
      ambient,
      active: j === now,
      driven,
      written:
        j === frame.step
          ? frame.written
          : left && path
            ? { path, text: left.text }
            : null,
    }
    return (
      <div
        // biome-ignore lint/suspicious/noArrayIndexKey: a step's view is its step
        key={j}
        data-slot="walkthrough-view"
        inert={j !== now}
        aria-hidden={j !== now || undefined}
        className="col-start-1 row-start-1 w-full"
        style={{
          opacity: shown,
          visibility: shown === 0 ? "hidden" : undefined,
        }}
      >
        {s.view(moment)}
      </div>
    )
  })
  return framed ? (
    <Card
      material={material}
      bodyClassName="grid min-h-0 place-items-center p-(--card-pad)"
    >
      {layers}
    </Card>
  ) : (
    <div data-slot="walkthrough-views" className="grid min-w-0">
      {layers}
    </div>
  )
}

/** The step's title and words, the last ones fading as these arrive, all laid in
 *  one cell so nothing below moves between steps. */
function Words({
  timeline,
  frame,
}: {
  timeline: WalkthroughTimed
  frame: WalkthroughFrame
}) {
  return (
    <div data-slot="walkthrough-words" className="grid">
      {timeline.steps.map((s, i) => {
        const shown =
          i === frame.step
            ? i === 0
              ? 1
              : frame.arrive
            : i === frame.step - 1
              ? 1 - frame.arrive
              : 0
        return (
          <div
            // biome-ignore lint/suspicious/noArrayIndexKey: a step's words are its step
            key={i}
            inert={i !== frame.step}
            aria-hidden={i !== frame.step || undefined}
            className="col-start-1 row-start-1 space-y-2"
            style={{
              opacity: shown,
              visibility: shown === 0 ? "hidden" : undefined,
            }}
          >
            <StepWords step={s} />
          </div>
        )
      })}
    </div>
  )
}

function StepWords({
  step,
  told = false,
}: {
  step: WalkthroughSteps[number]
  /** Beside a pinned stage: the page's own text, at its sizes. */
  told?: boolean
}) {
  return (
    <>
      <h3 className={told ? TOLD_TITLE : TITLE}>
        {step.heading ?? step.title}
      </h3>
      {step.words && (
        <div className={cn(told ? TOLD_WORDS : WORDS, told && "mt-3")}>
          {step.words}
        </div>
      )}
    </>
  )
}

export interface WalkthroughScrollProps {
  timeline: WalkthroughTimed
  /** Its name, for assistive tech and the steps' picker. */
  label: string
  rows?: number
  /** Scroll per step, a CSS length (scroll-stage's `pace`). */
  pace?: string
  material?: "plain" | "glass"
  /** The views in a card of their own (`Walkthrough`'s `framed`). */
  framed?: boolean
  className?: string
}

/** The walkthrough told on the scroll: its words beside the pinned frame, each step
 *  an act of a scroll stage cueing its own chapter, so the frame plays the step as
 *  it arrives and waits on its last frame. Where the stage does not pin (a narrow
 *  window, reduced motion) the steps stand one under another, each its words and a
 *  still of its last frame. */
export function WalkthroughScroll({
  timeline,
  label,
  rows,
  pace = "90svh",
  material,
  framed,
  className,
}: WalkthroughScrollProps) {
  return (
    <ScrollStage
      acts={timeline.steps.length}
      pace={pace}
      aria-label={label}
      className={className}
      stacked={
        <div data-slot="walkthrough-steps" className="space-y-12">
          {timeline.steps.map((s, i) => (
            <section
              // biome-ignore lint/suspicious/noArrayIndexKey: a step is its place
              key={i}
              aria-label={s.title}
              className="space-y-4"
            >
              <div>
                <StepWords step={s} told />
              </div>
              <Walkthrough
                timeline={timeline}
                progress={walkthroughRest(timeline, i)}
                words={false}
                rows={rows}
                material={material}
                framed={framed}
              />
            </section>
          ))}
        </div>
      }
    >
      <Pinned
        timeline={timeline}
        label={label}
        rows={rows}
        material={material}
        framed={framed}
      />
    </ScrollStage>
  )
}

function Pinned({
  timeline,
  label,
  rows,
  material,
  framed,
}: Omit<WalkthroughScrollProps, "pace" | "className">) {
  const act = useAct()
  const { seek } = useScrollStage()
  return (
    <div className="grid w-full items-center gap-12 lg:grid-cols-[minmax(0,24rem)_minmax(0,1fr)]">
      <div className="space-y-8">
        <div className="grid">
          {timeline.steps.map((s, i) => (
            <Act
              // biome-ignore lint/suspicious/noArrayIndexKey: a step is its act
              key={i}
              index={i}
              inert={act !== i}
              aria-hidden={act !== i || undefined}
              className="col-start-1 row-start-1 opacity-0 transition-opacity duration-500 data-on:opacity-100 motion-reduce:transition-none"
            >
              <StepWords step={s} told />
            </Act>
          ))}
        </div>
        {/* One step of several, the one on stage lit: a dot each. A press is the
            page's navigation (it scrolls to the step), so the pointer leaves no
            focus behind on a dot, which the page's own scroll keys would then ring;
            Tab still reaches them, ringed. */}
        <ToggleGroup
          variant="dots"
          aria-label={label}
          value={String(act)}
          onValueChange={(v) => seek(Number(v))}
        >
          {timeline.steps.map((s, i) => (
            <ToggleGroupItem
              // biome-ignore lint/suspicious/noArrayIndexKey: a step is its act
              key={i}
              value={String(i)}
              onMouseDown={(e) => e.preventDefault()}
            >
              go to {s.title}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </div>
      <Playback
        playable={timeline.playable}
        cue={chapterSpan(timeline.playable, act)}
      >
        <Walkthrough
          timeline={timeline}
          words={false}
          rows={rows}
          material={material}
          framed={framed}
        />
      </Playback>
    </div>
  )
}
