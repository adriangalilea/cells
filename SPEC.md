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

TCSV is its own format, read by TCSV readers. Its records borrow RFC 4180's quoting because it is the right rule for cells, never to stay readable by CSV tools; no rule here is bent for a reader that does not know TCSV. Considered: staying readable by them, as Oils' TSV8 does with an empty first column on every row and a row of types an old reader takes for data. That reader still needs the file cleaned by hand, so every file would pay, on every row, for a compatibility nobody gets.

## The file

- **Text.** A file is UTF-8; one that is not (a Latin-1 export, a byte that begins no character) is an error naming the file. A byte order mark before its first line is no part of that line, and a writer writes it back.
- **Control characters** are Unicode's Cc, U+0000 to U+001F, U+007F and U+0080 to U+009F, but for tab, `\n` and `\r`. They show nothing, so `INV-0042` and `INV-0042` with a NUL after it would be two keys that look one: no part of a file holds one unseen. One in a cell breaks the cell ([Reading](#reading)); one in a header cell, its name or its type, is an error naming the column; one in a prelude line, a comment included, is an error naming the line. A message names the character by its code point, `U+0000`, never by the character.
- **Records** are RFC 4180: fields separated by `,`, a field holding `,`, `"` or a line break is quoted, `""` is a quote inside one. A quoted field opens with `"` and closes at a lone `"`, which the end of the file, a `,` or a line ending follows; text after the closing quote, a `"` inside a field that did not open with one, and a quoted field never closed are errors naming the record (the header, or the row). Inside quotes every character is the field's, line breaks included; a control character quoted is still one.
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

A type opens with what the column holds: a base, a currency, a named type, a choice or a pair. Its format, bounds and marks follow, in any order. A type that opens with a bound has no base written and is a number (below). Spaces and tabs between the parts, and at the type's ends, are nothing (`travel | stay` is `travel|stay`, `text !` is `text!`); a space or a tab is never part of a word unless quoted. A type says each thing once: one base, at most one lower and one upper bound, one format, each mark once, and what a named type it uses already says counts as said. Anything else, two bases, a mark before the base, a part said twice, is an error naming the column.

### Bases

| Type | A cell | Its value |
|---|---|---|
| `text` | any text | the text |
| `int` | `-12`, `0`, `4200`, `+7`, `0042` | a whole number: a sign or none, then digits |
| `num` | `-1.5`, `84.20`, `3e8`, `.5` | a number: a sign or none, digits with a fraction or none (`5.` and `.5` too), then an exponent or none |
| `pct` | `12`, `12.5`, `12%` | a number, the percentage (`12%` is 12): a `num`, then `%` or none |
| `bool` | `true` or `false` | a boolean |
| `date` | `2026-08-14` | a calendar date, ISO 8601: `YYYY-MM-DD`, a day the Gregorian calendar has |
| `time` | `14:30`, `14:30:05` | a time of day: `HH:mm` or `HH:mm:ss`, hours `00` to `23`, minutes and seconds `00` to `59`, never a fraction of a second (`14:30:05.5` is no time) |
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
- **A currency** is any alphabetic code of ISO 4217's current list, in capitals: `EUR`, never `eur`; a withdrawn code (`HRK`) is none. Fund codes (`CLF`, `BOV`), the metals (`XAU`, `XAG`, `XPD`, `XPT`) and `XDR` count, since people hold gold and accounts hold units; `XTS` (for testing) and `XXX` (no currency) do not. An amount is any `num`; the currency's minor unit does not limit its digits (`JPY` takes `1200.5`).

A word that is none of these, no named type and not a choice is an error naming it: `cost:EURO` is `no type or currency "EURO"`, never silently text.

### Choices

`category:travel|stay|food|gear` is one of the words, written bare. A choice holding a reserved character (below) or a space or a tab, inside it or at its ends, is quoted with single quotes, a quote inside one doubled:

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

A format in parentheses says how the file writes the value: `on:date(DD/MM/YYYY)`, `at:time(HH:mm)`. Its letters: `YYYY` `MM` `DD` `HH` `mm` `ss`, everything else literal (`time(HHhmm)` reads `14h30`). Each letter stands for exactly as many digits as it has letters: `MM` is `08`, never `8`. The value read is the same as the ISO form's, and the cell must be a real day or time as the ISO form must; writing keeps the format.

- A format applies to `date` and `time` only, and every other type refuses one: a `datetime` is an instant, and an instant is written as ISO 8601 writes it, its offset included.
- A date's format holds `YYYY`, `MM` and `DD` once each, and no time letter. A time's holds `HH` and `mm` once each, `ss` once or not at all (its value then has `00` seconds), and no date letter. No letter reads a fraction of a second: `ss` is exactly two digits, so `time(HH:mm:ss)` refuses `14:30:05.5`. `date(D/M/YYYY)` is refused: `D` and `M` are literal, so the day and month are never read.
- A format is never empty, and its `(` is closed.

### Marks

| Mark | Means |
|---|---|
| `!` | the key: present in every row and unique |
| `?` | may be blank |

Without `?`, a blank cell is reported as missing. `note:text?` is an optional note; `item:text!` is the key. The two together are refused: a key is present in every row.

### Every shorthand has a word

Both are always valid and mean the same; a cell may mix them.

| Shorthand | Explicit |
|---|---|
| `text!` | `text key` |
| `text?` | `text optional` |
| `travel\|stay\|food` | `one of travel stay food` |
| `yes/no` | `bool yes no` |
| `EUR>=0` | `EUR min 0` |
| `int>0` | `int above 0` |
| `num<=10` | `num max 10` |
| `num<10` | `num below 10` |
| `0..100` | `num between 0 and 100` |

Explicit words are separated by spaces or tabs, never commas (a comma would force the header cell into CSV quotes). A choice with a space inside is quoted: `one of travel 'sd cards'`. `between` takes two numbers joined by `and`, both bounds inclusive. `one of` takes every word after it up to the next explicit word (`key` `optional` `one` `of` `bool` `min` `max` `above` `below` `between` `and`) or mark, and at least one; a choice that spells an explicit word is quoted there: `one of 'key' lock`. `bool` followed by two words that are not explicit words is a pair, and a pair's word that spells one is quoted (`bool 'key' lock`); `bool` alone is `true`/`false`.

### Reserved

Inside a type, outside single quotes: `|` `/` `!` `?` `<` `>` `=` `..` `(` `)` `'` `:`. Inside single quotes nothing is, but `'` itself, doubled; `,` and `"` are never in a type. A name reserves only `:`. Values reserve nothing.

## The prelude

```
---
type Category = travel|stay|food|gear     // a named type
total = cost * qty                        // a computed column
---
```

One statement a line; a blank line is nothing, and spaces and tabs at a line's ends and between a statement's parts are nothing (a tab counts as a space everywhere in the prelude, outside single quotes). A line that is no statement is an error naming it.

- **`type Name = <type>`** names a type for any header cell to use: `category:Category`. A name is ASCII letters, digits and `_`, starting with a capital letter (`type category = …` is refused), and never a currency code. A name is defined once; it may name another named type defined above it, never one below it or itself. A header cell may add to a named type what it does not already say: `cost:Cost?`, `cost:Money>=0`.
- **`name = <expression>`** is a computed column: shown and read as a column after the header's, in the prelude's order, never stored in the rows. Its name is ASCII letters, digits and `_`, not starting with a digit, and no other column's. Its expression is pure and total (it always finishes, the same answer in every implementation): numbers, the names of numeric columns, `+ - * /`, parentheses.
  - A number is digits with a fraction or none (`2`, `0.5`); no sign, no exponent. A `-` before a term negates it (`-cost`, `2 * -qty`).
  - A name is a stored column whose type is `int`, `num`, `pct` or a currency, read or inferred, or a computed column above it. Any other name (no column, a text column, a computed column below) is an error naming the line.
  - `*` and `/` bind before `+` and `-`, each from the left.
  - Arithmetic is exact decimal: `0.1 + 0.2` is `0.3`. A quotient is rounded to 34 significant digits, half to even (`1 / 3` is `0.3333333333333333333333333333333333`).
  - The column is `num?`. A blank or broken input, or a division by zero, makes the computed cell blank. A computed cell is never reported: what a person fixes is its input.
- **`//`** starts a comment to the line's end, in the prelude only, unless it is inside single quotes (`type Path = 'a//b'|c`).
- **No import yet.** Today a file refers to no other; references between files are the next part of the format ([Open](#open)), and `import` is no statement yet.

An error inside a statement (a type that does not read, an expression that names no numeric column) names the statement's line.

## Reading

- **Inference.** A column with no type is read from its values: every non-blank value an `int` is `int`, a `num` `num` (so integers beside one decimal are `num`), every one `true` or `false` `bool`, every one `yes` or `no` a boolean in those words (`yes/no`, a column of `yes` alone too), a `date` `date`, a `datetime` `datetime`, otherwise `text`. Nothing else is inferred: a time, a percentage, a URL, an address, an amount or an `id` is `text`, and so is a column mixing kinds (`true` beside `no`, a date beside an instant). A column with no value at all is `text`. An inferred column is always optional and its type is what its values are, so it is never reported but for a control character (below). An ambiguous value infers the plainer kind: `0042` is an integer, but `0042` beside `A12` is text.
- **A blank cell** is missing: a problem unless the type is `?`, its value null either way. A quoted empty field (`""`) is blank too.
- **A cell that breaks its type loads and is reported, never coerced.** The cell's text is kept as written, its value null, and a problem names the row, the column and why: `row 3, cost: "8x.50" is not a number`. A bound broken is the type broken. The file always opens, so the tool that can fix it can show it.
- **A control character in a cell**, quoted or not, breaks it whatever its column's type, and before the type is read: the cell is reported as any broken cell is, its text kept as written, its value null, and the problem shows the text before the character and names the character by its code point: `row 2, invoice: holds U+0000, a control character, after "INV-0042"`. A `text?` column is no exception, `?` saying only that a cell may be blank. An inferred column reports it too, its one problem, since nothing invisible gets through unseen; the cell takes no part in what the column infers, so `1`, `2`, and `3` with a NUL after it are an `int` column, its third cell broken.
- **Keys.** Two rows hold the same key when their values are equal: `7` and `007` in an `int` key, `1.5` and `1.50` in a currency, two spellings of one instant. A duplicate key is reported on every row that holds it. A blank or broken key is missing or broken, and in no comparison: `INV-0042`, and `INV-0042` with a NUL after it, are no duplicate, the second key being broken.
- A cell has at most one problem: missing, broken (a control character in it its one problem, whatever else its text breaks), or a duplicate key.

## Writing

A writer round-trips: a file read and written with nothing changed is the same bytes. A changed cell is written in its column's form (its format, its pair's words, quoted only when CSV needs it); everything else is written as it was read, the header's spelling (shorthand or explicit), each line's ending, the byte order mark and the prelude included. A cell set past the end of a short row writes the blank fields before it. A writer changing a type writes the shorthand, and rewrites no cell: a cell the new type breaks is reported, not changed. A writer never writes a control character into a cell it changes or adds: text holding one is refused, naming the row, and the file is left as it was. A cell read with one is written back as read, as every unchanged byte is.

The shorthand, as a writer writes it:

- the type: a base, a currency, a named type by its name, a choice's words joined by `|`, a pair's two words joined by `/` (`true/false` is `bool`);
- then its format, then its lower bound (`>=` or `>`), then its upper bound (`<=` or `<`), then `!` or `?`. A `num` with both bounds inclusive is its range alone: `0..10`, never `num>=0<=10`;
- a bound's number in plain decimal: `-` if negative, no exponent, no zero that changes nothing (`1e3` is `1000`, `0.50` is `0.5`, `-0` is `0`);
- a word bare unless it must be quoted: it holds a space, a tab or a reserved character, or it is a choice's only word.

So `cost:EUR min 0 key` is written `cost:EUR>=0!`, and `mood:one of 'so so' meh optional` is written `mood:'so so'|meh?`.

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
- **Imports, written as TypeScript writes them.** A prelude line brings another file in, under the name it gives:
  ```
  ---
  import accounts from "../accounts.tcsv"
  import { Kind, eurusd } from "../rates.tcsv"
  import vat from "https://example.com/es/vat.tcsv" with { sha256: "9c1e7f0a…" }
  ---
  id:id!,date:date,amount:EUR>0,account_from:accounts.account,account_to:accounts.account,description:text
  k7m2q9xa,2026-07-01,950.00,assets:bank:main,expenses:home:rent,rent july
  ```
  The default import is the file's table: its key a type for a column (`account_to:accounts.account`), every column of the row a reference lands on followed by a query or a check (`account_to.kind`). A named import is a type or a table the other file exports (`export type Kind = …`, `export eurusd = …`); what a file does not export is its own, so `accounts.account` only ever means a column. Nothing is copied in. Considered: `use path`, a syntax of our own where one exists that every reader and every highlighter knows, whose import attributes (ES2025) are where a pin goes.
- **A local import is live, a remote one pinned by its content.** A path is resolved at every read, so the graph is always current and nothing is duplicated; breaks are caught by the tools that see every file (the engine, `tcsv check`, the language server) before they land. A URL carries its content's hash, `with { sha256: "…" }`, and the hash is its identity: fetched from wherever it can be (the URL, a mirror, a cache) and accepted only when the bytes match. A URL with no hash is an error that prints the hash to write (`tcsv pin` writes it); a change at the source is reported with what changed, never followed. `with { rev: "a1b2c3d" }` reads a local file as git holds it at that commit, so last quarter's report reads last quarter's files. Data has no versions to resolve: no ranges, no resolver, no lockfile. A file read alone, its imports unreachable, still reads: its reference cells are text, marked unchecked, never broken. Considered: npm and Cargo pin in a lockfile beside the manifest, the sidecar TCSV exists to avoid; Zig keeps a URL and its hash together, as here. Go's public checksum log, proving everyone got the same bytes and not only you, is worth adding once files are published widely.
- **A constraint complains only where it is written.** A column with no type is read from its values and never reported. A file starts loose and tightens where it matters by writing a type in its header: no migration, its rows unchanged, those the new type breaks reported. A reference is a name made strict: it must exist, and tools offer the fix (`did you mean "rent"?`, `add it to accounts.tcsv`), so a new account never means opening the other file by hand. A plain text column is a name left open: any value holds, and one seen once beside a near spelling seen often is a hint, never a problem. A reference into several files joins them with `|`, as a choice joins words (`to:shared.account|clients.account`); a value two of them hold is an error naming both, never a silent pick. Considered: one rule for everyone. beancount declares every account before it is used (no typo gets in, every new account a detour), hledger takes any name (a typo becomes an account, the totals still add up, and it grew `--strict`). Both are right for someone, so the type says which.
- **Every row has an identity.** A file's key (`!`) is its rows' identity. A file with none gets an `id` column the first time the engine writes it: a new base type, `id`, eight characters from a base-32 alphabet, unique within the file, assigned only by the engine and never edited through it. Edits address a row by its identity, never by its position, so a line inserted by hand elsewhere never makes an edit land on the wrong row, and two versions of a file merge row by row. A row without one (added by a tool that bypasses the engine) is found by its text until the engine next writes the file, which gives it one; a copied row's repeated id is replaced on the later copy. Apps built on the engine never show the column.
- **One fact, one row, one file.** A movement is one row holding both its sides (`account_from`, `account_to`), so recording it is one write. A change that must touch several files (an account renamed in every file that references it) is an intent the engine journals before applying file by file, and replays after a crash; every intent is safe to apply twice.
- **What books need**, the accounting primitives (ACCOUNTING.md drafts the books with them):
  - **Money at its scale.** An amount is exact at its currency's minor unit (EUR 2 digits, JPY 0, KWD 3), more digits a problem naming the cell; a rate or a unit price is a `num`. This replaces today's rule that the minor unit does not limit an amount.
  - **An amount whose currency is another cell**, `amount:money in commodity`, or reached through a reference, `money in account.commodity`; and commodities beyond ISO 4217 (a share, a fund, points) declared in a file.
  - **`unique`** on a column that is not the key, among its non-blank cells, and over several columns together.
  - **`final`**: a column written once, never changed by an edit. A movement whose money columns are final is corrected by a new row, never edited, as TigerBeetle never edits a transfer.
  - **References to a file's own rows** (`pending:money.id`), for a payment that settles a hold, a row that corrects another, rows that are one transaction.
  - **`check`**: a rule over many rows or files, a query in the prelude returning the rows that break it, checked before every write and by `tcsv check` (a balance that must hold on a date, an account open on a movement's date). The prelude is where it lives, beside the data it guards; it is where SQL enters a file.
- **A computed column is a header cell with `=`.** `vat:EUR=amount*21/121` sits among the columns, typed like any of them (a currency at its scale, bounds, `?`). Its field in each row is left empty and computed on every read. A value written in one is that row's override, shown as an override and never a problem: the invoice says the VAT was 13.37 where the formula gives 13.36, and the invoice is what happened. This replaces the prelude's computed columns. Considered: the prelude (today's rule), where the column is never where it is read and is always `num?`; and a cached value in each field, as a spreadsheet keeps, a second truth wrong as soon as an input is edited by hand.
  ```
  item:text!,cost:EUR>=0,qty:int>=1,total:EUR=cost*qty
  fuel,84.20,1,
  split dinner,=96.75/3,1,
  ```
- **A cell can hold a formula.** A field starting with `=` is evaluated in its column's type. A formula names what it reads: a column of its own row (`=cost*qty`), a row by its key (`=accounts['assets:bank:main'].kind`, `=eurusd[date].rate`), a whole column (`=sum(cost)`). Rows chosen by a condition are a query, and a running balance a window over dates, both SQL. Considered: coordinates, `=$B$2`. A position survives an inserted row in a spreadsheet only because the program rewrites every reference when it happens; in a file edited by hand or merged by git nothing does, and every `B7` below the new line points at another row without a word. `$` decides how a formula copies when it is filled down, a gesture of a UI; a column's formula is the fill. An app may show letters and numbers, accept `=C2*D2` and write `=cost*qty`.
- **The expression language is SQL's scalar expressions**, in exact decimal: arithmetic, comparisons, `case`, `coalesce`, `round`, `||`, aggregates over a column, and the paths a reference opens. The core evaluates them, so a page and an app compute every formula; `check` and `tcsv sql` are the same language, whole, in DuckDB. An expression stays pure and total: no clock, no network, no file, so a formula from a stranger is as safe to open as a number (the formula injection spreadsheets are known for has nothing here to call). Considered: CEL, pure and total but without a decimal, so money in floats; Excel's formulas, built on positions, their separators changing with the locale; a language of our own, that nobody would want to learn.
- **The prelude names tables.** Every name in it is `name = <source>`: a table written inline in a ` ```tcsv ` fence, or a script's (below). A fence closes at a line of as many backticks as opened it, and what it holds is its language's, so a `//` inside one is no comment of the prelude's. A name is the file's own unless exported, addressed as any table: `fees.kind`, `fees[card].pct`. The prelude is the work; the header and rows after it are what the file shows, and an app shows the prelude folded.
  ````
  ---
  import accounts from "./accounts.tcsv"

  fees = ```tcsv
  kind:text!,pct:pct
  card,1.4
  transfer,0
  ```
  ---
  date:date,amount:EUR>0,to:accounts.account,kind:fees.kind,fee:EUR=amount*fees[kind].pct/100
  2026-11-03,120.00,hotel,card,
  ````
  Considered: a store beside the file, or hidden in it, which would make the file no longer all there is. A file of several sheets, as a workbook: a folder is the workbook and a file one table of it, so a table one file alone needs is written inline, and one another file needs moves to its own file, one import changed and no row.
- **A table can come from a script.** `run` and a schedule, then a fence naming its language. The script exports a function given the last stored rows and returning rows, and the header is its only schema: every row it returns is read by the same reader as a typed one. Under it the engine writes, and only the engine, what the last run returned (a ` ```tcsv ` block, each row naming in a `run` column, the engine's and hidden like `id`, the run that last wrote it) and the runs those rows came from (` ```tcsv runs `: when it ran, its outcome, its error). The time the data is about is the data's own column; the time a run happened is the runs table's.
  ````
  ---
  eurusd = run every day at 18:00 in Europe/Madrid
  ```ts
  export default async function ({ last }) {
    const res = await fetch(`https://api.exchange.example/usdeur?since=${last.date}`)
    return (await res.json()).map(d => ({ date: d.day, rate: String(d.close) }))
  }
  ```
  ```tcsv
  date:date!,rate:num>0,run:runs.run
  2026-11-03,0.9287,9
  2026-11-04,0.9312,9
  ```
  ```tcsv runs
  run:int!,at:datetime,outcome:ok|failed,error:text?
  9,2026-11-04T17:00:03Z,ok,
  12,2026-11-05T17:00:02Z,failed,HTTP 503
  ```
  ---
  date:date,usd:USD>0,note:text?,eur:EUR=usd*eurusd[date].rate
  2026-11-03,180.00,two nights,
  ````
  - **What a run writes.** A table with a key gains the keys it did not have and has the rows whose keys it returned replaced; a table without one is replaced whole. A run that fails, or returns a row its types break, writes nothing but its failure, and the stored rows stay as they were: a program's broken output never lands, where a person's broken cell opens to be fixed. A hand edit to the engine's blocks is reported and replaced at the next run. `runs` keeps the runs some row still names, the last one and the last failure; every run is in the engine's log.
  - **Schedules**, one a line, any of which starts it: `every 15m`, `every hour`, `every day at 18:00`, `every weekday at 09:00`, `every monday at 09:00`, `every month on 1 at 08:00`, `cron 0 9 * * 1-5` for the rest, `when ../money.tcsv changes`. A time of day is in the zone the line names: a file moves between machines, and 18:00 must be one moment on all of them. A run missed while the machine slept runs once on waking, never once per miss; retries are the engine's, the same for every file.
  - **One language, named.** v0 runs ` ```ts `, with Bun. A fence that names no language, or one with no runner, is an error naming it, so the day a second language comes (`python`, through `uv run` and the dependencies its PEP 723 block declares) no file has to be reread to know what it is. The contract under the function is the process (the context on stdin, rows on stdout, logs on stderr), so a runner is all a language needs. Considered: any language a fence names. A notebook's code runs only where its packages are, and of 1.4 million Jupyter notebooks on GitHub 24% ran without an error and 4% gave the results they stored. Python, the language of data, first: its `Decimal` suits money, but `Infer<"header">` in `ts/` already checks a script's rows against the header in the editor, and only JavaScript runs in a page and an app, if scripts ever run there. A Zod or Pydantic model of the rows: a second copy of the header that drifts from it; a language's types for the rows are generated from the header, never written.
  - **Opening never runs anything.** Reading, querying, charting and sending a file run no script: a script runs by `tcsv run` or on its schedule in the daemon, and a file opens with the rows its last run stored, as a spreadsheet opens with its last values. In v0 a script is a program with the whole access of its runner, its imports resolved as Bun resolves them, and the person who runs it carries its risk, as with any program they run. Considered: hosts and secrets declared on the `run` line, enforced as Deno enforces its flags. With no sandbox nothing would enforce them, and a declaration nothing enforces is ceremony; Deno's runtime itself ends in 2027. Trusted folders: trusting a file says nothing of the packages it imports. Adapters compiled to wasm, given only the hosts they declare: the right shape for a sandbox once one is needed, premature before. A fetch described in the file (a URL template, a path into the JSON): it grows into a language of auth, paging and retries, puts keys beside files that are shared, and makes every file opened a beacon to whoever wrote the URL.
- **Tooling, so the format is pleasant everywhere.** A tree-sitter grammar (highlighting in every editor that speaks tree-sitter, and in ui's code views), a language server (types checked as typed, a broken reference underlined, a referenced file's keys completed, a rename followed across the folder), native apps (tally, on the Mac) and web components (ui's table) built for it. Like rainbow CSV, but with the types.
- **Visualisation for free, because it is typed.** A chart needs to know what each column is: a date is a time axis, money is a measure in its currency, a choice is a category, a reference a link to follow. An untyped CSV makes every chart guess or be configured; a TCSV says it, so a table can be charted as it is (spending by month, by account, by vendor), and the graph of files drawn as a graph. Every place that shows TCSV shows its charts: ui's components, the Apple platforms' (Swift Charts over the typed columns), and the Mac app. What a chart needs beyond the types (which columns, which kind of chart) is declared in the prelude, never scripted, so a file sent as a chart is seen without running anything: a spreadsheet's charts are declarations too, and its macros apart.

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
- **Text that starts with `=`.** Where a cell may hold a formula, a text value beginning with `=` needs an escape; which one (a leading `'`, as spreadsheets use, which then needs its own) is open.
- **Narrowing a reference** to some of the rows it names (`from` an account of kind `assets` or `income`): in the header cell (`from:accounts.account(kind=assets|income)`), or as a `check`, which keeps the header readable where the rule spans two files.
- **What was known on a date.** A run replaces a corrected row, so the rate a report used before the correction is in git, not the file. Keeping every superseded row with when it was recorded (bitemporal rows) answers "as known on" from the file itself; `final` money columns already correct by adding rows. Undecided until a case needs more than git.
- **A folder sent as one file.** `tcsv pack` writing one file whose prelude carries a frozen copy of each file it imports, every reference resolved, for someone who should open one thing.
- **Several values in one cell** (`tags:text[]`, split on `;`): useful (tags on a movement), and where a table starts to stretch; undecided.

### Brainstorm, not planned

- **A replicated mode.** Engines on several devices agreeing on the order of the intent log (Viewstamped Replication, as TigerBeetle does), for books where two writers must never both succeed; the files then the cluster's readable view. The engine's write path (deterministic intents, logged) keeps it a protocol on top, never a rewrite (ENGINE.md, Later).
