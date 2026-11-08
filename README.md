# Cells

Programmable data: logic and data in a single file, open in any editor ([cellular.computer](https://cellular.computer)). A Cells file, `.cells`, is a table whose header cells carry their column's type, and its formula when it computes, in the file itself: one file, never a sidecar. Files import each other, so a folder of them is typed tables joined by references, and a table can be filled by a script. It is its own format, designed for its own ergonomics, never bent to suit readers that do not know it. **It is not meant to be compatible with CSV: adopting it is as simple as renaming a `.csv` to `.cells`, and that is the only bridge.** [SPEC.md](SPEC.md) is the whole format, and not final yet.

## From a CSV

A CSV is where most tables start, and it reads as a Cells file already: one with no types written.

```cells
item,cost,qty,paid,on
fuel,84.20,1,yes,2026-08-14
split dinner,32.25,1,no,2026-08-15
ferry,5O.00,1,no,2026-08-17
```

Nothing in it says `cost` is money, so the ferry's `5O.00`, typed with a letter O, is as good as any other value. A sum over the column is wrong, or fails somewhere far from the row that caused it.

### Types

Each header cell says what its column holds: `name:type`.

```cells problem
item:text,cost:EUR,qty:int,paid:yes/no,on:date
fuel,84.20,1,yes,2026-08-14
split dinner,32.25,1,no,2026-08-15
ferry,5O.00,1,no,2026-08-17
```

The file still reads, every other cell a typed value, and the ferry's is a problem at its place: `row 3, cost: "5O.00" is not a number`. Nothing is coerced, and nothing is lost.

### Rules

A type can say more: `!` makes a column the key, and bounds say what a value may be.

```cells
item:text!,cost:EUR>=0,qty:int>=1,paid:yes/no,on:date
fuel,84.20,1,yes,2026-08-14
split dinner,32.25,1,no,2026-08-15
ferry,50.00,2,no,2026-08-17
```

Two rows named `fuel`, a cost below zero or a quantity of none would each be a problem.

### A computed column

A CSV can hold a total, but only as a number: whether it is still `cost * qty` after a cost is edited, nothing knows. In a Cells file the formula is in the header, and each row stores the value it gave, before its `=`.

```cells
item:text!,cost:EUR>=0,qty:int>=1,total:EUR=cost * qty,paid:yes/no,on:date
fuel,84.20,1,84.20=,yes,2026-08-14
split dinner,32.25,1,32.25=,no,2026-08-15
ferry,50.00,2,100.00=,no,2026-08-17
```

Every value is in the file, so it reads in any editor without running anything, and the `=` says which ones were computed.

### A value left behind

Edit a cost in a plain editor and leave its total as it was:

```cells problem
item:text!,cost:EUR>=0,qty:int>=1,total:EUR=cost * qty,paid:yes/no,on:date
fuel,90.00,1,84.20=,yes,2026-08-14
```

The stored total is stale, and the reader says so with both numbers: `row 1, total: the formula gives 90.00, the file stores 84.20`. It is never overwritten unasked: `cells fix`, or saving through the language server, stores the new value.

### A cell's own formula, and rows above

The dinner was split three ways. A CSV keeps `32.25` and loses why; a Cells file keeps both. A formula can read the row above, for a running total:

```cells
item:text!,cost:EUR>=0,qty:int>=1,total:EUR=cost * qty,running:EUR=running[-1] + total
fuel,84.20,1,84.20=,84.20=
split dinner,32.25=96.75 / 3,1,32.25=,116.45=
ferry,50.00,2,100.00=,216.45=
```

### Names, and other files

A type used in many places gets a name, and a column can be a key of another file's table, so a value must exist there. Both go in the prelude, between two `---` lines:

```cells
---
import accounts from "./accounts.cells"
type Money = EUR>=0
---
item:text!,cost:Money,qty:int>=1,total:Money=cost * qty,from:accounts.account
fuel,84.20,1,84.20=,card
split dinner,32.25=96.75 / 3,1,32.25=,card
```

### All of it

A table a script fills, kept in the file, and read by a formula:

````cells
---
import accounts from "./accounts.cells"

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

### Aligned, with its columns in the prelude

A long formula makes a long header. A column can say its type and formula in the prelude instead, `name: type = formula`, and its header cell is then its name alone. `cells fmt` lines the columns up with spaces, which the reader takes as no part of a value:

```cells
---
import accounts from "./accounts.cells"
type Money = EUR>=0
total:   Money = cost * qty
running: Money = running[-1] + total
---
item:text!,     cost:Money,         qty:int>=1, total,      running,    from:accounts.account
fuel,           84.20,              1,          84.20=,     84.20=,     card
split dinner,   32.25=96.75 / 3,    1,          32.25=,     116.45=,    card
ferry,          50.00,              2,          100.00=,    216.45=,    cash
```

Each column starts on a multiple of four characters, two spaces at least after its widest value, so a small edit fits the padding already there, and a write takes it up so the line stays aligned. A value wider than 32 characters is left out of the reckoning. `cells diff` gives git a view with the padding taken out, so realigning a file is no change to review ([EDITORS.md](EDITORS.md)).

## The documents

| | |
|---|---|
| [SPEC.md](SPEC.md) | the format, and in its Open section what is decided in direction and what is not |
| [tests/README.md](tests/README.md) | the conformance suite: what a case is, and how an implementation runs it |
| [rust/README.md](rust/README.md) | the core in Rust: its crates, its API, what it measures, the wasm and Apple builds |
| [ENGINE.md](ENGINE.md) | the engine every app reads and writes through: v0, one Mac, and what comes later |
| [EDITORS.md](EDITORS.md) | the language server, `cells lsp`, and Helix, Neovim and VS Code set up for it |
| [web/README.md](web/README.md) | the site, cellular.computer: how it is built from this repo's engine and ui's items, and its docs drawn from these documents |
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

The prelude, between two `---` lines, names types (`type Kind = …`), imports other files (`import accounts from "../accounts.cells"`, `import { Kind } from "./types.cells"`, a URL pinned by its hash), and holds tables of the file's own, written inline in a `` ```cells `` fence or filled by a script (`rates = run every hour`), with `//` comments.

## Reading and writing

- **A broken cell opens and says why**, never coerced: `row 3, cost: "8x.50" is not a number`. Its text is kept, its value null, so the tool that can fix it can show it. A duplicate key reports both rows, a reference to no row says so (`"bar" is no key of accounts`), and so does a formula that does not compute. A broken grammar takes out only what it breaks, and the file opens with its table: a header cell typed halfway (`to:accounts.a`) is a column of text, named as broken, every other column typed. A control character (a NUL, an ESC, a C1 from a bank's export) breaks its cell in any column, named by its code point, `U+0000`, since it shows nothing, and no writer writes one.
- **Written back byte for byte** but for what changed: the line endings, the quoting, the header's spelling and the prelude as they were read. A write stores every computed value its change reaches, in one pass; a value typed over its `=` or left behind by a plain editor is a problem saying both numbers (`the formula gives 90.00, the file stores 84.20`), never overwritten unasked, and `cells fix` or a client's fix resolves it.
- **Merged by git without conflicts on what is computed**: `cells merge`, git's merge driver, merges what was written and computes the rest again.
- **Records quote as RFC 4180** (a field holding `,`, `"` or a line break is quoted, `""` inside), because it is the right rule for cells.
- **Opening never runs anything.** A script runs when asked (`cells run`), and a file opens with the rows its last run stored.

## Where it is going

Decided in direction, not yet in the spec ([SPEC.md, Open](SPEC.md#open)):

- **Your filesystem can be a graph database.** References join a folder's files, and one that names nothing is a problem; next, tools that see a break before it lands (a key renamed in one file, followed through every file that names it) and fixes offered where a reference names nothing (`did you mean "rent"?`).
- **Every row has an identity**, its key or an `id` the engine assigns, so an edit always lands on the row it meant and two versions of a file merge row by row.
- **One engine every app reads and writes through**: a daemon on one Mac (`cells serve`, its first part built) that every client reads through and that keeps each file's schedules, standalone in a page; files parsed once, changes heard as they happen, nobody's write ever lost, even an editor's that bypasses it. Later every device a full replica, books owned by one of them. [ENGINE.md](ENGINE.md) is the design.
- **Queries in standards, never our own language**: SQL (`cells sql` runs it), and SQL/PGQ to walk references.
- **Charts with no setup**, because the columns are typed: a date is a time axis, money a measure, a choice a category, a reference a link to follow; what a chart needs beyond the types declared, never scripted.
- **Tooling**: highlighting in every editor, and the language server ([EDITORS.md](EDITORS.md)) warning before a break lands: a key renamed in the other file, a reference about to dangle.

## What it is for

The use it is designed against is plain text accounting done properly. Books kept as plain files, typed like a database:

```text
~/books/
├── accounts.cells          every account, once
├── home/money.cells        each movement points at an account
├── travel/money.cells
└── studio/money.cells
```

```text
accounts.cells
---
export type Kind = assets|liabilities|expenses|income
---
account:text!,kind:Kind
assets:bank:main,assets
expenses:home:rent,expenses
expenses:home:power,expenses

home/money.cells
---
import accounts from "../accounts.cells"
---
id:id!,date:date,amount:EUR>0,account_from:accounts.account,account_to:accounts.account,description:text,tax_rate:pct,vat:EUR=amount * tax_rate / (100 + tax_rate)
k7m2q9xa,2026-07-01,950.00,assets:bank:main,expenses:home:rent,rent july,0,0.00=
p3w8c2nd,2026-07-12,64.30,assets:bank:main,expenses:home:power,power july,21,11.16=
q4x9t1mz,2026-07-30,950.00,assets:bank:main,expenses:home:rnet,rent august,0,0.00=
```

```console
$ cells check books
books/home/money.cells row 3, account_to: "expenses:home:rnet" is no key of accounts

$ cells show books/home/money.cells
id        date        amount  account_from      account_to           description  tax_rate  vat
k7m2q9xa  2026-07-01  950.00  assets:bank:main  expenses:home:rent   rent july    0         0.00
p3w8c2nd  2026-07-12  64.30   assets:bank:main  expenses:home:power  power july   21        11.16
q4x9t1mz  2026-07-30  950.00  assets:bank:main  expenses:home:rnet   rent august  0         0.00

$ cells sql books "SELECT account_to, SUM(amount) FROM money GROUP BY account_to ORDER BY account_to"
account_to           sum(amount)
expenses:home:power  64.30
expenses:home:rent   950.00
expenses:home:rnet   950.00
cells sql: 1 problem in 1 file read, a broken cell read as null: cells check books
```

`cells show` writes tab-separated text (aligned here), each cell its value: `vat` is euros, so 64.30 at 21% is `11.16`, rounded half to even at the euro's two digits, the same value the file stores beside its `=`. `cells sql` asks the folder as one database, in DuckDB's SQL: every file is a table named by its file's name (every `money.cells` under the folder is one table, `money`), its columns typed (`amount` is `DECIMAL(38,2)`, summed exactly, `date` a date), and it answers as `show` writes. It ran over a file with a problem, so it says so and exits 1.

Ledgers in plain text (hledger, beancount) keep books readable, diffable and yours, but each invents a syntax of its own and cannot say that a column is euros, that an account must exist, or that VAT is derived from the amount. Cells is the table those books are. [ACCOUNTING.md](ACCOUNTING.md) drafts them whole: pending payments, a payslip in several rows, exchanges, lots, corrections, balances that must hold.

## The pieces

One law and one engine. Every surface binds the same Rust core, so there is one reader to make fast and none to drift.

| Piece | Where | In |
|---|---|---|
| The format | [SPEC.md](SPEC.md) | prose |
| The law: every rule as a case, run against every binding | [`tests/`](tests) | data |
| The engine: parsing at memory speed, typed columns in Arrow's layout (exact decimals for money, days for dates, a dictionary for choices), formulas in exact decimal, imports followed, references checked, problems, the byte-for-byte writer | `rust/` | Rust, no dependencies |
| The command line: `cells check` (a folder, its imports followed), `fmt` (and `--check`), `diff`, `pin`, `run`, `show`, `sql` (DuckDB over the columns, a folder one database, so SQL is the standard's); then SQL/PGQ, which DuckDB has only as an extension it downloads | `rust/` | Rust, DuckDB |
| The daemon ([ENGINE.md](ENGINE.md)), `cells serve`: every client's one way to read and write, each file parsed once and re-parsed when it changes, the references held as a graph, a change stored in every file it reaches, edits by row identity, the schedules kept, never losing a write | `rust/cells-daemon`, `cells::folder` | Rust |
| The protocol: intents and queries in, Arrow frames out, over a Unix socket | `rust/cells-daemon` | Rust |
| The language server, `cells lsp`: diagnostics as you type, hover, completion, definitions, renames followed across the folder | `rust/` | Rust |
| The web: the engine as wasm, a synchronous `read(text)`, its imports followed through the files a page holds, columns read as typed arrays without a copy | `rust/` | Rust, wasm |
| The Apple platforms: the engine behind a C header, swift-utils' `Cells` and `CellsTable` over it | `rust/` + swift-utils | Rust, Swift |
| Python, Go | not started | Rust (PyO3), cgo |
| Highlighting in Helix, Neovim, Zed | `tree-sitter/` | tree-sitter |
| Highlighting in VS Code, GitHub, Shiki | `syntaxes/` | TextMate |

Rust because it is as fast as C and reaches everything else: a C ABI, wasm, PyO3, `tower-lsp`, DuckDB's bindings. Arrow because typed columns are what it is: DuckDB, Python, the browser and Swift take them as they lie, and a chart reads a column, never a row. DuckDB ships with the command line, the daemon and the apps, never in a page. Bun runs a file's scripts, never the core: it is a program, and no page or app can link it. Parquet is an export of the same columns, never the source: in memory both are Arrow, and what text costs at load the daemon's cache pays once, while Parquet can never be read, diffed in git or fixed by hand.

### Built

- **The law**: the suite, 490 cases (`plain/` the files that are not Cells files), run by the engine in Rust, through its wasm build and through Swift.
- **The engine** passes every case, the format as SPEC.md writes it: formulas in header cells and in cells, SQL's expressions in exact decimal, the language in capitals and `TABLE`, `[-1]` and ranges in the lines' order, windows over an order (a running balance, `lag`), names looked for nearest first, a spreadsheet's references taken as input, a currency's scale; tables written inline, columns declared in the prelude, imports and exports (a URL pinned by its hash, a `rev` through git), references checked across files, scripts' tables and schedules read; every computed value stored beside its `=`, a write storing what it changes, the `stale` problem and its two fixes; padding no part of a value, and a write keeping its line aligned; a broken grammar taking out only what it breaks, every break and problem carrying its span (the characters a client marks, pinned by the suite). It reads what people's files are (a `.csv` or `.tsv` by the separator its first lines agree on, never refused for its records, a ragged row read as blanks with its problem), inserts, removes, moves and renames columns, and writes a file in another dialect. It says what completes at a place of the text and what is said of it (`cells::complete`, `cells::hover`), what a cell may hold, and each problem's severity. 1M rows read in 13.8 ms on 18 threads ([rust/README.md](rust/README.md#numbers)).
- **The command line**: `cells check` over a folder with its imports followed, `fix`, `fmt` (aligning the columns) and `fmt --check`, `diff` (git's textconv), `merge` (git's merge driver), `pin`, `run` (with Bun), `show`, `sql` (a folder asked as one database through DuckDB, same-named files one table, every column its exact type), `serve` and `lsp`.
- **The daemon**, `cells serve` (`rust/cells-daemon`, [ENGINE.md](ENGINE.md)): a change written to a file by anything stores every computed cell it reaches, in that file and in every file reading it through its imports, nobody opening them; a value typed over its `=` is left stale for a person. Each version is logged in `.cells/log.jsonl` and each step said. `cells::folder`, in the core, decides (no disk, clock or thread, so it replays from the changes handed in), and one `World` trait, the disk's or a simulation's, carries it out. Clients reach it over its socket (`Cells/engine.sock`, JSON lines), admitted by their code signature (Cells.app and its signed `cells` command), ask for folders and hear every file that changes. Cells.app keeps it running (`CellsEngine.app`, a launch agent nested in the app); without Cells.app, a client that finds none answering starts it.
- **The language server**, `cells lsp` ([EDITORS.md](EDITORS.md)), checked in Helix: diagnostics, hover, completion, definitions, renames across the folder, computed values stored on save, the layout as inlay hints. It is a client of the daemon and watches nothing itself.
- **The web**: the wasm build, 141 KB brotli, with a synchronous `read`; ui's table and code blocks on it; the site, [cellular.computer](https://cellular.computer) ([web/README.md](web/README.md)).
- **The Apple platforms**: the XCFramework, released as `apple-<version>`. swift-utils pins `apple-0.4.0`; its `Cells` binding passes every case through Swift on macOS, iOS and tvOS with its imports followed (`cells_read_with`, and `cells_read_at` from the disk), and has what the other bindings have: padding, a formula's names and the columns they read, the prelude as statements edited by lines, what a cell may hold, each problem's severity, and the completions and hover `cells lsp` gives. Its `CellsTable` and Cells.app show a computed cell's value and edit its formula.
- **The grammars**, tree-sitter and TextMate: imports, tables, fences, formulas, a computed value its own node.
- **[`ts/`](ts)**, the first TypeScript reader, kept as a readable reference: `read`, `write`, `setCell`, `setType`, `parseHeader`, `compose`, and `Infer<"header">`, the row type from a header line at compile time. It predates the suite and fails many of its cases.

### Next, in order

1. **Cells.app reading through the engine.**
2. **A script runs only once its person allowed that version of it** (asked in Cells.app, the editor or `cells allow`), and its schedules are kept.
3. **Cells.app's window**, designed and used until it is right: its folder, its prelude a first-class panel written with completions, heads saying what each column is. Then the site's Cells window and hero mirror it.
4. **SQL/PGQ in `cells sql`**, references walked as edges, once DuckDB's `duckpgq` can be built into the binary ([rust/README.md](rust/README.md#cells-sql)).
5. **Charts** from the types.
6. **The npm package** `@adriangalilea/cells`: the wasm build and its wrapper, published.

### Open

**DuckDB makes `cells` 68 MB.** It is built into the binary for `cells sql`, and `cells lsp`, which every editor window starts, is that binary, so the format's own tools carry a database they never use: not acceptable as it stands. The likely answer is the SQL in a binary of its own, `cells-sql`; the measurements and the options weighed are in [rust/README.md](rust/README.md#cells-sql).

## Where it is used

- **[cellular.computer](https://cellular.computer)**: the format told live, every example read by the wasm engine in the page.
- **ui's table** ([ui.adriangalilea.com/table](https://ui.adriangalilea.com/table)): a `.cells` file opens as a table whose columns edit by their types, its broken cells marked, through `useTableDocument` and the `cells` codec (`lib/cells.ts`, this repo's wasm wrapper and module vendored by ui's `mise cells`).
- **InkTable and Cells for Swift**: the same table on the Apple platforms.
- **Cells.app**: the Mac app for `.cells`, CSV and TSV files, built on them.
