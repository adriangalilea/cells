// Every file the site shows, read by the engine, and what its step claims asserted: the
// problems the first file has as the page ships, and the values it computes. A step that
// says something the engine does not do fails here, before anyone reads it.
//
//   bun scripts/examples.ts

import { readFiles } from "@/lib/cells"
import { HERO } from "@/lib/hero"
import { STORY } from "@/lib/story"

let failed = 0
const fail = (where: string, what: string) => {
  failed++
  console.error(`✗ ${where}: ${what}`)
}

for (const step of STORY) {
  const before = failed
  const t = readFiles(step.files)
  if (t.error) {
    fail(step.id, `does not read: ${t.error}`)
    continue
  }
  const got = t.problems.map((p) => p.why)
  const want = step.expect.problems
  if (JSON.stringify(got) !== JSON.stringify(want)) fail(step.id, `problems ${JSON.stringify(got)}, expected ${JSON.stringify(want)}`)
  for (const [column, values] of Object.entries(step.expect.values ?? {})) {
    const have = t.values[column]
    if (JSON.stringify(have) !== JSON.stringify(values)) fail(step.id, `${column} is ${JSON.stringify(have)}, expected ${JSON.stringify(values)}`)
  }
  console.log(`${failed > before ? "✗" : "✓"} ${step.id}: ${t.rows.length} rows, ${got.length} problems`)
}

for (const [name, frame] of Object.entries(HERO.checkpoints)) {
  const t = readFiles([{ name: HERO.file, text: frame.text }])
  const got = t.error ? [`does not read: ${t.error}`] : t.problems.map((p) => p.why)
  if (JSON.stringify(got) !== JSON.stringify(frame.problems)) fail(`hero ${name}`, `problems ${JSON.stringify(got)}, expected ${JSON.stringify(frame.problems)}`)
  else console.log(`✓ hero ${name}: ${got.length} problems`)
}

// The terminal's failing check is the CLI's line for the typo: the file's name, then the
// engine's problem.
const typo = readFiles([{ name: HERO.file, text: HERO.checkpoints.typo.text }]).problems.map((p) => `${HERO.file} ${p.why}`)
if (JSON.stringify(typo) !== JSON.stringify([HERO.failing])) fail("hero terminal", `prints ${JSON.stringify([HERO.failing])}, the engine says ${JSON.stringify(typo)}`)
else console.log("✓ hero terminal: the check's line is the engine's")

if (failed) {
  console.error(`${failed} claim(s) the engine does not back`)
  process.exit(1)
}
