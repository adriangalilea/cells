// What the page costs a reader: the scripts index.html loads before it is interactive,
// gzipped, and the largest files the build wrote (loaded only when asked: Shiki's grammars,
// the engine's module).
//
//   bun scripts/weigh.ts

import { readdirSync, readFileSync, statSync } from "node:fs"
import { join } from "node:path"
import { gzipSync } from "node:zlib"

const out = `${import.meta.dir}/../out`
const html = readFileSync(`${out}/index.html`, "utf8")
const scripts = [...new Set(html.match(/\/_next\/static\/chunks\/[^"]+\.js/g) ?? [])]
let total = 0
for (const s of scripts) total += gzipSync(readFileSync(join(out, s))).length
console.log(`index.html ${(gzipSync(html).length / 1024).toFixed(0)} KB gz, ${scripts.length} scripts ${(total / 1024).toFixed(0)} KB gz`)

const files: [number, string][] = []
const walk = (dir: string) => {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) walk(p)
    else files.push([statSync(p).size, p.slice(out.length)])
  }
}
walk(out)
files.sort((a, b) => b[0] - a[0])
for (const [size, path] of files.slice(0, 6)) console.log(`${(size / 1024).toFixed(0)} KB ${path}`)
console.log(`${files.length} files, ${(files.reduce((n, [s]) => n + s, 0) / 1048576).toFixed(1)} MB`)
