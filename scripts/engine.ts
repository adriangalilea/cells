// Brings the Cells engine and grammar into the site from a Cells checkout: the wasm
// build's JS wrapper and its module (rust/cells-wasm/js), and the TextMate grammar every
// editor gets (syntaxes/cells.tmLanguage.json). Copied, never edited here: the site runs
// the same engine and colours with the same grammar as everything else. Re-run after the
// engine changes (`bun rust/cells-wasm/build.ts` there first).
//
//   CELLS=~/Developer/tcsv bun scripts/engine.ts

import { createHash } from "node:crypto"

const cells = process.env.CELLS
if (!cells) throw new Error("CELLS must name a Cells checkout")

const COPIES = [
  ["rust/cells-wasm/js/cells.ts", "lib/engine/cells.ts"],
  ["rust/cells-wasm/js/wasm.ts", "lib/engine/wasm.ts"],
  ["syntaxes/cells.tmLanguage.json", "lib/engine/cells.tmLanguage.json"],
] as const

const site = `${import.meta.dir}/..`
const manifest: Record<string, string> = {}
for (const [from, to] of COPIES) {
  const file = Bun.file(`${cells}/${from}`)
  if (!(await file.exists())) throw new Error(`${cells}/${from} does not exist: is CELLS a Cells checkout?`)
  const bytes = await file.bytes()
  await Bun.write(`${site}/${to}`, bytes)
  manifest[to] = `${from} sha256:${createHash("sha256").update(bytes).digest("hex").slice(0, 16)}`
}
await Bun.write(`${site}/lib/engine/SOURCE.json`, `${JSON.stringify({ from: cells, copied: new Date().toISOString(), files: manifest }, null, 2)}\n`)
for (const [to, from] of Object.entries(manifest)) console.log(`${to} ← ${from}`)
