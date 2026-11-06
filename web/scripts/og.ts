// THE LINK PREVIEW: cellular.computer's card, `app/card.png` (lib/card), which
// app/opengraph-image.ts serves as the og:image of every page, with its alt. Drawn
// from the site's own parts through ui's: a `.cell` file in Helix as the hero shows it
// (ui's terminal-editor still, its accent, the engine as its language server), across
// the card, the product's name and subline (lib/product) in a row under it, on the
// dark theme's ground, the
// extension in its edge light (app/globals.css). The file is read by the engine and
// what the card claims is asserted: its totals stored beside their `=`, the one typo
// named on its row, nothing else. The PNG is kept in the tree; `--check` draws it again
// and fails when the bytes differ, so a card that no longer matches the site, or lies
// about the engine, never ships.
//
//   bun scripts/og.ts            write it
//   bun scripts/og.ts --check    pnpm check

import { strict as assert } from "node:assert"
import { readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"
// Cells coloured as every code view on the site colours it.
import "@/lib/cells-code"
import { ACCENT } from "@/components/hero"
import { CARD_CELL, CARD_FILE } from "@/lib/card"
import { cellsDiagnose } from "@/lib/diagnose"
import { loadEngine } from "@/lib/engine-module"
import { ogStackCard } from "@/lib/og"
import { geistFiles, ogFace, rasterizeOg } from "@/lib/og-still"
import { PRODUCT } from "@/lib/product"
import { editorStill, renderEditorSvg } from "@/lib/terminal-editor-screen"
import { readFiles, savedAs } from "@/lib/view"

const APP = join(import.meta.dir, "../app")
const PNG = join(import.meta.dir, "..", CARD_FILE)

const engine = await loadEngine()

// ── the file ──

const name = CARD_CELL
/** As typed: a type, a formula, and a typo in a cost. The engine writes the rest. */
const TYPED = `item,cost:EUR>=0,qty,total:EUR=cost * qty
fuel,84.20,1
groceries,96.7x,2
coffee,2.10,3
`
/** As saved: what the language server's formatting writes (lib/view `savedAs`). */
const text = savedAs(engine, { name, text: TYPED }, { name, text: TYPED }, [])
const read = readFiles(engine, [{ name, text }])
assert.deepEqual(read.problems.map((p) => p.why), ['row 2, cost: "96.7x" is not a number'], "the card's one problem")
assert.deepEqual(read.broken, [], "the file's grammar reads whole")
assert.deepEqual(read.values.total, [84.2, null, 6.3], "the totals, and none for the broken row")
assert.equal(
  text,
  `item,cost:EUR>=0,qty,total:EUR=cost * qty
fuel,84.20,1,84.20=
groceries,96.7x,2,=
coffee,2.10,3,6.30=
`,
  "each total stored beside its =",
)

// ── the editor: the file open, the caret on the typo ──

const TYPO = { line: 3, column: "groceries,96.7".length + 1 }
const screen = await editorStill({
  files: [{ path: name, text }],
  steps: [
    { kind: "open", path: name },
    { kind: "place", ...TYPO, delay: 300 },
  ],
  progress: 1,
  // The file's four lines and the status line: as many as the card's height takes with
  // the editor across its width.
  rows: 5,
  // The typo's row, its message after it, with room: the width the card's height
  // leaves the editor at five rows.
  columns: 66,
  diagnose: cellsDiagnose(engine),
  accent: ACCENT,
})
assert.equal(screen.rows[2]?.worst?.message, 'cost: "96.7x" is not a number', "helix says the engine's words")
assert.deepEqual(screen.status.counts, [{ severity: "error", n: 1 }])
assert.ok(
  screen.rows.every((r) => !r || r.runs.reduce((n, x) => n + x.text.length, 0) + screen.gutter <= screen.columns),
  "every line whole on the screen",
)
const still = renderEditorSvg(screen, { fontSize: 20 })

// ── the card ──

/** A colour of the dark theme, as app/globals.css declares it. */
function themed(name: string): string {
  const css = readFileSync(join(APP, "globals.css"), "utf8")
  const dark = /\.dark\s*\{([^}]*)\}/.exec(css)?.[1]
  if (!dark) throw new Error("og: app/globals.css has no .dark block")
  const m = new RegExp(`--${name}:\\s*oklch\\(([\\d.]+)\\s+([\\d.]+)\\s+([\\d.]+)\\)`).exec(dark)
  if (!m) throw new Error(`og: the dark theme's --${name} is no plain oklch()`)
  return oklchHex(Number(m[1]), Number(m[2]), Number(m[3]))
}

/** OKLCH as sRGB hex: OKLab, LMS, linear sRGB, gamma; clipped to the gamut. */
function oklchHex(l: number, c: number, h: number): string {
  const a = c * Math.cos((h * Math.PI) / 180)
  const b = c * Math.sin((h * Math.PI) / 180)
  const L = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const M = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const S = (l - 0.0894841775 * a - 1.291485548 * b) ** 3
  return `#${[
    4.0767416621 * L - 3.3077115913 * M + 0.2309699292 * S,
    -1.2684380046 * L + 2.6097574011 * M - 0.3413193965 * S,
    -0.0041960863 * L - 0.7034186147 * M + 1.707614701 * S,
  ]
    .map((v) => {
      const x = Math.min(1, Math.max(0, v))
      const s = x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055
      return Math.round(s * 255)
        .toString(16)
        .padStart(2, "0")
    })
    .join("")}`
}

const edge = themed("edge")
const [before, after] = PRODUCT.subline.split(PRODUCT.extension) as [string, string]
assert.ok(PRODUCT.subline.split(PRODUCT.extension).length === 2, "the subline names the extension once")
const fonts = geistFiles()
const card = ogStackCard({
  title: PRODUCT.name,
  line: [before, { text: PRODUCT.extension, ink: edge }, after],
  faces: { title: ogFace(fonts.sansSemiBold, "Geist"), line: ogFace(fonts.sans, "Geist") },
  look: { ground: themed("background"), ink: themed("foreground"), muted: themed("muted-foreground"), light: ACCENT },
  still,
})
const png = await rasterizeOg(card, Object.values(fonts))

if (process.argv.includes("--check")) {
  if (!readFileSync(PNG).equals(png)) {
    console.error(`✗ og: ${CARD_FILE} is not the card the site draws now: bun scripts/og.ts`)
    process.exit(1)
  }
  console.log(`✓ og: the card reads with the engine and is the one kept (${png.length} bytes)`)
} else {
  writeFileSync(PNG, png)
  console.log(`og: ${PNG} (${png.length} bytes)`)
}
