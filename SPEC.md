# TCSV

> **Not yet final.** This spec is a draft: what follows is what the conformance suite ([`tests/`](tests)) holds every reader to, and [Open](#open) holds what is decided in direction but not written, and what is undecided. Any of it may still change.

Typed CSV. A `.tcsv` file is a table whose header cells carry their column's type, `name:type`, and its formula when it computes, `name:type=formula`; it may open with a prelude, fenced by `---`, that names types, imports other files and holds tables of its own, written inline or filled by a script. One file, never a sidecar: the types travel with the data. MIME type `text/tcsv`.

```
---
type Category = travel|stay|food|gear
---
item:text!,category:Category,cost:EUR>=0,qty:int>=1,total:EUR=cost * qty,paid:yes/no,on:date
fuel,travel,84.20,1,,yes,2026-08-14
groceries,food,`=96.75 / 3`,2,,no,2026-08-15
```

TCSV is its own format, read by TCSV readers. Its records borrow RFC 4180's quoting because it is the right rule for cells, never to stay readable by CSV tools; no rule here is bent for a reader that does not know TCSV. Considered: staying readable by them, as Oils' TSV8 does with an empty first column on every row and a row of types an old reader takes for data. That reader still needs the file cleaned by hand, so every file would pay, on every row, for a compatibility nobody gets.

## The file

- **Text.** A file is UTF-8; one that is not (a Latin-1 export, a byte that begins no character) is an error naming the file. A byte order mark before its first line is no part of that line, and a writer writes it back.
- **Control characters** are Unicode's Cc, U+0000 to U+001F, U+007F and U+0080 to U+009F, but for tab, `\n` and `\r`. They show nothing, so `INV-0042` and `INV-0042` with a NUL after it would be two keys that look one: no part of a file holds one unseen. One in a cell breaks the cell ([Reading](#reading)); one in a header cell, its name or its type, is an error naming the column; one in a prelude line, a comment or a fence's content included, is an error naming the line. A message names the character by its code point, `U+0000`, never by the character.
- **Records** are RFC 4180: fields separated by `,`, a field holding `,`, `"` or a line break is quoted, `""` is a quote inside one. A quoted field opens with `"` and closes at a lone `"`, which the end of the file, a `,` or a line ending follows; text after the closing quote, a `"` inside a field that did not open with one, and a quoted field never closed are errors naming the record (the header, or the row). Inside quotes every character is the field's, line breaks included; a control character quoted is still one.
- **Lines** end in `\n` or `\r\n`. Each line's ending is kept as read, so a file may mix them, and so is whether the file ends in a line break: one at the very end closes the last record and opens none. A `\r` that is not followed by `\n`, outside quotes, is an error naming its record.
- **The prelude** opens when the file's first line is exactly `---` and closes at the next line that is exactly `---` outside a fence ([The prelude](#the-prelude)); between them, TCSV's own language. It is optional; a file with none starts at its header. A prelude that opens and never closes is an error. A `---` on any other line is no fence: after the prelude it is the header (one column named `---`), below the header it is a row.
- **The header** is the first line after the prelude. Every other line is a row. A file with no line after its prelude (an empty file, or a prelude alone) has no columns and no rows.
- **A row** has as many fields as the header, or fewer, the missing ones blank. More is an error naming the row. A blank line among the rows is a row whose every field is blank.

## A header cell

```
cost:EUR>=0        total:EUR=cost * qty
 ①  ②  ③            ①   ②  ③  ④  ⑤
```

1. **The name** is everything before the first `:`, spaces included (` item` and `item` are two names). A name may hold any character but `:`, and is at least one character. No two columns share a name.
2. **`:`** says a type follows. A cell with no `:` is a plain column, its type inferred from its values (below). So every CSV is a TCSV with no types written.
3. **The type** is everything after it up to its formula, and never empty. It never holds a `,` or a `"`; a name that holds one quotes the whole cell (`"price, net:EUR"`).
4. **`=`** says a formula follows: the first `=` outside single quotes that closes no `>=` or `<=`.
5. **The formula** is everything after it, spaces and tabs at its ends cut, and never empty ([Formulas](#formulas)). A formula that holds a `,` or a `"` (`round(cost, 2)`) quotes the whole cell, as any field that holds one is quoted.

## Types

A type opens with what the column holds: a base, a currency, a named type, a choice, a pair or a reference. Its format, bounds and marks follow, in any order. A type that opens with a bound has no base written and is a number (below). Spaces and tabs between the parts, and at the type's ends, are nothing (`travel | stay` is `travel|stay`, `text !` is `text!`); a space or a tab is never part of a word unless quoted. A type says each thing once: one base, at most one lower and one upper bound, one format, each mark once, and what a named type it uses already says counts as said. Anything else, two bases, a mark before the base, a part said twice, is an error naming the column.

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
- **A currency** is any alphabetic code of ISO 4217's current list, in capitals: `EUR`, never `eur`; a withdrawn code (`HRK`) is none. Fund codes (`CLF`, `BOV`), the metals (`XAU`, `XAG`, `XPD`, `XPT`) and `XDR` count, since people hold gold and accounts hold units; `XTS` (for testing) and `XXX` (no currency) do not. A written amount is any `num`; the currency's minor unit does not limit its digits (`JPY` takes `1200.5`), where a formula's result is rounded to it ([What a formula gives](#what-a-formula-gives)).

A word that is none of these, no named type, no reference and not a choice is an error naming it: `cost:EURO` is `no type or currency "EURO"`, never silently text.

### Choices

`category:travel|stay|food|gear` is one of the words, written bare. A choice holding a reserved character (below) or a space or a tab, inside it or at its ends, is quoted with single quotes, a quote inside one doubled:

```
mood:'yay!'|meh|'so-so?'|'it''s fine'
```

A word beside `|` is always a choice's word, even one that spells a base or an explicit word (`text|image`, `key|lock`). A choice of one word is quoted (`'paperback'`), since a bare word alone is a type. A cell is one of the words exactly, case and spaces included. A choice's words differ from each other, and none is empty: `''` is refused, a blank cell being missing, never a word.

### A boolean in the file's own words

`paid:yes/no` is a boolean written `yes` for true and `no` for false, true first. `bool` alone reads `true`/`false`, and `true/false` is `bool`. The two words differ, are quoted as a choice's are, and are read exactly, case included.

### References

`to:accounts.account` is a key of another table: `accounts`, a table the prelude names ([The prelude](#the-prelude)), and `account`, its key column (`!`). Several tables join with `|`, as a choice's words do: `to:shared.account|clients.account`. A cell holds the key's text, read as the key column reads it (`7` and `007` are one key of an `int`). A word `name.column` is a reference only when the prelude names a table `name`; a table named twice in one type is refused, and so is a column that is no key of its table, naming the column. A reference column is a name made strict: what it holds must exist ([Reading](#reading)), and a plain `text` column is the same name left open, any value holding. Considered: one rule for everyone. beancount declares every account before it is used (no typo gets in, every new account a detour), hledger takes any name (a typo becomes an account, the totals still add up, and it grew `--strict`). Both are right for someone, so the type says which.

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

Inside a type, outside single quotes: `|` `/` `!` `?` `<` `>` `=` `..` `(` `)` `'` `:`. Inside single quotes nothing is, but `'` itself, doubled; `,` and `"` are never in a type. A name reserves only `:`. Values reserve nothing but a formula's backticks ([A formula in a cell](#a-formula-in-a-cell)).

## The prelude

````
---
import accounts from "../accounts.tcsv"       // the shared accounts
type Kind = card|transfer

fees = ```tcsv
kind:Kind!,pct:pct
card,1.4
transfer,0
```
---
````

The prelude is the work; the header and rows after it are what the file shows, and an app shows the prelude folded. One statement a line, a fence's content apart; a blank line is nothing, and spaces and tabs at a line's ends and between a statement's parts are nothing (a tab counts as a space everywhere in the prelude, outside quotes). A line that is no statement is an error naming it. Statements are read in order: each sees the types and tables named above it, never one below. A name is a type's or a table's, once in the file: one named twice, whatever the two are, is an error naming the second. An error inside a statement (a type that does not read, an import that does not follow, a table's cell grammar) names the statement's line, or the line inside the table's fence it is on.

- **`//`** starts a comment to the line's end, outside single and double quotes and outside fences (`type Path = 'a//b'|c`, `"https://example.com/r.tcsv"`).
- **A fence** opens with three backticks or more and its language, `` ```tcsv ``, and closes at the first line that is as many backticks and nothing else, spaces and tabs aside. What it holds is its language's, so a `//` or a `---` inside one is no comment and no end of the prelude's. A fence never closed is an error naming the line it opens on.
- **A computed column** is no statement: `total = cost * qty` is an error naming its line. It is a header cell, `total:EUR=cost * qty` ([Formulas](#formulas)).

### Named types

**`type Name = <type>`** names a type for any header cell to use: `category:Category`. A name is ASCII letters, digits and `_`, starting with a capital letter (`type category = …` is refused), and never a currency code. A name is defined once; it may name another named type defined above it, never one below it or itself. A header cell may add to a named type what it does not already say: `cost:Cost?`, `cost:Money>=0`. **`export type Name = <type>`** names one another file may import.

### Imports

```
import accounts from "../accounts.tcsv"
import { Kind, fees } from "./rates.tcsv"
import vat from "https://example.com/es/vat.tcsv" with { sha256: "9c1e7f0a…" }
import last_year from "./money.tcsv" with { rev: "a1b2c3d" }
```

- **The default import**, `import accounts from "…"`, names the other file's table: its key a type for a column (`to:accounts.account`), its columns read by a formula through a reference or by key.
- **A named import**, `import { Kind, fees } from "…"`, takes what the other file exports: a type (`export type Kind = …`) or a table (`export fees = …`). A name it does not export is an error naming the line, the names it does export said. What a file does not export is its own, so `accounts.account` only ever means a column.
- **Where from** is a quoted path, read from the importing file's folder (`"../accounts.tcsv"`), or a URL. Nothing is copied in: a local import is read again at every read, so it is always current.
- **A URL is pinned by its content**: `with { sha256: "…" }`, the hash of its bytes in lower-case hexadecimal, is its identity. A URL with no hash is an error naming the line (`tcsv pin` writes it). Bytes that do not match the hash are an error naming the line, the hash they have said; a change at the source is never followed. A reader fetches from wherever the bytes can be had (the URL, a cache of them by their hash).
- **`with { rev: "…" }`** reads a local file as its history holds it at that revision (git's, for the command line), so last quarter's report reads last quarter's files. A reader that holds no history reads it as unreachable.
- **A file that imports itself**, directly or through others, is an error naming the line.
- **An import that cannot be reached** (a file not there, a URL not fetched) is no error: the file still reads. A reference into the table it names is unchecked, a formula reading it gives null, a type it names (a name with a capital) reads as `text`.

Data has no versions to resolve: no ranges, no resolver, no lockfile. Considered: `use path`, a syntax of our own where one exists that every reader and every highlighter knows, whose import attributes (ES2025) are where a pin goes. npm and Cargo pin in a lockfile beside the manifest, the sidecar TCSV exists to avoid; Zig keeps a URL and its hash together, as here.

### Tables written inline

**`name = ```tcsv`**, its rows on the lines after it and a closing fence: a table of the file's own, a TCSV table (a header and rows, no prelude of its own) whose types may be the prelude's above it and whose references and formulas may read the tables above it. **`export name = ```tcsv`** is one another file may import. It is read as any table is: its problems are its own (`tcsv check` reports them under its name), and the file's formulas read it by name: `fees[kind].pct`, `sum(fees.pct)`. Considered: a store beside the file, or hidden in it, which would make the file no longer all there is. A file of several sheets, as a workbook: a folder is the workbook and a file one table of it, so a table one file alone needs is written inline, and one another file needs moves to its own file, one import changed and no row.

### Tables a script fills

````
eurusd = run every day at 18:00 in Europe/Madrid
```ts
export default async function ({ last }) {
  const res = await fetch(`https://api.exchange.example/usdeur?since=${last.date}`)
  return (await res.json()).map(d => ({ date: d.day, rate: String(d.close) }))
}
```
```tcsv
date:date!,rate:num>0,run:int?
2026-11-03,0.9287,9
2026-11-04,0.9312,9
```
```tcsv runs
run:int!,at:datetime,outcome:ok|failed,error:text?
9,2026-11-04T17:00:03Z,ok,
12,2026-11-05T17:00:02Z,failed,HTTP 503
```
````

**`name = run <schedule>`**, then three fences, blank lines between them nothing ([Running](#running) says what runs them):

1. **The script**, a fence naming its language. A fence that names none, or one no runner runs, is an error naming the line it opens on: v0 runs `ts`, TypeScript, with Bun.
2. **The table**, a `` ```tcsv `` fence: its header the table's, written by its author, at least; its rows what the last run returned. It is read as a table written inline is, and is the table the name names. A script with no table, or a table with no header, is an error naming the line.
3. **The runs**, a `` ```tcsv runs `` fence, the engine's: each run some row still names, the last one and the last failure, with when it ran, its outcome and its error. A table with none has not run.

Any other fence after a script, or one of these twice, is an error naming the line it opens on. **The schedule** is triggers joined by `,`, any of which starts a run, then `in <zone>` (`in Europe/Madrid`, `in UTC`) for their times of day:

| Trigger | Starts a run |
|---|---|
| `every minute` · `every 15m` · `every hour` · `every 6h` | at that interval |
| `every day at 18:00` | each day |
| `every weekday at 09:00` | Monday to Friday |
| `every monday at 09:00` | each week, the day named (`monday` to `sunday`) |
| `every month on 1 at 08:00` | each month, the day of the month named (1 to 31) |
| `cron 0 9 * * 1-5` | as cron's five fields say |
| `when ../money.tcsv changes` | each time that file changes |

A trigger with a time of day (and `cron`) needs the zone: a file moves between machines, and 18:00 must be one moment on all of them. A schedule that says none of these is an error naming the line.

## Formulas

A formula computes a cell's value from what the file can read. It is written in a column's header cell, `total:EUR=cost * qty`, where it computes every cell of the column whose field is blank, or in one cell, `` `=96.75 / 3` ``, where it computes that cell alone.

```
item:text!,cost:EUR>=0,qty:int>=1,total:EUR=cost * qty
fuel,84.20,1,
split dinner,`=96.75 / 3`,1,
invoice 42,80.00,1,79.99
```

### A column's formula

- **A blank field** of a column with a formula is computed: its value is what the formula gives on its row. It is never missing, whatever the column's `?`.
- **A written field** is the row's override: read by the column's type as any cell is, shown as an override and never a problem for being one. The invoice says the VAT was 13.37 where the formula gives 13.36, and the invoice is what happened.
- **A formula may read any column**, after its own or before it, and other formulas' columns: they are computed in the order their reads need. A formula that reads itself, directly or through others, is an error naming its column.
- Considered: the prelude, where the column was never where it is read and was always `num?`; and a cached value in each field, as a spreadsheet keeps, a second truth wrong as soon as an input is edited by hand.

### A formula in a cell

- **A field that is a backtick, `=`, a formula and a backtick**, `` `=96.75 / 3` ``, holds its own formula, in any column of a `.tcsv` file (a plain file's cells are text). Its value is what the formula gives on its row, read by the column's type. A field holding a `,` or a `"` (`` "`=round(qty * 2.25, 1)`" ``) is quoted as any field is.
- **A bare `=` is text**: `=> see invoice` is a note. Text that would itself read as a formula (backticks, `=`, anything, a closing backtick) is written with its first backtick doubled, which reading takes away: ```` ``=x` ```` is the text `` `=x` ``. Considered: a leading `=`, as spreadsheets write one, escaped by a leading `'`. That makes `'` special at the start of every cell and taxes the common case: notes beginning with `=` are common, a single cell's formula rare.
- **A cell's formula that does not compute** (it does not read, it names what is not there, it gives what its column cannot hold, it reads its own cell through others) is a problem of kind `formula` on its cell, its value null, the problem saying why.
- **A formula names what it reads, never where it lies**: a column of its own row, a row by its key, a whole column. Considered: coordinates, `=$B$2`. A position survives an inserted row in a spreadsheet only because the program rewrites every reference when it happens; in a file edited by hand or merged by git nothing does, and every `B7` below the new line points at another row without a word. `$` decides how a formula copies when it is filled down, a gesture of a UI; a column's formula is the fill. An app may show letters and numbers, accept `=C2*D2` and write `` `=cost * qty` ``.

### The language

SQL's scalar expressions, in exact decimal. Keywords are read in any case.

| Written | Is |
|---|---|
| `2`, `0.5` | a number: digits with a fraction or none, no sign, no exponent (`-` before a term negates it) |
| `'two nights'` | text, a `'` inside doubled |
| `true` `false` `null` | a boolean, and nothing |
| `cost` | the column `cost` of the row; `"price net"`, quoted, a column whose name is no word of letters, digits and `_` |
| `to.kind` | through the reference `to`, the column `kind` of the row it names; each `.` follows one more reference |
| `fees[kind].pct` · `eurusd['2026-11-04'].rate` | a table's row by its key (the key compared as the key column's value, so the text `'2026-11-04'` finds a date), then its column |
| `+ - * /` | arithmetic on numbers |
| `\|\|` | text joined, a number or a date as it is written |
| `= <> != < <= > >=` · `is null` · `is not null` | comparisons, of two of one kind (text and a date compare as the date's ISO text) |
| `and` `or` `not` | on booleans, SQL's three values: `false and null` is `false`, `true or null` `true` |
| `case when … then … else … end` | the first `then` whose `when` is true, else the `else`, else null |
| `round(x)` `round(x, 2)` `abs` `floor` `ceil` | numbers: half to even, to that many digits after the point |
| `coalesce(a, b, …)` · `nullif(a, b)` | the first that is not null · null when the two are equal |
| `lower` `upper` `trim` `length` | text |
| `sum(cost)` `avg` `min` `max` `count` | over every row of the column (`sum(fees.pct)`: of the table named), nulls left out |

`or` binds loosest, then `and`, `not`, a comparison, `||`, `+` and `-`, `*` and `/`, then a `-` before a term; each from the left, parentheses grouping. A formula is pure and total: it always finishes, with the same answer in every implementation, and reads no clock, no network and no file, so a formula from a stranger is as safe to open as a number (the formula injection spreadsheets are known for has nothing here to call).

- **Numbers are exact**: `0.1 + 0.2` is `0.3`. `+ - *` are exact; a quotient is rounded to 34 significant digits, half to even (`1 / 3` is `0.3333333333333333333333333333333333`). A division by zero is null.
- **Null goes through**: arithmetic, a comparison or a function given null gives null (but `coalesce`, `nullif`, `is null`, `and`, `or`). A blank or broken cell, a reference to no row, a table not reached are null.
- **Kinds are checked before anything runs**: an `int`, `num`, `pct` or currency column is a number, a `bool` or a pair a boolean, a `date` a date, every other column text. Arithmetic on text, a comparison of a number with text, a formula naming no column are errors: a column's formula naming its column, a cell's a problem of kind `formula`.
- **What a formula gives fits its column**: a number a numeric column, a boolean a boolean one, a date or text a `date` one; any other column takes what it gives as text. Anything else is refused as an error is.

Considered: CEL, pure and total but without a decimal, so money in floats; Excel's formulas, built on positions, their separators changing with the locale; a language of our own, that nobody would want to learn.

### What a formula gives

What a formula gives is read by its column's type as a written cell's text would be:

- **an amount** at its currency's minor unit, ISO 4217's, rounded half to even: `11.159504…` is `11.16` in `EUR`, `1234.5` is `1234` in `JPY`, `2.3451` is `2.345` in `KWD`; a unit with none (the metals, `XDR`) keeps every digit;
- **any other number** at the most digits after the point any of its column's results has (fewer when 38 digits would not hold the largest), an `int` column's as a whole number;
- **a boolean** in the column's words (`yes`, `no`), a date in its format, text as it is;
- **nothing** (null) blank: never missing, since what a person fixes is its input.

A result the type breaks (a negative `net` in an `EUR>=0` column, `2.5` in an `int` one) is a problem on its cell, of kind `type`, the problem saying what the formula gave: `row 1, net: the formula gives "-30", which is below 0`.

## Reading

- **Inference.** A column with no type is read from its values: every non-blank value an `int` is `int`, a `num` `num` (so integers beside one decimal are `num`), every one `true` or `false` `bool`, every one `yes` or `no` a boolean in those words (`yes/no`, a column of `yes` alone too), a `date` `date`, a `datetime` `datetime`, otherwise `text`. Nothing else is inferred: a time, a percentage, a URL, an address, an amount or an `id` is `text`, and so is a column mixing kinds (`true` beside `no`, a date beside an instant). A column with no value at all is `text`. An inferred column is always optional and its type is what its values are, so it is never reported but for a control character (below). An ambiguous value infers the plainer kind: `0042` is an integer, but `0042` beside `A12` is text. A cell holding a formula takes no part in what its column infers.
- **A blank cell** is missing: a problem unless the type is `?` or the column has a formula, its value null either way. A quoted empty field (`""`) is blank too.
- **A cell that breaks its type loads and is reported, never coerced.** The cell's text is kept as written, its value null, and a problem names the row, the column and why: `row 3, cost: "8x.50" is not a number`. A bound broken is the type broken. The file always opens, so the tool that can fix it can show it.
- **A control character in a cell**, quoted or not, breaks it whatever its column's type, and before the type is read: the cell is reported as any broken cell is, its text kept as written, its value null, and the problem shows the text before the character and names the character by its code point: `row 2, invoice: holds U+0000, a control character, after "INV-0042"`. A `text?` column is no exception, `?` saying only that a cell may be blank. An inferred column reports it too, its one problem, since nothing invisible gets through unseen; the cell takes no part in what the column infers, so `1`, `2`, and `3` with a NUL after it are an `int` column, its third cell broken.
- **Keys.** Two rows hold the same key when their values are equal: `7` and `007` in an `int` key, `1.5` and `1.50` in a currency, two spellings of one instant. A duplicate key is reported on every row that holds it. A blank or broken key is missing or broken, and in no comparison: `INV-0042`, and `INV-0042` with a NUL after it, are no duplicate, the second key being broken.
- **References.** A reference's key that no table it names holds is a problem of kind `reference` (`row 2, to: "bar" is no key of accounts`), and so is one two of them hold, never a silent pick; its text and value are kept. A reference into a table that could not be reached is unchecked.
- A cell has at most one problem: missing, broken (a control character in it its one problem, whatever else its text breaks), a duplicate key, a formula that does not compute, or a reference to no row.

## Writing

A writer round-trips: a file read and written with nothing changed is the same bytes. A changed cell is written in its column's form (its format, its pair's words, text that would read as a formula escaped, quoted only when CSV needs it); everything else is written as it was read, the header's spelling (shorthand or explicit), each line's ending, the byte order mark and the prelude included. A computed cell's field stays as written, blank or its formula: its value is never written into it. A cell set past the end of a short row writes the blank fields before it. A writer changing a type writes the shorthand, then its formula when it has one (`total:EUR>=0=cost * qty * 2`), and rewrites no cell: a cell the new type breaks is reported, not changed, and a change after which a formula no longer reads (one a column it reads no longer holds the kind it needs) is refused, naming the column changed. A writer never writes a control character into a cell it changes or adds: text holding one is refused, naming the row, and the file is left as it was. A cell read with one is written back as read, as every unchanged byte is. A file written as a plain one writes each computed cell's value, so the numbers a person saw are in the file.

The shorthand, as a writer writes it:

- the type: a base, a currency, a named type by its name, a choice's words joined by `|`, a pair's two words joined by `/` (`true/false` is `bool`), a reference's tables and keys joined by `|`;
- then its format, then its lower bound (`>=` or `>`), then its upper bound (`<=` or `<`), then `!` or `?`. A `num` with both bounds inclusive is its range alone: `0..10`, never `num>=0<=10`;
- a bound's number in plain decimal: `-` if negative, no exponent, no zero that changes nothing (`1e3` is `1000`, `0.50` is `0.5`, `-0` is `0`);
- a word bare unless it must be quoted: it holds a space, a tab or a reserved character, or it is a choice's only word.

So `cost:EUR min 0 key` is written `cost:EUR>=0!`, and `mood:one of 'so so' meh optional` is written `mood:'so so'|meh?`.

## Running

**Opening never runs anything.** Reading, querying, charting and sending a file run no script: a file opens with the rows its last run stored, as a spreadsheet opens with its last values. A script runs when it is asked to (`tcsv run <file> [name]`), or on its schedule where something keeps one ([Open](#open)).

- **The contract is the process.** The script's module exports a function by default; the runner calls it with the context, `{ rows, last, now }`: the table's rows as the file writes them, by column name (the engine's `run` aside), the last of them or null, and the time of the run, an instant. It returns a list of rows, objects by column name: text as it is, a number, a boolean (written in the column's words), or null (blank). Under the function, the context is JSON on stdin, each row returned a JSON line on stdout, and what the script prints on stderr, kept as the run's words.
- **What a run writes.** A table with a key gains the keys it did not have and has the rows whose keys it returned replaced; a table without one is replaced whole. Each row names, in the engine's column `run:int?` (added to the header the first time), the run that last wrote it; a row written by hand names none. The runs block gains the run, `ok`, and keeps the runs some row names, the last one and the last failure.
- **A run that fails writes nothing but its failure.** A script that exits with an error, returns a column its table does not have, or returns a row its types break leaves the stored rows as they were; the runs block gains the run, `failed`, and why. A program's broken output never lands, where a person's broken cell opens to be fixed.
- **The time a run happened is the runs table's**; the time the data is about is the data's own column.

In v0 a script is a program with the whole access of its runner, its imports resolved as Bun resolves them, and the person who runs it carries its risk, as with any program they run. Considered: hosts and secrets declared on the `run` line, enforced as Deno enforces its flags. With no sandbox nothing would enforce them, and a declaration nothing enforces is ceremony; Deno's runtime itself ends in 2027. Trusted folders: trusting a file says nothing of the packages it imports. Adapters compiled to wasm, given only the hosts they declare: the right shape for a sandbox once one is needed, premature before. A fetch described in the file (a URL template, a path into the JSON): it grows into a language of auth, paging and retries, puts keys beside files that are shared, and makes every file opened a beacon to whoever wrote the URL. Any language a fence names: a notebook's code runs only where its packages are, and of 1.4 million Jupyter notebooks on GitHub 24% ran without an error and 4% gave the results they stored; so the fence names its language, and a second one comes with its runner (`python`, through `uv run` and the dependencies its PEP 723 block declares), never by assumption. Python first: its `Decimal` suits money, but `Infer<"header">` in `ts/` already checks a script's rows against the header in the editor, and only JavaScript runs in a page and an app, if scripts ever run there. A Zod or Pydantic model of the rows: a second copy of the header that drifts from it; a language's types for the rows are generated from the header, never written.

## Open

### What it is for

The use it is designed against is **plain text accounting done properly**: books kept as plain files (a `money.tcsv` per folder), readable, diffable and the person's own, with what ledgers in plain text never could do: say that an amount is euros, that an account must exist, that VAT is derived from the amount, that an invoice appears once, and answer any question across every file. Every decision below is judged by whether it makes those books better.

### Decided in direction, not yet written

- **The filesystem as a graph database.** A folder of `.tcsv` files is a graph of typed tables, its edges the references: written today, a reference that breaks is a problem naming the file and row. What is not yet: tools that notice before a break lands (the language server warns on a key renamed in the other file, the writer refuses to save a dangling reference without saying so), and a rename followed through every file that references the key.
  ```
  ~/books/
  ├── accounts.tcsv              account:text!,kind:assets|liabilities|expenses|income
  ├── vendors.tcsv               vendor:text!,name:text,vat_id:text?
  ├── home/money.tcsv            account_from/account_to → accounts, vendor → vendors
  ├── travel/money.tcsv
  └── studio/money.tcsv
  ```
- **Fixes a reference offers**: `did you mean "rent"?`, `add it to accounts.tcsv`, so a new account never means opening the other file by hand; and for a plain text column, a value seen once beside a near spelling seen often a hint, never a problem.
- **Schedules kept.** A file's schedules are read today, never kept: the daemon (ENGINE.md) runs each script on its schedule. A run missed while the machine slept runs once on waking, never once per miss; retries are the engine's, the same for every file; every run is in the engine's log.
- **Every row has an identity.** A file's key (`!`) is its rows' identity. A file with none gets an `id` column the first time the engine writes it, eight characters from a base-32 alphabet, unique within the file, assigned only by the engine and never edited through it. Edits address a row by its identity, never by its position, so a line inserted by hand elsewhere never makes an edit land on the wrong row, and two versions of a file merge row by row. A row without one (added by a tool that bypasses the engine) is found by its text until the engine next writes the file, which gives it one; a copied row's repeated id is replaced on the later copy. Apps built on the engine never show the column.
- **One fact, one row, one file.** A movement is one row holding both its sides (`account_from`, `account_to`), so recording it is one write. A change that must touch several files (an account renamed in every file that references it) is an intent the engine journals before applying file by file, and replays after a crash; every intent is safe to apply twice.
- **What books need**, the accounting primitives (ACCOUNTING.md drafts the books with them):
  - **Money at its scale.** A written amount is exact at its currency's minor unit (EUR 2 digits, JPY 0, KWD 3), more digits a problem naming the cell, as a formula's result is already rounded to it; a rate or a unit price is a `num`. This replaces today's rule that the minor unit does not limit a written amount.
  - **An amount whose currency is another cell**, `amount:money in commodity`, or reached through a reference, `money in account.commodity`; and commodities beyond ISO 4217 (a share, a fund, points) declared in a file.
  - **`unique`** on a column that is not the key, among its non-blank cells, and over several columns together.
  - **`final`**: a column written once, never changed by an edit. A movement whose money columns are final is corrected by a new row, never edited, as TigerBeetle never edits a transfer.
  - **References to a file's own rows** (`pending:money.id`), for a payment that settles a hold, a row that corrects another, rows that are one transaction.
  - **`check`**: a rule over many rows or files, a query in the prelude returning the rows that break it, checked before every write and by `tcsv check` (a balance that must hold on a date, an account open on a movement's date). The prelude is where it lives, beside the data it guards; it is SQL, the language formulas are a part of, whole, in DuckDB.
- **Go's checksum log** for pinned imports, proving everyone got the same bytes and not only you, once files are published widely.
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
- **The command line beyond `check`, `fmt --check`, `pin`, `run` and `show`**: `tcsv sql` (above), and what else a folder-as-database needs (a visualisation of the graph, a rename followed through every file).
- **Words about a column and a title for the table.** A header cell has no room for a description without bloating it; the prelude is the natural place, its syntax for them open.
- **Strictness.** "A broken cell loads and is reported" is the proposal; whether a reader may also be asked to refuse a file is not decided.
- **Narrowing a reference** to some of the rows it names (`from` an account of kind `assets` or `income`): in the header cell (`from:accounts.account(kind=assets|income)`), or as a `check`, which keeps the header readable where the rule spans two files.
- **Provenance: what was known on a date, and who may write what.** Two problems. A run replaces a corrected row, so the rate a report used before the correction lives only in git. And anyone can edit a block the engine owns, or a file a process owns: locks on a Mac are advisory, so the engine can only detect an edit, never prevent one. The candidates, none decided:
  - **Owners.** A block or a file names who writes it; an edit by anyone else is refused through the engine and found by its hash otherwise. Solves who may write what; does not keep what was known.
  - **Append-only with a hash chain.** `run …, append`: rows never replaced, each with two engine columns, `recorded` and `chain` (BLAKE3 of the previous row's `chain` and the row's bytes), so "as known on" is the latest row of each key recorded by then, and an edit or a removal breaks every later link. Solves what was known and makes tampering evident; does not stop a whole chain written again from scratch, which stays valid unless each link is signed by a device's key, as ENGINE.md's Later plans for the log.
  - **Transitions as a type.** `phase:pending->post|void`: the values a cell may move to from the one it holds, checked on every write, replacing ACCOUNTING.md's `unique` on `pending` if adopted. Solves an illegal change of state; does not keep history or say who wrote it.
- **A folder sent as one file.** `tcsv pack` writing one file whose prelude carries a frozen copy of each file it imports, every reference resolved, for someone who should open one thing.
- **Several values in one cell** (`tags:text[]`, split on `;`): useful (tags on a movement), and where a table starts to stretch; undecided.

### Brainstorm, not planned

- **A replicated mode.** Engines on several devices agreeing on the order of the intent log (Viewstamped Replication, as TigerBeetle does), for books where two writers must never both succeed; the files then the cluster's readable view. The engine's write path (deterministic intents, logged) keeps it a protocol on top, never a rewrite (ENGINE.md, Later).
