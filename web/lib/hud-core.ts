// HUD: what an app tells a person without being asked, as data and its laws, with no
// React and no DOM. The kernel under `ui/hud`, Ink's `InkHUD` on the web (swift-utils,
// Sources/Ink/HUD-DESIGN.md is the design both follow).
//
// An ENTRY is keyed by its producer: posted again with its key, it changes in place.
// Its KIND decides when it shows and leaves, never the caller:
//   toast     something happened whose result is not on screen: gone after 5 s of
//             being seen by a host drawing its scope.
//   progress  work under way: shown once it has run past its `reveal` (300 ms: work
//             that ends sooner is never seen), then for 600 ms at least; it ends where
//             it stands, quietly, as a toast or as a problem.
//   problem   something failed or needs the person: stands until its cause is gone or
//             it is dismissed. One a source reconciles (a fact a reload says again) is
//             remembered dismissed with the rows it held until its source stops saying
//             it, or a row it did not hold appears.
//   state     a condition that holds: shown once it has held 1 s, gone 1 s after. ✕
//             hides it until its words change or its source stops saying it.
// The READOUT is the live value under the hand (a seek, a key refused): one at a time,
// standing its words' reading time, or held while a drag holds; a refusal longer than a
// glance stands as a problem instead.
//
// Order is arrival, never reordered. While a hand (the pointer or the keyboard) is in
// the column, and a short grace after: nothing moves. What ends keeps its room
// (`ended`), a row that leaves stays where it stood (`leftRows`), what arrives is counted
// (`held`), and the clock stands still; when it leaves, the column settles once. What a
// press in the column itself makes (an undo's toast) shows at once. The clock runs only
// while a host is seen and no hand is in. What ended this session is `earlier`, newest
// first. Every arrival, end and change of kind is said once (`said`), a problem at once.

/** Every kind, in the order they fold when the column is full (the first first). */
export const HUD_KINDS = ["toast", "progress", "state", "problem"] as const
export type HudKind = (typeof HUD_KINDS)[number]
export type HudTone = "ok" | "warn" | "bad" | "info" | "neutral"
/** How far work is: unknown (a spinner), or a share done, 0..1. */
export type HudProgress = "unknown" | number

export interface HudAction {
  label: string
  onClick: () => void
  /** What the verb will do, said under the hand. */
  tip?: string
  /** The one verb that answers. */
  prominent?: boolean
  /** Pressable now (default true): a verb that answers a question just asked waits
   *  until a second press of the click that asked could no longer land on it. */
  armed?: boolean
  /** Labels its place is as wide as, beside its own: every verb that stands there in
   *  another state of the row, so no verb moves as the row changes. */
  room?: string[]
  /** Drawn (default true): false holds the place of a verb of the row's other state,
   *  unseen and unpressable, so the row never wraps differently. */
  shown?: boolean
  /** It destroys what it names: a press is taken only when it reached the verb on
   *  purpose (`HudReach`), so a press that lands on it because the layout moved it there
   *  never takes. */
  destructive?: boolean
}

/** How a press reached its verb, read from the press itself: a click with how long the
 *  control had stood where it stands (ms since it appeared, moved, or another verb took
 *  its place) when the button went down, and the click's count (`detail`: a double
 *  click's second press is 2); a tap with how long the control had stood still when the
 *  finger landed; or a key or an assistive action, on purpose. */
export type HudReach =
  | { pointer: { still: number; clicks: number } }
  | { touch: { still: number } }
  | "key"

/** How long a destructive verb stands still before a click or a tap on it is taken: a
 *  double click's interval (the web has no system value; 500 ms is the platforms'
 *  default). */
export const HUD_DWELL = 500

/** Whether a person can put an entry away: everything but work, which ends by itself.
 *  The one rule the chrome and the ✕ read. */
export const dismissible = (entry: HudEntry) => entry.kind !== "progress"

export type HudRowState = "idle" | { working: HudProgress } | "done" | "failed"

/** One thing an entry is about (a file), with its own live state and verbs, and the
 *  rows under it. Its words are shown, never logged. */
export interface HudRow {
  /** Stable for the thing it names: what a dismissal is remembered by. */
  id: string
  title: string
  detail?: string
  tone?: HudTone
  state?: HudRowState
  /** A press on its title. */
  open?: HudAction
  verbs?: HudAction[]
  rows?: HudRow[]
}

export interface HudEntry {
  /** The producer's name for what it says, never a person's words or a reason (it is
   *  remembered). */
  key: string
  kind: HudKind
  title: string
  /** One line in the column, whole in the panel. */
  detail?: string
  tone?: HudTone
  progress?: HudProgress
  rows?: HudRow[]
  action?: HudAction
  cancel?: HudAction
  /** Where it belongs: a host draws the scopes it shows. Default "app". */
  scope?: string
  /** Progress: ms it runs before it shows. Default 300; 0 for work a person just
   *  started. */
  reveal?: number
  /** Progress whose result is on screen: it ends silently. */
  transient?: boolean
}

export type HudOutcome =
  | "quietly"
  | { toast: string; detail?: string }
  | { problem: string; detail?: string }

export interface HudReadout {
  words: string
  /** A key or a menu item refused: said at once. */
  refusal: boolean
  /** Where the hand is: a host draws the readouts of the scopes it shows. */
  scope: string
  /** Each reading's own, so the same words read twice are two readings. */
  serial: number
  /** The gesture it belongs to: what the drawing holds its widest width for. */
  gesture: number
}

/** Dismissals kept across reloads: a problem's rows by its key, rows by their entry's.
 *  Read whatever a later build added: a field not there is empty. */
export interface HudKept {
  entries: Record<string, string[]>
  rows: Record<string, string[]>
}

/** Where dismissals are kept (localStorage under a name, say); none forgets them. */
export interface HudMemory {
  read(): Partial<HudKept>
  write(kept: HudKept): void
}

/** One thing said to a screen reader: each its own, a problem's and a refusal's at
 *  once, the rest politely. */
export interface HudSaid {
  id: number
  words: string
  urgent: boolean
}

/** Which of an entry's own controls stand. */
export interface HudChrome {
  rows: boolean
  dismiss: boolean
}

export interface HudState {
  /** Every entry shown now, in arrival order. */
  entries: readonly HudEntry[]
  /** What ended this session, newest first, each its own id (a key can end twice). */
  earlier: readonly { id: number; entry: HudEntry }[]
  /** What a source still says that the person put away (a problem dismissed, a state
   *  hidden, every row of an entry kept), by key: listed under "earlier" with a way
   *  back (`bringBack`), so the column is never the only door to a fact that stands. */
  away: ReadonlyMap<string, HudEntry>
  /** Entries that ended while a hand was in the column: they keep their room, inert. */
  ended: ReadonlySet<string>
  /** Rows that left while a hand was in the column, by their entry's key: they keep
   *  their room, dim and inert. */
  leftRows: ReadonlyMap<string, ReadonlySet<string>>
  /** Each entry's own controls as they stood when a hand came in (the chevron that opens
   *  its rows, the ✕), held while it stays: a control that would appear waits, one that
   *  goes keeps its place, inert. Empty while no hand is in. */
  chrome: ReadonlyMap<string, HudChrome>
  /** Entries that arrived while a hand was in the column: counted, not drawn. */
  held: readonly HudEntry[]
  readout: HudReadout | null
  /** A hand is in a column: nothing moves. */
  frozen: boolean
  /** The scopes a seen host draws ("all" while one draws every scope): a page's own
   *  clocks run only while what they time is seen, as a toast's does. */
  seenScopes: ReadonlySet<string> | "all"
  /** The last few things said, oldest first. */
  said: readonly HudSaid[]
}

export interface Hud {
  state(): HudState
  /** Called after every change; returns the unsubscribe. */
  subscribe(listener: () => void): () => void
  post(entry: HudEntry): void
  update(key: string, change: (entry: HudEntry) => HudEntry): void
  end(key: string, outcome: HudOutcome): void
  /** Everything `source` says now: what it no longer says ends, the rest posts. One
   *  change, one notice. `remembered` false: the source's facts last the session, so a
   *  problem of it put away is remembered for the session only. A row put away is never
   *  forgotten for not being said (a source fills its rows bit by bit): its source
   *  forgets it (`forget`) when the thing it names is gone. */
  reconcile(source: string, entries: HudEntry[], remembered?: boolean): void
  /** The rows of `key` whose things are gone: their dismissals are forgotten. */
  forget(rows: Iterable<string>, key: string): void
  toast(title: string, options?: { detail?: string; action?: HudAction }): void
  /** A promise as work: progress while it runs, a toast or a problem as it settles.
   *  The promise is handed back. */
  promise<V>(
    promise: Promise<V>,
    said: {
      progress: string
      toast: string | ((value: V) => string)
      problem: string | ((error: unknown) => string)
    },
  ): Promise<V>
  read(
    words: string,
    options?: { refusal?: boolean; holding?: boolean; scope?: string },
  ): void
  clearReadout(): void
  /** The entry as last posted: what a press runs (its closures are never compared). */
  latest(key: string): HudEntry | undefined
  /** A source's entry as its producer last gave it, every row it holds, the rows put
   *  away included: what a producer reads beside `dismissedRows` to count what it said
   *  and what was kept. Nothing for an event, which is never said again. */
  given(key: string): HudEntry | undefined
  /** A press on an entry's action or its cancel, found in the entry as last said:
   *  refused while not armed or held unseen and, destructive, when it reached the verb
   *  by layout (a double click's second press, a click or a tap on a verb that had not
   *  stood still `HUD_DWELL`); what it posts shows at once, and a toast's action
   *  dismisses it. */
  act(key: string, which: "action" | "cancel", reach: HudReach): void
  /** A press on a row's verb (by its label), or on its title (`"open"`), found in the
   *  entry as last said (a row that left under the hand, or one under it, takes none);
   *  refused as `act` refuses; what it posts shows at once. */
  actRow(key: string, row: string, verb: string, reach: HudReach): void
  dismiss(key: string): void
  dismissRow(row: string, key: string): void
  /** An entry put away (`away`) shown again: its dismissal, and its rows' when every
   *  row was put away, forgotten, so it stands until it changes or is put away again. */
  bringBack(key: string): void
  dismissedRows(key: string): ReadonlySet<string>
  /** A hand (a host's part by its id) is in a column, or left it. */
  hand(host: string, inside: boolean): void
  /** A host drawing `scopes` is seen, or stopped being seen (the page hidden). */
  seen(host: string, seen: boolean, scopes?: readonly string[]): void
}

const TOAST_SEEN = 5000
const WORK_AT_LEAST = 600
const SETTLE = 1000
const EARLIER_KEPT = 50
const SAID_KEPT = 6
const TICK = 250
/** How long the column stays still after the hand leaves it: crossing a gap between
 *  two entries, or between the column and its panel, is no leaving. */
const HAND_GRACE = 300
/** How long a readout stands after the hand rests: the time to notice it, then each
 *  word's reading (`readingTime`). */
const NOTICE = 500
/** Reading speed for a readout's words: 200 words a minute, a third of a second a
 *  word rounded down. */
const PER_WORD = 300
/** A refusal that takes longer than this to read is no readout: it stands as a
 *  problem. */
const READOUT_MOST = 3000

/** How long `words` stand, ms: the notice, then each word's reading. A number's unit is
 *  part of its word ("−3 s", "40 %" read as one). */
export function readingTime(words: string): number {
  let count = 0
  let afterNumber = false
  for (const token of words.split(/\s+/).filter(Boolean)) {
    const unit = afterNumber && token.length <= 2 && !/\d/.test(token)
    if (!unit) count += 1
    afterNumber = /\d/.test(token)
  }
  return NOTICE + PER_WORD * count
}

/** Where a kind stands in the folding order (`HUD_KINDS`): the lowest folds first. */
export const foldOrder = (kind: HudKind) => HUD_KINDS.indexOf(kind)

/** The tone an entry wears: its own, else its kind's. */
export function toneOf(entry: HudEntry): HudTone {
  if (entry.tone) return entry.tone
  return entry.kind === "problem"
    ? "warn"
    : entry.kind === "toast"
      ? "neutral"
      : "info"
}

/** Two entries saying the same, their verbs' closures aside (JSON drops functions). */
const same = (a: HudEntry, b: HudEntry) =>
  JSON.stringify(a) === JSON.stringify(b)

/** A verb held unseen (`shown: false`) is never armed: one rule, as Ink's precondition. */
function heldUnseenNeverArmed(entry: HudEntry) {
  const check = (v: HudAction | undefined) => {
    if (v && v.shown === false && v.armed !== false)
      throw new Error(`hud: ${v.label} is held unseen, so it is never armed`)
  }
  const rows = (list: readonly HudRow[] | undefined) => {
    for (const row of list ?? []) {
      for (const v of row.verbs ?? []) check(v)
      rows(row.rows)
    }
  }
  check(entry.action)
  check(entry.cancel)
  rows(entry.rows)
}

// Under the hand nothing changes geometry: every row keeps its place, its height, its
// verbs' places and its child rows as they stood; words change in place; what would grow
// (a detail line, a verb, a child row) waits until the hand leaves, and what goes leaves
// its place behind, unseen (a verb) or inert (a row, at any depth). Only a new top-level
// row may join, at the end, where it moves nothing.

const unseen = (v: HudAction): HudAction => ({
  label: v.label,
  onClick: () => {},
  prominent: v.prominent,
  room: v.room,
  armed: false,
  shown: false,
})

/** The words a verb's place holds (its room and its own), as one key. */
const placeWords = (v: HudAction) =>
  [...new Set([...(v.room ?? []), v.label])].sort().join("\n")

/** Verbs by place: each standing place keeps its verb when the fresh one there is the
 *  same verb or one of its place's (`room`), else holds the place unseen; a verb past the
 *  standing places waits. Two verbs share a place only when they are drawn alike there:
 *  the same words held and the same prominence, so the place keeps its width. */
function holdVerbs(
  fresh: readonly HudAction[] | undefined,
  standing: readonly HudAction[] | undefined,
): HudAction[] {
  return (standing ?? []).map((s, i) => {
    const f = fresh?.[i]
    if (
      f &&
      (f.label === s.label ||
        (s.room ?? []).includes(f.label) ||
        (f.room ?? []).includes(s.label))
    ) {
      if (
        placeWords(f) !== placeWords(s) ||
        (f.prominent ?? false) !== (s.prominent ?? false)
      )
        throw new Error(
          `hud: ${f.label} and ${s.label} share a place but are drawn differently`,
        )
      return f
    }
    return unseen(s)
  })
}

/** An entry's control: one that goes holds its place unseen, one that would appear
 *  waits. */
function holdControl(
  fresh: HudAction | undefined,
  standing: HudAction | undefined,
): HudAction | undefined {
  if (!fresh && standing) return unseen(standing)
  if (fresh && !standing) return undefined
  return fresh
}

/** `row` and every row under it, left: inert in place. */
function leaveRow(row: HudRow, left: Set<string>) {
  left.add(row.id)
  for (const child of row.rows ?? []) leaveRow(child, left)
}

function holdRows(
  fresh: readonly HudRow[] | undefined,
  standing: readonly HudRow[] | undefined,
  left: Set<string>,
  top: boolean,
): HudRow[] {
  const now = new Map((fresh ?? []).map((r) => [r.id, r]))
  const rows: HudRow[] = []
  for (const row of standing ?? []) {
    const f = now.get(row.id)
    if (f) {
      rows.push({
        ...f,
        // A detail's line is held: one that would appear waits, one that goes keeps
        // its words.
        detail: row.detail === undefined ? undefined : (f.detail ?? row.detail),
        verbs: holdVerbs(f.verbs, row.verbs),
        rows: holdRows(f.rows, row.rows, left, false),
      })
    } else {
      rows.push(row)
      leaveRow(row, left)
    }
  }
  if (top) {
    const had = new Set((standing ?? []).map((r) => r.id))
    rows.push(...(fresh ?? []).filter((r) => !had.has(r.id)))
  }
  return rows
}

/** A row by its id, at any depth. */
function findRow(
  rows: readonly HudRow[] | undefined,
  id: string,
): HudRow | undefined {
  for (const row of rows ?? []) {
    if (row.id === id) return row
    const deeper = findRow(row.rows, id)
    if (deeper) return deeper
  }
  return undefined
}

interface Life {
  shown: number
  /** A toast's ms left to be seen. */
  seenLeft?: number
  /** When it leaves: a state that stopped holding, work ended before its 600 ms. */
  leaves?: number
}

/** The timers the store runs its laws on: the page's own, or a law's, which steps them
 *  itself rather than wait on the wall clock. */
export interface HudTimers {
  setTimeout(run: () => void, ms: number): unknown
  clearTimeout(id: unknown): void
  setInterval(run: () => void, ms: number): unknown
  clearInterval(id: unknown): void
}

const pageTimers: HudTimers = {
  setTimeout: (run, ms) => setTimeout(run, ms),
  clearTimeout: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
  setInterval: (run, ms) => setInterval(run, ms),
  clearInterval: (id) => clearInterval(id as ReturnType<typeof setInterval>),
}

export function createHud(
  options: { memory?: HudMemory; now?: () => number; timers?: HudTimers } = {},
): Hud {
  const now = options.now ?? (() => Date.now())
  const timers = options.timers ?? pageTimers
  const memory = options.memory
  // Read on first use, never at creation: a page's one HUD is made where no storage
  // exists yet (a server rendering it).
  // What a later build added to the memory, carried through every write untouched.
  let kept: {
    entries: Map<string, Set<string>>
    rows: Map<string, Set<string>>
    others: Record<string, unknown>
  } | null = null
  const keptNow = () => {
    if (kept) return kept
    const read = (memory?.read() ?? {}) as Record<string, unknown>
    const { entries, rows, ...others } = read
    const sets = (v: unknown) =>
      new Map(
        Object.entries((v as Record<string, string[]> | undefined) ?? {}).map(
          ([k, ids]) => [k, new Set(Array.isArray(ids) ? ids : [])],
        ),
      )
    kept = { entries: sets(entries), rows: sets(rows), others }
    return kept
  }
  const remember = () => {
    const k = keptNow()
    memory?.write({
      ...k.others,
      entries: Object.fromEntries(
        [...k.entries].map(([key, v]) => [key, [...v]]),
      ),
      rows: Object.fromEntries([...k.rows].map(([key, v]) => [key, [...v]])),
    } as HudKept)
  }
  /** Sources whose facts last the session: their problems put away are remembered for
   *  the session only. */
  const sessionSources = new Set<string>()
  const sessionDismissed = new Map<string, Set<string>>()
  /** Problems said as they arrived under the hand: not said again when they show. */
  const announcedHeld = new Set<string>()

  let state: HudState = {
    entries: [],
    earlier: [],
    away: new Map(),
    ended: new Set(),
    leftRows: new Map(),
    chrome: new Map(),
    held: [],
    readout: null,
    frozen: false,
    seenScopes: new Set(),
    said: [],
  }
  /** Each shown entry as last posted: its closures, and its data while a hand holds the
   *  column still. */
  const latestByKey = new Map<string, HudEntry>()
  /** Each entry as its producer last posted it, every row it holds: what putting a row
   *  away, an update and `away` start from, so all of it comes back. */
  const givenByKey = new Map<string, HudEntry>()
  const waiting = new Map<string, { entry: HudEntry; at: number }>()
  const hiddenStates = new Map<string, string>()
  const lives = new Map<string, Life>()
  const sources = new Map<string, Set<string>>()
  const hands = new Set<string>()
  const seers = new Map<string, readonly string[] | undefined>()
  const listeners = new Set<() => void>()
  let clock: unknown = null
  let unfreeze: unknown = null
  let readoutClear: unknown = null
  let serial = 0
  let quiet = 0
  let owed = false
  let acting = 0

  const notify = () => {
    if (quiet > 0) {
      owed = true
      return
    }
    for (const l of listeners) l()
  }
  const set = (next: Partial<HudState>) => {
    state = { ...state, ...next }
    notify()
  }
  /** Changes made inside `run` notify once, at its end. */
  const batch = (run: () => void) => {
    quiet += 1
    try {
      run()
    } finally {
      quiet -= 1
      if (quiet === 0 && owed) {
        owed = false
        notify()
      }
    }
  }

  const announce = (entry: HudEntry) => {
    serial += 1
    const words = [entry.title, entry.detail].filter(Boolean).join(", ")
    set({
      said: [
        ...state.said,
        { id: serial, words, urgent: entry.kind === "problem" },
      ].slice(-SAID_KEPT),
    })
  }

  const withoutDismissedRows = (entry: HudEntry): HudEntry => {
    const gone = keptNow().rows.get(entry.key)
    if (!gone?.size || !entry.rows) return entry
    return { ...entry, rows: entry.rows.filter((r) => !gone.has(r.id)) }
  }

  /** A problem dismissed with its rows (for the session, or across reloads): rowless,
   *  until its source stops saying it; with rows, until a row it did not hold appears. */
  const isDismissed = (entry: HudEntry) => {
    const ids = (entry.rows ?? []).map((r) => r.id)
    const session = sessionDismissed.get(entry.key)
    if (session) {
      if (ids.every((id) => session.has(id))) return true
      sessionDismissed.delete(entry.key)
      return false
    }
    const rows = keptNow().entries.get(entry.key)
    if (!rows) return false
    if (ids.every((id) => rows.has(id))) return true
    keptNow().entries.delete(entry.key)
    remember()
    return false
  }

  /** What the person put away and a source saying it again does not undo. */
  const putAway = (entry: HudEntry) =>
    entry.kind === "problem"
      ? isDismissed(entry)
      : entry.kind === "state" &&
        hiddenStates.get(entry.key) ===
          JSON.stringify([entry.title, entry.detail])

  /** An entry changed while a hand is in the column: its rows keep their order and
   *  their room (a row that left stays, `leftRows`), new ones join at the end. */
  const frozenRows = (entry: HudEntry, standing: HudEntry): HudEntry => {
    // Rebuilt from what stands: a row that left stays inert, one said again is a row
    // again.
    const left = new Set<string>()
    const rows = holdRows(entry.rows, standing.rows, left, true)
    const leftRows = new Map(state.leftRows)
    if (left.size > 0) leftRows.set(entry.key, left)
    else leftRows.delete(entry.key)
    set({ leftRows })
    return {
      ...entry,
      rows,
      action: holdControl(entry.action, standing.action),
      cancel: holdControl(entry.cancel, standing.cancel),
    }
  }

  /** The one rule a standing entry changes by, a post's, an end's and a press's alike:
   *  what it says shows at once; under the hand its rows keep their places, a row that
   *  left inert in its place, applied whole once the hand leaves; else it applies whole
   *  now, nothing left over. */
  const changed = (entry: HudEntry, standing: HudEntry): HudEntry => {
    if (state.frozen) return frozenRows(entry, standing)
    if (state.leftRows.has(entry.key)) {
      const leftRows = new Map(state.leftRows)
      leftRows.delete(entry.key)
      set({ leftRows })
    }
    return entry
  }

  const show = (entry: HudEntry) => {
    if (state.frozen && acting === 0) {
      set({ held: [...state.held, entry] })
      // A problem is said at once, even while the column holds still, and only then.
      if (entry.kind === "problem") {
        announce(entry)
        announcedHeld.add(entry.key)
      }
      return
    }
    latestByKey.set(entry.key, entry)
    lives.set(entry.key, {
      shown: now(),
      seenLeft: entry.kind === "toast" ? TOAST_SEEN : undefined,
    })
    set({ entries: [...state.entries, entry] })
    if (!announcedHeld.delete(entry.key)) announce(entry)
    run()
  }

  const leave = (key: string) => {
    const entry = state.entries.find((e) => e.key === key)
    if (!entry) return
    if (state.frozen) {
      if (!state.ended.has(key)) set({ ended: new Set([...state.ended, key]) })
      return
    }
    lives.delete(key)
    latestByKey.delete(key)
    serial += 1
    const leftRows = new Map(state.leftRows)
    leftRows.delete(key)
    set({
      entries: state.entries.filter((e) => e.key !== key),
      earlier: [{ id: serial, entry }, ...state.earlier].slice(0, EARLIER_KEPT),
      leftRows,
    })
  }

  /** Whether a source holds `key` (`reconcile`): its entry is said again, so what it
   *  gave is kept; an event's never is. */
  const sourced = (key: string) =>
    [...sources.values()].some((keys) => keys.has(key))

  /** `entry` kept as its producer gave it, every row it holds, for a source's key alone;
   *  written by every change of an entry, so it never goes stale. */
  const keepGiven = (entry: HudEntry) => {
    if (sourced(entry.key)) givenByKey.set(entry.key, entry)
  }

  /** `entry` ended as `kind`, from what its producer gave, kept as given and shown less
   *  the rows put away. */
  const ending = (
    entry: HudEntry,
    kind: "toast" | "problem",
    title: string,
    detail?: string,
  ): HudEntry => {
    const ended: HudEntry = {
      ...(givenByKey.get(entry.key) ?? entry),
      kind,
      title,
      detail,
      progress: undefined,
      tone: undefined,
      cancel: undefined,
    }
    keepGiven(ended)
    return withoutDismissedRows(ended)
  }

  const ticking = () =>
    !state.frozen &&
    seers.size > 0 &&
    (waiting.size > 0 ||
      [...lives.values()].some(
        (l) => l.seenLeft !== undefined || l.leaves !== undefined,
      ))

  const step = () => {
    const t = now()
    batch(() => {
      for (const [key, w] of waiting) {
        if (w.at <= t) {
          waiting.delete(key)
          show(w.entry)
        }
      }
      const seenScopes = new Set<string>()
      let everyScope = false
      for (const scopes of seers.values()) {
        if (!scopes) everyScope = true
        else for (const s of scopes) seenScopes.add(s)
      }
      for (const [key, life] of [...lives]) {
        const entry = latestByKey.get(key)
        if (life.leaves !== undefined && life.leaves <= t) leave(key)
        else if (
          life.seenLeft !== undefined &&
          entry &&
          (everyScope || seenScopes.has(entry.scope ?? "app"))
        ) {
          const less = life.seenLeft - TICK
          if (less <= 0) leave(key)
          else life.seenLeft = less
        }
      }
    })
    if (!ticking() && clock) {
      timers.clearInterval(clock)
      clock = null
    }
  }

  // One clock for every timed law, running only while it has something to do.
  const run = () => {
    if (!clock && ticking()) clock = timers.setInterval(step, TICK)
  }

  /** `key` kept as put away (`entry`), or no longer (`undefined`): written only when it
   *  changes. */
  const putBack = (key: string, entry: HudEntry | undefined) => {
    const was = state.away.get(key)
    if (entry ? was !== undefined && same(was, entry) : was === undefined)
      return
    const away = new Map(state.away)
    if (entry) away.set(key, entry)
    else away.delete(key)
    set({ away })
  }

  /** What its source stopped saying: a dismissal of it as a whole is forgotten; its
   *  rows' dismissals stay, forgotten only by its source (`forget`), so an entry that
   *  left when its last row was put away brings none back when it is said again. */
  const gone = (key: string) => {
    putBack(key, undefined)
    givenByKey.delete(key)
    hiddenStates.delete(key)
    sessionDismissed.delete(key)
    if (keptNow().entries.delete(key)) remember()
    if (waiting.delete(key)) return
    // Said under the hand and gone before it showed: its next showing is said.
    announcedHeld.delete(key)
    if (state.held.some((e) => e.key === key))
      set({ held: state.held.filter((e) => e.key !== key) })
    const entry = latestByKey.get(key)
    if (!entry) return
    if (entry.kind === "progress") hud.end(key, "quietly")
    else if (entry.kind === "state") {
      const life = lives.get(key)
      if (life) life.leaves = now() + SETTLE
      run()
    } else if (entry.kind === "problem") leave(key)
  }

  const settle = () => {
    batch(() => {
      for (const key of state.ended) {
        // Out of `ended` first, so `leave` takes it now.
        set({ ended: new Set([...state.ended].filter((k) => k !== key)) })
        leave(key)
      }
      const entries = state.entries.map((e) => latestByKey.get(e.key) ?? e)
      const arrived = state.held
      set({
        ended: new Set(),
        leftRows: new Map(),
        chrome: new Map(),
        held: [],
        entries,
      })
      for (const entry of arrived) show(entry)
    })
    run()
  }

  /** Whether a press reached `verb` on purpose. A destructive one takes a key or an
   *  assistive action, a single click on a verb that had stood still `HUD_DWELL`, and a
   *  tap on one that had stood still that long before the finger landed; a double
   *  click's second press, or a press on a verb that just appeared or moved there, is
   *  refused, felt and said in its input's words. */
  const reached = (verb: HudAction, reach: HudReach) => {
    if (!verb.destructive || reach === "key") return true
    const words =
      "touch" in reach
        ? reach.touch.still < HUD_DWELL && "hold still on it, then press"
        : reach.pointer.clicks >= 2
          ? "click it once"
          : reach.pointer.still < HUD_DWELL &&
            "rest the pointer on it, then press"
    if (!words) return true
    hud.read(words, { refusal: true })
    return false
  }

  /** Runs a press made in the column: what it posts shows at once. */
  const pressed = (press: () => void) => {
    acting += 1
    try {
      press()
    } finally {
      acting -= 1
    }
  }

  const hud: Hud = {
    state: () => state,
    subscribe(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    post(given) {
      if (
        typeof given.progress === "number" &&
        !Number.isFinite(given.progress)
      )
        throw new Error(
          `hud.post: ${given.key} says a share of ${given.progress}`,
        )
      heldUnseenNeverArmed(given)
      keepGiven(given)
      const entry = withoutDismissedRows(given)
      const i = state.entries.findIndex((e) => e.key === entry.key)
      if (i >= 0) {
        const standing = state.entries[i] as HudEntry
        const life = lives.get(entry.key)
        if (life) life.leaves = undefined
        latestByKey.set(entry.key, entry)
        const shown = changed(entry, standing)
        // Said again, it stands again: out of `ended`, whatever the hand; unless the
        // person put it away (a problem dismissed, a state hidden), which a source saying
        // it again never undoes.
        const ended = state.ended.has(entry.key) && !putAway(entry)
        if (same(shown, standing) && !ended) return
        if (standing.kind !== entry.kind) announce(entry)
        const entries = [...state.entries]
        entries[i] = shown
        set(
          ended
            ? {
                entries,
                ended: new Set([...state.ended].filter((k) => k !== entry.key)),
              }
            : { entries },
        )
        return
      }
      const h = state.held.findIndex((e) => e.key === entry.key)
      if (h >= 0) {
        const held = [...state.held]
        held[h] = entry
        set({ held })
        return
      }
      const w = waiting.get(entry.key)
      if (w) {
        w.entry = entry
        return
      }
      // Put away (it, or every row it holds): kept where the person can bring it back,
      // never the only door to a fact that stands.
      if (putAway(entry) || (given.rows?.length && !entry.rows?.length)) {
        putBack(given.key, given)
        return
      }
      putBack(entry.key, undefined)
      if (entry.kind === "state") hiddenStates.delete(entry.key)
      if (entry.kind === "progress" && (entry.reveal ?? 300) > 0) {
        waiting.set(entry.key, { entry, at: now() + (entry.reveal ?? 300) })
        run()
      } else if (entry.kind === "state") {
        waiting.set(entry.key, { entry, at: now() + SETTLE })
        run()
      } else show(entry)
    },
    update(key, change) {
      const shown = latestByKey.get(key)
      // Changed from what its producer said, every row it holds.
      if (shown) return hud.post(change(givenByKey.get(key) ?? shown))
      const w = waiting.get(key)
      if (w) {
        const changed = change(givenByKey.get(key) ?? w.entry)
        heldUnseenNeverArmed(changed)
        keepGiven(changed)
        w.entry = withoutDismissedRows(changed)
        return
      }
      const h = state.held.findIndex((e) => e.key === key)
      if (h >= 0) {
        const held = [...state.held]
        const changed = change(givenByKey.get(key) ?? (held[h] as HudEntry))
        heldUnseenNeverArmed(changed)
        keepGiven(changed)
        held[h] = withoutDismissedRows(changed)
        set({ held })
      }
    },
    end(key, outcome) {
      const w = waiting.get(key)
      if (w) {
        waiting.delete(key)
        if (outcome !== "quietly")
          show(
            "toast" in outcome
              ? ending(w.entry, "toast", outcome.toast, outcome.detail)
              : ending(w.entry, "problem", outcome.problem, outcome.detail),
          )
        return
      }
      const h = state.held.findIndex((e) => e.key === key)
      if (h >= 0) {
        const held = [...state.held]
        const was = held[h] as HudEntry
        if (outcome === "quietly") {
          held.splice(h, 1)
          announcedHeld.delete(key)
        } else if ("toast" in outcome) {
          // Not what was said under the hand: said when it shows.
          held[h] = ending(was, "toast", outcome.toast, outcome.detail)
          announcedHeld.delete(key)
        } else {
          const problem = ending(
            was,
            "problem",
            outcome.problem,
            outcome.detail,
          )
          held[h] = problem
          announce(problem)
          announcedHeld.add(key)
        }
        set({ held })
        return
      }
      const entry = latestByKey.get(key)
      if (!entry) return
      if (outcome === "quietly") {
        const leaves = (lives.get(key)?.shown ?? now()) + WORK_AT_LEAST
        if (leaves <= now()) leave(key)
        else {
          const life = lives.get(key)
          if (life) life.leaves = leaves
          run()
        }
        return
      }
      const ended =
        "toast" in outcome
          ? ending(entry, "toast", outcome.toast, outcome.detail)
          : ending(entry, "problem", outcome.problem, outcome.detail)
      lives.set(key, {
        shown: now(),
        seenLeft: "toast" in outcome ? TOAST_SEEN : undefined,
      })
      latestByKey.set(key, ended)
      // The one rule a standing entry changes by (`changed`).
      const entries = state.entries.map((e) =>
        e.key === key ? changed(ended, e) : e,
      )
      set({ entries })
      announce(ended)
      run()
    },
    reconcile(source, said, remembered = true) {
      const keys = new Set(said.map((e) => e.key))
      if (keys.size !== said.length)
        throw new Error(`hud.reconcile: ${source} says a key twice`)
      if (!remembered) sessionSources.add(source)
      batch(() => {
        for (const key of sources.get(source) ?? [])
          if (!keys.has(key)) gone(key)
        sources.set(source, keys)
        for (const entry of said) hud.post(entry)
      })
    },
    forget(rows, key) {
      // Read once: an iterable may be a generator, spent by its first walk.
      const ids = [...rows]
      const dismissed = keptNow().rows.get(key)
      if (!dismissed || !ids.some((id) => dismissed.has(id))) return
      for (const id of ids) dismissed.delete(id)
      if (dismissed.size === 0) keptNow().rows.delete(key)
      remember()
      notify()
    },
    toast(title, o = {}) {
      serial += 1
      hud.post({
        key: `toast ${serial}`,
        kind: "toast",
        title,
        detail: o.detail,
        action: o.action,
      })
    },
    promise(promise, said) {
      serial += 1
      const key = `promise ${serial}`
      hud.post({ key, kind: "progress", title: said.progress, reveal: 0 })
      promise.then(
        (v) =>
          hud.end(key, {
            toast:
              typeof said.toast === "function" ? said.toast(v) : said.toast,
          }),
        (e) =>
          hud.end(key, {
            problem:
              typeof said.problem === "function"
                ? said.problem(e)
                : said.problem,
          }),
      )
      return promise
    },
    read(words, o = {}) {
      const refusal = o.refusal ?? false
      const scope = o.scope ?? "app"
      const reading = readingTime(words)
      if (refusal && reading > READOUT_MOST) {
        hud.post({ key: "refused", kind: "problem", title: words, scope })
        return
      }
      if (
        refusal ||
        state.readout?.words !== words ||
        state.readout?.scope !== scope
      ) {
        serial += 1
        // A gesture is the readings shown without a rest between, in one place; a
        // refusal is a gesture of its own.
        const shown = state.readout
        const gesture =
          shown && !refusal && !shown.refusal && shown.scope === scope
            ? shown.gesture
            : serial
        set({ readout: { words, refusal, scope, serial, gesture } })
      }
      if (refusal) {
        serial += 1
        set({
          said: [...state.said, { id: serial, words, urgent: true }].slice(
            -SAID_KEPT,
          ),
        })
      }
      if (readoutClear) timers.clearTimeout(readoutClear)
      readoutClear = null
      if (o.holding) return
      readoutClear = timers.setTimeout(() => {
        readoutClear = null
        set({ readout: null })
      }, reading)
    },
    clearReadout() {
      if (readoutClear) timers.clearTimeout(readoutClear)
      readoutClear = null
      if (state.readout) set({ readout: null })
    },
    latest: (key) => latestByKey.get(key),
    given: (key) => givenByKey.get(key),
    act(key, which, reach) {
      const entry = latestByKey.get(key)
      if (!entry || state.ended.has(key)) return
      const verb = which === "action" ? entry.action : entry.cancel
      if (!verb || verb.armed === false || verb.shown === false) return
      if (!reached(verb, reach)) return
      pressed(() => {
        verb.onClick()
        if (entry.kind === "toast") hud.dismiss(key)
      })
    },
    actRow(key, row, verb, reach) {
      const entry = latestByKey.get(key)
      if (!entry || state.ended.has(key)) return
      if (state.leftRows.get(key)?.has(row)) return
      const found = findRow(entry.rows, row)
      const press =
        verb === "open"
          ? found?.open
          : found?.verbs?.find((v) => v.label === verb)
      if (press && press.armed !== false && press.shown !== false)
        if (reached(press, reach)) pressed(press.onClick)
    },
    dismiss(key) {
      const entry = latestByKey.get(key)
      if (!entry) return
      const source = [...sources].find(([, keys]) => keys.has(key))?.[0]
      const reconciled = source !== undefined
      if (reconciled && entry.kind === "problem") {
        const rows = new Set((entry.rows ?? []).map((r) => r.id))
        if (sessionSources.has(source)) sessionDismissed.set(key, rows)
        else {
          keptNow().entries.set(key, rows)
          remember()
        }
      }
      if (reconciled && entry.kind === "state")
        hiddenStates.set(key, JSON.stringify([entry.title, entry.detail]))
      if (reconciled && (entry.kind === "problem" || entry.kind === "state"))
        putBack(key, givenByKey.get(key) ?? entry)
      leave(key)
    },
    bringBack(key) {
      const entry = state.away.get(key)
      if (!entry) return
      putBack(key, undefined)
      sessionDismissed.delete(key)
      hiddenStates.delete(key)
      const k = keptNow()
      k.entries.delete(key)
      const kept = k.rows.get(key)
      if (kept && !withoutDismissedRows(entry).rows?.length) {
        // Only the rows it holds come back: another row kept stays kept.
        for (const r of entry.rows ?? []) kept.delete(r.id)
        if (kept.size === 0) k.rows.delete(key)
      }
      remember()
      // A press in the panel: what it shows, shows at once.
      pressed(() => hud.post(entry))
    },
    dismissRow(row, key) {
      const k = keptNow()
      const rows = k.rows.get(key) ?? new Set<string>()
      rows.add(row)
      k.rows.set(key, rows)
      remember()
      const entry = latestByKey.get(key)
      if (!entry) {
        notify()
        return
      }
      // Said again as its producer said it: the post leaves the rows put away out, and
      // what was given stays whole.
      const whole = givenByKey.get(key) ?? entry
      if (!withoutDismissedRows(whole).rows?.length) {
        // Kept where it can come back only while a source says it.
        if ([...sources.values()].some((keys) => keys.has(key)))
          putBack(key, whole)
        leave(key)
      } else hud.post(whole)
    },
    dismissedRows: (key) => new Set(keptNow().rows.get(key) ?? []),
    hand(host, inside) {
      if (inside) hands.add(host)
      else hands.delete(host)
      if (hands.size > 0) {
        if (unfreeze) timers.clearTimeout(unfreeze)
        unfreeze = null
        if (!state.frozen)
          set({
            frozen: true,
            chrome: new Map(
              state.entries.map((e) => [
                e.key,
                {
                  rows: (e.rows?.length ?? 0) > 0,
                  dismiss: dismissible(e),
                },
              ]),
            ),
          })
        return
      }
      if (!state.frozen || unfreeze) return
      unfreeze = timers.setTimeout(() => {
        unfreeze = null
        if (hands.size > 0) return
        set({ frozen: false })
        settle()
      }, HAND_GRACE)
    },
    seen(host, seen, scopes) {
      if (seen) seers.set(host, scopes)
      else seers.delete(host)
      const all = [...seers.values()]
      const next: ReadonlySet<string> | "all" = all.some((s) => !s)
        ? "all"
        : new Set(all.flatMap((s) => s ?? []))
      const was = state.seenScopes
      const same =
        next === "all" || was === "all"
          ? next === was
          : next.size === was.size && [...next].every((s) => was.has(s))
      if (!same) set({ seenScopes: next })
      run()
    },
  }
  return hud
}
