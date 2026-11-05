"use client"

// A VALUE THAT IDENTIFIES ITS OWNER (a serial, a licence, an account key) shown
// veiled: dots, so a screenshot or a shared screen never leaks it. A click peeks it
// for a few seconds and it veils itself again. While veiled the value is in no text
// on screen, no title and nothing a screen reader says, so selecting the page finds
// dots. It is in the page's data all the same (a client component's props travel
// with the page): Veiled keeps a value off a screen, not out of a browser. A secret
// the browser must not hold is never sent to it.
//
// Monospaced, so a dot is as wide as any character and peeking never moves the line;
// the eye waits under the hand in a slot that is always there. `keep` leaves the
// first characters legible (enough to tell two apart).
//
// To copy it without showing it, a `Copy` beside it, never around it: a click on the
// value peeks, a click on the mark copies.
//
//   <Veiled value={serial} />
//   <Veiled value={serial} /> <Copy value={serial} tip="copy without showing it" />
//
// Swift's `Veiled` is the same.

import { Eye, EyeOff } from "lucide-react"
import * as React from "react"
import { cn } from "@/lib/utils"

/** How long a peek lasts: long enough to read it out, short enough that it never
 *  stays up for the next screenshot. */
export const PEEK_MS = 5000

/** The value as dots, a dot per character after the first `keep`. */
export function veil(value: string, keep = 0) {
  return value.slice(0, keep) + "•".repeat(Math.max(0, value.length - keep))
}

/** A peek at `value`: `shown` for `PEEK_MS` after `toggle`, or until toggled again.
 *  The value being peeked is held, not a flag: a new value is not it, so it arrives
 *  veiled, whatever the old one was doing. */
export function usePeek(value: string) {
  const [peeked, setPeeked] = React.useState<string | null>(null)
  const timer = React.useRef(0)
  React.useEffect(() => () => window.clearTimeout(timer.current), [])
  const shown = peeked === value
  const toggle = React.useCallback(() => {
    window.clearTimeout(timer.current)
    if (shown) return setPeeked(null)
    setPeeked(value)
    timer.current = window.setTimeout(() => setPeeked(null), PEEK_MS)
  }, [shown, value])
  return { shown, toggle }
}

export interface VeiledProps
  extends Omit<React.ComponentProps<"button">, "value" | "children"> {
  value: string
  /** Leading characters left legible. Default none. */
  keep?: number
}

export function Veiled({
  value,
  keep = 0,
  className,
  onClick,
  ...props
}: VeiledProps) {
  const { shown, toggle } = usePeek(value)
  return (
    <button
      data-slot="veiled"
      data-shown={shown ? "" : undefined}
      type="button"
      aria-label={shown ? `veiled value: ${value}` : "veiled value, hidden"}
      aria-pressed={shown}
      onClick={(event) => {
        onClick?.(event)
        if (!event.defaultPrevented) toggle()
      }}
      className={cn(
        "group/veiled inline-flex cursor-pointer items-center gap-1 rounded-sm font-mono outline-none focus-visible:ring-1 focus-visible:ring-current/40",
        className,
      )}
      {...props}
    >
      <span data-slot="veiled-value" aria-hidden>
        {shown ? value : veil(value, keep)}
      </span>
      <span
        data-slot="veiled-mark"
        aria-hidden
        className="shrink-0 text-current/60 opacity-0 transition-opacity group-hover/veiled:opacity-100 group-focus-visible/veiled:opacity-100"
      >
        {shown ? <EyeOff size={12} /> : <Eye size={12} />}
      </span>
    </button>
  )
}
