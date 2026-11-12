// Brings the Cells engine and grammar into the site from this repository: the wasm build's
// JS wrapper and its module (rust/cells-wasm/js), and the TextMate grammar every editor
// gets (syntaxes/cells.tmLanguage.json). The site runs the same engine and colours with
// the same grammar as everything else. Re-run after the engine changes
// (`bun rust/cells-wasm/build.ts` there first).
//
// One change is made to the wrapper, and only this one: it takes its compiled module from
// lib/engine-module.ts instead of decoding the base64 it embeds, so a browser streams
// `/cells.wasm` (compressed on the wire, compiled while it arrives) and no page carries
// the module inside its JavaScript. The line it replaces must be there exactly: a wrapper
// that instantiates its module another way stops this script rather than being patched
// blind.
//
//   bun scripts/engine.ts

import { createHash } from "node:crypto"

/** The repository's root: the site lives in its `web/`. */
const cells = new URL("../..", import.meta.url).pathname.replace(/\/$/, "")

const INSTANTIATE = "const x = new WebAssembly.Instance(new WebAssembly.Module(decodeBase64(wasm)), {}).exports as unknown as Exports"
const IMPORT = 'import { wasm } from "./wasm.ts"'

const wrapper = (text: string) => {
  if (!text.includes(INSTANTIATE) || !text.includes(IMPORT))
    throw new Error("the wrapper no longer instantiates its module the way this script replaces: read rust/cells-wasm/js/cells.ts and update scripts/engine.ts")
  return text
    .replace(IMPORT, 'import { engineModule } from "@/lib/engine-module"')
    .replace(INSTANTIATE, "const x = new WebAssembly.Instance(engineModule(), {}).exports as unknown as Exports")
    .replace(/\nfunction decodeBase64\(s: string\)[^]*?\n}\n/, "\n")
}

const COPIES: [string, string, ((t: string) => string)?][] = [
  ["rust/cells-wasm/js/cells.ts", "lib/engine/cells.ts", wrapper],
  ["rust/cells-wasm/js/cells.wasm", "public/cells.wasm"],
  ["syntaxes/cells.tmLanguage.json", "lib/engine/cells.tmLanguage.json"],
]

const site = `${import.meta.dir}/..`
const manifest: Record<string, string> = {}
for (const [from, to, change] of COPIES) {
  const file = Bun.file(`${cells}/${from}`)
  if (!(await file.exists())) throw new Error(`${cells}/${from} does not exist: build the wasm first (bun rust/cells-wasm/build.ts)`)
  const bytes = await file.bytes()
  await Bun.write(`${site}/${to}`, change ? change(new TextDecoder().decode(bytes)) : bytes)
  manifest[to] = `${from} sha256:${createHash("sha256").update(bytes).digest("hex").slice(0, 16)}`
}
await Bun.write(`${site}/lib/engine/SOURCE.json`, `${JSON.stringify({ copied: new Date().toISOString(), files: manifest }, null, 2)}\n`)
for (const [to, from] of Object.entries(manifest)) console.log(`${to} ← ${from}`)
