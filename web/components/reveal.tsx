"use client"

// THE REVEAL's scenes, each one tool shown doing what it is good at, in the app (ui's
// table-session, every commit the engine's): a database, two files linked and a name
// that is no friend caught where it is typed; a script, a table that gains the day's
// rate every evening with no hand on it. The spreadsheet is the window the hero let go
// of, beside them (components/hero `Landing`). Each plays in view and round again.

import * as React from "react"
import { CellsWindow } from "@/components/cells-window"
import { Playback, usePlayhead } from "@/components/ui/playback"
import { useEngine } from "@/components/use-engine"
import { PRODUCT } from "@/lib/product"
import { sceneOf, type TableFile, type TableStep, type TableTimeline, tableFrameAt, tableTimeline } from "@/lib/table-session"
import { type Engine, writeCell } from "@/lib/view"

const ext = PRODUCT.extension

const PEOPLE: TableFile = {
  path: `people${ext}`,
  text: "name:text!,email:email\nana,ana@example.com\nben,ben@example.com\ncarla,carla@example.com\ndan,dan@example.com\n",
}
const TRIP: TableFile = {
  path: `lisbon${ext}`,
  text: `---\nimport people from "./${PEOPLE.path}"\n---\nitem,paid:people.name?,cost:EUR>=0\nflights,ana,412.00\napartment,ben,540.00\n`,
}
/** The dinner typed, paid by a name no friend has. */
const LINKED: TableStep[] = [
  { kind: "select", row: 2, column: "item", delay: 700 },
  { kind: "type", text: "dinner" },
  { kind: "select", row: 2, column: "cost", delay: 200 },
  { kind: "type", text: "96.75" },
  { kind: "select", row: 2, column: "paid", delay: 300 },
  { kind: "type", text: "karla" },
  { kind: "select", row: 2, column: "paid", delay: 2400 },
  { kind: "type", text: "carla" },
]

const RATES = (rows: string[]) => ({ path: `rates${ext}`, text: `on:date!,eur:num\n${rows.join("\n")}\n` })
const DAYS = ["2026-11-03,0.0061", "2026-11-04,0.0062", "2026-11-05,0.0060", "2026-11-06,0.0063"]
/** Each evening's run writes the day's rate into the table, no hand on it. */
const CURRENT: TableStep[] = DAYS.slice(1).map((_, i) => ({
  kind: "written" as const,
  path: RATES([]).path,
  text: RATES(DAYS.slice(0, i + 2)).text,
  delay: 1500,
}))

/** A scene's timeline, its commits the engine's: made once per engine. */
const timelines = new WeakMap<Engine, { linked: TableTimeline; current: TableTimeline }>()
function useTimelines() {
  const engine = useEngine()
  let t = timelines.get(engine)
  if (!t) {
    const write = (f: TableFile, c: { row: number; column: string }, v: string, o: readonly TableFile[]) => writeCell(engine, f, c, v, o)
    t = {
      linked: tableTimeline([TRIP, PEOPLE], LINKED, write),
      current: tableTimeline([RATES(DAYS.slice(0, 1))], CURRENT, write),
    }
    timelines.set(engine, t)
  }
  return t
}

/** The held last frame, ms, before a scene starts again. */
const HOLD_MS = 2200

function Played({ timeline, children }: { timeline: TableTimeline; children: (ms: number) => React.ReactNode }) {
  const total = timeline.total + HOLD_MS
  return (
    <Playback playable={{ duration: total, chapters: timeline.playable.chapters }} playing>
      <AtPlayhead total={total} timeline={timeline}>
        {children}
      </AtPlayhead>
    </Playback>
  )
}

function AtPlayhead({ total, timeline, children }: { total: number; timeline: TableTimeline; children: (ms: number) => React.ReactNode }) {
  const ms = usePlayhead(undefined).at * total
  return children(Math.min(ms, timeline.total))
}

/** A database: the trip's costs, each paid by one of the friends `people` lists. */
export function LinkedScene() {
  const { linked } = useTimelines()
  return (
    <Played timeline={linked}>
      {(ms) => {
        const f = tableFrameAt(linked, ms)
        const people = { name: PEOPLE.path, text: f.texts[PEOPLE.path] as string }
        return (
          <div className="grid gap-3 sm:grid-cols-[1.4fr_1fr]">
            {/* room for its rows as they are typed and its prelude under them */}
            <CellsWindow name={TRIP.path} text={f.texts[TRIP.path] as string} others={[people]} scene={sceneOf(f, TRIP.path)} className="h-[14rem]" />
            <CellsWindow name={people.name} text={people.text} className="h-[14rem]" />
          </div>
        )
      }}
    </Played>
  )
}

/** A script: the rates table, a row more every evening. */
export function CurrentScene() {
  const { current } = useTimelines()
  const path = RATES([]).path
  return (
    <Played timeline={current}>
      {(ms) => {
        const f = tableFrameAt(current, ms)
        return <CellsWindow name={path} text={f.texts[path] as string} className="h-[12rem]" />
      }}
    </Played>
  )
}
