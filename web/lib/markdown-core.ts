// MARKDOWN AS A TREE, framework-free: the one parse every reader and writer of
// markdown here shares (the renderer, the editor's converter), so a file means the same
// thing wherever it is shown or edited. CommonMark with GitHub's extensions (tables,
// strikethrough, task lists, autolinks, footnotes) and front matter (YAML or TOML,
// kept as its own node, never read as a rule and a heading). The tree is mdast; every
// node keeps its source position, so a reader can always fall back to the exact text.

import type { Nodes, PhrasingContent, Root } from "mdast"
import { fromMarkdown } from "mdast-util-from-markdown"
import { frontmatterFromMarkdown } from "mdast-util-frontmatter"
import { gfmFromMarkdown } from "mdast-util-gfm"
import { frontmatter } from "micromark-extension-frontmatter"
import { gfm } from "micromark-extension-gfm"

/** The front matter fences understood. */
export const FRONTMATTER = ["yaml", "toml"] as const

/** The syntax every parse here reads: GitHub's extensions and front matter, as
 *  micromark's extensions and their tree builders (markdown-prepare adds tags to it). */
export const SYNTAX = {
  extensions: [gfm(), frontmatter([...FRONTMATTER])],
  mdastExtensions: [
    gfmFromMarkdown(),
    frontmatterFromMarkdown([...FRONTMATTER]),
  ],
}

/** A markdown text as its tree. */
export function parseMarkdown(text: string): Root {
  return fromMarkdown(text, SYNTAX)
}

/** A tree with every run of text outside code and links replaced by what `f` makes of
 *  its words (the words again, links, emphasis): the seam for autolinking a ticket
 *  number or a mention without ever reaching into code or an existing link. A new
 *  tree; the given one is untouched. */
export function mapText(
  tree: Root,
  f: (value: string) => PhrasingContent[],
): Root {
  const walk = <N extends Nodes>(node: N): N => {
    if (!("children" in node)) return node
    if (node.type === "link" || node.type === "linkReference") return node
    const children = (node.children as Nodes[]).flatMap((child): Nodes[] =>
      child.type === "text" ? f(child.value) : [walk(child)],
    )
    return { ...node, children } as N
  }
  return walk(tree)
}

/** The protocols a link may use: the web, mail, a phone, the page and the site. */
const SAFE_HREF =
  /^(?:https?:|mailto:|tel:|#|\/|\.{1,2}\/|[^:/?#]+(?:[/?#]|$))/i
/** The sources an image may come from: the web and the site, never a data or script
 *  URL. */
const SAFE_SRC = /^(?:https?:|\/|\.{1,2}\/|[^:/?#]+(?:[/?#]|$))/i

/** A link's target when it is safe to follow, else null: `javascript:`, `data:` and
 *  the rest never become a live link. */
export function safeHref(url: string): string | null {
  const u = url.trim()
  return SAFE_HREF.test(u) ? u : null
}

/** An image's source when it is safe to load, else null. */
export function safeSrc(url: string): string | null {
  const u = url.trim()
  return SAFE_SRC.test(u) ? u : null
}

/** A name for a file or a route: its words, lowercased, accents gone, joined by
 *  dashes. A heading's anchor is GitHub's (`anchorOf`). */
export function slugOf(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
}

/** What a code fence asks for after its language
 *  (```` ```ts showLineNumbers {2,4-6} ````): its lines numbered, and the lines to
 *  mark, in the words rehype-pretty-code, Docusaurus and MDX sites already write.
 *  Other words are other tools' (a `title="…"`), and text anyone wrote may carry
 *  them: they are passed over. */
export function fenceOf(meta: string | null | undefined): {
  lines: boolean
  highlight?: string
} {
  const words = (meta ?? "").split(/\s+/)
  const marked = words.find((w) => /^\{\d[\d,-]*\}$/.test(w))
  return {
    lines: words.includes("showLineNumbers"),
    highlight: marked?.slice(1, -1),
  }
}

/** A heading's anchor as GitHub writes it, so a link written against a file on GitHub
 *  lands on the same heading here: lowercased, every character that is no letter,
 *  digit, space, `-` or `_` dropped, each space a dash (`A column's formula` is
 *  `a-columns-formula`, `a — b` is `a--b`). */
export function anchorOf(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{M}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-")
}

/** Anchors for one document's headings, asked in their order (GitHub's, `anchorOf`): a
 *  title used twice takes `-1` the second time, so every anchor is the page's own. */
export function anchors(): (text: string) => string {
  const seen = new Map<string, number>()
  return (text) => {
    const base = anchorOf(text) || "section"
    const n = seen.get(base) ?? 0
    seen.set(base, n + 1)
    return n === 0 ? base : `${base}-${n}`
  }
}

/** A node's words, as text: inline runs joined, blocks set apart by a space. */
export function plainOf(node: Nodes): string {
  return "value" in node && typeof node.value === "string"
    ? node.value
    : "children" in node
      ? (node.children as Nodes[])
          .map(plainOf)
          .join(node.type === "paragraph" || node.type === "heading" ? "" : " ")
      : ""
}
