"use client"

// THE STORY'S STEPS, drawn from lib/story: each one idea, its words on one side and its
// files on the other, the first file editable and read by the engine at every keystroke
// into what it becomes. The step list is the whole content; a walkthrough player can draw
// the same list (its steps: words, a file state, a live view).

import * as React from "react"
import { CellEditor } from "@/components/cell-editor"
import { CellsTable } from "@/components/cells-window"
import { useEngine } from "@/components/use-engine"
import { readFiles, type ViewTable } from "@/lib/cells"
import { PRODUCT } from "@/lib/product"
import { STORY, type StoryFile, type StoryStep } from "@/lib/story"
import { cn } from "@/lib/utils"

export function Story() {
  return (
    <div className="space-y-40 md:space-y-56">
      {STORY.map((step, i) => (
        <Step key={step.id} step={step} index={i} />
      ))}
    </div>
  )
}

function Step({ step, index }: { step: StoryStep; index: number }) {
  const [files, setFiles] = React.useState<StoryFile[]>(step.files)
  const [open, setOpen] = React.useState(0)
  const engine = useEngine()
  const read = React.useMemo(() => readFiles(engine, files), [engine, files])
  const file = files[open] as StoryFile
  // The file shown in the editor is judged as the reader would open it: the first through
  // its imports, any other alone.
  const judged = React.useMemo(() => (open === 0 ? read : readFiles(engine, [file])), [engine, open, read, file])
  const edit = (text: string) => setFiles((fs) => fs.map((f, k) => (k === open ? { ...f, text } : f)))
  return (
    <section id={step.id} aria-labelledby={`${step.id}-title`} className="mx-auto grid max-w-6xl scroll-mt-24 gap-10 px-6 lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)] lg:gap-16">
      <div className="lg:sticky lg:top-28 lg:self-start">
        <p className="font-mono text-[11px] tracking-[0.18em] text-muted-foreground/70">{String(index + 1).padStart(2, "0")}</p>
        <h2 id={`${step.id}-title`} className="mt-4 text-balance text-[clamp(1.75rem,3.2vw,2.5rem)] leading-[1.1] font-medium tracking-[-0.025em]">
          {step.lead} <span className="text-muted-foreground">{step.rest}</span>
        </h2>
        <p className="mt-6 max-w-[42ch] text-pretty text-[15px] leading-relaxed text-muted-foreground">{step.why}</p>
        {step.reads && (
          <dl className="mt-6 max-w-[44ch] space-y-2 text-[14px]">
            {step.reads.map((r) => (
              <div key={r.formula} className="grid grid-cols-[minmax(0,11rem)_1fr] gap-3">
                <dt className="font-mono text-[12.5px] text-foreground/90">{r.formula.replaceAll("`", "")}</dt>
                <dd className="text-muted-foreground">{r.says}</dd>
              </div>
            ))}
          </dl>
        )}
        {step.soon && (
          <p className="mt-5 max-w-[44ch] text-pretty text-[13px] leading-relaxed text-muted-foreground/80">
            <span className="font-mono text-[11px] tracking-[0.14em] text-haze/80">COMING </span>
            <Ticks text={step.soon} />
          </p>
        )}
        {step.try && (
          <p className="mt-6 font-mono text-[12px] text-edge/90">
            <span className="text-muted-foreground/60">try </span>
            {step.try.replaceAll("`", "")}
          </p>
        )}
      </div>
      <div className="min-w-0 space-y-4">
        <Pane>
          {files.length > 1 && (
            <div role="tablist" aria-label="files" className="flex gap-1 border-b border-border px-2 pt-2">
              {files.map((f, k) => (
                <button
                  key={f.name}
                  role="tab"
                  type="button"
                  aria-selected={k === open}
                  onClick={() => setOpen(k)}
                  className={cn(
                    "rounded-t-md px-3 py-1.5 font-mono text-[11.5px] transition-colors",
                    k === open ? "bg-foreground/6 text-foreground" : "text-muted-foreground hover:text-foreground",
                  )}
                >
                  {f.name}
                </button>
              ))}
            </div>
          )}
          {files.length === 1 && <FileName name={file.name} />}
          <CellEditor name={file.name} value={file.text} onChange={edit} problems={judged.problems} label={`edit ${file.name}`} />
        </Pane>
        <View step={step} files={files} read={read} />
      </div>
    </section>
  )
}

/** Prose whose `backticked` parts are code, set in the mono face. */
function Ticks({ text }: { text: string }) {
  return text.split(/(`[^`]+`)/).map((part, i) =>
    part.startsWith("`") ? (
      // biome-ignore lint/suspicious/noArrayIndexKey: the parts of one fixed string, in order
      <code key={i} className="font-mono text-[12px] text-foreground/80">
        {part.slice(1, -1)}
      </code>
    ) : (
      part
    ),
  )
}

function Pane({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn("overflow-hidden rounded-xl border border-border bg-card/70 shadow-[0_1px_0_0_rgb(255_255_255/0.04)_inset] backdrop-blur-sm", className)}>
      {children}
    </div>
  )
}

function FileName({ name, note }: { name: string; note?: string }) {
  return (
    <div className="flex items-center justify-between border-b border-border px-4 py-2 font-mono text-[11.5px] text-muted-foreground">
      <span>{name}</span>
      {note && <span className="text-muted-foreground/60">{note}</span>}
    </div>
  )
}

function Problems({ read }: { read: ViewTable }) {
  if (read.error) return <p className="px-4 py-3 font-mono text-[12px] text-haze">{read.error}</p>
  if (read.problems.length === 0)
    return <p className="px-4 py-3 font-mono text-[12px] text-edge/80">every cell holds its type</p>
  return (
    <ul className="space-y-1 px-4 py-3 font-mono text-[12px] text-haze">
      {read.problems.map((p) => (
        <li key={`${p.row}:${p.at}`}>{p.why}</li>
      ))}
    </ul>
  )
}

function View({ step, files, read }: { step: StoryStep; files: StoryFile[]; read: ViewTable }) {
  const [first, ...others] = files as [StoryFile, ...StoryFile[]]
  if (step.view.kind === "query") return <Query files={files} />
  if (step.view.kind === "run") return <Run file={first} />
  return (
    <Pane>
      <FileName name={`what ${PRODUCT.name} reads`} note={step.view.kind === "linked" ? `${first.name} with its imports` : undefined} />
      <CellsTable name={first.name} text={first.text} others={others} label={`${first.name}, read`} className="max-h-72" />
      <div className="border-t border-border">
        <Problems read={read} />
      </div>
    </Pane>
  )
}

/** A script's table, as the file keeps it: the rows its last run stored and its runs, from
 *  the file's own fences. Nothing runs here. */
function Run({ file }: { file: StoryFile }) {
  const fence = (lang: string) => {
    const m = new RegExp("```" + lang + "\\n([\\s\\S]*?)\\n```").exec(file.text)
    return m ? `${m[1]}\n` : ""
  }
  const stored = fence("cells")
  const runs = fence("cells runs")
  return (
    <div className="grid gap-4 md:grid-cols-2">
      <Pane>
        <FileName name="eurusd, as stored" note="last run 12" />
        <CellsTable name="eurusd.cell" text={stored} label="the rows the last run stored" />
      </Pane>
      <Pane>
        <FileName name="its runs" note="recorded" />
        <CellsTable name="runs.cell" text={runs} label="the script's runs" />
      </Pane>
      <Pane className="md:col-span-2">
        <FileName name={`${file.name}, read`} note="eur from each day's rate" />
        <CellsTable name={file.name} text={file.text} label={`${file.name}, read`} />
        <p className="border-t border-border px-4 py-3 text-[12.5px] text-muted-foreground">
          These runs are recorded: this page runs no script. <code className="font-mono text-foreground/80">{PRODUCT.cli} run {file.name}</code> runs it, with Bun, when you ask.
        </p>
      </Pane>
    </div>
  )
}

/** The folder asked a question: every movement, by the kind of the account it went to.
 *  Answered here from the engine's typed columns; `cells sql` (DuckDB over the same
 *  columns) is next. */
function Query({ files }: { files: StoryFile[] }) {
  const accounts = files.find((f) => f.name.startsWith("accounts")) as StoryFile
  const money = files.filter((f) => f !== accounts)
  const engine = useEngine()
  const kinds = readFiles(engine, [accounts])
  const kindOf = new Map<string, string>()
  kinds.values.account?.forEach((a, i) => kindOf.set(String(a), String(kinds.values.kind?.[i])))
  const sums = new Map<string, number>()
  for (const m of money) {
    const t = readFiles(engine, [m, ...files.filter((f) => f !== m)])
    t.values.to?.forEach((to, i) => {
      const kind = kindOf.get(String(to)) ?? "unknown"
      sums.set(kind, (sums.get(kind) ?? 0) + Number(t.values.amount?.[i] ?? 0))
    })
  }
  const answer = `kind:text!,total:EUR\n${[...sums].map(([k, v]) => `${k},${v.toFixed(2)}`).join("\n")}\n`
  return (
    <Pane>
      <FileName name="every money file, by where it went" note="direction: cells sql" />
      <pre className="overflow-x-auto px-4 py-3 font-mono text-[12.5px] leading-relaxed text-foreground/85">
        {`$ ${PRODUCT.cli} sql . "select a.kind, sum(m.amount) from money m
    join accounts a on m.to = a.account group by a.kind"`}
      </pre>
      <div className="border-t border-border">
        <CellsTable name="answer.cell" text={answer} label="the answer" />
      </div>
      <p className="border-t border-border px-4 py-3 text-[12.5px] text-muted-foreground">
        Answered here from the engine's typed columns of {money.map((m) => m.name).join(" and ")}. <code className="font-mono text-foreground/80">{PRODUCT.cli} sql</code>, DuckDB over the same columns, is what comes next.
      </p>
    </Pane>
  )
}
