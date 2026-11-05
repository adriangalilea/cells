# Cells

Programmable data: logic and data in a single file, open in any editor ([cellular.computer](https://cellular.computer)). A cell, a `.cell` file, is a table whose header cells carry their column's type, and its formula when it computes, in the file itself: one file, never a sidecar. Files import each other, so a folder of them is typed tables joined by references, and a table can be filled by a script. It is its own format, designed for its own ergonomics, never bent to suit readers that do not know it. [SPEC.md](SPEC.md) is the whole format, and not final yet.

````
---
import accounts from "./accounts.cell"

eurusd = run every day at 18:00 in Europe/Madrid
```ts
export default async function ({ last }) {
  const res = await fetch(`https://api.exchange.example/usdeur?since=${last.date}`)
  return (await res.json()).map(d => ({ date: d.day, rate: String(d.close) }))
}
```
```cells
date:date!,rate:num>0,run:int?
2026-11-03,0.9287,9
2026-11-04,0.9312,9
```
---
date:date,usd:USD>0,from:accounts.account,to:accounts.account,eur:EUR=usd * eurusd[date].rate,note:text?
2026-11-03,180.00,card,hotel,167.17=,two nights
2026-11-04,42.50,card,food,39.58=,
2026-11-04,37.80=12.60 * 3,card,food,35.20=,three coffees
````

What a person sees is the table under the second `---`: `eur` computed on every row from the day's rate and stored beside its `=`, a reference that must name an account, a cell holding its own formula and the value it gave (`37.80=12.60 * 3`). Above it is the work, which an app shows folded: the accounts imported from the next file, and the rates a script fetched and the file keeps.

## The documents

| | |
|---|---|
| [SPEC.md](SPEC.md) | the format, and in its Open section what is decided in direction and what is not |
| [tests/README.md](tests/README.md) | the conformance suite: what a case is, and how an implementation runs it |
| [rust/README.md](rust/README.md) | the core in Rust: its crates, its API, what it measures, the wasm and Apple builds |
| [ENGINE.md](ENGINE.md) | the engine every app reads and writes through: v0, one Mac, and what comes later |
| [EDITORS.md](EDITORS.md) | the language server, `cells lsp`, and Helix, Neovim and VS Code set up for it |
| [web/README.md](web/README.md) | the site, cellular.computer: how it is built from this repo's engine and ui's items |
| [web/TODO.md](web/TODO.md) | what the site still owes |
| [ACCOUNTING.md](ACCOUNTING.md) | books kept in Cells, drafted with the primitives the spec proposes |

## The syntax

A header cell is `name:type`, or `name:type=formula`. A cell with no `:` is a plain column, its type read from its values.

| Header cell | The column |
|---|---|
| `item:text!` | text, the key: in every row, unique |
| `category:Category` | one of the words the prelude named |
| `cost:EUR>=0` | euros, at least 0 (any ISO 4217 code), at cents |
| `fee:EUR(6)` | euros at six digits after the point, for what is billed in fractions of a cent |
| `qty:int>=1` | a whole number, at least 1 |
| `paid:yes/no` | a boolean the file writes `yes` and `no` |
| `on:date` | an ISO date; `on:date(DD/MM/YYYY)` reads the file's own form |
| `note:text?` | `?`: may be blank |
| `mood:'yay!'\|meh` | a choice; single quotes hold a reserved character |
| `score:0..10` | a number from 0 to 10 |
| `to:accounts.account` | a key of the table the prelude names `accounts`: it must exist |
| `total:EUR=cost * qty` | euros its formula computes on every row, each row storing what it gave (`84.20=`); a value written with no `=` is that row's override |
| `running:EUR=running[-1] + total` | the running total: the one on the line above, plus this row's |

The bases are `text` `int` `num` `pct` `bool` `date` `time` `datetime` `url` `email` and every currency. Every shorthand has a word, so a header reads either way: `cost:EUR>=0` is `cost:EUR min 0`, `paid:yes/no` is `paid:bool yes no`, `item:text!` is `item:text key`.

A cell is a value (`84.20`), a value its column's formula computed (`84.20=`), or a value and its own formula (`32.25=96.75 / 3`): left of the `=` what the file stores, right of it how it is computed, so every value is in the file, readable in any editor. Formulas are SQL's expressions in exact decimal, the language in capitals (`CASE`, `COALESCE`, `ROUND`, `||`, `SUM(cost)`) and every lower-case word a name: a column of the row, `to.kind` through a reference, `fees['card'].pct` a row by its key, `running[-1]` the line above, `SUM(total[FIRST:THIS])` the rows down to this one. A spreadsheet's `=SUM($E$2:E2)` is accepted as typed and stored in names, so no line inserted by hand ever moves what a formula reads.

The prelude, between two `---` lines, names types (`type Kind = …`), imports other files (`import accounts from "../accounts.cell"`, `import { Kind } from "./types.cell"`, a URL pinned by its hash), and holds tables of the file's own, written inline in a `` ```cells `` fence or filled by a script (`rates = run every hour`), with `//` comments.

## Reading and writing

- **A broken cell opens and says why**, never coerced: `row 3, cost: "8x.50" is not a number`. Its text is kept, its value null, so the tool that can fix it can show it. A duplicate key reports both rows, a reference to no row says so (`"bar" is no key of accounts`), and so does a formula that does not compute. A broken grammar takes out only what it breaks, and the file opens with its table: a header cell typed halfway (`to:accounts.a`) is a column of text, named as broken, every other column typed. A control character (a NUL, an ESC, a C1 from a bank's export) breaks its cell in any column, named by its code point, `U+0000`, since it shows nothing, and no writer writes one.
- **Written back byte for byte** but for what changed: the line endings, the quoting, the header's spelling and the prelude as they were read. A write stores every computed value its change reaches, in one pass; a value typed over its `=` or left behind by a plain editor is a problem saying both numbers (`the formula gives 90.00, the file stores 84.20`), never overwritten unasked, and `cells fix` or a client's fix resolves it.
- **Merged by git without conflicts on what is computed**: `cells merge`, git's merge driver, merges what was written and computes the rest again.
- **Records quote as RFC 4180** (a field holding `,`, `"` or a line break is quoted, `""` inside), because it is the right rule for cells.
- **Opening never runs anything.** A script runs when asked (`cells run`), and a file opens with the rows its last run stored.

## Where it is going

Decided in direction, not yet in the spec ([SPEC.md, Open](SPEC.md#open)):

- **Your filesystem can be a graph database.** References join a folder's files today; next, tools that see a break before it lands (a key renamed in one file, followed through every file that names it) and fixes offered where a reference names nothing (`did you mean "rent"?`).
- **Every row has an identity**, its key or an `id` the engine assigns, so an edit always lands on the row it meant and two versions of a file merge row by row.
- **One engine every app reads and writes through**: first a daemon on one Mac that keeps each file's schedules, standalone in a page; files parsed once, changes heard as they happen, nobody's write ever lost, even an editor's that bypasses it. Later every device a full replica, books owned by one of them. [ENGINE.md](ENGINE.md) is the design.
- **Queries in standards, never our own language**: SQL, and SQL/PGQ to walk references.
- **Charts with no setup**, because the columns are typed: a date is a time axis, money a measure, a choice a category, a reference a link to follow; what a chart needs beyond the types declared, never scripted.
- **Tooling**: highlighting in every editor, and the language server ([EDITORS.md](EDITORS.md)) warning before a break lands: a key renamed in the other file, a reference about to dangle.

## What it is for

The use it is designed against is plain text accounting done properly. Books kept as plain files, typed like a database:

```
~/books/
├── accounts.cell          every account, once
├── home/money.cell        each movement points at an account
├── travel/money.cell
└── studio/money.cell
```

```
accounts.cell
---
export type Kind = assets|liabilities|expenses|income
---
account:text!,kind:Kind
assets:bank:main,assets
expenses:home:rent,expenses
expenses:home:power,expenses

home/money.cell
---
import accounts from "../accounts.cell"
---
id:id!,date:date,amount:EUR>0,account_from:accounts.account,account_to:accounts.account,description:text,tax_rate:pct,vat:EUR=amount * tax_rate / (100 + tax_rate)
k7m2q9xa,2026-07-01,950.00,assets:bank:main,expenses:home:rent,rent july,0,0.00=
p3w8c2nd,2026-07-12,64.30,assets:bank:main,expenses:home:power,power july,21,11.16=
q4x9t1mz,2026-07-30,950.00,assets:bank:main,expenses:home:rnet,rent august,0,0.00=
```

```
$ cells check books
books/home/money.cell row 3, account_to: "expenses:home:rnet" is no key of accounts

$ cells show books/home/money.cell
id        date        amount  account_from      account_to           description  tax_rate  vat
k7m2q9xa  2026-07-01  950.00  assets:bank:main  expenses:home:rent   rent july    0         0.00
p3w8c2nd  2026-07-12  64.30   assets:bank:main  expenses:home:power  power july   21        11.16
q4x9t1mz  2026-07-30  950.00  assets:bank:main  expenses:home:rnet   rent august  0         0.00
```

`cells show` writes tab-separated text (aligned here), each cell its value: `vat` is euros, so 64.30 at 21% is `11.16`, rounded half to even at the euro's two digits, the same value the file stores beside its `=`. Next: `cells sql ~/books "SELECT account_to, SUM(amount) FROM money GROUP BY account_to"`.

Ledgers in plain text (hledger, beancount) keep books readable, diffable and yours, but each invents a syntax of its own and cannot say that a column is euros, that an account must exist, or that VAT is derived from the amount. Cells is the table those books are. [ACCOUNTING.md](ACCOUNTING.md) drafts them whole: pending payments, a payslip in several rows, exchanges, lots, corrections, balances that must hold.

## The pieces

One law and one engine. Every surface binds the same Rust core, so there is one reader to make fast and none to drift.

| Piece | Where | In |
|---|---|---|
| The format | [SPEC.md](SPEC.md) | prose |
| The law: every rule as a case, run against every binding | [`tests/`](tests) | data |
| The engine: parsing at memory speed, typed columns in Arrow's layout (exact decimals for money, days for dates, a dictionary for choices), formulas in exact decimal, imports followed, references checked, problems, the byte-for-byte writer | `rust/` | Rust, no dependencies |
| The command line: `cells check` (a folder, its imports followed), `fmt --check`, `pin`, `run`, `show`; then `cells sql` (DuckDB over the columns, so SQL and SQL/PGQ are the standards') | `rust/` | Rust |
| The daemon ([ENGINE.md](ENGINE.md)): every client's one way to read and write, each file parsed once and re-parsed when it changes, the references held as a graph, edits by row identity, the schedules kept, never losing a write | `rust/` | Rust |
| The protocol: intents and queries in, Arrow frames out, over XPC or a Unix socket | `rust/` | Rust |
| The language server, `cells lsp`: diagnostics as you type, hover, completion, definitions, renames followed across the folder | `rust/` | Rust |
| The web: the engine as wasm, a synchronous `read(text)`, its imports followed through the files a page holds, columns read as typed arrays without a copy | `rust/` | Rust, wasm |
| The Apple platforms: the engine behind a C header, swift-utils' `Cells` and `CellsTable` over it | `rust/` + swift-utils | Rust, Swift |
| Python, Go | pending: after the Apple platforms and the web | Rust (PyO3), cgo |
| Highlighting in Helix, Neovim, Zed | `tree-sitter/` | tree-sitter |
| Highlighting in VS Code, GitHub, Shiki | `syntaxes/` | TextMate |

Rust because it is as fast as C and reaches everything else: a C ABI, wasm, PyO3, `tower-lsp`, DuckDB's bindings. Arrow because typed columns are what it is: DuckDB, Python, the browser and Swift take them as they lie, and a chart reads a column, never a row. DuckDB ships with the command line, the daemon and the apps, never in a page. Bun runs a file's scripts, never the core: it is a program, and no page or app can link it. Parquet is an export of the same columns, never the source: in memory both are Arrow, and what text costs at load the daemon's cache pays once, while Parquet can never be read, diffed in git or fixed by hand.

Built: the suite (467 cases, `plain/` the files that are not cells); the engine passing all of them in Rust and through its wasm build (a 119 KB brotli wasm with a synchronous `read`; 1M rows read in 12 ms on 18 threads, rust/README.md): formulas in header cells and in cells, SQL's expressions in exact decimal, `this` the file's own table, windows over an order (a running balance, `lag`), names looked for nearest first, tables written inline, imports and exports (a URL pinned by its hash, a `rev` through git), references checked across files, scripts' tables and schedules read; a broken grammar taking out only what it breaks, every break and every problem carrying its span (the characters a client marks, in every binding, pinned by the suite); the command line (`cells check` over a folder with its imports followed, `fmt --check`, `pin`, `run` with Bun, `show`); the engine reading what people's files are (a `.csv` or `.tsv` by the separator its first lines agree on, never refused for its records, a ragged row read as blanks with its problem), columns inserted, removed, moved and renamed, and a file written in another dialect; ui's table and Cells on the wasm build; the Apple XCFramework, released as `apple-<version>`, and swift-utils' `Cells` a binding over it on macOS, iOS and tvOS, passing every case through Swift with its imports followed (`cells_read_with`, and `cells_read_at` from the disk), its `CellsTable` and Cells.app showing a computed cell's value and editing its formula; the grammars (tree-sitter, TextMate) reading imports, tables, fences and formulas; the language server, `cells lsp`, checked in Helix ([EDITORS.md](EDITORS.md)). Being built, the format as SPEC.md now writes it, in order: the suite's cases for it; the engine (every computed value stored beside its `=`, a write storing what it changes, the `stale` problem, `[-1]` and ranges in the lines' order, the language in capitals and `TABLE`, a spreadsheet's references taken as input, a currency's scale, `cells fix`, `cells merge`); the language server (computed values stored on save, a stale cell's two fixes); the grammars; the wasm and Apple builds; the site. Next, in order: `cells sql`, a folder asked as one database; the daemon keeping the schedules; charts from the types. To do: the npm package `@adriangalilea/cells` (the wasm build and its wrapper, published). [`ts/`](ts) is the first TypeScript reader, kept as a readable reference: `read`, `write`, `setCell`, `setType`, `parseHeader`, `compose`, and `Infer<"header">`, the row type from a header line at compile time; it predates the suite and fails many of its cases, the engine passes them all.

## Where it is used

- **[cellular.computer](https://cellular.computer)**: the format told live, every example read by the wasm engine in the page.
- **ui's table** ([ui.adriangalilea.com/table](https://ui.adriangalilea.com/table)): a `.cell` file opens as a table whose columns edit by their types, its broken cells marked, through `useTableDocument` and the `cells` codec (`lib/cells.ts`, this repo's wasm wrapper and module vendored by ui's `mise cells`).
- **InkTable and Cells for Swift**: the same table on the Apple platforms.
- **Cells.app**: the Mac app for `.cell`, CSV and TSV files, built on them.
