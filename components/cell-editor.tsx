"use client"

// A FILE TO TYPE INTO: the text coloured by the grammar every editor gets (code's
// highlighter with Cells' own TextMate grammar), and the engine's problems drawn on
// their characters as an editor draws a language server's (the span undercurled, the
// reason under the line it breaks). A textarea over its own highlighted copy: the
// browser's caret, selection, undo and IME, the colours underneath.
//
// A scene shows something happening in it first (`band`): those characters lit by the
// edge light, everything else dimmed (`dim`), a caret where keys are landing; read-only
// until the scene has played.

import * as React from "react"
import { useCodeColours } from "@/components/ui/use-code-colours"
import { type CodeRun, langOf, runsOf } from "@/lib/code-tokens"
import { revealX } from "@/lib/reveal"
import { placeBroken, placeProblems, type ProblemAt } from "@/lib/spans"
import { cn } from "@/lib/utils"

/** The editor's type: one size, one line height, shared by the colours, the textarea
 *  and the marks so all three lie on the same characters. */
/** The size is the box's (CellEditor's `fit`, else 15px, 19px from md), inherited. */
const TYPE = "font-mono leading-[1.7] slashed-zero [tab-size:2]"
const PAD = "px-4 py-3.5 md:px-6 md:py-5"

/** A line's runs cut where the band starts and ends, each piece knowing which side it is. */
function cut(runs: CodeRun[], lineStart: number, band: [number, number] | null): (CodeRun & { lit: boolean })[] {
  const out: (CodeRun & { lit: boolean })[] = []
  let at = lineStart
  for (const run of runs) {
    let from = 0
    while (from < run.text.length) {
      const here = at + from
      const lit = band !== null && here >= band[0] && here < band[1]
      const edge = band === null ? run.text.length : lit ? band[1] - at : here < band[0] ? band[0] - at : run.text.length
      const to = Math.min(run.text.length, Math.max(from + 1, edge))
      out.push({ text: run.text.slice(from, to), ink: run.ink, lit })
      from = to
    }
    at += run.text.length
  }
  return out
}

/** A line's pieces gathered into runs of the same side of the band. */
function groups(pieces: (CodeRun & { lit: boolean })[]): { lit: boolean; runs: CodeRun[] }[] {
  const out: { lit: boolean; runs: CodeRun[] }[] = []
  for (const p of pieces) {
    const last = out.at(-1)
    if (last && last.lit === p.lit) last.runs.push(p)
    else out.push({ lit: p.lit, runs: [p] })
  }
  return out
}

function Piece({ run, opacity }: { run: CodeRun; opacity: number }) {
  return (
    <span
      className={cn("transition-opacity duration-300", run.ink && "text-(--ink-light) dark:text-(--ink-dark)")}
      style={{
        ...(run.ink ? { "--ink-light": run.ink.light, "--ink-dark": run.ink.dark, fontStyle: run.ink.italic ? "italic" : undefined } : {}),
        opacity,
      } as React.CSSProperties}
    >
      {run.text}
    </span>
  )
}

export function CellEditor({
  name,
  value,
  onChange,
  problems,
  broken,
  label,
  band = null,
  dim = 0,
  caret = null,
  fit,
  className,
}: {
  name: string
  value: string
  /** Absent: read-only (a scene still showing). */
  onChange?: (text: string) => void
  problems: readonly ProblemAt[]
  /** Where the file's grammar breaks, marked on its characters as a problem is. */
  broken: readonly { why: string; span?: [number, number] }[]
  label: string
  /** The characters a scene marks, [from, to) in `value`. */
  band?: [number, number] | null
  /** How far everything but the band is dimmed, 0..1. */
  dim?: number
  /** Where keys are landing, an offset into `value`. */
  caret?: number | null
  /** The characters a line must show whole: the type is as large as the box allows for
   *  them, up to its usual size, never below 12px (past that, the line scrolls). */
  fit?: number
  className?: string
}) {
  const colours = useCodeColours(value, langOf(name))
  const lines = runsOf(value, colours)
  const placed = [...placeBroken(value, broken), ...placeProblems(value, problems)]
  const byLine = new Map<number, typeof placed>()
  for (const p of placed) byLine.set(p.line, [...(byLine.get(p.line) ?? []), p])
  const faded = 1 - 0.7 * dim
  // Where keys land, else the band's end, kept in sight when the line is wider than the box.
  const root = React.useRef<HTMLDivElement>(null)
  React.useLayoutEffect(() => {
    if (!band) return
    const el = root.current
    revealX(el?.querySelector<HTMLElement>("[data-caret]") ?? [...(el?.querySelectorAll<HTMLElement>("[data-lit]") ?? [])].at(-1) ?? null)
  }, [band, caret])
  let lineStart = 0
  // Geist Mono's advance is 0.6 em, and the box loses its padding (3rem at most) to the text.
  const size = fit ? { fontSize: `min(var(--type-max), max(12px, calc((100cqw - 3rem) / ${fit * 0.6})))` } : undefined
  return (
    <div className="@container">
    <div ref={root} data-slot="cell-editor" className={cn("relative grid overflow-auto text-[15px] [--type-max:15px] md:text-[19px] md:[--type-max:19px]", className)} style={size}>
      <pre aria-hidden className={cn(TYPE, PAD, "pointer-events-none col-start-1 row-start-1 m-0 whitespace-pre")}>
        {lines.map((runs, i) => {
          const marks = byLine.get(i + 1) ?? []
          const text = runs.map((r) => r.text).join("")
          const start = lineStart
          lineStart += text.length + 1
          const caretHere = caret !== null && caret >= start && caret <= start + text.length
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: a file's lines, in order
            <div key={i} className="relative min-h-[1.7em]">
              {groups(cut(runs, start, band)).map((g, k) =>
                g.lit ? (
                  // The band: one rounded span over every piece it covers, whatever their colours.
                  // biome-ignore lint/suspicious/noArrayIndexKey: groups are positional
                  <span key={k} data-lit className="rounded-[4px] bg-edge/16 shadow-[0_0_0_1px_color-mix(in_oklch,var(--edge)_50%,transparent)]">
                    {g.runs.map((run, j) => (
                      // biome-ignore lint/suspicious/noArrayIndexKey: pieces are positional
                      <Piece key={j} run={run} opacity={1} />
                    ))}
                  </span>
                ) : (
                  // biome-ignore lint/suspicious/noArrayIndexKey: pieces are positional
                  g.runs.map((run, j) => <Piece key={`${k}.${j}`} run={run} opacity={faded} />)
                ),
              )}
              {caretHere && <span data-caret className="absolute top-[0.15em] h-[1.4em] w-[2px] bg-edge" style={{ left: `${(caret as number) - start}ch` }} />}
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
                <span className="ml-[2ch] text-[0.75em] text-haze/90">{marks.map((m) => m.message).join(" · ")}</span>
              )}
            </div>
          )
        })}
      </pre>
      <textarea
        aria-label={label}
        value={value}
        readOnly={!onChange}
        onChange={(e) => onChange?.(e.target.value)}
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
        style={{ minHeight: `calc(${value.split("\n").length} * 1.7em)` }}
      />
    </div>
    </div>
  )
}
