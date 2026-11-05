// What a scene marks is kept in sight: the nearest box that scrolls sideways around an
// element is scrolled just far enough to show it, never the page.

/** The nearest ancestor of `el` that scrolls sideways, or null. */
function scrollerOf(el: HTMLElement): HTMLElement | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const x = getComputedStyle(p).overflowX
    if ((x === "auto" || x === "scroll") && p.scrollWidth > p.clientWidth) return p
  }
  return null
}

/** Scrolls `el`'s sideways scroller so `el` is in it, with `margin` px beside it. */
export function revealX(el: HTMLElement | null, margin = 24) {
  if (!el) return
  const box = scrollerOf(el)
  if (!box) return
  const b = box.getBoundingClientRect()
  const e = el.getBoundingClientRect()
  if (e.right + margin > b.right) box.scrollLeft += e.right + margin - b.right
  else if (e.left - margin < b.left) box.scrollLeft -= b.left - (e.left - margin)
}
