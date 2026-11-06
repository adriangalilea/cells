"use client"

// THE STORY on ui's walkthrough: one file built act by act (lib/story), a step per act,
// pinned while the page scrolls (WalkthroughScroll), the step's words beside it. Its view
// is the editor and the app, the hero's own: Helix in a terminal (ui's terminal-editor)
// playing the act's edit with the engine as its language server (the problem marked in
// the gutter, undercurled and said at the line's end), and the Cells window reading each
// write, what the act is about lit once it has played. Where the stage does not pin (a
// narrow window, reduced motion) the steps stand one under another, each at rest.

import * as React from "react"
import { CellsWindow, FileName } from "@/components/cells-window"
import { ACCENT } from "@/components/hero"
import { Playhead } from "@/components/ui/playback"
import { Terminal } from "@/components/ui/terminal"
import { TerminalEditor } from "@/components/ui/terminal-editor"
import { type WalkthroughMoment, type WalkthroughSteps, WalkthroughScroll } from "@/components/ui/walkthrough"
import { useCellsDiagnose } from "@/components/use-cells-diagnose"
import { useEngine } from "@/components/use-engine"
import { codeTimeline } from "@/lib/code-session"
import { editOf, plain, type StoryAct, story } from "@/lib/story"
import { walkthroughTimeline } from "@/lib/walkthrough-session"
import { savedAt } from "@/lib/written"

/** One act, played: Helix on its edit at the step's progress, the Cells window on the file
 *  as Helix last wrote it. `rows`, the editor's: the same for every act. */
function ActView({ act, p, rows }: { act: StoryAct; p: number; rows: number }) {
  const edit = React.useMemo(() => editOf(act), [act])
  const timeline = React.useMemo(() => codeTimeline(edit.files, edit.steps), [edit])
  const diagnose = useCellsDiagnose()
  const ms = p * timeline.total
  const text = savedAt(timeline, edit.path, ms)
  return (
    <div className="grid gap-4">
      <Playhead progress={p}>
        <Terminal alt="" accent={ACCENT} size="clamp(11px,0.95vw,13.5px)">
          <TerminalEditor files={edit.files} steps={edit.steps} rows={rows} softWrap bufferline="multiple" diagnose={diagnose} />
        </Terminal>
      </Playhead>
      <CellsWindow name={edit.path} text={text} others={act.others} light={p >= 1 ? act.light : undefined} className="h-[13rem]" />
    </div>
  )
}

/** What a file holds, in told words: set as code; a file's name or an extension with the
 *  extension lit, what tells a CSV from a Cells file. */
function Code({ text }: { text: string }) {
  return (
    <code className="font-mono text-[0.92em] text-foreground">
      {/^[\w-]*\.[a-z]+$/.test(text) ? <FileName name={text} /> : text}
    </code>
  )
}

/** Told words, what stands between backticks set as code. */
function Told({ text }: { text: string }) {
  return text.split("`").map((part, i) => (i % 2 ? <Code key={i} text={part} /> : part))
}

/** A step per act. */
function stepsOf(acts: StoryAct[]): WalkthroughSteps {
  // The editor's rows: the longest file the story writes, its imports' bufferline, and
  // room under the last line for the header soft-wrapped onto a second and a third row.
  const rows = Math.max(...acts.map((a) => a.end.text.split("\n").length)) + 4
  return acts.map((act) => {
    const edit = editOf(act)
    return {
      title: plain(act.lead),
      heading: <Told text={act.lead} />,
      words: act.rest && (
        <p>
          <Told text={act.rest} />
        </p>
      ),
      // The step plays as long as its edit: the editor's clock is the step's.
      play: codeTimeline(edit.files, edit.steps).total,
      view: ({ p }: WalkthroughMoment) => <ActView act={act} p={p} rows={rows} />,
    }
  })
}

export function Story() {
  const engine = useEngine()
  const timeline = React.useMemo(() => walkthroughTimeline(stepsOf(story(engine))), [engine])
  return (
    <section aria-label="how it works" className="relative">
      {/* A scrim over the ground for the story's whole stage, fading in and out at its
          ends: the words and the windows read on a calm ground, the facets still there. */}
      <div aria-hidden className="pointer-events-none absolute inset-0 -z-10 bg-background/80 [mask-image:linear-gradient(transparent,black_20svh,black_calc(100%-20svh),transparent)]" />
      <div className="mx-auto max-w-6xl px-6">
        <WalkthroughScroll timeline={timeline} label="one file, a step at a time" framed={false} />
      </div>
    </section>
  )
}
