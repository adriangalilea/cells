// AN EDIT, scripted: what a developer does to a project's files told as steps (open a
// file, type at a place in it, replace a range, save, ask the language server: a
// hover, a completion, a signature, go to a definition, rename a symbol across
// files, the problems), timed into a playable any driver moves through, and folded into
// the frame an editor draws at an instant: the tabs open, the file in front and what
// it holds then, where the caret is, a selection, what is unsaved, the modal editor's
// mode and command line, the diagnostics, the popup open, the lines flashed. `ide`
// draws it as VS Code, `terminal-editor` as a modal editor in the terminal; a Player,
// a scroll stage or a fixed progress moves time. Framework-free.
//
// Every character ever in a file has its place in the file's one list, with when it
// came (`born`) and when it went (`died`): the text at an instant is the characters
// standing then. The text is highlighted once per STATE it passes through (a file as
// each edit leaves it, `states`), and each character in a frame points at its offset
// in the state current then (`origin`, `state`), so a half-typed attribute already
// wears its colour, a removal never leaves a stale one, and a scrubbed edit is the
// played one.
//
// Diagnostics are COMPUTED, never scripted: `diagnose(path, workspace)` runs on each
// file with every file's text at the instant (memoised per workspace), so what the
// editor underlines is what the tool says of the project as it stands, one file's
// problem caused by another included; `diagnoseWords` is the engine for a demo
// without one. Hovers, completions, signatures, a goto and a rename are steps.
//
// The pacing is one table (TYPE_MS, OPEN_MS, ...): a step's time is decided here and
// nowhere else, so a played edit and a scrubbed one are the same frames.

import type { Chapter, Playable } from "@/lib/playable"

/** A file of the project: its path (`/` between folders), and what it holds before
 *  the session. */
export interface CodeFile {
  path: string
  text: string
}

/** A completion item: what the list shows, its kind (the icon), a detail beside
 *  it, and what it inserts (the label when absent). */
export interface CodeCompletion {
  label: string
  kind?: CodeCompletionKind
  detail?: string
  insert?: string
}

export type CodeCompletionKind =
  | "function"
  | "method"
  | "variable"
  | "constant"
  | "property"
  | "field"
  | "class"
  | "interface"
  | "module"
  | "keyword"
  | "snippet"
  | "text"

/** One thing done to the project, after `delay` ms (default none). A place is a
 *  `line` (from 1) and a `column` (from 1, absent the line's end), in the text as it
 *  is when the step starts. */
export type CodeStep = (
  | /** The file comes to the front, in a tab of its own (opened once, then
   *  switched to). */
  { kind: "open"; path: string }
  /** Keys typed into the file in front, at a place or, with neither, at the caret
   *  (a file's start before any). A newline's indentation lands with it. */
  | { kind: "type"; text: string; line?: number; column?: number }
  /** `length` characters from a place selected, then typed over with `text` (the
   *  first key takes the selection); an empty `text` deletes them. */
  | {
      kind: "replace"
      line: number
      column: number
      length: number
      text: string
    }
  /** The file in front saved: VS Code's ⌘S, a modal editor's `:w`. `formatted`, the
   *  text a language server's formatting on save leaves it as: what it changes lands
   *  as the write does, every other character stays. */
  | { kind: "save"; formatted?: string }
  /** What the language server says of a place: `text` (its type, its docs),
   *  with the diagnostics there. */
  | { kind: "hover"; line: number; column: number; text: string }
  /** The completion list at the caret (or a place), the selection moving down to
   *  `pick` (an index of `items`), which lands as a replace of the word before the
   *  caret. */
  | {
      kind: "complete"
      items: CodeCompletion[]
      pick: number
      line?: number
      column?: number
    }
  /** A call's signature at the caret (or a place), `active` the parameter being
   *  written (its text in `label`). */
  | {
      kind: "signature"
      label: string
      active?: string
      line?: number
      column?: number
    }
  /** Go to a definition: the file at `path` to the front, its caret at a place
   *  there (in that file's text), the line flashed. */
  | { kind: "goto"; path: string; line: number; column?: number }
  /** The symbol at a place renamed `to` in every file (or those of `files`) that
   *  names it as a whole word: the new name typed into the rename prompt, then
   *  each file changed shown in turn, the front one last. */
  | {
      kind: "rename"
      line: number
      column: number
      to: string
      files?: string[]
    }
  /** The problems shown: VS Code's Problems panel, helix's diagnostics picker. */
  | { kind: "problems" }
) & { delay?: number }

/** One key typed into a file: a person who knows what they are writing. */
export const TYPE_MS = 55
/** A file coming to the front, before the next step. */
const OPEN_MS = 450
/** The caret placed (a click, a jump to the line) before the first key. */
const PLACE_MS = 300
/** A range shown selected before the first key takes it. */
const SELECT_MS = 400
/** A save: the modal editor's `:` and `w` and return, a key each. */
const SAVE_KEY_MS = 140
const SAVE_MS = SAVE_KEY_MS * 3 + 300
/** Insert mode left (Esc) after the last key. */
const ESC_MS = 220
/** A hover or a signature held open, read. */
const POPUP_MS = 2000
/** The completion list opening, each step down it, and the pick held after. */
const LIST_MS = 450
const LIST_STEP_MS = 220
const PICKED_MS = 700
/** A goto's press, and the line it lands on flashed. */
const GOTO_MS = 350
const FLASH_MS = 1000
/** The rename prompt opening, Enter, and each file changed shown. */
const PROMPT_MS = 350
const ENTER_MS = 250
const SHOW_MS = 1100
/** The problems held open. */
const PROBLEMS_MS = 2400
/** The last frame held on screen after the last step, so it is seen. */
const HOLD_MS = 1400

/** A modal editor's mode: `insert` while keys are typed, `select` while a range is
 *  selected, `command` while a command line is, else `normal`. VS Code has none,
 *  and ignores it. */
export type CodeMode = "normal" | "insert" | "select" | "command"

export type CodeSeverity = "error" | "warning" | "info" | "hint"

/** What a tool says of a range of a text: offsets into it, `to` exclusive. */
export interface CodeDiagnostic {
  from: number
  to: number
  severity: CodeSeverity
  message: string
  /** Who says it (`ts`, `eslint`, `cells`). */
  source?: string
}

/** Every file of the project at an instant: its text as it stands then, saved or
 *  not (what a language server reads), by path. */
export type CodeWorkspace = Readonly<Record<string, string>>

/** A language server's diagnostics of the file at `path`, computed from the whole
 *  workspace (its own text is `workspace[path]`), so a problem in one file that
 *  another causes (an import, a reference to another file's key) shows, and goes
 *  when that other file is fixed. Offsets into `workspace[path]`. */
export type CodeDiagnose = (
  path: string,
  workspace: CodeWorkspace,
) => readonly CodeDiagnostic[]

/** A popup open over the code, anchored at an offset of the front file's text. */
export type CodePopup =
  | {
      kind: "hover"
      at: number
      text: string
      diagnostics: CodeDiagnostic[]
    }
  | {
      kind: "completion"
      at: number
      items: CodeCompletion[]
      selected: number
    }
  | {
      kind: "signature"
      at: number
      label: string
      active: { from: number; to: number } | null
    }

/** The editor at an instant. */
export interface CodeFrame {
  /** The files open, in the order their tabs were opened. */
  open: string[]
  /** The file in front; null before one opens. */
  active: string | null
  /** The front file's text at this instant. */
  text: string
  /** The state the front file's text is coloured by (an index into the timeline's
   *  `states`), and each character's offset in it. */
  state: number
  origin: number[]
  /** The caret, an offset into `text`. */
  caret: number
  /** A range selected, offsets into `text`. */
  selection: { from: number; to: number } | null
  /** The files changed since they were last saved. */
  dirty: string[]
  mode: CodeMode
  /** The command line while one is typed (`:w`), else empty. */
  command: string
  /** A prompt being answered (helix's `rename-to:`, VS Code's rename box), at an
   *  offset of `text`. */
  prompt: { label: string; text: string; at: number } | null
  /** The last save, done: the file and the instant (ms) it landed. */
  saved: { path: string; at: number } | null
  /** The front file's diagnostics, offsets into `text`. */
  diagnostics: CodeDiagnostic[]
  /** Every file's, those with any. */
  problems: { path: string; diagnostics: CodeDiagnostic[] }[]
  popup: CodePopup | null
  /** Ranges of `text` flashed: a goto's line, a rename's new names. */
  flash: { from: number; to: number }[]
  /** A panel open: the problems. */
  panel: "problems" | null
  /** The step under way or last done, -1 before the first. */
  step: number
}

/** A character of a file: when it came (ms, -1 when it was there before) and when
 *  it went (Infinity while it stands). */
export interface CodeChar {
  ch: string
  born: number
  died: number
}
type Char = CodeChar

/** A text a file passes through, current from `from` (ms): what is coloured. */
export interface CodeState {
  path: string
  text: string
  from: number
}

/** What a step does, placed and timed. */
type Plan =
  | { kind: "open"; path: string }
  | { kind: "type"; path: string; chars: Char[] }
  | {
      kind: "replace"
      path: string
      removed: Char[]
      chars: Char[]
      cut: number
      /** Where the caret stood before the first key. */
      before: Char | null
    }
  | { kind: "save"; path: string }
  | { kind: "hover"; path: string; caret: Char | null; text: string }
  | {
      kind: "complete"
      path: string
      /** Where the caret stands while the list is open, and where the word it
       *  replaces starts. */
      place: Char | null
      anchor: Char | null
      items: CodeCompletion[]
      pick: number
      open: number
      accept: number
      chars: Char[]
    }
  | {
      kind: "signature"
      path: string
      /** The caret moved there first, or left where it is. */
      place: Char | null | undefined
      anchor: Char | null
      label: string
      active: { from: number; to: number } | null
    }
  | {
      kind: "goto"
      path: string
      caret: Char | null
      jump: number
      line: Char[]
    }
  | {
      kind: "rename"
      path: string
      anchor: Char | null
      to: string
      typing: number
      accept: number
      /** Each file changed, the new names' characters, shown from `from`. */
      tour: { path: string; from: number; names: Char[][] }[]
    }
  | { kind: "problems" }

export interface CodeTimeline {
  files: CodeFile[]
  steps: CodeStep[]
  /** When each step begins (after its delay) and ends, ms. */
  starts: number[]
  ends: number[]
  total: number
  /** Each file as the session leaves it, by path. */
  final: Record<string, string>
  /** The edit as a playable: one chapter per file opened and per thing done. */
  playable: Playable
  /** Each file's characters, every one it ever held, in place. */
  chars: Record<string, CodeChar[]>
  /** Every text a file passes through, each highlighted once. */
  states: CodeState[]
  /** Each state's characters at their offsets in it. */
  order: Map<Char, number>[]
  plans: Plan[]
  /** Every file's diagnostics in a workspace, memoised per workspace (its texts in
   *  `files`' order); null without an engine. */
  diagnose: ((workspace: CodeWorkspace) => CodeProblems) | null
}

/** Every file's diagnostics, those with any, in `files`' order. */
export type CodeProblems = { path: string; diagnostics: CodeDiagnostic[] }[]

export interface CodeTimelineOptions {
  /** The clock's length when longer than the edit (a clock shared with a browser
   *  that loads the result after it), ms: the last frame holds to it. */
  duration?: number
  /** The language server's diagnostics, run on each file with the workspace at the
   *  instant. */
  diagnose?: CodeDiagnose
}

/** Every file's diagnostics in `workspace`, by `diagnose`, each checked to lie in its
 *  file's text. THROWS on one that does not. */
export function problemsOf(
  paths: readonly string[],
  workspace: CodeWorkspace,
  diagnose: CodeDiagnose,
): CodeProblems {
  return paths.flatMap((path) => {
    const text = workspace[path]
    if (text === undefined)
      throw new Error(`code-session: ${path} is not in the workspace`)
    const d = diagnose(path, workspace)
    for (const x of d)
      if (!(x.from >= 0 && x.from <= x.to && x.to <= text.length))
        throw new Error(
          `code-session: a diagnostic of ${path} at ${x.from}..${x.to}, past its ${text.length} characters`,
        )
    return d.length ? [{ path, diagnostics: [...d] }] : []
  })
}

const stands = (c: Char, ms: number) => c.born <= ms && ms < c.died
const standing = (doc: readonly Char[]) =>
  doc.filter((c) => c.died === Number.POSITIVE_INFINITY)
const textOf = (chars: readonly Char[]) => chars.map((c) => c.ch).join("")
const WORD = /[\w$]/

/** Offset of `line`:`column` (both from 1; no column: the line's end) in `chars`.
 *  Throws on a place the text does not have. */
function offsetOf(
  chars: readonly Char[],
  line: number,
  column: number | undefined,
  path: string,
): number {
  let at = 0
  for (let l = 1; l < line; l++) {
    while (at < chars.length && chars[at]?.ch !== "\n") at++
    if (at >= chars.length)
      throw new Error(
        `code-session: ${path} has ${l} lines, no line ${line} to type at`,
      )
    at++
  }
  let end = at
  while (end < chars.length && chars[end]?.ch !== "\n") end++
  if (column === undefined) return end
  if (column < 1 || at + column - 1 > end)
    throw new Error(
      `code-session: ${path}:${line} has ${end - at} columns, no column ${column}`,
    )
  return at + column - 1
}

/** A diff that size or larger screams: the longest common run is a table of it. */
const DIFF_CELLS = 4_000_000

/** The edits that make `live` read `to`, each at an offset of `live`, in order: what
 *  both start and end with kept, the rest by their longest common run of characters.
 *  Throws on a change too wide to diff (DIFF_CELLS). */
function changes(
  live: readonly Char[],
  to: readonly string[],
): { from: number; length: number; text: string[] }[] {
  let head = 0
  while (head < live.length && head < to.length && live[head]?.ch === to[head])
    head++
  let tail = 0
  while (
    tail < live.length - head &&
    tail < to.length - head &&
    live[live.length - 1 - tail]?.ch === to[to.length - 1 - tail]
  )
    tail++
  const a = live.slice(head, live.length - tail).map((c) => c.ch)
  const b = to.slice(head, to.length - tail)
  const w = b.length + 1
  if ((a.length + 1) * w >= DIFF_CELLS)
    throw new Error(
      `code-session: a formatted save changes ${a.length} characters into ${b.length}, too wide to diff`,
    )
  // run[i * w + j]: the longest common run of a[i..] and b[j..].
  const run = new Uint32Array((a.length + 1) * w)
  for (let i = a.length - 1; i >= 0; i--)
    for (let j = b.length - 1; j >= 0; j--)
      run[i * w + j] =
        a[i] === b[j]
          ? (run[(i + 1) * w + j + 1] as number) + 1
          : Math.max(
              run[(i + 1) * w + j] as number,
              run[i * w + j + 1] as number,
            )
  const out: { from: number; length: number; text: string[] }[] = []
  let open: { from: number; length: number; text: string[] } | null = null
  const edit = () => {
    open ??= { from: head + i, length: 0, text: [] }
    return open
  }
  let i = 0
  let j = 0
  while (i < a.length || j < b.length) {
    if (i < a.length && j < b.length && a[i] === b[j]) {
      if (open) out.push(open)
      open = null
      i++
      j++
    } else if (
      j < b.length &&
      (i === a.length ||
        (run[i * w + j + 1] as number) >= (run[(i + 1) * w + j] as number))
    ) {
      edit().text.push(b[j] as string)
      j++
    } else {
      edit().length++
      i++
    }
  }
  if (open) out.push(open)
  return out
}

/** Keys typed from `first` (ms) on, a newline's indentation landing with it. */
function keysOf(text: string, first: number): { chars: Char[]; keys: number } {
  const chars: Char[] = []
  let keys = 0
  let indenting = false
  for (const ch of text) {
    if (ch === "\n") indenting = true
    else if (indenting && (ch === " " || ch === "\t")) {
      chars.push({ ch, born: first + (keys - 1) * TYPE_MS, died: Infinity })
      continue
    } else indenting = false
    chars.push({ ch, born: first + keys * TYPE_MS, died: Infinity })
    keys++
  }
  return { chars, keys }
}

/** The text `diagnoseWords` reads: each word a tool would flag, and what it says. */
export interface CodeWordRule {
  word: string
  message: string
  severity?: CodeSeverity
  source?: string
}

/** An engine for a demo without one: every whole-word occurrence of a rule's word
 *  is flagged, in whatever file it is run on, so a typo fixed is a diagnostic gone.
 *  It reads the file alone, never the rest of the workspace. */
export function diagnoseWords(rules: readonly CodeWordRule[]): CodeDiagnose {
  return (path, workspace) => {
    const text = workspace[path] as string
    return rules.flatMap((r) => {
      const out: CodeDiagnostic[] = []
      const escaped = r.word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
      for (const m of text.matchAll(
        new RegExp(`(?<![\\w$])${escaped}(?![\\w$])`, "g"),
      ))
        out.push({
          from: m.index,
          to: m.index + r.word.length,
          severity: r.severity ?? "error",
          message: r.message,
          source: r.source,
        })
      return out
    })
  }
}

/** Times an edit of `files`. THROWS on a step that cannot happen: a path not in
 *  `files`, keys with no file in front, a place the text does not have, a range
 *  past it, a pick outside the list, a rename of no word, a delay below zero, a
 *  `duration` shorter than the edit. */
export function codeTimeline(
  files: CodeFile[],
  steps: CodeStep[],
  { duration, diagnose }: CodeTimelineOptions = {},
): CodeTimeline {
  const chars: Record<string, Char[]> = {}
  const states: CodeState[] = []
  const order: Map<Char, number>[] = []
  const snapshot = (path: string, from: number) => {
    const live = standing(chars[path] as Char[])
    states.push({ path, text: textOf(live), from })
    order.push(new Map(live.map((c, i) => [c, i])))
  }
  for (const f of files) {
    if (chars[f.path])
      throw new Error(`code-session: ${f.path} is listed twice`)
    chars[f.path] = [...f.text].map((ch) => ({ ch, born: -1, died: Infinity }))
    snapshot(f.path, -1)
  }
  const carets: Record<string, Char | null> = {}
  const starts: number[] = []
  const ends: number[] = []
  const plans: Plan[] = []
  const chapters: Chapter[] = []
  let active: string | null = null
  let at = 0
  const front = (what: string): string => {
    if (!active) throw new Error(`code-session: ${what} with no file open`)
    return active
  }
  const fileOf = (path: string): Char[] => {
    const doc = chars[path]
    if (!doc) throw new Error(`code-session: ${path}, which files lacks`)
    return doc
  }
  /** The standing character at a place of `path` (null: its end). */
  const charAt = (
    path: string,
    line: number,
    column: number | undefined,
  ): Char | null => {
    const live = standing(fileOf(path))
    return live[offsetOf(live, line, column, path)] ?? null
  }
  /** Where the caret of `path` stands: the character it stands before, the file's
   *  start before any. */
  const caretOf = (path: string): Char | null => {
    const c = carets[path]
    if (c !== undefined) return c
    return standing(fileOf(path))[0] ?? null
  }
  /** Insert `placed` into `path` before `before` (null: the end). */
  const insert = (path: string, before: Char | null, placed: Char[]) => {
    const doc = fileOf(path)
    doc.splice(before ? doc.indexOf(before) : doc.length, 0, ...placed)
  }
  for (const s of steps) {
    if ((s.delay ?? 0) < 0)
      throw new Error(`code-session: delay ${s.delay} is below zero`)
    at += s.delay ?? 0
    starts.push(at)
    let took: number
    switch (s.kind) {
      case "open": {
        fileOf(s.path)
        active = s.path
        chapters.push({ start: at, title: s.path.split("/").at(-1) as string })
        plans.push({ kind: "open", path: s.path })
        took = OPEN_MS
        break
      }
      case "type": {
        const path = front("keys typed")
        if (s.line === undefined && s.column !== undefined)
          throw new Error("code-session: a column needs its line")
        const before =
          s.line !== undefined ? charAt(path, s.line, s.column) : caretOf(path)
        const { chars: placed, keys } = keysOf(s.text, at + PLACE_MS)
        insert(path, before, placed)
        carets[path] = before
        snapshot(path, at)
        plans.push({ kind: "type", path, chars: placed })
        chapters.push({
          start: at,
          title: s.text.trim().split("\n")[0] as string,
          code: true,
        })
        took = PLACE_MS + keys * TYPE_MS + ESC_MS
        break
      }
      case "replace": {
        const path = front("a replace")
        const live = standing(fileOf(path))
        const from = offsetOf(live, s.line, s.column, path)
        if (s.length < 0 || from + s.length > live.length)
          throw new Error(
            `code-session: ${path}:${s.line}:${s.column} has no ${s.length} characters to replace`,
          )
        const removed = live.slice(from, from + s.length)
        const after = live[from + s.length] ?? null
        const cut = at + PLACE_MS + SELECT_MS
        for (const c of removed) c.died = cut
        const { chars: placed, keys } = keysOf(s.text, cut)
        insert(path, after, placed)
        carets[path] = after
        snapshot(path, cut)
        plans.push({
          kind: "replace",
          path,
          removed,
          chars: placed,
          cut,
          before: after,
        })
        chapters.push({
          start: at,
          title: s.text.trim().split("\n")[0] || "delete",
          code: s.text.trim() !== "",
        })
        took = PLACE_MS + SELECT_MS + Math.max(1, keys) * TYPE_MS + ESC_MS
        break
      }
      case "save": {
        const path = front("save")
        if (s.formatted !== undefined) {
          const landed = at + SAVE_KEY_MS * 3
          const live = standing(fileOf(path))
          for (const e of changes(live, [...s.formatted])) {
            for (const c of live.slice(e.from, e.from + e.length))
              c.died = landed
            const placed = e.text.map((ch) => ({
              ch,
              born: landed,
              died: Infinity,
            }))
            insert(path, live[e.from + e.length] ?? null, placed)
          }
          snapshot(path, landed)
        }
        plans.push({ kind: "save", path })
        took = SAVE_MS
        break
      }
      case "hover": {
        const path = front("a hover")
        const caret = charAt(path, s.line, s.column)
        carets[path] = caret
        plans.push({ kind: "hover", path, caret, text: s.text })
        took = PLACE_MS + POPUP_MS
        break
      }
      case "complete": {
        const path = front("a completion")
        const item = s.items[s.pick]
        if (!item)
          throw new Error(
            `code-session: pick ${s.pick} of ${s.items.length} completions`,
          )
        const place =
          s.line !== undefined ? charAt(path, s.line, s.column) : caretOf(path)
        const live = standing(fileOf(path))
        const end = place ? live.indexOf(place) : live.length
        let start = end
        while (start > 0 && WORD.test((live[start - 1] as Char).ch)) start--
        const open = at + (s.line !== undefined ? PLACE_MS : 0) + LIST_MS
        const accept = open + (s.pick + 1) * LIST_STEP_MS
        for (const c of live.slice(start, end)) c.died = accept
        const placed = [...(item.insert ?? item.label)].map((ch) => ({
          ch,
          born: accept,
          died: Infinity,
        }))
        insert(path, place, placed)
        carets[path] = place
        snapshot(path, accept)
        plans.push({
          kind: "complete",
          path,
          place,
          anchor: live[start] ?? place,
          items: s.items,
          pick: s.pick,
          open,
          accept,
          chars: placed,
        })
        chapters.push({ start: at, title: item.label, code: true })
        took = accept - at + PICKED_MS
        break
      }
      case "signature": {
        const path = front("a signature")
        const anchor =
          s.line !== undefined ? charAt(path, s.line, s.column) : caretOf(path)
        if (s.line !== undefined) carets[path] = anchor
        const i = s.active ? s.label.indexOf(s.active) : -1
        if (s.active && i < 0)
          throw new Error(
            `code-session: "${s.active}" is not in the signature ${s.label}`,
          )
        plans.push({
          kind: "signature",
          path,
          place: s.line !== undefined ? anchor : undefined,
          anchor,
          label: s.label,
          active:
            s.active && i >= 0 ? { from: i, to: i + s.active.length } : null,
        })
        took = (s.line !== undefined ? PLACE_MS : 0) + POPUP_MS
        break
      }
      case "goto": {
        const caret = charAt(s.path, s.line, s.column ?? 1)
        const live = standing(fileOf(s.path))
        const lineStart = offsetOf(live, s.line, 1, s.path)
        const lineEnd = offsetOf(live, s.line, undefined, s.path)
        carets[s.path] = caret
        active = s.path
        plans.push({
          kind: "goto",
          path: s.path,
          caret,
          jump: at + GOTO_MS,
          line: live.slice(lineStart, lineEnd),
        })
        chapters.push({
          start: at,
          title: `${s.path.split("/").at(-1)}:${s.line}`,
        })
        took = GOTO_MS + FLASH_MS
        break
      }
      case "rename": {
        const path = front("a rename")
        const live = standing(fileOf(path))
        const place = offsetOf(live, s.line, s.column, path)
        let a = place
        let b = place
        while (a > 0 && WORD.test((live[a - 1] as Char).ch)) a--
        while (b < live.length && WORD.test((live[b] as Char).ch)) b++
        const name = textOf(live.slice(a, b))
        if (!name)
          throw new Error(
            `code-session: ${path}:${s.line}:${s.column} is on no word to rename`,
          )
        const typing = at + PLACE_MS + PROMPT_MS
        const accept = typing + s.to.length * TYPE_MS + ENTER_MS
        const word = new RegExp(
          `(?<![\\w$])${name.replace(/\$/g, "\\$")}(?![\\w$])`,
          "g",
        )
        const names: Record<string, Char[][]> = {}
        for (const p of s.files ?? files.map((f) => f.path)) {
          const doc = standing(fileOf(p))
          const text = textOf(doc)
          const found = [...text.matchAll(word)]
          if (!found.length) continue
          names[p] = found.map((m) => {
            const gone = doc.slice(m.index, m.index + name.length)
            for (const c of gone) c.died = accept
            const fresh = [...s.to].map((ch) => ({
              ch,
              born: accept,
              died: Infinity,
            }))
            insert(p, doc[m.index + name.length] ?? null, fresh)
            return fresh
          })
          snapshot(p, accept)
        }
        if (!names[path])
          throw new Error(
            `code-session: ${name} is not a whole word in ${path}`,
          )
        const order = [path, ...Object.keys(names).filter((p) => p !== path)]
        const tour = order.map((p, i) => ({
          path: p,
          from: accept + i * SHOW_MS,
          names: names[p] as Char[][],
        }))
        carets[path] = live[b] ?? null
        plans.push({
          kind: "rename",
          path,
          anchor: live[a] ?? null,
          to: s.to,
          typing,
          accept,
          tour,
        })
        chapters.push({ start: at, title: `${name} → ${s.to}`, code: true })
        // The tour ends back on the file the rename began in.
        took = accept - at + order.length * SHOW_MS
        active = path
        break
      }
      case "problems": {
        plans.push({ kind: "problems" })
        took = PROBLEMS_MS
        break
      }
    }
    at += took
    ends.push(at)
  }
  const natural = at + HOLD_MS
  if (duration !== undefined && duration < natural)
    throw new Error(
      `code-session: duration ${duration} ms is shorter than the edit's ${natural} ms`,
    )
  const total = duration ?? natural
  // An edit that opens a file at once never shows the empty editor as a chapter.
  if (chapters[0]?.start !== 0)
    chapters.unshift({ start: 0, title: "the editor" })
  const final: Record<string, string> = {}
  for (const [path, doc] of Object.entries(chars))
    final[path] = textOf(standing(doc))
  const paths = files.map((f) => f.path)
  const memo = new Map<string, CodeProblems>()
  return {
    files,
    steps,
    starts,
    ends,
    total,
    final,
    playable: { duration: total, chapters },
    chars,
    states,
    order,
    plans,
    diagnose: diagnose
      ? (workspace) => {
          // Each text after its length, so no text can read as two.
          const key = paths
            .map((p) => `${workspace[p]?.length}:${workspace[p]}`)
            .join("")
          let d = memo.get(key)
          if (!d) {
            d = problemsOf(paths, workspace, diagnose)
            memo.set(key, d)
          }
          return d
        }
      : null,
  }
}

/** Where a character stands among those standing at `ms` (one gone: where it was;
 *  null: the end). */
function placeIn(doc: readonly Char[], c: Char | null, ms: number): number {
  const end = c ? doc.indexOf(c) : doc.length
  let n = 0
  for (let i = 0; i < end; i++) if (stands(doc[i] as Char, ms)) n++
  return n
}

/** Every file's text at `ms`, by path: what `diagnose` is given. */
export function workspaceAt(timeline: CodeTimeline, ms: number): CodeWorkspace {
  return Object.fromEntries(
    timeline.files.map((f) => [
      f.path,
      textOf((timeline.chars[f.path] as Char[]).filter((c) => stands(c, ms))),
    ]),
  )
}

/** The frame at `ms`: every step done so far applied, the one under way partly. */
export function codeFrameAt(timeline: CodeTimeline, ms: number): CodeFrame {
  const open: string[] = []
  let active: string | null = null
  let mode: CodeMode = "normal"
  let command = ""
  let saved: CodeFrame["saved"] = null
  let step = -1
  // What the step under way shows, held as characters until the front file's text
  // at `ms` is known, then placed in it.
  let popup: { plan: Plan; anchor: Char | null } | null = null
  let selection: { first: Char | null; after: Char | null } | null = null
  let prompt: { label: string; text: string; anchor: Char | null } | null = null
  let panel: CodeFrame["panel"] = null
  let flashes: Char[][] = []
  /** Per file: the caret (the character it stands before, null at the end). */
  const carets: Record<string, Char | null> = {}
  /** Per file: the last save landed so far. */
  const savedAt: Record<string, number> = {}
  const show = (path: string) => {
    if (!open.includes(path)) open.push(path)
    active = path
  }
  const after = (path: string, last: Char | undefined | null) => {
    const doc = timeline.chars[path] as Char[]
    return last ? (doc[doc.indexOf(last) + 1] ?? null) : null
  }
  for (const [i, plan] of timeline.plans.entries()) {
    const from = timeline.starts[i] as number
    const to = timeline.ends[i] as number
    if (ms < from) break
    step = i
    const now = ms < to
    popup = null
    selection = null
    prompt = null
    panel = null
    flashes = []
    switch (plan.kind) {
      case "open":
        show(plan.path)
        break
      case "type": {
        // The caret stands after the last key landed, else where the keys go.
        const last = plan.chars.findLast((c) => c.born <= ms)
        carets[plan.path] = last
          ? after(plan.path, last)
          : (plan.chars[0] ?? carets[plan.path] ?? null)
        if (now) mode = "insert"
        break
      }
      case "replace": {
        if (ms < plan.cut) {
          carets[plan.path] = plan.before
          if (now && ms >= from + PLACE_MS) {
            mode = "select"
            selection = {
              first: plan.removed[0] ?? plan.before,
              after: plan.before,
            }
          }
        } else {
          const last = plan.chars.findLast((c) => c.born <= ms)
          carets[plan.path] = last ? after(plan.path, last) : plan.before
          if (now) mode = "insert"
        }
        break
      }
      case "save": {
        const landed = from + SAVE_KEY_MS * 3
        if (ms < landed) {
          mode = "command"
          command = ms < from + SAVE_KEY_MS ? ":" : ":w"
        } else {
          savedAt[plan.path] = landed
          saved = { path: plan.path, at: landed }
        }
        break
      }
      case "hover":
        carets[plan.path] = plan.caret
        if (now && ms >= from + PLACE_MS) popup = { plan, anchor: plan.caret }
        break
      case "complete": {
        carets[plan.path] =
          ms >= plan.accept && plan.chars.length
            ? after(plan.path, plan.chars.at(-1))
            : plan.place
        if (now) {
          mode = "insert"
          if (ms >= plan.open && ms < plan.accept)
            popup = { plan, anchor: plan.anchor }
        }
        break
      }
      case "signature":
        if (plan.place !== undefined) carets[plan.path] = plan.place
        if (now) popup = { plan, anchor: plan.anchor }
        break
      case "goto":
        if (ms >= plan.jump) {
          show(plan.path)
          carets[plan.path] = plan.caret
          if (now) flashes = [plan.line]
        }
        break
      case "rename": {
        if (now && ms >= from + PLACE_MS && ms < plan.accept) {
          const typed = Math.min(
            plan.to.length,
            Math.max(0, Math.floor((ms - plan.typing) / TYPE_MS) + 1),
          )
          prompt = {
            label: "rename-to",
            text: ms < plan.typing ? "" : plan.to.slice(0, typed),
            anchor: plan.anchor,
          }
        }
        if (ms >= plan.accept) {
          const visit = plan.tour.findLast((t) => ms >= t.from)
          if (!visit) throw new Error("code-session: a rename with no file")
          // Each file changed in turn, then back to the one it began in.
          const back = !now
          show(back ? plan.path : visit.path)
          if (!back) flashes = visit.names
        }
        break
      }
      case "problems":
        if (now) panel = "problems"
        break
    }
  }

  const doc = active ? (timeline.chars[active] as Char[]) : []
  const shown = doc.filter((c) => stands(c, ms))
  const text = textOf(shown)
  // The state the front file is coloured by: the last one current at `ms`.
  let state = -1
  for (const [n, s] of timeline.states.entries())
    if (s.path === active && s.from <= ms) state = n
  const order = timeline.order[state]
  const origin = order
    ? shown.map((c) => {
        const o = order.get(c)
        if (o === undefined)
          throw new Error(
            `code-session: a character shown at ${ms} ms is not in the state it is coloured by`,
          )
        return o
      })
    : []
  const place = (c: Char | null) => placeIn(doc, c, ms)
  const caret = active
    ? place(
        carets[active] !== undefined
          ? (carets[active] as Char | null)
          : (doc[0] ?? null),
      )
    : 0
  const dirty = open.filter((p) =>
    (timeline.chars[p] as Char[]).some((c) => {
      const since = savedAt[p] ?? -1
      return (
        (c.born >= 0 && c.born <= ms && c.born > since) ||
        (c.died <= ms && c.died > since)
      )
    }),
  )
  const problems = timeline.diagnose
    ? timeline.diagnose(workspaceAt(timeline, ms))
    : []
  const diagnostics = problems.find((p) => p.path === active)?.diagnostics ?? []
  const ranges = (groups: Char[][]) =>
    groups
      .filter((g) => g.length && doc.includes(g[0] as Char))
      .map((g) => ({
        from: place(g[0] as Char),
        to: place(g.at(-1) as Char) + 1,
      }))
  return {
    open,
    active,
    text,
    state,
    origin,
    caret,
    selection:
      selection && active
        ? { from: place(selection.first), to: place(selection.after) }
        : null,
    dirty,
    mode,
    command,
    prompt: prompt ? { ...prompt, at: place(prompt.anchor) } : null,
    saved,
    diagnostics,
    problems,
    popup: popup
      ? popupAt(popup.plan, place(popup.anchor), caret, diagnostics, ms)
      : null,
    flash: ranges(flashes),
    panel,
    step,
  }
}

/** A step's popup, placed at `at` in the front file's text. */
function popupAt(
  plan: Plan,
  at: number,
  caret: number,
  diagnostics: readonly CodeDiagnostic[],
  ms: number,
): CodePopup {
  switch (plan.kind) {
    case "hover":
      return {
        kind: "hover",
        at,
        text: plan.text,
        // What the tool says of the place hovered, as both editors show it there.
        diagnostics: diagnostics.filter(
          (d) => d.from <= caret && caret <= d.to,
        ),
      }
    case "complete":
      return {
        kind: "completion",
        at,
        items: plan.items,
        // One step down the list each LIST_STEP_MS from its opening, resting on
        // the pick.
        selected: Math.min(
          plan.pick,
          Math.max(0, Math.floor((ms - plan.open) / LIST_STEP_MS)),
        ),
      }
    case "signature":
      return { kind: "signature", at, label: plan.label, active: plan.active }
    default:
      throw new Error(`code-session: a ${plan.kind} step has no popup`)
  }
}

/** A caret offset in `text` as a place: `line` and `column`, both from 1. */
export function placeOf(
  text: string,
  offset: number,
): { line: number; column: number } {
  let line = 1
  let start = 0
  for (let i = 0; i < offset; i++)
    if (text[i] === "\n") {
      line++
      start = i + 1
    }
  return { line, column: offset - start + 1 }
}
