"use client"

// A NEXT LINK THAT PREFETCHES WHAT THE READER REACHES FOR, never what merely scrolls
// past. Next's own <Link>, so navigation, `replace`, `scroll` and the rest are Next's,
// with one difference: Next prefetches a static route IN FULL as each link enters the
// viewport, and on a site of static pages that is most of them, fetched by one reader
// scrolling one page (ui.adriangalilea.com's home fetched 96 item pages, 26 MB, 16
// times the page itself; adriangalilea.com's header made every page fetch the home's
// feed, 294 KB). Each of those is a read of the deployment's prerender store, which
// Vercel bills as ISR reads and Fast Origin Transfer. Here the pointer resting on the
// link, the keyboard reaching it, or a finger landing on it starts the prefetch, a
// beat before the click, so the page is there when it lands.
//
// Next only: it is next/link's own prefetch the reader's intent hands over. Make it
// every internal link of a site (a lint rule refusing `next/link` elsewhere keeps it
// so, as biome's `noRestrictedImports` does).

import NextLink from "next/link"
import * as React from "react"

export type LinkProps = Omit<React.ComponentProps<typeof NextLink>, "prefetch">

export function Link({
  onMouseEnter,
  onFocus,
  onTouchStart,
  ...props
}: LinkProps) {
  // Once wanted, the route is Next's own to prefetch (`null`: its default for a static
  // route, the whole page) and stays so.
  const [wanted, setWanted] = React.useState(false)
  return (
    <NextLink
      {...props}
      prefetch={wanted ? null : false}
      onMouseEnter={(event) => {
        setWanted(true)
        onMouseEnter?.(event)
      }}
      onFocus={(event) => {
        setWanted(true)
        onFocus?.(event)
      }}
      onTouchStart={(event) => {
        setWanted(true)
        onTouchStart?.(event)
      }}
    />
  )
}
