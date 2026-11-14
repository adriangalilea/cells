// THE HERO TERMINAL'S FRAMES (lib/hero): the screens the real `cells chart … --watch`
// draws, never a drawing of them. This checkout's `cells` is built, run in a
// pseudo-terminal of a fixed size (a true colour terminal, as Ghostty is, that answers no
// cell border query, so the frame and gridlines are box drawing), on a folder holding the
// hero's file as found; each text the hero writes is then written to that file, and the
// screen the program redraws is read once it settles. Its bytes are read as a terminal
// reads them (the cursor moved, styles set, characters written), so a frame is the screen
// a person would see, cell by cell. `lib/hero-screens.ts` keeps them, each beside the hash
// of the text it was drawn from; scripts/examples.ts fails when a text of the hero has no
// frame drawn from it.
//
//   bun scripts/charts.ts                write them
//   CELLS=<a cells binary> bun scripts/charts.ts

import { createHash } from "node:crypto"
import { mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { type ScreenColor, type ScreenStyle, SCREEN } from "@/lib/chart-screen"
import { loadEngine } from "@/lib/engine-module"
import { hero, SPENDING_PATH, WATCH_COMMAND } from "@/lib/hero"

const repo = join(import.meta.dir, "../..")

/** This checkout's `cells`, built now, unless one is named. */
function cellsBinary(): string {
  const named = process.env.CELLS
  if (named) return named
  const built = Bun.spawnSync(["cargo", "build", "-p", "cells-cli"], { cwd: join(repo, "rust"), stdout: "inherit", stderr: "inherit" })
  if (built.exitCode !== 0) throw new Error(`charts: cargo build -p cells-cli exited ${built.exitCode}`)
  return join(repo, "rust/target/debug/cells")
}

// ── a terminal's screen, as the program's bytes leave it ──

interface Cell {
  ch: string
  style: ScreenStyle
}

const PLAIN: ScreenStyle = {}

/** What an SGR sequence's codes make of `style`: what crossterm writes for ratatui (reset,
 *  bold, dim, the named colours, 256 and true colour, fore and back, the underline's colour
 *  reset). Any other code
 *  screams: a frame holds only what it can name. */
function styled(style: ScreenStyle, params: string): ScreenStyle {
  const codes = (params || "0").split(";").map(Number)
  let s = { ...style }
  const colour = (i: number): [ScreenColor, number] => {
    if (codes[i + 1] === 5) return [codes[i + 2] as number, 2]
    if (codes[i + 1] === 2) {
      const hex = codes.slice(i + 2, i + 5).map((c) => c.toString(16).padStart(2, "0"))
      return [`#${hex.join("")}`, 4]
    }
    throw new Error(`charts: SGR ${codes[i]} with no colour after it (${params})`)
  }
  for (let i = 0; i < codes.length; i++) {
    const c = codes[i] as number
    if (c === 0) s = {}
    else if (c === 1) s.bold = true
    else if (c === 2) s.dim = true
    else if (c === 22) {
      delete s.bold
      delete s.dim
    } else if (c >= 30 && c <= 37) s.fg = c - 30
    else if (c >= 90 && c <= 97) s.fg = c - 90 + 8
    else if (c >= 40 && c <= 47) s.bg = c - 40
    else if (c >= 100 && c <= 107) s.bg = c - 100 + 8
    // the underline's colour reset, which crossterm writes with every style: nothing here
    // is underlined
    else if (c === 59) continue
    else if (c === 39) delete s.fg
    else if (c === 49) delete s.bg
    else if (c === 38 || c === 48) {
      const [value, used] = colour(i)
      if (c === 38) s.fg = value
      else s.bg = value
      i += used
    } else throw new Error(`charts: SGR ${c} is not drawn (${params})`)
  }
  return s
}

/** The screen of `cols` by `rows` the bytes leave, from the moment the program enters
 *  the alternate screen (none before it has): cursor moves, styles and printed characters.
 *  Escapes that change nothing drawn (modes set and reset, a query asked) pass; any other
 *  screams. */
function screenOf(bytes: string, cols: number, rows: number): Cell[][] | null {
  const enter = bytes.lastIndexOf("\x1b[?1049h")
  if (enter < 0) return null
  const blank = () => Array.from({ length: cols }, () => ({ ch: " ", style: PLAIN }))
  const grid = Array.from({ length: rows }, blank)
  let r = 0
  let c = 0
  let style = PLAIN
  // biome-ignore lint/suspicious/noControlCharactersInRegex: a terminal's bytes are read
  const token = /\x1b\[([0-9;?]*)([A-Za-z])|\x1bP[^\x1b]*\x1b\\|(\r|\n)|([^\x1b\r\n])/gsu
  for (const m of bytes.slice(enter).matchAll(token)) {
    if (m[4] !== undefined) {
      const row = grid[r]
      if (row && c < cols) row[c] = { ch: m[4], style }
      c++
    } else if (m[3] === "\r") c = 0
    else if (m[3] === "\n") r++
    else if (m[2] !== undefined) {
      const [params, final] = [m[1] as string, m[2]]
      if (final === "H") {
        const [row, col] = params.split(";").map(Number)
        r = (row || 1) - 1
        c = (col || 1) - 1
      } else if (final === "m") style = styled(style, params)
      else if (final === "J" && params === "2") grid.splice(0, rows, ...Array.from({ length: rows }, blank))
      else if (!params.startsWith("?")) throw new Error(`charts: ESC [ ${params}${final} is not read`)
    }
  }
  return grid
}

// ── the session ──

const engine = await loadEngine()
const scene = hero(engine)
const cells = cellsBinary()
const dir = mkdtempSync(join(tmpdir(), "cells-charts-"))
const file = join(dir, SPENDING_PATH)
writeFileSync(file, scene.texts[0] as string)

let bytes = ""
let heard = Date.now()
const decoder = new TextDecoder()
const terminal = new Bun.Terminal({
  cols: SCREEN.cols,
  rows: SCREEN.rows,
  name: "xterm-256color",
  data(_t, data) {
    bytes += decoder.decode(data, { stream: true })
    heard = Date.now()
  },
})
const [program, ...args] = WATCH_COMMAND.split(" ")
if (program !== "cells") throw new Error(`charts: the hero's command is ${program}'s, not cells'`)
const env = { ...process.env, TERM: "xterm-256color", COLORTERM: "truecolor" }
const proc = Bun.spawn([cells, ...args], { cwd: dir, env, terminal })

/** The screen once the program has drawn and been still for a while, its foot saying the
 *  file was read now. */
async function settled(since: number): Promise<Cell[][]> {
  const deadline = Date.now() + 8000
  for (;;) {
    await Bun.sleep(100)
    if (Date.now() > deadline) throw new Error("charts: the program drew nothing new within 8s")
    if (heard <= since || Date.now() - heard < 300) continue
    const grid = screenOf(bytes, SCREEN.cols, SCREEN.rows)
    if (!grid) continue
    const text = grid.map((row) => row.map((x) => x.ch).join("")).join("\n")
    if (text.includes(`${SPENDING_PATH} · read now`)) return grid
  }
}

/** A row as runs of one style each, every style by its place in `styles`. */
const styles: ScreenStyle[] = []
const styleKey = (s: ScreenStyle) => JSON.stringify(s)
const runsOf = (row: Cell[]): [string, number][] => {
  const runs: [string, number][] = []
  for (const cell of row) {
    const key = styleKey(cell.style)
    let k = styles.findIndex((s) => styleKey(s) === key)
    if (k < 0) k = styles.push(cell.style) - 1
    const last = runs.at(-1)
    if (last && last[1] === k) last[0] += cell.ch
    else runs.push([cell.ch, k])
  }
  return runs
}

const hash = (text: string) => createHash("sha256").update(text).digest("hex").slice(0, 16)
const frames: { source: string; rows: [string, number][][] }[] = []
let since = 0
for (const [i, text] of scene.texts.entries()) {
  if (i > 0) {
    since = Date.now()
    writeFileSync(file, text)
  }
  const grid = await settled(since)
  frames.push({ source: hash(text), rows: grid.map(runsOf) })
  console.log(`✓ frame ${i + 1} of ${scene.texts.length}: ${hash(text)}`)
}
terminal.write("q")
await proc.exited
terminal.close()

const at = Bun.spawnSync(["git", "rev-parse", "--short=12", "HEAD"], { cwd: repo }).stdout.toString().trim()
const out = join(import.meta.dir, "../lib/hero-screens.ts")
writeFileSync(
  out,
  `// Generated by scripts/charts.ts (pnpm charts) from \`${WATCH_COMMAND}\`, cells at ${at}, in a terminal of ${SCREEN.cols} by ${SCREEN.rows}: never edited here.\n` +
    `import type { ScreenFrames } from "@/lib/chart-screen"\n\n` +
    `export const SCREENS: ScreenFrames = ${JSON.stringify({ styles, frames })}\n`,
)
console.log(`✓ ${frames.length} frames → lib/hero-screens.ts`)
