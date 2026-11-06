// MARKDOWN, PREPARED: what a page of markdown needs worked out once, where the files
// are (a build, a script), so the page only draws. `prepareMarkdown` parses the text
// with markdown-core's own syntax and returns the tree `Markdown` draws (`tree`), with
// what a page around it reads: its headings as a table of contents, its words as plain
// text (a description, a preview), its pictures measured. `highlightTree` then colours
// its code blocks as `code` does (the same highlighter and themes, so they are `code`'s
// blocks), and no highlighter loads in the browser.
//
// FACTS COME FROM WHOEVER HOLDS THEM. A picture's size is in its file, and only the
// host knows where its files are: it passes `measure`, which answers a local picture's
// size or null for one that is not its own (another site's), and THROWS for one of its
// own that is missing, the broken post a build should refuse.
//
// LINKS ARE CHECKED the same way. A link to a heading of the text (`#open`) must find
// one, or it throws; every other link goes through the host's `resolve`, which knows
// its files: a file of its own becomes its page, one that is missing throws. So a
// heading renamed, or a file moved, breaks the build, never a reader's click.
// `sectionsOf` cuts a prepared text into a page per heading, each keeping the text's
// anchors.
//
// TAGS ARE DATA. A text that names components (`<Bars unit="s" data={[…]} />`) says
// which with `tags`; a tag not among them throws, and so does a prop that is not a
// literal (a string, a number, true, false, null, an array or an object of them): a
// prop is read here, never run. The props land on the node (`data.props`) as JSON, so
// a prepared tree can be kept on disk and drawn later. With tags, raw HTML and `<url>`
// autolinks are not markdown (as in MDX): a `<` opens a tag.
//
// Every node keeps its source position. Framework-free: no react, no DOM.

import type { Expression, Program } from "acorn"
import * as acorn from "acorn"
import type * as M from "mdast"
import { fromMarkdown } from "mdast-util-from-markdown"
import {
  type MdxJsxFlowElement,
  type MdxJsxTextElement,
  mdxJsxFromMarkdown,
} from "mdast-util-mdx-jsx"
import { mdxJsx } from "micromark-extension-mdx-jsx"
import {
  codeHtml,
  knownLanguage,
} from "@/lib/code-highlight"
import {
  anchors,
  fenceOf,
  plainOf,
  SYNTAX,
} from "@/lib/markdown-core"

/** A heading, as a table of contents lists it: its depth, its words, its anchor. */
export type Heading = { depth: number; text: string; id: string }

/** A picture's size in pixels. */
export type Size = { width: number; height: number }

export type PreparedMarkdown = {
  /** The tree `Markdown` draws (`<Markdown tree={…} />`): headings carry their
   *  anchor (`data.id`), measured pictures their size (`data.width`, `data.height`),
   *  tags their props (`data.props`). */
  tree: M.Root
  /** Every heading, in order. */
  headings: Heading[]
  /** The words, markup gone: a description, a preview, a search. */
  text: string
  /** The pictures it shows that were measured, in order. */
  images: (Size & { src: string })[]
  /** How many times each tag is used, by name. */
  tags: Record<string, number>
  /** Every link and link definition, as written and as a reader follows it. */
  links: { url: string; href: string }[]
}

export type PrepareOptions = {
  /** A picture's size by its source: null for one that is not the host's (it is
   *  drawn unmeasured); throws for one of the host's that is missing. */
  measure?: (src: string) => Size | null
  /** The components the text may name as tags. None: the text is plain markdown. */
  tags?: readonly string[]
  /** A link's target as the host holds its files: the URL a reader follows (a file
   *  of its own as its page, `SPEC.md#open` as `/docs/spec/open`), null for one that
   *  is not the host's (it stays as written); throws for one of its own that is
   *  missing, the broken link a build should refuse. A link to a heading of the text
   *  itself (`#open`) is checked first, and one to no heading throws. */
  resolve?: (url: string) => string | null
}

/** A node prepare annotates: its `data` carries what was worked out. */
type Annotated = { data?: Record<string, unknown> }

/** The text, prepared: parsed once, its headings anchored, its pictures measured,
 *  its tags' props read. */
export function prepareMarkdown(
  source: string,
  { measure, tags, resolve }: PrepareOptions = {},
): PreparedMarkdown {
  const allowed = new Set(tags ?? [])
  const tree = fromMarkdown(source, {
    extensions: [
      ...SYNTAX.extensions,
      ...(tags
        ? [
            mdxJsx({ acorn, addResult: true }),
            // A `<` opens a tag: raw HTML and `<url>` would read the same text
            // another way.
            { disable: { null: ["htmlFlow", "htmlText", "autolink"] } },
          ]
        : []),
    ],
    mdastExtensions: [
      ...SYNTAX.mdastExtensions,
      ...(tags ? [mdxJsxFromMarkdown()] : []),
    ],
  })
  const anchor = anchors()
  const headings: Heading[] = []
  const images: PreparedMarkdown["images"] = []
  const used: Record<string, number> = {}
  const walk = (node: M.Nodes) => {
    if (node.type === "heading") {
      const text = plainOf(node)
      const id = anchor(text)
      headings.push({ depth: node.depth, text, id })
      annotate(node, { id })
    }
    if (node.type === "image" && measure) {
      const size = measure(node.url)
      if (size) {
        images.push({ src: node.url, ...size })
        annotate(node, { ...size })
      }
    }
    if (
      node.type === "mdxJsxFlowElement" ||
      node.type === "mdxJsxTextElement"
    ) {
      const name = node.name
      if (!name || !allowed.has(name))
        throw new Error(
          `markdown-prepare: <${name ?? ""}> is not a tag this text may use (${[...allowed].join(", ") || "none"})`,
        )
      used[name] = (used[name] ?? 0) + 1
      annotate(node, { props: propsOf(node) })
    }
    if ("children" in node) for (const child of node.children) walk(child)
  }
  walk(tree)
  // Links once every heading is known: one may point at a heading below it.
  const own = new Set(headings.map((h) => h.id))
  const links: PreparedMarkdown["links"] = []
  const follow = (node: M.Nodes) => {
    if (node.type === "link" || node.type === "definition") {
      const url = node.url
      if (url.startsWith("#") && !own.has(url.slice(1)))
        throw new Error(
          `markdown-prepare: [${plainOf(node) || url}](${url}) points at no heading of this text`,
        )
      const href = resolve?.(url) ?? url
      links.push({ url, href })
      if (href !== url) {
        node.url = href
        annotate(node, { written: url })
      }
    }
    if ("children" in node) for (const child of node.children) follow(child)
  }
  follow(tree)
  return {
    tree,
    headings,
    text: plainOf(tree).trim(),
    images,
    tags: used,
    links,
  }
}

/** A part of a text, cut at a heading: the heading it opens with (none for what comes
 *  before the first), the tree `Markdown` draws, its headings and its words. */
export type MarkdownSection = {
  heading: Heading | null
  tree: M.Root
  headings: Heading[]
  text: string
}

/** A prepared text cut into parts at its headings of `depth` or shallower (2: a part
 *  for each `##`), each part keeping the text's anchors, so a link from one part to
 *  another lands where its heading now is (the host's `resolve` says on which page);
 *  and each carrying the link definitions the whole text has and the footnotes it
 *  refers to, so its links resolve and its notes gather at its own foot. */
export function sectionsOf(
  prepared: PreparedMarkdown,
  depth: number,
): MarkdownSection[] {
  const definitions: M.RootContent[] = []
  const notes = new Map<string, M.FootnoteDefinition>()
  const parts: M.RootContent[][] = [[]]
  for (const node of prepared.tree.children) {
    if (node.type === "definition") definitions.push(node)
    else if (node.type === "footnoteDefinition")
      notes.set(node.identifier, node)
    else {
      if (node.type === "heading" && node.depth <= depth) parts.push([])
      ;(parts.at(-1) as M.RootContent[]).push(node)
    }
  }
  return parts
    .filter((children) => children.length > 0)
    .map((children) => {
      const cited = new Set<string>()
      const found: Heading[] = []
      const walk = (node: M.Nodes) => {
        if (node.type === "footnoteReference") cited.add(node.identifier)
        if (node.type === "heading")
          found.push({
            depth: node.depth,
            text: plainOf(node),
            id: (node.data as { id?: string } | undefined)?.id as string,
          })
        if ("children" in node) for (const child of node.children) walk(child)
      }
      for (const child of children) walk(child)
      const first = children[0]
      const root: M.Root = { type: "root", children }
      return {
        heading:
          first?.type === "heading" && first.depth <= depth
            ? (found[0] as Heading)
            : null,
        tree: {
          type: "root",
          children: [
            ...children,
            ...definitions,
            ...[...cited].flatMap((id) => notes.get(id) ?? []),
          ],
        },
        headings: found,
        text: plainOf(root).trim(),
      }
    })
}

function annotate(node: M.Nodes | MdxJsxFlowElement, data: object) {
  const n = node as Annotated
  n.data = { ...n.data, ...data }
}

/** A tag's props, read as data: a bare name is `true`, a quoted value its string,
 *  `{…}` a literal. Anything else throws, naming the tag and the prop. */
function propsOf(
  node: MdxJsxFlowElement | MdxJsxTextElement,
): Record<string, unknown> {
  const props: Record<string, unknown> = {}
  for (const attribute of node.attributes) {
    if (attribute.type === "mdxJsxExpressionAttribute")
      throw new Error(
        `markdown-prepare: <${node.name} {${attribute.value}}>: a spread is code, a prop is data`,
      )
    const { name, value } = attribute
    if (value === null || value === undefined) props[name] = true
    else if (typeof value === "string") props[name] = value
    else {
      // The tree builder types it as ESTree's Program; acorn made it, by acorn's.
      const program = value.data?.estree as unknown as Program | undefined
      const statement = program?.body[0]
      if (statement?.type !== "ExpressionStatement")
        throw new Error(
          `markdown-prepare: <${node.name} ${name}={${value.value}}> holds no value`,
        )
      props[name] = literalOf(statement.expression, `<${node.name} ${name}>`)
      // Read: its syntax tree goes, so the prepared tree stays plain JSON (the
      // value's source text stays, `value.value`).
      delete value.data
    }
  }
  return props
}

/** A JavaScript literal's value, never run: a string, a number (signed), true, false,
 *  null, a template with no holes, an array or an object of them. */
function literalOf(node: Expression, where: string): unknown {
  switch (node.type) {
    case "Literal":
      if ("regex" in node || "bigint" in node)
        throw new Error(
          `markdown-prepare: ${where} holds a ${node.raw}, not data`,
        )
      return node.value
    case "UnaryExpression":
      if (
        (node.operator === "-" || node.operator === "+") &&
        node.argument.type === "Literal" &&
        typeof node.argument.value === "number"
      )
        return node.operator === "-"
          ? -node.argument.value
          : node.argument.value
      break
    case "TemplateLiteral":
      if (node.expressions.length === 0)
        return node.quasis.map((q) => q.value.cooked).join("")
      break
    case "ArrayExpression":
      return node.elements.map((element) => {
        if (!element || element.type === "SpreadElement")
          throw new Error(`markdown-prepare: ${where} has a hole or a spread`)
        return literalOf(element, where)
      })
    case "ObjectExpression":
      return Object.fromEntries(
        node.properties.map((property) => {
          if (
            property.type !== "Property" ||
            property.computed ||
            property.kind !== "init" ||
            property.method
          )
            throw new Error(
              `markdown-prepare: ${where} has a key that is not a plain name`,
            )
          const key =
            property.key.type === "Identifier"
              ? property.key.name
              : property.key.type === "Literal"
                ? String(property.key.value)
                : null
          if (key === null)
            throw new Error(`markdown-prepare: ${where} has a computed key`)
          return [key, literalOf(property.value as Expression, where)]
        }),
      )
  }
  throw new Error(
    `markdown-prepare: ${where} holds ${node.type}, which is code: a prop is a literal`,
  )
}

/** The tree with every code block coloured ahead (`data.html`), by `code`'s own
 *  highlighter (lib/code-highlight), its fence's marked lines marked: what the
 *  browser would colour, done here. A block in a language Shiki does not know stays
 *  plain. A new tree; the given one is untouched. */
export async function highlightTree(tree: M.Root): Promise<M.Root> {
  const copy = structuredClone(tree)
  const blocks: M.Code[] = []
  const walk = (node: M.Nodes) => {
    if (node.type === "code") blocks.push(node)
    if ("children" in node) for (const child of node.children) walk(child)
  }
  walk(copy)
  await Promise.all(
    blocks.map(async (block) => {
      const lang = block.lang ?? "text"
      if (!(await knownLanguage(lang))) return
      const { highlight } = fenceOf(block.meta)
      annotate(block, {
        html: await codeHtml(block.value, { lang, highlight }),
      })
    }),
  )
  return copy
}
