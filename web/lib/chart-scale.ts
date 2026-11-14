// A chart's scale, as every Cells reader draws one (rust/cells/src/chart.rs `scale`): over
// what the chart draws, `lo..hi`, about `steps` steps, each 1, 2 or 5 times a power of ten,
// its ends whole steps. The engine's summary says what is drawn, not the scale (the wasm
// exports no `scale`), so the site keeps this one rule beside the engine's, the same
// arithmetic; when the wasm exports it, this file goes.

import type { ChartSummary } from "@/lib/engine/cells.ts"

export interface Scale {
  lo: number
  hi: number
  step: number
}

/** The step of a scale near `rough`: 1, 2 or 5 times a power of ten. */
function nice(rough: number): number {
  let power = 1
  while (power * 10 <= rough) power *= 10
  while (power > rough && power > 1e-9) power /= 10
  return [1, 2, 5, 10].map((m) => m * power).find((s) => s >= rough) ?? 10 * power
}

/** A scale a person reads over `lo..hi` in about `steps` steps. */
export function scale(lo: number, hi: number, steps: number): Scale {
  const step = nice((hi - lo) / Math.max(1, steps))
  return { lo: Math.floor(lo / step) * step, hi: Math.ceil(hi / step) * step, step }
}

/** What a chart draws at each place, every layer's: a stacked chart's sums, a line's
 *  points; the range a scale is laid over, from zero. */
export function extent(s: ChartSummary): { lo: number; hi: number } {
  const parts = s.layers.length ? s.layers : [s]
  let lo = 0
  let hi = 0
  for (const p of parts)
    for (const row of p.values) {
      const nums = row.map((v) => (v === null ? 0 : Number(v)))
      const drawn = p.kind === "bars" && p.stack === "stacked" ? [nums.reduce((a, b) => a + b, 0)] : nums
      for (const v of drawn) {
        lo = Math.min(lo, v)
        hi = Math.max(hi, v)
      }
    }
  return { lo, hi }
}

/** The steps a chart's scale takes on the site's chart view. */
export const VIEW_STEPS = 4

/** A chart's scale as the site's chart view draws it. */
export function chartScale(s: ChartSummary): Scale {
  const { lo, hi } = extent(s)
  return scale(lo, hi, VIEW_STEPS)
}
