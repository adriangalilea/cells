// Every file the site shows, read by the engine, and what its act claims asserted: the
// problems the file has as the act leaves it, and the values it computes. An act that
// says something the engine does not do fails here, before anyone reads it.
//
//   bun scripts/examples.ts

import { readFiles as readWith } from "@/lib/view"
import { loadEngine } from "@/lib/engine-module"
import { codeTimeline } from "@/lib/code-session"
import { hero } from "@/lib/hero"
import { savedAt, textAt } from "@/lib/written"
import { tableFrameAt } from "@/lib/table-session"
import { editOf, story } from "@/lib/story"
import { PRODUCT } from "@/lib/product"
import { PROOFS, TRIP_FILES, TRIP_PROBLEM } from "@/lib/proofs"

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

const heroFiles = (f: { money: string; accounts: string }) => [
  { name: HERO.money, text: f.money },
  { name: HERO.accounts, text: f.accounts },
]
for (const [name, frame] of Object.entries(HERO.checkpoints)) {
  const t = readFiles(heroFiles(frame))
  const got = [...t.broken.map((b) => b.why), ...t.problems.map((p) => p.why)]
  if (JSON.stringify(got) !== JSON.stringify(frame.problems)) fail(`hero ${name}`, `problems ${JSON.stringify(got)}, expected ${JSON.stringify(frame.problems)}`)
  else console.log(`✓ hero ${name}: ${got.length} problems`)
}

// The VAT is there as the amount lands: the new row's, computed and stored.
{
  const t = readFiles(heroFiles(HERO.checkpoints.vat))
  const vat = t.columns.find((c) => c.name === "vat")
  const got = vat ? t.rows[2]?.[t.columns.indexOf(vat)]?.shown : undefined
  if (got !== "0.66") fail("hero vat", `the new row's vat is ${got}, the hero shows 0.66`)
  else console.log("✓ hero vat: 3.80's VAT computed as it lands, 0.66")
}

// Every frame the windows draw reads whole: no commit leaves a file the app cannot read.
{
  const t = HERO.timeline
  let commits = 0
  for (const p of t.plans) {
    if (p.step.kind !== "type") continue
    commits++
    const f = tableFrameAt(t, p.end + 1)
    const r = readFiles(heroFiles({ money: f.texts[HERO.money] as string, accounts: f.texts[HERO.accounts] as string }))
    if (r.broken.length) fail("hero commits", `commit ${commits} breaks: ${r.broken.map((b) => b.why).join("; ")}`)
  }
  console.log(`✓ hero commits: the ${commits} commits all read whole`)
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
  if (failed === before) console.log(`✓ proofs: the check's line is the engine's, every file shown reads whole`)
}

if (failed) {
  console.error(`${failed} claim(s) the engine does not back`)
  process.exit(1)
}
