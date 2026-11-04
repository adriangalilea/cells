# TCSV

Typed CSV. A `.tcsv` file is a table in plain text whose header cells carry their column's type, in the file itself: one file, never a sidecar. It is its own format, designed for its own ergonomics, never bent to suit readers that do not know it. [SPEC.md](SPEC.md) is the whole format, and not final yet.

```
---
type Category = travel|stay|food|gear
total = cost * qty                        // computed, never stored
---
item:text!,category:Category,cost:EUR>=0,qty:int>=1,paid:yes/no,on:date,note:text?
fuel,travel,84.20,1,yes,2026-08-14,
groceries,food,96.75,2,no,2026-08-15,the market on the square
```

## The syntax

A header cell is `name:type`. A cell with no `:` is a plain column, its type read from its values.

| Header cell | The column |
|---|---|
| `item:text!` | text, the key: in every row, unique |
| `category:Category` | one of the words the prelude named |
| `cost:EUR>=0` | euros, at least 0 (any ISO 4217 code) |
| `qty:int>=1` | a whole number, at least 1 |
| `paid:yes/no` | a boolean the file writes `yes` and `no` |
| `on:date` | an ISO date; `on:date(DD/MM/YYYY)` reads the file's own form |
| `note:text?` | `?`: may be blank |
| `mood:'yay!'\|meh` | a choice; single quotes hold a reserved character |
| `score:0..10` | a number from 0 to 10 |

The bases are `text` `int` `num` `pct` `bool` `date` `time` `datetime` `url` `email` and every currency. Every shorthand has a word, so a header reads either way: `cost:EUR>=0` is `cost:EUR min 0`, `paid:yes/no` is `paid:bool yes no`, `item:text!` is `item:text key`.

The prelude, between two `---` lines, names types (`type Category = …`) and computes columns (`total = cost * qty`), with `//` comments.

## Reading and writing

- **A broken cell opens and says why**, never coerced: `row 3, cost: "8x.50" is not a number`. Its text is kept, its value null, so the tool that can fix it can show it. A duplicate key reports both rows.
- **Written back byte for byte** but for what changed: the line endings, the quoting, the header's spelling and the prelude as they were read.
- **Records quote as RFC 4180** (a field holding `,`, `"` or a line break is quoted, `""` inside), because it is the right rule for cells.

## Where it is going

Decided in direction, not yet in the spec ([SPEC.md, Open](SPEC.md#open)):

- **Files that point at each other.** A column typed by another file's key (`account_to:accounts.account`) is a reference, brought in by the prelude (`use ../accounts.tcsv`). A folder of `.tcsv` files is typed tables joined by references: **your filesystem can be a graph database.** A reference that breaks names the file and row that broke it, and tools say so before the break lands. Whether a reference resolves live or is pinned in the file is open.
- **Queries in standards, never our own language**: SQL, and SQL/PGQ to walk references.
- **Charts with no setup**, because the columns are typed: a date is a time axis, money a measure, a choice a category, a reference a link to follow.
- **Tooling**: a tree-sitter grammar, a language server (types checked as typed, a broken reference underlined, a referenced file's keys completed), highlighting in every editor.

## What it is for

The use it is designed against is plain text accounting done properly. Books kept as plain files, typed like a database:

```
~/books/
├── accounts.tcsv          every account, once
├── home/money.tcsv        each movement points at an account
├── travel/money.tcsv
└── studio/money.tcsv
```

```
accounts.tcsv
---
type Kind = assets|liabilities|expenses|income
---
account:text!,kind:Kind
assets:bank:main,assets
expenses:home:rent,expenses
expenses:home:power,expenses

home/money.tcsv
---
use ../accounts.tcsv
vat = amount * tax_rate / (100 + tax_rate)
---
date:date,amount:EUR>0,account_from:accounts.account,account_to:accounts.account,description:text,tax_rate:pct
2026-07-01,950.00,assets:bank:main,expenses:home:rent,rent july,0
2026-07-12,64.30,assets:bank:main,expenses:home:power,power july,21
```

```
$ tcsv sql ~/books "select account_to, sum(amount) from money
                    where account_to like 'expenses:home:%' group by account_to"

$ tcsv check ~/books
travel/money.tcsv row 14: account "assets:bank:mian" does not exist
home/money.tcsv rows 3 and 9: the same invoice twice
```

Ledgers in plain text (hledger, beancount) keep books readable, diffable and yours, but each invents a syntax of its own and cannot say that a column is euros, that an account must exist, or that VAT is derived from the amount. TCSV is the table those books are.

## The pieces

One law, one core, native readers held to the law. TOML is the model: `toml-test` is its law, `taplo` (Rust) its parser, formatter, language server and command line, and every language keeps a reader of its own.

| Piece | Where | In |
|---|---|---|
| The format | [SPEC.md](SPEC.md) | prose |
| The law: every rule as a case, read by every implementation | [`tests/`](tests) | data |
| The core: reader, writer, checker, the folder as a graph | [`rust/`](rust) | Rust |
| The command line: `tcsv check`, `tcsv sql` (DuckDB, so SQL and SQL/PGQ are the standards') | [`rust/`](rust) | Rust |
| The language server: types checked as typed, references completed and followed, semantic highlighting in every editor that speaks LSP | [`rust/`](rust) | Rust |
| Python | the core, bound | Rust (PyO3) |
| Highlighting in Helix, Neovim, Zed | [`tree-sitter/`](tree-sitter) | tree-sitter |
| Highlighting in VS Code, GitHub, Shiki | [`syntaxes/`](syntaxes) | TextMate |
| TypeScript, the browser's reader, no dependencies | [`ts/`](ts) | TypeScript |
| Swift, the Apple platforms' reader | swift-utils' `TCSV` and `TCSVTable` | Swift |
| Go | TODO, native | Go |

Rust for the core because it is as fast as C and reaches every other place: a C ABI, wasm for the web, PyO3 for Python, `tower-lsp` for the server, DuckDB's bindings for queries. The TypeScript and Swift readers stay native so a page needs no wasm and an app no Rust toolchain; the suite keeps them saying the same thing.

The TypeScript reader's API: `read`, `write`, `setCell`, `setType`, `parseHeader`, `compose`, and `Infer<"header">`, the row type from a header line at compile time. `bun run examples` runs its examples.

## Where it is used

- **ui's table** ([ui.adriangalilea.com/table](https://ui.adriangalilea.com/table)): a `.tcsv` file opens as a table whose columns edit by their types, its broken cells marked, through `useTableDocument` and the `tcsv` codec (`lib/tcsv.ts`, this repo's `ts/src/tcsv.ts` vendored).
- **InkTable and TCSV for Swift**: the same table on the Apple platforms.
- **tally**: a macOS app for CSV, TSV and TCSV files, built on them.
