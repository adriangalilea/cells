"use client"

// Copy to the clipboard, with the only feedback that matters: the control says it
// worked, in place, and goes back on its own. No toast, nothing to dismiss.

import { Check, CircleAlert, Clipboard, Eye, EyeOff } from "lucide-react"
import * as React from "react"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import {
  frontButton,
  InputGroup,
  type InputSize,
} from "@/components/ui/input"
import type { Material } from "@/components/ui/material"
import { Tip } from "@/components/ui/tooltip"
import { usePeek, veil } from "@/components/ui/veiled"

/** How long the control admits it copied. Long enough to read at a glance, short
 *  enough that a second copy is never blocked by the first one's applause. */
export const COPIED_MS = 1400

/** `copied` flips true on a successful write and clears itself. The timer is cleared
 *  on unmount and on every new copy, so a fast second press restarts it rather than
 *  being cut short by the first one's expiry. */
export function useCopy(text: string | (() => string)) {
  const [copied, setCopied] = React.useState(false)
  const [error, setError] = React.useState<unknown>(null)
  const timer = React.useRef(0)
  React.useEffect(() => () => window.clearTimeout(timer.current), [])
  const copy = React.useCallback(async () => {
    const value = typeof text === "function" ? text() : text
    // Empty is not a copy: writing "" silently wipes what the reader already had.
    window.clearTimeout(timer.current)
    setCopied(false)
    setError(null)
    if (!value) return false
    try {
      await navigator.clipboard.writeText(value)
      setCopied(true)
      timer.current = window.setTimeout(() => setCopied(false), COPIED_MS)
      return true
    } catch (cause) {
      setError(cause)
      return false
    }
  }, [text])
  return { copied, copy, error }
}

export interface CopyProps
  extends Omit<React.ComponentProps<"button">, "value"> {
  /** The text to write. A function is read at press time, for a live value. */
  value: string | (() => string)
  /** Show the word beside the mark. Off in tight corners, where the mark is enough. */
  label?: boolean
  /** The text itself is the button (an address, a hash, an order number in a row):
   *  shown as given, the mark beside it under the hand and while it confirms, so a
   *  column of values reads as values and not as a column of buttons. It shrinks with
   *  its row, so a face that truncates (`MiddleTruncate`) still does. */
  children?: React.ReactNode
  /** Its tooltip (`Tip`), and the only one on its hover: a `MiddleTruncate` inside
   *  stops showing its own. `tip={address}` shows the whole value even when it fits;
   *  without one, a cut `MiddleTruncate` shows the whole only when cut. */
  tip?: React.ReactNode
}

const UNAVAILABLE =
  "Clipboard unavailable. Try again or select the text to copy."

/** One press, three faces: the mark, the mark and its word (`label`), or your text
 *  with the mark beside it (`children`). */
export function Copy({
  value,
  label = false,
  children,
  tip,
  className,
  onClick,
  ...props
}: CopyProps) {
  const { copied, copy, error } = useCopy(value)
  const text = children !== undefined
  if (text && label)
    throw new Error("Copy: text is its own label, pass children or label")
  const state = error ? "error" : copied ? "copied" : "idle"
  const press = (event: React.MouseEvent<HTMLButtonElement>) => {
    onClick?.(event)
    if (!event.defaultPrevented) void copy()
  }
  const mark = error ? <CircleAlert /> : copied ? <Check /> : <Clipboard />
  // The mark and its word are a Button, ghost (nothing until the hand comes), so
  // they stand at a button's heights and corners beside any other control. The
  // word sits in a cell as wide as its widest form, so "copied" moves nothing.
  const button = !text ? (
    <Button
      data-slot="copy"
      data-face="mark"
      data-state={state}
      variant="ghost"
      size={label ? "sm" : "icon-sm"}
      icon={label ? mark : undefined}
      onClick={press}
      aria-label={error ? "Copy failed. Try again" : copied ? "Copied" : "Copy"}
      title={error && tip === undefined ? UNAVAILABLE : undefined}
      className={cn(
        "shrink-0 font-mono text-muted-foreground hover:text-foreground data-[state=copied]:text-foreground",
        className,
      )}
      {...props}
    >
      {label ? (
        <span data-slot="copy-label" className="grid">
          <span aria-hidden className="invisible col-start-1 row-start-1">
            copied
          </span>
          <span className="col-start-1 row-start-1 text-left">
            {error ? "retry" : copied ? "copied" : "copy"}
          </span>
        </span>
      ) : (
        mark
      )}
    </Button>
  ) : (
    // The text is the button: the value reads as itself in its line (an order
    // number in a sentence), so no button surface, only a faint wash under the hand.
    // The accessible name carries the state; text at rest is its own name.
    <button
      data-slot="copy"
      data-face="text"
      data-state={state}
      type="button"
      onClick={press}
      aria-label={
        error ? "Copy failed. Try again" : copied ? "Copied" : undefined
      }
      title={error && tip === undefined ? UNAVAILABLE : undefined}
      className={cn(
        "group/copy -mx-1 inline-flex min-w-0 max-w-full cursor-pointer items-center gap-1 rounded-sm px-1 text-left transition-colors hover:bg-fill-row-hover focus-visible:bg-fill-row-hover focus-visible:outline-none disabled:pointer-events-none disabled:opacity-40",
        className,
      )}
      {...props}
    >
      <span data-slot="copy-face" className="flex min-w-0">
        {children}
      </span>
      {/* Beside text the mark is a step dimmer than the words, and waits for the hand. */}
      <span
        data-slot="copy-mark"
        aria-hidden
        className="shrink-0 text-current/60 opacity-0 transition-opacity group-hover/copy:opacity-100 group-focus-visible/copy:opacity-100 group-data-[state=copied]/copy:opacity-100 group-data-[state=error]/copy:opacity-100 [&_svg]:size-3"
      >
        {mark}
      </span>
    </button>
  )
  if (tip === undefined) return button
  // Over text, the tip keeps to the text's bounds: it starts where the text starts
  // and is no wider than it (8rem at least, so words under a short value still
  // read), a whole value wrapping where it must instead of hanging past the value.
  // The mark alone centres its tip.
  return (
    <Tip
      content={error ? UNAVAILABLE : tip}
      align={text ? "start" : "center"}
      className={
        text ? "max-w-[max(var(--anchor-width),8rem)] wrap-anywhere" : undefined
      }
    >
      {button}
    </Tip>
  )
}

export interface CopyFieldProps {
  /** What the copy writes, whole. */
  value: string
  /** A value that identifies its owner (a license key, a token), as `Veiled` draws
   *  one: dots, an eye on the front beside the copy to peek it for a few seconds. */
  veiled?: boolean
  /** Its tooltip on the copy mark. */
  tip?: React.ReactNode
  /** An accessible name for the value ("license key"). */
  label: string
  size?: InputSize
  material?: Material
  className?: string
}

/** A value to be copied, in the field every input is (`InputGroup`): read-only and
 *  monospaced in the well, cut at its end when longer than the field, the copy on
 *  the field's front, the strip at its end. An install id, an API key; a license key
 *  `veiled`, copied without being shown. */
export function CopyField({
  value,
  veiled = false,
  tip,
  label,
  size = "lg",
  material,
  className,
}: CopyFieldProps) {
  const { shown, toggle } = usePeek(value)
  const hidden = veiled && !shown
  return (
    <InputGroup
      readOnly
      value={hidden ? veil(value) : value}
      aria-label={hidden ? `${label}, hidden` : label}
      size={size}
      material={material}
      className={cn("font-mono", className)}
      onFocus={(event) => {
        if (!hidden) event.currentTarget.select()
      }}
      actions={
        <>
          {veiled && (
            <Button
              data-slot="copy-field-peek"
              variant="ghost"
              size="icon-sm"
              className={frontButton(size)}
              aria-label={shown ? "hide" : "show"}
              pressed={shown}
              onClick={toggle}
            >
              {shown ? <EyeOff aria-hidden /> : <Eye aria-hidden />}
            </Button>
          )}
          <Copy value={value} tip={tip} className={frontButton(size)} />
        </>
      }
    />
  )
}
