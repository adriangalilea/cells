import { PRODUCT } from "@/lib/product"

// The bar every page opens with: the name, home; then how it works, the docs and the
// repository. Its links say where they go from any page.

const LINK = "transition-colors hover:text-foreground"

export function SiteHeader({ className }: { className?: string }) {
  return (
    <header
      className={`relative z-10 mx-auto flex h-[4.5rem] max-w-6xl items-center justify-between px-6 text-[13px] ${className ?? ""}`}
    >
      <a href="/" className="font-medium tracking-tight">
        {PRODUCT.name.toLowerCase()}
      </a>
      <nav className="flex gap-6 text-muted-foreground">
        <a className={LINK} href="/#story">
          how it works
        </a>
        <a className={LINK} href="/docs">
          docs
        </a>
        <a className={LINK} href={PRODUCT.repo}>
          github
        </a>
      </nav>
    </header>
  )
}
