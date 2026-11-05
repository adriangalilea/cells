"use client"

import { Toggle as BaseToggle } from "@base-ui/react/toggle"
import { ToggleGroup as BaseToggleGroup } from "@base-ui/react/toggle-group"
import * as React from "react"
import { cn } from "@/lib/utils"
import { useField } from "@/components/ui/field-context"
import {
  type Material,
  MaterialProvider,
  type Scheme,
  useMaterial,
  useSurfaceScheme,
} from "@/components/ui/material"
import {
  glassEdge,
  glassLift,
  glassVariants,
  schemeOf,
} from "@/components/ui/material-glass"
import {
  GLASS_GHOST,
  useSetting,
} from "@/components/ui/setting-context"
import { Tip } from "@/components/ui/tooltip"

// A few options side by side in one bar, a segmented control (Base UI's toggle group:
// toggle buttons, one Tab stop, the arrow keys moving between them).
//
//   one       `value` is one option, always one once chosen (pressing the chosen one
//             keeps it): a highlight GLIDES to it, so a change reads as one thing
//             moving, never two states blinking.
//   multiple  `value` is a set: each pressed option lit on its own (bold, italic).
//
// Plain: a sunken well, the chosen lifted out of it as the page's own surface.
// `material="glass"`: a bar of glass, the chosen a lit pane inside it. At the input's
// heights, so it sits in a form's row as a field does. `readOnly`: shown, never
// changed (a press changes nothing, the Tab stop and the arrows stay). Inside a
// `Setting` whose value is inherited, the mark on the option that applies is a
// ghost, hollow and dashed (glass: the material's ghost), and pressing that option
// sets it there.
//
// Two looks (`variant`): `segment`, the bar above; `dots`, each option a dot and the
// chosen one stretched into a short pill (a carousel's pager, a story's acts), its
// words kept for a screen reader only, its target still a comfortable 24 px.
//
//   <ToggleGroup variant="dots" value={act} onValueChange={seek}>
//     {acts.map((a) => <ToggleGroupItem key={a.id} value={a.id}>{a.title}</ToggleGroupItem>)}
//   </ToggleGroup>
//
// A segment's option may carry an `icon`, and the group says what each option shows
// (`show`): `both`, the icon and the words; `icon`, the icon alone, the words its
// tooltip and its name; `selected`, the icon alone except on the chosen option,
// which shows its words too. The bar keeps one width whichever is chosen: the
// chosen option's words take the room of the widest option's, measured, so nothing
// beside the bar moves when the choice does.
//
//   <ToggleGroup show="selected" value={view} onValueChange={setView}>
//     <ToggleGroupItem value="grid" icon={<LayoutGrid />}>grid</ToggleGroupItem>
//     <ToggleGroupItem value="list" icon={<List />}>list</ToggleGroupItem>
//   </ToggleGroup>

export type ToggleGroupSize = "sm" | "md" | "lg"
export type ToggleGroupVariant = "segment" | "dots"
/** What a segment's option shows: its icon and words, its icon alone, or its icon
 *  alone unless chosen. */
export type ToggleGroupShow = "both" | "icon" | "selected"

/** The room the chosen option's words and its `whileChosen` take, in every option
 *  of a one-of group: the widest option's, measured (consumed as
 *  `w-(--ag-toggle-group-open)`). */
const OPEN = "--ag-toggle-group-open"

/** A dot, and the chosen one stretched: the text's colour at a low alpha, lit and
 *  wider when chosen, the width easing on the same curve as the segment's glide.
 *  Glass: the same in the glass's words' colour, the chosen one with the rim's
 *  light round it. */
const DOT = cn(
  "block h-1.5 w-1.5 rounded-full bg-current/30 transition-[width,background-color] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] motion-reduce:transition-none",
  "group-hover/toggle:bg-current/55 group-data-pressed/toggle:w-4 group-data-pressed/toggle:bg-current/90",
)
const GLASS_DOT =
  "group-data-pressed/toggle:shadow-[0_0_6px_color-mix(in_oklab,currentColor_55%,transparent)]"

const BAR: Record<ToggleGroupSize, string> = {
  sm: "h-7 text-[12px]",
  md: "h-8 text-[13px]",
  lg: "h-10 text-sm",
}
const ITEM: Record<ToggleGroupSize, string> = {
  sm: "h-6 px-2 [&_svg:not([class*='size-'])]:size-3.5",
  md: "h-7 px-2.5 [&_svg:not([class*='size-'])]:size-3.5",
  lg: "h-9 px-3.5 [&_svg:not([class*='size-'])]:size-4",
}

const PLAIN_BAR = "rounded-lg bg-fill-rest"
/** The glass bar is a pill: its radius half of `BAR`'s height. */
const GLASS_BAR: Record<ToggleGroupSize, string> = {
  sm: "[--glass-radius:14px]",
  md: "[--glass-radius:16px]",
  lg: "[--glass-radius:20px]",
}
/** What lifts the chosen: the page's surface in the well, a lit pane in the glass
 *  (`glassLift`, the glass's own edge). One element either way: the highlight that
 *  glides to the one chosen, or each pressed option's own where several can be. */
const LIFT: Record<Material, string> = {
  plain:
    "rounded-md bg-background shadow-[0_1px_2px_rgb(0_0_0/0.15)] ring-1 ring-foreground/8 scheme-dark:bg-foreground/12",
  glass: glassLift,
}
/** The glass lift is a pill: its radius half of `ITEM`'s height. */
const LIFT_RADIUS: Record<ToggleGroupSize, string> = {
  sm: "[--glass-radius:12px]",
  md: "[--glass-radius:14px]",
  lg: "[--glass-radius:18px]",
}

/** Inherited (inside a `Setting`, nothing set here): the mark on the option that
 *  applies is a ghost, hollow. Plain, a dashed outline where the lift would be;
 *  glass, the lit pane at the material's ghost, its fill a third as strong. */
const GHOST_LIFT: Record<Material, string> = {
  plain: "rounded-md border border-dashed border-current/45",
  glass: cn("bg-current/4", glassEdge, GLASS_GHOST),
}

/** An icon beside words is a step dimmer than them (brighter under the hand); an
 *  icon alone is the label and stays whole. `selected`: alone until chosen. */
const ICON: Record<ToggleGroupShow, string> = {
  both: "opacity-60 group-hover/toggle:opacity-90",
  icon: "",
  selected:
    "group-data-pressed/toggle:opacity-60 group-data-pressed/toggle:group-hover/toggle:opacity-90",
}

const Group = React.createContext<{
  size: ToggleGroupSize
  variant: ToggleGroupVariant
  show: ToggleGroupShow
  material: Material
  multiple: boolean
  ghost: boolean
} | null>(null)

type One = {
  multiple?: false
  value?: string | null
  defaultValue?: string | null
  onValueChange?: (value: string) => void
}
type Many = {
  multiple: true
  value?: readonly string[]
  defaultValue?: readonly string[]
  onValueChange?: (value: string[]) => void
}

export type ToggleGroupProps = (One | Many) & {
  size?: ToggleGroupSize
  /** `segment` (default), a bar; `dots`, a dot per option (`ToggleGroupVariant`). */
  variant?: ToggleGroupVariant
  /** What a segment's option shows (`ToggleGroupShow`): `both` (default), its icon
   *  and words; `icon`, the icon alone; `selected`, the icon alone unless chosen.
   *  `icon` and `selected` need an `icon` on every option; `selected` is one-of. */
  show?: ToggleGroupShow
  /** What the root is, when a control is built on the group (sort's `sort`). */
  "data-slot"?: string
  material?: Material
  /** The colour scheme it stands in (material): default the nearest provider's. */
  scheme?: Scheme
  disabled?: boolean
  /** Shown, never changed here: a press changes nothing, the options stay in the
   *  Tab order and the arrows still move between them. Never dimmed (that is
   *  `disabled`). A field's read-only and a locked setting's hold it the same way. */
  readOnly?: boolean
  /** What is being chosen, for assistive tech. */
  "aria-label"?: string
  /** Or the label that names it (a field's: `aria-labelledby={c.labelId}`). */
  "aria-labelledby"?: string
  /** What else says something about it (a field's description and error). */
  "aria-describedby"?: string
  className?: string
  children: React.ReactNode
}

const list = (v: string | null | undefined) => (v ? [v] : [])

export function ToggleGroup(props: ToggleGroupProps) {
  const {
    size = "md",
    variant = "segment",
    show = "both",
    material,
    scheme,
    disabled,
    "aria-label": label,
    "data-slot": slot = "toggle-group",
    className,
    children,
  } = props
  if (show === "selected" && props.multiple)
    throw new Error(
      'ToggleGroup: show="selected" shows the words of the one chosen option; a multiple group has several',
    )
  const m = useMaterial(material)
  const field = useField()
  // Inside a setting: the chosen option's mark a ghost while the value is
  // inherited, described by where it comes from, held still while locked there.
  const setting = useSetting()
  const ghost = setting?.inherited ?? false
  const readOnly =
    (props.readOnly ?? false) ||
    (field?.readOnly ?? false) ||
    (setting?.readOnly ?? false)
  const multiple = props.multiple === true
  const bar = React.useRef<HTMLDivElement>(null)
  // Over a picture the bar is read by the part of it under the bar.
  const [s, here] = useSurfaceScheme(scheme, bar)
  // One: the highlight's place, measured off the pressed option itself, so it fits
  // labels of any width.
  const [at, setAt] = React.useState<{ left: number; width: number } | null>(
    null,
  )
  // One: the room every option keeps for what shows only while chosen, the widest
  // option's (each one's content measured at its own width, collapsed or not).
  const [open, setOpen] = React.useState<number | null>(null)
  React.useLayoutEffect(() => {
    if (multiple) return
    const contents = bar.current?.querySelectorAll<HTMLElement>(
      "[data-slot=toggle-group-open] > span",
    )
    const widest = contents?.length
      ? Math.max(...Array.from(contents, (c) => c.offsetWidth))
      : null
    if (widest !== open) setOpen(widest)
    const chosen = bar.current?.querySelector<HTMLElement>(
      "[data-slot=toggle-group-item][data-pressed]",
    )
    const next = chosen
      ? { left: chosen.offsetLeft, width: chosen.offsetWidth }
      : null
    if (next?.left !== at?.left || next?.width !== at?.width) setAt(next)
  })
  // One, uncontrolled: the group holds the choice itself, not Base UI, so a press
  // renders the group and the highlight above is measured again (held inside Base UI
  // a press changed the pressed option and left the highlight where it was).
  const [own, setOwn] = React.useState(
    multiple ? null : ((props as One).defaultValue ?? null),
  )
  const controlled = props.value !== undefined
  const value = multiple
    ? (props as Many).value
    : list(controlled ? (props as One).value : own)
  const defaultValue = multiple ? (props as Many).defaultValue : undefined
  const look = React.useMemo(
    () => ({ size, variant, show, material: m, multiple, ghost }),
    [size, variant, show, m, multiple, ghost],
  )
  const dots = variant === "dots"
  const described =
    [
      props["aria-describedby"] ?? field?.wiring["aria-describedby"],
      setting?.says,
    ]
      .filter(Boolean)
      .join(" ") || undefined
  return (
    <MaterialProvider material={m} scheme={s}>
      <Group.Provider value={look}>
        <BaseToggleGroup
          ref={here}
          data-slot={slot}
          data-size={size}
          data-show={dots ? undefined : show}
          style={
            open === null
              ? undefined
              : ({ [OPEN]: `${open}px` } as React.CSSProperties)
          }
          data-material={m}
          data-inherited={ghost ? "" : undefined}
          data-readonly={readOnly ? "" : undefined}
          aria-label={label}
          // Inside a field: named by its label and described by it, unless named
          // here; described by a setting's words too.
          aria-labelledby={
            props["aria-labelledby"] ?? (label ? undefined : field?.labelId)
          }
          aria-describedby={described}
          aria-invalid={field?.wiring["aria-invalid"]}
          multiple={multiple}
          disabled={disabled ?? field?.wiring.disabled}
          value={value as string[] | undefined}
          defaultValue={defaultValue as string[] | undefined}
          onValueChange={(next, details) => {
            // Read-only: a press changes nothing, the focus stays where it is.
            if (readOnly) return details.cancel()
            if (multiple) return (props as Many).onValueChange?.(next)
            const [chosen] = next
            if (chosen === undefined) {
              // One, always one: pressing the chosen one keeps it. While its value
              // is inherited, that press says "this one, set here": reported.
              const kept = controlled ? (props as One).value : own
              if (ghost && kept) (props as One).onValueChange?.(kept)
              return
            }
            if (!controlled) setOwn(chosen)
            ;(props as One).onValueChange?.(chosen)
          }}
          className={cn(
            "relative inline-flex w-fit items-center data-disabled:opacity-50",
            schemeOf(s),
            dots
              ? // No bar: the dots stand on what is round them.
                "gap-0"
              : [
                  "gap-0.5 p-0.5",
                  BAR[size],
                  m === "glass"
                    ? [glassVariants({ shape: "surface" }), GLASS_BAR[size]]
                    : PLAIN_BAR,
                ],
            className,
          )}
        >
          {!multiple && !dots && (
            <span
              aria-hidden
              data-slot="toggle-group-highlight"
              className={cn(
                "pointer-events-none absolute top-0.5 bottom-0.5 transition-[left,width,opacity] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] motion-reduce:transition-none",
                ghost ? GHOST_LIFT[m] : LIFT[m],
                m === "glass" && LIFT_RADIUS[size],
                at ? "opacity-100" : "opacity-0",
              )}
              style={at ?? undefined}
            />
          )}
          {children}
        </BaseToggleGroup>
      </Group.Provider>
    </MaterialProvider>
  )
}

export type ToggleGroupItemProps = Omit<
  React.ComponentProps<typeof BaseToggle>,
  "className"
> & {
  /** Its name, when it shows only a glyph. */
  "aria-label"?: string
  /** What kind of option it is, before its words (the group's `show` says when it
   *  stands alone). */
  icon?: React.ReactNode
  /** Shown after its words only while it is chosen (sort's direction). Every option
   *  of a one-of group keeps the widest one's room for it, so the bar never changes
   *  width. */
  whileChosen?: React.ReactNode
  /** Its tooltip. Default: its words, wherever `show` hides them. */
  tip?: React.ReactNode
  className?: string
}

/** One option of the group around it. Its children are its words. */
export function ToggleGroupItem({
  className,
  children,
  icon,
  whileChosen,
  tip,
  ...props
}: ToggleGroupItemProps) {
  const group = React.useContext(Group)
  if (!group) throw new Error("ToggleGroupItem: place it inside a ToggleGroup")
  const glass = group.material === "glass"
  if (group.variant === "dots")
    return (
      <BaseToggle
        className={cn(
          "group/toggle grid size-6 shrink-0 cursor-pointer place-items-center rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring/60 data-disabled:cursor-not-allowed data-disabled:opacity-50 in-data-readonly:cursor-default",
          className,
        )}
        {...props}
        // After the props: a trigger rendering it (a tooltip's) never renames it.
        data-slot="toggle-group-item"
      >
        <span aria-hidden className={cn(DOT, glass && GLASS_DOT)} />
        {/* Its words, said and never shown. */}
        <span className="sr-only">{children}</span>
      </BaseToggle>
    )
  const { show } = group
  if (show !== "both" && icon == null)
    throw new Error(
      `ToggleGroupItem: show="${show}" draws an option by its icon; give each one an \`icon\``,
    )
  // `selected`: the words open with the chosen option; `icon`: never shown, said.
  const words = show === "selected" ? null : children
  const opens = show === "selected" || whileChosen != null
  const item = (
    <BaseToggle
      className={cn(
        "group/toggle relative isolate inline-flex shrink-0 cursor-pointer items-center justify-center gap-1.5 font-medium whitespace-nowrap transition-[color,background-color,opacity,scale] duration-200 select-none data-disabled:cursor-not-allowed data-disabled:opacity-50 in-data-readonly:cursor-default [&_svg]:shrink-0",
        ITEM[group.size],
        glass
          ? "rounded-full ease-[cubic-bezier(0.34,1.56,0.64,1)] active:scale-[1.06] motion-reduce:active:scale-100 in-data-readonly:active:scale-100"
          : "rounded-md",
        // Inherited, the option that applies is lit a step less than one set here.
        glass
          ? cn(
              "opacity-60 hover:opacity-90 data-pressed:opacity-100",
              group.ghost && "data-pressed:opacity-80",
            )
          : cn(
              "text-foreground/55 hover:text-foreground/85 data-pressed:text-foreground",
              group.ghost && "data-pressed:text-foreground/75",
            ),
        className,
      )}
      {...props}
      data-slot="toggle-group-item"
    >
      {/* Several: each pressed option lifts on its own, under its words. */}
      {group.multiple && (
        <span
          aria-hidden
          data-slot="toggle-group-lift"
          className={cn(
            "pointer-events-none absolute inset-0 -z-10 opacity-0 transition-opacity duration-200 group-data-pressed/toggle:opacity-100 motion-reduce:transition-none",
            group.ghost ? GHOST_LIFT[group.material] : LIFT[group.material],
            glass && LIFT_RADIUS[group.size],
          )}
        />
      )}
      {icon != null && (
        <span
          aria-hidden
          data-slot="toggle-group-icon"
          className={cn(
            "inline-flex transition-opacity duration-200 motion-reduce:transition-none",
            ICON[show],
          )}
        >
          {icon}
        </span>
      )}
      {show === "icon" ? <span className="sr-only">{children}</span> : words}
      {opens && (
        // Collapsed it takes no room and gives back the gap before it; open, the
        // widest option's room (one-of) or its own (several), its content centred.
        <span
          data-slot="toggle-group-open"
          className={cn(
            "-ms-1.5 inline-flex w-0 justify-center overflow-hidden group-data-pressed/toggle:ms-0",
            group.multiple
              ? "group-data-pressed/toggle:w-auto"
              : "group-data-pressed/toggle:w-(--ag-toggle-group-open)",
          )}
        >
          <span className="inline-flex w-max items-center gap-1.5 opacity-0 transition-opacity duration-200 group-data-pressed/toggle:opacity-100 motion-reduce:transition-none">
            {show === "selected" && children}
            {whileChosen}
          </span>
        </span>
      )}
    </BaseToggle>
  )
  const says = tip ?? (show === "both" ? null : children)
  return says == null ? item : <Tip content={says}>{item}</Tip>
}
