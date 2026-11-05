"use client"

import { Eye, EyeOff } from "lucide-react"
import * as React from "react"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import {
  type Material,
  MaterialProvider,
  type Scheme,
  useMaterial,
  useSurfaceScheme,
  type WornScheme,
} from "@/components/ui/material"
import {
  GlassEdge,
  glassFront,
  glassVariants,
  schemeOf,
} from "@/components/ui/material-glass"
import {
  GHOST_EDGE,
  GLASS_GHOST,
  useSetting,
} from "@/components/ui/setting-context"

// A text field that looks like one: a sunken well (a faint fill and a hairline edge),
// the edge brightening under the hand and clear on focus, red when the value is
// wrong. Wrong is the browser's verdict on the field's own constraints (`minLength`,
// `type="email"`, `pattern`, `required`: `:user-invalid`, once the person has typed
// and moved on, so no state and no handler) or the app's (`aria-invalid`, for what
// only it can know: a name taken, a key refused). At the button's heights, so a field and its button sit
// as one row. `Input` alone, or inside `InputGroup` with things before and after it
// (an icon, a unit, a key, a clear button) that stay inside the well.
//
// `material="glass"`: a well of glass, a pill like the glass button beside it, its
// rim catching more light under the hand and more again while you type in it, red
// light on the rim when the value is wrong. The glass is the field's wrapper, never
// the <input>: an input draws no ::before, where the glass's rim lives.
//
// Inside a `Setting` whose value is inherited, the field is empty and its
// placeholder is the value that applies, drawn as a value (a ghost: the well
// dashed, the words a step back), not a hint; emptied, it clears what is set here.

export type InputSize = "sm" | "md" | "lg"

const HEIGHT: Record<InputSize, string> = {
  sm: "h-7 text-[12px]",
  md: "h-8 text-[13px]",
  lg: "h-10 text-sm",
}
const PAD: Record<InputSize, string> = {
  sm: "px-2",
  md: "px-2.5",
  lg: "px-3",
}
/** A pill's ends are rounder: the glass field's text stands further in. Its radius
 *  is half its height. */
const GLASS_PAD: Record<InputSize, string> = {
  sm: "px-3 [--glass-radius:14px]",
  md: "px-3.5 [--glass-radius:16px]",
  lg: "px-4 [--glass-radius:20px]",
}

/** The well: everything that makes a box read as a field, on whatever holds it (a
 *  field of chips wears it too, chip-input). */
export const WELL =
  "rounded-md border border-foreground/12 bg-fill-well transition-[border-color,background-color] hover:border-foreground/22 has-[input:focus-visible]:bg-fill-well-on has-[input:focus-visible]:border-foreground/40 has-[input[aria-invalid=true]]:border-tone-bad/70 has-[input:user-invalid]:border-tone-bad/70 has-[input:disabled]:opacity-50"

/** The well in glass. A text field's focus is its rim catching light whichever way
 *  focus came (you type where it is, so a click is as much focus as a Tab); wrong, the
 *  rim takes the registry's red. */
export const GLASS_WELL =
  "hover:[--glass-rim:1.3]! focus-within:[--glass-rim:1.7]! has-[input[aria-invalid=true]]:[--glass-stain:var(--tone-bad)]! has-[input[aria-invalid=true]]:[--glass-rim-color:var(--tone-bad)]! has-[input[aria-invalid=true]]:[--glass-rim:2.2]! has-[input:user-invalid]:[--glass-stain:var(--tone-bad)]! has-[input:user-invalid]:[--glass-rim-color:var(--tone-bad)]! has-[input:user-invalid]:[--glass-rim:2.2]! has-[input:disabled]:opacity-50"

/** The bare text, for inside a well. */
const FIELD =
  "min-w-0 flex-1 bg-transparent outline-none disabled:cursor-not-allowed"

/** Inherited (inside a `Setting`, nothing set here): the field is empty and its
 *  placeholder IS the value that applies, so it reads as a value, not a hint: in
 *  the words' ink a step back (a hint's is fainter), the well unfilled and dashed;
 *  glass, the material's ghost. */
const GHOST: Record<Material, string> = {
  plain: cn(
    "relative border-transparent bg-transparent [&_input]:placeholder:text-foreground/60",
    GHOST_EDGE,
  ),
  glass: cn(GLASS_GHOST, "[&_input]:placeholder:text-current/60"),
}

/** What a setting around the field changes: empty and with the fallback as its
 *  placeholder while inherited, emptied it clears, read-only while locked,
 *  described by where its value comes from. */
function useSettingInput({
  value,
  placeholder,
  readOnly,
  onChange,
  "aria-describedby": describedBy,
}: InputProps) {
  const setting = useSetting()
  if (!setting) return { ghost: false, props: {} }
  const ghost = setting.inherited
  return {
    ghost,
    props: {
      value: ghost ? "" : value,
      placeholder: setting.fallback
        ? String(setting.fallback.value)
        : placeholder,
      readOnly: readOnly || setting.readOnly,
      "aria-describedby":
        [describedBy, setting.says].filter(Boolean).join(" ") || undefined,
      onChange: (e: React.ChangeEvent<HTMLInputElement>) => {
        if (e.target.value === "" && setting.clear) setting.clear()
        else onChange?.(e)
      },
    } satisfies Partial<InputProps>,
  }
}

export type InputProps = Omit<React.ComponentPropsWithRef<"input">, "size"> & {
  size?: InputSize
  material?: Material
  /** The colour scheme it stands in (material): default the nearest provider's. */
  scheme?: Scheme
}

/** The wrapper's face in either material, in its scheme. */
function well(m: Material, size: InputSize, s: WornScheme) {
  return m === "glass"
    ? [
        schemeOf(s),
        glassVariants({ shape: "surface" }),
        GLASS_WELL,
        GLASS_PAD[size],
        "[&_input]:placeholder:text-current/40",
      ]
    : [schemeOf(s), WELL, PAD[size], "[&_input]:placeholder:text-foreground/35"]
}

/** The field's FRONT SURFACE, down its end: the strip its buttons stand on (show,
 *  copy, clear), part of the field and not a piece laid on it, the same front a
 *  card has across its head, raised a step off the well: flush to the field's
 *  right edge and its corners, full height, one hairline as its inner edge. It sits
 *  inside the field's edge (a border plain; glass, `GlassEdge` drawn over it), so
 *  that edge runs unbroken round both, and clips what it holds to the field's
 *  corners. Its buttons are born for it (`frontButton`): square to the field's
 *  height, square-cornered, filling it. */
const FRONT_WIDTH: Record<InputSize, string> = {
  sm: "w-7",
  md: "w-8",
  lg: "w-10",
}

/** A button on a field's front, given by whoever puts it there (`className`):
 *  the front's height, square to the field, its corners the front's. */
export function frontButton(size: InputSize) {
  return cn("h-auto self-stretch rounded-none", FRONT_WIDTH[size])
}

function EndFront({ m, children }: { m: Material; children: React.ReactNode }) {
  return (
    <span
      data-slot="input-actions"
      className={cn(
        "relative flex shrink-0 items-stretch self-stretch overflow-hidden rounded-r-[inherit]",
        m === "glass"
          ? cn(
              glassFront.end,
              // A pill's end is a half circle, so the last button's shape is a
              // square ending in one: its mass sits 0.093 of the radius left of
              // its box's middle, and an icon in the middle reads pushed into
              // the curve. Padding its right by twice that sets the icon on the
              // shape's centroid instead.
              // `--glass-radius` does not inherit (it is registered so), so the
              // front and its button take the field's explicitly.
              "[--glass-radius:inherit] [&_button:last-child]:[--glass-radius:inherit] [&_button:last-child]:pr-[calc(var(--glass-radius)*0.186)]",
            )
          : "border-foreground/12 border-l bg-fill-band",
      )}
      // A step lighter than the well in both materials, the buttons' surface
      // raised, and translucent, so it is a step off whatever the field stands
      // on. A front of the page's ground (glass) or a card's solid strip (plain,
      // a step darker than the CARD) read as a dark slab wherever the field stood
      // on anything but that card, a glass card's body among them.
      style={
        m === "glass"
          ? ({
              "--glass-front-color":
                "color-mix(in oklab, var(--foreground) 10%, transparent)",
            } as React.CSSProperties)
          : undefined
      }
    >
      {/* Its buttons wear plain: the front is their surface, glass or not. */}
      <MaterialProvider material="plain">{children}</MaterialProvider>
    </span>
  )
}

/** An address a person can be written to: something, `@`, a domain with a dot. The
 *  browser's own email rule takes `adrian@x`, a typo on the public internet. */
export const EMAIL_PATTERN = "[^@\\s]+@[^@\\s]+\\.[^@\\s]+"

/** What a field of this type checks unless told otherwise; given before the caller's
 *  props, so a `pattern` of its own replaces it and `pattern={undefined}` drops it
 *  (an intranet's `admin@localhost`). */
function rules(type: string | undefined) {
  return type === "email" ? { pattern: EMAIL_PATTERN } : undefined
}

/** A field on its own: the well and the text are one element. */
export function Input({
  size = "md",
  material,
  scheme,
  className,
  ...props
}: InputProps) {
  const m = useMaterial(material)
  // Over a picture the well is read by the part of it under the well; `ref` is the
  // <input>'s.
  const [s, here] = useSurfaceScheme<HTMLSpanElement>(scheme)
  const setting = useSettingInput(props)
  return (
    <span
      ref={here}
      data-slot="input"
      data-size={size}
      data-material={m}
      data-inherited={setting.ghost ? "" : undefined}
      className={cn(
        "flex items-center",
        HEIGHT[size],
        well(m, size, s),
        setting.ghost && GHOST[m],
        className,
      )}
    >
      <input
        className={FIELD}
        {...rules(props.type)}
        {...props}
        {...setting.props}
      />
    </span>
  )
}

/** A field with things in the well beside the text: `start` before it (an icon), `end`
 *  after it (a unit, a key, a clear button), set a little apart; `prefix`, text the
 *  value continues (`github.com/`, `@`), joined to it with no gap at all, so the two
 *  read as one address. They sit quiet, in the field's height. */
export function InputGroup({
  size = "md",
  material,
  scheme,
  start,
  prefix,
  end,
  actions,
  className,
  ...props
}: Omit<InputProps, "prefix"> & {
  start?: React.ReactNode
  /** Text the value continues, joined to it: never a gap between them. */
  prefix?: React.ReactNode
  end?: React.ReactNode
  /** Buttons on the field's front down its end (`EndFront`), the strip at its right
   *  (`Button variant="ghost" size="icon"`, a `Copy`). */
  actions?: React.ReactNode
}) {
  const m = useMaterial(material)
  const [s, here] = useSurfaceScheme<HTMLSpanElement>(scheme)
  const setting = useSettingInput(props)
  const quiet = m === "glass" ? "text-current/55" : "text-foreground/45"
  return (
    <span
      ref={here}
      data-slot="input-group"
      data-size={size}
      data-material={m}
      data-inherited={setting.ghost ? "" : undefined}
      className={cn(
        "flex items-center gap-1.5",
        HEIGHT[size],
        well(m, size, s),
        setting.ghost && GHOST[m],
        "[&_svg]:size-3.5 [&_svg]:shrink-0",
        actions && "relative",
        // One edge, drawn once. Plain it is the border, and the front stands
        // inside it. Glass, the front would cover the face's own rim, so the
        // edge is GlassEdge over everything (below) and the face's rim is off.
        actions && m === "glass" && "before:hidden after:hidden",
        className,
      )}
      // The front runs to the field's right edge: no text padding after it.
      style={actions ? { paddingRight: 0 } : undefined}
    >
      {/* What sits in the well stands on the well: it wears the well's scheme. */}
      <MaterialProvider scheme={s}>
        {start && (
          <span
            data-slot="input-start"
            className={cn("flex shrink-0 items-center", quiet)}
          >
            {start}
          </span>
        )}
        {prefix ? (
          // The prefix and the value in one run of text: the group's gap is
          // between things beside the text, never inside it.
          <span className="flex min-w-0 flex-1 items-center">
            <span
              data-slot="input-prefix"
              className={cn("shrink-0 whitespace-pre", quiet)}
            >
              {prefix}
            </span>
            <input
              className={FIELD}
              {...rules(props.type)}
              {...props}
              {...setting.props}
            />
          </span>
        ) : (
          <input
            className={FIELD}
            {...rules(props.type)}
            {...props}
            {...setting.props}
          />
        )}
        {end && (
          <span
            data-slot="input-end"
            className={cn("flex shrink-0 items-center", quiet)}
          >
            {end}
          </span>
        )}
        {actions && <EndFront m={m}>{actions}</EndFront>}
      </MaterialProvider>
      {/* The glass's edge over everything it holds, the front included: a face's
          own rim lies under anything positioned (a glass Card does the same). Its
          radius is the field's own, inherited (`var()` of the property it sets
          would be a cycle). */}
      {actions && m === "glass" && <GlassEdge radius="inherit" scheme={s} />}
    </span>
  )
}

export interface SecretInputProps extends Omit<InputProps, "type" | "prefix"> {
  start?: React.ReactNode
  /** Before the end front, in the well: a strength meter, a key, a sensor. */
  end?: React.ReactNode
}

/** A secret typed in: hidden as it is typed, an eye in the well to show it while the
 *  person checks it, quiet beside the words. `autoComplete` lets AutoFill in (the
 *  browser's, a password manager's): "current-password" (the default) fills one,
 *  "new-password" offers to make one. Wrong is read as for any field (`minLength`).
 *  Swift's `SecretField` is the same. For a secret shown rather than typed, `veiled`. */
export function SecretInput({
  end,
  autoComplete = "current-password",
  size = "md",
  ...props
}: SecretInputProps) {
  const [shown, setShown] = React.useState(false)
  return (
    <InputGroup
      {...props}
      size={size}
      type={shown ? "text" : "password"}
      autoComplete={autoComplete}
      autoCapitalize="off"
      spellCheck={false}
      end={end}
      actions={
        <Button
          data-slot="secret-input-reveal"
          variant="ghost"
          size="icon-sm"
          className={frontButton(size)}
          aria-label={shown ? "hide" : "show"}
          pressed={shown}
          onClick={() => setShown((was) => !was)}
        >
          {shown ? <EyeOff aria-hidden /> : <Eye aria-hidden />}
        </Button>
      }
    />
  )
}
