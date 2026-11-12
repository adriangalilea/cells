"use client"

// The HUD: what an app tells a person without being asked, in one column at the
// window's top end, under the page's header. Its kinds and laws are lib/hud-core's.
//
//   <Hud />                                       once per page, in the layout
//   hud.toast("exported wall.mov", { detail: "in your downloads" })
//   hud.post({ key: "export", kind: "progress", title: "exporting wall", progress: 0.04, reveal: 0 })
//   hud.update("export", (e) => ({ ...e, progress: 0.5, detail: "writing" }))
//   hud.end("export", { toast: "exported wall.mov" })      or { problem: "it stopped at 62%" }
//   hud.reconcile("library", facts.map(toEntry))  everything a source holds now
//   hud.promise(save(), { progress: "saving", toast: "saved", problem: (e) => String(e) })
//   hud.read("−3 s")                              the live value under the hand
//
// `hud` can be called from anywhere, inside React or out (an action, a fetch's catch).
// Every entry is one width (22.5rem) and two lines tall, whatever it says, in arrival
// order, never reordered; progress is a band along its foot. An entry's rows open in a
// panel beside the column, so opening one moves no other, and a row's verb shows its
// work on that row. What does not fit in half the window folds into one last row,
// never evicted, with what ended this session under "earlier". While the pointer or the
// keyboard is in the column nothing moves: what ends keeps its room, inert, a row that
// leaves stays where it stood, what arrives is counted; what a press there makes (an
// undo's toast) shows at once. A toast's time runs only while a page drawing it is seen.
// What the store says (`said`) is read out from live regions apart from the drawing, a
// problem and a refusal at once. F6 moves the keyboard into the column and back.
//
// ONE Hud per page (a second throws). It draws in the body, inside whatever element is
// fullscreen while one is, or in the `container` it is given. `top` is where it stands
// (under the shell's bar, `--shell-bar`, by default). `scopes` are the entries it shows
// (default every one).

import {
  ChevronLeft,
  CircleAlert,
  CircleCheck,
  CircleX,
  Info,
  X,
} from "lucide-react"
import * as React from "react"
import { createPortal } from "react-dom"
import { cn } from "@/lib/utils"
import {
  createHud,
  dismissible,
  foldOrder,
  type HudEntry,
  type HudKept,
  type HudReach,
  type HudRow,
  type HudTone,
  toneOf,
} from "@/lib/hud-core"
import { Button } from "@/components/ui/button"
import {
  type Material,
  type Scheme,
  useMaterial,
  useSurfaceScheme,
} from "@/components/ui/material"
import {
  FLARE_INK,
  FLARE_LIGHT,
  glassVariants,
  schemeOf,
} from "@/components/ui/material-glass"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Spinner } from "@/components/ui/spinner"

export type {
  HudAction,
  HudEntry,
  HudKind,
  HudOutcome,
  HudProgress,
  HudRow,
  HudTone,
} from "@/lib/hud-core"

/** Where dismissals are kept between visits. */
const REMEMBERED = "ag-hud-dismissed"

/** The page's one HUD: every call goes here, the <Hud /> draws it. Its memory lives in
 *  `localStorage`, which a browser may refuse (private windows, a full quota): then
 *  dismissals last the visit, and the page is told once in the console. */
export const hud = createHud({
  memory: {
    read: () => {
      try {
        return JSON.parse(
          localStorage.getItem(REMEMBERED) ?? "{}",
        ) as Partial<HudKept>
      } catch (error) {
        console.warn(
          "hud: dismissals could not be read; they last this visit",
          error,
        )
        return {}
      }
    },
    write: (kept) => {
      try {
        localStorage.setItem(REMEMBERED, JSON.stringify(kept))
      } catch (error) {
        console.warn(
          "hud: dismissals could not be kept; they last this visit",
          error,
        )
      }
    },
  },
})

/** The HUD's state, re-read on every change. */
export function useHud() {
  return React.useSyncExternalStore(hud.subscribe, hud.state, hud.state)
}

/** Each tone as the tokens name it. */
const TONE: Record<HudTone, string | null> = {
  ok: "var(--tone-ok)",
  warn: "var(--tone-warn)",
  bad: "var(--tone-bad)",
  info: "var(--tone-info)",
  neutral: null,
}

const PLAIN_FACE =
  "rounded-xl border border-border bg-popover text-popover-foreground shadow-lg"
const GLASS_FACE = cn(
  glassVariants({ shape: "surface" }),
  "[--glass-radius:--theme(--radius-2xl)] [--glass-blur:18px]",
)
/** An entry arrives fading in from its trailing edge; still under reduced motion. */
const ARRIVES =
  "transition-[opacity,translate] duration-150 starting:translate-x-2 starting:opacity-0 motion-reduce:transition-none"
/** The gap between entries, in px: what an entry's offset is counted in. */
const GAP = 8
/** How long a leaving entry fades before its room closes, ms. */
const LEAVES = 150

/** Huds mounted now: one at most (the header says why). */
let mounted = 0

/** The element fullscreen right now, which the HUD must be drawn inside to be seen. */
function useFullscreenElement(): Element | null {
  const [el, setEl] = React.useState<Element | null>(null)
  React.useEffect(() => {
    const read = () => setEl(document.fullscreenElement)
    read()
    document.addEventListener("fullscreenchange", read)
    return () => document.removeEventListener("fullscreenchange", read)
  }, [])
  return el
}

/** The entries as drawn: each that left kept a moment, fading, before its room closes. */
function useLeaving(entries: readonly HudEntry[]) {
  const [leaving, setLeaving] = React.useState<HudEntry[]>([])
  const last = React.useRef<readonly HudEntry[]>(entries)
  React.useEffect(() => {
    const now = new Set(entries.map((e) => e.key))
    const gone = last.current.filter((e) => !now.has(e.key))
    last.current = entries
    if (gone.length === 0) return
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return
    setLeaving((l) => [...l, ...gone])
    const t = window.setTimeout(
      () => setLeaving((l) => l.filter((e) => !gone.includes(e))),
      LEAVES,
    )
    return () => window.clearTimeout(t)
  }, [entries])
  return leaving
}

/** The keyboard is in the column or its panel: the controls there are Tab's stops,
 *  and only then (Tab from the page never stops in the HUD). */
const KeysIn = React.createContext(false)

/** What is opened beside the column: an entry's rows, or what the fold holds. */
type Opened = { entry: string } | "fold" | null

/** The HUD, drawn: once per page. */
export function Hud({
  top = "calc(var(--shell-bar, 0px) + 1rem)",
  scopes,
  material,
  scheme,
  container,
}: {
  /** Where the column starts, a CSS length: under the shell's bar by default. */
  top?: string
  /** The entries it shows by their scope ("app" when they name none); every one by
   *  default. */
  scopes?: string[]
  material?: Material
  scheme?: Scheme
  /** Where it is drawn. Default: the body, or the fullscreen element while there is
   *  one. */
  container?: Element | null
}) {
  React.useEffect(() => {
    mounted += 1
    if (mounted > 1) {
      mounted -= 1
      throw new Error(
        "a second <Hud> is mounted: every hud call goes to one store, so it would be drawn and announced twice; mount one Hud per page",
      )
    }
    return () => {
      mounted -= 1
    }
  }, [])
  const host = React.useId()
  const state = useHud()
  const m = useMaterial(material)
  const fullscreen = useFullscreenElement()
  const [opened, setOpened] = React.useState<Opened>(null)
  const [room, setRoom] = React.useState(0)
  const [entryHeight, setEntryHeight] = React.useState(0)
  const [panelOffset, setPanelOffset] = React.useState(0)
  const [panelHeight, setPanelHeight] = React.useState(0)
  const panelBox = React.useRef<HTMLDivElement>(null)
  /** The keyboard is in the column or its panel: their controls are Tab's stops. */
  const [keysIn, setKeysIn] = React.useState(false)
  React.useEffect(() => {
    const el = panelBox.current
    if (!el) {
      setPanelHeight(0)
      return
    }
    const o = new ResizeObserver(() => setPanelHeight(el.offsetHeight))
    o.observe(el)
    return () => o.disconnect()
  })
  const column = React.useRef<HTMLElement>(null)
  const before = React.useRef<HTMLElement | null>(null)
  const [target, setTarget] = React.useState<Element | null>(null)
  /** The keys shown when a hand came in: what the hand stays over. */
  const [still, setStill] = React.useState<string[] | null>(null)

  const shows = React.useCallback(
    (e: HudEntry) => !scopes || scopes.includes(e.scope ?? "app"),
    [scopes],
  )
  const scopeKey = scopes?.join(" ")

  React.useEffect(() => setTarget(document.body), [])
  React.useEffect(() => {
    const said = () =>
      hud.seen(
        host,
        document.visibilityState === "visible",
        scopeKey?.split(" "),
      )
    said()
    document.addEventListener("visibilitychange", said)
    const measure = () => setRoom(window.innerHeight / 2)
    measure()
    window.addEventListener("resize", measure)
    return () => {
      hud.seen(host, false)
      hud.hand(`${host} column`, false)
      hud.hand(`${host} panel`, false)
      document.removeEventListener("visibilitychange", said)
      window.removeEventListener("resize", measure)
    }
  }, [host, scopeKey])

  const entries = state.entries.filter(shows)
  const held = state.held.filter(shows).length
  // What was put away and still holds, in key order: the fold row shows while any is.
  const away = [...state.away.values()]
    .filter(shows)
    .sort((a, b) => (a.key < b.key ? -1 : a.key > b.key ? 1 : 0))
  const earlier = state.earlier.filter(
    ({ entry }) => !state.away.has(entry.key),
  )
  const fits =
    entryHeight > 0
      ? Math.max(1, Math.floor((room + GAP) / (entryHeight + GAP)))
      : Number.POSITIVE_INFINITY
  // The column as the hand found it, while it is in.
  React.useEffect(() => {
    if (!state.frozen) setStill(null)
    else
      setStill(
        (s) =>
          s ??
          lay(state.entries.filter(shows), 0, fits, null).shown.map(
            (e) => e.key,
          ),
      )
  }, [state.frozen, state.entries, shows, fits])
  const laid = lay(entries, held, fits, still)
  const leaving = useLeaving(laid.shown)

  // An open panel whose entry left, or ended under the hand, closes.
  React.useEffect(() => {
    if (opened === null || opened === "fold") return
    if (
      !state.entries.some((e) => e.key === opened.entry) ||
      state.ended.has(opened.entry)
    )
      setOpened(null)
  }, [opened, state.entries, state.ended])

  // F6: the keyboard into the column and back to where it was; nothing when the column
  // holds nothing.
  React.useEffect(() => {
    const f6 = (e: KeyboardEvent) => {
      if (e.key !== "F6") return
      const el = column.current
      if (!el) return
      if (el.contains(document.activeElement)) {
        e.preventDefault()
        const back = before.current
        before.current = null
        if (back?.isConnected) back.focus()
        else (document.activeElement as HTMLElement | null)?.blur()
        return
      }
      const first = el.querySelector<HTMLElement>("[data-slot=hud-entry]")
      if (!first) return
      e.preventDefault()
      before.current = document.activeElement as HTMLElement | null
      first.focus()
    }
    window.addEventListener("keydown", f6)
    return () => window.removeEventListener("keydown", f6)
  }, [])

  const openedEntry =
    opened && opened !== "fold"
      ? state.entries.find((e) => e.key === opened.entry)
      : undefined
  // The panel stands in the column's region (its room, half the window): level with what
  // it opens, risen as far as it must to end inside the region. While a hand is in, its
  // rise holds from its first measured layout, so a row joining its foot never lifts the
  // rows above.
  const openedId =
    opened === null ? null : opened === "fold" ? "fold" : opened.entry
  const rise = Math.max(0, Math.min(panelOffset, room - panelHeight))
  const [heldRise, setHeldRise] = React.useState<{
    opened: string
    rise: number
  } | null>(null)
  React.useLayoutEffect(() => {
    if (!state.frozen) setHeldRise(null)
    else if (openedId !== null && panelHeight > 0)
      setHeldRise((h) =>
        h?.opened === openedId ? h : { opened: openedId, rise },
      )
  }, [state.frozen, openedId, panelHeight, rise])
  const hand = (part: string) => ({
    onPointerEnter: () => hud.hand(`${host} ${part}`, true),
    onPointerLeave: () => hud.hand(`${host} ${part}`, false),
    onFocus: () => {
      hud.hand(`${host} ${part} keys`, true)
      setKeysIn(true)
    },
    onBlur: (e: React.FocusEvent) => {
      const next = e.relatedTarget as Node | null
      if (e.currentTarget.contains(next)) return
      hud.hand(`${host} ${part} keys`, false)
      const root = column.current?.parentElement
      if (!root?.contains(next)) setKeysIn(false)
    },
  })

  const where = container ?? fullscreen ?? target
  if (!where) return null
  return createPortal(
    <KeysIn.Provider value={keysIn}>
      <div
        data-slot="hud"
        className="pointer-events-none fixed inset-x-4 z-[60] flex flex-row-reverse items-start gap-2 sm:right-6"
        style={{ top }}
        // A control is pressed by a key's first press alone: a held key's repeats press
        // nothing, so a yes armed while ⏎ or space is held is never pressed by the hold,
        // and a held ⏎ opens a panel once.
        onKeyDownCapture={(e) => {
          if (e.repeat && (e.key === "Enter" || e.key === " ")) {
            e.preventDefault()
            e.stopPropagation()
          }
        }}
      >
        <section
          ref={column}
          aria-label="notifications"
          className="pointer-events-auto flex w-[min(22.5rem,calc(100vw-2rem))] flex-col gap-2"
          {...hand("column")}
          onKeyDown={(e) => {
            const items = [
              ...(column.current?.querySelectorAll<HTMLElement>(
                "[data-slot=hud-entry]",
              ) ?? []),
            ]
            const i = items.indexOf(document.activeElement as HTMLElement)
            if (i < 0) return
            if (e.key === "ArrowDown" || e.key === "ArrowUp") {
              e.preventDefault()
              items[
                Math.min(
                  Math.max(i + (e.key === "ArrowDown" ? 1 : -1), 0),
                  items.length - 1,
                )
              ]?.focus()
            } else if (e.key === "Enter") {
              const item = items[i] as HTMLElement
              const key = item.dataset.key
              if (item.dataset.rows === undefined || !key) return
              e.preventDefault()
              setPanelOffset(item.offsetTop)
              setOpened((o) =>
                o !== null && o !== "fold" && o.entry === key
                  ? null
                  : { entry: key },
              )
            } else if (e.key === "Escape") {
              e.preventDefault()
              if (opened) setOpened(null)
              else {
                const back = before.current
                before.current = null
                if (back?.isConnected) back.focus()
                else (document.activeElement as HTMLElement | null)?.blur()
              }
            }
          }}
        >
          {[...laid.shown, ...leaving].map((e, i) => {
            const gone = leaving.includes(e)
            return (
              <Entry
                key={gone ? `${e.key} leaving` : e.key}
                entry={e}
                ended={gone || state.ended.has(e.key)}
                leaving={gone}
                open={
                  opened !== null && opened !== "fold" && opened.entry === e.key
                }
                toggle={(offset) => {
                  setPanelOffset(offset)
                  setOpened((o) =>
                    o !== null && o !== "fold" && o.entry === e.key
                      ? null
                      : { entry: e.key },
                  )
                }}
                measure={i === 0 ? setEntryHeight : undefined}
                material={m}
                scheme={scheme}
              />
            )
          })}
          {(laid.folds || away.length > 0) && (
            <Fold
              folded={laid.folded}
              held={held}
              away={away.length}
              open={opened === "fold"}
              toggle={(offset) => {
                setPanelOffset(offset)
                setOpened((o) => (o === "fold" ? null : "fold"))
              }}
              material={m}
              scheme={scheme}
            />
          )}
        </section>
        {(openedEntry || opened === "fold") && (
          <div
            ref={panelBox}
            className="pointer-events-auto w-[min(22.5rem,calc(100vw-2rem))]"
            style={{
              marginTop: heldRise?.opened === openedId ? heldRise.rise : rise,
            }}
            {...hand("panel")}
          >
            <Panel
              title={openedEntry?.title ?? "more"}
              maxHeight={`${Math.max(room, entryHeight)}px`}
              material={m}
              scheme={scheme}
              close={() => setOpened(null)}
            >
              {openedEntry
                ? (openedEntry.rows ?? []).map((r) => (
                    <Row
                      key={r.id}
                      entry={openedEntry.key}
                      row={r}
                      leftRows={state.leftRows.get(openedEntry.key) ?? NO_ROWS}
                      frozen={state.frozen}
                      material={m}
                      scheme={scheme}
                    />
                  ))
                : [
                    ...laid.folded.map((e) => (
                      <Entry
                        key={e.key}
                        entry={e}
                        ended={state.ended.has(e.key)}
                        leaving={false}
                        open={false}
                        toggle={() => setOpened({ entry: e.key })}
                        material={m}
                        scheme={scheme}
                      />
                    )),
                    (away.length > 0 || earlier.length > 0) && (
                      <p
                        key="earlier"
                        className="pt-1 font-medium text-[11px] text-current/55 uppercase tracking-wide"
                      >
                        earlier
                      </p>
                    ),
                    // What was put away and still holds: first, each with its way back.
                    ...away.map((entry) => (
                      <div
                        key={`away ${entry.key}`}
                        className="flex items-baseline gap-2"
                      >
                        <div className="min-w-0 flex-1">
                          <Row
                            entry={null}
                            row={{
                              id: entry.key,
                              title: entry.title,
                              detail: entry.detail,
                              tone: toneOf(entry),
                            }}
                            leftRows={NO_ROWS}
                            frozen={false}
                            material={m}
                            scheme={scheme}
                          />
                        </div>
                        <Button
                          variant="secondary"
                          size="sm"
                          material={m}
                          tabIndex={keysIn ? 0 : -1}
                          onClick={() => hud.bringBack(entry.key)}
                        >
                          show again
                        </Button>
                      </div>
                    )),
                    ...earlier.slice(0, 20).map(({ id, entry }) => (
                      <Row
                        key={`earlier ${id}`}
                        entry={null}
                        row={{
                          id: String(id),
                          title: entry.title,
                          detail: entry.detail,
                          tone: toneOf(entry),
                        }}
                        leftRows={NO_ROWS}
                        frozen={false}
                        material={m}
                        scheme={scheme}
                      />
                    )),
                  ]}
            </Panel>
          </div>
        )}
        <Readout scopes={scopes} material={m} scheme={scheme} />
        <Announcer />
      </div>
    </KeysIn.Provider>,
    where,
  )
}

/** The column as it is laid: what shows, in arrival order, and what folds into one
 *  row (the lowest kind first, then the oldest; problems last). Under the hand
 *  (`still`): what showed stays, the rest is the fold's, after it. */
function lay(
  entries: readonly HudEntry[],
  held: number,
  fits: number,
  still: string[] | null,
) {
  if (still) {
    const kept = new Set(still)
    return {
      shown: still.flatMap((key) => entries.filter((e) => e.key === key)),
      folded: entries.filter((e) => !kept.has(e.key)),
      folds: held > 0 || entries.some((e) => !kept.has(e.key)),
    }
  }
  const room =
    held > 0 || entries.length > fits ? Math.max(0, fits - 1) : entries.length
  const over = entries.length - room
  if (over <= 0)
    return { shown: [...entries], folded: [] as HudEntry[], folds: held > 0 }
  const folding = new Set(
    entries
      .map((e, i) => ({ e, i }))
      .sort((a, b) => foldOrder(a.e.kind) - foldOrder(b.e.kind) || a.i - b.i)
      .slice(0, over)
      .map((x) => x.e.key),
  )
  return {
    shown: entries.filter((e) => !folding.has(e.key)),
    folded: entries.filter((e) => folding.has(e.key)),
    folds: true,
  }
}

/** A surface of the HUD's: an entry, the fold, the panel, the readout. */
function useSurface(
  material: Material,
  scheme: Scheme | undefined,
  tone: HudTone,
) {
  const [s, here] = useSurfaceScheme<HTMLDivElement>(scheme)
  const hue = TONE[tone]
  return {
    s,
    here,
    className: cn(
      schemeOf(s),
      material === "glass" ? GLASS_FACE : PLAIN_FACE,
      hue !== null && [material === "glass" && FLARE_LIGHT, FLARE_INK],
    ),
    style:
      hue === null
        ? undefined
        : ({ "--flare-hue": hue } as React.CSSProperties),
  }
}

const MARK: Record<HudTone, React.ReactNode> = {
  ok: <CircleCheck />,
  warn: <CircleAlert />,
  bad: <CircleX />,
  info: <Info />,
  neutral: <CircleCheck />,
}

/** A mark: work whose share is not known turns; the rest wear their tone's symbol. */
function Mark({ tone, turning }: { tone: HudTone; turning: boolean }) {
  return (
    <span
      aria-hidden
      className="grid size-4 shrink-0 place-items-center text-(--flare-ink,currentColor) [&_svg]:size-4"
    >
      {turning ? <Spinner size="sm" /> : MARK[tone]}
    </span>
  )
}

/** One entry: two lines whatever it says, so every entry is one height. */
function Entry({
  entry,
  ended,
  leaving,
  open,
  toggle,
  measure,
  material,
  scheme,
}: {
  entry: HudEntry
  ended: boolean
  leaving: boolean
  open: boolean
  toggle: (offset: number) => void
  measure?: (height: number) => void
  material: Material
  scheme?: Scheme
}) {
  const surface = useSurface(material, scheme, toneOf(entry))
  // Its controls are Tab's stops only while the keyboard is in the column.
  const stop = React.useContext(KeysIn) ? 0 : -1
  const box = React.useRef<HTMLDivElement | null>(null)
  React.useEffect(() => {
    const el = box.current
    if (!measure || !el) return
    const o = new ResizeObserver(() => measure(el.offsetHeight))
    o.observe(el)
    return () => o.disconnect()
  }, [measure])
  const share = typeof entry.progress === "number" ? entry.progress : null
  const rows = entry.rows ?? []
  const canDismiss = dismissible(entry)
  const chrome = useHud().chrome.get(entry.key)
  const offset = () => box.current?.offsetTop ?? 0
  const percent = share === null ? undefined : `${Math.floor(share * 100)}%`
  return (
    <div
      ref={(node) => {
        box.current = node
        surface.here(node)
      }}
      data-slot="hud-entry"
      data-kind={entry.kind}
      data-material={material}
      // Reached on purpose (F6, then the arrows), never by Tab from the page.
      tabIndex={-1}
      data-key={entry.key}
      data-rows={rows.length > 0 ? "" : undefined}
      className={cn(
        surface.className,
        ARRIVES,
        "relative flex items-center gap-3 overflow-hidden px-4 py-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring",
        ended && "opacity-50",
        leaving && "opacity-0",
      )}
      style={surface.style}
      inert={ended}
    >
      <Mark tone={toneOf(entry)} turning={entry.progress === "unknown"} />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <p className="truncate font-medium leading-5">{entry.title}</p>
        <p className="truncate text-current/65 leading-5" title={entry.detail}>
          {entry.detail ?? " "}
          {percent && <span className="tabular-nums"> · {percent}</span>}
        </p>
      </div>
      <div
        data-slot="hud-entry-actions"
        className="-my-1 flex shrink-0 items-center gap-1"
      >
        {(["action", "cancel"] as const).map((which) => {
          const verb = entry[which]
          if (!verb) return null
          // Held unseen: its place kept, hidden, unpressable, out of reach.
          const shown = verb.shown !== false
          return (
            <PressButton
              // Another verb in the place (its words, or shown where it was held) is
              // another control: its stillness starts afresh.
              key={`${which} ${verb.label} ${shown}`}
              variant={which === "action" ? "secondary" : "ghost"}
              size="sm"
              material={material}
              scheme={surface.s}
              tabIndex={shown ? stop : -1}
              disabled={verb.armed === false || !shown}
              aria-hidden={shown ? undefined : true}
              className={cn(!shown && "invisible")}
              press={(reach) => hud.act(entry.key, which, reach)}
            >
              {verb.label}
            </PressButton>
          )
        })}
        {/* The chevron and the ✕ as they stood when a hand came in: one that would
            appear waits until it leaves, one that goes keeps its place, inert. */}
        {(chrome?.rows ?? rows.length > 0) && (
          <Button
            variant="ghost"
            size="icon-sm"
            corners="pill"
            material={material}
            scheme={surface.s}
            tabIndex={rows.length > 0 ? stop : -1}
            disabled={rows.length === 0}
            aria-hidden={rows.length > 0 ? undefined : true}
            aria-label={open ? "close" : "show each"}
            aria-expanded={open}
            onClick={() => toggle(offset())}
            className={cn(
              "text-current/55 hover:text-current",
              rows.length === 0 && "invisible",
            )}
          >
            <ChevronLeft
              className={cn("transition-transform", open && "rotate-180")}
            />
          </Button>
        )}
        {(chrome?.dismiss ?? canDismiss) && (
          <Button
            variant="ghost"
            size="icon-sm"
            corners="pill"
            material={material}
            scheme={surface.s}
            tabIndex={canDismiss ? stop : -1}
            disabled={!canDismiss}
            aria-hidden={canDismiss ? undefined : true}
            aria-label="dismiss"
            onClick={() => hud.dismiss(entry.key)}
            className={cn(
              "-mr-2 text-current/45 hover:text-current",
              !canDismiss && "invisible",
            )}
          >
            <X />
          </Button>
        )}
      </div>
      {share !== null && (
        <span
          aria-hidden
          className="absolute inset-x-4 bottom-0 h-0.5 origin-left bg-current/15"
        >
          <span
            className="block h-full origin-left bg-(--flare-ink,currentColor)"
            style={{ transform: `scaleX(${share})` }}
          />
        </span>
      )}
    </div>
  )
}

/** What the column holds past its room, what arrived under the hand, and what was put
 *  away and still holds: one row, an entry's height, the problems it holds said in
 *  their tone. */
function Fold({
  folded,
  held,
  away,
  open,
  toggle,
  material,
  scheme,
}: {
  folded: HudEntry[]
  held: number
  away: number
  open: boolean
  toggle: (offset: number) => void
  material: Material
  scheme?: Scheme
}) {
  const problems = folded.filter((e) => e.kind === "problem").length
  const surface = useSurface(
    material,
    scheme,
    problems > 0 ? "warn" : "neutral",
  )
  const box = React.useRef<HTMLDivElement | null>(null)
  const words = [
    folded.length > 0 && `${folded.length} more`,
    held > 0 && `${held} new`,
    away > 0 && `${away} put away`,
  ]
    .filter(Boolean)
    .join(" · ")
  return (
    <div
      ref={(node) => {
        box.current = node
        surface.here(node)
      }}
      data-slot="hud-fold"
      className={cn(
        surface.className,
        "flex items-center gap-3 px-4 py-3 text-sm",
      )}
      style={surface.style}
    >
      <Mark tone={problems > 0 ? "warn" : "neutral"} turning={false} />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <p className="truncate font-medium leading-5">{words}</p>
        <p className="truncate text-(--flare-ink,currentColor)/80 leading-5">
          {problems > 0
            ? `${problems} problem${problems === 1 ? "" : "s"} among them`
            : " "}
        </p>
      </div>
      <Button
        variant="ghost"
        size="icon-sm"
        corners="pill"
        material={material}
        scheme={surface.s}
        tabIndex={React.useContext(KeysIn) ? 0 : -1}
        aria-label={open ? "close" : "show what is folded"}
        aria-expanded={open}
        onClick={() => toggle(box.current?.offsetTop ?? 0)}
        className="-mr-2 text-current/55 hover:text-current"
      >
        <ChevronLeft
          className={cn("transition-transform", open && "rotate-180")}
        />
      </Button>
    </div>
  )
}

/** The panel beside the column: what an entry is about, row by row, scrolling past its
 *  room (`maxHeight`, the window less where it stands). */
function Panel({
  title,
  maxHeight,
  close,
  material,
  scheme,
  children,
}: {
  title: string
  maxHeight: string
  close: () => void
  material: Material
  scheme?: Scheme
  children: React.ReactNode
}) {
  const surface = useSurface(material, scheme, "neutral")
  return (
    <div
      ref={surface.here}
      data-slot="hud-panel"
      className={cn(surface.className, ARRIVES, "flex flex-col text-sm")}
      style={{ maxHeight }}
    >
      <div className="flex items-center gap-2 px-4 pt-3 pb-2">
        <p className="min-w-0 flex-1 truncate font-medium">{title}</p>
        <Button
          variant="ghost"
          size="icon-sm"
          corners="pill"
          material={material}
          scheme={surface.s}
          aria-label="close"
          onClick={close}
          className="-mr-2 text-current/45 hover:text-current"
        >
          <X />
        </Button>
      </div>
      <ScrollArea className="flex min-h-0 flex-col gap-3 px-4 pb-3">
        {children}
      </ScrollArea>
    </div>
  )
}

const ROW_STATE_TONE = { done: "ok", failed: "bad" } as const
const NO_ROWS: ReadonlySet<string> = new Set()

/** How a press reached a control (`HudReach`), read from the press itself: a click by its
 *  count (`detail`; 0 when no pointer made it, a key or an assistive action), a tap by its
 *  `pointerType`; with how long the control had stood where it stands when the press
 *  began, timed from its mounting (another verb in the place is another control, by its
 *  key), from any render that moved it, and from a move seen as a pointer comes over it
 *  (a scroll under a resting pointer). */
function useReach() {
  const box = React.useRef<HTMLElement | null>(null)
  const since = React.useRef(0)
  const at = React.useRef<DOMRect | null>(null)
  const down = React.useRef<{ time: number; touch: boolean } | null>(null)
  const moved = React.useCallback(() => {
    const el = box.current
    if (!el) return
    const r = el.getBoundingClientRect()
    const was = at.current
    if (
      was &&
      was.x === r.x &&
      was.y === r.y &&
      was.width === r.width &&
      was.height === r.height
    )
      return
    at.current = r
    since.current = performance.now()
  }, [])
  // Every render: the first seeds it, any that moved it starts it again.
  React.useLayoutEffect(moved)
  return {
    ref: box,
    onPointerEnter: moved,
    onPointerLeave: moved,
    onPointerDown: (e: React.PointerEvent) => {
      moved()
      down.current = {
        time: performance.now(),
        touch: e.pointerType === "touch",
      }
    },
    reach: (e: React.MouseEvent): HudReach => {
      const d = down.current
      down.current = null
      if (e.detail === 0 || !d) return "key"
      const still = d.time - since.current
      return d.touch
        ? { touch: { still } }
        : { pointer: { still, clicks: e.detail } }
    },
  }
}

/** A HUD verb: its press says how it reached it (`useReach`). */
function PressButton({
  press,
  ...props
}: Omit<React.ComponentProps<typeof Button>, "onClick" | "ref"> & {
  press: (reach: HudReach) => void
}) {
  const { reach, ...handlers } = useReach()
  return <Button {...props} {...handlers} onClick={(e) => press(reach(e))} />
}

/** One row of a panel: its mark, its words whole, its own state, its verbs (run as the
 *  entry was last posted), the rows under it. A row that left under the hand stays,
 *  dim and inert, at any depth; under the hand a row is never shorter than it stood
 *  when the hand came in, so a row losing a verb or a line moves nothing below it. */
function Row({
  entry,
  row,
  leftRows,
  frozen,
  material,
  scheme,
  depth = 0,
}: {
  /** The entry it belongs to; null for what is only read (earlier). */
  entry: string | null
  row: HudRow
  /** The rows of its entry that left under the hand. */
  leftRows: ReadonlySet<string>
  /** A hand is in the column. */
  frozen: boolean
  material: Material
  scheme?: Scheme
  depth?: number
}) {
  const left = leftRows.has(row.id)
  const open = useReach()
  // Its height, and its detail's, held while a hand is in: the row never shorter than the
  // tallest it stood (from when the hand came in, or from its first layout in a panel
  // opened under it), the detail's words held to the lines they stood in.
  const box = React.useRef<HTMLDivElement | null>(null)
  const detail = React.useRef<HTMLParagraphElement | null>(null)
  const [held, setHeld] = React.useState<number | null>(null)
  const [detailHeld, setDetailHeld] = React.useState<{
    height: number
    lines: number
  } | null>(null)
  React.useLayoutEffect(() => {
    if (!frozen) {
      setHeld(null)
      setDetailHeld(null)
      return
    }
    const measure = () => {
      const el = box.current
      if (el) setHeld((h) => Math.max(h ?? 0, el.offsetHeight))
      const p = detail.current
      if (p)
        setDetailHeld(
          (d) =>
            d ?? {
              height: p.offsetHeight,
              lines: Math.max(
                1,
                Math.round(
                  p.offsetHeight /
                    Number.parseFloat(getComputedStyle(p).lineHeight),
                ),
              ),
            },
        )
    }
    measure()
    const o = new ResizeObserver(measure)
    if (box.current) o.observe(box.current)
    return () => o.disconnect()
  }, [frozen])
  const st = row.state ?? "idle"
  const working = typeof st === "object"
  const share = working && typeof st.working === "number" ? st.working : null
  const tone: HudTone =
    st === "done" || st === "failed"
      ? ROW_STATE_TONE[st]
      : working
        ? "info"
        : (row.tone ?? "neutral")
  const [s] = useSurfaceScheme<HTMLDivElement>(scheme)
  return (
    <div
      ref={box}
      data-slot="hud-row"
      className={cn(
        "flex flex-col gap-1",
        depth > 0 && "pl-4",
        left && "opacity-50",
      )}
      style={held === null ? undefined : { minHeight: held }}
      inert={left}
    >
      <div
        className="flex items-start gap-2"
        style={
          TONE[tone]
            ? ({ "--flare-ink": TONE[tone] } as React.CSSProperties)
            : undefined
        }
      >
        <span className="mt-0.5">
          <Mark tone={tone} turning={working && share === null} />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          {row.open && entry ? (
            <button
              type="button"
              title={row.open.tip ?? row.open.label}
              ref={open.ref as React.RefObject<HTMLButtonElement>}
              onPointerEnter={open.onPointerEnter}
              onPointerLeave={open.onPointerLeave}
              onPointerDown={open.onPointerDown}
              onClick={(e) => hud.actRow(entry, row.id, "open", open.reach(e))}
              className="truncate text-left font-medium leading-5 hover:underline"
            >
              {row.title}
            </button>
          ) : (
            <p className="truncate font-medium leading-5">{row.title}</p>
          )}
          {row.detail && (
            <p
              ref={detail}
              className="overflow-hidden text-current/65 text-xs leading-4"
              style={
                detailHeld
                  ? {
                      height: detailHeld.height,
                      display: "-webkit-box",
                      WebkitBoxOrient: "vertical",
                      WebkitLineClamp: detailHeld.lines,
                    }
                  : undefined
              }
            >
              {row.detail}
            </p>
          )}
          {/* The room of a working row's line, kept by every row a press acts on, drawn
              empty while it does not work: work starting moves nothing. */}
          {entry && (
            <span
              aria-hidden
              className={cn(
                "mt-1 block h-0.5",
                share !== null && "bg-current/15",
              )}
            >
              {share !== null && (
                <span
                  className="block h-full origin-left bg-current"
                  style={{ transform: `scaleX(${share})` }}
                />
              )}
            </span>
          )}
          {entry && (row.verbs ?? []).length > 0 && (
            <div className="mt-1 flex flex-wrap gap-1">
              {(row.verbs ?? []).map((v) => {
                const shown = v.shown !== false
                return (
                  <PressButton
                    // Another verb in the place is another control: its stillness
                    // starts afresh.
                    key={`${v.label} ${shown}`}
                    variant={v.prominent ? "default" : "secondary"}
                    size="sm"
                    material={material}
                    scheme={s}
                    title={v.tip}
                    disabled={v.armed === false || !shown}
                    aria-hidden={shown ? undefined : true}
                    tabIndex={shown ? undefined : -1}
                    className={cn(!shown && "invisible")}
                    press={(reach) => hud.actRow(entry, row.id, v.label, reach)}
                  >
                    {/* As wide as every verb that stands in this place. */}
                    <span className="grid">
                      {[...new Set(v.room ?? [])]
                        .filter((words) => words !== v.label)
                        .map((words) => (
                          <span
                            key={words}
                            className="invisible col-start-1 row-start-1"
                          >
                            {words}
                          </span>
                        ))}
                      <span className="col-start-1 row-start-1">{v.label}</span>
                    </span>
                  </PressButton>
                )
              })}
            </div>
          )}
        </div>
      </div>
      {(row.rows ?? []).map((r) => (
        <Row
          key={r.id}
          entry={entry}
          row={r}
          leftRows={leftRows}
          frozen={frozen}
          material={material}
          scheme={scheme}
          depth={depth + 1}
        />
      ))}
    </div>
  )
}

/** The live value under the hand, centred over the page, never taking a press: drawn
 *  where its scope is shown. One for a whole gesture, arriving once, its words changing
 *  in place, as wide as the widest words it has said since it showed, so it never
 *  resizes as the value moves. */
function Readout({
  scopes,
  material,
  scheme,
}: {
  scopes?: string[]
  material: Material
  scheme?: Scheme
}) {
  const { readout } = useHud()
  const surface = useSurface(
    material,
    scheme,
    readout?.refusal ? "warn" : "neutral",
  )
  const shown = !!readout && (!scopes || scopes.includes(readout.scope))
  const box = React.useRef<HTMLDivElement | null>(null)
  // Held for one gesture: another starts from its own words.
  const [widest, setWidest] = React.useState({ gesture: -1, width: 0 })
  const here = surface.here
  const at = React.useCallback(
    (node: HTMLDivElement | null) => {
      here(node)
      box.current = node
    },
    [here],
  )
  const words = readout?.words
  const gesture = readout?.gesture ?? -1
  const held = widest.gesture === gesture ? widest.width : 0
  // biome-ignore lint/correctness/useExhaustiveDependencies: measured again as its words change
  React.useLayoutEffect(() => {
    if (!shown) return
    const node = box.current
    if (!node) return
    // Its own width, the held one set aside.
    const was = node.style.minWidth
    node.style.minWidth = ""
    const width = node.offsetWidth
    node.style.minWidth = was
    setWidest((w) =>
      w.gesture === gesture
        ? { gesture, width: Math.max(w.width, width) }
        : { gesture, width },
    )
  }, [shown, words, gesture])
  if (!readout || !shown) return null
  return (
    <div
      ref={at}
      data-slot="hud-readout"
      aria-hidden
      className={cn(
        surface.className,
        ARRIVES,
        "-translate-x-1/2 -translate-y-1/2 pointer-events-none fixed top-1/2 left-1/2 px-4 py-2 text-center font-medium tabular-nums",
      )}
      style={{ ...surface.style, minWidth: held || undefined }}
    >
      {readout.words}
    </div>
  )
}

/** What the store says, read out once each from live regions apart from the drawing:
 *  a problem and a refusal at once, the rest politely. Every one its own node, so
 *  several arrivals are each said. */
function Announcer() {
  const { said } = useHud()
  return (
    <>
      <div role="status" className="sr-only">
        {said
          .filter((s) => !s.urgent)
          .map((s) => (
            <p key={s.id}>{s.words}</p>
          ))}
      </div>
      <div role="alert" className="sr-only">
        {said
          .filter((s) => s.urgent)
          .map((s) => (
            <p key={s.id}>{s.words}</p>
          ))}
      </div>
    </>
  )
}

/** The rows a producer's problem counts, its dismissed ones left out. */
export function undismissed(key: string, rows: HudRow[]): HudRow[] {
  const gone = hud.dismissedRows(key)
  return rows.filter((r) => !gone.has(r.id))
}
