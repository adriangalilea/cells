// The TypeScript reader (ts/src/tcsv.ts) on a generated budget, under bun: the best of
// five reads, as tcsv-bench prints its own.
//
//   bun rust/tcsv-bench/ts-read.ts rust/bench-data/budget-100000.tcsv

import { read } from "../../ts/src/tcsv.ts"

const path = process.argv[2]
if (!path) throw new Error("ts-read.ts <file>")
const text = await Bun.file(path).text()
const bytes = Buffer.byteLength(text)
let best = Number.POSITIVE_INFINITY
let rows = 0
const runs = bytes > 20_000_000 ? 3 : 5
for (let k = 0; k < runs; k++) {
  const start = performance.now()
  const got = read(text)
  best = Math.min(best, performance.now() - start)
  rows = got.rows.length
}
const secs = best / 1000
console.log(
  `${"ts reader (bun)".padEnd(28)} ${best.toFixed(1).padStart(9)} ms ${(bytes / secs / 1e6).toFixed(0).padStart(9)} MB/s ${(rows / secs).toFixed(0).padStart(12)} rows/s  (${rows} rows)`,
)
