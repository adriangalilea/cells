"use client"

// The engine as helix's language server (lib/diagnose), for the page's editors: one
// answer per loaded engine.

import * as React from "react"
import { useEngine } from "@/components/use-engine"
import type { CodeDiagnose } from "@/lib/code-session"
import { cellsDiagnose } from "@/lib/diagnose"

export function useCellsDiagnose(): CodeDiagnose {
  const engine = useEngine()
  return React.useMemo(() => cellsDiagnose(engine), [engine])
}
