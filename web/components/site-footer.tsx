import { PRODUCT } from "@/lib/product"

// The line every page ends with: the name and its line, the docs, the spec and the
// repository.

const LINK = "hover:text-foreground"

export function SiteFooter() {
  return (
    <footer className="mx-auto mt-48 flex max-w-6xl flex-col gap-3 border-t border-border px-6 py-10 text-[13px] text-muted-foreground md:flex-row md:justify-between">
      <span>
        {PRODUCT.name} · {PRODUCT.tagline}
      </span>
      <span className="flex gap-6">
        <a className={LINK} href="/docs">
          docs
        </a>
        <a className={LINK} href="/docs/spec">
          spec
        </a>
        <a className={LINK} href={PRODUCT.repo}>
          github
        </a>
      </span>
    </footer>
  )
}
