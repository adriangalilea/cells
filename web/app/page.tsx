import { Fragment, Suspense } from "react"
import { Ground } from "@/components/ground"
import { Hero, Landing } from "@/components/hero"
import { Playground } from "@/components/playground"
import { CurrentScene, LinkedScene } from "@/components/reveal"
import { SiteFooter } from "@/components/site-footer"
import { SiteHeader } from "@/components/site-header"
import { Story } from "@/components/story"
import { Code as Block } from "@/components/ui/code"
import { Glass } from "@/components/ui/material-glass"
import { PRODUCT } from "@/lib/product"
import { ONE_FILE, PROOFS } from "@/lib/proofs"

/** What each neighbour does, honestly, and what a Cells file does instead. */
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

/** Something written in a file, as the words beside it name it. */
function Spelled({ children }: { children: React.ReactNode }) {
  return <code className="font-mono text-[0.92em] text-foreground">{children}</code>
}

/** One tool of the three: its name, what it is good at, and where it stops. */
function Beat({ name, good, but }: { name: string; good: string; but: string }) {
  return (
    <div className="max-w-[40ch]">
      <h2 className="text-[clamp(1.75rem,3vw,2.5rem)] leading-[1.1] font-medium tracking-[-0.03em]">{name}</h2>
      <p className="mt-4 text-[17px] leading-relaxed text-foreground/85">{good}</p>
      <p className="mt-3 text-[17px] leading-relaxed text-muted-foreground">{but}</p>
    </div>
  )
}

/** A section's small capitals over its heading. */
function Eyebrow({ children }: { children: React.ReactNode }) {
  return <p className="font-mono text-[11px] tracking-[0.18em] text-muted-foreground/70 uppercase">{children}</p>
}

/** What it opens up, each shown, not described: what a reader gets, and the proof. */
const LEVERS: { title: string; says: React.ReactNode; proof?: (typeof PROOFS)[keyof typeof PROOFS] }[] = [
  {
    title: "A folder is a database",
    says: (
      <>
        Files point at files. One command checks every link across the folder, and a key renamed in one file is renamed in every
        file that uses it.
      </>
    ),
    proof: PROOFS.folder,
  },
  {
    title: "Git understands your numbers",
    says: (
      <>
        Two people add rows to a running total on two branches. The merge is clean and every total is right: a computed value is
        never a conflict.
      </>
    ),
    proof: PROOFS.git,
  },
  {
    title: "Live data, still plain text",
    says: <>A script fills a table on a schedule, and the file keeps what it fetched: the rates are there to read with nothing running.</>,
    proof: PROOFS.live,
  },
  {
    title: "Your editor knows the data",
    says: (
      <>
        A broken value named as you type, hover to see how a value is computed, a key completed from another file, a rename
        followed across the folder. <Spelled>{PRODUCT.cli} lsp</Spelled>, in Helix and Neovim today.
      </>
    ),
  },
  {
    title: "Safe to hand to an agent",
    says: <>An AI edits it like any text file, and the engine names every value it breaks, at its row, before anyone relies on it.</>,
  },
  {
    title: "Nothing runs when you open it",
    says: <>A formula reads no clock, network or file. A script runs only when told to. Opening a file is reading it.</>,
  },
]

/** Where a Cells file opens today, and what comes next. */
const PLATFORMS: { name: string; now: boolean }[] = [
  { name: `The command line: ${PRODUCT.cli} check, fmt, merge, lsp`, now: true },
  { name: "Helix and Neovim, through the language server", now: true },
  { name: "Cells.app, on the Mac", now: true },
  { name: "Swift, on macOS, iOS and tvOS", now: true },
  { name: "The web: the engine as WebAssembly, what runs on this page", now: true },
  { name: "A VS Code extension", now: false },
  { name: "The npm package", now: false },
  { name: "Prebuilt binaries", now: false },
  { name: "Highlighting on GitHub", now: false },
]

/** Three things to do to the file in the window, each one word of the line. */
const TRIES = [
  {
    word: "Typed",
    code: "cost:EUR>=0",
    says: "holds euros, never below zero.",
    act: (
      <>
        Make a cost negative.
      </>
    ),
  },
  {
    word: "Reactive",
    code: "share:EUR=cost / 4",
    says: "is computed for every row and written beside its =.",
    act: (
      <>
        Change the flights to <Spelled>500.00</Spelled> and watch the share and the total follow.
      </>
    ),
  },
  {
    word: "Composable",
    code: "paid:people.name",
    says: `names a friend in people${PRODUCT.extension}, checked across the two files.`,
    act: <>Say the taxi was paid by someone who was not on the trip.</>,
  },
]

export default function Page() {
  return (
    <main>
      <Ground />
      <SiteHeader />

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

      {/* Three tools, one at a time, each shown doing what it is good at and said where it
          stops; then the one file they are. The first is the window the hero's screen let
          go of, landing here. */}
      <section className="mx-auto grid max-w-6xl items-center gap-10 px-6 md:mt-8 md:grid-cols-[auto_minmax(0,1fr)] md:gap-16">
        <Suspense>
          <Landing />
        </Suspense>
        <Beat
          name="A spreadsheet."
          good="Type a number and every total follows. You can see into it and play with it."
          but="But its formulas point at B7, and its file opens in one app."
        />
      </section>

      <section className="mx-auto mt-32 grid max-w-6xl items-center gap-10 px-6 md:grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)] md:gap-16">
        <Beat
          name="A database."
          good="Typed, linked and checked: a name that is no friend of the trip is caught where it is typed."
          but="But to every plain-text tool it is a black box: no grep, no diff, nothing git can merge."
        />
        <Suspense>
          <LinkedScene />
        </Suspense>
      </section>

      <section className="mx-auto mt-32 grid max-w-6xl items-center gap-10 px-6 md:grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)] md:gap-16">
        <Suspense>
          <CurrentScene />
        </Suspense>
        <Beat
          name="A script that keeps it current."
          good="The day's rate, fetched every evening at 18:00, there when you open the table."
          but="But it lives outside the data, on someone's machine, and only they know it runs."
        />
      </section>

      <section className="mx-auto mt-40 max-w-3xl px-6 text-center">
        <p className="text-[17px] text-muted-foreground">Three tools you use today.</p>
        <h2 className="mt-3 text-balance text-[clamp(2rem,4vw,3.25rem)] leading-[1.05] font-medium tracking-[-0.035em]">
          They are not three tools. It is one plain text file.
        </h2>
      </section>
      <div className="mx-auto mt-10 max-w-3xl px-6">
        <Block filename={ONE_FILE.name}>{ONE_FILE.text}</Block>
      </div>

      <div id="story" className="mt-24 scroll-mt-20 md:mt-32">
        <Suspense>
          <Story />
        </Suspense>
      </div>

      <section className="mx-auto mt-48 max-w-3xl px-6">
        <Eyebrow>The price</Eyebrow>
        <h2 className="mt-4 text-balance text-[clamp(2rem,4vw,3.25rem)] leading-[1.05] font-medium tracking-[-0.035em]">
          Unapologetically incompatible with CSV.
        </h2>
        <p className="mt-8 text-[18px] leading-relaxed text-foreground/85">
          Rename a <Spelled>.csv</Spelled> to <Spelled>{PRODUCT.extension}</Spelled> and you are in. From then on, tools that only read CSV
          will not read it. That is the price, and it is paid once.
        </p>
        <p className="mt-5 text-[17px] leading-relaxed text-muted-foreground">
          Formats that bend to stay compatible pay forever: every rule becomes a compromise, and what they get back is partial,
          inconsistent and confusing. Cells makes the cut once, and stays familiar: CSV's syntax, a spreadsheet's formulas, and the
          logic written inline or in a prelude, as Markdown has its frontmatter.
        </p>
        <a href="/docs" className="mt-6 inline-block text-[15px] text-muted-foreground underline decoration-border underline-offset-4 hover:text-foreground">
          The whole format, in the docs
        </a>
      </section>

      <section className="mx-auto mt-48 max-w-6xl px-6">
        <Eyebrow>What it opens</Eyebrow>
        <h2 className="mt-4 text-balance text-[clamp(1.75rem,3.2vw,2.5rem)] leading-[1.1] font-medium tracking-[-0.025em]">
          {PRODUCT.tagline} <span className="text-muted-foreground">What that buys you.</span>
        </h2>
        <div className="mt-14 grid gap-x-12 gap-y-14 md:grid-cols-2">
          {LEVERS.map((l) => (
            <div key={l.title}>
              <h3 className="text-[clamp(1.25rem,1.8vw,1.5rem)] leading-tight font-medium tracking-[-0.02em]">{l.title}</h3>
              <p className="mt-2 text-[15px] leading-relaxed text-muted-foreground">{l.says}</p>
              {l.proof && (
                <Block lang={l.proof.lang} className="mt-4">
                  {l.proof.text}
                </Block>
              )}
            </div>
          ))}
        </div>
        <p className="mt-16 text-[15px] leading-relaxed text-muted-foreground">
          Next: <Spelled>{PRODUCT.cli} sql</Spelled>, a folder asked as one database, and charts straight from the types.
        </p>
      </section>

      <section className="mx-auto mt-48 max-w-6xl px-6">
        <Eyebrow>Where it opens</Eyebrow>
        <ul className="mt-8 grid gap-x-12 gap-y-3 md:grid-cols-2">
          {PLATFORMS.map((p) => (
            <li key={p.name} className="flex gap-3 text-[15px] leading-relaxed">
              <span aria-hidden className={p.now ? "text-edge" : "text-muted-foreground/50"}>
                {p.now ? "✓" : "○"}
              </span>
              <span className={p.now ? "text-foreground/85" : "text-muted-foreground"}>
                {p.name}
                {p.now ? "" : ", coming"}
              </span>
            </li>
          ))}
        </ul>
        <p className="mt-8 text-[15px] leading-relaxed text-muted-foreground">
          And every file is plain text: open it in Vim, Helix or any editor you like.{" "}
          <a href="/docs/editors" className="underline decoration-border underline-offset-4 hover:text-foreground">
            Setting up an editor
          </a>
        </p>
      </section>

      <section className="mx-auto mt-48 grid max-w-6xl items-start gap-10 px-6 md:grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)] md:gap-14">
        <Suspense>
          <Playground />
        </Suspense>
        <Glass shape="surface" className="p-8 [--glass-radius:1.25rem]">
          <Eyebrow>Your turn</Eyebrow>
          <dl className="mt-6 space-y-6">
            {TRIES.map((t) => (
              <div key={t.word}>
                <dt className="text-[clamp(1.25rem,2vw,1.625rem)] leading-tight font-medium tracking-[-0.02em]">{t.word}</dt>
                <dd className="mt-1.5 text-[15px] leading-relaxed text-muted-foreground">
                  <Spelled>{t.code}</Spelled> {t.says}
                </dd>
                <dd className="mt-1 text-[15px] leading-relaxed text-foreground/85">{t.act}</dd>
              </div>
            ))}
          </dl>
          <p className="mt-8 text-[15px] text-muted-foreground">
            <a href="/docs" className="underline decoration-border underline-offset-4 hover:text-foreground">
              Read the docs
            </a>{" "}
            ·{" "}
            <a href={PRODUCT.repo} className="underline decoration-border underline-offset-4 hover:text-foreground">
              The source
            </a>
          </p>
        </Glass>
      </section>

      <section className="mx-auto mt-48 max-w-6xl px-6">
        <Eyebrow>And not</Eyebrow>
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

      <SiteFooter />
    </main>
  )
}
