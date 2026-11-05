"use client"

import { useRender } from "@base-ui/react/use-render"
import { cn } from "@/lib/utils"
import {
  type Material,
  type Scheme,
  useMaterial,
  useSurfaceScheme,
} from "@/components/ui/material"
import {
  glassStyle,
  glassVariants,
  schemeOf,
} from "@/components/ui/material-glass"

// A small label: a status, a version, a count, a kind. Metadata's voice (mono, small,
// lowercase), which is where it parts from shadcn's sans badge: a badge says what
// something IS about the thing beside it, never what it says, and the registry keeps
// that voice in mono everywhere. shadcn's four variants; a pill one step below the
// smallest button. Not interactive unless `render` makes it a link.
// `material="glass"`: a sliver of the same glass the buttons are made of.

export type BadgeVariant =
  | "default"
  | "secondary"
  | "outline"
  | "success"
  | "warning"
  | "destructive"
  | "info"

/** A status's colour, as the tokens name it (the alert's kinds, the same words);
 *  `info` is news that is no trouble (the current one, a suggestion waiting). */
const STATUS: Partial<Record<BadgeVariant, string>> = {
  success: "var(--tone-ok)",
  warning: "var(--tone-warn)",
  destructive: "var(--tone-bad)",
  info: "var(--tone-info)",
}

const BASE =
  "inline-flex shrink-0 items-center whitespace-nowrap rounded-full font-mono lowercase"
/** A label's shape: words at the badge's height. */
const LABEL = "h-6 gap-1.5 px-2.5 text-[11px] [&_svg]:size-3 [&_svg]:shrink-0"
/** A count's shape: a circle while it is one digit (its width at least its
 *  height, the digit centred), a pill as digits come, small enough to sit beside a
 *  label's words (a tab's, a menu item's) on their centre line. */
const COUNT = "h-4 min-w-4 justify-center px-1 text-[10px] tabular-nums"

/** A status in plain: its colour washed under its words. On a light surface the
 *  words are the colour deepened (its lightness held at 0.55, as `TONE_WORDS`), so
 *  they read on their pale wash; on a dark one, the colour itself. */
const PLAIN_STATUS =
  "bg-(--ag-badge-ink)/15 text-[oklch(from_var(--ag-badge-ink)_min(l,0.55)_c_h)] scheme-dark:text-(--ag-badge-ink)"
const PLAIN: Record<BadgeVariant, string> = {
  default: "bg-foreground text-background",
  secondary: "bg-fill-rest text-foreground/80",
  outline: "text-foreground/75 ring-1 ring-foreground/15 ring-inset",
  success: PLAIN_STATUS,
  warning: PLAIN_STATUS,
  destructive: PLAIN_STATUS,
  info: PLAIN_STATUS,
}

/** A status in glass: the glass stained its colour as any tint is, the colour said
 *  again on its mark (an icon) and its words the glass's own. Its words in the colour
 *  too, on a stain twice a tint's, read far louder than the tints beside it. */
const GLASS_STATUS = "text-current/85 [&_svg]:text-(--ag-badge-ink)"
const GLASS: Record<BadgeVariant, string> = {
  default: "text-current/85",
  secondary:
    "text-current/70 [--glass-rim:0.7] [--glass-tint:rgb(238_241_245/0.4)] scheme-dark:[--glass-tint:rgb(16_16_18/0.35)]",
  // The glass's edge with no body under it (`edge: "only"`).
  outline: "text-current/80",
  success: GLASS_STATUS,
  warning: GLASS_STATUS,
  destructive: GLASS_STATUS,
  info: GLASS_STATUS,
}

export type BadgeProps = useRender.ComponentProps<"span"> & {
  variant?: BadgeVariant
  /** A count instead of words: drawn as a circle while one digit, a pill past
   *  it, sized to sit beside a label (`<TabsTab>alerts <Badge count={3} /></TabsTab>`).
   *  It is the badge's content: give it no children. */
  count?: number
  material?: Material
  /** The colour scheme it stands in (material): default the nearest provider's. */
  scheme?: Scheme
}

export function Badge({
  variant = "default",
  count,
  material,
  scheme,
  render,
  className,
  style,
  children,
  ref,
  ...props
}: BadgeProps) {
  if (count !== undefined && children !== undefined)
    throw new Error("Badge: a `count` is its content; give it no children")
  if (count !== undefined && !(Number.isInteger(count) && count >= 0))
    throw new Error(`Badge: a count is a whole number from 0, got ${count}`)
  const m = useMaterial(material)
  const [s, here] = useSurfaceScheme<HTMLElement>(scheme, ref)
  const ink = STATUS[variant]
  return useRender({
    defaultTagName: "span",
    render,
    ref: here,
    props: {
      "data-slot": "badge",
      "data-variant": variant,
      "data-material": m,
      style: ink
        ? ({
            "--ag-badge-ink": ink,
            ...(m === "glass" ? glassStyle({ tint: ink }) : {}),
            ...style,
          } as React.CSSProperties)
        : style,
      className: cn(
        BASE,
        count === undefined ? LABEL : COUNT,
        schemeOf(s),
        m === "glass"
          ? [
              glassVariants({
                shape: "surface",
                edge: variant === "outline" ? "only" : true,
              }),
              // A pill: its radius half its height.
              count === undefined
                ? "[--glass-blur:8px] [--glass-radius:12px]"
                : "[--glass-blur:8px] [--glass-radius:8px]",
              GLASS[variant],
            ]
          : PLAIN[variant],
        className,
      ),
      children: count === undefined ? children : count,
      ...props,
    },
  })
}
