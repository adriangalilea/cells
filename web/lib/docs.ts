// THE DOCS, read from the repository's own markdown: README.md, SPEC.md and the rest are
// the only text, so a page can never say something its file does not. Worked out once
// at build (ui's markdown-prepare): each file parsed, the specification cut into a page
// per section (`sectionsOf`), every heading anchored as GitHub anchors it, and every link
// resolved where its file now lives: a document of these to its page (`SPEC.md#reading`
// to `/docs/spec/reading#reading`), any other file of the repository to it on GitHub.
// A link to a heading no document has, or to a file the repository lacks, throws, so
// the build fails before a reader ever clicks it. Server only (node:fs).

import { existsSync, readFileSync, statSync } from "node:fs"
import { join, normalize } from "node:path"
import type { Root } from "mdast"
import {
  type Heading,
  highlightTree,
  type MarkdownSection,
  type PreparedMarkdown,
  prepareMarkdown,
  sectionsOf,
} from "@/lib/markdown-prepare"
import { PRODUCT } from "@/lib/product"

/** The repository the site is built from: the folder above the site's. */
const REPO = normalize(join(process.cwd(), ".."))

/** A document of the docs: its file in the repository, where it lives on the site,
 *  what the sidebar calls it, and whether it is cut into a page per section. */
interface Doc {
  file: string
  path: string
  title: string
  split: boolean
}

/** Every document, in the sidebar's order. */
const DOCS: Doc[] = [
  { file: "README.md", path: "/docs", title: "Overview", split: false },
  { file: "SPEC.md", path: "/docs/spec", title: "Specification", split: true },
  { file: "EDITORS.md", path: "/docs/editors", title: "Editors", split: false },
  { file: "ACCOUNTING.md", path: "/docs/accounting", title: "Books in Cells", split: false },
  { file: "ENGINE.md", path: "/docs/engine", title: "The engine", split: false },
]

/** A page of the docs: where it is, its title and the anchor it is linked by, the tree
 *  it draws under its title, the headings its table of contents lists, its words (a
 *  search, a description), and the file it came from. */
export interface DocPage {
  path: string
  title: string
  id: string
  tree: Root
  headings: Heading[]
  text: string
  file: string
}

/** A document's pages, by where its sections lie: the whole file one page, or, cut, its
 *  title and introduction at the document's own path and each `##` at a path of its
 *  anchor. Each page's parts: its opening heading (its title), and what follows. */
function pagesOf(doc: Doc, prepared: PreparedMarkdown): { path: string; part: MarkdownSection }[] {
  const whole: MarkdownSection = {
    heading: prepared.headings[0] ?? null,
    tree: prepared.tree,
    headings: prepared.headings,
    text: prepared.text,
  }
  if (!doc.split) return [{ path: doc.path, part: whole }]
  return sectionsOf(prepared, 2).map((part) => ({
    path: part.heading && part.heading.depth > 1 ? `${doc.path}/${part.heading.id}` : doc.path,
    part,
  }))
}

/** Where each heading of each document lives: its anchor, by document file, to the
 *  page it is on. */
function anchorsOf(): Map<string, Map<string, string>> {
  const all = new Map<string, Map<string, string>>()
  for (const doc of DOCS) {
    const prepared = prepareMarkdown(readFileSync(join(REPO, doc.file), "utf8"))
    const on = new Map<string, string>()
    for (const { path, part } of pagesOf(doc, prepared)) for (const h of part.headings) on.set(h.id, path)
    all.set(doc.file, on)
  }
  return all
}

/** A link of `doc`'s, as a reader follows it on the site; null for one that is not the
 *  repository's (another site, a mail). Throws for a heading or a file that is not. */
function resolver(doc: Doc, anchors: Map<string, Map<string, string>>) {
  return (url: string): string | null => {
    if (/^[a-z][a-z+.-]*:/i.test(url)) return null
    const [target = "", hash = ""] = url.split("#")
    const file = target === "" ? doc.file : normalize(target).replace(/^\.\//, "")
    const on = anchors.get(file)
    if (on) {
      if (!hash) {
        const home = DOCS.find((d) => d.file === file) as Doc
        return home.path
      }
      const page = on.get(hash)
      if (!page) throw new Error(`docs: ${doc.file} links ${url}, and ${file} has no heading #${hash}`)
      return `${page}#${hash}`
    }
    const onDisk = join(REPO, file)
    if (!existsSync(onDisk)) throw new Error(`docs: ${doc.file} links ${url}, and the repository has no ${file}`)
    const kind = statSync(onDisk).isDirectory() ? "tree" : "blob"
    return `${PRODUCT.repo}/${kind}/main/${file}${hash ? `#${hash}` : ""}`
  }
}

/** A part's tree opening with the page's title as its `h1`, in its opening heading's
 *  place and with its anchor, so the sidebar and the page's title are one word and a
 *  link to the heading still lands on it. */
function titledTree(part: MarkdownSection, title: string, id: string): Root {
  const [first, ...rest] = part.tree.children
  const after = first?.type === "heading" && part.heading ? rest : part.tree.children
  return {
    ...part.tree,
    children: [{ type: "heading", depth: 1, children: [{ type: "text", value: title }], data: { id } }, ...after],
  }
}

/** A document's code blocks, each with the language its fence names; one that names
 *  none throws, since it would be drawn uncoloured and read by nothing. */
export function blocksOf(file: string, tree: Root): { lang: string; meta: string | null; value: string }[] {
  const blocks: { lang: string; meta: string | null; value: string }[] = []
  const walk = (node: Root | Root["children"][number]) => {
    if (node.type === "code") {
      if (!node.lang)
        throw new Error(`docs: ${file} line ${node.position?.start.line} opens a code block that names no language`)
      blocks.push({ lang: node.lang, meta: node.meta ?? null, value: node.value })
    }
    if ("children" in node) for (const child of node.children) walk(child as Root["children"][number])
  }
  walk(tree)
  return blocks
}

let built: Promise<DocPage[]> | null = null

/** Every page of the docs, each link resolved and each code block coloured ahead. */
export function docPages(): Promise<DocPage[]> {
  built ??= (async () => {
    const anchors = anchorsOf()
    const pages: DocPage[] = []
    for (const doc of DOCS) {
      const prepared = prepareMarkdown(readFileSync(join(REPO, doc.file), "utf8"), {
        resolve: resolver(doc, anchors),
      })
      blocksOf(doc.file, prepared.tree)
      for (const { path, part } of pagesOf(doc, prepared)) {
        // A document's own page is titled as the sidebar names it (its file's `#` is the
        // project's name); a section's page by its heading.
        const own = path === doc.path
        const title = own ? doc.title : (part.heading?.text ?? doc.title)
        const id = part.heading?.id ?? "top"
        pages.push({
          path,
          title,
          id,
          tree: await highlightTree(titledTree(part, title, id)),
          // The heading the page opens with is its title, not an entry of its table.
          headings: part.headings.filter((h) => h.id !== part.heading?.id),
          text: part.text,
          file: doc.file,
        })
      }
    }
    return pages
  })()
  return built
}

/** A page's file on GitHub, where it is edited. */
export const sourceOf = (page: DocPage) => `${PRODUCT.repo}/blob/main/${page.file}`

/** The pages before and after `page`, in the sidebar's order. */
export async function neighbours(page: DocPage) {
  const pages = await docPages()
  const at = pages.findIndex((p) => p.path === page.path)
  return { before: pages[at - 1], after: pages[at + 1] }
}

/** The sidebar: each document with its pages, in order. */
export async function docNav() {
  const pages = await docPages()
  return DOCS.map((doc) => ({
    title: doc.title,
    path: doc.path,
    pages: doc.split ? pages.filter((p) => p.file === doc.file && p.path !== doc.path) : [],
  }))
}

/** The page at `path`, or none. */
export async function docPage(path: string): Promise<DocPage | undefined> {
  return (await docPages()).find((p) => p.path === path)
}
