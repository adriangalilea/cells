"use client"

// THE ENGINE AS HELIX'S LANGUAGE SERVER: ui's terminal-editor asks for a file's
// diagnostics with every file's text at the instant (`CodeDiagnose`), and this answers
// with the engine's own: the file read with the others as its imports, each break of its
// grammar and each problem of its cells placed on the characters it names, in the
// engine's words. A problem money.cell has because of accounts.cell shows, and goes once
// accounts.cell has the key.

import * as React from "react"
import { useEngine } from "@/components/use-engine"
import type { CodeDiagnose } from "@/lib/code-session"
import { placeBroken, placeProblems } from "@/lib/spans"
import { readFiles } from "@/lib/view"

export function useCellsDiagnose(): CodeDiagnose {
  const engine = useEngine()
  return React.useMemo<CodeDiagnose>(
    () => (path, workspace) => {
      const text = workspace[path]
      if (text === undefined) throw new Error(`diagnose: ${path} is not in the workspace`)
      const others = Object.entries(workspace)
        .filter(([p]) => p !== path)
        .map(([name, t]) => ({ name, text: t }))
      const read = readFiles(engine, [{ name: path, text }, ...others])
      const starts = [0]
      for (let i = 0; i < text.length; i++) if (text[i] === "\n") starts.push(i + 1)
      const at = (line: number, column: number) => (starts[line - 1] as number) + column
      return [...placeBroken(text, read.broken), ...placeProblems(text, read.problems)].map((p) => ({
        from: at(p.line, p.from),
        to: at(p.line, p.to),
        severity: "error" as const,
        message: p.message,
        source: "cells",
      }))
    },
    [engine],
  )
}
