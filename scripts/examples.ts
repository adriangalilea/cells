// Every file the site shows, read by the engine, and what its step claims asserted: the
// problems the first file has as the page ships, and the values it computes. A step that
// says something the engine does not do fails here, before anyone reads it.
//
//   bun scripts/examples.ts

import { readFiles as readWith } from "@/lib/cells"
import { loadEngine } from "@/lib/engine-module"
import { codeTimeline } from "@/lib/code-session"
import { HERO, textAt } from "@/lib/hero"
import { placeBroken } from "@/lib/spans"
import { STORY } from "@/lib/story"

const engine = await loadEngine()
const readFiles = (files: Parameters<typeof readWith>[1]) => readWith(engine, files)

let failed = 0
const fail = (where: string, what: string) => {
  failed++
  console.error(`✗ ${where}: ${what}`)
}

for (const step of STORY) {
  const before = failed
  const t = readFiles(step.files)
  if (t.broken.length) fail(step.id, `its grammar breaks: ${t.broken.map((b) => b.why).join("; ")}`)
  const got = t.problems.map((p) => p.why)
  const want = step.expect.problems
  if (JSON.stringify(got) !== JSON.stringify(want)) fail(step.id, `problems ${JSON.stringify(got)}, expected ${JSON.stringify(want)}`)
  for (const [column, values] of Object.entries(step.expect.values ?? {})) {
    const have = t.values[column]
    if (JSON.stringify(have) !== JSON.stringify(values)) fail(step.id, `${column} is ${JSON.stringify(have)}, expected ${JSON.stringify(values)}`)
  }
  console.log(`${failed > before ? "✗" : "✓"} ${step.id}: ${t.rows.length} rows, ${got.length} problems`)
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

// The running total follows its column's order, never the lines': the same rows in
// another order give each item the same running value.
{
  const step = STORY.find((s) => s.id === "formula")
  const file = step?.files[0]
  if (!file) throw new Error("examples: no formula step")
  const [header, ...rows] = file.text.trimEnd().split("\n")
  const shuffled = { name: file.name, text: `${header}\n${[...rows].reverse().join("\n")}\n` }
  const by = (t: ReturnType<typeof readFiles>) =>
    Object.fromEntries((t.values.item ?? []).map((item, i) => [String(item), t.values.running?.[i]]))
  const a = by(readFiles([file]))
  const b = by(readFiles([shuffled]))
  if (JSON.stringify(a) !== JSON.stringify(Object.fromEntries(Object.keys(a).map((k) => [k, b[k]]))))
    fail("formula order", `running depends on the lines: ${JSON.stringify(a)} vs ${JSON.stringify(b)}`)
  else console.log("✓ formula order: the running total follows on, not the lines")
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

// The terminal's failing check is the CLI's line for the typo: the file's name, then the
// engine's problem.
const typo = readFiles(heroFiles(HERO.checkpoints.typo)).problems.map((p) => `${HERO.money} ${p.why}`)
if (JSON.stringify(typo) !== JSON.stringify([HERO.failing])) fail("hero terminal", `prints ${JSON.stringify([HERO.failing])}, the engine says ${JSON.stringify(typo)}`)
else console.log("✓ hero terminal: the check's line is the engine's")

if (failed) {
  console.error(`${failed} claim(s) the engine does not back`)
  process.exit(1)
}
