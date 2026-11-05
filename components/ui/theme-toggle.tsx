"use client"

// A three-state theme control: light, dark, and FOLLOW THE SYSTEM. Most toggles
// collapse that to a boolean, which quietly throws away the only setting a reader
// actually chose, and stops tracking when they change it at the OS.
//
// It is deliberately provider-agnostic: it takes the value and a setter, so it works
// with next-themes, with a hand-rolled context, or with nothing at all. Depending on
// a provider would make the item unusable to anyone who already has one.

import { Monitor, Moon, Sun } from "lucide-react"
import {
  PreviewPicker,
  type PreviewPickerProps,
} from "@/components/ui/preview-picker"

export type Theme = "light" | "dark" | "system"

const THEMES: { value: Theme; label: string; Icon: typeof Sun }[] = [
  { value: "light", label: "light", Icon: Sun },
  { value: "system", label: "system", Icon: Monitor },
  { value: "dark", label: "dark", Icon: Moon },
]

export interface ThemeToggleProps
  extends Omit<PreviewPickerProps<Theme>, "value" | "label" | "options"> {
  /** The chosen theme, `system` included. NOT the resolved one: a control that shows
   *  `dark` when the reader picked `system` at night is reporting a fact they did not
   *  choose, and the next press then reads as a no-op.
   *
   *  Undefined while a provider is still resolving, which is a real render on every
   *  load: it shows `system`, the provider's own default, rather than nothing, because
   *  a three-way control with no option marked reads as broken, and a reader cannot
   *  tell it apart from one whose selected state is invisible. */
  value: Theme | undefined
  onChange: (theme: Theme) => void
  className?: string
}

export function ThemeToggle({
  value,
  onChange,
  className,
  ...props
}: ThemeToggleProps) {
  return (
    <PreviewPicker
      {...props}
      label="Theme"
      value={value ?? "system"}
      onChange={onChange}
      options={THEMES.map(({ value, label, Icon }) => ({
        value,
        label,
        icon: <Icon aria-hidden />,
      }))}
      size="compact"
      iconsOnly
      className={className}
    />
  )
}
