"use client"

// TABLE OF CONTENTS: which part of a page is being read, and a page's headings as links
// with that one lit, as docs' "on this page" does. Two pieces, because the deciding is
// the same everywhere and the look is not: `useReading(ids)` says which part is being
// read, and `TableOfContents` draws it, as a list beside docs (`look="list"`) or as a
// few words in a landing's margin (`look="words"`). Where it stands (a sticky column,
// the margin's middle) is the page's layout, never the item's.
//
//   <TableOfContents headings={prepared.headings} />                 from markdown-prepare
//   <TableOfContents headings={[{ id: "terms", text: "Terms" }]} />   a page's own parts
//
// THE PART BEING READ is the last whose top has passed the reading line, a share of the
// window's height from its top (`line`), as the eye reads a page: a heading just gone
// under a top bar is still the one being read. At the page's foot the last part is
// being read whatever the line says, so a short last part is ever lit. Read on every
// scroll of the page or of any box in it, once a frame. A part it is given that the
// page lacks screams: the table and the page are one list.

import * as React from "react"

/** A part of a page as a table of contents lists it: its anchor, its words, and how
 *  deep it lies (a heading's depth; 2 when not given, a page's `##`). */
export interface TableOfContentsHeading {
  id: string
  text: string
  depth?: number
}

/** How far down the window the reading line lies by default. */
const LINE = 0.25

/** Which of the parts `ids` names is being read, by its id; the first before any has
 *  reached the reading line (`line`, a share of the window's height from its top). */
export function useReading(
  ids: readonly string[],
  { line = LINE }: { line?: number } = {},
): string | null {
  const [current, setCurrent] = React.useState<string | null>(null)
  const key = ids.join(" ")
  React.useEffect(() => {
    const parts = key
      .split(" ")
      .filter(Boolean)
      .map((id) => {
        const part = document.getElementById(id)
        if (!part) throw new Error(`table-of-contents: the page has no #${id}`)
        return part
      })
    let frame = 0
    const read = () => {
      frame = 0
      const scroller = document.scrollingElement ?? document.documentElement
      const atFoot =
        scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2
      const reading = innerHeight * line
      let at = parts[0]?.id ?? null
      for (const part of parts)
        if (part.getBoundingClientRect().top <= reading) at = part.id
      setCurrent(atFoot && parts.length ? (parts.at(-1) as HTMLElement).id : at)
    }
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(read)
    }
    read()
    // Capture: a scroll of any box on the page, as well as the page's own.
    addEventListener("scroll", schedule, { capture: true, passive: true })
    addEventListener("resize", schedule)
    return () => {
      cancelAnimationFrame(frame)
      removeEventListener("scroll", schedule, { capture: true })
      removeEventListener("resize", schedule)
    }
  }, [key, line])
  return current
}

/** How the table is drawn: `list`, a rule down its side, the part being read marked on
 *  it, deeper headings indented (docs); `words`, the parts' words alone in the page's
 *  margin, end-aligned in mono, the one being read the bright one (a landing's few
 *  sections). Each a look, never a layout: where the table stands is the page's. */
const LOOKS = {
  list: {
    list: "border-l border-foreground/10 text-[13px]",
    link: "-ml-px block border-l border-transparent py-1 pr-2 leading-snug text-muted-foreground transition-colors hover:text-foreground data-read:text-foreground aria-[current=location]:border-foreground aria-[current=location]:text-foreground",
    indent: true,
  },
  words: {
    list: "flex flex-col items-end gap-2.5 font-mono text-xs select-none",
    link: "text-muted-foreground/50 transition-colors duration-300 hover:text-muted-foreground data-read:text-foreground aria-[current=location]:text-foreground",
    indent: false,
  },
} as const

export type TableOfContentsLook = keyof typeof LOOKS

export interface TableOfContentsProps {
  headings: readonly TableOfContentsHeading[]
  /** How it is drawn (`list`, the default, or `words`). */
  look?: TableOfContentsLook
  /** The deepest heading listed: 3 by default, a page's `###`. */
  depth?: number
  /** Every part read so far lit, not only the one being read: the table a trail of
   *  how far the page has been read. */
  trail?: boolean
  /** A link puts its part in the address (`#terms`), so it can be shared from there.
   *  Off, a link scrolls to its part (smoothly, unless motion is reduced) and leaves
   *  the address without a hash; its `href` still opens the part in a new tab. On by
   *  default. */
  hash?: boolean
  /** The reading line, a share of the window's height from its top. */
  line?: number
  /** What it is named to assistive tech. */
  label?: string
  className?: string
}

/** A page's headings as links, the one being read lit; nothing for a page with none. */
export function TableOfContents({
  headings,
  look = "list",
  depth = 3,
  trail = false,
  hash = true,
  line = LINE,
  label = "on this page",
  className,
}: TableOfContentsProps) {
  const listed = headings.filter((h) => (h.depth ?? 2) <= depth)
  const top = Math.min(...listed.map((h) => h.depth ?? 2))
  const current = useReading(
    listed.map((h) => h.id),
    { line },
  )
  // A part clicked is the one lit while the page travels to it, not each part it
  // passes on the way; arrived, or the reader scrolling or keying on their own, the
  // reading takes over again.
  const [chosen, setChosen] = React.useState<string | null>(null)
  if (chosen !== null && chosen === current) setChosen(null)
  React.useEffect(() => {
    if (chosen === null) return
    const own = () => setChosen(null)
    const events = ["wheel", "touchmove", "keydown"] as const
    for (const e of events) addEventListener(e, own, { passive: true })
    return () => {
      for (const e of events) removeEventListener(e, own)
    }
  }, [chosen])
  const lit = chosen ?? current
  const reached = listed.findIndex((h) => h.id === lit)
  // A page with no heading to list has no table to draw.
  if (listed.length === 0) return null
  const drawn = LOOKS[look]
  return (
    <nav
      aria-label={label}
      data-slot="table-of-contents"
      data-look={look}
      className={className}
    >
      <ul className={drawn.list}>
        {listed.map((h, i) => (
          <li key={h.id}>
            <a
              href={`#${h.id}`}
              aria-current={h.id === lit ? "location" : undefined}
              data-read={trail && i <= reached ? "" : undefined}
              data-slot="table-of-contents-link"
              onClick={(e) => {
                if (opensElsewhere(e)) return
                setChosen(h.id)
                if (!hash) scrollToPart(e, h.id)
              }}
              className={drawn.link}
              style={
                drawn.indent
                  ? {
                      paddingLeft: `${0.75 + ((h.depth ?? 2) - top) * 0.75}rem`,
                    }
                  : undefined
              }
            >
              {h.text}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  )
}

/** A click that opens the link somewhere else (a new tab, a new window): the
 *  browser's, never the table's. */
const opensElsewhere = (e: React.MouseEvent) =>
  e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0

/** A link's click with `hash` off: its part scrolled to, smoothly unless motion is
 *  reduced, and the address left with no hash (one already there taken out), so a
 *  reload starts at the top. */
function scrollToPart(e: React.MouseEvent<HTMLAnchorElement>, id: string) {
  e.preventDefault()
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches
  document
    .getElementById(id)
    ?.scrollIntoView({ behavior: still ? "auto" : "smooth", block: "start" })
  history.replaceState(history.state, "", location.pathname + location.search)
}
