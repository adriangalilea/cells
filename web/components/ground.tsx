"use client"

// THE PAGE'S GROUND, back to front, everything fixed so the page scrolls over it:
// 1. light: a few broad, very soft pools (ui's Ambient, computed per pixel and dithered,
//    so no rings), neutral, never a hue: colour on a ground reads as a gaming rig, and
//    this page keeps its two colours for meaning (the edge light on what is computed,
//    the haze on what is broken). Never flat black.
// 2. ui's shimmer at a whisper on a sheet's grid, its cells wider than tall: no lines
//    drawn, the grid sensed only through the facets lying along it, moving with the
//    page, gathering under the hand. Across the window, and a little stronger round the
//    hero (HeroShimmer) on the same grid. It draws only while seen and stands still under
//    reduced motion (the item's own rules).
// What the page holds lies above both.

import * as React from "react"
import { Ambient } from "@/components/ui/ambient"
import { Shimmer, type ShimmerGrid } from "@/components/ui/shimmer"

/** e-id's surface of facets, the same tuning: sparse, each speck a hard pixel, white with
 *  a trace of fire, at one canvas pixel per CSS pixel; frosted as a grid frosts it. */
export const FACETS = { density: 0.3, colour: 0.1, resolution: 1 } as const

/** A sheet's cells, 112 by 32, their columns centred on the page and their rows from its
 *  top: the one grid every shimmer on the page lies along. */
const GRID: ShimmerGrid = { width: 112, height: 32, x: "50%", y: 0 }

export function Ground() {
  return (
    <div aria-hidden className="pointer-events-none fixed inset-0 -z-20">
      <div className="absolute inset-0 hidden dark:block">
        <Ambient
          glows={[
            { color: "oklch(0.3 0 0)", x: 0.14, y: -0.05, rx: 0.75, ry: 0.85, strength: 0.3 },
            { color: "oklch(0.26 0 0)", x: 0.9, y: 0.4, rx: 0.6, ry: 0.75, strength: 0.22 },
            { color: "oklch(0.24 0 0)", x: 0.45, y: 1.08, rx: 0.85, ry: 0.55, strength: 0.25 },
          ]}
        />
      </div>
      <div className="absolute inset-0 dark:hidden">
        <Ambient
          ground="var(--background)"
          glows={[
            { color: "oklch(1 0 0)", x: 0.18, y: -0.05, rx: 0.7, ry: 0.8, strength: 0.7 },
            { color: "oklch(0.9 0 0)", x: 0.92, y: 0.5, rx: 0.6, ry: 0.7, strength: 0.25 },
          ]}
        />
      </div>
      <div className="absolute inset-0 hidden dark:block">
        <Shimmer {...FACETS} grid={GRID} strength={0.3} fade={{ x: 0.5, y: 0.45, rx: 0.75, ry: 0.7, inner: 0.35 }} />
      </div>
    </div>
  )
}

/** Where the hero's stage starts on the page: at its top from md (the stage runs under
 *  the header), under the 72px header on a phone. */
const useStageTop = () =>
  React.useSyncExternalStore(
    (changed) => {
      const q = matchMedia("(min-width: 768px)")
      q.addEventListener("change", changed)
      return () => q.removeEventListener("change", changed)
    },
    () => (matchMedia("(min-width: 768px)").matches ? 0 : 72),
    () => 0,
  )

/** Round the hero, stronger than the page's: behind the Mac, the sparkle the eye meets
 *  first, cut to nothing well before the hero's foot. Its rows start where the page's
 *  fall in its box, so the two grids are one. */
export function HeroShimmer() {
  const top = useStageTop()
  const grid = { ...GRID, y: (GRID.height - (top % GRID.height)) % GRID.height }
  return (
    <div aria-hidden className="pointer-events-none absolute inset-0 -z-10 hidden dark:block">
      <Shimmer {...FACETS} grid={grid} strength={0.5} fade={{ x: 0.5, y: 0.42, rx: 0.7, ry: 0.75, inner: 0.3, foot: 0.6 }} />
    </div>
  )
}
