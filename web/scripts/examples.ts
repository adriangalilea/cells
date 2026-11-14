// Every file the site shows, read by the engine, and what its act claims asserted: the
// problems the file has as the act leaves it, and the values it computes. An act that
// says something the engine does not do fails here, before anyone reads it.
//
//   bun scripts/examples.ts

import { createHash } from "node:crypto"
import { readFiles as readWith } from "@/lib/view"
import { DINNER, hero, SPENDING_PATH, SPENDING_SHOWN, WATCHED_CHARTS } from "@/lib/hero"
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
// names; every category is drawn in its tint, as the app's window shows it; the typo puts
// May's spending over May's budget and the fix takes it back under, as the engine sums
// them (chart 4's two layers); and every text the hero writes has the terminal's frame
// drawn from it by `pnpm charts`, in the hero's order.
{
  const before = failed
  const scene = HERO
  const window = readFiles([{ name: SPENDING_PATH, text: scene.checkpoints.found }])
  const tints = window.faces.category ?? []
  if (tints.length !== window.rows.length || tints.some((f) => !f?.tint)) fail("hero", "a category is drawn with no tint")
  const titles = ["Spending by category", "Spent", "Budget", "Spent against budget"]
  const may = (text: string) => {
    const r = engine.read(text, { at: SPENDING_PATH, files: {} })
    try {
      if (r.broken.length || r.problems.length) fail("hero", [...r.broken.map((b) => b.why), ...r.problems.map((p) => p.why)].join("; "))
      const charts = r.doc.charts()
      if (JSON.stringify(charts.map((c) => c.title)) !== JSON.stringify(titles)) fail("hero", `the file declares ${JSON.stringify(charts.map((c) => c.title))}`)
      const layers = r.doc.summary(WATCHED_CHARTS[1] - 1).layers
      const [spent, budget] = layers.map((l) => Number(l.values[l.places.indexOf("2026-05-01")]?.[0]))
      if (layers.length !== 2 || !Number.isFinite(spent) || !Number.isFinite(budget)) fail("hero", "chart 4 is not May's spent and budget")
      return { spent: spent as number, budget: budget as number }
    } finally {
      r.doc.free()
    }
  }
  // the file as the charts section shows it reads whole, its four charts declared
  may(SPENDING_SHOWN)
  const found = may(scene.checkpoints.found)
  const typo = may(scene.checkpoints.typo)
  const fixed = may(scene.checkpoints.fixed)
  if (!(found.spent <= found.budget)) fail("hero", `May is over budget before the typo: ${found.spent} of ${found.budget}`)
  if (!(typo.spent > typo.budget)) fail("hero", `the typo leaves May under budget: ${typo.spent} of ${typo.budget}`)
  if (!(fixed.spent <= fixed.budget)) fail("hero", `the fix leaves May over budget: ${fixed.spent} of ${fixed.budget}`)
  const rows = engine.read(scene.checkpoints.typo, { at: SPENDING_PATH, files: {} })
  try {
    const amount = rows.columns.find((c) => c.name === "amount")
    if (amount?.decimal(DINNER.row) !== DINNER.typo) fail("hero", `the typo's row holds ${amount?.decimal(DINNER.row)}, not ${DINNER.typo}`)
  } finally {
    rows.doc.free()
  }
  const hash = (text: string) => createHash("sha256").update(text).digest("hex").slice(0, 16)
  const drawn = SCREENS.frames.map((f) => f.source)
  const wanted = scene.texts.map(hash)
  if (JSON.stringify(drawn) !== JSON.stringify(wanted)) fail("hero", `the terminal's frames were drawn from other texts than the scene writes: pnpm charts`)
  if (failed === before)
    console.log(`✓ hero: May ${found.spent} of ${found.budget}, ${typo.spent} with the typo, ${fixed.spent} fixed; ${drawn.length} frames, each from its text`)
}

if (failed) {
  console.error(`${failed} claim(s) the engine does not back`)
  process.exit(1)
}
