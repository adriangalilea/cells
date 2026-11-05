import { Hero } from "@/components/hero"
import { Story } from "@/components/story"
import { Code } from "@/components/ui/code"
import { HERO } from "@/lib/hero"
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
    lack: "formulas point at B7, which moves when a row is inserted; macros run when a file is opened",
    cells: "formulas name columns and keys; opening a file runs nothing",
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

      <section className="hero-camera relative md:-mt-[4.5rem] md:h-[190svh]">
        <div className="flex flex-col items-center gap-8 px-4 pt-6 md:sticky md:top-0 md:h-svh md:justify-center md:gap-10 md:overflow-hidden md:pt-16">
          <div className="hero-words max-w-3xl text-center">
            <h1 className="text-balance text-[clamp(2.25rem,4.6vw,3.75rem)] leading-[1.03] font-medium tracking-[-0.04em]">
              {PRODUCT.tagline}
            </h1>
            <p className="mx-auto mt-5 max-w-[54ch] text-pretty text-[16px] leading-relaxed text-muted-foreground md:text-[17px]">
              A <span className="font-mono text-foreground/85">{PRODUCT.extension}</span> file is a table that says what its columns are, computes what it can, and knows the files it points at. Plain text, checked by every reader.
            </p>
          </div>
          <div className="hero-device relative w-[min(72rem,94vw,calc((100svh-17rem)*1.6))]">
            <div aria-hidden className="absolute inset-x-[14%] top-[4%] bottom-[22%] -z-10 rounded-[40%] bg-edge/12 blur-[80px]" />
            <Hero />
          </div>
        </div>
      </section>

      <section id="story" className="mx-auto mt-32 max-w-6xl scroll-mt-20 px-6 md:mt-48">
        <p className="font-mono text-[11px] tracking-[0.18em] text-muted-foreground/70">THE FILE ON THAT SCREEN</p>
        <h2 className="mt-4 max-w-[24ch] text-balance text-[clamp(1.75rem,3.2vw,2.5rem)] leading-[1.1] font-medium tracking-[-0.025em]">
          This is a cell. <span className="text-muted-foreground">Everything it does is in its text.</span>
        </h2>
        <div className="mt-10 overflow-hidden rounded-xl border border-border bg-card/70">
          <Code lang="cell" filename={HERO.file} copy>
            {HERO.checkpoints.fixed.text.trimEnd()}
          </Code>
        </div>
      </section>

      <div className="mt-40 md:mt-56">
        <Story />
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
              <li>Formulas in headers and cells, tables written inline, imports pinned by hash, references checked across files.</li>
              <li>
                <code className="font-mono text-foreground/80">{PRODUCT.cli} check</code>, <code className="font-mono text-foreground/80">show</code>, <code className="font-mono text-foreground/80">pin</code>, and <code className="font-mono text-foreground/80">run</code> with Bun.
              </li>
              <li>The table you saw, on the web and on Apple platforms.</li>
            </ul>
          </div>
          <div>
            <h3 className="text-lg font-medium">Being built, then next</h3>
            <ul className="mt-4 space-y-2.5 text-[15px] leading-relaxed text-muted-foreground">
              <li>
                <code className="font-mono text-foreground/80">{PRODUCT.cli} lsp</code>, being built now: the problems you saw on this page, in your editor, with hover, completion, go to and rename. Helix first.
              </li>
              <li>A daemon that keeps the schedules and serves every app.</li>
              <li>
                <code className="font-mono text-foreground/80">{PRODUCT.cli} sql</code>, DuckDB over the typed columns of a folder.
              </li>
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
