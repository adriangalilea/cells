// HISTORY: what can be undone, as named steps. The kernel under `ui/history` (which
// puts ⌘Z on the page and says what each undo did), with no React and no DOM, so any
// state can keep one: a table's sheet, a board's cards, a document.
//
// A STEP is a change already made, said by its name and how to take it back and make
// it again: `{ label: "delete 3 rows", undo, redo }`. The consumer makes the change,
// then records it; the history never makes one itself, it only replays them. A step's
// two functions are exact inverses the consumer owns (two states, or a move and its
// reverse), so redo is the change itself, never a guess at it.
//
// One gesture is one step. What a person did at once (a paste over a block, a fill, a
// drag that moved a card and opened a gap) is recorded once, its label the gesture's.
// Typing into one place coalesces: a step with the same `merges` key as the last one,
// recorded within `COALESCE_MS` of it, joins it (the first's undo, the newest's redo),
// so ⌘Z takes back the word typed, not its last letter.
//
// Recording a step clears what was undone: the future after a new change is a
// different future. The steps kept are at most `limit`, the oldest dropped first.
//
// Decided, built on this same API when a consumer needs it (TODO.md, "undo"): the
// branch a change after an undo leaves kept as a tree (⌘Z stays linear), steps as
// patches for an undo that survives a reload or is shared, selective undo (only one's
// own changes) for a multiplayer document, and a delayed commit for an effect with no
// inverse (a message sent), which is not undo at all.

export interface HistoryStep {
  /** What the change was, as a verb and its object ("move the card to done"):
   *  said when it is undone or redone. */
  label: string
  undo: () => void
  redo: () => void
  /** Where the change is, brought into view after it is undone or redone: its cells
   *  selected and scrolled to, its card focused, the caret where it was. A change
   *  taken back out of sight is how a person stops trusting undo. */
  reveal?: () => void
}

export interface RecordOptions {
  /** Steps of one kind on one target ("type:B3") that join the last one when it has
   *  the same key and is recent (`COALESCE_MS`). */
  merges?: string
}

export interface HistoryState {
  /** The steps that can be undone, oldest first. */
  done: readonly HistoryStep[]
  /** The steps that can be redone, the next one last. */
  undone: readonly HistoryStep[]
}

export interface History {
  /** The steps now, for a consumer drawing them (a button's label, a list). */
  state(): HistoryState
  record(step: HistoryStep, options?: RecordOptions): void
  /** Takes back the newest step and returns it; nothing to undo returns null. */
  undo(): HistoryStep | null
  /** Makes again the step undone last and returns it; nothing to redo returns null. */
  redo(): HistoryStep | null
  /** Goes to the point where `count` steps are done, undoing or redoing every step
   *  between (a history list's click), and returns how many it moved. */
  goTo(count: number): number
  /** Forgets every step (a document closed, a new one loaded). */
  clear(): void
  /** Called after every change to the steps; returns the way off. */
  subscribe(listener: () => void): () => void
}

/** How recent a step must be to join the last one of its kind, ms. */
export const COALESCE_MS = 1000
/** How many steps a history keeps unless told otherwise. */
export const LIMIT = 200

export function createHistory({
  limit = LIMIT,
  now = () => Date.now(),
}: {
  limit?: number
  now?: () => number
} = {}): History {
  if (!(limit >= 1))
    throw new Error(`history: a limit of ${limit} keeps nothing`)
  let state: HistoryState = { done: [], undone: [] }
  // The last step's merge key and when it was recorded, for coalescing.
  let last: { merges: string; at: number } | null = null
  const listeners = new Set<() => void>()
  const set = (next: HistoryState) => {
    state = next
    for (const l of listeners) l()
  }
  const history: History = {
    state: () => state,
    record(step, { merges } = {}) {
      const at = now()
      const prior = state.done.at(-1)
      const joins =
        merges !== undefined &&
        prior !== undefined &&
        last !== null &&
        last.merges === merges &&
        at - last.at <= COALESCE_MS
      last = merges === undefined ? null : { merges, at }
      if (joins) {
        const joined: HistoryStep = {
          label: step.label,
          undo: prior.undo,
          redo: step.redo,
          reveal: step.reveal,
        }
        set({ done: [...state.done.slice(0, -1), joined], undone: [] })
        return
      }
      set({ done: [...state.done, step].slice(-limit), undone: [] })
    },
    undo() {
      const step = state.done.at(-1)
      if (!step) return null
      last = null
      step.undo()
      step.reveal?.()
      set({ done: state.done.slice(0, -1), undone: [...state.undone, step] })
      return step
    },
    redo() {
      const step = state.undone.at(-1)
      if (!step) return null
      last = null
      step.redo()
      step.reveal?.()
      set({ done: [...state.done, step], undone: state.undone.slice(0, -1) })
      return step
    },
    goTo(count) {
      const total = state.done.length + state.undone.length
      if (!Number.isInteger(count) || count < 0 || count > total)
        throw new Error(`history: no point ${count} among ${total} steps`)
      let moved = 0
      while (state.done.length > count && history.undo()) moved++
      while (state.done.length < count && history.redo()) moved++
      return moved
    },
    clear() {
      last = null
      set({ done: [], undone: [] })
    },
    subscribe(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
  return history
}
