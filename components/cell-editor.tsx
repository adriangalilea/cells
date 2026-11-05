"use client"

// A FILE TO TYPE INTO: the text coloured by the grammar every editor gets (code's
// highlighter with Cells' own TextMate grammar), and the engine's problems drawn on
// their characters as an editor draws a language server's (the span undercurled, the
// reason under the line it breaks). A textarea over its own highlighted copy: the
// browser's caret, selection, undo and IME, the colours underneath.

import * as React from "react"
import { useCodeColours } from "@/components/ui/use-code-colours"
import { langOf, runsOf } from "@/lib/code-tokens"
import { placeProblems, type ProblemAt } from "@/lib/spans"
import { cn } from "@/lib/utils"

/** The editor's type: one size, one line height, shared by the colours, the textarea
 *  and the marks so all three lie on the same characters. */
const TYPE = "font-mono text-[13px] leading-[1.7] slashed-zero [tab-size:2]"
const PAD = "px-4 py-3.5"

export function CellEditor({
  name,
  value,
  onChange,
  problems,
  label,
  className,
}: {
  name: string
  value: string
  onChange: (text: string) => void
  problems: readonly ProblemAt[]
  label: string
  className?: string
}) {
  const colours = useCodeColours(value, langOf(name))
  const lines = runsOf(value, colours)
  const placed = placeProblems(value, problems)
  const byLine = new Map<number, typeof placed>()
  for (const p of placed) byLine.set(p.line, [...(byLine.get(p.line) ?? []), p])
  return (
    <div data-slot="cell-editor" className={cn("relative grid overflow-auto", className)}>
      <pre aria-hidden className={cn(TYPE, PAD, "pointer-events-none col-start-1 row-start-1 m-0 whitespace-pre")}>
        {lines.map((runs, i) => {
          const marks = byLine.get(i + 1) ?? []
          const text = runs.map((r) => r.text).join("")
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: a file's lines, in order
            <div key={i} className="relative min-h-[1.7em]">
              {runs.map((run, k) => (
                <span
                  // biome-ignore lint/suspicious/noArrayIndexKey: runs are positional
                  key={k}
                  className={run.ink ? "text-(--ink-light) dark:text-(--ink-dark)" : undefined}
                  style={
                    run.ink
                      ? ({
                          "--ink-light": run.ink.light,
                          "--ink-dark": run.ink.dark,
                          fontStyle: run.ink.italic ? "italic" : undefined,
                        } as React.CSSProperties)
                      : undefined
                  }
                >
                  {run.text}
                </span>
              ))}
              {marks.map((m, k) => (
                <span
                  // biome-ignore lint/suspicious/noArrayIndexKey: a line's marks, in order
                  key={k}
                  className="absolute top-0 text-transparent underline decoration-haze decoration-wavy decoration-1 underline-offset-4"
                  style={{ left: `${m.from}ch` }}
                >
                  {text.slice(m.from, m.to)}
                </span>
              ))}
              {marks.length > 0 && (
                <span className="ml-[2ch] text-[11.5px] text-haze/90">{marks.map((m) => m.message).join(" · ")}</span>
              )}
            </div>
          )
        })}
      </pre>
      <textarea
        aria-label={label}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        spellCheck={false}
        autoCapitalize="off"
        autoComplete="off"
        autoCorrect="off"
        wrap="off"
        className={cn(
          TYPE,
          PAD,
          "col-start-1 row-start-1 m-0 resize-none overflow-hidden whitespace-pre bg-transparent text-transparent caret-edge outline-none selection:bg-edge/25 selection:text-transparent",
        )}
        style={{ minHeight: `calc(${value.split("\n").length} * 1.7em + 1.75rem)` }}
      />
    </div>
  )
}
