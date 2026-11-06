// THE CARD TO A PICTURE, at build: lib/og's SVG rasterized by resvg with the font
// FILES handed to it outright, so the type is the face the card names and never
// whatever a build machine's fontconfig holds; and each face measured from its file
// (`ogFace`), so lib/og breaks a line where resvg will. Node only: a build script, a
// prepare step or a server demo reads it, never a browser.
//
//   const files = geistFiles()
//   const faces = { title: ogFace(files.sansSemiBold, "Geist"), line: ogFace(files.sans, "Geist") }
//   const png = await rasterizeOg(ogStillCard({ …, faces }), Object.values(files))
//
// Equal inputs give equal bytes (resvg is deterministic), so a site can keep the PNG
// in its tree and its check can draw it again and compare.

import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { renderAsync } from "@resvg/resvg-js"
import { OG, type OgFace } from "@/lib/og"

/** The Geist files the card and its stills are set in, from the `geist` package (the
 *  faces next/font serves the same sites): the sans for the words, the mono for an
 *  editor or a terminal still. Read from the project the build runs in, by path, so no
 *  bundler follows it into a page. */
export function geistFiles() {
  const root = join(
    /*turbopackIgnore: true*/ process.cwd(),
    "node_modules/geist/dist/fonts",
  )
  const files = {
    sans: join(root, "geist-sans/Geist-Regular.ttf"),
    sansSemiBold: join(root, "geist-sans/Geist-SemiBold.ttf"),
    mono: join(root, "geist-mono/GeistMono-Regular.ttf"),
    monoBold: join(root, "geist-mono/GeistMono-Bold.ttf"),
  }
  for (const f of Object.values(files))
    if (!existsSync(f))
      throw new Error(
        `og-still: ${f} is missing: install geist in ${process.cwd()}`,
      )
  return files
}

/** A TrueType file's tables, by tag. */
function tables(font: Buffer): Map<string, DataView> {
  const view = new DataView(font.buffer, font.byteOffset, font.byteLength)
  const count = view.getUint16(4)
  const out = new Map<string, DataView>()
  for (let i = 0; i < count; i++) {
    const at = 12 + i * 16
    const tag = font.toString("latin1", at, at + 4)
    const offset = view.getUint32(at + 8)
    const length = view.getUint32(at + 12)
    out.set(tag, new DataView(font.buffer, font.byteOffset + offset, length))
  }
  return out
}

/** A face as its file measures it: each character's advance (cmap format 4 to hmtx),
 *  its ascent and descent (hhea), its weight (OS/2), all in em. Kerning is not read:
 *  a line measured here runs a hair long, so it breaks early rather than late. */
export function ogFace(file: string, family: string): OgFace {
  const t = tables(readFileSync(file))
  const need = (tag: string) => {
    const v = t.get(tag)
    if (!v) throw new Error(`og-still: ${file} has no ${tag} table`)
    return v
  }
  const head = need("head")
  const hhea = need("hhea")
  const hmtx = need("hmtx")
  const os2 = need("OS/2")
  const cmap = need("cmap")
  const em = head.getUint16(18)
  const metrics = hhea.getUint16(34)
  const advance = (glyph: number) =>
    hmtx.getUint16(4 * Math.min(glyph, metrics - 1)) / em
  // The Unicode BMP subtable (platform 3 encoding 1, or platform 0), format 4.
  let sub: DataView | null = null
  for (let i = 0; i < cmap.getUint16(2); i++) {
    const at = 4 + i * 8
    const platform = cmap.getUint16(at)
    const encoding = cmap.getUint16(at + 2)
    const offset = cmap.getUint32(at + 4)
    if (
      ((platform === 3 && encoding === 1) || platform === 0) &&
      cmap.getUint16(offset) === 4
    ) {
      sub = new DataView(cmap.buffer, cmap.byteOffset + offset)
      break
    }
  }
  if (!sub) throw new Error(`og-still: ${file} has no format 4 cmap`)
  const s = sub
  const segs = s.getUint16(6) / 2
  const ends = 14
  const starts = ends + segs * 2 + 2
  const deltas = starts + segs * 2
  const ranges = deltas + segs * 2
  const glyphOf = (c: number) => {
    for (let i = 0; i < segs; i++) {
      if (c > s.getUint16(ends + i * 2)) continue
      const start = s.getUint16(starts + i * 2)
      if (c < start) return 0
      const delta = s.getInt16(deltas + i * 2)
      const range = s.getUint16(ranges + i * 2)
      if (range === 0) return (c + delta) & 0xffff
      const g = s.getUint16(ranges + i * 2 + range + 2 * (c - start))
      return g === 0 ? 0 : (g + delta) & 0xffff
    }
    return 0
  }
  const widths = new Map<number, number>()
  const of = (c: number) => {
    let w = widths.get(c)
    if (w === undefined) {
      const g = glyphOf(c)
      if (g === 0)
        throw new Error(
          `og-still: ${family} (${file}) has no glyph for U+${c.toString(16).toUpperCase()}`,
        )
      w = advance(g)
      widths.set(c, w)
    }
    return w
  }
  return {
    family,
    weight: os2.getUint16(4),
    ascent: hhea.getInt16(4) / em,
    descent: -hhea.getInt16(6) / em,
    width: (text) => {
      let w = 0
      for (const ch of text) w += of(ch.codePointAt(0) as number)
      return w
    },
  }
}

/** The card as a PNG at `width` px (the card's own by default), set in `fonts` and
 *  nothing else: a family the SVG names that no file holds falls to the first file,
 *  never to the system's. */
export async function rasterizeOg(
  svg: string,
  fonts: readonly string[],
  { width = OG.width }: { width?: number } = {},
): Promise<Buffer> {
  if (fonts.length === 0)
    throw new Error("og-still: no font files to set the card in")
  const image = await renderAsync(svg, {
    fitTo: { mode: "width", value: width },
    font: { loadSystemFonts: false, fontFiles: [...fonts] },
  })
  return Buffer.from(image.asPng())
}
