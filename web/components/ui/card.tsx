"use client"

import { useRender } from "@base-ui/react/use-render"
import * as React from "react"
import { cn } from "@/lib/utils"
import {
  type Material,
  MaterialProvider,
  type Scheme,
  useMaterial,
  useSurfaceScheme,
} from "@/components/ui/material"
import {
  FLARE_INK,
  FLARE_LIGHT,
  GlassEdge,
  type GlassLight,
  GlassSpill,
  type GlassStyle,
  glassFront,
  glassStyle,
  glassVariants,
  PLAIN_BAND,
  PLAIN_FRONT,
  schemeOf,
} from "@/components/ui/material-glass"
import { ScrollArea } from "@/components/ui/scroll-area"

// THE CARD: every box that holds content, shadcn's Card by its own name and parts, so
// anyone who knows shadcn knows it, in either material: plain, the page's card with its
// hairline; glass, the material's glass, rounder, as glass is. Glass is a face of the
// card, never a family of its own.
//
//   <Card>                                   a region: a settings group, a list
//     <CardHeader>
//       <CardTitle>release 1.4</CardTitle>
//       <CardDescription>ready to ship</CardDescription>
//       <CardAction><Button …/></CardAction>
//     </CardHeader>
//     <CardContent>…</CardContent>
//     <CardFooter>                           where it is answered, a band
//       <Button variant="secondary">later</Button>
//       <Button>publish</Button>
//     </CardFooter>
//   </Card>
//
// An item, one thing a person reads and acts on (a ticket, a step, a post), is a card
// too: `<Card hue padding="compact" interactive>`, its hue, padding and the hand's
// light being options here. What only an item has (its kind, its author, its counts,
// opening it whole) is `item-card`, an item of its own on this one: those parts need
// an avatar, a dialog and a button, which no region should install.
//
// THE FLARE RULE (tokens.css, material-glass `FLARE_LIGHT` and `FLARE_INK`): a card
// that carries a hue (`hue`) wears that ONE hue, alpha the only variable. Glass: the
// hue stains the glass (6%) and its rim catches it; the marks inside (its kind, a count
// that is yours) in the hue made readable on the face. Plain: the page's card and its
// hairline, never a tinted fill or a coloured edge; the hue only in the marks. An
// `interactive` card (one a person points at and acts on) catches more light under the
// hand and nothing moves (a container never moves): glass a step more stain and rim,
// plain a step brighter. `lifted` holds that light without the hand (a card carried).
//
// Its FRONT SURFACES, for a card that scrolls or pins: a `head` across its top and a
// `foot` along its bottom, strips of the card itself, not pieces laid on it (a tab
// list, a board's column names, a card's buttons). In glass each is the material's
// front (`glassFront`): its own blur over what passes under it, one lit hairline as its
// inner edge, the card's edge unbroken over both. Plain, a solid strip a step darker
// than the card. A head is `headHeight` tall (3rem), a foot `footHeight`, told to what
// it holds as `--card-head` and `--card-foot`; what pins under the head (a table's
// header row) pins at `--card-under`. They are props, not children, because they are
// laid beside the scroller, never inside it.
//
//   <Card material="glass" head={<TabsList>…</TabsList>} maxHeight="30rem">…</Card>
//
// It SCROLLS when it is given a height: `maxHeight` (it grows to the cap, then
// scrolls) or `height` (it keeps it whatever it holds; what fills it, a log, scrolls
// itself). Its body is then the one capped scroller (`ScrollArea`), the head and the
// foot laid OVER it, so what scrolls passes beneath them and comes out from under them:
// in glass blurred by them, plain hidden by them. Never a front inside the scroller:
// a plain scroller's fade is a mask, which makes it a backdrop root, and a glass front
// inside it would blur nothing. Long ones wear thumbs (`thumb`, on).
// Growing with what it holds, its head can PIN to the page while the body scrolls
// under it (`pin`, a CSS length: below a site header). Plain, its frame is then drawn
// by the head and the body, so the head pinned mid-card is still the card's rounded
// top. `glide` eases the body to a new height (a tab of another length) instead of
// jumping.
//
// `title`, `description`, `action` and `footer` are the same parts given as props, for
// the one-line card: `<Card title="tasks" action={…} footer={…}>`. They draw
// CardHeader and CardFooter, one drawing.
//
// Glass is built of the material's parts, bottom to top: a back layer (the body: its
// blur, its fill, the shadow it casts), what it holds, the spill (`GlassSpill`: what
// nears the edge spills its light round it, on unless `spill={false}`), the front
// surfaces, the edge (`GlassEdge`). The root never blurs: a backdrop blur inside
// another sees only what is between the two, so a front inside a blurring root
// blurred nothing passing under it. What it holds stands on glass (`--glass-body`):
// a control inside takes its plain face unless chosen otherwise (no glass on glass).
//
// A frame is drawn once. What draws a box of its own when it stands alone (a log, a
// trace) asks `useInFrame()` and draws none inside one: a card, or anything that says
// so with `<InFrame>`. A box inside a box is two edges, two corners and two grounds a
// few pixels apart; the default makes that impossible to write by accident, and the
// item's own `frame` prop still decides when given. A card inside another frame draws
// the inset face, a step off what it stands on.

const Framed = React.createContext(false)

/** What it holds stands inside a frame already drawn: a box of its own is not. */
export function InFrame({ children }: { children: React.ReactNode }) {
  return <Framed.Provider value>{children}</Framed.Provider>
}

/** Whether a frame is already drawn round this point (a card). */
export function useInFrame(): boolean {
  return React.useContext(Framed)
}

/** Inside a card's head: what fits itself into the strip (a tab list) reads it. */
const InHead = React.createContext<{ material: Material } | null>(null)

/** The head this is inside, or null: a tab list in a head fits the strip (it stands
 *  as far from the card's side as from the strip's top, a line list on the strip's
 *  rule). */
export function useCardHead(): { material: Material } | null {
  return React.useContext(InHead)
}

/** The body of a card that scrolls: its scroller, and whether the card keeps a height
 *  its body fills (`height`). */
export type CardBody = {
  scroller: React.RefObject<HTMLDivElement | null>
  filled: boolean
}
const InBody = React.createContext<CardBody | null>(null)

/** The body of the card this is inside when that card scrolls, or null. A tab's panel
 *  keeps its own place in the scroll by it, and fills a card of a set height. */
export function useCardBody(): CardBody | null {
  return React.useContext(InBody)
}

/** How far a card's content stands from its edge: a region's (a settings group, a
 *  list: the rhythm's rung at the body's type, 1.25rem), an item standing alone
 *  (1rem), or a row-like item in a list or a board (0.75rem). Its foot band runs out
 *  to the edge by the same. */
export type CardPadding = "region" | "regular" | "compact"
const PAD: Record<CardPadding, string> = {
  region: "1.25rem",
  regular: "1rem",
  compact: "0.75rem",
}

/** The face a card drew, as its parts read it: its material, and whether it stands
 *  inside another frame (the inset face, translucent, so its foot band is a
 *  translucent step of it and never the page's card colour laid over a pane). */
export type CardFace = { material: Material; inset: boolean }
const CardContext = React.createContext<CardFace | null>(null)

/** The face of the card `part` stands in, read by a card's parts (here and in
 *  `item-card`); outside a card it screams. */
export function useCardFace(part: string): CardFace {
  const c = React.useContext(CardContext)
  if (!c) throw new Error(`${part}: a card's part stands inside a Card`)
  return c
}

export type CardProps = Omit<
  useRender.ComponentProps<"section">,
  "title" | "style"
> &
  GlassLight & {
    /** What it holds, in a few words: a CardHeader's CardTitle. */
    title?: React.ReactNode
    /** A line under the title: its CardDescription. */
    description?: React.ReactNode
    /** Something to do with it (a button, a menu): its CardAction, at the head's end
     *  when it has a `head`. */
    action?: React.ReactNode
    /** Where it is answered: its CardFooter, the answers at the trailing end, the
     *  secondary first, the answering one last (`<Button variant="secondary">later
     *  </Button><Button>publish</Button>`); a note before them with `me-auto`. */
    footer?: React.ReactNode
    /** A front surface across its top: a tab list, column names. */
    head?: React.ReactNode
    /** The head's height, a CSS length (3rem). */
    headHeight?: string
    headClassName?: string
    /** A front surface along its bottom, laid over what scrolls: a card's answers,
     *  laid as `footer`'s are (trailing, the answering one last), or a name. */
    foot?: React.ReactNode
    /** The foot's height, a CSS length (3rem). */
    footHeight?: string
    footClassName?: string
    /** A cap on its height, a CSS length: past it the body scrolls. */
    maxHeight?: string
    /** Its height, a CSS length, whatever it holds: the body scrolls what does not
     *  fit, and what scrolls itself (a log) fills it. */
    height?: string
    /** Where its head pins in a page that scrolls, a CSS length (`"0px"`, below a
     *  site header `"var(--header-height)"`). Only for a card that grows. */
    pin?: string
    /** The head and the foot lie OVER what it holds, not above and below it, though
     *  the card itself does not scroll: what it holds runs under them and pads itself
     *  by `--card-head` and `--card-foot` (a board's columns, each its own
     *  `ScrollArea cover="var(--card-head)"`, scrolling under the head). */
    over?: boolean
    /** Overlay thumbs while its body scrolls. Default on. */
    thumb?: boolean
    /** Its body eases to a new height rather than jumping (a tab of another
     *  length). Only for a card that grows. */
    glide?: boolean
    /** In glass, what nears its edge spills its light round it. Off for content
     *  that must stay crisp to the edge. Default on. */
    spill?: boolean
    /** Draws its surface (plain's card, glass's body and edge). Off, it is its
     *  layout and its context alone: what draws a box of its own when it stands
     *  alone (a log, a trace) passes `frame={!useInFrame()}` and is drawn once.
     *  Default on. */
    frame?: boolean
    /** Its corners, a CSS length (never a percentage): a region's plain the theme's
     *  xl, glass 2.4 times the theme's radius (24 px by default); an item's 0.75rem;
     *  inside another card, concentric with it. */
    radius?: string
    /** How far what it holds stands from its edge (`CardPadding`): a region's by
     *  default. */
    padding?: CardPadding
    /** Its flare, a CSS colour (its kind's), worn by the flare rule: none, a neutral
     *  card. */
    hue?: string
    /** One a person points at and acts on (a ticket, a step): it catches more light
     *  under the hand, nothing moving. */
    interactive?: boolean
    /** The hand's light held on it without the hand: a card carried. */
    lifted?: boolean
    /** Classes on what lays out what it holds (its padding, its gap: `flex flex-col
     *  gap-4 p-(--card-pad)`, the card's one padding). On the card itself unless it
     *  has a head, a foot, a height or a glide; there `className` is the frame's
     *  alone. */
    bodyClassName?: string
    /** Glass: its body's colour (default the glass's own), and its front surfaces'
     *  (default the face's own at 55%), any CSS colour thin enough to stay glass. Set
     *  on that surface alone, so a card inside keeps its own. */
    backColor?: string
    frontColor?: string
    material?: Material
    /** The colour scheme it stands in (material): default the nearest provider's.
     *  Dark, everything inside reads in the dark scheme too. */
    scheme?: Scheme
    style?: GlassStyle
  }

/** What lays out what a card holds, unless `bodyClassName` says otherwise: a region's
 *  content a field's gap apart, an item's a word's. */
const BODY: Record<CardPadding, string> = {
  region: "flex flex-col gap-4 p-(--card-pad)",
  regular: "flex flex-col gap-2 p-(--card-pad)",
  compact: "flex flex-col gap-2 p-(--card-pad)",
}

/** A card inside a frame already drawn (`useInFrame`: a card's body, or what says so
 *  with `InFrame`), in either material: a step of light off the surface under it, a
 *  hairline and a corner concentric with its holder's (`useNestedRadius`), nothing of
 *  glass's own (no blur, no rim, no spill: glass inside glass drew two rims, two blurs
 *  and two bodies where the eye wants one surface and what is on it). A card that is a
 *  link brightens a step under the hand. Every box that stands in another (a profile,
 *  a post, a preview) is this one, drawn once here. */
const INSET =
  "rounded-(--card-radius) border border-current/[0.07] bg-fill-group [--fill-ink:currentColor] transition-colors [a&]:hover:bg-fill-row-hover"
/** On glass the inset is a clear step off the body (Apple's Materials: glass on glass
 *  never without a step of separation): the resting fill and a brighter hairline,
 *  where the group's 3.5% left a card on a glass lane the lane's own smoked tone. */
const INSET_ON_GLASS =
  "[@container_style(--glass-body:1)]:bg-fill-rest [@container_style(--glass-body:1)]:border-current/[0.12]"

/** The light an interactive card catches, by the face drawn: at rest, under the hand
 *  and held (`data-lifted`), a step of the same light and nothing else. Glass's stain
 *  is the flare rule's (`FLARE_LIGHT`). */
const HAND: Record<Material, string> = {
  glass:
    "[--glass-rim:1] hover:[--glass-rim:1.6] data-[lifted]:[--glass-rim:1.6]",
  plain:
    "hover:bg-[color-mix(in_srgb,var(--card),var(--foreground)_3%)] data-[lifted]:bg-[color-mix(in_srgb,var(--card),var(--foreground)_3%)]",
}

/** A plain card whose head pins to the page draws its frame by its parts: the head is
 *  its TOP (its top and side edges, the top corners) and the body the rest (its
 *  sides, its foot, the bottom corners, the card's colour). Pinned mid-card the head
 *  is still the card's rounded top, the body's sides running on below it; drawn by
 *  the card round them, the pinned head would be a square strip between two
 *  straight edges. */
const PLAIN_HEAD_EDGE = "border rounded-t-(--card-radius)"
const PLAIN_BODY_EDGE =
  "border-border border-x border-b bg-background bg-[linear-gradient(var(--color-card),var(--color-card))] rounded-b-(--card-radius)"

/** What a plain pinned head pins in: a square of the page's ground, the head
 *  (rounded) inside it, so rows passing under it never show behind its corners. */
const PLAIN_PIN = "sticky top-(--card-pin) z-10 bg-background"

/** How a glass card's light follows its state: a card that moves (lifting under the
 *  pointer) and catches more light does both as one gesture, the light on its own
 *  500 ms ease trailing the movement by a breath. Eased on the two elements that draw
 *  it (the edge, the fronts), never on the root: the rim is inherited, and moving
 *  there restyled everything the card holds on every frame. */
const CARD_LIGHT =
  "[transition:--glass-rim_500ms_cubic-bezier(0.22,1,0.36,1)_40ms] motion-reduce:[transition:none]"

/** The body's glide (`glide`): the height it is given follows what it holds, eased
 *  on the tabs' curve; on after the first reading, so the page opens at its height,
 *  never growing from nothing. */
const GLIDE =
  "transition-[height] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] motion-reduce:transition-none"

function useGlide(on: boolean) {
  const [inner, setInner] = React.useState<HTMLDivElement | null>(null)
  const [height, setHeight] = React.useState<number | null>(null)
  const [moving, setMoving] = React.useState(false)
  React.useLayoutEffect(() => {
    if (!on || !inner) return
    const read = () => setHeight(inner.getBoundingClientRect().height)
    read()
    const frame = requestAnimationFrame(() => setMoving(true))
    const sizes = new ResizeObserver(read)
    sizes.observe(inner)
    return () => {
      cancelAnimationFrame(frame)
      sizes.disconnect()
    }
  }, [on, inner])
  return { setInner, height, moving }
}

/** A region's glass corner, from the theme's one radius: 24 px at the default
 *  0.625rem, and with it whatever a theme sets. */
const GLASS_RADIUS = "calc(var(--radius) * 2.4)"
/** An item's corner, in either material: the designed card. */
const ITEM_RADIUS = "0.75rem"

/** A card's corner as drawn, a length in px read off the element: what the glass's
 *  edge needs, an SVG outline that can read no variable. Until it is read, the
 *  default theme's. */
function useDrawnRadius(node: HTMLElement | null, on: boolean): string {
  const [drawn, setDrawn] = React.useState("1.5rem")
  React.useLayoutEffect(() => {
    if (!on || !node) return
    const r = getComputedStyle(node).borderTopLeftRadius
    if (!r.endsWith("px"))
      throw new Error(`Card: its corner was drawn as ${r}, not a length`)
    setDrawn(r)
  }, [node, on])
  return drawn
}

/** A card inside a card nests by the rule below.
 *  Hugging its holder, set in less than the holder's corner (a pane a gap inside its
 *  frame), it is concentric: the holder's corner less the inset, the two curves
 *  parallel. Standing well inside it (a block in, the default body's padding), it
 *  takes a card's own corner, never rounder than the holder's: concentric there would
 *  fall below any radius and every deep card would read square. Measured, since what
 *  sets it in (the outer's padding, a gap, a wrapper) is a class no CSS can read;
 *  across, since how far down it stands moves as its holder scrolls. Until it is
 *  measured, a card's own corner, the default body's answer. */
function useNestedRadius(node: HTMLElement | null, on: boolean): string {
  const [nested, setNested] = React.useState("var(--radius-xl)")
  React.useLayoutEffect(() => {
    if (!on || !node) return
    const outer = node.parentElement?.closest<HTMLElement>("[data-slot=card]")
    if (!outer) return
    const read = () => {
      const o = outer.getBoundingClientRect()
      const i = node.getBoundingClientRect()
      const inset = Math.max(0, Math.min(i.left - o.left, o.right - i.right))
      const r = Number.parseFloat(getComputedStyle(outer).borderTopLeftRadius)
      setNested(inset < r ? `${r - inset}px` : `min(var(--radius-xl), ${r}px)`)
    }
    read()
    const sizes = new ResizeObserver(read)
    sizes.observe(outer)
    sizes.observe(node)
    return () => sizes.disconnect()
  }, [node, on])
  return nested
}

export function Card({
  title,
  description,
  action,
  footer,
  head,
  headHeight = "3rem",
  headClassName,
  foot,
  footHeight = "3rem",
  footClassName,
  maxHeight,
  height,
  pin,
  over = false,
  thumb = true,
  glide = false,
  spill = true,
  frame = true,
  radius,
  padding = "region",
  hue,
  interactive = false,
  lifted = false,
  bodyClassName,
  backColor,
  frontColor,
  rim,
  rimWidth,
  rimColor,
  tint,
  tintAmount,
  material,
  scheme,
  render,
  className,
  style,
  children,
  ref,
  ...props
}: CardProps) {
  // Inside a frame already drawn, its surface is the inset one (`INSET`) whatever
  // the material: its glass would be glass on glass.
  const inFrame = useInFrame()
  const own = useMaterial(material)
  const inset = frame && inFrame
  const m = inset ? "plain" : own
  const glass = m === "glass"
  const [s, worn] = useSurfaceScheme<HTMLElement>(scheme, ref)
  const [node, setNode] = React.useState<HTMLElement | null>(null)
  const here = React.useCallback(
    (n: HTMLElement | null) => {
      setNode(n)
      worn(n)
    },
    [worn],
  )
  const nested = useNestedRadius(node, inset && radius === undefined)
  const drawn = useDrawnRadius(node, glass && frame && radius === undefined)
  const scroller = React.useRef<HTMLDivElement | null>(null)
  const scrolls = maxHeight !== undefined || height !== undefined
  // The fronts lie over what it holds: its own scroller's, or (`over`) content
  // that runs under them by itself.
  const laid = scrolls || over
  if (over && (scrolls || pin !== undefined))
    throw new Error(
      "Card: `over` lays the fronts over content that scrolls by itself; a card given a height lays them over its scroller already, and a pinned head is in the flow",
    )
  if (maxHeight !== undefined && height !== undefined)
    throw new Error(
      "Card: `maxHeight` caps a card that grows, `height` keeps one still; give one",
    )
  if (scrolls && (pin !== undefined || glide))
    throw new Error(
      "Card: `pin` and `glide` are for a card that grows with what it holds; one given a height scrolls itself, its head laid over its top",
    )
  if (pin !== undefined && head === undefined)
    throw new Error("Card: `pin` pins the head to the page; give a `head`")
  if (head !== undefined && (title !== undefined || description !== undefined))
    throw new Error(
      "Card: a card with a head names itself in it (or in a heading above it); `title` and `description` are for one without",
    )
  // Asked of the material the caller chose, never the one drawn: a glass card in a
  // frame draws the inset face, and its light is simply not drawn there. Asked of the
  // drawn one it would throw on a nested glass card's own tint.
  if (
    own !== "glass" &&
    [backColor, frontColor, rim, rimWidth, rimColor, tint, tintAmount].some(
      (v) => v !== undefined,
    )
  )
    throw new Error(
      'Card: `backColor`, `frontColor` and the light (`rim`, `tint`, …) are the glass face\'s; give `material="glass"`',
    )
  if (hue !== undefined && (tint !== undefined || tintAmount !== undefined))
    throw new Error(
      "Card: `hue` is its colour by the flare rule, `tint` a glass's own light; give one",
    )
  const glideBody = useGlide(glide)
  const titled = title || description || (action && head === undefined)
  // The card is its own body (what lays out what it holds) unless something stands
  // apart from that body: a front surface, a scroller, a glide.
  const parts = head !== undefined || foot !== undefined || scrolls || glide
  // Plain and pinned to the page: the frame is drawn by the head and the body.
  const partsFrame = !glass && pin !== undefined && head !== undefined
  const body = cn(BODY[padding], bodyClassName)

  const header = titled && (
    <CardHeader>
      {title && <CardTitle>{title}</CardTitle>}
      {description && <CardDescription>{description}</CardDescription>}
      {action && <CardAction>{action}</CardAction>}
    </CardHeader>
  )
  const held = (
    <>
      {header}
      {children}
      {footer && <CardFooter>{footer}</CardFooter>}
    </>
  )

  // A front surface: laid over the scroller when the card scrolls (or over what it
  // holds, `over`), else in the card's flow (pinned to the page with `pin`).
  const front = (at: "top" | "bottom") => {
    const top = at === "top"
    const pinned = top && pin !== undefined
    return cn(
      "z-10 flex shrink-0 items-center px-(--card-pad)",
      // The head leads with what it holds, a gap apart; the foot holds the card's
      // answers at its trailing end, a pair's gap apart (button's ANSWERS rule).
      top ? "gap-4" : "justify-end gap-2",
      // A folder's tabs (a plain line list, tabs' `data-folder`) stand from a little
      // below the strip's top: the action stands as low, on their centre line.
      "[&:has(>[data-folder])>[data-slot=card-action]]:self-stretch [&:has(>[data-folder])>[data-slot=card-action]]:pt-2",
      top ? "h-(--card-head)" : "h-(--card-foot)",
      glass
        ? [
            glassFront[at],
            CARD_LIGHT,
            // Pinned mid-card it is square, the card's clip rounding it at rest.
            pinned && "rounded-none",
          ]
        : [
            PLAIN_FRONT,
            top
              ? partsFrame
                ? PLAIN_HEAD_EDGE
                : "rounded-t-[inherit] border-b"
              : "rounded-b-[inherit] border-t",
          ],
      laid
        ? ["absolute inset-x-0", top ? "top-0" : "bottom-0"]
        : pinned
          ? glass
            ? "sticky top-(--card-pin)"
            : "relative"
          : "relative",
      top ? headClassName : footClassName,
    )
  }
  const headPart = head !== undefined && (
    <div data-slot="card-head" className={front("top")}>
      <InHead.Provider value={{ material: m }}>{head}</InHead.Provider>
      {action && (
        <div
          data-slot="card-action"
          className="ml-auto flex shrink-0 items-center gap-2"
        >
          {action}
        </div>
      )}
    </div>
  )
  const footPart = foot !== undefined && (
    <div data-slot="card-foot" className={front("bottom")}>
      {foot}
    </div>
  )

  // What it holds stands on glass, told so (a sibling of the back layer, not inside
  // it): a control inside takes its plain face.
  const onGlass = glass && frame && "[--glass-body:1]"
  const content = scrolls ? (
    <ScrollArea
      ref={scroller}
      data-slot="card-scroller"
      thumb={thumb}
      // edge: in glass the spill ends it, a fade under it ended it twice.
      fade={!(glass && frame && spill)}
      cover={head === undefined ? undefined : "var(--card-head)"}
      coverEnd={foot === undefined ? undefined : "var(--card-foot)"}
      className={cn(
        onGlass,
        maxHeight !== undefined ? "max-h-(--card-max)" : "flex flex-1 flex-col",
      )}
    >
      <InBody.Provider value={{ scroller, filled: height !== undefined }}>
        <div
          data-slot="card-body"
          data-card-pad=""
          className={cn(body, height !== undefined && "flex-1")}
        >
          {held}
        </div>
      </InBody.Provider>
    </ScrollArea>
  ) : parts ? (
    <div
      data-slot="card-body"
      data-card-pad={glide ? undefined : ""}
      className={cn(
        onGlass,
        glide && "overflow-clip",
        glide && glideBody.moving && GLIDE,
        partsFrame && PLAIN_BODY_EDGE,
        !glide && body,
      )}
      style={
        glide && glideBody.height !== null
          ? { height: `${glideBody.height}px` }
          : undefined
      }
    >
      {glide ? (
        <div ref={glideBody.setInner} data-card-pad="" className={body}>
          {held}
        </div>
      ) : (
        held
      )}
    </div>
  ) : glass ? (
    // The card is its own body: its layout stays the root's, and what it holds is
    // marked as standing on glass by a wrapper that lays out nothing.
    // It passes the card's corners on, so a foot band (`rounded-b-[inherit]`) rounds
    // with the card: through a wrapper of no corners it drew square bottom corners.
    <div className={cn("contents rounded-[inherit]", onGlass)}>{held}</div>
  ) : (
    held
  )

  const corner =
    radius ??
    (inset
      ? nested
      : padding !== "region"
        ? ITEM_RADIUS
        : glass
          ? GLASS_RADIUS
          : "var(--radius-xl)")
  const vars = {
    // The one padding its body and its head stand in by, read by what lines up with
    // them inside (tabs in the head, a foot band running to the edge).
    "--card-pad": PAD[padding],
    "--card-radius": corner,
    ...(hue === undefined ? {} : { "--flare-hue": hue }),
    ...(head === undefined
      ? {}
      : {
          "--card-head": headHeight,
          // Where the head ends, for what pins under it (a table's header row,
          // `sticky top-(--card-under)`): below the pinned head on the page, or the
          // head's own height inside the card's scroller.
          "--card-under":
            pin === undefined
              ? "var(--card-head)"
              : "calc(var(--card-pin) + var(--card-head))",
        }),
    ...(foot === undefined ? {} : { "--card-foot": footHeight }),
    ...(maxHeight === undefined ? {} : { "--card-max": maxHeight }),
    ...(height === undefined ? {} : { "--card-height": height }),
    ...(pin === undefined ? {} : { "--card-pin": pin }),
    ...(glass
      ? {
          "--glass-radius": "var(--card-radius)",
          ...(frontColor === undefined
            ? {}
            : { "--glass-front-color": frontColor }),
          ...glassStyle({ rim, rimWidth, rimColor, tint, tintAmount }),
        }
      : {}),
  }
  // The edge and the spill draw the corners as an outline, which needs the length
  // itself, never a variable: the corner as it was drawn.
  const edgeRadius = radius ?? drawn

  // Pinned in glass: the head is square, so the card's corners are a clip round the
  // head and the body; `clip`, which is no scroller, so the head pins on.
  const flow =
    glass && pin !== undefined && head !== undefined ? (
      <div className="flex flex-col overflow-clip rounded-[inherit]">
        {headPart}
        {content}
        {footPart}
      </div>
    ) : (
      <>
        {!laid &&
          headPart &&
          (partsFrame ? (
            <div className={cn(PLAIN_PIN, "shrink-0")}>{headPart}</div>
          ) : (
            headPart
          ))}
        {content}
        {!laid && footPart}
      </>
    )

  return useRender({
    defaultTagName: "section",
    render,
    ref: here,
    props: {
      "data-slot": "card",
      "data-material": m,
      "data-lifted": lifted || undefined,
      // The element whose padding is the body's (here, or the body part): what runs
      // to the card's edge reads it there (tabs' track).
      "data-card-pad": parts ? undefined : "",
      className: cn(
        "relative flex flex-col rounded-(--card-radius)",
        !parts && body,
        height !== undefined && "h-(--card-height)",
        schemeOf(s),
        !frame
          ? undefined
          : glass
            ? // No glass of its own (its body is the back layer): its corners, the
              // words a glass's body gives what it holds, and its own movement (a
              // card lifting under the pointer), which its light follows on the
              // same ease (CARD_LIGHT).
              "isolate text-foreground scheme-light:text-zinc-950 [transition:translate_500ms_cubic-bezier(0.22,1,0.36,1),scale_500ms_cubic-bezier(0.22,1,0.36,1)] motion-reduce:[transition:none]"
            : inset
              ? [INSET, INSET_ON_GLASS]
              : partsFrame
                ? "shadow-sm"
                : // A card that is a link brightens a step under the hand.
                  "border border-border bg-card text-card-foreground shadow-sm transition-colors [a&]:hover:bg-[color-mix(in_srgb,var(--card),var(--foreground)_3%)]",
        (interactive || lifted) && HAND[m],
        hue !== undefined && [
          FLARE_INK,
          "[--card-ink:var(--flare-ink)]",
          own === "glass" && FLARE_LIGHT,
        ],
        className,
      ),
      style: { ...vars, ...style } as React.CSSProperties,
      ...props,
      // What is inside stands on the card, not on what is behind it: it wears the
      // card's scheme, whatever decided it (a picture under the card included).
      children: (
        <MaterialProvider scheme={s}>
          <CardContext.Provider value={{ material: m, inset }}>
            <InFrame>
              {glass && frame && (
                <div
                  aria-hidden
                  data-slot="card-back"
                  className={cn(
                    glassVariants({ shape: "surface", edge: false }),
                    "pointer-events-none absolute inset-0 -z-10 [--glass-radius:inherit]",
                  )}
                  style={
                    backColor === undefined
                      ? undefined
                      : ({ "--glass-tint": backColor } as GlassStyle)
                  }
                />
              )}
              {flow}
              {glass && frame && spill && <GlassSpill radius={edgeRadius} />}
              {laid && headPart}
              {laid && footPart}
              {glass && frame && (
                <GlassEdge
                  radius={edgeRadius}
                  className={cn("z-20", CARD_LIGHT)}
                />
              )}
            </InFrame>
          </CardContext.Provider>
        </MaterialProvider>
      ),
    },
  })
}

// ── its parts, shadcn's names ───────────────────────────────────────────────────

/** What a card is, above what it holds: its title, a line under it, and something to
 *  do with it at its end (`CardAction`), shadcn's grid: the action beside the words,
 *  as tall as both. The title and what it is for stand a gap apart: one thing, as a
 *  dialog's head. */
export function CardHeader({
  className,
  ...props
}: React.ComponentProps<"header">) {
  return (
    <header
      data-slot="card-header"
      className={cn(
        "grid auto-rows-min items-start gap-x-4 gap-y-2 has-[>[data-slot=card-action]]:grid-cols-[minmax(0,1fr)_auto]",
        className,
      )}
      {...props}
    />
  )
}

/** What it holds, in a few words. */
export function CardTitle({ className, ...props }: React.ComponentProps<"h3">) {
  return (
    <h3
      data-slot="card-title"
      className={cn("font-medium text-sm leading-tight", className)}
      {...props}
    />
  )
}

/** A line under the title: at the muted 65%, which glass holds at 4.5:1. */
export function CardDescription({
  className,
  ...props
}: React.ComponentProps<"p">) {
  return (
    <p
      data-slot="card-description"
      className={cn("text-pretty text-current/65 text-xs leading-5", className)}
      {...props}
    />
  )
}

/** Something to do with the card (a button, a menu), at the header's end beside the
 *  title and its line. */
export function CardAction({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-action"
      className={cn(
        "col-start-2 row-span-2 row-start-1 -my-1 shrink-0 self-start justify-self-end",
        className,
      )}
      {...props}
    />
  )
}

/** What it holds. The card pads it; it adds nothing of its own. */
export function CardContent({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return <div data-slot="card-content" className={className} {...props} />
}

/** Where a card is answered: a band along its foot, edge to edge, the dialog's own
 *  band (the material's front: plain a step of the page's ink with a hairline, glass
 *  the glass's front, inset in a frame a translucent step of the inset face), square
 *  where it meets the card and on its curve along the foot. Its answers at the
 *  trailing end, a pair's gap apart, the answering one last (button's ANSWERS rule);
 *  a note before them takes the leading side (`me-auto`, as `CardAuthor` does). */
export function CardFooter({
  className,
  ...props
}: React.ComponentProps<"div">) {
  const { material, inset } = useCardFace("CardFooter")
  return (
    <div
      data-slot="card-footer"
      className={cn(
        "relative mt-auto -mx-(--card-pad) -mb-(--card-pad)",
        "flex items-center justify-end gap-2 px-(--card-pad) py-[calc(var(--card-pad)/2)] text-xs",
        "[&>[data-slot=card-author]]:me-auto",
        material === "glass"
          ? glassFront.bottom
          : inset
            ? "border-current/[0.07] border-t bg-current/[0.04]"
            : [PLAIN_BAND, "border-t [--band-base:var(--card)]"],
        "rounded-b-[inherit]",
        className,
      )}
      {...props}
    />
  )
}
