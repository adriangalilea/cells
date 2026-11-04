// tcsv.tmLanguage.json, written from this file: the TextMate grammar for VS
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
].map((scope, i) => `meta.column.${i + 1}.tcsv${scope ? ` ${scope}.column.tcsv` : ""}`)

const S = {
  fence: "punctuation.separator.prelude.tcsv",
  prelude: "meta.embedded.block.prelude.tcsv",
  comment: "comment.line.double-slash.tcsv",
  commentMark: "punctuation.definition.comment.tcsv",
  typeKeyword: "storage.type.tcsv",
  typeName: "entity.name.type.tcsv",
  assign: "keyword.operator.assignment.tcsv",
  use: "keyword.control.import.tcsv", // PROVISIONAL
  path: "string.unquoted.path.tcsv", // PROVISIONAL
  computed: "entity.name.function.computed.tcsv",
  columnRef: "variable.other.property.tcsv",
  arithmetic: "keyword.operator.arithmetic.tcsv",
  parens: "punctuation.section.parens.tcsv",
  number: "constant.numeric.tcsv",
  colon: "punctuation.separator.type.tcsv",
  comma: "punctuation.separator.comma.tcsv",
  type: "meta.type.tcsv",
  base: "support.type.primitive.tcsv",
  currency: "support.type.currency.tcsv",
  named: "entity.name.type.tcsv",
  referenceFile: "entity.name.namespace.tcsv", // PROVISIONAL
  accessor: "punctuation.accessor.tcsv", // PROVISIONAL
  referenceColumn: "variable.other.property.tcsv", // PROVISIONAL
  choice: "variable.other.enummember.choice.tcsv",
  pipe: "punctuation.separator.choice.tcsv",
  oneOf: "keyword.other.one-of.tcsv",
  boolWord: "constant.language.boolean.tcsv",
  slash: "punctuation.separator.boolean.tcsv",
  comparison: "keyword.operator.comparison.tcsv",
  range: "keyword.operator.range.tcsv",
  wordOperator: "keyword.operator.word.tcsv",
  formatField: "constant.other.placeholder.format.tcsv",
  formatLiteral: "punctuation.separator.format.tcsv",
  // TypeScript's own split: a modifier for the key, an operator for `?`.
  key: "storage.modifier.key.tcsv",
  optional: "keyword.operator.optional.tcsv",
  escape: "constant.character.escape.tcsv",
  string: "string.quoted.double.tcsv",
  stringBegin: "punctuation.definition.string.begin.tcsv",
  stringEnd: "punctuation.definition.string.end.tcsv",
  boolean: "constant.language.boolean.tcsv",
  // A date, a time or an instant: a constant apart from the numbers in most
  // themes (constant.other.symbol), never a string's colour.
  date: "constant.other.symbol.date.tcsv",
  link: "markup.underline.link.tcsv",
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
    comment: "A format: how the file writes a date or a time.",
    match: raw`(\()([^)]*)(\))`,
    captures: {
      1: { name: `${S.parens} punctuation.section.parens.begin.tcsv` },
      2: { patterns: [{ match: "YYYY|MM|DD|HH|mm|ss", name: S.formatField }, { match: ".", name: S.formatLiteral }] },
      3: { name: `${S.parens} punctuation.section.parens.end.tcsv` },
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
    comment: "PROVISIONAL (SPEC, Open): a column typed by another file's key, accounts.account.",
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
const headerCell = raw`(?:(${CLOSED})|([^:,\r\n"][^:,\r\n]*)?(?:(:)([^,\r\n]*))?)`
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

const header = cycle(headerCell, 4, (column) => ({
  0: { name: `${RAINBOW[column]} string.quoted.double.name.tcsv` },
  1: { name: RAINBOW[column] },
  2: { name: S.colon },
  3: { name: S.type, patterns: [{ include: "#type" }] },
}))

// A cell is classified by its spelling alone, the whole cell: `12 apples` is
// text, `84.20` a number.
const whole = (body: string) => raw`(?<=^|,)[ \t]*(?:${body})[ \t]*$`

const value = [
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
]

const row = cycle(rowCell, 1, (column) => ({ 0: { name: RAINBOW[column], patterns: [{ include: "#value" }] } }))

// ── The prelude ─────────────────────────────────────────────────────────────

const prelude = {
  begin: raw`\A(---)[ \t]*$`,
  beginCaptures: { 1: { name: S.fence } },
  end: raw`^(---)[ \t]*$`,
  endCaptures: { 1: { name: S.fence } },
  contentName: S.prelude,
  patterns: [
    { include: "#comment" },
    {
      comment: "PROVISIONAL (SPEC, Open: imports; the keyword is not decided).",
      match: raw`^[ \t]*(use)[ \t]+(\S+)`,
      captures: { 1: { name: S.use }, 2: { name: S.path } },
    },
    {
      match: raw`^[ \t]*(type)[ \t]+([A-Z][A-Za-z0-9_]*)[ \t]*(=)[ \t]*((?:[^/\r\n]|/(?!/))*)`,
      captures: {
        1: { name: S.typeKeyword },
        2: { name: S.typeName },
        3: { name: S.assign },
        4: { name: S.type, patterns: [{ include: "#type" }] },
      },
    },
    {
      comment: "A computed column: shown and read after the header's, never stored.",
      begin: raw`^[ \t]*([A-Za-z_][A-Za-z0-9_]*)[ \t]*(=)`,
      beginCaptures: { 1: { name: S.computed }, 2: { name: S.assign } },
      end: raw`(?=//)|$`,
      patterns: [
        { match: NUM.replace("[+-]?", ""), name: S.number },
        { match: raw`[-+*/]`, name: S.arithmetic },
        { match: raw`[()]`, name: S.parens },
        { match: raw`[A-Za-z_][A-Za-z0-9_]*`, name: S.columnRef },
      ],
    },
  ],
}

// ── The file ────────────────────────────────────────────────────────────────

const body = {
  comment: "The header, then every row: never ends.",
  begin: raw`^(?=[^\r\n])`,
  end: raw`(?!)`,
  patterns: [
    { comment: "The header is the body's first line.", begin: raw`\G`, end: "$", name: "meta.header.tcsv", patterns: [header] },
    {
      comment: "A quoted field that holds a line break: a string until its closing quote.",
      begin: raw`(?:^|(,))(")(?=(?:[^"]|"")*$)`,
      beginCaptures: { 1: { name: S.comma }, 2: { name: S.stringBegin } },
      end: raw`(")(?!")`,
      endCaptures: { 1: { name: S.stringEnd } },
      name: S.string,
      patterns: [{ match: '""', name: S.escape }],
    },
    { ...row, name: "meta.row.tcsv" },
  ],
}

const grammar = {
  $schema: "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
  name: "tcsv",
  displayName: "TCSV",
  scopeName: "source.tcsv",
  fileTypes: ["tcsv"],
  comment: "GENERATED by syntaxes/build.ts. TCSV, typed CSV: see SPEC.md.",
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
    value: { patterns: value },
  },
}

const out = join(import.meta.dir, "tcsv.tmLanguage.json")
writeFileSync(out, JSON.stringify(grammar, null, 2) + "\n")
console.log(`wrote ${out}`)
