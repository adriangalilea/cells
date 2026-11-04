# TCSV

> **Not yet final.** This spec is a draft: what follows is what the conformance suite ([`tests/`](tests)) holds every reader to, and [Open](#open) holds what is decided in direction but not written, and what is undecided. Any of it may still change.

Typed CSV. A `.tcsv` file is a table whose header cells carry their column's type, `name:type`, and which may open with a prelude of named types and computed columns, fenced by `---`. One file, never a sidecar: the types travel with the data. MIME type `text/tcsv`.

```
---
type Category = travel|stay|food|gear
total = cost * qty                        // computed, never stored
---
item:text!,category:Category,cost:EUR>=0,qty:int>=1,paid:yes/no,on:date
fuel,travel,84.20,1,yes,2026-08-14
groceries,food,96.75,2,no,2026-08-15
```

TCSV is its own format, read by TCSV readers. Its records borrow RFC 4180's quoting because it is the right rule for cells, never to stay readable by CSV tools; no rule here is bent for a reader that does not know TCSV.

## The file

- **Text.** A file is UTF-8. A byte order mark before its first line is no part of that line, and a writer writes it back.
- **Records** are RFC 4180: fields separated by `,`, a field holding `,`, `"` or a line break is quoted, `""` is a quote inside one. A quoted field opens with `"` and closes at a lone `"`, which the end of the file, a `,` or a line ending follows; text after the closing quote, a `"` inside a field that did not open with one, and a quoted field never closed are errors naming the record (the header, or the row). Inside quotes every character is the field's, line breaks included.
- **Lines** end in `\n` or `\r\n`. Each line's ending is kept as read, so a file may mix them, and so is whether the file ends in a line break: one at the very end closes the last record and opens none. A `\r` that is not followed by `\n`, outside quotes, is an error naming its record.
- **The prelude** opens when the file's first line is exactly `---` and closes at the next line that is exactly `---`; between them, TCSV's own language (below). It is optional; a file with none starts at its header. A prelude that opens and never closes is an error. A `---` on any other line is no fence: after the prelude it is the header (one column named `---`), below the header it is a row.
- **The header** is the first line after the prelude. Every other line is a row. A file with no line after its prelude (an empty file, or a prelude alone) has no columns and no rows.
- **A row** has as many fields as the header, or fewer, the missing ones blank. More is an error naming the row. A blank line among the rows is a row whose every field is blank.

## A header cell

```
cost:EUR>=0
 ①  ②  ③
```

1. **The name** is everything before the first `:`, spaces included (` item` and `item` are two names). A name may hold any character but `:`, and is at least one character. No two columns share a name, computed columns included.
2. **`:`** says a type follows. A cell with no `:` is a plain column, its type inferred from its values (below). So every CSV is a TCSV with no types written.
3. **The type** is everything after it, and never empty. It never holds a `,` or a `"`, so a header cell never needs CSV's quoting; a name that holds one quotes the whole cell (`"price, net:EUR"`).

## Types

A type opens with what the column holds: a base, a currency, a named type, a choice or a pair. Its format, bounds and marks follow, in any order. A type that opens with a bound has no base written and is a number (below). Spaces between the parts are nothing (`travel | stay` is `travel|stay`, `text !` is `text!`); a space is never part of a word unless quoted. A type says each thing once: one base, at most one lower and one upper bound, one format, each mark once, and what a named type it uses already says counts as said. Anything else, two bases, a mark before the base, a part said twice, is an error naming the column.

### Bases

| Type | A cell | Its value |
|---|---|---|
| `text` | any text | the text |
| `int` | `-12`, `0`, `4200`, `+7`, `0042` | a whole number: a sign or none, then digits |
| `num` | `-1.5`, `84.20`, `3e8`, `.5` | a number: a sign or none, digits with a fraction or none (`5.` and `.5` too), then an exponent or none |
| `pct` | `12`, `12.5`, `12%` | a number, the percentage (`12%` is 12): a `num`, then `%` or none |
| `bool` | `true` or `false` | a boolean |
| `date` | `2026-08-14` | a calendar date, ISO 8601: `YYYY-MM-DD`, a day the Gregorian calendar has |
| `time` | `14:30`, `14:30:05` | a time of day: `HH:mm` or `HH:mm:ss`, hours `00` to `23`, minutes and seconds `00` to `59` |
| `datetime` | `2026-08-14T14:30:00Z` | an instant, ISO 8601 as RFC 3339 writes it (below) |
| `url` | `https://example.com/a` | an absolute URL, as RFC 3986 writes it (below) |
| `email` | `ada@example.com` | an address (below) |
| any ISO 4217 code, `EUR`, `USD`, `JPY` | `84.20` | an amount of money in that currency, a number |
| `id` | `k7m2q9xa` | a row's identity: eight characters of Crockford's base 32 in lower case (`0-9`, `a-z` but `i l o u`), assigned by the engine, never typed by a person |

A cell is read exactly as written: never trimmed, never folded in case. ` 12` is not an `int`, `TRUE` is not a `bool`.

- **`datetime`** is `YYYY-MM-DDTHH:mm:ss`, then a fraction of a second of one to nine digits or none, then `Z` or an offset `+HH:mm` or `-HH:mm` (hours `00` to `23`). `T` and `Z` are capitals, the seconds are always written, the offset never left out: a time with no offset is no instant. Its value is the instant, so `2026-08-14T14:30:00+02:00` and `2026-08-14T12:30:00Z` are one value.
- **`url`** is a scheme (a letter, then letters, digits, `+`, `-` or `.`), `:`, then at least one character, each a letter, a digit, one of `-._~:/?#[]@!$&'()*+,;=`, or `%` and two hexadecimal digits. No space, nothing outside ASCII: a URL with other characters writes them percent-encoded. `mailto:ada@example.com` is a URL; `example.com` and `//example.com/a` are not.
- **`email`** is a local part, `@`, then a domain. The local part is at least one ASCII character from `!` to `~` but `@`. The domain is two or more labels joined by `.`, each label ASCII letters, digits and `-`, never starting or ending with `-`. `ada@localhost` is not an address.
- **`id`** is exactly eight characters, each of `0123456789abcdefghjkmnpqrstvwxyz`: lower case only, never `i`, `l`, `o` or `u`. Its value is the text; two ids are equal when their text is.
- **A currency** is any alphabetic code of ISO 4217's current list, in capitals: `EUR`, never `eur`; a withdrawn code (`HRK`) is none. An amount is any `num`; the currency's minor unit does not limit its digits (`JPY` takes `1200.5`).

A word that is none of these, no named type and not a choice is an error naming it: `cost:EURO` is `no type or currency "EURO"`, never silently text.

### Choices

`category:travel|stay|food|gear` is one of the words, written bare. A choice holding a reserved character (below) or a space, inside it or at its ends, is quoted with single quotes, a quote inside one doubled:

```
mood:'yay!'|meh|'so-so?'|'it''s fine'
```

A word beside `|` is always a choice's word, even one that spells a base or an explicit word (`text|image`, `key|lock`). A choice of one word is quoted (`'paperback'`), since a bare word alone is a type. A cell is one of the words exactly, case and spaces included. A choice's words differ from each other, and none is empty: `''` is refused, a blank cell being missing, never a word.

### A boolean in the file's own words

`paid:yes/no` is a boolean written `yes` for true and `no` for false, true first. `bool` alone reads `true`/`false`, and `true/false` is `bool`. The two words differ, are quoted as a choice's are, and are read exactly, case included.

### Constraints

| Written | On a number | On text |
|---|---|---|
| `>=0` `>0` `<=100` `<100` | the value | its length |
| `0..100` | at least 0 and at most 100 | a length between |

`cost:EUR>=0`, `pct:0..100`, `name:text<=40`, `qty:int>=1<=12`. A type that is only bounds is a number: `score:0..10` is `num` from 0 to 10, `>=0` is `num` at least 0. A range after a base bounds it: `int 1..5`, `text 1..40`.

- Bounds apply to `int`, `num`, `pct`, a currency, and `text`; every other type refuses them. A `pct` is bounded by its percentage: `pct<=100` refuses `150%`.
- A bound is a `num` (`-1.5`, `3e8`). On text it is a whole number, at least 0.
- A text's length is its count of Unicode code points: `café` written with `é` is 4, written with `e` and a combining accent is 5.
- Bounds no value can meet are refused: a lower bound above the upper, or the two equal with either exclusive. `5..5` is 5 alone.

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
---
type Category = travel|stay|food|gear     // a named type
total = cost * qty                        // a computed column
---
```

One statement a line; a blank line is nothing.

- **`type Name = <type>`** names a type for any header cell to use: `category:Category`. A name is a word starting with a capital letter; it may name another named type defined above it.
- **`name = <expression>`** is a computed column: shown and read as a column after the header's, never stored in the rows. Its expression is pure and total (it always finishes, the same answer in every implementation): numbers, the names of numeric columns, `+ - * /`, parentheses. A blank or broken input makes the computed cell blank.
- **`//`** starts a comment to the line's end, in the prelude only.
- **No import yet.** Today a file refers to no other; references between files are the next part of the format ([Open](#open)).

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

- **The filesystem as a graph database.** Files reference each other: a column typed by another file's key (`account_to:accounts.account`, `vendor:vendors.vendor`) is an edge, so a folder of `.tcsv` files is a graph of typed tables. No limit on how files compose. A reference that breaks (an account that does not exist, a key renamed in the other file) is a problem naming the file and row that broke it, never hidden; a tool notices before a break lands (the language server warns on the rename, the writer refuses to save a dangling reference without saying so).
  ```
  ~/books/
  ├── accounts.tcsv              account:text!,kind:assets|liabilities|expenses|income
  ├── vendors.tcsv               vendor:text!,name:text,vat_id:text?
  ├── home/money.tcsv            account_from/account_to → accounts, vendor → vendors
  ├── home/garden/money.tcsv
  ├── travel/money.tcsv
  └── studio/money.tcsv
  ```
- **`use`.** A prelude statement brings another file into scope: `use ../accounts.tcsv`, named by its file's stem (`accounts`), or `use ../travel/money.tcsv as trip` where two stems would clash. A `use` brings the whole file: its named types, its key as a type for a column (`account_to:accounts.account`), and every column of the row a reference lands on, so a query or a check follows it (`account_to.kind`). `use`, never `import`: nothing is copied in.
  ```
  ---
  use ../accounts.tcsv
  type Rate = pct
  vat = amount * tax_rate / (100 + tax_rate)
  ---
  id:id!,date:date,amount:EUR>0,account_from:accounts.account,account_to:accounts.account,description:text,tax_rate:Rate
  k7m2q9xa,2026-07-01,950.00,assets:bank:main,expenses:home:rent,rent july,0
  ```
- **A local `use` is live, a remote one pinned.** A path is resolved at every read, so the graph is always current and nothing is duplicated; breaks are caught by the tools that see every file (the engine, `tcsv check`, the language server) before they land. A URL is pinned by its content's hash (`use https://example.com/rates.tcsv @ sha256:…`): fetched once, a change at the source reported, never followed silently. A file read alone, its references unreachable, still reads: its reference cells are text, marked unchecked, never broken.
- **Every row has an identity.** A file's key (`!`) is its rows' identity. A file with none gets an `id` column the first time the engine writes it: a new base type, `id`, eight characters from a base-32 alphabet, unique within the file, assigned only by the engine and never edited through it. Edits address a row by its identity, never by its position, so a line inserted by hand elsewhere never makes an edit land on the wrong row, and two versions of a file merge row by row. A row without one (added by a tool that bypasses the engine) is found by its text until the engine next writes the file, which gives it one; a copied row's repeated id is replaced on the later copy. Apps built on the engine never show the column.
- **One fact, one row, one file.** A movement is one row holding both its sides (`account_from`, `account_to`), so recording it is one write. A change that must touch several files (an account renamed in every file that references it) is an intent the engine journals before applying file by file, and replays after a crash; every intent is safe to apply twice.
- **Tooling, so the format is pleasant everywhere.** A tree-sitter grammar (highlighting in every editor that speaks tree-sitter, and in ui's code views), a language server (types checked as typed, a broken reference underlined, a referenced file's keys completed, a rename followed across the folder), native apps (tally, on the Mac) and web components (ui's table) built for it. Like rainbow CSV, but with the types.
- **Visualisation for free, because it is typed.** A chart needs to know what each column is: a date is a time axis, money is a measure in its currency, a choice is a category, a reference a link to follow. An untyped CSV makes every chart guess or be configured; a TCSV says it, so a table can be charted as it is (spending by month, by account, by vendor), and the graph of files drawn as a graph. Every place that shows TCSV shows its charts: ui's components, the Apple platforms' (Swift Charts over the typed columns), and the Mac app.

### Undecided

- **The query language.** Never one of ours. The candidates:
  - **SQL**, which every reader knows and which already runs over files (DuckDB reads a folder of CSV as tables); a folder of `.tcsv` is typed tables, so `amount` is a number and `date` a date:
    ```
    $ tcsv sql ~/books "
      select vendor, sum(amount) from money
      where account_to like 'expenses:home:%' and year(date) = 2026
      group by vendor"
    ```
    `money` is every `money.tcsv` under the folder, one table.
  - **GQL** (ISO/IEC 39075, 2024, Cypher's successor), the shape of the graph: following references from a movement to its account, an account to its parent, a vendor to everything it touched; awkward for sums and grouping, which are most of accounting.
  - **SQL/PGQ** (SQL:2023, property graph queries): GQL's `MATCH` inside SQL, so one query walks the graph and then sums and groups; DuckDB has it (`duckpgq`). The likely answer: the two standards, composed, nothing invented.
    ```
    $ tcsv sql ~/books "
      from graph_table(books
        match (m:money)-[:account_to]->(a:accounts)
        where a.account like 'expenses:%'
        columns (a.account, m.amount))
      select account, sum(amount) group by account"
    ```
- **The command line.** `tcsv sql` (above), `tcsv check` (every broken cell, dangling reference and duplicate key across a folder, by file and row), and what else a folder-as-database needs (a visualisation of the graph, a rename followed through every file).
  ```
  $ tcsv check ~/books
  travel/money.tcsv row 14: account "assets:bank:mian" does not exist (accounts.tcsv)
  home/money.tcsv rows 3 and 9: the same invoice INV-0042 twice
  ```
- **Words about a column and a title for the table.** A header cell has no room for a description without bloating it; the prelude is the natural place, its syntax for them open.
- **Strictness.** "A broken cell loads and is reported" is the proposal; whether a reader may also be asked to refuse a file is not decided.
- **The expression language.** The arithmetic above is the floor. Its grammar beyond it (comparisons, text, conditionals, functions, reaching into a referenced file), and whether it is CEL itself, is undecided. It stays pure and total: an expression always finishes and gives the same answer in every implementation.
- **Several values in one cell** (`tags:text[]`, split on `;`): useful (tags on a movement), and where a table starts to stretch; undecided.
- **The accounting model.** TigerBeetle's is the reference to study: accounts and transfers, amounts as integers, pending transfers posted or voided later, linked transfers that succeed or fail together.

### Brainstorm, not planned

- **A replicated mode.** Daemons in a cluster agreeing on the order of the engine's intent log (Viewstamped Replication, as TigerBeetle does), for books where two writers must never both succeed; the files then the cluster's readable view. The engine's write path (deterministic intents, logged) keeps it a protocol on top, never a rewrite.
