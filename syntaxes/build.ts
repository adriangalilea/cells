// cells.tmLanguage.json, written from this file: the TextMate grammar for VS
// Code, GitHub (linguist) and Shiki. The regexes are composed here from
// named pieces so the shapes the spec defines are spelled once; the JSON is
// never edited by hand.
//
//   bun run build.ts
//
// Shapes TextMate cannot follow, said once here: a line is tokenised alone,
// so a quoted field holding a line break is coloured as a string across the
// break but the column count starts again after it; and the header is the
// first line after the prelude, found by nesting (the prelude's region holds
// the body's region, whose first line is the header). Typed colouring per
// column (a cell as the money or date its header says) is the language
// server's, as semantic tokens laid over these scopes.

import { writeFileSync } from "node:fs"
import { join } from "node:path"

const raw = String.raw

const BASES = ["text", "int", "num", "pct", "bool", "date", "time", "datetime", "url", "email"]

// ISO 4217's active codes, the same list the TypeScript reader and the
// tree-sitter grammar hold.
const CURRENCIES = (
  "AED AFN ALL AMD AOA ARS AUD AWG AZN BAM BBD BDT BGN BHD BIF BMD BND BOB BRL BSD " +
  "BTN BWP BYN BZD CAD CDF CHF CLP CNY COP CRC CUP CVE CZK DJF DKK DOP DZD EGP ERN " +
  "ETB EUR FJD FKP GBP GEL GHS GIP GMD GNF GTQ GYD HKD HNL HTG HUF IDR ILS INR IQD " +
  "IRR ISK JMD JOD JPY KES KGS KHR KMF KPW KRW KWD KYD KZT LAK LBP LKR LRD LSL LYD " +
  "MAD MDL MGA MKD MMK MNT MOP MRU MUR MVR MWK MXN MYR MZN NAD NGN NIO NOK NPR NZD " +
  "OMR PAB PEN PGK PHP PKR PLN PYG QAR RON RSD RUB RWF SAR SBD SCR SDG SEK SGD SHP " +
  "SLE SOS SRD SSP STN SVC SYP SZL THB TJS TMT TND TOP TRY TTD TWD TZS UAH UGX USD " +
  "UYU UZS VES VND VUV WST XAF XCD XCG XOF XPF YER ZAR ZMW ZWG"
).split(" ")

const KEYWORDS = ["key", "optional", "one", "of", "min", "max", "between", "and"]

// ── Pieces of a type ────────────────────────────────────────────────────────
// A word inside a type is anything but a reserved character or a space; a
// single dot may stand inside one (v1.2), `..` never.
const WORD_CHAR = raw`[^\s|/!?<>=()',":.]`
const BARE = raw`${WORD_CHAR}+(?:\.${WORD_CHAR}+)*`
const QUOTED = raw`'(?:[^']|'')*'`
const ITEM = raw`(?:${QUOTED}|${BARE})`
const NUM = raw`[+-]?(?:\d+(?:\.\d+)?|\.\d+)(?:[eE][+-]?\d+)?`
// Word edges: no word character (a dot counts, so v1.2 is one word) on the side.
const BEFORE = raw`(?<![^\s|/!?<>=()',":])`
const AFTER = raw`(?![^\s|/!?<>=()',":])`
const word = (alternatives: string[]) => raw`${BEFORE}(?:${alternatives.join("|")})${AFTER}`
const NOT_KEYWORD = raw`(?!${word(KEYWORDS)})`

// ── Scopes ──────────────────────────────────────────────────────────────────
// A column's name in the header and its text cells share one of these, in
// turn, as the tree-sitter queries do. The first column is the text's own
// colour; the rest are scopes every showcase theme paints. No theme there
// gives all six apart (`bun run colours.ts` measures what each gives), and
// Rosé Pine's palette has only five colours that are not a number's or a
// string's, so no choice of scopes could.
const RAINBOW = [
  null,
  "entity.name.function",
  "variable.parameter",
  "entity.name.tag",
  "entity.other.attribute-name",
  "support.function",
].map((scope, i) => `meta.column.${i + 1}.cells${scope ? ` ${scope}.column.cells` : ""}`)

const S = {
  preludeFence: "punctuation.separator.prelude.cells",
  prelude: "meta.embedded.block.prelude.cells",
  comment: "comment.line.double-slash.cells",
  commentMark: "punctuation.definition.comment.cells",
  typeKeyword: "storage.type.cells",
  typeName: "entity.name.type.cells",
  assign: "keyword.operator.assignment.cells",
  export: "storage.modifier.export.cells",
  import: "keyword.control.import.cells",
  path: "string.quoted.double.path.cells",
  brace: "punctuation.section.braces.cells",
  attribute: "entity.other.attribute-name.import.cells",
  table: "entity.name.function.table.cells",
  run: "keyword.control.run.cells",
  schedule: "keyword.other.schedule.cells",
  scheduleWord: "string.unquoted.schedule.cells",
  fence: "punctuation.definition.fence.cells",
  fenceLanguage: "entity.name.section.language.cells",
  fenceLabel: "entity.other.attribute-name.fence.cells",
  formula: "meta.formula.cells",
  formulaMark: "punctuation.definition.formula.cells",
  function: "support.function.formula.cells",
  formulaWord: "keyword.operator.word.formula.cells",
  // TABLE, FIRST, THIS, LAST: the language's own names, as `this` is a language's.
  languageWord: "variable.language.cells",
  text: "string.quoted.single.cells",
  null: "constant.language.null.cells",
  columnRef: "variable.other.property.cells",
  arithmetic: "keyword.operator.arithmetic.cells",
  parens: "punctuation.section.parens.cells",
  brackets: "punctuation.section.brackets.cells",
  number: "constant.numeric.cells",
  colon: "punctuation.separator.type.cells",
  comma: "punctuation.separator.comma.cells",
  type: "meta.type.cells",
  base: "support.type.primitive.cells",
  currency: "support.type.currency.cells",
  named: "entity.name.type.cells",
  referenceFile: "entity.name.namespace.cells",
  accessor: "punctuation.accessor.cells",
  referenceColumn: "variable.other.property.cells",
  choice: "variable.other.enummember.choice.cells",
  pipe: "punctuation.separator.choice.cells",
  oneOf: "keyword.other.one-of.cells",
  boolWord: "constant.language.boolean.cells",
  slash: "punctuation.separator.boolean.cells",
  comparison: "keyword.operator.comparison.cells",
  range: "keyword.operator.range.cells",
  wordOperator: "keyword.operator.word.cells",
  formatField: "constant.other.placeholder.format.cells",
  formatLiteral: "punctuation.separator.format.cells",
  // TypeScript's own split: a modifier for the key, an operator for `?`.
  key: "storage.modifier.key.cells",
  optional: "keyword.operator.optional.cells",
  escape: "constant.character.escape.cells",
  string: "string.quoted.double.cells",
  stringBegin: "punctuation.definition.string.begin.cells",
  stringEnd: "punctuation.definition.string.end.cells",
  boolean: "constant.language.boolean.cells",
  // A date, a time or an instant: a constant apart from the numbers in most
  // themes (constant.other.symbol), never a string's colour.
  date: "constant.other.symbol.date.cells",
  link: "markup.underline.link.cells",
}

// ── The type ────────────────────────────────────────────────────────────────

const quotedChoice = (scope: string) => ({
  match: QUOTED,
  name: scope,
  captures: { 0: { patterns: [{ match: "''", name: S.escape }] } },
})
const bareChoice = (scope: string) => ({ match: BARE, name: scope })
const item = (scope: string) => [quotedChoice(scope), bareChoice(scope)]

const type = [
  {
    comment: "A column's formula: everything after the first `=` that closes no `>=` or `<=`.",
    match: raw`(?<![<>])(=)(.*)`,
    captures: { 1: { name: S.assign }, 2: { name: S.formula, patterns: [{ include: "#formula" }] } },
  },
  {
    comment: "A currency's scale, EUR(6): its digits after the point.",
    match: raw`(\()\s*(\d+)\s*(\))`,
    captures: {
      1: { name: `${S.parens} punctuation.section.parens.begin.cells` },
      2: { name: S.number },
      3: { name: `${S.parens} punctuation.section.parens.end.cells` },
    },
  },
  {
    comment: "A format: how the file writes a date or a time.",
    match: raw`(\()([^)]*)(\))`,
    captures: {
      1: { name: `${S.parens} punctuation.section.parens.begin.cells` },
      2: { patterns: [{ match: "YYYY|MM|DD|HH|mm|ss", name: S.formatField }, { match: ".", name: S.formatLiteral }] },
      3: { name: `${S.parens} punctuation.section.parens.end.cells` },
    },
  },
  {
    comment: "bool yes no: a boolean in the file's own words, spelled out.",
    match: raw`${BEFORE}(bool)\s+${NOT_KEYWORD}(${ITEM})\s+${NOT_KEYWORD}(${ITEM})`,
    captures: {
      1: { name: S.base },
      2: { patterns: item(S.boolWord) },
      3: { patterns: item(S.boolWord) },
    },
  },
  {
    comment: "yes/no: a boolean in the file's own words, true first.",
    match: raw`(${ITEM})\s*(/)\s*(${ITEM})`,
    captures: {
      1: { patterns: item(S.boolWord) },
      2: { name: S.slash },
      3: { patterns: item(S.boolWord) },
    },
  },
  {
    comment: "one of a b c: choices spelled out, until an explicit keyword.",
    match: raw`${BEFORE}(one)\s+(of)${AFTER}((?:\s+${NOT_KEYWORD}${ITEM})+)`,
    captures: {
      1: { name: S.oneOf },
      2: { name: S.oneOf },
      3: { patterns: item(S.choice) },
    },
  },
  {
    comment: "a|b|c: choices.",
    match: raw`${ITEM}(?:\s*\|\s*${ITEM})+`,
    captures: { 0: { patterns: [...item(S.choice), { match: raw`\|`, name: S.pipe }] } },
  },
  {
    comment: "A column typed by another table's key, accounts.account.",
    match: raw`${BEFORE}([A-Za-z_][A-Za-z0-9_-]*)(\.)([A-Za-z0-9_][A-Za-z0-9_-]*)${AFTER}(?!\s*[|/])`,
    captures: { 1: { name: S.referenceFile }, 2: { name: S.accessor }, 3: { name: S.referenceColumn } },
  },
  {
    match: raw`${BEFORE}(between)\s+(${NUM})\s+(and)\s+(${NUM})`,
    captures: { 1: { name: S.wordOperator }, 2: { name: S.number }, 3: { name: S.wordOperator }, 4: { name: S.number } },
  },
  {
    match: raw`(${NUM})\s*(\.\.)\s*(${NUM})`,
    captures: { 1: { name: S.number }, 2: { name: S.range }, 3: { name: S.number } },
  },
  {
    match: raw`(>=|<=|>|<)\s*(${NUM})`,
    captures: { 1: { name: S.comparison }, 2: { name: S.number } },
  },
  {
    match: raw`${BEFORE}(min|max)\s+(${NUM})`,
    captures: { 1: { name: S.wordOperator }, 2: { name: S.number } },
  },
  { comment: "The key: in every row and unique.", match: raw`!|${word(["key"])}`, name: S.key },
  { comment: "May be blank.", match: raw`\?|${word(["optional"])}`, name: S.optional },
  { match: word(BASES), name: S.base },
  { match: word(CURRENCIES), name: S.currency },
  { match: raw`${BEFORE}[A-Z][A-Za-z0-9_]*${AFTER}`, name: S.named },
  { comment: "A lone quoted word is a type of one choice.", ...quotedChoice(S.choice) },
]

// ── A header cell and a row, six columns at a time ──────────────────────────
// Each regex matches six cells and is applied again from where it stopped,
// so the seventh column takes the first colour: the cycle tree-sitter's
// queries follow too.

// A quoted cell closes on this line, its last quote ending the cell; one
// that does not is the multi-line string rule's, below.
const CLOSED = raw`"(?:[^"]|"")*"(?=,|$)`
// A header cell quoted (its formula holds a `,` or a `"`) is read as a bare one inside
// its quotes: its name up to the first `:`, then its type and formula.
const headerCell = raw`(?:(")((?:[^":\r\n]|"")*)(?:(:)((?:[^"\r\n]|"")*))?(")(?=,|$)|([^:,\r\n"][^:,\r\n]*)?(?:(:)([^,\r\n]*))?)`
const rowCell = raw`(${CLOSED}|(?!")[^,\r\n]*)`

function cycle(cell: string, groupsPerCell: number, captureCell: (column: number) => Record<number, unknown>) {
  let match = raw`(?:^|(,))${cell}`
  for (let i = 1; i < RAINBOW.length; i++) match += raw`(?:(,)${cell})?`
  const captures: Record<number, unknown> = { 1: { name: S.comma } }
  let group = 2
  for (let column = 0; column < RAINBOW.length; column++) {
    if (column > 0) captures[group++] = { name: S.comma }
    const cellCaptures = captureCell(column)
    for (let g = 0; g < groupsPerCell; g++) captures[group + g] = cellCaptures[g]
    group += groupsPerCell
  }
  return { match, captures }
}

const header = cycle(headerCell, 8, (column) => ({
  0: { name: S.stringBegin },
  1: { name: RAINBOW[column] },
  2: { name: S.colon },
  3: { name: S.type, patterns: [{ include: "#type" }] },
  4: { name: S.stringEnd },
  5: { name: RAINBOW[column] },
  6: { name: S.colon },
  7: { name: S.type, patterns: [{ include: "#type" }] },
}))

// A cell is classified by its spelling alone, the whole cell: `12 apples` is
// text, `84.20` a number.
const whole = (body: string) => raw`(?<=^|,)[ \t]*(?:${body})[ \t]*$`

// A cell is what is written (84.20), what a formula computed (84.20=), or that and its
// own formula (32.25=96.75 / 3): the first `=` that is no `'=` splits what the file
// stores from how it is computed.
const value = [
  {
    comment: "A quoted computed cell (its own formula holds a `,`): the same parts inside its quotes.",
    match: whole(raw`(")((?:'=|""|[^="])*?)(?<!')(=)((?:""|[^"])*)(")`),
    captures: {
      1: { name: S.stringBegin },
      2: { name: "meta.stored.cells", patterns: [{ include: "#written" }] },
      3: { name: S.formulaMark },
      4: { name: S.formula, patterns: [{ include: "#formula" }] },
      5: { name: S.stringEnd },
    },
  },
  {
    match: whole(raw`(")((?:[^"]|"")*)(")`),
    name: S.string,
    captures: {
      1: { name: S.stringBegin },
      2: { patterns: [{ match: '""', name: S.escape }] },
      3: { name: S.stringEnd },
    },
  },
  {
    comment: "A computed cell: what the file stores, `=`, then its own formula or none (its column's).",
    match: raw`(?<=^|,)[ \t]*((?:'=|[^=])*?)(?<!')(=)(.*)$`,
    captures: {
      1: { name: "meta.stored.cells", patterns: [{ include: "#written" }] },
      2: { name: S.formulaMark },
      3: { name: S.formula, patterns: [{ include: "#formula" }] },
    },
  },
  { include: "#written" },
]

// What a cell holds as written, by its spelling alone.
const written = [
  {
    match: whole(raw`(")((?:[^"]|"")*)(")`),
    name: S.string,
    captures: {
      1: { name: S.stringBegin },
      2: { patterns: [{ match: '""', name: S.escape }] },
      3: { name: S.stringEnd },
    },
  },
  { match: whole(raw`\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?`), name: S.date },
  { match: whole(raw`\d{4}-\d{2}-\d{2}`), name: S.date },
  { match: whole(raw`\d{2}:\d{2}(?::\d{2})?`), name: S.date },
  { match: whole(raw`[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?%?`), name: S.number },
  { match: whole("true|false|yes|no"), name: S.boolean },
  { match: whole(raw`[A-Za-z][A-Za-z0-9+.-]*://[^,\s"]+`), name: S.link },
  { match: whole(raw`[^\s@,"]+@[^\s@,"]+\.[^\s@,"]+`), name: S.link },
  { comment: "A text's `=`, written `'=`.", match: "'=", name: S.escape },
]

const row = cycle(rowCell, 1, (column) => ({ 0: { name: RAINBOW[column], patterns: [{ include: "#value" }] } }))

// ── The prelude ─────────────────────────────────────────────────────────────

// ── A formula ───────────────────────────────────────────────────────────────
// SQL's scalar expressions, the language in capitals and only in capitals: a word
// in lower case is always a name the file gave, never a function or a keyword.

const caps = (words: string[]) => raw`\b(?:${words.join("|")})\b`
// A row by where it is from this one, or an end of a range of rows.
const END = raw`FIRST|THIS|LAST|[+-]\d+`

const formula = [
  {
    match: raw`'((?:[^']|'')*)'`,
    name: S.text,
    captures: { 1: { patterns: [{ match: "''", name: S.escape }] } },
  },
  { match: raw`"[^"]*"`, name: S.columnRef },
  {
    comment: "A range of rows by where they are, total[FIRST:THIS], cost[-2:+2].",
    match: raw`(\[)\s*(${END})\s*(:)\s*(${END})\s*(\])`,
    captures: {
      1: { name: S.brackets },
      2: { patterns: [{ match: "FIRST|THIS|LAST", name: S.languageWord }, { match: raw`[+-]\d+`, name: S.number }] },
      3: { name: S.range },
      4: { patterns: [{ match: "FIRST|THIS|LAST", name: S.languageWord }, { match: raw`[+-]\d+`, name: S.number }] },
      5: { name: S.brackets },
    },
  },
  {
    comment: "A row by where it is from this one, running[-1].",
    match: raw`(\[)\s*([+-]\d+)\s*(\])`,
    captures: { 1: { name: S.brackets }, 2: { name: S.number }, 3: { name: S.brackets } },
  },
  { match: NUM.replace("[+-]?", ""), name: S.number },
  { match: caps(["TRUE", "FALSE"]), name: S.boolean },
  { match: caps(["NULL"]), name: S.null },
  { match: caps(["TABLE", "FIRST", "THIS", "LAST"]), name: S.languageWord },
  {
    match: caps(["AND", "OR", "NOT", "IS", "CASE", "WHEN", "THEN", "ELSE", "END", "OVER", "ORDER", "BY", "DESC"]),
    name: S.formulaWord,
  },
  { match: raw`\b[A-Z][A-Z0-9_]*(?=\s*\()`, name: S.function },
  { match: raw`\|\||<>|!=|<=|>=|[-+*/<>=]`, name: S.arithmetic },
  { match: raw`[()]`, name: S.parens },
  { match: raw`[\[\]]`, name: S.brackets },
  { match: raw`\.`, name: S.accessor },
  { match: ",", name: S.comma },
  { match: raw`[A-Za-z_][A-Za-z0-9_]*`, name: S.columnRef },
]

// ── The prelude ─────────────────────────────────────────────────────────────

const NAME = raw`[A-Za-z_][A-Za-z0-9_]*`
const STRING = raw`"[^"\r\n]*"`

// A fence: the opening backticks (three or more) and their language, closed by
// a line of as many. The body of a ```cells fence is a header and rows.
const fence = (language: string, inner: object[], contentName: string, label = "") => ({
  begin: raw`(\x60{3,})(${language})${label ? raw`(?:[ \t]+(${label}))?` : ""}[ \t]*$`,
  beginCaptures: { 1: { name: S.fence }, 2: { name: S.fenceLanguage }, 3: { name: S.fenceLabel } },
  end: raw`^(\1)[ \t]*$`,
  endCaptures: { 1: { name: S.fence } },
  contentName,
  patterns: inner,
})

const fenced = [
  fence("ts", [{ include: "source.ts" }], "meta.embedded.block.typescript"),
  fence(
    "cells",
    [
      {
        comment: "The table: its header, then its rows, up to the closing fence.",
        begin: raw`^(?=[^\r\n])`,
        end: raw`^(?=\x60{3,}[ \t]*$)`,
        patterns: [
          { begin: raw`\G`, end: "$", name: "meta.header.cells", patterns: [{ include: "#header" }] },
          { include: "#row" },
        ],
      },
    ],
    "meta.embedded.block.cells",
    "runs",
  ),
  fence(raw`[A-Za-z0-9_+-]*`, [], "meta.embedded.block"),
]

const prelude = {
  begin: raw`\A(---)[ \t]*$`,
  beginCaptures: { 1: { name: S.preludeFence } },
  end: raw`^(---)[ \t]*$`,
  endCaptures: { 1: { name: S.preludeFence } },
  contentName: S.prelude,
  patterns: [
    { include: "#comment" },
    {
      comment: "An import: a table by default, or what the other file exports, from a path or a pinned URL.",
      match: raw`^[ \t]*(import)[ \t]+(?:(${NAME})|(\{[^}]*\}))[ \t]+(from)[ \t]+(${STRING})(?:[ \t]+(with)[ \t]+(\{[^}]*\}))?`,
      captures: {
        1: { name: S.import },
        2: { name: S.referenceFile },
        3: { patterns: [{ match: "[{}]", name: S.brace }, { match: ",", name: S.comma }, { match: NAME, name: S.referenceFile }] },
        4: { name: S.import },
        5: { name: S.path },
        6: { name: S.import },
        7: {
          patterns: [
            { match: "[{}]", name: S.brace },
            { match: ",", name: S.comma },
            { match: raw`(${NAME})[ \t]*(:)`, captures: { 1: { name: S.attribute }, 2: { name: S.colon } } },
            { match: STRING, name: S.path },
          ],
        },
      },
    },
    {
      match: raw`^[ \t]*(?:(export)[ \t]+)?(type)[ \t]+([A-Z][A-Za-z0-9_]*)[ \t]*(=)[ \t]*((?:[^/\r\n]|/(?!/))*)`,
      captures: {
        1: { name: S.export },
        2: { name: S.typeKeyword },
        3: { name: S.typeName },
        4: { name: S.assign },
        5: { name: S.type, patterns: [{ include: "#type" }] },
      },
    },
    {
      comment: "A table a script fills: its name, `run` and the schedule; its fences follow.",
      match: raw`^[ \t]*(?:(export)[ \t]+)?(${NAME})[ \t]*(=)[ \t]*(run)\b((?:[^/\r\n]|/(?!/))*)`,
      captures: {
        1: { name: S.export },
        2: { name: S.table },
        3: { name: S.assign },
        4: { name: S.run },
        5: {
          patterns: [
            { match: raw`\b(?:every|at|on|in|cron|when|changes)\b`, name: S.schedule },
            { match: raw`\b\d{1,2}:\d{2}\b`, name: S.number },
            { match: ",", name: S.comma },
            { match: raw`[^\s,]+`, name: S.scheduleWord },
          ],
        },
      },
    },
    {
      comment: "A table written inline: its name, then its fence on the same line.",
      match: raw`^[ \t]*(?:(export)[ \t]+)?(${NAME})[ \t]*(=)(?=[ \t]*\x60{3,})`,
      captures: { 1: { name: S.export }, 2: { name: S.table }, 3: { name: S.assign } },
    },
    ...fenced,
  ],
}

// ── The file ────────────────────────────────────────────────────────────────

const body = {
  comment: "The header, then every row: never ends.",
  begin: raw`^(?=[^\r\n])`,
  end: raw`(?!)`,
  patterns: [
    { comment: "The header is the body's first line.", begin: raw`\G`, end: "$", name: "meta.header.cells", patterns: [{ include: "#header" }] },
    {
      comment: "A quoted field that holds a line break: a string until its closing quote.",
      begin: raw`(?:^|(,))(")(?=(?:[^"]|"")*$)`,
      beginCaptures: { 1: { name: S.comma }, 2: { name: S.stringBegin } },
      end: raw`(")(?!")`,
      endCaptures: { 1: { name: S.stringEnd } },
      name: S.string,
      patterns: [{ match: '""', name: S.escape }],
    },
    { include: "#row" },
  ],
}

const grammar = {
  $schema: "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
  name: "cells",
  displayName: "Cells",
  scopeName: "source.cells",
  fileTypes: ["cell"],
  comment: "GENERATED by syntaxes/build.ts. Cells, data and its logic in one plain-text file: see SPEC.md.",
  patterns: [
    {
      comment: "A file that opens with a prelude: the prelude, then the body.",
      begin: raw`\A(?=---[ \t]*$)`,
      end: raw`(?!)`,
      patterns: [prelude, body],
    },
    body,
  ],
  repository: {
    comment: {
      match: raw`(//).*$`,
      name: S.comment,
      captures: { 1: { name: S.commentMark } },
    },
    type: { patterns: type },
    formula: { patterns: formula },
    value: { patterns: value },
    written: { patterns: written },
    header,
    row: { ...row, name: "meta.row.cells" },
  },
}

const out = join(import.meta.dir, "cells.tmLanguage.json")
writeFileSync(out, JSON.stringify(grammar, null, 2) + "\n")
console.log(`wrote ${out}`)
