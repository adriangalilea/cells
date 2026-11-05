"use client"

// The engine in a component. In the browser it is there before any component renders:
// this module waits for it at its top (top-level await), so the page hydrates once the
// module has streamed in, and nothing suspends while hydrating. A Suspense boundary that
// suspended mid-hydration lost its server HTML the moment anything above it updated (a
// media query, the clock), and the hero and every table went blank until the engine came.
// At build the engine is reached through `use`, inside the page's Suspense boundaries.

import * as React from "react"
import type { Engine } from "@/lib/view"
import { loadEngine } from "@/lib/engine-module"

const ready: Engine | null = typeof window === "undefined" ? null : await loadEngine()

export function useEngine(): Engine {
  return ready ?? React.use(loadEngine())
}
