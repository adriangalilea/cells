"use client"

// THE PAGE'S GROUND, back to front, everything fixed so the page scrolls over it:
// 1. light: a few broad, very soft pools (ui's AmbientLight, computed per pixel and
//    dithered, so no rings), neutral, never a hue: colour on a ground reads as a gaming
//    rig, and this page keeps its two colours for meaning (the edge light on what is
//    computed, the haze on what is broken). Never flat black.
// 2. the table: a faint grid of a sheet's cells, wider than tall, noticed on a second
//    look (globals.css `.ground-grid`).
// 3. ui's shimmer at a whisper, facets flashing at rest and gathering under the hand,
//    fading at its own edges, across the window and a little stronger round the hero
//    (HeroShimmer). It draws only while seen and stands still under reduced motion (the
//    item's own rules).
// What the page holds lies above all three.

import { AmbientLight } from "@/components/ui/ambient-light"
import { Shimmer } from "@/components/ui/shimmer"

/** e-id's surface of facets, the same tuning: sparse, barely frosted, each speck a hard
 *  pixel, white with a trace of fire, at one canvas pixel per CSS pixel. */
export const FACETS = { density: 0.3, frost: 0.01, colour: 0.1, resolution: 1 } as const


export function Ground() {
  return (
    <div aria-hidden className="pointer-events-none fixed inset-0 -z-20">
      <div className="absolute inset-0 hidden dark:block">
        <AmbientLight
          glows={[
            { color: "oklch(0.3 0 0)", x: 0.14, y: -0.05, rx: 0.75, ry: 0.85, strength: 0.3 },
            { color: "oklch(0.26 0 0)", x: 0.9, y: 0.4, rx: 0.6, ry: 0.75, strength: 0.22 },
            { color: "oklch(0.24 0 0)", x: 0.45, y: 1.08, rx: 0.85, ry: 0.55, strength: 0.25 },
          ]}
        />
      </div>
      <div className="absolute inset-0 dark:hidden">
        <AmbientLight
          ground="var(--background)"
          glows={[
            { color: "oklch(1 0 0)", x: 0.18, y: -0.05, rx: 0.7, ry: 0.8, strength: 0.7 },
            { color: "oklch(0.9 0 0)", x: 0.92, y: 0.5, rx: 0.6, ry: 0.7, strength: 0.25 },
          ]}
        />
      </div>
      <div className="ground-grid absolute inset-0" />
      {/* Faint and on its own, never cut to the grid: a shimmer that follows the grid's
          lines is for ui to draw (for ui: a `lines` option on shimmer, the facets laid on a
          given grid). */}
      <div className="absolute inset-0 hidden dark:block">
        <Shimmer {...FACETS} strength={0.3} fade={{ x: 0.5, y: 0.45, rx: 0.75, ry: 0.7, inner: 0.35 }} />
      </div>
    </div>
  )
}

/** Round the hero, stronger than the page's: behind the Mac, the sparkle the eye meets
 *  first, cut to nothing well before the hero's foot. */
export function HeroShimmer() {
  return (
    <div aria-hidden className="pointer-events-none absolute inset-0 -z-10 hidden dark:block">
      <Shimmer {...FACETS} strength={0.5} fade={{ x: 0.5, y: 0.42, rx: 0.7, ry: 0.75, inner: 0.3, foot: 0.6 }} />
    </div>
  )
}
