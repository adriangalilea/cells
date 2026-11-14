"use client"

// Several devices on one desk, seen as a camera sees a desk: each at its real width, made
// larger the nearer it stands (its apparent size its width over its distance, one rule),
// its foot lower the nearer it is, and the nearer in front. A MacBook a little in front of
// a Studio Display reads as the laptop and the monitor it is, never two pictures scaled to
// fit. A device is any element (a device-frame, a macos stage); the desk only places them.
// Time is the page's: devices inside one Playback (or one Playhead) read one playhead, so a
// desk of two macos stages is one scene on one clock, each drawing its own timeline at the
// same instant.

import * as React from "react"
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
  /** The distance, in centimetres, at which a device is drawn at its own width: the scale
   *  every distance is measured against. */
  view?: number
  /** How high the eye stands over the desk, in centimetres at `view`: the higher, the
   *  further a nearer device's foot drops below a farther one's; 0 stands every device on
   *  one line. */
  eye?: number
  children: React.ReactNode
}

export interface DeskDeviceProps {
  /** Its outer width in centimetres (`DEVICE_WIDTH_CM`, or a device of your own). */
  width: number
  /** Where its centre stands along the desk, in centimetres from the desk's middle,
   *  negative to the left. */
  x?: number
  /** How far it stands from the viewer, in centimetres; the desk's `view` when absent. */
  distance?: number
  className?: string
  style?: React.CSSProperties
  children: React.ReactNode
}

/** One device on the desk: its place read by the desk, which draws it. */
export function DeskDevice(_props: DeskDeviceProps): React.ReactNode {
  throw new Error(
    "desk: a DeskDevice is drawn by the Desk around it, never alone",
  )
}

/** A device as the camera sees it, in centimetres at the desk's `view`: its left and right
 *  along the desk, its foot below the horizon, how near it is. */
function seen(d: DeskDeviceProps, view: number, eye: number) {
  const distance = d.distance ?? view
  if (!(d.width > 0) || !(distance > 0))
    throw new Error(`desk: a device ${d.width} cm wide at ${distance} cm`)
  const s = view / distance
  const centre = (d.x ?? 0) * s
  return {
    left: centre - (d.width * s) / 2,
    width: d.width * s,
    foot: eye * s,
    distance,
  }
}

/** The desk: its devices as a camera sees them, the row as wide as the desk's box. The
 *  nearest device's foot is the desk's foot; a farther one stands higher, by the eye's
 *  height, and behind. */
export function Desk({
  view = 100,
  eye = 10,
  className,
  style,
  children,
  ...props
}: DeskProps) {
  const devices = React.Children.toArray(children).map((child) => {
    if (
      !React.isValidElement<DeskDeviceProps>(child) ||
      child.type !== DeskDevice
    )
      throw new Error("desk: every child of a Desk is a DeskDevice")
    return {
      props: child.props,
      at: seen(child.props, view, eye),
      key: child.key,
    }
  })
  if (!devices.length) throw new Error("desk: a desk with no device")
  const from = Math.min(...devices.map((d) => d.at.left))
  const to = Math.max(...devices.map((d) => d.at.left + d.at.width))
  const lowest = Math.max(...devices.map((d) => d.at.foot))
  /** A length on the desk as a share of its box's width. */
  const cq = (cm: number) => `${(cm / (to - from)) * 100}cqw`
  return (
    <div
      data-slot="desk"
      {...props}
      className={cn("grid w-full [container-type:inline-size]", className)}
      style={style}
    >
      {devices.map(({ props: d, at, key }) => (
        <div
          key={key}
          data-slot="desk-device"
          className={cn("min-w-0 self-end justify-self-start", d.className)}
          style={{
            ...d.style,
            gridArea: "1 / 1",
            width: cq(at.width),
            marginLeft: cq(at.left - from),
            marginBottom: cq(lowest - at.foot),
            // the nearer in front
            zIndex: Math.round(10000 / at.distance),
          }}
        >
          {d.children}
        </div>
      ))}
    </div>
  )
}
