// showcase.html: every example in tree-sitter/examples, highlighted by the
// TextMate grammar through Shiki, a dark and a light pane side by side, in
// the pairs of themes.ts, which know nothing of Cells. One self-contained file.
//
//   bun run showcase.ts

import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs"
import { join, relative } from "node:path"
import { createHighlighter } from "shiki"
import grammar from "./cells.tmLanguage.json"
import { PAIRS } from "./themes.ts"

const examples = join(import.meta.dir, "../tree-sitter/examples")

// The order a reader meets them: the trip, the books from the chart of
// accounts down, then everything at once.
const ORDER = ["trip-budget.cells", "books/accounts.cells", "books/vendors.cells", "books/rates.cells", "books/home/money.cells", "every-type.cells"]

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    return statSync(path).isDirectory() ? files(path) : name.endsWith(".cells") ? [path] : []
  })
}

const found = files(examples).map((path) => relative(examples, path))
const missing = ORDER.filter((name) => !found.includes(name))
const unlisted = found.filter((name) => !ORDER.includes(name))
if (missing.length || unlisted.length)
  throw new Error(`ORDER and tree-sitter/examples disagree: missing ${missing.join(", ") || "none"}, unlisted ${unlisted.join(", ") || "none"}`)

const themes = Object.fromEntries(PAIRS.flatMap((p) => [[`${p.id}-dark`, p.dark], [`${p.id}-light`, p.light]]))
const highlighter = await createHighlighter({ themes: Object.values(themes), langs: [grammar as never, "typescript"] })

const escape = (s: string) => s.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;")

const blocks = ORDER.map((name) => {
  const code = readFileSync(join(examples, name), "utf8").replace(/\n$/, "")
  const html = highlighter.codeToHtml(code, { lang: "cells", themes, defaultColor: false })
  const caption = (mode: "dark" | "light") =>
    PAIRS.map((p) => `<span class="theme" data-pair="${p.id}">${p[mode]}</span>`).join("")
  return `
  <section>
    <h2>${escape(name)}</h2>
    <div class="panes">
      <figure class="pane" data-mode="dark"><figcaption>${caption("dark")}</figcaption>${html}</figure>
      <figure class="pane" data-mode="light"><figcaption>${caption("light")}</figcaption>${html}</figure>
    </div>
  </section>`
}).join("\n")

// Each pane paints its tokens from the variables of the chosen pair's theme
// for its mode: --shiki-<pair>-<mode>.
const css = PAIRS.flatMap((p) =>
  (["dark", "light"] as const).map((mode) => {
    const key = `${p.id}-${mode}`
    return `body[data-pair="${p.id}"] .pane[data-mode="${mode}"] pre { background: var(--shiki-${key}-bg); color: var(--shiki-${key}); }
body[data-pair="${p.id}"] .pane[data-mode="${mode}"] pre span { color: var(--shiki-${key}); font-style: var(--shiki-${key}-font-style); font-weight: var(--shiki-${key}-font-weight); text-decoration: var(--shiki-${key}-text-decoration); }`
  }).concat(`body[data-pair="${p.id}"] .theme[data-pair="${p.id}"] { display: inline; }`),
).join("\n")

const buttons = PAIRS.map((p, i) => `<button data-pair="${p.id}"${i === 0 ? ' aria-pressed="true"' : ""}>${p.label}</button>`).join("")

const page = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Cells highlighting</title>
<style>
  :root { color-scheme: light dark; font-family: -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif; }
  body { margin: 0; padding: 32px; background: #f4f3f0; color: #1f1d2b; }
  @media (prefers-color-scheme: dark) { body { background: #16151d; color: #e0def4; } }
  header { max-width: 72ch; margin-bottom: 28px; }
  h1 { font-size: 22px; margin: 0 0 8px; }
  h2 { font: 600 13px ui-monospace, "SF Mono", Menlo, monospace; margin: 36px 0 10px; opacity: 0.8; }
  p { margin: 0 0 10px; line-height: 1.5; opacity: 0.85; }
  nav { display: flex; gap: 6px; margin-top: 16px; }
  nav button { font: inherit; font-size: 13px; padding: 5px 12px; border-radius: 999px; border: 1px solid currentColor; background: none; color: inherit; opacity: 0.55; cursor: pointer; }
  nav button[aria-pressed="true"] { opacity: 1; font-weight: 600; }
  /* In the code's own font, so a pane side by side holds 72 characters of code. */
  .panes { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 72ch), 1fr)); gap: 10px; font: 12.5px ui-monospace, "SF Mono", Menlo, monospace; }
  .pane { min-width: 0; margin: 0; }
  .pane figcaption { font: 11px ui-monospace, "SF Mono", Menlo, monospace; opacity: 0.55; margin: 0 0 4px 2px; }
  .theme { display: none; }
  .pane pre { margin: 0; padding: 14px 16px; border-radius: 10px; overflow-x: auto; font: 12.5px/1.6 ui-monospace, "SF Mono", Menlo, monospace; counter-reset: line; }
  .pane pre code { display: block; width: max-content; }
  .pane pre .line::before { counter-increment: line; content: counter(line); display: inline-block; width: 2.2ch; margin-right: 1.6ch; text-align: right; opacity: 0.35; }
${css}
</style>
</head>
<body data-pair="${PAIRS[0].id}">
<header>
  <h1>Cells, highlighted</h1>
  <p>Each example in <code>tree-sitter/examples</code>, through the TextMate grammar (<code>syntaxes/cells.tmLanguage.json</code>) and Shiki, the engine GitHub and VS Code share. The themes know nothing of Cells: every colour comes from a scope they already paint.</p>
  <p>A column's name and its text cells share a colour, six in turn; numbers, dates, booleans, links and quoted text keep their own. In the header, the type, each mark, constraint and choice wear their own scope; the prelude is its own region. <code>use</code> and references like <code>accounts.account</code> are provisional, from SPEC's Open section.</p>
  <nav>${buttons}</nav>
</header>
${blocks}
<script>
  for (const b of document.querySelectorAll("nav button"))
    b.addEventListener("click", () => {
      document.body.dataset.pair = b.dataset.pair
      for (const o of document.querySelectorAll("nav button")) o.setAttribute("aria-pressed", String(o === b))
    })
</script>
</body>
</html>
`

const out = join(import.meta.dir, "showcase.html")
writeFileSync(out, page)
console.log(`wrote ${out}`)
