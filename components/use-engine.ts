"use client"

// The engine in a component: suspends until its module has streamed in. Inside a Suspense
// boundary that hydrates, React keeps the page's server-rendered HTML on screen meanwhile,
// so a reader sees every table at once and the page is live a moment later.

import * as React from "react"
import type { Engine } from "@/lib/cells"
import { loadEngine } from "@/lib/engine-module"

export function useEngine(): Engine {
  return React.use(loadEngine())
}
