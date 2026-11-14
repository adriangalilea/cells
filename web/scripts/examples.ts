// Every file the site shows, read by the engine, and what its act claims asserted: the
// problems the file has as the act leaves it, and the values it computes. An act that
// says something the engine does not do fails here, before anyone reads it.
//
//   bun scripts/examples.ts

import { createHash } from "node:crypto"
import { readFiles as readWith } from "@/lib/view"
import { hero, SPENDING_PATH, SPENDING_SHOWN, SPENDING_TEXT, WATCHED_CHARTS } from "@/lib/hero"
import { SCREENS } from "@/lib/hero-screens"
import { loadEngine } from "@/lib/engine-module"
import { codeTimeline } from "@/lib/code-session"
import { savedAt, textAt } from "@/lib/written"
import { editOf, story } from "@/lib/story"
import { PRODUCT } from "@/lib/product"
import { ONE_FILE, PROOFS, TRIP_FILES, TRIP_PROBLEM } from "@/lib/proofs"

const engine = await loadEngine()
const readFiles = (files: Parameters<typeof readWith>[1]) => readWith(engine, files)
const STORY = story(engine)
const HERO = hero(engine)

let failed = 0
const fail = (where: string, what: string) => {
  failed++
  console.error(`✗ ${where}: ${what}`)
}

for (const act of STORY) {
  const before = failed
  const t = readFiles([act.end, ...act.others])
  if (t.broken.length) fail(act.id, `its grammar breaks: ${t.broken.map((b) => b.why).join("; ")}`)
  // The file as the act finds it reads whole too.
  const was = readFiles([act.from, ...act.others])
  if (was.broken.length) fail(act.id, `before the act its grammar breaks: ${was.broken.map((b) => b.why).join("; ")}`)
  if (act.light && !t.columns.some((c) => c.name === act.light?.column)) fail(act.id, `no column ${act.light.column} to light`)
  const got = t.problems.map((p) => p.why)
  const want = act.expect.problems
  if (JSON.stringify(got) !== JSON.stringify(want)) fail(act.id, `problems ${JSON.stringify(got)}, expected ${JSON.stringify(want)}`)
  for (const [column, values] of Object.entries(act.expect.values ?? {})) {
    const have = t.values[column]
    if (JSON.stringify(have) !== JSON.stringify(values)) fail(act.id, `${column} is ${JSON.stringify(have)}, expected ${JSON.stringify(values)}`)
  }
  // As written between its edits, what the act shows on the way.
  const between = act.saves.slice(0, -1).map((text) => {
    const r = readFiles([{ name: act.end.name, text }, ...act.others])
    return [...r.broken.map((b) => b.why), ...r.problems.map((p) => p.why)]
  })
  if (JSON.stringify(between) !== JSON.stringify(act.expect.between ?? between.map(() => [])))
    fail(act.id, `between its edits ${JSON.stringify(between)}, expected ${JSON.stringify(act.expect.between ?? [])}`)
  console.log(`${failed > before ? "✗" : "✓"} ${act.id}: ${t.rows.length} rows, ${got.length} problems`)
}

// Played in the editor, every act's edit leaves its file as the act says, as typed and as
// written (the keys landed where they were meant to, and the last change was saved).
{
  let total = 0
  for (const act of STORY) {
    const edit = editOf(act)
    const t = codeTimeline(edit.files, edit.steps)
    total += t.total
    if (textAt(t, edit.path, t.total) !== act.end.text) fail(act.id, `played, the editor holds ${JSON.stringify(textAt(t, edit.path, t.total).slice(0, 80))}…`)
    if (savedAt(t, edit.path, t.total) !== act.end.text) fail(act.id, "played, its last change is never written")
  }
  console.log(`✓ the edits: ${STORY.length} acts, ${(total / 1000).toFixed(1)}s in the editor`)
}

// The landing's proofs: the check's line is the engine's on the folder it names, and every
// Cells file shown reads whole.
{
  const before = failed
  const t = readFiles([...TRIP_FILES].reverse())
  const said = t.problems.map((p) => p.why)
  if (said.join() !== TRIP_PROBLEM) fail("proof folder", `the engine says ${JSON.stringify(said)}, the page ${TRIP_PROBLEM}`)
  for (const [name, p] of Object.entries(PROOFS)) {
    if (p.lang !== "cells") continue
    const r = readFiles([{ name: `${name}${PRODUCT.extension}`, text: p.text }])
    if (r.broken.length || r.problems.length)
      fail(`proof ${name}`, [...r.broken.map((b) => b.why), ...r.problems.map((x) => x.why)].join("; "))
  }
  const one = readFiles([ONE_FILE, TRIP_FILES[0] as (typeof TRIP_FILES)[number]])
  if (one.broken.length || one.problems.length)
    fail("proof one file", [...one.broken.map((b) => b.why), ...one.problems.map((x) => x.why)].join("; "))
  if (failed === before) console.log(`✓ proofs: the check's line is the engine's, every file shown reads whole`)
}

// The hero: its file reads whole at every moment and declares the four charts the page
// names; every category is drawn in its tint, as the app's window shows it; the loop ends
// on the file it began with; a moment's only problems are the new row's cells not typed
// yet; neither chart's scale moves at any moment, as the app's chart view draws it from
// the engine's summaries and as the terminal drew it (every frame's axis labels and places
// the first's, at every size); and every text the hero writes has its frame, drawn from
// it by `pnpm charts`, in the hero's order.
{
  const before = failed
  const window = readFiles([{ name: SPENDING_PATH, text: SPENDING_TEXT }])
  const tints = window.faces.category ?? []
  if (tints.length !== window.rows.length || tints.some((f) => !f?.tint)) fail("hero", "a category is drawn with no tint")
  const titles = ["Spending by category", "Spent", "Budget", "Spent against budget"]
  /** A text's problems, and the scales of the charts the hero shows. */
  const moment = (text: string) => {
    const r = engine.read(text, { at: SPENDING_PATH, files: {} })
    try {
      if (r.broken.length) fail("hero", r.broken.map((b) => b.why).join("; "))
      const charts = r.doc.charts()
      if (JSON.stringify(charts.map((c) => c.title)) !== JSON.stringify(titles)) fail("hero", `the file declares ${JSON.stringify(charts.map((c) => c.title))}`)
      return { problems: r.problems.map((p) => p.why), scales: WATCHED_CHARTS.map((n) => {
          const s = r.doc.summary(n - 1).scale
          if (!s) throw new Error(`hero: chart ${n} is drawn on no scale`)
          return s
        }),
      }
    } finally {
      r.doc.free()
    }
  }
  // the file as the charts section shows it reads whole, its four charts declared
  if (moment(SPENDING_SHOWN).problems.length) fail("hero", "the file as the charts section shows it has problems")
  const first = moment(SPENDING_TEXT)
  if (first.problems.length) fail("hero", `the file as found has problems: ${first.problems.join("; ")}`)
  if (HERO.states.at(-1) !== SPENDING_TEXT) fail("hero", "the loop does not end on the file it began with")
  // the row typed after the last, as a problem names it (rows from 1)
  const newRow = window.rows.length + 1
  for (const [k, text] of HERO.states.entries()) {
    const m = moment(text)
    const stray = m.problems.filter((p) => !p.startsWith(`row ${newRow}, `))
    if (stray.length) fail("hero", `edit ${k + 1} leaves ${stray.join("; ")}`)
    if (JSON.stringify(m.scales) !== JSON.stringify(first.scales))
      fail("hero", `edit ${k + 1} moves a chart's scale: ${JSON.stringify(m.scales)}, first ${JSON.stringify(first.scales)}`)
  }
  const hash = (text: string) => createHash("sha256").update(text).digest("hex").slice(0, 16)
  const wanted = HERO.texts.map(hash)
  /** What of a frame tells its scale: each gridline's label by its row, and the places'
   *  lines. */
  const axis = (rows: [string, number][][]) =>
    rows.flatMap((runs, r) => {
      const line = runs.map(([t]) => t).join("")
      const label = /^│\s+(\d+(?: [A-Z]{3})?)\s+─/.exec(line)?.[1]
      if (label) return [`${r}: ${label}`]
      return /May 26/.test(line) ? [`${r}: ${line.trim()}`] : []
    })
  for (const [at, size] of Object.entries(SCREENS.sizes)) {
    if (JSON.stringify(size.frames.map((f) => f.source)) !== JSON.stringify(wanted))
      fail("hero", `the ${at} terminal's frames were drawn from other texts than the hero writes: pnpm charts`)
    const firstAxis = axis((size.frames[0] as (typeof size.frames)[number]).rows)
    if (firstAxis.length < 4) fail("hero", `the ${at} terminal's first frame shows no scale`)
    // the terminal labels the engine's steps, the app's, never steps of its own: each
    // label a step of a chart's scale, and every scale's ends labelled
    const labelled = firstAxis.flatMap((l) => /^\d+: (\d+)/.exec(l)?.[1] ?? []).map(Number)
    const onScale = (v: number) => first.scales.some((s) => v >= s.lo && v <= s.hi && Number.isInteger((v - s.lo) / s.step))
    if (!labelled.every(onScale)) fail("hero", `the ${at} terminal labels ${labelled.join(", ")}, not all steps of ${first.scales.map((s) => `${s.lo}..${s.hi} by ${s.step}`).join(", ")}`)
    if (!first.scales.every((s) => labelled.includes(s.lo) && labelled.includes(s.hi))) fail("hero", `the ${at} terminal's scale is not the engine's: ${labelled.join(", ")}`)
    for (const [k, f] of size.frames.entries())
      if (JSON.stringify(axis(f.rows)) !== JSON.stringify(firstAxis)) fail("hero", `the ${at} terminal's frame ${k + 1} moves a scale`)
  }
  if (failed === before)
    console.log(
      `✓ hero: ${HERO.states.length} edits, the loop back on the file as found, every scale the first's (${first.scales.map((s) => `${s.lo}..${s.hi} by ${s.step}`).join(", ")}) in the app and in ${Object.keys(SCREENS.sizes).length} terminals, ${wanted.length} frames each from its text`,
    )
}

if (failed) {
  console.error(`${failed} claim(s) the engine does not back`)
  process.exit(1)
}
