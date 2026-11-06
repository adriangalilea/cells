// LINK PREVIEWS: the card an unfurler shows for a page (Open Graph's `og:image`, X's
// `summary_large_image`), drawn as one SVG from data and rasterized before the build
// (lib/og-still), so a crawler fetches a static file and no function runs for it.
// Framework-free by contract: no React, no DOM, no Node; what reads files is og-still.
//
//   const card = ogStillCard({ title, line, still, faces, look })   // an SVG string
//   const png = await rasterizeOg(card, files)                      // lib/og-still
//
// Designed once against every crop (`OG_CROPS`): whatever the card says sits inside
// `OG_SAFE`, the part no unfurler cuts, and the type is set for the size a reader sees
// the card at, a third of it or less, never for the 1200 px it is drawn at. A still on
// the card is another item's SVG still (terminal-editor-screen `renderEditorSvg`,
// terminal-session `renderSessionSvg`), drawn in: one drawing, never a second one here.
// A quote's card is quote-card's `renderQuoteSvg` alone, not a layout of this file.

/** The card, px: the size every unfurler asks for (1.91:1). */
export const OG = { width: 1200, height: 630 } as const

/** A part of the card an unfurler shows, and about how wide a reader sees it, px. */
export interface OgCrop {
  name: string
  /** Who shows it so. */
  who: string
  x: number
  y: number
  width: number
  height: number
  /** About how wide it is shown, px: what the type is set for. */
  seen: number
}

/** What each unfurler keeps of the card: the whole of it in a large preview, X's 2:1
 *  cut from its middle, and a centred square where a preview is a thumbnail beside the
 *  words (WhatsApp; Telegram, iMessage and Slack when they show it small). */
export const OG_CROPS: readonly OgCrop[] = [
  {
    name: "whole",
    who: "Telegram, Slack, Discord, iMessage, LinkedIn, Facebook",
    x: 0,
    y: 0,
    width: OG.width,
    height: OG.height,
    seen: 400,
  },
  {
    name: "2:1",
    who: "X, summary_large_image",
    x: 0,
    y: (OG.height - OG.width / 2) / 2,
    width: OG.width,
    height: OG.width / 2,
    seen: 506,
  },
  {
    name: "square",
    who: "a thumbnail beside the words: WhatsApp, and the small previews",
    x: (OG.width - OG.height) / 2,
    y: 0,
    width: OG.height,
    height: OG.height,
    seen: 96,
  },
]

/** The card's margin, px: a tenth of its height on every side. */
export const OG_PAD = OG.height / 10

/** What every crop but the square keeps, inside the margin: where the words go. */
export const OG_SAFE = {
  x: OG_PAD,
  y: (OG.height - OG.width / 2) / 2 + OG_PAD,
  width: OG.width - 2 * OG_PAD,
  height: OG.width / 2 - 2 * OG_PAD,
} as const

/** A face the card sets words in, as its file measures it: `width` of a text in em
 *  (lib/og-still `ogFace` reads it from the font), so a line breaks where the
 *  rasterizer will break it, never at a guessed average. */
export interface OgFace {
  /** The family the SVG names, as the rasterizer is handed its file. */
  family: string
  weight: number
  ascent: number
  descent: number
  /** The capitals' height above the baseline, em: where words are seen to begin, which
   *  a line's ascent overshoots. */
  cap: number
  width: (text: string) => number
}

/** The card's colours: its ground, the words, the quieter words. */
export interface OgLook {
  ground: string
  ink: string
  muted: string
  /** A soft light from the still's side, over the ground. None when unset. */
  light?: string
}

/** A line of words, some in their own ink (a product's extension lit in its colour). */
export type OgWords = readonly (string | { text: string; ink: string })[]

/** Text as SVG markup takes it, in an element or an attribute's quotes. */
const xmlText = (s: string) =>
  s
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")

/** `words` broken into lines no wider than `width` px at `size`, each line a list of
 *  pieces in one ink. A single word wider than the line screams: shrink the type or
 *  say it shorter, never a word cut. */
export function wrapWords(
  words: OgWords,
  face: OgFace,
  size: number,
  width: number,
): { text: string; ink: string | null }[][] {
  const tokens = words.flatMap((w) => {
    const { text, ink } = typeof w === "string" ? { text: w, ink: null } : w
    return text
      .split(/(\s+)/)
      .filter(Boolean)
      .map((t) => ({ text: t, ink }))
  })
  const lines: { text: string; ink: string | null }[][] = [[]]
  let used = 0
  for (const t of tokens) {
    const line = lines.at(-1) as { text: string; ink: string | null }[]
    if (/^\s+$/.test(t.text)) {
      if (line.length) {
        line.push({ text: " ", ink: t.ink })
        used += face.width(" ") * size
      }
      continue
    }
    const w = face.width(t.text) * size
    if (w > width)
      throw new Error(
        `og: "${t.text}" is ${Math.round(w)} px at ${size} px, wider than the ${Math.round(width)} px line`,
      )
    // A word that does not fit starts the next line; the space it followed goes.
    if (used + w > width && line.length) {
      if (line.at(-1)?.text === " ") line.pop()
      lines.push([t])
      used = w
    } else {
      line.push(t)
      used += w
    }
  }
  return lines.map((l) => {
    const merged: { text: string; ink: string | null }[] = []
    for (const p of l) {
      const last = merged.at(-1)
      if (last && last.ink === p.ink) last.text += p.text
      else merged.push({ ...p })
    }
    return merged
  })
}

/** Lines of words as SVG `<text>`, the first baseline at `y`. */
function wordsSvg(
  lines: { text: string; ink: string | null }[][],
  {
    x,
    y,
    size,
    leading,
    face,
    ink,
  }: {
    x: number
    y: number
    size: number
    leading: number
    face: OgFace
    ink: string
  },
): string {
  return lines
    .map(
      (line, i) =>
        `<text x="${x}" y="${y + i * size * leading}" font-size="${size}" font-weight="${face.weight}" fill="${ink}" style="font-family: ${xmlText(face.family)}">${line
          .map((p) =>
            p.ink
              ? `<tspan fill="${p.ink}">${xmlText(p.text)}</tspan>`
              : xmlText(p.text),
          )
          .join("")}</text>`,
    )
    .join("\n  ")
}

/** A still's root and its `viewBox`: what keeps its shape when it is scaled. */
function rootOf(svg: string): { box: string; ratio: number } {
  const open = /^<svg\b([^>]*)>/.exec(svg.trim())
  if (!open) throw new Error("og: a still is an <svg> element")
  const box = /viewBox="([\d.\s-]+)"/.exec(open[1] as string)?.[1]
  if (!box) throw new Error("og: a still without a viewBox cannot be scaled")
  const [, , w, h] = box.split(/\s+/).map(Number) as number[]
  return { box, ratio: (h as number) / (w as number) }
}

/** A still drawn inside the card at `x`, `y`, `width` wide, its root's size replaced. */
function placed(svg: string, x: number, y: number, width: number): string {
  const { box, ratio } = rootOf(svg)
  return svg
    .trim()
    .replace(
      /^<svg\b[^>]*>/,
      `<svg x="${x}" y="${y}" width="${width}" height="${width * ratio}" viewBox="${box}">`,
    )
}

export interface OgCard {
  /** The name, large. */
  title: string
  /** What it is, under the name. */
  line: OgWords
  faces: { title: OgFace; line: OgFace }
  look: OgLook
}

/** The type, px, for a reader who sees the card a third of its size: the name a
 *  headline (about 35 px seen), the line a sentence (about 13 px seen). */
export const OG_TYPE = { title: 104, line: 38, leading: 1.25 } as const

/** The card's ground, and its light when it has one. */
function ground(look: OgLook, from: { x: number; y: number }): string {
  const light = look.light
    ? `<defs><radialGradient id="og-light" cx="${from.x / OG.width}" cy="${from.y / OG.height}" r="0.75"><stop offset="0" stop-color="${look.light}" stop-opacity="0.22"/><stop offset="1" stop-color="${look.light}" stop-opacity="0"/></radialGradient></defs>
  <rect width="${OG.width}" height="${OG.height}" fill="url(#og-light)"/>`
    : ""
  return `<rect width="${OG.width}" height="${OG.height}" fill="${look.ground}"/>
  ${light}`
}

/** The name and the line as a block `width` wide at `x`, centred on the card's
 *  height: the lines and the block's height. */
function words(card: OgCard, x: number, width: number): string {
  const { title, line } = card.faces
  const titleLines = wrapWords([card.title], title, OG_TYPE.title, width)
  const lineLines = wrapWords(card.line, line, OG_TYPE.line, width)
  const titleH = OG_TYPE.title * (title.ascent + title.descent)
  const gap = OG_TYPE.line * 0.9
  const linesH = lineLines.length * OG_TYPE.line * OG_TYPE.leading
  const top = (OG.height - (titleLines.length * titleH + gap + linesH)) / 2
  if (top < OG_SAFE.y)
    throw new Error(
      `og: the words are ${Math.round(OG.height - 2 * top)} px tall, more than the ${OG_SAFE.height} px every crop keeps`,
    )
  return `${wordsSvg(titleLines, {
    x,
    y: top + OG_TYPE.title * title.ascent,
    size: OG_TYPE.title,
    leading: title.ascent + title.descent,
    face: title,
    ink: card.look.ink,
  })}
  ${wordsSvg(lineLines, {
    x,
    y:
      top +
      titleLines.length * titleH +
      gap +
      (OG_TYPE.line * (OG_TYPE.leading - (line.ascent + line.descent))) / 2 +
      OG_TYPE.line * line.ascent,
    size: OG_TYPE.line,
    leading: OG_TYPE.leading,
    face: line,
    ink: card.look.muted,
  })}`
}

const svgOf = (body: string) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${OG.width} ${OG.height}" width="${OG.width}" height="${OG.height}">
  ${body}
</svg>
`

/** The name and the line alone, on the ground: a page with nothing to show but
 *  what it is. */
export function ogWordmarkCard(card: OgCard): string {
  return svgOf(
    `${ground(card.look, { x: OG.width, y: 0 })}
  ${words(card, OG_SAFE.x, OG_SAFE.width * 0.75)}`,
  )
}

/** A stacked card's type, px: the name large over the still, the line small under it.
 *  Its spaces in one unit, the name's gap to the still: the margin above the name the
 *  golden ratio of it (`above`), so the name sits nearer what it names than the edge;
 *  the line's space, around it, a share of it, so the line only breathes; and the least
 *  the line's space may be, such that X's 2:1 crop (15 px off each end) still leaves it
 *  a margin. */
export const OG_STACK = {
  title: 92,
  line: 24,
  above: (1 + Math.sqrt(5)) / 2,
  lineShare: 0.5,
  least: 28,
} as const

/** The name, a still across the card's width (an editor, a terminal) under it, and the
 *  line small under that, centred: the thing itself as large as the card allows, what
 *  it is called above it. Each part is placed as it is seen, words from their capitals'
 *  top to their baseline (a line box's ascent and descent are empty space no reader
 *  sees), and the height left over is shared in `OG_STACK`'s proportions: the margin
 *  above the name, the name's gap to the still, the line's space above it and below it.
 *  A line's space under `OG_STACK.least`, or a line that will not fit on one line,
 *  screams. */
export function ogStackCard(card: OgCard & { still: string }): string {
  const width = OG.width - 2 * OG_PAD
  const height = width * rootOf(card.still).ratio
  const { title, line } = card.faces
  const lineLines = wrapWords(card.line, line, OG_STACK.line, width)
  if (lineLines.length > 1)
    throw new Error(
      `og: the line takes ${lineLines.length} lines in ${width} px: say it shorter`,
    )
  const titleCap = OG_STACK.title * title.cap
  const lineCap = OG_STACK.line * line.cap
  // The height left over in units of the name's gap: the margin above the name, the
  // gap itself, the line's space above it and below it.
  const gap =
    (OG.height - (titleCap + height + lineCap)) /
    (OG_STACK.above + 1 + 2 * OG_STACK.lineShare)
  const lineSpace = gap * OG_STACK.lineShare
  if (lineSpace < OG_STACK.least)
    throw new Error(
      `og: the line's space is ${Math.round(lineSpace)} px, less than ${OG_STACK.least}: give the still fewer rows or more columns`,
    )
  const titleBase = gap * OG_STACK.above + titleCap
  const stillTop = titleBase + gap
  const lineBase = stillTop + height + lineSpace + lineCap
  const lineW = (lineLines[0] ?? []).reduce(
    (w, piece) => w + line.width(piece.text) * OG_STACK.line,
    0,
  )
  return svgOf(
    `${ground(card.look, { x: OG.width / 2, y: stillTop + height / 2 })}
  ${wordsSvg(
    wrapWords(
      [card.title],
      title,
      OG_STACK.title,
      title.width(card.title) * OG_STACK.title + 1,
    ),
    {
      x: OG_PAD,
      y: titleBase,
      size: OG_STACK.title,
      leading: title.ascent + title.descent,
      face: title,
      ink: card.look.ink,
    },
  )}
  ${placed(card.still, OG_PAD, stillTop, width)}
  ${wordsSvg(lineLines, {
    x: (OG.width - lineW) / 2,
    y: lineBase,
    size: OG_STACK.line,
    leading: OG_TYPE.leading,
    face: line,
    ink: card.look.muted,
  })}`,
  )
}

/** The still's share of the card's width, and the gap between it and the words. */
export const OG_STILL = { share: 0.52, gap: 48 } as const

/** The name and the line at the left, a still at the right (an editor, a terminal):
 *  the thing itself beside what it is called. The still is centred on the card's
 *  height and screams when it is taller than what every crop keeps. */
export function ogStillCard(card: OgCard & { still: string }): string {
  const width = OG.width * OG_STILL.share
  const height = width * rootOf(card.still).ratio
  if (height > OG_SAFE.height + 2 * OG_PAD)
    throw new Error(
      `og: the still is ${Math.round(height)} px tall at ${Math.round(width)} px wide, more than the ${OG.width / 2} px X keeps: give it fewer rows`,
    )
  const x = OG.width - OG_PAD - width
  const y = (OG.height - height) / 2
  return svgOf(
    `${ground(card.look, { x: x + width / 2, y: y + height / 2 })}
  ${words(card, OG_PAD, x - OG_STILL.gap - OG_PAD)}
  ${placed(card.still, x, y, width)}`,
  )
}
