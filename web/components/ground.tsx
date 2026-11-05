"use client"

// THE PAGE'S GROUND, fixed so the page scrolls over it: ui's shimmer on a sheet's grid at
// its own defaults, over the ground and the one soft light its demo lies on ("a grid under
// a card"). One shimmer, one grid, across the whole page: its look is the item's, tuned
// there and never here. It draws only while seen and stands still under reduced motion
// (the item's own rules). What the page holds lies above it.

import { Ambient } from "@/components/ui/ambient"
import { Shimmer } from "@/components/ui/shimmer"

/** A sheet's cells, 112 by 32, their columns centred on the page. */
const SHEET = { width: 112, height: 32, x: "50%" } as const

export function Ground() {
  return (
    <div aria-hidden className="pointer-events-none fixed inset-0 -z-20">
      <div className="absolute inset-0 hidden dark:block">
        <Ambient ground="var(--background)" glows={[{ color: "#fff", x: 0.5, y: -0.1, rx: 0.55, ry: 0.95, strength: 0.1 }]} />
        <Shimmer grid={SHEET} />
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
    </div>
  )
}
