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
import { placeBroken } from "@/lib/spans"
import { editOf, story } from "@/lib/story"

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

// A header cell half typed: the table still reads, that column as text, and the break is
// placed on the characters the engine names.
{
  const half = HERO.checkpoints.typo.money.replace("to:accounts.account", "to:accounts.a")
  const t = readFiles(heroFiles({ money: half, accounts: HERO.checkpoints.typo.accounts }))
  const placed = placeBroken(half, t.broken)
  const line = half.split("\n")[(placed[0]?.line ?? 1) - 1] ?? ""
  const marked = placed[0] ? line.slice(placed[0].from, placed[0].to) : ""
  if (t.rows.length !== 3 || t.broken.length !== 1 || !t.columns.some((c) => c.name === "to") || !marked.includes("accounts.a"))
    fail("hero header", `a half-typed header: ${t.rows.length} rows, breaks ${JSON.stringify(t.broken)}, marked "${marked}"`)
  else console.log(`✓ hero header: half typed, the table reads, "${marked}" marked: ${t.broken[0]?.why}`)
}

// The edit as played ends where the checkpoints say it does.
const played = codeTimeline(HERO.files, HERO.steps)
const end = played.total
if (textAt(played, HERO.money, end) !== HERO.checkpoints.fixed.money || textAt(played, HERO.accounts, end) !== HERO.checkpoints.fixed.accounts)
  fail("hero edit", "the played edit does not end on the fixed checkpoint")
else console.log("✓ hero edit: the played edit ends on the fixed checkpoint")

// What the Cells window reads is the file as written: at every save of the edit, both
// files read whole (no half-typed header, no value computed from half a formula).
{
  let saves = 0
  for (const [i, s] of played.steps.entries()) {
    if (s.kind !== "save") continue
    saves++
    const ms = played.ends[i] as number
    const t = readFiles([
      { name: HERO.money, text: savedAt(played, HERO.money, ms) },
      { name: HERO.accounts, text: savedAt(played, HERO.accounts, ms) },
    ])
    if (t.broken.length) fail("hero saves", `save ${saves} breaks: ${t.broken.map((b) => b.why).join("; ")}`)
  }
  console.log(`✓ hero saves: the ${saves} saved states all read whole`)
}

// The terminal's failing check is the CLI's line for the typo: the file's name, then the
// engine's problem.
const typo = readFiles(heroFiles(HERO.checkpoints.typo)).problems.map((p) => `${HERO.money} ${p.why}`)
if (JSON.stringify(typo) !== JSON.stringify([HERO.failing])) fail("hero terminal", `prints ${JSON.stringify([HERO.failing])}, the engine says ${JSON.stringify(typo)}`)
else console.log("✓ hero terminal: the check's line is the engine's")

if (failed) {
  console.error(`${failed} claim(s) the engine does not back`)
  process.exit(1)
}
