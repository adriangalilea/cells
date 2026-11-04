// showcase.html: every example in tree-sitter/examples, highlighted by the
// TextMate grammar through Shiki, a dark and a light pane side by side, in
// four pairs of themes that know nothing of TCSV. One self-contained file.
//
//   bun run showcase.ts

import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs"
import { join, relative } from "node:path"
import { createHighlighter } from "shiki"
import grammar from "./tcsv.tmLanguage.json"

const examples = join(import.meta.dir, "../tree-sitter/examples")

// The pairs, the first shown first: rose pine is the palette Adrian's Helix
// theme is built on.
const PAIRS = [
  { id: "rose-pine", label: "Rosé Pine", dark: "rose-pine-moon", light: "rose-pine-dawn" },
  { id: "github", label: "GitHub", dark: "github-dark", light: "github-light" },
  { id: "one", label: "One", dark: "one-dark-pro", light: "one-light" },
  { id: "catppuccin", label: "Catppuccin", dark: "catppuccin-mocha", light: "catppuccin-latte" },
] as const

// The order a reader meets them: the trip, the books from the chart of
// accounts down, then everything at once.
const ORDER = ["trip-budget.tcsv", "books/accounts.tcsv", "books/vendors.tcsv", "books/home/money.tcsv", "every-type.tcsv"]

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    return statSync(path).isDirectory() ? files(path) : name.endsWith(".tcsv") ? [path] : []
  })
}

const found = files(examples).map((path) => relative(examples, path))
const missing = ORDER.filter((name) => !found.includes(name))
const unlisted = found.filter((name) => !ORDER.includes(name))
if (missing.length || unlisted.length)
  throw new Error(`ORDER and tree-sitter/examples disagree: missing ${missing.join(", ") || "none"}, unlisted ${unlisted.join(", ") || "none"}`)

const themes = Object.fromEntries(PAIRS.flatMap((p) => [[`${p.id}-dark`, p.dark], [`${p.id}-light`, p.light]]))
const highlighter = await createHighlighter({ themes: Object.values(themes), langs: [grammar as never] })

const escape = (s: string) => s.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;")

const blocks = ORDER.map((name) => {
  const code = readFileSync(join(examples, name), "utf8").replace(/\n$/, "")
  const html = highlighter.codeToHtml(code, { lang: "tcsv", themes, defaultColor: false })
  return `
  <section>
    <h2>${escape(name)}</h2>
    <div class="panes">
      <div class="pane" data-mode="dark">${html}</div>
      <div class="pane" data-mode="light">${html}</div>
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
  }),
).join("\n")

const buttons = PAIRS.map((p, i) => `<button data-pair="${p.id}"${i === 0 ? ' aria-pressed="true"' : ""}>${p.label}</button>`).join("")

const page = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>TCSV highlighting</title>
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
  .panes { display: grid; grid-template-columns: 1fr; gap: 10px; }
  .pane { min-width: 0; }
  .pane pre { margin: 0; padding: 14px 16px; border-radius: 10px; overflow-x: auto; font: 12.5px/1.6 ui-monospace, "SF Mono", Menlo, monospace; counter-reset: line; }
  .pane pre code { display: block; width: max-content; }
  .pane pre .line::before { counter-increment: line; content: counter(line); display: inline-block; width: 2.2ch; margin-right: 1.6ch; text-align: right; opacity: 0.35; }
${css}
</style>
</head>
<body data-pair="${PAIRS[0].id}">
<header>
  <h1>TCSV, highlighted</h1>
  <p>Each example in <code>tree-sitter/examples</code>, through the TextMate grammar (<code>syntaxes/tcsv.tmLanguage.json</code>) and Shiki, the engine GitHub and VS Code share. The themes know nothing of TCSV: every colour comes from a scope they already paint.</p>
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
