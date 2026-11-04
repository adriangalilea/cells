# TCSV

Typed CSV. A `.tcsv` file is a table in plain text whose header cells carry their column's type, `cost:EUR>=0`, in the file itself: one file, never a sidecar. A TCSV reader gets typed rows, every broken cell reported rather than coerced, and writes the file back byte for byte but for what changed. It is its own format, designed for its own ergonomics, never bent to suit readers that do not know it.

```
#! type Category = travel|stay|food|gear
#! total = cost * qty
item:text!,category:Category,cost:EUR>=0,qty:int>=1,paid:yes/no,on:date
fuel,travel,84.20,1,yes,2026-08-14
groceries,food,96.75,2,no,2026-08-15
```

`item` is the key, `category` one of four words, `cost` euros at least 0, `paid` a boolean the file writes yes and no, `on` a date, `total` computed and never stored. [SPEC.md](SPEC.md) is the whole format.

## Why

The pain it is for: plain text accounting done properly. Ledgers in plain text (hledger, beancount, a `money.csv` per folder) keep the books readable, diffable and yours, but each invents a syntax of its own, checks only what it was built to check, and cannot say that a column is an amount in euros, that an account must exist, or that VAT is derived from the amount. TCSV is the table those books are: typed columns, named types, computed columns, and files that reference each other.

## Where it is going

Not the spec yet; decided in direction, to be written into SPEC.md:

- **A folder is a database.** Files import each other, live: a column can be typed by another file's key (`to:accounts.account`), so an account that does not exist, or one renamed in the other file, is a broken cell naming the file and row that broke it, and a tool says so before the break lands. Never a limit on how files compose.
- **The prelude a fenced block** (`---`) in TCSV's own language, `//` comments, in place of a `#!` on every line.
- **Tooling, so the format is pleasant everywhere:** a tree-sitter grammar and a language server (types checked as you type, a broken reference underlined, completion of a referenced file's keys), highlighting in every editor and in ui's code views.

The books, in that direction:

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
date:date,amount:EUR>0,from:accounts.account,to:accounts.account,description:text,tax_rate:pct
2026-07-01,950.00,assets:bank:main,expenses:home:rent,rent july,0
2026-07-12,64.30,assets:bank:main,expenses:home:power,power july,21
```

## Implementations

- **TypeScript**, the reference: [`ts/`](ts). No dependencies; `read`, `write`, `setCell`, `setType`, `parseHeader`, and `Infer<"header">`, the row type from a header line at compile time. `bun run examples` runs every rule.
- **Swift**: a `TCSV` module, and `TCSVTable`, a document edited in an `InkTable`; not published yet.
- **Go**: TODO.
- **Python**: TODO.

## Where it is used

- **ui's table** ([ui.adriangalilea.com/table](https://ui.adriangalilea.com/table)): a `.tcsv` file opens as a table whose columns edit by their types, its broken cells marked, through `useTableDocument` and the `tcsv` codec (`lib/tcsv.ts`, this repo's `ts/src/tcsv.ts` vendored).
- **InkTable and TCSV for Swift**: the same table on the Apple platforms, and a Swift reader and writer.
- **tally**: a macOS app for CSV, TSV and TCSV files, built on them.
