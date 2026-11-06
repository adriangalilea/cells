// Code's colours, character by character: the same highlighter `code` renders with
// (Shiki, VS Code's own grammars), read as one style per character instead of as
// markup, for a view that draws text which is not all there yet (an editor whose
// file is being typed: each character keeps the colour it has in the finished file,
// so a half-typed attribute already reads as one). Framework-free; Shiki loads on the
// first call, so the page ships none of it until something asks, and then only its
// core, the JavaScript regex engine and the one grammar and two themes asked for.

/** The two themes every code view here wears, light and dark, picked by the scheme
 *  it stands in. `code` and `markdown` render with them. */
export const CODE_THEMES = {
  light: "github-light",
  dark: "github-dark",
} as const

/** VS Code's default themes' token colours, which `ide` wears: Dark Modern and
 *  Light Modern are built on Dark+ and Light+. */
export const VSCODE_THEMES = {
  light: "light-plus",
  dark: "dark-plus",
} as const

/** Lines to mark, as every code view here spells them: `"2"` or `"1,4-6"`, counted
 *  from 1. Screams at anything else. */
export function linesOf(spec: string): Set<number> {
  const out = new Set<number>()
  for (const part of spec.split(",")) {
    const range = part.trim()
    if (!range) continue
    const [a, b = a] = range.split("-").map((n) => Number(n.trim()))
    if (!Number.isInteger(a) || !Number.isInteger(b) || a === undefined)
      throw new Error(`code: "${range}" is not a line or a range of lines`)
    for (let n = a; n <= (b as number); n++) out.add(n)
  }
  return out
}

/** A pair of Shiki theme ids, light and dark. */
export interface CodeThemes {
  light: string
  dark: string
}

/** One style a character can wear, in both themes. */
export interface CodeInk {
  light: string
  dark: string
  italic: boolean
  bold: boolean
}

/** A text's colours: `inks` the distinct styles, `of[i]` the one character `i` of
 *  the text wears (an index into `inks`). */
export interface CodeColours {
  inks: CodeInk[]
  of: Uint16Array
}

/** A language Shiki does not bundle, told to code: its grammar, loaded the first time
 *  the language is asked for (Shiki's `LanguageInput`: one registration whose `name`
 *  is the language's id, or a list, the languages it embeds first), and the other
 *  names a file's extension or a fence calls it by. */
export interface CodeLanguage {
  grammar: () => Promise<
    | import("shiki").LanguageRegistration
    | import("shiki").LanguageRegistration[]
  >
  /** `cell` for `cells`: what `money.cell` and a ```` ```cell ```` fence say. */
  aliases?: readonly string[]
  /** What the language's own scopes wear in every theme, over its colours: a Cells
   *  header's names bold. Each a TextMate scope selector, which a theme's rule for a
   *  deeper scope still wins over. */
  styles?: readonly { scope: string; bold?: boolean; italic?: boolean }[]
}

/** The languages code colours beyond Shiki's bundle, by id, and every alias to its
 *  id, filled by `registerLanguage`. `cells` is code's own: the Cells grammar (lib/cells-grammar, vendored from
 *  github.com/adriangalilea/cells), its columns in rainbow and its types by scope. */
const LANGUAGES = new Map<string, CodeLanguage>()
const ALIASES = new Map<string, string>()

/** Tells code a language Shiki does not bundle, so every view that picks a language
 *  from a file's name or a fence (`code`, `markdown`, `walkthrough`, `ide`,
 *  `terminal-editor`) colours it. Call it at the top of a module the pages that draw
 *  it import, once in each place code runs (the server for `Code`, the browser for an
 *  editor): a language registered in neither is drawn plain. Registering an id again
 *  replaces its grammar (a module evaluated again in development); an alias another
 *  language holds, or one that is a registered id, throws. */
export function registerLanguage(id: string, language: CodeLanguage): void {
  const lower = id.toLowerCase()
  if (lower !== id)
    throw new Error(`code-tokens: a language id is lowercase, not ${id}`)
  if (ALIASES.has(id))
    throw new Error(
      `code-tokens: ${id} is already an alias of ${ALIASES.get(id)}`,
    )
  for (const alias of language.aliases ?? []) {
    const held = ALIASES.get(alias)
    if (
      (held !== undefined && held !== id) ||
      (alias !== id && LANGUAGES.has(alias))
    )
      throw new Error(
        `code-tokens: the alias ${alias} of ${id} already names ${held ?? alias}`,
      )
  }
  for (const [alias, of] of ALIASES) if (of === id) ALIASES.delete(alias)
  LANGUAGES.set(id, language)
  for (const alias of language.aliases ?? []) ALIASES.set(alias, id)
  if (language.styles?.length) styled++
}

/** Bumped whenever a language brings styles: a theme loaded before is loaded again,
 *  wearing them. */
let styled = 0

/** Every registered language's styles, as a theme's rules. */
const styleRules = () =>
  [...LANGUAGES.values()].flatMap((l) =>
    (l.styles ?? []).map((s) => ({
      scope: s.scope,
      settings: {
        fontStyle: [s.bold && "bold", s.italic && "italic"]
          .filter(Boolean)
          .join(" "),
      },
    })),
  )

registerLanguage("cells", {
  grammar: () =>
    import("@/lib/cells-grammar").then(
      (m) => m.cellsGrammar,
    ),
  aliases: ["cell"],
  // A header's names, each `meta.column.<n>` (its rainbow colour), bold.
  styles: [{ scope: "meta.header.cells meta.column", bold: true }],
})

/** The id a language name stands for: a registered language's alias resolved
 *  (`cell` is `cells`), any other name as it is (Shiki reads its own aliases). */
export const languageOf = (name: string): string => {
  const lower = name.toLowerCase()
  return ALIASES.get(lower) ?? lower
}

/** The language a path is written in: its extension (`html`, `ts`, `md`, `cell`)
 *  read through `languageOf`, which Shiki reads as a language or one of its aliases;
 *  `text` with none. */
export function langOf(path: string): string {
  const ext = /\.([a-z0-9]+)$/i.exec(path)?.[1]
  return ext ? languageOf(ext) : "text"
}

/** Whether `lang` (an id or an alias) is a registered language. */
export const registeredLanguage = (lang: string) =>
  LANGUAGES.has(languageOf(lang))

/** The grammar loaded for `id`, which must name its language `id`: a grammar called
 *  anything else would load and never be found. */
async function grammarOf(id: string, language: CodeLanguage) {
  const g = await language.grammar()
  const list = Array.isArray(g) ? g : [g]
  if (!list.some((r) => r.name === id))
    throw new Error(
      `code-tokens: the grammar registered as ${id} names itself ${list.map((r) => r.name).join(", ")}`,
    )
  return list
}

/** A registered language's grammar (an id or an alias), loaded and checked;
 *  undefined for any other. */
export const ownGrammar = (lang: string) => {
  const id = languageOf(lang)
  const language = LANGUAGES.get(id)
  return language ? grammarOf(id, language) : undefined
}

type Core = import("shiki/core").HighlighterCore

/** One highlighter for the page, made on the first call: Shiki's core with its
 *  JavaScript regex engine, never the WebAssembly one (Oniguruma, 600 KB a browser
 *  would download to colour a few lines), each language and theme loaded the first
 *  time it is asked for, alone. */
let core: Promise<Core> | undefined
async function highlighter(lang: string, themes: CodeThemes): Promise<Core> {
  core ??= (async () => {
    const [{ createHighlighterCore }, { createJavaScriptRegexEngine }] =
      await Promise.all([
        import("shiki/core"),
        import("shiki/engine/javascript"),
      ])
    return createHighlighterCore({
      engine: createJavaScriptRegexEngine(),
      themes: [],
      langs: [],
    })
  })()
  const h = await core
  const [{ bundledLanguages }, { bundledThemes }] = await Promise.all([
    import("shiki/langs"),
    import("shiki/themes"),
  ])
  const loading: Promise<void>[] = []
  if (lang !== "text" && !h.getLoadedLanguages().includes(lang)) {
    const bundled = bundledLanguages[lang as keyof typeof bundledLanguages]
    const own = ownGrammar(lang)
    if (bundled) loading.push(h.loadLanguage(bundled))
    else if (own) loading.push(own.then((g) => h.loadLanguage(...g)))
  }
  for (const name of [themes.light, themes.dark]) {
    let worn = wearing.get(name)
    if (worn?.styled !== styled) {
      const theme = bundledThemes[name as keyof typeof bundledThemes]
      if (!theme) throw new Error(`code-tokens: no Shiki theme ${name}`)
      worn = {
        styled,
        loaded: theme().then(({ default: t }) =>
          h.loadTheme({
            ...t,
            tokenColors: [
              ...(t.tokenColors ?? t.settings ?? []),
              ...styleRules(),
            ],
          }),
        ),
      }
      wearing.set(name, worn)
    }
    // A call made while the theme loads waits for the same load.
    loading.push(worn.loaded)
  }
  await Promise.all(loading)
  return h
}

/** Each theme's load, and the languages' styles it wears (`styled` when it began). */
const wearing = new Map<string, { styled: number; loaded: Promise<void> }>()

/** Every character of `text` coloured by `lang`'s grammar in both `themes`. A
 *  language Shiki does not know is `text`: one ink, the theme's foreground. Runs
 *  where it is called: on the server for a still (passed to a view as its
 *  `colours`), or in the browser for text that changes. */
export async function codeColours(
  text: string,
  name: string,
  themes: CodeThemes = CODE_THEMES,
): Promise<CodeColours> {
  const lang = languageOf(name)
  const h = await highlighter(lang, themes)
  const lines = h.codeToTokensWithThemes(text, {
    lang: h.getLoadedLanguages().includes(lang) ? lang : "text",
    themes: { light: themes.light, dark: themes.dark },
  })
  const inks: CodeInk[] = []
  const known = new Map<string, number>()
  const of = new Uint16Array(text.length)
  for (const line of lines)
    for (const token of line) {
      const light = token.variants.light
      const dark = token.variants.dark
      if (!light || !dark)
        throw new Error("code-tokens: a token without both themes' colours")
      // Shiki's FontStyle flags: 1 italic, 2 bold.
      const style = light.fontStyle ?? 0
      const ink: CodeInk = {
        light: light.color ?? "inherit",
        dark: dark.color ?? "inherit",
        italic: (style & 1) !== 0,
        bold: (style & 2) !== 0,
      }
      const key = `${ink.light} ${ink.dark} ${style}`
      let n = known.get(key)
      if (n === undefined) {
        n = inks.length
        inks.push(ink)
        known.set(key, n)
      }
      of.fill(n, token.offset, token.offset + token.content.length)
    }
  return { inks, of }
}

/** A stretch of a line in one ink (null: the view's own foreground, before the
 *  colours have loaded). */
export interface CodeRun {
  text: string
  ink: CodeInk | null
}

/** A run with what lies over it: the marks of every range it falls in. */
export interface CodePaint<M> extends CodeRun {
  marks: M[]
}

/** A line's runs cut again where `ranges` begin and end (offsets of the whole text;
 *  the line begins at `start`), each piece carrying the marks of the ranges over it:
 *  a diagnostic's squiggle, a selection, a flash. */
export function paintRuns<M>(
  runs: readonly CodeRun[],
  start: number,
  ranges: readonly { from: number; to: number; mark: M }[],
): CodePaint<M>[] {
  const end = start + runs.reduce((n, r) => n + r.text.length, 0)
  const here = ranges.filter((r) => r.from < end && r.to > start)
  if (!here.length) return runs.map((r) => ({ ...r, marks: [] }))
  const cuts = new Set<number>()
  for (const r of here) {
    cuts.add(Math.max(start, r.from))
    cuts.add(Math.min(end, r.to))
  }
  const out: CodePaint<M>[] = []
  let at = start
  for (const run of runs) {
    const runEnd = at + run.text.length
    const points = [
      at,
      ...[...cuts].filter((c) => c > at && c < runEnd).sort((a, b) => a - b),
      runEnd,
    ]
    for (let i = 0; i + 1 < points.length; i++) {
      const a = points[i] as number
      const b = points[i + 1] as number
      out.push({
        text: run.text.slice(a - at, b - at),
        ink: run.ink,
        marks: here.filter((r) => r.from <= a && b <= r.to).map((r) => r.mark),
      })
    }
    at = runEnd
  }
  return out
}

/** `text` cut into lines of runs, each character inked by `colours` at its offset
 *  in the text they were read from: `origin[i]` for character `i` (a file being
 *  typed, coloured as it will be), or `i` itself without one. */
export function runsOf(
  text: string,
  colours: CodeColours | null,
  origin?: readonly number[],
): CodeRun[][] {
  const lines: CodeRun[][] = [[]]
  let line = lines[0] as CodeRun[]
  let run: CodeRun | null = null
  for (let i = 0; i < text.length; i++) {
    const ch = text[i] as string
    if (ch === "\n") {
      line = []
      lines.push(line)
      run = null
      continue
    }
    const at = origin ? (origin[i] as number) : i
    const ink = colours
      ? (colours.inks[colours.of[at] as number] ?? null)
      : null
    if (run && run.ink === ink) run.text += ch
    else {
      run = { text: ch, ink }
      line.push(run)
    }
  }
  return lines
}
