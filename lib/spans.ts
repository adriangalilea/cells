// Where in a file's text each problem's cell is: its line and the columns it spans,
// what an editor underlines and where it hangs the message. The engine says a problem's
// row and column today; when it says the span of the characters that broke (start and
// length inside the cell's text, `span`), that span is used and the cell's edges only
// place it. Framework-free.

export interface ProblemAt {
  row: number
  at: number
  /** The column's name: a message on its own line says which cell it is about. */
  column: string
  reason: string
  /** The characters that broke, inside the cell's text, when the engine says them. */
  span?: { start: number; length: number }
}

/** A problem placed in the text: 1-based line, 0-based columns [from, to). */
export interface Placed {
  line: number
  from: number
  to: number
  message: string
}

/** The fence a prelude line opens, its backticks; `null` when it opens none. */
function fenceOpened(line: string): number | null {
  const m = /(`{3,})[a-z]+(?:\s+[a-z]+)?\s*$/.exec(line)
  return m ? (m[1] as string).length : null
}

/** The offset where the header starts: after the prelude when there is one, a `---`
 *  inside a fence being the fence's. */
function headerStart(text: string): number {
  if (!text.startsWith("---\n") && !text.startsWith("---\r\n")) return 0
  let at = text.indexOf("\n") + 1
  let fence: number | null = null
  while (at < text.length) {
    const end = text.indexOf("\n", at)
    const line = text.slice(at, end === -1 ? text.length : end).replace(/\r$/, "")
    const next = end === -1 ? text.length : end + 1
    if (fence !== null) {
      if (line.trim() === "`".repeat(fence)) fence = null
    } else if (line === "---") return next
    else fence = fenceOpened(line)
    at = next
  }
  return text.length
}

/** Each record's fields from `start`: their [from, to) offsets in the text, quotes
 *  included, as RFC 4180 reads them (a quoted field may hold commas and line breaks). */
function recordsFrom(text: string, start: number): [number, number][][] {
  const records: [number, number][][] = []
  let fields: [number, number][] = []
  let from = start
  let quoted = false
  for (let i = start; i <= text.length; i++) {
    const ch = text[i]
    if (quoted) {
      if (ch === '"') {
        if (text[i + 1] === '"') i++
        else quoted = false
      }
      continue
    }
    if (ch === '"' && i === from) quoted = true
    else if (ch === ",") {
      fields.push([from, i])
      from = i + 1
    } else if (ch === "\n" || ch === undefined) {
      const to = text[i - 1] === "\r" ? i - 1 : i
      if (!(ch === undefined && from === i && fields.length === 0)) {
        fields.push([from, to])
        records.push(fields)
      }
      fields = []
      from = i + 1
    }
  }
  return records
}

/** Every problem placed on its cell's characters. A problem whose cell the text does
 *  not hold (a file mid-edit, a row the engine read that this scan did not) is left out
 *  rather than drawn in the wrong place. */
export function placeProblems(text: string, problems: readonly ProblemAt[]): Placed[] {
  const records = recordsFrom(text, headerStart(text))
  const lineOf = (offset: number) => text.slice(0, offset).split("\n").length
  const columnOf = (offset: number) => offset - (text.lastIndexOf("\n", offset - 1) + 1)
  const placed: Placed[] = []
  for (const p of problems) {
    const field = records[p.row + 1]?.[p.at]
    if (!field) continue
    const [start, end] = field
    const from = p.span ? start + p.span.start : start
    const to = p.span ? from + Math.max(1, p.span.length) : Math.max(end, start + 1)
    // The engine's own words without the row (the mark is on it): `to: "coffee" is no
    // key of accounts`, never a bare `is missing`.
    placed.push({ line: lineOf(from), from: columnOf(from), to: columnOf(from) + (to - from), message: `${p.column}: ${p.reason}` })
  }
  return placed
}
