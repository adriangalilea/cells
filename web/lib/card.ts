// The link preview's picture: one PNG kept in the tree (public/og.png, drawn and checked
// by scripts/og.ts), named as every page's og:image and X's image with its size, its
// words for a reader who cannot see it, and its content's hash in the address, so an
// unfurler that keeps pictures by address fetches a new card when it changes. Read at
// build (app/layout.tsx's metadata), never in a browser.

import { createHash } from "node:crypto"
import { readFileSync } from "node:fs"
import { join } from "node:path"
import { OG } from "@/lib/og"
import { PRODUCT } from "@/lib/product"

/** The file the card is in, from the site's root, and where it is served. */
export const CARD_FILE = "public/og.png"
const CARD_PATH = "/og.png"
/** The `.cell` file the card shows open. */
export const CARD_CELL = `groceries${PRODUCT.extension}`
const CARD_ALT = `${PRODUCT.name}: ${PRODUCT.subline} Helix open on ${CARD_CELL}, each total stored beside its =, and a typo in a cost named on its row.`

/** The card as metadata names a picture. */
export function cardImage() {
  const bytes = readFileSync(join(/*turbopackIgnore: true*/ process.cwd(), CARD_FILE))
  const v = createHash("sha256").update(bytes).digest("hex").slice(0, 12)
  return { url: `${CARD_PATH}?v=${v}`, width: OG.width, height: OG.height, alt: CARD_ALT, type: "image/png" }
}
