// The docs, built and held to what they claim: every page made (a link to a heading or
// a file that is not there throws, a code block naming no language throws), and every
// Cells file they show read by the engine, which must read it whole: an example the
// engine cannot read is a sentence of the docs that is not true, and so is one with a
// problem nobody shows. A `cells problem` block is shown for its problem: it must have
// one, and its page must say it in the engine's words. A `cells draft` block is written
// with what the spec's Open section proposes, and must NOT read yet: once the engine
// reads it, its `draft` is a label that has gone stale.
//
//   bun scripts/docs.ts

import { loadEngine } from "@/lib/engine-module"
import { blocksOf, docPages } from "@/lib/docs"

const engine = await loadEngine()
const pages = await docPages()
let failed = 0
let read = 0
let drafts = 0
let shown = 0
for (const page of pages)
  for (const block of blocksOf(page.file, page.tree)) {
    if (block.lang !== "cells") continue
    const tags = (block.meta ?? "").split(/\s+/)
    const draft = tags.includes("draft")
    const problem = tags.includes("problem")
    const r = engine.read(block.value)
    const wrong = (why: string) => {
      failed++
      console.error(`✗ ${page.path} (${page.file}): ${why}`)
    }
    try {
      const whole = r.broken.length === 0
      const whys = r.problems.map((p) => p.why)
      if (draft && whole) wrong("a cells draft now reads whole: the engine has what it proposes, drop its draft")
      else if (draft) drafts++
      else if (!whole) wrong(`a cells example does not read: ${r.broken.map((b) => b.why).join("; ")}`)
      else if (problem) {
        // An example shown for its problem: it has one, and the page says it as the
        // engine does, so the words beside it can never drift from what a reader sees.
        if (whys.length === 0) wrong("a cells problem example has no problem: drop its problem, or show one")
        for (const why of whys)
          if (!page.text.includes(why)) wrong(`a cells problem example's problem is not on its page as the engine says it: ${why}`)
        shown++
      } else if (whys.length > 0) wrong(`a cells example has problems it does not show (tag it problem): ${whys.join("; ")}`)
      else read++
    } finally {
      r.doc.free()
    }
  }
console.log(
  `${failed ? "✗" : "✓"} docs: ${pages.length} pages, every link resolved; ${read} cells examples read whole, ${shown} show their problem as the engine says it, ${drafts} drafts not yet${failed ? `; ${failed} wrong` : ""}`,
)
if (failed) process.exit(1)
