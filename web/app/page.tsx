import { Fragment, Suspense } from "react"
import { Hero, Landing } from "@/components/hero"
import { Glass } from "@/components/ui/material-glass"
import { Story } from "@/components/story"
import { PRODUCT } from "@/lib/product"

/** What each neighbour does, honestly, and what a cell does instead. */
const ROWS = [
  {
    them: "CSV with a JSON schema beside it",
    lack: "two files that drift: the schema says one thing, the file another, and nothing notices",
    cells: "the types are the header: one file, read and checked as one",
  },
  {
    them: "Parquet",
    lack: "typed, fast, and a binary: no keys, no references, nothing you can read, diff in git or fix by hand",
    cells: "text you read and diff, keys and references checked; Parquet stays a good export",
  },
  {
    them: "Oils' TSV8",
    lack: "types in the file for shell pipes; the spec is syntax only, so int, float or decimal is each reader's guess",
    cells: "what a value means is the spec, money exact at its cents, the same in every reader",
  },
  {
    them: "a spreadsheet",
    lack: "a formula points at B7, and a row inserted above moves what B7 is; a macro can run when the file opens",
    cells: "a formula names columns and keys, and reads no clock, network or file; opening a file runs nothing",
  },
]

export default function Page() {
  return (
    <main>
      <header className="relative z-10 mx-auto flex h-[4.5rem] max-w-6xl items-center justify-between px-6 text-[13px]">
        <span className="font-medium tracking-tight">{PRODUCT.name.toLowerCase()}</span>
        <nav className="flex gap-6 text-muted-foreground">
          <a className="transition-colors hover:text-foreground" href="#story">
            how it works
          </a>
          <a className="transition-colors hover:text-foreground" href={`${PRODUCT.repo}/blob/main/SPEC.md`}>
            spec
          </a>
          <a className="transition-colors hover:text-foreground" href={PRODUCT.repo}>
            github
          </a>
        </nav>
      </header>

      {/* The hero's stage sticks while its extra height scrolls by: over it the lid shuts,
          and the file's text leaves the screen for the block below (components/hero). */}
      <section data-hero-section className="relative md:-mt-[4.5rem] md:h-[170svh]">
        <div className="relative isolate flex flex-col items-center gap-8 px-4 pt-6 md:sticky md:top-0 md:h-svh md:justify-center md:gap-10 md:pt-16">
          <div className="max-w-3xl text-center">
            <h1 className="text-balance text-[clamp(2.25rem,4.6vw,3.75rem)] leading-[1.03] font-medium tracking-[-0.04em] text-foreground">
              {PRODUCT.tagline}
            </h1>
            <p className="mx-auto mt-5 max-w-[50ch] text-pretty text-[16px] leading-relaxed text-foreground/70 md:text-[17px]">
              {PRODUCT.subline.split(PRODUCT.extension).map((part, i) =>
                i ? (
                  <Fragment key={part}>
                    <span className="font-mono text-edge">{PRODUCT.extension}</span>
                    {part}
                  </Fragment>
                ) : (
                  part
                ),
              )}
            </p>
          </div>
          <div className="relative w-[min(72rem,94vw,calc((100svh-17rem)*1.6))]">
            {/* A soft rim of light behind the machine, neutral: a product shot at night. */}
            <div aria-hidden className="absolute inset-x-[14%] top-[4%] bottom-[22%] -z-10 rounded-[40%] bg-white/5 blur-[80px] dark:bg-white/6" />
            {/* The engine streams in; the server's HTML of the hero and the story stays on
                screen until it has (components/use-engine). */}
            <Suspense>
              <Hero />
            </Suspense>
          </div>
        </div>
      </section>

      {/* The file the hero's screen let go of lands here first, so its block leads and its
          words follow: nothing stands between the screen and where the text lands. */}
      <section id="story" className="mx-auto grid max-w-6xl scroll-mt-20 items-center gap-10 px-6 md:mt-8 md:grid-cols-[auto_minmax(0,1fr)] md:gap-16">
        {/* The editor window the Mac let go of, the same window: its two files a tab each;
            beside it, what it showed. */}
        <Suspense>
          <Landing />
        </Suspense>
        {/* Glass, as the shimmer's own demo sets words over it: its blur frosts the
            facets behind, the words in front stay sharp. */}
        <Glass shape="surface" className="max-w-[52ch] p-8 [--glass-radius:1.25rem]">
          <p className="font-mono text-[11px] tracking-[0.18em] text-muted-foreground/70">TRY IT</p>
          <h2 className="mt-4 text-balance text-[clamp(1.5rem,2.6vw,2.125rem)] leading-[1.15] font-medium tracking-[-0.02em]">
            Files that check each other.
          </h2>
          <div className="mt-6 space-y-4 text-[16px] leading-relaxed text-muted-foreground">
            <p>
              The header holds the types, the formulas and the links to other files. Every value is checked against them as you type, across files.
            </p>
            <p className="text-foreground/80">
              Change <code className="font-mono text-foreground">rent</code> to <code className="font-mono text-foreground">rant</code> in the editor.
            </p>
          </div>
        </Glass>
      </section>

      <div className="mt-24 md:mt-32">
        <Suspense>
          <Story />
        </Suspense>
      </div>

      <section className="mx-auto mt-48 max-w-6xl px-6">
        <p className="font-mono text-[11px] tracking-[0.18em] text-muted-foreground/70">AND NOT</p>
        <h2 className="mt-4 text-balance text-[clamp(1.75rem,3.2vw,2.5rem)] leading-[1.1] font-medium tracking-[-0.025em]">
          Why not what exists. <span className="text-muted-foreground">Each is good at something else.</span>
        </h2>
        <dl className="mt-12 divide-y divide-border border-y border-border">
          {ROWS.map((r) => (
            <div key={r.them} className="grid gap-2 py-6 md:grid-cols-[14rem_1fr_1fr] md:gap-8">
              <dt className="font-medium">{r.them}</dt>
              <dd className="text-[15px] leading-relaxed text-muted-foreground">{r.lack}</dd>
              <dd className="text-[15px] leading-relaxed">{r.cells}</dd>
            </div>
          ))}
        </dl>
      </section>

      <section className="mx-auto mt-48 max-w-6xl px-6">
        <p className="font-mono text-[11px] tracking-[0.18em] text-muted-foreground/70">WHERE IT STANDS</p>
        <div className="mt-8 grid gap-12 md:grid-cols-2">
          <div>
            <h3 className="text-lg font-medium">Built</h3>
            <ul className="mt-4 space-y-2.5 text-[15px] leading-relaxed text-muted-foreground">
              <li>The spec, and a conformance suite every reader runs.</li>
              <li>The engine in Rust, passing all of it natively and as WebAssembly: what runs on this page.</li>
              <li>Formulas in headers and cells, tables written inline, imports (a URL's pinned by its hash), references checked across files.</li>
              <li>
                <code className="font-mono text-foreground/80">{PRODUCT.cli} check</code>, <code className="font-mono text-foreground/80">fix</code>, <code className="font-mono text-foreground/80">show</code>, <code className="font-mono text-foreground/80">pin</code>, and <code className="font-mono text-foreground/80">run</code> with Bun.
              </li>
              <li>
                <code className="font-mono text-foreground/80">{PRODUCT.cli} lsp</code>: the problems you saw on this page, in your editor, with hover, completion, go to, rename, and every computed value stored as you save. Checked in Helix.
              </li>
              <li>
                <code className="font-mono text-foreground/80">{PRODUCT.cli} merge</code>, git's merge driver: a computed value is never a conflict.
              </li>
              <li>The engine bound on the web (the tables on this page) and in Swift, on macOS, iOS and tvOS.</li>
            </ul>
          </div>
          <div>
            <h3 className="text-lg font-medium">Next</h3>
            <ul className="mt-4 space-y-2.5 text-[15px] leading-relaxed text-muted-foreground">
              <li>
                <code className="font-mono text-foreground/80">{PRODUCT.cli} sql</code>, DuckDB over the typed columns of a folder.
              </li>
              <li>A daemon that keeps the schedules and serves every app.</li>
              <li>Charts from the types, with no setup.</li>
            </ul>
          </div>
        </div>
      </section>

      <footer className="mx-auto mt-48 flex max-w-6xl flex-col gap-3 border-t border-border px-6 py-10 text-[13px] text-muted-foreground md:flex-row md:justify-between">
        <span>
          {PRODUCT.name} · {PRODUCT.tagline}
        </span>
        <span className="flex gap-6">
          <a className="hover:text-foreground" href={`${PRODUCT.repo}/blob/main/SPEC.md`}>
            spec
          </a>
          <a className="hover:text-foreground" href={PRODUCT.repo}>
            github
          </a>
        </span>
      </footer>
    </main>
  )
}
