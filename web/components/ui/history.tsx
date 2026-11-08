"use client"

// UNDO, page-wide: a history (lib/history) owned by a part of the page, its ⌘Z and ⇧⌘Z
// (and Ctrl+Y off a Mac) the page's keys, so they reach it wherever the focus is: in
// it, on a bar or a button that acted on it, or nowhere in particular. key-router
// decides whose they are as it decides every key: the part holding the focus, else the
// one under the pointer, else the one last used, else the newest. A press typed into a
// field is the field's own undo, never the history's.
//
//   const history = useHistory({ root: frame })
//   history.record({ label: "delete 3 rows", undo: () => set(before), redo: () => set(after) })
//
// Every undo and redo says what it took back or made again ("undid: delete 3 rows"), in
// a toast on the page's HUD that offers the way back (redo, or undo again), and to a
// screen reader. A change a person could regret at once (a delete) is recorded with
// `offer: true`: the toast says it was done and offers undo, as the change happens.

import { Redo2, Undo2 } from "lucide-react"
import * as React from "react"
import { cn } from "@/lib/utils"
import {
  createHistory,
  type History,
  type HistoryState,
  type HistoryStep,
  type RecordOptions,
} from "@/lib/history-core"
import { Button } from "@/components/ui/button"
import { hud } from "@/components/ui/hud"
import { useActions } from "@/components/ui/key-router"
import type { MenuVerb } from "@/components/ui/menu"
import { usePlatform } from "@/components/ui/shortcut"
import { Tip } from "@/components/ui/tooltip"

export type {
  History,
  HistoryState,
  HistoryStep,
} from "@/lib/history-core"

/** A history whose undo and redo say what they did, and whose record can offer undo. */
export interface PageHistory extends History {
  record(step: HistoryStep, options?: RecordOptions & { offer?: boolean }): void
}

/** One history for a part of the page, its keys on the page's router. `root` is the
 *  part's element (focus, the pointer or a last press inside it puts it first);
 *  without one it answers only what nothing nearer takes. */
export function useHistory({
  root,
  title = "history",
  limit,
  enabled = true,
}: {
  root?: React.RefObject<Element | null>
  /** What the part is called on the page's sheet of keys, where undo and redo are
   *  listed ("budget.csv"). */
  title?: string
  limit?: number
  /** Off, its keys are not on the router (a picture of the part, an inert one). */
  enabled?: boolean
} = {}): PageHistory {
  const platform = usePlatform()
  const [history] = React.useState<PageHistory>(() => {
    const kernel = createHistory({ limit })
    const said = (step: HistoryStep | null, undid: boolean) => {
      if (!step) return step
      hud.toast(`${undid ? "undid" : "redid"}: ${step.label}`, {
        action: {
          label: undid ? "redo" : "undo",
          onClick: () => (undid ? page.redo() : page.undo()),
        },
      })
      return step
    }
    const page: PageHistory = {
      ...kernel,
      record(step, { offer, ...options } = {}) {
        kernel.record(step, options)
        if (offer)
          hud.toast(step.label, {
            action: { label: "undo", onClick: () => page.undo() },
          })
      },
      undo: () => said(kernel.undo(), true),
      redo: () => said(kernel.redo(), false),
      // A jump is said once, by where it landed, never step by step.
      goTo(count) {
        const moved = kernel.goTo(count)
        if (moved > 0) {
          const at = kernel.state().done.at(-1)
          hud.toast(at ? `back to: ${at.label}` : "back to the start")
        }
        return moved
      },
    }
    return page
  })
  // Undo and redo as the part's actions: listed on the page's sheet under its title,
  // and unavailable (nothing to take back) still the part's while it is in hand, so
  // ⌘Z never undoes another part the person is not in.
  const { done, undone } = useHistoryState(history)
  useActions(
    {
      title,
      actions: [
        {
          id: "undo",
          label: done.at(-1) ? `undo: ${done.at(-1)?.label}` : "undo",
          keys: ["mod+z"],
          section: "edit",
          when: () => history.state().done.length > 0,
          run: () => history.undo(),
        },
        {
          id: "redo",
          label: undone.at(-1) ? `redo: ${undone.at(-1)?.label}` : "redo",
          keys:
            platform === "other" ? ["mod+shift+z", "ctrl+y"] : ["mod+shift+z"],
          section: "edit",
          when: () => history.state().undone.length > 0,
          run: () => history.redo(),
        },
      ],
    },
    { root, enabled },
  )
  return history
}

/** The steps now, re-rendering as they change: what an undo button says and whether it
 *  can be pressed. */
export function useHistoryState(history: History): HistoryState {
  return React.useSyncExternalStore(
    history.subscribe,
    history.state,
    history.state,
  )
}

// ── Its faces. One history, three places it shows, each reading it and never keeping
// a copy: a pair of buttons (a toolbar, a panel's head), its steps as a list to jump
// through (a panel), and its verbs for any menu. Each names the step it would take
// back or make again, so a person knows what a press does before pressing.

/** Undo and redo as a pair of buttons, each named by its step ("undo: delete 3 rows")
 *  in its tooltip with its keys, quiet while there is nothing to take back. */
export function UndoRedo({
  history,
  size = "icon-sm",
  className,
}: {
  history: History
  size?: "icon-sm" | "icon" | "icon-lg"
  className?: string
}) {
  const { done, undone } = useHistoryState(history)
  const back = done.at(-1)
  const ahead = undone.at(-1)
  return (
    <div data-slot="undo-redo" className={cn("flex items-center", className)}>
      <Tip
        content={back ? `undo: ${back.label}` : "nothing to undo"}
        keys="mod+z"
      >
        <Button
          variant="ghost"
          size={size}
          aria-label={back ? `undo: ${back.label}` : "undo"}
          disabled={!back}
          onClick={() => history.undo()}
        >
          <Undo2 />
        </Button>
      </Tip>
      <Tip
        content={ahead ? `redo: ${ahead.label}` : "nothing to redo"}
        keys="mod+shift+z"
      >
        <Button
          variant="ghost"
          size={size}
          aria-label={ahead ? `redo: ${ahead.label}` : "redo"}
          disabled={!ahead}
          onClick={() => history.redo()}
        >
          <Redo2 />
        </Button>
      </Tip>
    </div>
  )
}

/** The steps, oldest first, the start above them: a press on one goes there, undoing
 *  or redoing every step between, as a photo editor's history panel. Steps undone are
 *  struck through, there to go back to until a new change replaces them. */
export function HistoryList({
  history,
  className,
}: {
  history: History
  className?: string
}) {
  const { done, undone } = useHistoryState(history)
  const steps = [...done, ...[...undone].reverse()]
  const row = (label: string, count: number) => {
    const here = count === done.length
    const ahead = count > done.length
    return (
      <li key={count}>
        <button
          type="button"
          aria-current={here ? "step" : undefined}
          onClick={() => history.goTo(count)}
          className={cn(
            "flex w-full cursor-pointer items-center rounded-md px-2 py-1 text-left text-[13px] hover:bg-fill-row-hover",
            here ? "bg-fill-row-on text-foreground" : "text-foreground/70",
            ahead && "text-foreground/40 line-through",
          )}
        >
          {label}
        </button>
      </li>
    )
  }
  return (
    <ol
      data-slot="history-list"
      aria-label="history"
      className={cn("flex flex-col", className)}
    >
      {row("the start", 0)}
      {steps.map((s, i) => row(s.label, i + 1))}
    </ol>
  )
}

/** Undo and redo as a menu's verbs, named by their steps, with their keys: spread into
 *  any menu's entries (`[...historyVerbs(history), "separator", ...]`). */
export function historyVerbs(history: History): MenuVerb[] {
  const { done, undone } = history.state()
  const back = done.at(-1)
  const ahead = undone.at(-1)
  return [
    {
      icon: <Undo2 />,
      label: back ? `undo: ${back.label}` : "undo",
      hint: "mod+z",
      disabled: !back,
      onSelect: () => history.undo(),
    },
    {
      icon: <Redo2 />,
      label: ahead ? `redo: ${ahead.label}` : "redo",
      hint: "mod+shift+z",
      disabled: !ahead,
      onSelect: () => history.redo(),
    },
  ]
}
