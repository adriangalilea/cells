"use client"

// Several devices on one desk, each at its real width beside the others and all standing
// on one line, the desk's: a MacBook beside a Studio Display reads as the laptop and the
// monitor it is, never two pictures scaled to fit. A device is any element (a
// device-frame, a macos stage); the desk only places them. Time is the page's: devices
// inside one Playback (or one Playhead) read one playhead, so a desk of two macos stages
// is one scene on one clock, each drawing its own timeline at the same instant.

import type * as React from "react"
import { cn } from "@/lib/utils"

/** The devices' outer widths, in centimetres, as their frames draw them (device-frame):
 *  a MacBook by its base (the 13-inch Air the frame draws), a Studio Display by its
 *  panel, an iPhone by its body. */
export const DEVICE_WIDTH_CM = {
  macbook: 30.41,
  display: 62.3,
  iphone: 7.15,
} as const

export interface DeskProps extends React.ComponentProps<"div"> {
  /** The room between two devices, in centimetres, on the desk's scale. */
  gap?: number
}

export interface DeskDeviceProps extends React.ComponentProps<"div"> {
  /** Its outer width in centimetres (`DEVICE_WIDTH_CM`, or a device of your own): its
   *  share of the desk's width is its share of the devices' widths and the gaps. */
  width: number
}

/** The desk: its devices in a row, each its share of the row by its real width, all
 *  standing on the desk's line (their feet, the bottoms of their frames). */
export function Desk({
  gap = 8,
  className,
  style,
  children,
  ...props
}: DeskProps) {
  return (
    <div
      data-slot="desk"
      {...props}
      className={cn("flex w-full items-end", className)}
      style={{ ...style, "--desk-gap": gap } as React.CSSProperties}
    >
      {children}
    </div>
  )
}

/** One device on the desk. The gap before it is the desk's, held in the same
 *  centimetres as the widths, so the whole row scales as one. */
export function DeskDevice({
  width,
  className,
  style,
  children,
  ...props
}: DeskDeviceProps) {
  if (!(width > 0)) throw new Error(`desk: a device ${width} cm wide`)
  return (
    <>
      <div
        aria-hidden
        data-slot="desk-gap"
        className="first:hidden"
        style={{ flex: "var(--desk-gap) 1 0" }}
      />
      <div
        data-slot="desk-device"
        {...props}
        className={cn("min-w-0", className)}
        style={{ ...style, flex: `${width} 1 0` }}
      >
        {children}
      </div>
    </>
  )
}
