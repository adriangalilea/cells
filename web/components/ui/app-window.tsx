"use client"

// A MAC APP'S WINDOW for anything: the three window buttons, a title centred in the
// bar, and a body that holds whatever React it is given (a table, a form, a chart),
// so it reads as a real app on a page or on a `macos` screen:
//
//   <AppWindow title="money.cell"><Table … /></AppWindow>
//
// It is the one drawing of a Mac window's chrome here: `browser` and `ide` draw their
// buttons with `TrafficLights`, `macos` its terminal windows with `AppWindow`. The
// window takes its size from its `className` (or its place in a layout) and its body
// the rest of it; the body clips, so what it holds scrolls inside it. Its colours are
// properties set per scheme (`--ag-app-window-ground`, `-ink`, `-bar`, `-shadow`): a
// consumer replaces one with a plain utility in `className` (`bg-…`, `shadow-…`) or
// sets the property on `style`, the bar's included. `active` is macOS's key window:
// the buttons in their colours and the title whole; behind another, both grey.

import type * as React from "react"
import { cn } from "@/lib/utils"
import { type Scheme, useSurfaceScheme } from "@/components/ui/material"
import { schemeOf } from "@/components/ui/material-glass"

/** The window buttons' own colours: close, minimise, zoom. */
const LIGHTS = ["#ff5f57", "#febc2e", "#28c840"] as const

export interface TrafficLightsProps {
  /** macOS's key window: the buttons in their own colours. Off: the grey of a
   *  window behind another. */
  active?: boolean
  /** One colour for all three, for a window that draws its chrome in its own
   *  palette (a terminal's phosphor); it wins over `active`. */
  tone?: string
  className?: string
}

/** The three window buttons at the head of a Mac window's bar. */
export function TrafficLights({
  active = true,
  tone,
  className,
}: TrafficLightsProps) {
  return (
    <span
      data-slot="app-window-lights"
      aria-hidden
      className={cn("flex shrink-0 gap-2", className)}
    >
      {LIGHTS.map((c) => (
        <span
          key={c}
          className="size-3 rounded-full shadow-[inset_0_0_0_0.5px_rgb(0_0_0/0.18)]"
          style={{
            background:
              tone ??
              (active
                ? c
                : "color-mix(in oklab, currentColor 22%, transparent)"),
          }}
        />
      ))}
    </span>
  )
}

export interface AppWindowProps
  extends Omit<React.ComponentProps<"div">, "title"> {
  /** The window's title, centred in the bar: the document's name, the app's. */
  title?: React.ReactNode
  /** macOS's key window (default): the buttons in colour and the title whole. */
  active?: boolean
  /** The buttons in one colour (`TrafficLights`' `tone`). */
  tone?: string
  /** The colour scheme the window stands in: the page's, or its own. */
  scheme?: Scheme
}

/** A Mac app's window: its bar (the buttons, the title) and its body, `children`. */
export function AppWindow({
  title,
  active = true,
  tone,
  scheme,
  className,
  children,
  ref,
  ...props
}: AppWindowProps) {
  const [worn, root] = useSurfaceScheme<HTMLDivElement>(scheme, ref)
  return (
    <div
      {...props}
      ref={root}
      data-slot="app-window"
      data-active={active || undefined}
      className={cn(
        "relative isolate flex flex-col overflow-hidden rounded-[16px]",
        // Each colour drawn from its property, set per scheme: a consumer's plain
        // `bg-*`, `text-*` or `shadow-*` replaces the utility whatever the scheme,
        // where a `scheme-dark:` utility would outrank it.
        "bg-(--ag-app-window-ground) text-(color:--ag-app-window-ink) shadow-(--ag-app-window-shadow)",
        "[--ag-app-window-bar:#f3f3f3] [--ag-app-window-ground:#ffffff] [--ag-app-window-ink:#1d1d1f] [--ag-app-window-shadow:0_0_0_0.5px_rgb(0_0_0/0.22),0_24px_60px_-16px_rgb(0_0_0/0.45)]",
        "scheme-dark:[--ag-app-window-bar:#2b2b2d] scheme-dark:[--ag-app-window-ground:#1e1e1e] scheme-dark:[--ag-app-window-ink:#e5e5e7] scheme-dark:[--ag-app-window-shadow:0_0_0_0.5px_rgb(255_255_255/0.16),0_24px_60px_-16px_rgb(0_0_0/0.6)]",
        schemeOf(worn),
        className,
      )}
    >
      <div
        data-slot="app-window-bar"
        className="relative flex h-7 shrink-0 items-center bg-(--ag-app-window-bar) px-3"
      >
        <TrafficLights active={active} tone={tone} />
        {title !== undefined && (
          <span
            data-slot="app-window-title"
            className={cn(
              "pointer-events-none absolute inset-x-20 truncate text-center font-sans font-semibold text-[13px]",
              active ? "opacity-85" : "opacity-45",
            )}
          >
            {title}
          </span>
        )}
      </div>
      <div
        data-slot="app-window-body"
        className="relative min-h-0 flex-1 overflow-hidden"
      >
        {children}
      </div>
    </div>
  )
}
