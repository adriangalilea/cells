// MARKDOWN, READ: a markdown text drawn as a page. The parse is lib/markdown-core's (the
// same one the editor uses), the tree is drawn as plain tags, and the look is one set
// of rules on those tags (`MARKDOWN`), so anything that draws the same tags wears it:
// this renderer, and the editor's document, which then reads as it will be read.
//
// IT RENDERS ANYWHERE: a server component (the text parsed there, no parser shipped
// to the browser, `components` and `transform` free to be server functions) or inside
// a client one. Only a code block is the browser's (markdown-code's `Highlighted`).
//
// THE LOOK (Adrian's site, adriangalilea.com, is the reference): body text a step
// below the headings; h1 to h3 in the serif voice, regular weight, tight tracking, the
// h2 over a hairline; links underlined by a hairline of their own colour that turns to
// a highlighter's stroke under the hand; inline code on a quiet chip; code blocks in
// code's frame, highlighted in the browser with code's themes; quotes in the quote
// voice; tables with small uppercase heads and hairline rows; rules with air around
// them. Every size is in em, so a body set small (a file's face in a gallery) is the
// same page made small, never a page reflowed.
//
// SAFE BY CONSTRUCTION. Nothing is set as HTML except Shiki's own escaped markup of a
// code block: raw HTML in the text is shown as text, front matter and link definitions
// are not shown. A link is live only to the web, mail, a phone, the page or the site,
// an image loads only from the web or the site (`safeHref`, `safeSrc`): a
// `javascript:` or `data:` URL is its words, never a link. So text anyone wrote is safe
// to draw. Reference links resolve through their definitions; footnotes gather at the
// foot.
//
// IT COMPOSES. `components` replaces how a tag is drawn (a link through a router, an
// image through the image item or only from the host's CDN, inline code as keys when
// it spells a shortcut), the way MDX's components do; `transform` changes the tree
// before it is drawn (`mapText` autolinks a ticket number or a mention, never inside
// code or a link). The look is overridden where it is worn, three ways, coarse to
// fine: `--ag-markdown-link` gives links a colour (a site's accent) without touching
// anything else; `className` overrides any rule, a later class of the same kind
// winning (`text-sm`, `[&_p]:my-0`, `[&_h2]:border-0`); and `MARKDOWN` is exported for
// a host that draws the tags itself (the editor).

import { Check } from "lucide-react"
import type * as M from "mdast"
import * as React from "react"
import { cn } from "@/lib/utils"
import {
  anchors,
  fenceOf,
  parseMarkdown,
  plainOf,
  safeHref,
  safeSrc,
} from "@/lib/markdown-core"
import { youtubeOf } from "@/lib/youtube-data"
import { Image } from "@/components/ui/image"
import {
  Highlighted,
  type HighlightedProps,
} from "@/components/ui/markdown-code"
import { Quote, type QuoteProps } from "@/components/ui/quote"
import { QuoteMarkMask } from "@/components/ui/quote-mark"
import { YouTube } from "@/components/ui/youtube"

/** The look of a markdown body, on plain tags, in em. Whoever draws the tags wears it
 *  on their container. One colour throughout, the page's own, at the alpha ladder's
 *  tiers: titles at 90, the words at 70, what is quoted at 80, what is struck or
 *  annotated at 40 to 50, every line a hairline (5 to 8) and every surface a wash (3 to
 *  6). Nothing is tinted; a link is told by its hairline, not a hue.
 *
 *  EVERY RULE STOPS AT A THING THAT DRAWS ITSELF: an embed (`[data-embed]`: a post, a
 *  video) and a quote's attribution are drawn as on their own pages, so each tag rule
 *  carries `:not(:is([data-embed],[data-slot=quote-by]) *)`. Without it a post's avatar
 *  took the page's full-width image rule and its name the link's hairline. */
export const MARKDOWN = [
  "text-foreground/70 leading-[1.7] [overflow-wrap:anywhere]",
  // The first and last blocks sit flush with the body's edges, whatever they are:
  // important, since each tag's own margin below is the more specific rule.
  "[&>:first-child]:mt-0! [&>:last-child]:mb-0!",
  "[&_p:not([data-slot=quote-body]_p):not(:is([data-embed],[data-slot=quote-by])_*)]:my-[1em]",
  // headings: the serif voice for the three that title, the sans for the rest
  "[&_:is(h1,h2,h3,h4,h5,h6):not(:is([data-embed],[data-slot=quote-by])_*)]:text-foreground/90 [&_:is(h1,h2,h3,h4,h5,h6):not(:is([data-embed],[data-slot=quote-by])_*)]:text-balance [&_:is(h1,h2,h3,h4,h5,h6):not(:is([data-embed],[data-slot=quote-by])_*)]:scroll-mt-20",
  "[&_:is(h1,h2,h3):not(:is([data-embed],[data-slot=quote-by])_*)]:font-serif [&_:is(h1,h2,h3):not(:is([data-embed],[data-slot=quote-by])_*)]:font-normal",
  "[&_h1:not(:is([data-embed],[data-slot=quote-by])_*)]:mt-[1.5em] [&_h1:not(:is([data-embed],[data-slot=quote-by])_*)]:mb-[0.6em] [&_h1:not(:is([data-embed],[data-slot=quote-by])_*)]:text-[2.25em] [&_h1:not(:is([data-embed],[data-slot=quote-by])_*)]:leading-[1.15] [&_h1:not(:is([data-embed],[data-slot=quote-by])_*)]:tracking-[-0.03em]",
  "[&_h2:not(:is([data-embed],[data-slot=quote-by])_*)]:mt-[1.7em] [&_h2:not(:is([data-embed],[data-slot=quote-by])_*)]:mb-[0.7em] [&_h2:not(:is([data-embed],[data-slot=quote-by])_*)]:border-b [&_h2:not(:is([data-embed],[data-slot=quote-by])_*)]:border-foreground/8 [&_h2:not(:is([data-embed],[data-slot=quote-by])_*)]:pb-[0.4em] [&_h2:not(:is([data-embed],[data-slot=quote-by])_*)]:text-[1.75em] [&_h2:not(:is([data-embed],[data-slot=quote-by])_*)]:leading-[1.2] [&_h2:not(:is([data-embed],[data-slot=quote-by])_*)]:tracking-[-0.03em]",
  "[&_h3:not(:is([data-embed],[data-slot=quote-by])_*)]:mt-[1.6em] [&_h3:not(:is([data-embed],[data-slot=quote-by])_*)]:mb-[0.55em] [&_h3:not(:is([data-embed],[data-slot=quote-by])_*)]:text-[1.375em] [&_h3:not(:is([data-embed],[data-slot=quote-by])_*)]:leading-[1.25] [&_h3:not(:is([data-embed],[data-slot=quote-by])_*)]:tracking-[-0.02em]",
  "[&_:is(h4,h5,h6):not(:is([data-embed],[data-slot=quote-by])_*)]:mt-[1.75em] [&_:is(h4,h5,h6):not(:is([data-embed],[data-slot=quote-by])_*)]:mb-[0.5em] [&_:is(h4,h5,h6):not(:is([data-embed],[data-slot=quote-by])_*)]:font-semibold [&_:is(h4,h5,h6):not(:is([data-embed],[data-slot=quote-by])_*)]:tracking-[-0.01em]",
  // words
  "[&_strong:not(:is([data-embed],[data-slot=quote-by])_*)]:font-semibold [&_strong:not(:is([data-embed],[data-slot=quote-by])_*)]:text-foreground/90 [&_em:not(:is([data-embed],[data-slot=quote-by])_*)]:italic [&_:is(s,del):not(:is([data-embed],[data-slot=quote-by])_*)]:text-foreground/40",
  // links: a hairline at a third of their colour, a highlighter's wash under the hand.
  // Their colour is `--ag-markdown-link` when set (a site's accent: the hairline and
  // the wash follow it), else the page's own at 90. A link that holds a picture (a
  // figure's lightbox, a badge) is the picture's, and wears none of it.
  // a heading's link to itself (linkHeadings): faint, after its words, seen under the
  // hand or on focus, and none of a link's underline
  "[&_[data-slot=markdown-anchor]]:ml-[0.35em] [&_[data-slot=markdown-anchor]]:font-sans [&_[data-slot=markdown-anchor]]:text-foreground/30! [&_[data-slot=markdown-anchor]]:shadow-none! [&_[data-slot=markdown-anchor]]:opacity-0 [&_[data-slot=markdown-anchor]]:transition-opacity [&_:is(h1,h2,h3,h4,h5,h6):hover_[data-slot=markdown-anchor]]:opacity-100 [&_[data-slot=markdown-anchor]:focus-visible]:opacity-100 [&_[data-slot=markdown-anchor]:hover]:text-foreground/70!",
  "[&_a:not(:is([data-embed],[data-slot=quote-by])_*):not(:has(img))]:text-[var(--ag-markdown-link,color-mix(in_oklab,var(--color-foreground)_90%,transparent))] [&_a:not(:is([data-embed],[data-slot=quote-by])_*):not(:has(img))]:no-underline [&_a:not(:is([data-embed],[data-slot=quote-by])_*):not(:has(img))]:shadow-[inset_0_-1px_0_0_color-mix(in_oklab,currentColor_35%,transparent)] [&_a:not(:is([data-embed],[data-slot=quote-by])_*):not(:has(img))]:transition-shadow [&_a:not(:is([data-embed],[data-slot=quote-by])_*):not(:has(img))]:duration-150 [&_a:not(:is([data-embed],[data-slot=quote-by])_*):not(:has(img)):hover]:shadow-[inset_0_-0.5em_0_0_color-mix(in_oklab,currentColor_12%,transparent)]",
  // lists, tight: an item's own paragraph (the editor wraps items in one) adds nothing
  "[&_ul:not(:is([data-embed],[data-slot=quote-by])_*)]:list-disc [&_ol:not(:is([data-embed],[data-slot=quote-by])_*)]:list-decimal [&_:is(ul,ol):not(:is([data-embed],[data-slot=quote-by])_*)]:my-[1em] [&_:is(ul,ol):not(:is([data-embed],[data-slot=quote-by])_*)]:pl-[1.5em] [&_:is(ul,ol)_:is(ul,ol):not(:is([data-embed],[data-slot=quote-by])_*)]:my-[0.25em]",
  "[&_li:not(:is([data-embed],[data-slot=quote-by])_*)]:my-[0.3em] [&_li:not(:is([data-embed],[data-slot=quote-by])_*)]:pl-[0.25em] [&_li>p:not(:is([data-embed],[data-slot=quote-by])_*)]:my-0 [&_li:not(:is([data-embed],[data-slot=quote-by])_*)::marker]:text-foreground/30",
  "[&_li[data-task]:not(:is([data-embed],[data-slot=quote-by])_*)]:-ml-[1.25em] [&_li[data-task]:not(:is([data-embed],[data-slot=quote-by])_*)]:flex [&_li[data-task]:not(:is([data-embed],[data-slot=quote-by])_*)]:list-none [&_li[data-task]:not(:is([data-embed],[data-slot=quote-by])_*)]:gap-[0.6em]",
  // code: a chip inline; a block in code's frame (renderer) or bare (editor), both a
  // hairline round a wash. edge: the bare block is the editor's own <pre>, framed
  // on itself, which a fade would fade with its frame; the renderer's blocks scroll
  // inside code's frame, fading (code.css).
  "[&_:not(pre)>code:not(:is([data-embed],[data-slot=quote-by])_*)]:rounded-[0.375em] [&_:not(pre)>code:not(:is([data-embed],[data-slot=quote-by])_*)]:bg-fill-rest [&_:not(pre)>code:not(:is([data-embed],[data-slot=quote-by])_*)]:px-[0.4em] [&_:not(pre)>code:not(:is([data-embed],[data-slot=quote-by])_*)]:py-[0.15em] [&_:not(pre)>code:not(:is([data-embed],[data-slot=quote-by])_*)]:font-mono [&_:not(pre)>code:not(:is([data-embed],[data-slot=quote-by])_*)]:text-[0.85em] [&_:not(pre)>code:not(:is([data-embed],[data-slot=quote-by])_*)]:text-foreground/85",
  "[&_.ag-code:not(:is([data-embed],[data-slot=quote-by])_*)]:my-[1.5em] [&_.ag-code:not(:is([data-embed],[data-slot=quote-by])_*)]:rounded-[0.75em] [&_.ag-code:not(:is([data-embed],[data-slot=quote-by])_*)]:border-foreground/8 [&_.ag-code:not(:is([data-embed],[data-slot=quote-by])_*)]:bg-fill-group [&_.ag-code_pre:not(:is([data-embed],[data-slot=quote-by])_*)]:px-[1.1em] [&_.ag-code_pre:not(:is([data-embed],[data-slot=quote-by])_*)]:py-[1em] [&_.ag-code_pre:not(:is([data-embed],[data-slot=quote-by])_*)]:text-[0.8125em]",
  "[&_:not(.ag-code-scroll)>pre:not(:is([data-embed],[data-slot=quote-by])_*)]:my-[1.5em] [&_:not(.ag-code-scroll)>pre:not(:is([data-embed],[data-slot=quote-by])_*)]:overflow-x-auto [&_:not(.ag-code-scroll)>pre:not(:is([data-embed],[data-slot=quote-by])_*)]:rounded-[0.75em] [&_:not(.ag-code-scroll)>pre:not(:is([data-embed],[data-slot=quote-by])_*)]:border [&_:not(.ag-code-scroll)>pre:not(:is([data-embed],[data-slot=quote-by])_*)]:border-foreground/8 [&_:not(.ag-code-scroll)>pre:not(:is([data-embed],[data-slot=quote-by])_*)]:bg-fill-group [&_:not(.ag-code-scroll)>pre:not(:is([data-embed],[data-slot=quote-by])_*)]:px-[1.1em] [&_:not(.ag-code-scroll)>pre:not(:is([data-embed],[data-slot=quote-by])_*)]:py-[1em] [&_:not(.ag-code-scroll)>pre:not(:is([data-embed],[data-slot=quote-by])_*)]:font-mono [&_:not(.ag-code-scroll)>pre:not(:is([data-embed],[data-slot=quote-by])_*)]:text-[0.8125em] [&_:not(.ag-code-scroll)>pre:not(:is([data-embed],[data-slot=quote-by])_*)]:leading-[1.65] [&_:not(.ag-code-scroll)>pre:not(:is([data-embed],[data-slot=quote-by])_*)]:text-foreground/80",
  // source kept as it came (the editor's verbatim block): set apart and quiet, read as
  // the file's own text, not the page's
  "[&_:not(.ag-code-scroll)>pre[data-language=verbatim]]:border-dashed [&_:not(.ag-code-scroll)>pre[data-language=verbatim]]:bg-transparent [&_:not(.ag-code-scroll)>pre[data-language=verbatim]]:py-[0.6em] [&_:not(.ag-code-scroll)>pre[data-language=verbatim]]:text-[0.75em] [&_:not(.ag-code-scroll)>pre[data-language=verbatim]]:text-foreground/40",
  // quotes: the quote item's prose weight. The renderer draws `Quote` itself (its words,
  // its attribution); a bare blockquote (the editor's) is drawn the same way here: one
  // step up on a surface, the quote voice, the quote's own mark in the gutter
  // (`--ag-quote-mark-mask`, set once per page by quote's QuoteMarkMask, which whoever
  // wears this look renders)
  // an embed: set apart, and given back the page's own ink and leading
  // (a quote or an embed of the body's own: one inside an embed is the embed's to place)
  "[&_[data-slot=quote]:not([data-embed]_*)]:my-[1.5em] [&_[data-embed]:not([data-embed]_*)]:my-[1.5em] [&_[data-embed]:not([data-embed]_*)]:text-foreground [&_[data-embed]:not([data-embed]_*)]:leading-normal [&_[data-embed]:not([data-embed]_*)]:[overflow-wrap:normal]",
  "[&_blockquote:not([data-slot=quote-body])]:relative [&_blockquote:not([data-slot=quote-body])]:my-[1.5em] [&_blockquote:not([data-slot=quote-body])]:rounded-xl [&_blockquote:not([data-slot=quote-body])]:bg-fill-group [&_blockquote:not([data-slot=quote-body])]:py-[0.889em] [&_blockquote:not([data-slot=quote-body])]:pr-[1.111em] [&_blockquote:not([data-slot=quote-body])]:pl-[2em] [&_blockquote:not([data-slot=quote-body])]:font-[family-name:var(--font-quote)] [&_blockquote:not([data-slot=quote-body])]:text-[1.125em] [&_blockquote:not([data-slot=quote-body])]:text-foreground/92",
  "[&_blockquote:not([data-slot=quote-body])]:before:absolute [&_blockquote:not([data-slot=quote-body])]:before:top-[1.089em] [&_blockquote:not([data-slot=quote-body])]:before:left-[0.8em] [&_blockquote:not([data-slot=quote-body])]:before:h-[0.95em] [&_blockquote:not([data-slot=quote-body])]:before:w-[1.083em] [&_blockquote:not([data-slot=quote-body])]:before:bg-current [&_blockquote:not([data-slot=quote-body])]:before:opacity-30 [&_blockquote:not([data-slot=quote-body])]:before:content-[''] [&_blockquote:not([data-slot=quote-body])]:before:[mask:var(--ag-quote-mark-mask)_center/contain_no-repeat]",
  "[&_blockquote_p]:my-0 [&_blockquote_p+p]:mt-[0.5em]",
  // rules
  "[&_hr:not(:is([data-embed],[data-slot=quote-by])_*)]:my-[3em] [&_hr:not(:is([data-embed],[data-slot=quote-by])_*)]:border-0 [&_hr:not(:is([data-embed],[data-slot=quote-by])_*)]:border-t [&_hr:not(:is([data-embed],[data-slot=quote-by])_*)]:border-foreground/8",
  // tables: no grid, a hairline under the head and between rows
  "[&_table:not(:is([data-embed],[data-slot=quote-by])_*)]:my-[1.5em] [&_table:not(:is([data-embed],[data-slot=quote-by])_*)]:w-full [&_table:not(:is([data-embed],[data-slot=quote-by])_*)]:border-collapse [&_table:not(:is([data-embed],[data-slot=quote-by])_*)]:text-[0.875em] [&_table:not(:is([data-embed],[data-slot=quote-by])_*)]:leading-[1.5]",
  "[&_th:not(:is([data-embed],[data-slot=quote-by])_*)]:border-b [&_th:not(:is([data-embed],[data-slot=quote-by])_*)]:border-foreground/8 [&_th:not(:is([data-embed],[data-slot=quote-by])_*)]:px-[1em] [&_th:not(:is([data-embed],[data-slot=quote-by])_*)]:py-[0.75em] [&_th:not(:is([data-embed],[data-slot=quote-by])_*)]:text-left [&_th:not(:is([data-embed],[data-slot=quote-by])_*)]:text-[0.75em] [&_th:not(:is([data-embed],[data-slot=quote-by])_*)]:font-medium [&_th:not(:is([data-embed],[data-slot=quote-by])_*)]:uppercase [&_th:not(:is([data-embed],[data-slot=quote-by])_*)]:tracking-[0.05em] [&_th:not(:is([data-embed],[data-slot=quote-by])_*)]:text-foreground/50",
  "[&_td:not(:is([data-embed],[data-slot=quote-by])_*)]:border-b [&_td:not(:is([data-embed],[data-slot=quote-by])_*)]:border-foreground/5 [&_td:not(:is([data-embed],[data-slot=quote-by])_*)]:px-[1em] [&_td:not(:is([data-embed],[data-slot=quote-by])_*)]:py-[0.75em] [&_td:not(:is([data-embed],[data-slot=quote-by])_*)]:align-top [&_tr:last-child>td:not(:is([data-embed],[data-slot=quote-by])_*)]:border-b-0",
  // images and the foot
  "[&_img:not(:is([data-embed],[data-slot=quote-by])_*)]:my-[2em] [&_img:not(:is([data-embed],[data-slot=quote-by])_*)]:h-auto [&_img:not(:is([data-embed],[data-slot=quote-by])_*)]:w-full [&_img:not(:is([data-embed],[data-slot=quote-by])_*)]:rounded-[0.75em]",
  // a figure: the picture flush in it, its caption small and centred under it (a
  // picture's figure, never a quote's, which is a figure with a slot of its own)
  "[&_figure:not([data-slot]):not(:is([data-embed],[data-slot=quote-by])_*)]:my-[2em] [&_figure:not([data-slot])_img:not(:is([data-embed],[data-slot=quote-by])_*)]:my-0 [&_figcaption:not([data-slot]):not(:is([data-embed],[data-slot=quote-by])_*)]:mt-[0.75em] [&_figcaption:not([data-slot]):not(:is([data-embed],[data-slot=quote-by])_*)]:text-center [&_figcaption:not([data-slot]):not(:is([data-embed],[data-slot=quote-by])_*)]:text-[0.8125em] [&_figcaption:not([data-slot]):not(:is([data-embed],[data-slot=quote-by])_*)]:text-foreground/50",
  "[&_[data-footnotes]]:mt-[3em] [&_[data-footnotes]]:border-t [&_[data-footnotes]]:border-foreground/8 [&_[data-footnotes]]:pt-[1em] [&_[data-footnotes]]:text-[0.875em] [&_[data-footnotes]]:text-foreground/50",
].join(" ")

/** A picture as the text gives it: its checked source, its words, its title, and its
 *  size where it was measured (markdown-prepare). */
export type MarkdownImage = {
  src: string
  alt: string
  title?: string
  width?: number
  height?: number
}

type Tags = {
  a: React.ComponentType<React.ComponentProps<"a">>
  img: React.ComponentType<MarkdownImage>
  /** A picture alone on its line, and the caption in italics under it, if any: drawn
   *  as a figure. `children` is the picture as `img` draws it. */
  figure: React.ComponentType<{
    image: MarkdownImage
    caption?: React.ReactNode
    children: React.ReactNode
  }>
  /** Inline code, given its text. */
  code: React.ComponentType<{ children: string }>
  /** A code block: its text, its language, Shiki's markup of it where it was
   *  coloured ahead (markdown-prepare `highlightTree`), else null, and what its fence
   *  asks for (```` ```ts showLineNumbers {2,4-6} ````). By default `code`'s own block. */
  pre: React.ComponentType<HighlightedProps>
  /** A quotation: its words drawn, who said it when the last line names them. */
  blockquote: React.ComponentType<QuoteProps>
}

export type MarkdownProps = (
  | {
      /** The markdown text, parsed as it is drawn. */
      children: string
      tree?: never
    }
  | {
      /** A tree prepared ahead (markdown-prepare `prepareMarkdown`): parsed once,
       *  its headings anchored, its pictures measured, its tags' props read. */
      tree: M.Root
      children?: never
    }
) & {
  /** How a link, an image, a figure, code or a quotation is drawn, when not as the
   *  registry draws them. Links and images receive URLs already checked safe; a host
   *  narrows further (images only from its CDN). */
  components?: Partial<Tags>
  /** The components a prepared text names as tags (`<Bars data={…} />`), by name:
   *  each is given the tag's props, and what the tag holds as its children. A tag
   *  with none here throws. */
  elements?: Record<string, React.ElementType>
  /** A link alone on its line, as the thing it points to: return what to draw for a
   *  URL (a post on X from its facts, a Telegram post), `undefined` to leave it to the
   *  defaults, or `null` to keep it a link (`() => null`: nothing embedded, for text
   *  that must load nothing from another host). A YouTube link is a `YouTube` by
   *  default; any other stays a link. */
  embeds?: (url: string) => React.ReactNode
  /** The tree, changed before it is drawn: autolinking a ticket number or a mention
   *  (`mapText`), dropping a section. A pure function, the given tree untouched. */
  transform?: (tree: M.Root) => M.Root
  /** Each heading a link to itself, shown under the hand or on focus: a page others
   *  link into (docs, a long post). Off by default, a tab stop a heading. */
  linkHeadings?: boolean
  className?: string
}

/** A tag in a prepared tree (markdown-prepare): its name, its props read as data,
 *  what it holds. Typed here, so drawing one needs no MDX package. */
type TagNode = {
  type: "mdxJsxFlowElement" | "mdxJsxTextElement"
  name: string | null
  data?: { props?: Record<string, unknown> }
  children: M.Nodes[]
}

/** What prepare measured or worked out on a node, if anything. */
const dataOf = (node: M.Nodes) =>
  (node.data ?? {}) as {
    id?: string
    width?: number
    height?: number
    html?: string
  }

/** The embed the registry draws by itself: a YouTube link as its video. */
const builtInEmbed = (url: string): React.ReactNode =>
  youtubeOf(url) ? <YouTube video={url} /> : undefined

/** A blockquote's last line naming who said it: `— Ada Lovelace` (an em dash, two
 *  hyphens, or a hyphen, then a space), a link around the name taken as theirs. A
 *  last line written `- Ada` is markdown's one-item list, and names them all the
 *  same. */
function attributionOf(node: M.Blockquote) {
  const last = node.children.at(-1)
  if (node.children.length < 2) return null
  const only = last?.type === "list" && last.children.length === 1
  const line =
    only && last.children[0]?.children.length === 1
      ? last.children[0].children[0]
      : last
  if (line?.type !== "paragraph") return null
  const [first, ...rest] = line.children
  if (first?.type !== "text") return null
  const lead = only ? null : /^(?:—|–|--|-)\s+/.exec(first.value)
  if (!only && !lead) return null
  const nodes: M.PhrasingContent[] = [
    { ...first, value: first.value.slice(lead?.[0].length ?? 0) },
    ...rest,
  ]
  const link = nodes.find((n): n is M.Link => n.type === "link")
  const name = nodes.map(plainOf).join("").trim()
  if (!name) return null
  return { name, href: link ? safeHref(link.url) : null }
}

const PlainLink = (props: React.ComponentProps<"a">) => {
  const away = /^https?:\/\//.test(props.href ?? "")
  return (
    <a
      {...props}
      {...(away ? { target: "_blank", rel: "noopener noreferrer" } : {})}
    />
  )
}
const PlainCode = ({ children }: { children: string }) => (
  <code>{children}</code>
)
const PlainFigure = ({
  caption,
  children,
}: {
  caption?: React.ReactNode
  children: React.ReactNode
}) => (
  <figure>
    {children}
    {caption && <figcaption>{caption}</figcaption>}
  </figure>
)
const PlainImage = ({ src, alt, title, width, height }: MarkdownImage) => (
  <Image src={src} alt={alt} title={title} width={width} height={height} />
)

/** A markdown text, drawn. */
export function Markdown({
  children,
  tree: prepared,
  components,
  elements,
  embeds,
  transform,
  linkHeadings = false,
  className,
}: MarkdownProps) {
  const tree = React.useMemo(() => {
    const parsed = prepared ?? parseMarkdown(children as string)
    return transform ? transform(parsed) : parsed
  }, [children, prepared, transform])
  const tags: Tags = {
    a: PlainLink,
    img: PlainImage,
    figure: PlainFigure,
    code: PlainCode,
    pre: Highlighted,
    blockquote: Quote,
    ...components,
  }
  // `undefined` leaves the link to the defaults; `null` keeps it a link, defaults and
  // all (user content that must load nothing from another host).
  const embed = (url: string) => {
    const own = embeds?.(url)
    return own === undefined ? builtInEmbed(url) : own
  }
  const draw = drawer(tree, tags, elements ?? {}, embed, linkHeadings)
  return (
    <div data-slot="markdown" className={cn(MARKDOWN, className)}>
      <QuoteMarkMask />
      {tree.children.map(draw.block)}
      {draw.notes.length > 0 && (
        <section data-footnotes="">
          <ol>
            {draw.notes.map((n) => (
              <li key={n.identifier} id={`fn-${n.identifier}`}>
                {n.children.map(draw.block)}
              </li>
            ))}
          </ol>
        </section>
      )}
    </div>
  )
}

/** What draws a tree: its definitions resolved, its headings' anchors unique. */
function drawer(
  tree: M.Root,
  tags: Tags,
  elements: Record<string, React.ElementType>,
  embed: (url: string) => React.ReactNode,
  linkHeadings: boolean,
) {
  const definitions = new Map<string, M.Definition>()
  const notes: M.FootnoteDefinition[] = []
  for (const node of tree.children) {
    if (node.type === "definition") definitions.set(node.identifier, node)
    if (node.type === "footnoteDefinition") notes.push(node)
  }
  // A prepared heading carries its anchor; one parsed here takes the same rule.
  const anchor = anchors()
  const footnote = (id: string) =>
    notes.findIndex((n) => n.identifier === id) + 1
  // A link or an image only where its URL is safe: an unsafe link is its words, an
  // unsafe image its alt text. The drawing components see only checked URLs.
  const link = (
    url: string,
    title: string | null | undefined,
    children: readonly M.PhrasingContent[],
    key: number,
  ) => {
    const href = safeHref(url)
    const words = children.map(inline)
    return href ? (
      <tags.a key={key} href={href} title={title ?? undefined}>
        {words}
      </tags.a>
    ) : (
      <React.Fragment key={key}>{words}</React.Fragment>
    )
  }
  /** A picture's facts, its URL checked; null where it may not load. */
  const pictureOf = (
    url: string,
    alt: string | null | undefined,
    title: string | null | undefined,
    measured: { width?: number; height?: number },
  ): MarkdownImage | null => {
    const src = safeSrc(url)
    return src
      ? {
          src,
          alt: alt ?? "",
          title: title ?? undefined,
          width: measured.width,
          height: measured.height,
        }
      : null
  }
  const image = (
    url: string,
    alt: string | null | undefined,
    title: string | null | undefined,
    key: number,
    measured: { width?: number; height?: number } = {},
  ) => {
    const facts = pictureOf(url, alt, title, measured)
    const Picture = tags.img
    return facts ? <Picture key={key} {...facts} /> : (alt ?? "")
  }
  /** A tag of the text's own (markdown-prepare): its component, given its props and
   *  what it holds. */
  const tag = (node: TagNode, key: number, holds: React.ReactNode) => {
    const Element = node.name ? elements[node.name] : undefined
    if (!Element)
      throw new Error(
        `markdown: the text names <${node.name ?? ""}>, and elements has no such component`,
      )
    return (
      <Element key={key} {...node.data?.props}>
        {holds}
      </Element>
    )
  }

  const inline = (node: M.PhrasingContent, key: number): React.ReactNode => {
    switch (node.type) {
      case "text":
        return node.value
      case "emphasis":
        return <em key={key}>{node.children.map(inline)}</em>
      case "strong":
        return <strong key={key}>{node.children.map(inline)}</strong>
      case "delete":
        return <s key={key}>{node.children.map(inline)}</s>
      case "inlineCode":
        return <tags.code key={key}>{node.value}</tags.code>
      case "break":
        return <br key={key} />
      case "link":
        return link(node.url, node.title, node.children, key)
      case "image":
        return image(node.url, node.alt, node.title, key, dataOf(node))
      case "linkReference": {
        const d = definitions.get(node.identifier)
        return link(d?.url ?? "", d?.title, node.children, key)
      }
      case "imageReference": {
        const d = definitions.get(node.identifier)
        return image(d?.url ?? "", node.alt, d?.title, key)
      }
      case "footnoteReference":
        return (
          <sup key={key}>
            <a href={`#fn-${node.identifier}`}>{footnote(node.identifier)}</a>
          </sup>
        )
      case "html":
        return <code key={key}>{node.value}</code>
      default: {
        const own = node as unknown as TagNode
        if (own.type === "mdxJsxTextElement")
          return tag(
            own,
            key,
            (own.children as M.PhrasingContent[]).map(inline),
          )
        return null
      }
    }
  }

  /** A paragraph that opens with a picture: the picture as a figure, captioned by
   *  the italics after it when they are all there is, and any other words after it
   *  as a paragraph of their own. Null for any other paragraph. */
  const figureOf = (node: M.Paragraph, key: number): React.ReactNode => {
    const meaningful = node.children.filter(
      (c) =>
        !(c.type === "text" && c.value.trim() === "") && c.type !== "break",
    )
    const [first, ...after] = meaningful
    if (first?.type !== "image") return null
    const facts = pictureOf(first.url, first.alt, first.title, dataOf(first))
    if (!facts) return null
    const Picture = tags.img
    const Figure = tags.figure
    const caption =
      after.length === 1 && after[0]?.type === "emphasis"
        ? after[0].children.map(inline)
        : undefined
    const figure = (
      <Figure key={key} image={facts} caption={caption}>
        <Picture {...facts} />
      </Figure>
    )
    if (caption || after.length === 0) return figure
    return (
      <React.Fragment key={key}>
        {figure}
        <p>{after.map(inline)}</p>
      </React.Fragment>
    )
  }

  const textOf = (nodes: readonly M.PhrasingContent[]): string =>
    nodes
      .map((n) =>
        "value" in n ? n.value : "children" in n ? textOf(n.children) : "",
      )
      .join("")

  const block = (node: M.RootContent, key: number): React.ReactNode => {
    switch (node.type) {
      case "paragraph": {
        // A link alone on its line (a bare URL, or one whose words are its URL) is the
        // thing it points to, when there is one to draw; the URL is checked safe first.
        const only = node.children.length === 1 ? node.children[0] : undefined
        const words =
          only?.type === "link" && only.children.length === 1
            ? only.children[0]
            : undefined
        const href =
          only?.type === "link" &&
          words?.type === "text" &&
          words.value === only.url
            ? safeHref(only.url)
            : null
        const embedded = href ? embed(href) : undefined
        if (embedded)
          return (
            <div key={key} data-slot="markdown-embed" data-embed="">
              {embedded}
            </div>
          )
        return (
          figureOf(node, key) ?? <p key={key}>{node.children.map(inline)}</p>
        )
      }
      case "heading": {
        const Tag = `h${node.depth}` as const
        // Asked in order either way, so a parsed heading numbers as a prepared one.
        const own = anchor(textOf(node.children))
        const id = dataOf(node).id ?? own
        return (
          <Tag key={key} id={id}>
            {node.children.map(inline)}
            {linkHeadings && (
              <a
                data-slot="markdown-anchor"
                href={`#${id}`}
                aria-label={`link to ${textOf(node.children)}`}
              >
                #
              </a>
            )}
          </Tag>
        )
      }
      case "blockquote": {
        // The quote item's prose weight: its words, and who said it when the last line
        // names them (`— Name`), drawn as the quote's attribution.
        const by = attributionOf(node)
        const words = by ? node.children.slice(0, -1) : node.children
        return (
          <tags.blockquote
            key={key}
            text={words.map(plainOf).join("\n\n")}
            author={by && { name: by.name, href: by.href }}
          >
            {words.map(block)}
          </tags.blockquote>
        )
      }
      case "list": {
        const items = node.children.map((item, i) => {
          // A tight list's items are their words; a loose one's, paragraphs.
          const kids =
            !node.spread &&
            item.children.length === 1 &&
            item.children[0]?.type === "paragraph"
              ? item.children[0].children.map(inline)
              : item.children.map(block)
          return item.checked == null ? (
            // biome-ignore lint/suspicious/noArrayIndexKey: a list's items are its order
            <li key={i}>{kids}</li>
          ) : (
            // biome-ignore lint/suspicious/noArrayIndexKey: a list's items are its order
            <li key={i} data-task={item.checked ? "done" : "open"}>
              <Box done={item.checked} />
              <span>{kids}</span>
            </li>
          )
        })
        return node.ordered ? (
          <ol key={key} start={node.start ?? undefined}>
            {items}
          </ol>
        ) : (
          <ul key={key}>{items}</ul>
        )
      }
      case "code":
        return (
          <tags.pre
            key={key}
            text={node.value}
            lang={node.lang ?? "text"}
            html={dataOf(node).html ?? null}
            {...fenceOf(node.meta)}
          />
        )
      case "thematicBreak":
        return <hr key={key} />
      case "table": {
        const [head, ...body] = node.children
        const align = (i: number) => node.align?.[i] ?? undefined
        return (
          <table key={key}>
            {head && (
              <thead>
                <tr>
                  {head.children.map((c, i) => (
                    // biome-ignore lint/suspicious/noArrayIndexKey: a row's cells are its columns
                    <th key={i} style={{ textAlign: align(i) }}>
                      {c.children.map(inline)}
                    </th>
                  ))}
                </tr>
              </thead>
            )}
            <tbody>
              {body.map((r, ri) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: a table's rows are its order
                <tr key={ri}>
                  {r.children.map((c, i) => (
                    // biome-ignore lint/suspicious/noArrayIndexKey: a row's cells are its columns
                    <td key={i} style={{ textAlign: align(i) }}>
                      {c.children.map(inline)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        )
      }
      case "html":
        return <Highlighted key={key} text={node.value} lang="html" />
      // Shown elsewhere or not at all: front matter, link definitions, footnotes (at
      // the foot).
      case "yaml":
      case "definition":
      case "footnoteDefinition":
        return null
      default: {
        const own = node as unknown as TagNode
        // A tag on its own line draws itself, as an embed does: set apart, and out
        // of the look's reach (`[data-embed]`).
        if (own.type === "mdxJsxFlowElement")
          return (
            <div key={key} data-slot="markdown-element" data-embed="">
              {tag(own, key, (own.children as M.RootContent[]).map(block))}
            </div>
          )
        return null
      }
    }
  }
  return { block, notes }
}

/** A task's box: done or open. */
function Box({ done }: { done: boolean }) {
  return (
    <span
      aria-label={done ? "done" : "to do"}
      role="img"
      className={cn(
        "mt-[0.3em] grid size-[1em] shrink-0 place-items-center rounded-[0.25em] border border-foreground/30 text-background",
        done && "border-foreground/70 bg-foreground/70",
      )}
    >
      {done && <Check className="size-[0.7em]" strokeWidth={3.5} />}
    </span>
  )
}
