// The docs, built and held to what they claim: every page made (a link to a heading or
// a file that is not there throws, a code block naming no language throws), and every
// Cells file they show read by the engine, which must read it whole: an example the
// engine cannot read is a sentence of the docs that is not true. A `cells draft` block
// is written with what the spec's Open section proposes, and must NOT read yet: once
// the engine reads it, its `draft` is a label that has gone stale.
//
//   bun scripts/docs.ts

import { loadEngine } from "@/lib/engine-module"
import { blocksOf, docPages } from "@/lib/docs"

const engine = await loadEngine()
const pages = await docPages()
let failed = 0
let read = 0
let drafts = 0
for (const page of pages)
  for (const block of blocksOf(page.file, page.tree)) {
    if (block.lang !== "cells") continue
    const draft = (block.meta ?? "").split(/\s+/).includes("draft")
    const r = engine.read(block.value)
    try {
      const whole = r.broken.length === 0
      if (draft && whole) {
        failed++
        console.error(`✗ ${page.path} (${page.file}): a cells draft now reads whole: the engine has what it proposes, drop its draft`)
      } else if (draft) drafts++
      else if (!whole) {
        failed++
        console.error(`✗ ${page.path} (${page.file}): a cells example does not read: ${r.broken.map((b) => b.why).join("; ")}`)
      } else read++
    } finally {
      r.doc.free()
    }
  }
console.log(
  `${failed ? "✗" : "✓"} docs: ${pages.length} pages, every link resolved; ${read} cells examples read whole, ${drafts} drafts not yet${failed ? `; ${failed} wrong` : ""}`,
)
if (failed) process.exit(1)
