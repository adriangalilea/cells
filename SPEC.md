# TCSV

> **Not yet final.** This spec is a draft: what follows is what the TypeScript and Swift readers implement today, and [Open](#open) holds what is decided in direction but not written, and what is undecided. Any of it may still change.

Typed CSV. A `.tcsv` file is an RFC 4180 CSV whose header cells carry their column's type, `name:type`, and which may open with a prelude of named types and computed columns. One file, never a sidecar: the types travel with the data. MIME type `text/tcsv`.

```
#! type Category = travel|stay|food|gear
#! total = cost * qty                     // computed, never stored
item:text!,category:Category,cost:EUR>=0,qty:int>=1,paid:yes/no,on:date
fuel,travel,84.20,1,yes,2026-08-14
groceries,food,96.75,2,no,2026-08-15
```

TCSV is its own format, read by TCSV readers. Its records borrow RFC 4180's quoting because it is the right rule for cells, never to stay readable by CSV tools; no rule here is bent for a reader that does not know TCSV.

## The file

- **Records** are RFC 4180: fields separated by `,`, a field holding `,`, `"` or a line break is quoted, `""` is a quote inside one. Lines end in `\n` or `\r\n`; whichever the file uses is kept, and so is whether it ends in a line break.
- **The prelude** is the lines before the header that begin with `#!`. Each is a named type or a computed column (below). It is optional; a file with none starts at its header.
- **The header** is the first line after the prelude. Every other line is a row.
- **A row** has as many fields as the header, or fewer, the missing ones blank. More is an error naming the row.

## A header cell

```
cost:EUR>=0
 ①  ②  ③
```

1. **The name** is everything before the first `:`. A name may hold any character but `:`.
2. **`:`** says a type follows. A cell with no `:` is a plain column, its type inferred from its values (below). So every CSV is a TCSV with no types written.
3. **The type** is everything after it. It never holds a `,` or a `"`, so a header cell never needs CSV's quoting.

## Types

### Bases

| Type | A cell | Its value |
|---|---|---|
| `text` | any text | the text |
| `int` | `-12`, `0`, `4200` | a whole number |
| `num` | `-1.5`, `84.20`, `3e8` | a number |
| `pct` | `12`, `12.5`, `12%` | a number, the percentage (`12%` is 12) |
| `bool` | `true` or `false` | a boolean |
| `date` | `2026-08-14` | a calendar date, ISO 8601 |
| `time` | `14:30`, `14:30:05` | a time of day |
| `datetime` | `2026-08-14T14:30:00Z` | an instant, ISO 8601 |
| `url` | `https://example.com/a` | an absolute URL |
| `email` | `ada@example.com` | an address |
| any ISO 4217 code, `EUR`, `USD`, `JPY` | `84.20` | an amount of money in that currency, a number |

A word that is none of these, no named type and not a choice is an error naming it: `cost:EURO` is `no type or currency "EURO"`, never silently text.

### Choices

`category:travel|stay|food|gear` is one of the words, written bare. A choice holding a reserved character (below) or a space at its ends is quoted with single quotes, a quote inside one doubled:

```
mood:'yay!'|meh|'so-so?'|'it''s fine'
```

### A boolean in the file's own words

`paid:yes/no` is a boolean written `yes` for true and `no` for false, true first. `bool` alone reads `true`/`false`.

### Constraints

| Written | On a number | On text |
|---|---|---|
| `>=0` `>0` `<=100` `<100` | the value | its length |
| `0..100` | at least 0 and at most 100 | a length between |

`cost:EUR>=0`, `pct:0..100`, `name:text<=40`. A type that is only a range is a number: `score:0..10` is `num` from 0 to 10.

### Formats

A format in parentheses says how the file writes the value: `on:date(DD/MM/YYYY)`, `at:time(HH:mm)`. Its letters: `YYYY` `MM` `DD` `HH` `mm` `ss`, everything else literal. The value read is the same as the ISO form's; writing keeps the format.

### Marks

| Mark | Means |
|---|---|
| `!` | the key: present in every row and unique |
| `?` | may be blank |

Without `?`, a blank cell is reported as missing. `note:text?` is an optional note; `item:text!` is the key.

### Every shorthand has a word

Both are always valid and mean the same; a cell may mix them.

| Shorthand | Explicit |
|---|---|
| `text!` | `text key` |
| `text?` | `text optional` |
| `travel\|stay\|food` | `one of travel stay food` |
| `yes/no` | `bool yes no` |
| `EUR>=0` | `EUR min 0` |
| `num<=10` | `num max 10` |
| `0..100` | `num between 0 and 100` |

Explicit words are separated by spaces, never commas (a comma would force the header cell into CSV quotes). A choice with a space inside is quoted: `one of travel 'sd cards'`.

### Reserved

Inside a type, outside single quotes: `|` `/` `!` `?` `<` `>` `=` `..` `(` `)` `'`. A name reserves only `:`. Values reserve nothing.

## The prelude

```
#! type Category = travel|stay|food|gear     // a named type
#! total = cost * qty                        // a computed column
```

- **`#! type Name = <type>`** names a type for any header cell to use: `category:Category`. A name is a word starting with a capital letter; it may name another named type defined above it.
- **`#! name = <expression>`** is a computed column: shown and read as a column after the header's, never stored in the rows. Its expression is pure and total (it always finishes, the same answer in every implementation): numbers, the names of numeric columns, `+ - * /`, parentheses. A blank or broken input makes the computed cell blank.
- **`//`** starts a comment to the line's end, in the prelude only.
- **Never an import.** A file never refers to another: what travels is one file.

## Reading

- **Inference.** A column with no type is read from its values: every non-blank value an integer is `int`, a number `num`, `true`/`false` or `yes`/`no` a boolean in those words, an ISO date `date`, an ISO instant `datetime`, otherwise `text`. An inferred column is always optional. An ambiguous value infers the plainer kind: `0042` is an integer, but `0042` beside `A12` is text.
- **A blank cell** is missing: a problem unless the type is `?`, its value null either way.
- **A cell that breaks its type loads and is reported, never coerced.** The cell's text is kept as written, its value null, and a problem names the row, the column and why: `row 3, cost: "8x.50" is not a number`. A duplicate key reports both rows. The file always opens, so the tool that can fix it can show it.

## Writing

A writer round-trips: a file read and written with nothing changed is the same bytes. A changed cell is written in its column's form (its format, its pair's words, quoted only when CSV needs it); everything else is written as it was read, the header's spelling (shorthand or explicit) and the prelude included. A writer changing a type writes the shorthand.

## Open

### What it is for

The use it is designed against is **plain text accounting done properly**: books kept as plain files (a `money.tcsv` per folder), readable, diffable and the person's own, with what ledgers in plain text never could do: say that an amount is euros, that an account must exist, that VAT is derived from the amount, that an invoice appears once, and answer any question across every file. Every decision below is judged by whether it makes those books better.

### Decided in direction, not yet written

- **TCSV is its own format.** It is read by TCSV readers; no rule is bent to keep a file readable by CSV tools. Compatibility survives only where it costs nothing.
- **The filesystem as a graph database.** Files reference each other: a column typed by another file's key (`account_to:accounts.account`, `vendor:vendors.vendor`) is an edge, so a folder of `.tcsv` files is a graph of typed tables. No limit on how files compose. A reference that breaks (an account that does not exist, a key renamed in the other file) is a problem naming the file and row that broke it, never hidden; a tool notices before a break lands (the language server warns on the rename, the writer refuses to save a dangling reference without saying so).
  ```
  ~/
  ├── accounts.tcsv              account:text!,kind:assets|liabilities|expenses|income
  ├── vendors.tcsv               vendor:text!,name:text,vat_id:text?
  ├── home/money.tcsv            account_from/account_to → accounts, vendor → vendors
  ├── home/garden/money.tcsv
  ├── travel/money.tcsv
  └── studio/money.tcsv
  ```
- **Imports.** A file brings another's names (types, keys) into scope. Undecided between two semantics, or both: resolved live at every read (the graph is always current; a file alone is incomplete), or resolved when written with the imported definitions carried in the file, as a lockfile pins what it uses (a file alone is whole; a stale copy is detectable against its source). The syntax is open (`use ../accounts.tcsv`).
- **The prelude a fenced block.** `---` above and below, TCSV's own language inside, `//` comments; in place of a `#!` on every line, which existed only so CSV tools could skip it.
  ```
  ---
  use ../accounts.tcsv
  type Rate = pct
  vat = amount * tax_rate / (100 + tax_rate)
  ---
  date:date,amount:EUR>0,account_from:accounts.account,account_to:accounts.account,description:text,tax_rate:Rate
  2026-07-01,950.00,assets:bank:main,expenses:home:rent,rent july,0
  ```
- **Tooling, so the format is pleasant everywhere.** A tree-sitter grammar (highlighting in every editor that speaks tree-sitter, and in ui's code views), a language server (types checked as typed, a broken reference underlined, a referenced file's keys completed, a rename followed across the folder), native apps (tally, on the Mac) and web components (ui's table) built for it. Like rainbow CSV, but with the types.
- **Visualisation for free, because it is typed.** A chart needs to know what each column is: a date is a time axis, money is a measure in its currency, a choice is a category, a reference a link to follow. An untyped CSV makes every chart guess or be configured; a TCSV says it, so a table can be charted as it is (spending by month, by account, by vendor), and the graph of files drawn as a graph. Every place that shows TCSV shows its charts: ui's components, the Apple platforms' (Swift Charts over the typed columns), and the Mac app.

### Undecided

- **The query language.** Never one of ours. The candidates:
  - **SQL**, which every reader knows and which already runs over files (DuckDB reads a folder of CSV as tables); a folder of `.tcsv` is typed tables, so `amount` is a number and `date` a date:
    ```
    $ tcsv sql ~ "
      select vendor, sum(amount) from money
      where account_to like 'expenses:house:%' and year(date) = 2026
      group by vendor"
    ```
    `money` is every `money.tcsv` under the folder, one table.
  - **GQL** (ISO/IEC 39075, 2024, Cypher's successor), the shape of the graph: following references from a movement to its account, an account to its parent, a vendor to everything it touched; awkward for sums and grouping, which are most of accounting.
  - **SQL/PGQ** (SQL:2023, property graph queries): GQL's `MATCH` inside SQL, so one query walks the graph and then sums and groups; DuckDB has it (`duckpgq`). The likely answer: the two standards, composed, nothing invented.
    ```
    $ tcsv sql ~ "
      from graph_table(books
        match (m:money)-[:account_to]->(a:accounts)
        where a.account like 'expenses:%'
        columns (a.account, m.amount))
      select account, sum(amount) group by account"
    ```
- **The command line.** `tcsv sql` (above), `tcsv check` (every broken cell, dangling reference and duplicate key across a folder, by file and row), and what else a folder-as-database needs (a visualisation of the graph, a rename followed through every file).
  ```
  $ tcsv check ~
  travel/money.tcsv row 14: account "assets:bank:mian" does not exist (accounts.tcsv)
  home/money.tcsv rows 3 and 9: the same invoice INV-0042 twice
  ```
- **Words about a column and a title for the table.** A header cell has no room for a description without bloating it. With a fenced prelude, the prelude is the natural place; its syntax for them is open.
- **Strictness.** "A broken cell loads and is reported" is the proposal; whether a reader may also be asked to refuse a file is not decided.
- **The expression language.** The arithmetic above is the floor. Its grammar beyond it (comparisons, text, conditionals, functions, reaching into a referenced file), and whether it is CEL itself, is undecided. It stays pure and total: an expression always finishes and gives the same answer in every implementation.
- **Several values in one cell** (`tags:text[]`, split on `;`): useful (tags on a movement), and where a table starts to stretch; undecided.
