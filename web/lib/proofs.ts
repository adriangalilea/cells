// WHAT THE LANDING SHOWS AS PROOF of what Cells opens up: each a file or a command as it
// is, never a sketch. `scripts/examples.ts` reads every Cells one with the engine (it must
// read whole) and makes the check's line from the engine's own words, so a proof on the
// page can never claim what the engine does not do.

import { PRODUCT } from "@/lib/product"

const ext = PRODUCT.extension

/** The folder the check runs on: a trip's costs, one paid by a name no friend has. */
export const TRIP_FOLDER = "trips"
export const TRIP_FILES = [
  {
    name: `people${ext}`,
    text: "name:text!,email:email\nana,ana@example.com\nben,ben@example.com\ncarla,carla@example.com\ndan,dan@example.com\n",
  },
  {
    name: `lisbon${ext}`,
    text: `---\nimport people from "./people${ext}"\n---\nitem,paid:people.name,cost:EUR>=0,share:EUR=cost / 4\nflights,ana,412.00,103.00=\ndinner,karla,96.75,24.19=\n`,
  },
]
/** The check's one problem, as the engine says it. */
export const TRIP_PROBLEM = 'row 2, paid: "karla" is no key of people'

export const PROOFS = {
  folder: {
    lang: "console",
    text: `$ ${PRODUCT.cli} check ${TRIP_FOLDER}\n${TRIP_FOLDER}/lisbon${ext} ${TRIP_PROBLEM}`,
  },
  git: { lang: "text", text: `# .gitattributes\n*${ext} merge=cells` },
  live: {
    lang: "cells",
    text: `---
rates = run every day at 18:00 in Europe/Lisbon
\`\`\`ts
export default async () => (await fetch("https://api.example.com/jpy-eur")).json()
\`\`\`
\`\`\`cells
on:date!,eur:num
2026-11-03,0.0061
\`\`\`
---
item,cost:JPY,on:date,eur:EUR=cost * rates[on].eur
ramen,1200,2026-11-03,7.32=
`,
  },
} as const
