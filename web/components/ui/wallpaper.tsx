// A device's picture: originals, not Apple's (theirs are theirs), in the spirit of
// the ones macOS 26 and iOS 26 ship. `macos`, a desktop picture: deep blue night with
// a luminous sweep of violet and teal across it. `ios`, a Lock Screen: indigo night
// over a violet bloom with a ribbon of teal and rose rising through it. Pure CSS, so
// they cost no request and are sharp at any size. Each fills any box, a Mac's screen,
// a phone, a card, and reads as the same picture in each.
//
// A picture is composed once on its device's screen (its `frame`, in design pixels:
// a MacBook at half scale, an iPhone in points) and shown the way a screen shows a
// picture: covering the box, held where the picture says (`align`). The box's size is
// the one fact this needs, read by CSS as container units, so the server's HTML is
// already the picture at its true size: one design pixel is
// `max(100cqw / width, 100cqh / height)`, and every light's place, size and blur is a
// count of them. A box wider than the frame crops the picture's top and foot, a
// narrower one its sides.

import type * as React from "react"
import { cn } from "@/lib/utils"

/** Which device's picture: a Mac's desktop or an iPhone's Lock Screen. */
export type WallpaperOf = "macos" | "ios"

export interface WallpaperProps {
  /** Which device's picture. Said by the caller: a Mac and a phone never share one. */
  of: WallpaperOf
  /** How far the picture is darkened, 0..1: a lock screen's is about 0.28. */
  dim?: number
  className?: string
  /** Drawn over the picture, in the box's flow. */
  children?: React.ReactNode
}

/** One light of a picture, in design pixels: its box on the frame, its blur, and
 *  what it is made of (its colours, its turn, its shape). */
interface Light {
  name: string
  x: number
  y: number
  width: number
  height: number
  blur: number
  className: string
}

interface Picture {
  /** The screen it is composed on, in design pixels. */
  frame: { width: number; height: number }
  /** Where the picture is held when the box crops it, 0..1 of each axis: the part
   *  that stays when the box is wider or narrower than the frame. */
  align: { x: number; y: number }
  /** The night under the lights. */
  ground: string
  lights: Light[]
}

const PICTURES: Record<WallpaperOf, Picture> = {
  macos: {
    frame: { width: 960, height: 600 },
    // Held at the top, as a Mac's screen is: a taller display shows more of its foot.
    align: { x: 0.5, y: 0 },
    ground: "bg-[linear-gradient(165deg,#0b1230_0%,#070a1c_45%,#04050d_100%)]",
    lights: [
      {
        name: "sweep",
        x: -180,
        y: 260,
        width: 900,
        height: 420,
        blur: 70,
        className:
          "rotate-[-18deg] rounded-[50%] bg-[conic-gradient(from_200deg_at_55%_50%,#2f5dff_0deg,#7b4dff_80deg,#1fb6c9_170deg,#2f5dff_260deg,#1a2a80_360deg)] opacity-75",
      },
      {
        name: "streak",
        x: 120,
        y: 330,
        width: 760,
        height: 140,
        blur: 40,
        className:
          "rotate-[-14deg] rounded-[50%] bg-[linear-gradient(90deg,transparent,rgb(180_200_255/0.55),rgb(120_230_240/0.35),transparent)]",
      },
      {
        name: "glow",
        x: 560,
        y: -160,
        width: 520,
        height: 420,
        blur: 90,
        className:
          "rounded-full bg-[radial-gradient(closest-side,#1d6fd1,transparent)] opacity-55",
      },
    ],
  },
  ios: {
    frame: { width: 390, height: 844 },
    // Held at the middle: the bloom is the picture, wherever the box crops it.
    align: { x: 0.5, y: 0.5 },
    ground:
      "bg-[linear-gradient(180deg,#070b22_0%,#0d0a26_40%,#120821_70%,#040309_100%)]",
    lights: [
      {
        name: "glow",
        x: 150,
        y: -150,
        width: 360,
        height: 340,
        blur: 70,
        className:
          "rounded-full bg-[radial-gradient(closest-side,#1d6fd1,transparent)] opacity-45",
      },
      {
        name: "bloom",
        x: -150,
        y: 400,
        width: 600,
        height: 600,
        blur: 60,
        className:
          "rounded-full bg-[radial-gradient(closest-side,#7b4dff,#3b1aa8_55%,transparent)] opacity-80",
      },
      {
        name: "ribbon",
        x: 70,
        y: 220,
        width: 250,
        height: 640,
        blur: 46,
        className:
          "rotate-[24deg] rounded-[50%] bg-[conic-gradient(from_160deg_at_50%_55%,#1fb6c9_0deg,#2f5dff_90deg,#7b4dff_170deg,#e0559a_250deg,#1fb6c9_360deg)] opacity-70",
      },
      {
        name: "streak",
        x: 20,
        y: 500,
        width: 380,
        height: 80,
        blur: 26,
        className:
          "rotate-[-34deg] rounded-[50%] bg-[linear-gradient(90deg,transparent,rgb(200_210_255/0.5),rgb(140_235_240/0.35),transparent)]",
      },
    ],
  },
}

const LIGHT =
  "absolute top-(--ag-wallpaper-top) left-(--ag-wallpaper-left) h-(--ag-wallpaper-height) w-(--ag-wallpaper-width) blur-(--ag-wallpaper-blur)"

/** A count of design pixels as a length of the box. */
const px = (n: number) => `calc(${n} * var(--ag-wallpaper-unit))`

/** One design pixel of the box, the larger of its two shares so the picture covers. */
const unitOf = ({ frame }: Picture) =>
  `max(calc(100cqw / ${frame.width}), calc(100cqh / ${frame.height}))`

/** A light's place on the box: on each axis, the box's held point plus the light's
 *  offset from the frame's held point. */
function placed({ frame, align }: Picture, l: Light): React.CSSProperties {
  return {
    "--ag-wallpaper-left": `calc(${align.x * 100}cqw + ${px(l.x - align.x * frame.width)})`,
    "--ag-wallpaper-top": `calc(${align.y * 100}cqh + ${px(l.y - align.y * frame.height)})`,
    "--ag-wallpaper-width": px(l.width),
    "--ag-wallpaper-height": px(l.height),
    "--ag-wallpaper-blur": px(l.blur),
  } as React.CSSProperties
}

/** A device's picture, filling its box; `children` stand over it. Size the box with
 *  `className` (`absolute inset-0` behind a screen, an aspect for a card). */
export function Wallpaper({
  of,
  dim = 0,
  className,
  children,
}: WallpaperProps) {
  const picture = PICTURES[of]
  if (!picture) throw new Error(`wallpaper: no picture of "${of}"`)
  return (
    <div
      data-slot="wallpaper"
      data-of={of}
      className={cn("relative isolate", className)}
    >
      <div
        aria-hidden="true"
        data-slot="wallpaper-picture"
        className={cn(
          "absolute inset-0 -z-10 overflow-hidden [container-type:size]",
          picture.ground,
        )}
        style={
          { "--ag-wallpaper-unit": unitOf(picture) } as React.CSSProperties
        }
      >
        {picture.lights.map((l) => (
          <div
            key={l.name}
            data-slot={`wallpaper-${l.name}`}
            className={cn(LIGHT, l.className)}
            style={placed(picture, l)}
          />
        ))}
        {dim > 0 && (
          <div
            data-slot="wallpaper-dim"
            className="absolute inset-0 bg-black opacity-(--ag-wallpaper-dim)"
            style={{ "--ag-wallpaper-dim": dim } as React.CSSProperties}
          />
        )}
      </div>
      {children}
    </div>
  )
}
