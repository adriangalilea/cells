"use client"

// YOUR TURN: the story's file as its last act left it (lib/story), in Helix taking typing,
// beside the Cells window reading each write. Every key is the edit's next step (ui's
// code-session), and a key that changes the text is written at once as the language
// server's formatting writes it (lib/written `saveAfter`), so a cost changed is a share
// and a total that follow it, and a name no friend has is flagged in both windows.

import * as React from "react"
import { CellsWindow } from "@/components/cells-window"
import { ACCENT } from "@/components/hero"
import { Playhead } from "@/components/ui/playback"
import { Terminal } from "@/components/ui/terminal"
import { TerminalEditor } from "@/components/ui/terminal-editor"
import { useCellsDiagnose } from "@/components/use-cells-diagnose"
import { useEngine } from "@/components/use-engine"
import { type CodeStep, codeTimeline } from "@/lib/code-session"
import "@/lib/cells-code"
import { story } from "@/lib/story"
import { saveAfter, savedAt } from "@/lib/written"

const NO_STEPS: CodeStep[] = []

export function Playground() {
  const engine = useEngine()
  const diagnose = useCellsDiagnose()
  const last = React.useMemo(() => story(engine).at(-1), [engine])
  if (!last) throw new Error("playground: the story has no acts")
  const path = last.end.name
  const files = React.useMemo(
    () => [{ path, text: last.end.text }, ...last.others.map((o) => ({ path: o.name, text: o.text }))],
    [last, path],
  )
  const opened = React.useMemo<CodeStep[]>(
    () => [...last.others.map((o) => ({ kind: "open" as const, path: o.name })), { kind: "open", path }],
    [last, path],
  )
  const [added, setAdded] = React.useState<CodeStep[]>(NO_STEPS)
  const steps = React.useMemo(() => [...opened, ...added], [opened, added])
  const step = React.useCallback(
    (s: CodeStep) =>
      setAdded((a) => {
        const next = [...a, s]
        return s.kind === "type" || s.kind === "replace" ? [...next, saveAfter(engine, files, [...opened, ...next])] : next
      }),
    [engine, files, opened],
  )
  const timeline = React.useMemo(() => codeTimeline(files, steps), [files, steps])
  const text = savedAt(timeline, path, timeline.total)
  const others = last.others.map((o) => ({ name: o.name, text: savedAt(timeline, o.name, timeline.total) }))
  return (
    <div className="grid gap-4">
      <Playhead progress={1}>
        <Terminal alt={`Helix on ${path}: type into it`} interactive accent={ACCENT} size="clamp(12px,1vw,14px)">
          <TerminalEditor
            files={files}
            steps={steps}
            rows={last.end.text.split("\n").length + 3}
            softWrap
            bufferline="always"
            diagnose={diagnose}
            onOpen={(p) => step({ kind: "open", path: p })}
            onEdit={step}
          />
        </Terminal>
      </Playhead>
      <CellsWindow name={path} text={text} others={others} className="h-[14rem]" />
    </div>
  )
}
