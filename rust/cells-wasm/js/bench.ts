// The wasm wrapper against the TypeScript reader (ts/src/cells.ts), under bun, on one
// generated budget: the best of five reads each. `read` leaves the values in the
// module's memory; "+ rows" builds every row's object as the TypeScript reader does.
//
//   bun rust/cells-wasm/js/bench.ts rust/bench-data/budget-100000.cells

import { read as tsRead } from "../../../ts/src/cells.ts"
import { read } from "./cells.ts"

const path = process.argv[2]
if (!path) throw new Error("bench.ts <file>")
const text = await Bun.file(path).text()
const bytes = Buffer.byteLength(text)

function best(runs: number, f: () => number): [number, number] {
  let t = Number.POSITIVE_INFINITY
  let rows = 0
  for (let k = 0; k < runs; k++) {
    const start = performance.now()
    rows = f()
    t = Math.min(t, performance.now() - start)
  }
  return [t, rows]
}

function report(what: string, [ms, rows]: [number, number]) {
  const s = ms / 1000
  console.log(`${what.padEnd(28)} ${ms.toFixed(1).padStart(9)} ms ${(bytes / s / 1e6).toFixed(0).padStart(9)} MB/s ${(rows / s).toFixed(0).padStart(12)} rows/s  (${rows} rows)`)
}

const runs = bytes > 20_000_000 ? 3 : 5
console.log(`${path} (${(bytes / 1e6).toFixed(1)} MB)`)
report(
  "wasm read",
  best(runs, () => {
    const r = read(text)
    const n = r.doc.rowCount
    r.doc.free()
    return n
  }),
)
report(
  "wasm read + rows",
  best(runs, () => {
    const r = read(text)
    const n = r.rows.length
    r.doc.free()
    return n
  }),
)
report(
  "wasm read + one column",
  best(runs, () => {
    const r = read(text)
    const cost = r.doc.column("cost")
    let sum = 0
    for (let i = 0; i < cost.length; i++) sum += (cost.value(i) as number | null) ?? 0
    r.doc.free()
    return sum > 0 ? cost.length : 0
  }),
)
report("ts reader", best(runs, () => tsRead(text).rows.length))
