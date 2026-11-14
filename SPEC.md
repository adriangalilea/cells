# Cells

> **Not yet final.** This spec is a draft: what follows is what the conformance suite ([`tests/`](tests)) holds every reader to, and [Open](#open) holds what is decided in direction but not written, and what is undecided. Any of it may still change.

Data and its logic, in one plain-text file. A Cells file, `.cells`, is a table whose header cells carry their column's type, `name: type`, and its formula when it computes, `name: type = formula`; its rows hold every value, the computed ones too, each beside how it is computed; it may open with a prelude, fenced by `---`, that names types, imports other files and holds tables of its own, written inline or filled by a script. One file, never a sidecar: the types and the logic travel with the data, and the data is all there to read in any editor. MIME type `text/vnd.cells`.

```cells
---
type Category = travel|stay|food|gear
---
item: text!,category: Category,cost: EUR>=0,qty: int>=1,total: EUR = cost * qty,paid: yes/no,on: date
fuel,travel,84.20,1,84.20=,yes,2026-08-14
groceries,food,32.25=96.75 / 3,2,64.50=,no,2026-08-15
```

`84.20=` is computed by its column's formula, `32.25=96.75 / 3` by its own: left of `=` what the file stores, right of it how it is computed ([A cell](#a-cell)).

**Cells is not meant to be compatible with CSV.** Adopting it is as simple as renaming a `.csv` to `.cells`; nothing promises the way back, and no CSV tool is a reader it serves. Cells is its own format, read by Cells readers. Its records borrow RFC 4180's quoting because it is the right rule for cells, never to stay readable by CSV tools; no rule here is bent for a reader that does not know Cells. Considered: staying readable by them, as Oils' TSV8 does with an empty first column on every row and a row of types an old reader takes for data. That reader still needs the file cleaned by hand, so every file would pay, on every row, for a compatibility nobody gets.

## The file

- **Text.** A file is UTF-8; one that is not (a Latin-1 export, a byte that begins no character) is an error naming the file. A byte order mark before its first line is no part of that line, and a writer writes it back.
- **Control characters** are Unicode's Cc, U+0000 to U+001F, U+007F and U+0080 to U+009F, but for tab, `\n` and `\r`. They show nothing, so `INV-0042` and `INV-0042` with a NUL after it would be two keys that look one: no part of a file holds one unseen. One in a cell breaks the cell ([Reading](#reading)); one in a header cell, its name or its type, is an error naming the column; one in a prelude line, a comment or a fence's content included, is an error naming the line. A message names the character by its code point, `U+0000`, never by the character.
- **Records** are RFC 4180: fields separated by `,`, a field holding `,`, `"` or a line break is quoted, `""` is a quote inside one. A quoted field opens with `"` and closes at a lone `"`, which the end of the file, a `,` or a line ending follows; text after the closing quote, a `"` inside a field that did not open with one, and a quoted field never closed are errors naming the record (the header, or the row). Inside quotes every character is the field's, line breaks included; a control character quoted is still one.
- **Padding.** Spaces and tabs at either end of an unquoted field are no part of it, in the header and in every row, so a table can be laid out as one ([Formatting](#formatting)) and reads the same aligned or not. A value whose own ends are spaces is quoted (`" item"`): inside quotes every character is the field's.
- **Lines** end in `\n` or `\r\n`. Each line's ending is kept as read, so a file may mix them, and so is whether the file ends in a line break: one at the very end closes the last record and opens none. A `\r` that is not followed by `\n`, outside quotes, is an error naming its record.
- **The prelude** opens when the file's first line is exactly `---` and closes at the next line that is exactly `---` outside a fence ([The prelude](#the-prelude)); between them, the format's own language. It is optional; a file with none starts at its header. A prelude that opens and never closes is an error. A `---` on any other line is no fence: after the prelude it is the header (one column named `---`), below the header it is a row.
- **The header** is the first line after the prelude. Every other line is a row. A file with no line after its prelude (an empty file, or a prelude alone) has no columns and no rows.
- **A row** has as many fields as the header, or fewer, the missing ones blank. More is a problem of kind `ragged` on its last cell, which takes the place of any other problem there: its fields are read, and those past the header kept as written. A blank line among the rows is a row whose every field is blank.

## A header cell

```text
cost: EUR>=0        total: EUR = cost * qty
 ①  ②   ③            ①   ②  ③  ④      ⑤
```

1. **The name** is everything before the first `:`, the field's padding aside (` item` and `item` are one name; quoted, `" item: text"` is the name ` item`). A name may hold any character but `:`, and is at least one character. No two columns share a name.
2. **`:`** says a type follows. A cell with no `:` is a plain column, its type inferred from its values (below). So every CSV is a Cells file with no types written.
3. **The type** is everything after it up to its formula, and never empty. It never holds a `,` or a `"`; a name that holds one quotes the whole cell (`"price, net: EUR"`).
4. **`=`** says a formula follows: the first `=` outside single quotes that closes no `>=` or `<=`.
5. **The formula** is everything after it, spaces and tabs at its ends cut, and never empty ([Formulas](#formulas)). A formula that holds a `,` or a `"` (`ROUND(cost, 2)`) quotes the whole cell, as any field that holds one is quoted.

A writer writes a typed header cell `name: type`, and a computed one `name: type = formula`: a space after the `:`, a space on each side of the `=` that opens the formula, the form a column's declaration has ([Columns](#columns)), so a column is written one way wherever it is said. Inside the type the shorthand stays compact (`EUR>=0!`, `travel|stay|food`), and the formula keeps its own spacing. To a reader the spaces are nothing: `cost:EUR>=0` is `cost: EUR>=0`. Considered: the compact `name:type=formula`, which glues the type to the formula so `num?=price` reads as one word.

## A cell

A cell of a Cells file is one of four forms, in every column and every table:

| Written | Is |
|---|---|
| `84.20` | written: a value a person or a program put there, read by its column's type |
| `84.20=` | computed by its column's formula: the value it gave, stored |
| `32.25=96.75 / 3` | computed by its own formula, right of the `=`: the value it gave, left of it, stored |
| `=96.75 / 3` | its own formula, not computed yet: the next write stores its value before the `=` |

The first `=` in a cell is the split, so a formula's own `=` (`0=CASE WHEN kind = 'needs' THEN cost ELSE 0 END`) comes after it. A text that holds `=` writes it `'=`, which reading takes back to `=`, and an apostrophe anywhere else is itself: `'=> see invoice` is the text `=> see invoice`, `it's` is `it's`; `''=` is the text `'=`. A plain file's cells (a `.csv`, a `.tsv`) are text, `=` included.

What is left of the `=` is stored so that the file holds its data whole: a value is there to read in any editor, to diff and to send, never only in a tool that computes it. What is right of it is the truth: a reader computes every computed cell, and the value it shows is what the formula gives ([Formulas](#formulas)). Considered: a computed cell left blank, its value only in the tools, so the file held the logic and not the data; a formula marked by backticks (`` `=96.75 / 3` ``), two characters on every formula cell to keep a bare `=` text, where text holding `=` is the rarer cell.

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

A cell is read as written, its padding aside: never folded in case, never coerced. `TRUE` is not a `bool`; quoted, `" 12"` is not an `int`.

- **`datetime`** is `YYYY-MM-DDTHH:mm:ss`, then a fraction of a second of one to nine digits or none, then `Z` or an offset `+HH:mm` or `-HH:mm` (hours `00` to `23`). `T` and `Z` are capitals, the seconds are always written, the offset never left out: a time with no offset is no instant. Its value is the instant, so `2026-08-14T14:30:00+02:00` and `2026-08-14T12:30:00Z` are one value.
- **`url`** is a scheme (a letter, then letters, digits, `+`, `-` or `.`), `:`, then at least one character, each a letter, a digit, one of `-._~:/?#[]@!$&'()*+,;=`, or `%` and two hexadecimal digits. No space, nothing outside ASCII: a URL with other characters writes them percent-encoded. `mailto:ada@example.com` is a URL; `example.com` and `//example.com/a` are not.
- **`email`** is a local part, `@`, then a domain. The local part is at least one ASCII character from `!` to `~` but `@`. The domain is two or more labels joined by `.`, each label ASCII letters, digits and `-`, never starting or ending with `-`. `ada@localhost` is not an address.
- **`id`** is exactly eight characters, each of `0123456789abcdefghjkmnpqrstvwxyz`: lower case only, never `i`, `l`, `o` or `u`. Its value is the text; two ids are equal when their text is.
- **A currency** is any alphabetic code of ISO 4217's current list, in capitals: `EUR`, never `eur`; a withdrawn code (`HRK`) is none. Fund codes (`CLF`, `BOV`), the metals (`XAU`, `XAG`, `XPD`, `XPT`) and `XDR` count, since people hold gold and accounts hold units; `XTS` (for testing) and `XXX` (no currency) do not.
- **An amount's scale** is the digits after the point a column holds: its currency's minor unit, ISO 4217's (`EUR` 2, `JPY` 0, `KWD` 3), unless the type gives its own in parentheses, `EUR(6)`, a whole number from 0 to 18. A written amount is a `num` with at most that many digits after the point: `96.755` in an `EUR` column breaks its type, `0.001200` in an `EUR(6)` one is an amount, exact. A formula's result is rounded to its column's scale ([What a formula gives](#what-a-formula-gives)), once, never along the way: `SUM(fee)` over ten thousand `EUR(6)` fees of `0.001200` is exactly `12.00` in an `EUR` column. A unit with no minor unit (the metals, `XDR`) and no scale given keeps every digit. Considered: a rate or a unit price kept as a `num` beside the money, which loses that it is money; and the minor unit as a ceiling, which refuses what a meter or an API bills in fractions of a cent.

A word that is none of these, no named type, no reference and not a choice is an error naming it: `cost: EURO` is `no type or currency "EURO"`, never silently text.

### Choices

`category: travel|stay|food|gear` is one of the words, written bare. A choice holding a reserved character (below) or a space or a tab, inside it or at its ends, is quoted with single quotes, a quote inside one doubled:

```cells
mood: 'yay!'|meh|'so-so?'|'it''s fine'
```

A word beside `|` is always a choice's word, even one that spells a base or an explicit word (`text|image`, `key|lock`). A choice of one word is quoted (`'paperback'`), since a bare word alone is a type. A cell is one of the words exactly, case and spaces included. A choice's words differ from each other, and none is empty: `''` is refused, a blank cell being missing, never a word.

**A word's tint** follows it after a `#`: `kind: income#green|cost#red|transfer#blue|pending`. A choice is the set of words people colour (a tag's colour, a status's), so the colour is written beside its word, once, in the type, and a header cell, a named type and an imported one carry it alike. A tint is one of ten names, `red` `orange` `yellow` `green` `teal` `blue` `purple` `pink` `brown` `gray`, which each reader draws in its own theme's colour, light and dark, at a contrast that reads; or six hexadecimal digits (`income#3fa34d`), a colour no name says, the same everywhere. No other word is a tint: `income#grene` breaks the column's type, naming the word. A cell holds the word alone (`income`), never its tint: a tint is how the word looks ([How a value looks](#how-a-value-looks)), never part of a value, a key or a reference. Considered: the tint in a `look` statement only, which keeps the type a contract and nothing else, but writes each word twice in two places that drift and leaves a file that is a header alone no way to tint; and CSS's hundred-odd colour names, which tie the format to a web list and draw badly in a dark theme.

### A boolean in the file's own words

`paid: yes/no` is a boolean written `yes` for true and `no` for false, true first. `bool` alone reads `true`/`false`, and `true/false` is `bool`. The two words differ, are quoted as a choice's are, and are read exactly, case included.

### References

`to: accounts.account` is a key of another table: `accounts`, a table the prelude names ([The prelude](#the-prelude)), and `account`, its key column (`!`). Several tables join with `|`, as a choice's words do: `to: shared.account|clients.account`. A cell holds the key's text, read as the key column reads it (`7` and `007` are one key of an `int`). A word `name.column` is a reference only when the prelude names a table `name`; a table named twice in one type is refused, and so is a column that is no key of its table, naming the column. A reference column is a name made strict: what it holds must exist ([Reading](#reading)), and a plain `text` column is the same name left open, any value holding. Considered: one rule for everyone. beancount declares every account before it is used (no typo gets in, every new account a detour), hledger takes any name (a typo becomes an account, the totals still add up, and it grew `--strict`). Both are right for someone, so the type says which.

### Constraints

| Written | On a number | On text |
|---|---|---|
| `>=0` `>0` `<=100` `<100` | the value | its length |
| `0..100` | at least 0 and at most 100 | a length between |

`cost: EUR>=0`, `pct: 0..100`, `name: text<=40`, `qty: int>=1<=12`. A type that is only bounds is a number: `score: 0..10` is `num` from 0 to 10, `>=0` is `num` at least 0. A range after a base bounds it: `int 1..5`, `text 1..40`.

- Bounds apply to `int`, `num`, `pct`, a currency, and `text`; every other type refuses them. A `pct` is bounded by its percentage: `pct<=100` refuses `150%`.
- A bound is a `num` (`-1.5`, `3e8`). On text it is a whole number, at least 0.
- A text's length is its count of Unicode code points: `café` written with `é` is 4, written with `e` and a combining accent is 5.
- Bounds no value can meet are refused: a lower bound above the upper, or the two equal with either exclusive. `5..5` is 5 alone.

### Formats

A format in parentheses says how the file writes the value: `on: date(DD/MM/YYYY)`, `at: time(HH:mm)`. Its letters: `YYYY` `MM` `DD` `HH` `mm` `ss`, everything else literal (`time(HHhmm)` reads `14h30`). Each letter stands for exactly as many digits as it has letters: `MM` is `08`, never `8`. The value read is the same as the ISO form's, and the cell must be a real day or time as the ISO form must; writing keeps the format.

- A format applies to `date` and `time` only, and every other type refuses one: a `datetime` is an instant, and an instant is written as ISO 8601 writes it, its offset included. A currency's parentheses are its scale, a number ([Bases](#bases)), never a format.
- A date's format holds `YYYY`, `MM` and `DD` once each, and no time letter. A time's holds `HH` and `mm` once each, `ss` once or not at all (its value then has `00` seconds), and no date letter. No letter reads a fraction of a second: `ss` is exactly two digits, so `time(HH:mm:ss)` refuses `14:30:05.5`. `date(D/M/YYYY)` is refused: `D` and `M` are literal, so the day and month are never read.
- A format is never empty, and its `(` is closed.

### Marks

| Mark | Means |
|---|---|
| `!` | the key: present in every row and unique |
| `?` | may be blank |

Without `?`, a blank cell is reported as missing. `note: text?` is an optional note; `item: text!` is the key. The two together are refused: a key is present in every row.

### Every shorthand has a word

Both are always valid and mean the same; a header cell may mix them.

| Shorthand | Explicit |
|---|---|
| `text!` | `text key` |
| `text?` | `text optional` |
| `travel\|stay\|food` | `one of travel stay food` |
| `income#green\|cost` | `one of income tint green cost` |
| `yes/no` | `bool yes no` |
| `EUR>=0` | `EUR min 0` |
| `int>0` | `int above 0` |
| `num<=10` | `num max 10` |
| `num<10` | `num below 10` |
| `0..100` | `num between 0 and 100` |

Explicit words are separated by spaces or tabs, never commas (a comma would force the header cell into CSV quotes). A choice with a space inside is quoted: `one of travel 'sd cards'`. `between` takes two numbers joined by `and`, both bounds inclusive. `one of` takes every word after it up to the next explicit word (`key` `optional` `one` `of` `bool` `min` `max` `above` `below` `between` `and` `tint`) or mark, and at least one, each followed by `tint` and its tint when it has one; a choice that spells an explicit word is quoted there: `one of 'key' lock`. `bool` followed by two words that are not explicit words is a pair, and a pair's word that spells one is quoted (`bool 'key' lock`); `bool` alone is `true`/`false`.

### Reserved

Inside a type, outside single quotes: `|` `/` `!` `?` `<` `>` `=` `..` `(` `)` `'` `:` `#`, so a choice's word holding `#` is quoted (`'C#'|rust`). Inside single quotes nothing is, but `'` itself, doubled; `,` and `"` are never in a type. A name reserves only `:`. A value reserves `=`, which a text writes `'=` ([A cell](#a-cell)).

## The prelude

````cells
---
import accounts from "../accounts.cells"       // the shared accounts
type Kind = card|transfer

fees = ```cells
kind: Kind!,pct: pct
card,1.4
transfer,0
```
---
````

The prelude is the work; the header and rows after it are what the file shows, and an app shows the prelude folded. One statement a line, a fence's content apart; a blank line is nothing, and spaces and tabs at a line's ends and between a statement's parts are nothing (a tab counts as a space everywhere in the prelude, outside quotes). A line that is no statement is an error naming it. Statements are read in order: each sees the types and tables named above it, never one below. A name is a type's, a table's or a chart's, once in the file: one named twice, whatever the two are, is an error naming the second. An error inside a statement (a type that does not read, an import that does not follow, a table's cell grammar) names the statement's line, or the line inside the table's fence it is on.

- **`//`** starts a comment to the line's end, outside single and double quotes and outside fences (`type Path = 'a//b'|c`, `"https://example.com/r.cells"`).
- **A fence** opens with three backticks or more and its language, `` ```cells ``, then optionally one word of lower-case letters, its label (`` ```cells runs ``, `` ```chart bars ``), and closes at the first line that is as many backticks and nothing else, spaces and tabs aside. What it holds is its language's, so a `//` or a `---` inside one is no comment and no end of the prelude's. A fence never closed is an error naming the line it opens on.
- **A column** is declared with a `:`, `total: Money = cost * qty` ([Columns](#columns)); `total = cost * qty`, with no type, is no statement and an error naming its line.
- **A chart** is a fence of its own, `` ```chart ``, or named, `actual = ```chart` ([Charts](#charts)).
- **A statement by its first word**: `type`, `import`, `export`, `look` ([How a value looks](#how-a-value-looks)) and `packed` ([Packed imports](#packed-imports)), each followed by a space, open the statement they name, so a declared column whose name opens with one and a space is quoted (`"look ahead": text`).

### Named types

**`type Name = <type>`** names a type for any header cell to use: `category: Category`. A name is ASCII letters, digits and `_`, starting with a capital letter (`type category = …` is refused), and never a currency code. A name is defined once; it may name another named type defined above it, never one below it or itself. A header cell may add to a named type what it does not already say: `cost: Cost?`, `cost: Money>=0`. **`export type Name = <type>`** names one another file may import.

### Columns

````cells
---
type Money = EUR>=0
item:    text!
cost:    Money
qty:     int>=1
total:   Money = cost * qty
running: Money = running[-1] + total
---
item,           cost,   qty,    total,  running
fuel,           84.20,  1,      84.20=, 84.20=
split dinner,   32.25,  1,      32.25=, 116.45=
````

**`name: type`**, or **`name: type = formula`**, declares a column of the file's table in the prelude, in a header cell's own grammar, spaces around its `:` and `=` being nothing as anywhere in the prelude. The header then names it alone (`total`), and the column is the declaration's: its type and its formula. A declaration names a column the header has, and a column is declared once: a header cell with a `:` whose column is declared too, a column declared twice, and a declaration whose name no header cell has, are each an error naming the line. A header name with no `:` and no declaration is a plain column, as ever. Declarations are statements, read in order, so a named type is above the columns that use it; a name that holds a `:`, a `,` or a `"` is quoted, `"price, net": EUR`. A table written inline declares its columns in its own header. Considered: the type and formula only in the header cell, so a column says where it is read how it is computed. The rows still say which cells are computed (`84.20=`), and a header of names lines up with its rows, which one of `cost: EUR>=0` and formulas never does; both forms read, and a writer keeps the one a file uses. A declared column's type changed is its declaration written again, its type made plain its declaration gone; renamed or removed, its declaration goes with it; what does not read in its type or its formula is an error naming its line. A new column is a header cell.

### Imports

```cells
---
import accounts from "../accounts.cells"
import { Kind, fees } from "./rates.cells"
import vat from "https://example.com/es/vat.cells" with { sha256: "9c1e7f0a…" }
import last_year from "./money.cells" with { rev: "a1b2c3d" }
---
```

- **The default import**, `import accounts from "…"`, names the other file's table: its key a type for a column (`to: accounts.account`), its columns read by a formula through a reference or by key.
- **A named import**, `import { Kind, fees } from "…"`, takes what the other file exports: a type (`export type Kind = …`) or a table (`export fees = …`). A name it does not export is an error naming the line, the names it does export said. What a file does not export is its own, so `accounts.account` only ever means a column.
- **Where from** is a quoted path, read from the importing file's folder (`"../accounts.cells"`), or a URL. Nothing is copied in: a local import is read again at every read, so it is always current.
- **A URL is pinned by its content**: `with { sha256: "…" }`, the hash of its bytes in lower-case hexadecimal, is its identity. A URL with no hash is an error naming the line (`cells pin` writes it). Bytes that do not match the hash are an error naming the line, the hash they have said; a change at the source is never followed. A reader fetches from wherever the bytes can be had (the URL, a cache of them by their hash).
- **`with { rev: "…" }`** reads a local file as its history holds it at that revision (git's, for the command line), so last quarter's report reads last quarter's files. A reader that holds no history reads it as unreachable.
- **A file that imports itself**, directly or through others, is an error naming the line.
- **An import that cannot be reached** (a file not there, a URL not fetched) is no error: the file still reads. A reference into the table it names is unchecked, a computed cell whose formula reads it keeps the value the file stores ([Reading](#reading)), and a type it names (a name with a capital) reads as `text`.

- **`with { pack: "auto" }`** keeps the import's packed copy in the file, refreshed on every write ([Packed imports](#packed-imports)).

Data has no versions to resolve: no ranges, no resolver, no lockfile. Considered: `use path`, a syntax of our own where one exists that every reader and every highlighter knows, whose import attributes (ES2025) are where a pin goes. npm and Cargo pin in a lockfile beside the manifest, the sidecar Cells exists to avoid; Zig keeps a URL and its hash together, as here.

### Packed imports

````cells
---
import accounts from "../accounts.cells"
packed accounts = ```cells
account: text!,kind: assets|expenses,limit: EUR
rent,expenses,1000.00
```
import { Money, fees } from "./rates.cells" with { pack: "auto" }
packed type Money = EUR>=0
packed look Money: grouped
packed fees = ```cells
kind: income#green|cost#red!,pct: pct
cost,1.5
```
---
````

A file that imports others reads whole only where they are. **A packed copy** keeps what an import exposes in the file itself, so the file reads the same sent alone: `packed <name> = ```cells` for a table (the default import's, or a table a named import takes), `packed type <Name> = <type>` for a named type, `packed look <Name>: <rules>` for that type's looks. Only what the import exposes is copied: for a default import its table, for a named import the types and tables it names. A copy is the prelude's, never after the header and rows: the body stays pure rows.

- **What a copy holds.** A table's copy is its header and rows, each column `name: type` written whole (a named type spelled out, a reference written as its key's type, since the table it names is the source's, never the reader's) and no formula, each computed cell its value: so it reads with nothing around it. A type's copy is the type written whole; its looks' copy, every rule the type's looks give it, its named types' first. A copy follows its import, below it; a writer writes it right after its import line.
- **Reading.** While the import's source is reached it is the truth: the copy is checked against it and never read. A copy that is not what packing the source gives now (the source changed, or the copy was edited), compared as `cells diff` compares, without its padding, is `stale`, naming the import, and never silently used. When the source is not reached (the file sent alone, a folder moved, a revision a reader has no history for, an import refused) the copy is read in its place: its types for the header, its table for references and formulas, its looks, so nothing reads as [unreached](#reading). A copy is said as one either way: `cells check` notes a copy read in its source's place and fails on a stale one.
- **A copy is of an import.** A `packed` statement whose name no import above it names is an error naming its line, and so is one written twice, a type's copy of a table (or a table's of a type), and `export packed`.
- **Read only.** A copy is its source's, edited there: an edit of it, through an app, the language server or the engine, is refused with the reason (`a copy of ../accounts.cells: edit that file`), and an app shows a copy as one, labelled, wherever it shows the prelude's tables.
- **Written two ways.** `cells pack <file>` writes every import's copy once, fresh where its source is reached, a copy of a source not reached kept as it is; `cells pack --out <path>` writes the self-contained file there and leaves the file as it was; `cells pack --remove` takes every copy out. **`with { pack: "auto" }`** on an import keeps its copy written: every write a Cells writer makes of the file refreshes it while the source is reached, and a change to the source reaches the copy in every file importing it as it reaches their computed cells ([Writing](#writing)). Without it a copy is only ever written by `cells pack`.
- **Settled by construction**, so packing never loops and never grows past what is imported:
  - *Copies never nest.* A copy is its source's table or type written whole, never the source's own copies or imports: a file grows by what it imports directly, once, so a chain of ten files holds one copy each and a diamond copies its two sides, never the file beneath them twice.
  - *A copy is data.* Its fence holds rows, never a prelude: nothing in it is followed, so no import and no cycle comes back through one.
  - *A refresh that changes nothing writes nothing.* A copy that is what packing gives already, its padding aside, is left byte for byte, so a writer laying it out (`cells fmt`) and one packing it never undo each other, and a write heard back is no change.
  - *A refresh rides its write.* One change writes each file it reaches once, in the order their reads need, its copies and its stored cells in the same write.
  - *A source not reached never writes.* A copy standing in for it is read, never refreshed: a file sent alone and opened is never rewritten for it.
  - *A URL's copy follows its pin.* It is refreshed only from the pinned bytes, fetched once and kept by their hash; bytes that are not them are refused and the copy kept. A write never fetches, and a copy moves only when `cells pin` moves the pin.
  - *Two writers converge.* A copy is a function of its source's bytes, so two writers packing one source write the same bytes, and an edit made before a refresh is laid over the refreshed copy (the engine's intents, [ENGINE.md](ENGINE.md)), never back over it.
  - *A bound screams.* A file past 4 GiB is refused by every reader, so packing that would make one stops, naming the import, never cuts a copy short.
- **What each costs.** Auto keeps every importer self-contained at all times, at the cost of a copy in each importer, the file's size, and a diff in every importer on every change to the source. Explicit packs only when a file is sent, so the books stay small and their history quiet, and a copy left in them goes stale until packed again. A large table copies whole, every row: auto-packing a large table copies it into each file that imports it, which is what `--out` is for.

Considered: the copy as fences under the import line, as a script's rows are under its `run` (no name written twice, never without its import), which gives a type no place and makes a named import of three things one block of mixed parts; a footer after the rows, which makes the body no longer pure rows; a hash of the source alone, which says the copy is out of date but leaves nothing to read; packing every import always, where a copy in every importer of a shared accounts file is the cost a person should choose.

### Tables written inline

**`name = ```cells`**, its rows on the lines after it and a closing fence: a table of the file's own, read as a Cells file's table is (a header and rows, no prelude of its own) whose types may be the prelude's above it and whose references and formulas may read the tables above it. **`export name = ```cells`** is one another file may import. It is read as any table is: its problems are its own (`cells check` reports them under its name), and the file's formulas read it by name: `fees[kind].pct`, `SUM(fees.pct)`. Considered: a store beside the file, or hidden in it, which would make the file no longer all there is. A file of several sheets, as a workbook: a folder is the workbook and a file one table of it, so a table one file alone needs is written inline, and one another file needs moves to its own file, one import changed and no row.

### Tables a script fills

````cells
---
eurusd = run every day at 18:00 in Europe/Madrid
```ts
export default async function ({ last }) {
  const res = await fetch(`https://api.exchange.example/usdeur?since=${last.date}`)
  return (await res.json()).map(d => ({ date: d.day, rate: String(d.close) }))
}
```
```cells
date: date!,rate: num>0,run: int?
2026-11-03,0.9287,9
2026-11-04,0.9312,9
```
```cells runs
run: int!,at: datetime,outcome: ok|failed,error: text?
9,2026-11-04T17:00:03Z,ok,
12,2026-11-05T17:00:02Z,failed,HTTP 503
```
---
````

**`name = run <schedule>`**, then three fences, blank lines between them nothing ([Running](#running) says what runs them):

1. **The script**, a fence naming its language. A fence that names none, or one no runner runs, is an error naming the line it opens on: v0 runs `ts`, TypeScript, with Bun.
2. **The table**, a `` ```cells `` fence: its header the table's, written by its author, at least; its rows what the last run returned. It is read as a table written inline is, and is the table the name names. A script with no table, or a table with no header, is an error naming the line.
3. **The runs**, a `` ```cells runs `` fence, the engine's: each run some row still names, the last one and the last failure, with when it ran, its outcome and its error. A table with none has not run.

Any other fence after a script, or one of these twice, is an error naming the line it opens on. **The schedule** is triggers joined by `,`, any of which starts a run, then `in <zone>` (`in Europe/Madrid`, `in UTC`) for their times of day:

| Trigger | Starts a run |
|---|---|
| `every minute` · `every 15m` · `every hour` · `every 6h` | at that interval |
| `every day at 18:00` | each day |
| `every weekday at 09:00` | Monday to Friday |
| `every monday at 09:00` | each week, the day named (`monday` to `sunday`) |
| `every month on 1 at 08:00` | each month, the day of the month named (1 to 31) |
| `cron 0 9 * * 1-5` | as cron's five fields say |
| `when ../money.cells changes` | each time that file changes |

A trigger with a time of day (and `cron`) needs the zone: a file moves between machines, and 18:00 must be one moment on all of them. A schedule that says none of these is an error naming the line.

### How a value looks

```cells
---
type Kind = income#green|cost#red|transfer#blue|pending
type Money = EUR
look Money: grouped, negative red
look amount: negative in parentheses
look balance: red when balance < budget, green when balance >= budget
look score: scale red..green
look on: as 'ddd D MMM YYYY'
look row: gray when paid
---
item: text!,kind: Kind,amount: Money,budget: Money,balance: Money,score: 0..10,on: date,paid: yes/no
rent,cost,-950.00,1000.00,50.00,7,2026-11-03,yes
```

**`look <name>: <rule>, <rule>`** says how a column's values look, what a spreadsheet's number formats and conditional formats give. It names a column, a named type above it, or `row`, the whole row (a column named `row` is `look "row": …`). **A look is how a value shows wherever it shows**: a table's cell and a chart's mark alike, so `income#green` is the word's chip and its series' colour, `negative red` the cell and the bar. A chart ([Charts](#charts)) reads the same looks: one vocabulary for how values show, never a table's and a chart's.

| Rule | A value shows |
|---|---|
| `grouped` | its thousands grouped, a number |
| `decimals 1` | at that many digits after the point (0 to 18), rounded half to even for display only; a `num` or a `pct`, since a currency shows at its scale and an `int` has none |
| `negative in parentheses` | below zero as `(950.00)`, accounting's form |
| `negative red` · `positive green` | in that tint, by its sign |
| `red when balance < budget` | in that tint when the condition holds |
| `scale red..green` | in a tint between two, from the column's least value to its greatest |
| `as 'D MMM YYYY'` | a date or a time in that form |

- **A condition** is a formula over the row ([Formulas](#formulas)) giving a boolean, the language a row is already read in, so `>= budget` reads another column with nothing new to learn; a condition that gives null does not hold. `look row:` takes conditions alone: it tints the row.
- **A pattern** says a date's or a time's display form, apart from the format the file writes it in ([Formats](#formats)): `YYYY` `YY` the year, `MMMM` `MMM` the month's name and short name, `MM` `M` its number, `DD` `D` the day, `dddd` `ddd` the weekday's name and short name, `HH` `H` the hour of 24, `hh` `h` of 12, `mm` `ss`, `A` before or after noon, a word between brackets as it is (`D [de] MMMM`), every other character as it is. A letter that is no piece is a broken rule, and so is a pattern the column's base cannot fill (`HH` on a date).
- **The reader's locale** gives the marks and the words: the grouping and decimal marks (`1.234,60` where a comma is the decimal mark), the months' and the weekdays' names, the words for before and after noon. A file says how a value looks, never which country reads it; with no rule that formats numbers, a number shows as the file writes it.
- **Which wins**: a column's looks are its type's (a named type's, then those of the types it names), then its own, in the order written, and where two say the same thing (two tints, two decimals) the later holds. A word's tint goes first, so a rule's tint over a choice's word holds over the word's own. The specific goes after the general.
- **Where a look lives**: on a named type, it holds wherever the type is used, and an exported type carries its looks to every file that imports it (`export look` is no statement: a look travels with its type). On a column, it adds to its type's.
- **A rule that does not read** (`look amount: negative grene`, a condition that does not compute, a target that names nothing, `decimals 2` on a date) breaks its prelude line, naming the rule; the file reads, its values whole, every other rule of the line kept and that rule alone left out ([Reading](#reading)).
- **No value changes.** A look changes what a reader draws: `cells show`, `cells sql`, every export and every formula read values, never looks, and a reader that ignores looks reads every value as it would.

Considered: `show` (the command `cells show`, which writes values), `style` (CSS's word, which promises far more), `format` (how the file writes a value already, [Formats](#formats)); a language of conditions of its own, refused since formulas are the language of a row; the first rule that applies, as a spreadsheet's "stop if true", where the later holding lets the general go first and the specific after it; the file naming its separators, which only a file meant for one country wants.

### Charts

````cells
---
type Kind = income#green|cost#red|transfer#blue

```chart bars
title: Spending by month
x:     DATE_TRUNC('month', date)
y:     SUM(amount)
color: kind
stack: stacked
```

```chart
title: Cash
x:     date
y:     LAST_VALUE(balance)
```

actual = ```chart bars
x:     DATE_TRUNC('month', date)
y:     SUM(amount)
where: account = 'rent'
```

plan = ```chart line
x: DATE_TRUNC('month', date)
y: SUM(budget)
where: account = 'rent'
```

```chart
title:  Rent, actual and planned
layers: actual, plan
```
---
date: date,amount: EUR,kind: Kind,account: text,budget: EUR?,balance: EUR = SUM(amount) OVER (ORDER BY date)
````

**A chart is a fence, `` ```chart ``**, the kind after the language (`` ```chart bars ``), each line in it an entry: a key, a `:` and the rest of the line, read by that key's grammar. A value runs to the line's end, so a formula holds what commas it needs; spaces after the `:` are nothing. A key a chart's kind does not take, or one written twice, is an error naming its line. A file's charts are its own, in the order written: the first is chart 1. **A name**, `actual = ```chart bars`, is written only for a chart another one reads (`layers`), once in the file with the types and tables.

**The entries.** A formula is read over the chart's table's rows, as a column's formula is, in the same language ([Formulas](#formulas)). The axis groups the rows by its value; the measure is an aggregate over each group's rows.

| Key | Value | Is |
|---|---|---|
| `x` | a formula per row | what runs along the axis: the rows of one value make one place |
| `y` | an aggregate (`SUM(amount)`, `LAST_VALUE(balance)`, `COUNT(*)`) | each place's value |
| `color` | a formula per row | the series: one per value, in its colours ([What a chart is drawn in](#what-a-chart-is-drawn-in)) |
| `stack` | `stacked` · `grouped` · `percent` | how the series share a place: on each other, side by side, as shares of it |
| `where` | a condition | the rows charted: those it holds for |
| `top` | a whole number | how many places are drawn, the largest; the rest one place, `other` |
| `title` | text, as written | what the chart is called where it is shown; none, what it measures by what runs along its axis, as written: `SUM(amount) by DATE_TRUNC('month', date)` |
| `from` | a table's name, or `TABLE` | the table charted, inline, a script's or imported; the file's own when none, or `TABLE` |
| `layers` | names of charts, `,` between | the charts drawn on one plot, sharing its axes, in the order named |

**The kinds**, each taking the entries it draws:

| Kind | Takes | Draws |
|---|---|---|
| `bars` | `x` `y` `color` `stack` `where` `top` | a bar a place; across when `x` is text or a choice, up when it is a date or a number |
| `line` · `area` · `steps` | `x` `y` `color` `where` | a line through the places, the area under it, or a step at each |
| `points` | `x` `y` `color` `where` | a point a row: `y` a formula per row, never an aggregate |
| `heat` | `x` `y` `value` `where` | a cell a pair of `x` and `y`, `value` (an aggregate) its depth of one tint |
| `figure` | `value` `compare` `where` | one number, `value`, and its change from the place before along `compare` (a formula per row), with that run drawn under it |

Every kind also takes `title` and `from`; a chart of `layers` takes `title` and `layers` alone, each layer drawn from its own table.

**A chart of another table**, `from: rates`, names it as a formula does ([The language](#the-language)): a table the prelude names above or below the chart, inline, a script's or imported, or `TABLE`, the file's own, as no `from` is. Its entries are read over that table's rows as the table's own formulas are: its columns by name, `TABLE` the table charted, and the tables the prelude names above it; the file's own columns are not among them, and a choice's tints and a column's currency are the charted table's. A name that is no table, and an import not reached, are errors naming the line, and the chart is left out.

**No kind written**, the chart is drawn as its entries and the types say, never the data: `value` alone is a `figure`; `x`, `y` and `value` a `heat`; a `y` that aggregates nothing `points`; a `y` of `LAST_VALUE` or `FIRST_VALUE` `steps`; any other `y` over a date `bars`, over text or a choice `bars` across. **No chart in the file**, a reader draws one from the columns by the same rule: the first date (else choice) as `x`, the first currency (else number) column summed as `y`, `color` a choice of six words or fewer; nothing to draw when the file has no date, choice or number. A chart a person keeps is written as a fence; until then it is the reader's view, never the file's.

**The scale is the data's**, the same in every view at every size: from zero, or the least a place draws below it, to the most a place draws (a stacked bar its sum, its parts below zero apart; a chart of layers every layer's), in steps of 1, 2 or 5 times a power of ten, the smallest that takes it in about four steps, its ends whole steps: `SUM(amount)` of 2,400 at most runs 0 to 3,000 by 1,000, a bar of shares 0 to 100% by 50. A view too short to label every step labels fewer of them, every second or only the ends, on the same axis; a view never picks steps of its own. A `figure` has none, and a `heat` drawn alone none: its rows stand where a scale's labels would.

**A `percent` stack draws shares, computed once by the engine**: each value is its share of its place, its size (its value without its sign) over the sum of the place's sizes, in percent to two places, its sign kept; rounding gives the hundredths it drops to the largest remainders, the first series first among equals, so a place's sizes sum to exactly 100 (300 and 120 are 71.43% and 28.57%; three equal values 33.34%, 33.33%, 33.33%). A place whose values are all zero keeps them. The shares are what every view draws, stacked, on the scale they give, its values labelled as percent; the measure's amounts are the stacked chart's and the table's, never carried beside them, so no view divides a value of its own.

#### What a chart is drawn in

Colours are the looks', never a chart's ([How a value looks](#how-a-value-looks)):

- **A series of a word with a tint** is drawn in it: `income#green` is green wherever it is drawn.
- **A series of a word with none** takes the next tint no word of its kind wears, in the order `blue` `orange` `teal` `purple` `pink` `yellow` `brown` `green` `red`, the words taken in a lasting order: a choice's as its type lists them, any other value's as the file first holds it. Gray is not among them: it is `other`'s and the blank's. A series keeps its colour whatever the axis, the filter or the other series; none takes its colour from its size.
- **More words with none than tints free** (the nine, less those its words wear): the tints go round the words drawn, in the same order, and when those are still more, the smallest by their total over the chart fold into one series, `other`, where the measure is a sum or a count (any other measure leaves them out), so no two series share a colour. A word with a tint of its own is always drawn in it.
- **One series** is drawn in its measured column's colour; `other` and a blank value gray. A `heat`'s rows are one tint, the first.
- **Layers** are each drawn as alone, but a layer of one series whose column's colour a layer named before it wears takes the next of the columns' colours none before it wears: a column's colour is its place in its table, so `SUM(amount)` of the file and `SUM(budget)` of `plan`, each its table's second column, would otherwise be one colour on one plot. Past the six colours a layer keeps its column's.
- **A look's condition** (`negative red`, `red when …`) tints the marks it holds for, unless the chart has series, whose words' colours hold.

#### What travels in the file

What the next reader would be wrong without is the file's; where this reader looks is the reader's: a kept chart, a look, a column's type travel; the view open (table, chart, both), the charts shown, a sort, a filter, a selection, a hover, a zoom, a column's width and a pane's place do not. A reader that changes a kept chart's entries writes them, one edit each.

Considered: one line, `chart spending: amount by month, split by kind`, whose commas part both the chart's words and a formula's arguments and whose words meet the names a file gives; YAML or TOML in the fence, which coerce a value's kind (`no` a boolean) or quote every formula; SQL in a chart, which says again what the types say (`DATE_TRUNC` to bucket a date the column already calls a date) in a second language beside the entries; the kind as an entry, `mark: heat`, where a heatmap is a kind of chart and a mark only what it is drawn with; every chart named, where a name earns its place only when something reads it; a chart of another table reading the file's own columns beside the table's, one name in two tables at once, which no formula does (a chart that joins them charts a column of one that reads the other, `rates[currency].rate`); `from` on a chart of layers as every layer's table, where each layer says its own; `top` counting the series too, one number for two folds a person sets apart (ten places of three series, three places of ten), when the series' count is the tints', a fact the file holds; the words past the tints sharing a last one, two series a reader cannot tell apart; a `percent` stack's summary holding the amounts and each view dividing them, where one view's shares of 1 on its own axis and another's amounts disagree; its amounts riding beside its shares for a hover, two numbers a mark where one is drawn.

## Formulas

A formula computes a cell's value from what the file can read. It is written in a column's header cell, `total: EUR = cost * qty`, where it computes every cell of the column but the ones written, or in one cell, after the cell's `=` (`32.25=96.75 / 3`), where it computes that cell alone. Either way the file stores the value it gave, left of the cell's `=` ([A cell](#a-cell)).

```cells
item: text!,cost: EUR>=0,qty: int>=1,total: EUR = cost * qty
fuel,84.20,1,84.20=
split dinner,32.25=96.75 / 3,1,32.25=
invoice 42,80.00,1,79.99
```

### A column's formula

- **A computed cell**, `84.20=`, is the column's formula's on its row: its value is what the formula gives there, and the file stores it. A blank field, or a lone `=`, is a computed cell whose value is not stored yet (a file written by hand): it reads the same, the next write stores it, and until then it is said, a note ([Reading](#reading), `unstored`). A computed cell is never missing, whatever the column's `?`.
- **A written cell**, with no `=`, is the row's override: read by the column's type as any cell is, shown as an override and never a problem for being one, never touched by a write. The invoice says the VAT was 13.37 where the formula gives 13.36, and the invoice is what happened.
- **A formula may read any column**, after its own or before it, and other formulas' columns: they are computed in the order their reads need. It reads its own column only on other rows ([Rows by where they are](#rows-by-where-they-are)); one that reads its own cell, directly or through others, is an error naming its column.
- Considered: the formula in the prelude alone, where the column is never where it is read and is always `num?`.

### A formula in a cell

- **A cell's own formula**, right of its `=` (`32.25=96.75 / 3`, or `=96.75 / 3` before its value is stored), is in any column of a `.cells` file. Its value is what the formula gives on its row, read by the column's type; a write stores it, and never changes the formula. In a column with its own formula, a cell's formula beats it for that cell. A field holding a `,` or a `"` (`"7.43=ROUND(qty * 2.25, 1)"`) is quoted as any field is.
- **A cell's formula that does not compute** (it does not read, it names what is not there, it gives what its column cannot hold, it reads its own cell through others) is a problem of kind `formula` on its cell, its value null, the problem saying why, and what the file stores left of its `=` kept as written.
- **`#n` is input, never stored.** An app or the language server takes `TABLE[#3]`, the row numbered 3 where it shows now (from 1, as a table numbers its rows), and writes that row's key in its place: `TABLE['k7m2q9xa']`. A file holding `#` is an error naming it. A bare `TABLE[3]` stays the key `3`, so an `int` key never collides with a row number. Completing inside `TABLE[` lists the rows, each by its key and a few of its columns.
- **A spreadsheet's references are input, never stored.** An app or the language server takes a formula as a spreadsheet writes it, `=C2*D2` or `=SUM($E$2:E2)`, and writes it in names: a column by the header's name for its letter, this row bare, another row by where it is from this one (`[-1]`), the first data row (a spreadsheet's 2) `FIRST`, a fixed row by its key (`TABLE['k7m2q9xa'].cost`), a whole column (`E:E`) by its name. `=C2*D2` is written `=cost * qty`, `=SUM($E$2:E2)` `=SUM(total[FIRST:THIS])`; filled down a column, the formula moves to its header cell. A file never holds a letter's column or a numbered row, so neither ever points at the wrong one after a line or a column is inserted by hand: a spreadsheet survives that only because it rewrites every reference when it happens, and in a file edited by hand or merged by git nothing does. A reader that finds `C2` reads it as a name.

### Rows by where they are

A formula reads its own row by its columns' names; another row by where it is from this one, in the order of the file's lines, as a spreadsheet's formula filled down reads it:

| Written | Is |
|---|---|
| `running[-1]` · `cost[+1]` | the column on the row that many lines above (`-`) or below (`+`) |
| `SUM(total[FIRST:THIS])` | the column on the rows from the first to this one, an aggregate's argument |
| `MAX(cost[-2:+2])` · `COUNT(item[THIS:LAST])` | any two ends: `FIRST`, `THIS`, `LAST`, or a row by where it is |

- **Past the rows** there is no cell: a number reads 0 there, as an empty cell does in a spreadsheet's arithmetic, so `running: EUR = running[-1] + total` starts at the first row's total; any other kind reads null. A range clipped by the rows keeps the rows it has.
- **Its own column**, on rows above or below its own: `running[-1]` is the running total on the line above. A range holding its own row (`running[FIRST:THIS]`) reads its own cell, and is an error naming the column, or a cell's problem of kind `formula`.
- **The lines are the order.** Sorting the file changes what is above a row, as sorting a spreadsheet changes what a filled formula sums; an order that must survive a sort is a window's ([The language](#the-language)).
- A range stands only as an aggregate's argument, `SUM`, `AVG`, `MIN`, `MAX`, `COUNT`; a bare `total[FIRST:THIS]` is an error.

### The language

SQL's scalar expressions, in exact decimal. **The language is written in capitals, and only in capitals**: its functions (`SUM`, `ROUND`), its keywords (`CASE`, `AND`, `OVER`) and its words (`TRUE`, `NULL`, `TABLE`, `FIRST`, `THIS`, `LAST`). A word in lower case is always a name the file gave: `sum(cost)` calls no function, and its error says so (`no column "sum"; the function is SUM`). A column named in capitals (`"TOTAL"`) is written quoted, so a formula's capitals are never one of its names.

| Written | Is |
|---|---|
| `2`, `0.5` | a number: digits with a fraction or none, no sign, no exponent (`-` before a term negates it) |
| `'two nights'` | text, a `'` inside doubled |
| `TRUE` `FALSE` `NULL` | a boolean, and nothing |
| `cost` | the column `cost` of the row; `"price net"`, quoted, a column whose name is no word of letters, digits and `_`, or one in capitals |
| `to.kind` | through the reference `to`, the column `kind` of the row it names; each `.` follows one more reference |
| `fees[kind].pct` · `eurusd['2026-11-04'].rate` | a table's row by its key (the key compared as the key column's value, so the text `'2026-11-04'` finds a date), then its column |
| `TABLE['k7m2q9xa'].amount` · `SUM(TABLE.amount)` | the file's own table, whatever the file is named: a row of it by its key, a column of it whole (`SUM(TABLE.amount)` is `SUM(amount)`) |
| `running[-1]` · `SUM(total[FIRST:THIS])` | a row by where it is from this one, and a range of them ([Rows by where they are](#rows-by-where-they-are)) |
| `+ - * /` | arithmetic on numbers |
| `\|\|` | text joined, a number or a date as it is written |
| `= <> != < <= > >=` · `IS NULL` · `IS NOT NULL` | comparisons, of two of one kind (text and a date compare as the date's ISO text) |
| `AND` `OR` `NOT` | on booleans, SQL's three values: `FALSE AND NULL` is `FALSE`, `TRUE OR NULL` `TRUE` |
| `CASE WHEN … THEN … ELSE … END` | the first `THEN` whose `WHEN` is true, else the `ELSE`, else null |
| `ROUND(x)` `ROUND(x, 2)` `ABS` `FLOOR` `CEIL` | numbers: half to even, to that many digits after the point |
| `COALESCE(a, b, …)` · `NULLIF(a, b)` | the first that is not null · null when the two are equal |
| `LOWER` `UPPER` `TRIM` `LENGTH` | text |
| `SUM(cost)` `AVG` `MIN` `MAX` `COUNT` | over every row of the column (`SUM(fees.pct)`: of the table named) or a range of its rows, nulls left out |
| `COUNT(*)` | the rows, nulls and all |
| `FIRST_VALUE(balance)` · `LAST_VALUE(balance)` | the first or last value that is not null, over the rows in the lines' order: an aggregate, as `SUM` |
| `DATE_TRUNC('month', date)` | a date to the first day of its `day`, `week` (from Monday), `month`, `quarter` or `year` |
| `SUM(amount) OVER (ORDER BY date)` · `LAG(balance) OVER (ORDER BY date DESC, id)` | a window, in a column's formula only: an aggregate of the rows up to the row, in that order, or `LAG`, the value on the row before it |

`OR` binds loosest, then `AND`, `NOT`, a comparison, `||`, `+` and `-`, `*` and `/`, then a `-` before a term; each from the left, parentheses grouping. A formula is pure and total: it always finishes, with the same answer in every implementation, and reads no clock, no network and no file, so a formula from a stranger is as safe to open as a number (the formula injection spreadsheets are known for has nothing here to call).

- **Names are looked for nearest first**: in the row (its columns, and the references followed from them with `.`), then in the file (`TABLE`, and every table the prelude names, imported or written inline), then nowhere, since another file is reachable only through an import. A column spelled as a prelude table is an error naming both, never one hiding the other; the column reads as written, and a formula naming it is refused.
- **A window orders the rows by its keys**, never by where they are written: `balance: EUR = SUM(amount) OVER (ORDER BY date)` is a running balance however the file is sorted. Each key ascends, or descends with `DESC`, a null after every value. Rows the order does not tell apart have no value, each a problem of kind `formula` on its cell naming the row it ties with, and for `LAG` so does the row after them, since none of them is the one before it; another key breaks the tie (`ORDER BY date, id`). A window stands only in a column's formula, since a cell's own computes one row, and never inside an aggregate. The lines' order, a spreadsheet's, is `[-1]` and its ranges; a window is for an order a sort must not change.
- **Numbers are exact**: `0.1 + 0.2` is `0.3`. `+ - *` are exact. A division by zero is null.
- **A quotient keeps 15 significant digits**, `/` and `AVG` alike, rounded half to even where it is computed, its whole part always kept: `1 / 3` is `0.333333333333333`, `-7 / 3` `-2.33333333333333`, `1 / 7000` `0.000142857142857143`, `20000000000000000000 / 3` `6666666666666666667`, and a quotient that ends sooner is exact (`10 / 4` is `2.5`). A quotient's digits are not written places: it holds as many after the point as its digits need, and a sum or a product keeps the places its written operands have (a cell's, a literal's) and no zero at its end past them, so `1.10 * 3` is `3.30` and `cost / SUM(cost) * 100` is `20.710859672857`; a cell storing `20.71085967285700` holds the same value, never stale. 15 is the most digits a binary double holds for every decimal, so a computed value is the same in a spreadsheet, in JSON read by JavaScript and in any tool reading floats, and it is what a spreadsheet keeps and shows. Every other operation is exact, so a sum of quotients is the sum of the values the file stores: a running total over shares ends on `SUM` of them, never drifting from the column above it, and computing again from the stored inputs gives the stored digits, so a write never churns them. **In a column with a scale** (an amount, [Bases](#bases)) a quotient keeps 34 significant digits instead, since its result is rounded again to that scale and that rounding is the only one: `300000000000.04499 / 3` is `100000000000.01` in `EUR`, where a quotient cut to 15 digits first (`100000000000.015`) would round to `.02`. How many digits a value shows is its look's ([How a value looks](#how-a-value-looks), `decimals 2`), never its value's. Considered: 34 digits everywhere (decimal128's), which stores `0.1234567890123456789012345678901234` for a share nobody reads past its fourth digit and no spreadsheet can hold; rounding only what is shown, the file storing every digit, where what a person sees is not what the file holds and a value pasted from the screen is another number; digits derived from the operands' scales, as SQL Server (scale at least 6) and PostgreSQL (at least 16 significant digits, at least the operands' scale) do, which ties a share's precision to how its inputs happened to be written; a scale for `num` (`num(4)`), which a person must choose before they know the values and which a look's `decimals` already says for the eye; and a quotient as a binary double, DuckDB's, which is not exact; a product at the sum of its operands' scales whatever they are, which writes a share times 100 as `20.71085967285700`, zeros no one wrote; every result at the fewest places, which writes `1.10 * 3` as `3.3`, dropping a place a person wrote.
- **Null goes through**: arithmetic, a comparison or a function given null gives null (but `COALESCE`, `NULLIF`, `IS NULL`, `AND`, `OR`). A blank or broken cell and a reference to no row are null; a formula reading a table not reached is not computed, its cell's stored value standing ([Reading](#reading)).
- **Kinds are checked before anything runs**: an `int`, `num`, `pct` or currency column is a number, a `bool` or a pair a boolean, a `date` a date, every other column text. Arithmetic on text, a comparison of a number with text, a formula naming no column are errors: a column's formula naming its column, a cell's a problem of kind `formula`.
- **What a formula gives fits its column**: a number a numeric column, a boolean a boolean one, a date or text a `date` one; any other column takes what it gives as text. Anything else is refused as an error is.

Considered: CEL, pure and total but without a decimal, so money in floats; Excel's formulas, built on positions, their separators changing with the locale; a language of our own, that nobody would want to learn; SQL's words in any case, where `sum` beside a column named `total` reads as one more name.

### What a formula gives

What a formula gives is read by its column's type as a written cell's text would be:

- **an amount** at its column's scale ([Bases](#bases)), rounded half to even: `11.159504…` is `11.16` in `EUR`, `1234.5` is `1234` in `JPY`, `2.3451` is `2.345` in `KWD`, `0.0012004` is `0.001200` in `EUR(6)`; a unit with no minor unit and no scale given (the metals, `XDR`) keeps every digit;
- **any other number** at the most digits after the point any of its column's results has (fewer when 38 digits would not hold the largest), an `int` column's as a whole number;
- **a boolean** in the column's words (`yes`, `no`), a date in its format, text as it is;
- **nothing** (null) as nothing left of the `=`: never missing, since what a person fixes is its input.

A result the type breaks (a negative `net` in an `EUR>=0` column, `2.5` in an `int` one) is a problem on its cell, of kind `type`, the problem saying what the formula gave: `row 1, net: the formula gives "-30", which is below 0`.

## Reading

- **A broken grammar takes out only what it breaks, and the file opens with its table.** Where this spec calls something an error naming a place, the file still reads and the error is reported at that place, in words that name what broke on their own (`to: accounts has no column "a" (its key is account)`):
  - a header cell that does not read (its name, its type, a reference to no key, a control character) is a column of `text?` under the name it spells, every other column typed as written;
  - a column formula that does not compute (it does not read, names what is not there, gives what its column cannot hold, reads itself) leaves the column its type, its formula's cells null, and what they store kept as written;
  - a prelude statement that does not settle is left out, and the rest applies; what names it (a type, a header cell) breaks in turn, at its own place. An import refused (its hash, a cycle) names what it imports as one not reached does, unchecked;
  - a record that does not read (its quoting, a lone `\r`) ends the rows: those before it read, and it and every byte after it are kept as written. A prelude never closed holds the whole file, so nothing applies and there is no header.

  Only bytes that are not UTF-8, and a file past 4 GiB, are refused whole. Nothing is less strict for it: every error is reported, and a checker (`cells check`) fails on any. Considered: errors that refuse the file. A person typing a header cell (`to: accounts.a`, the column name not finished) breaks it at every keystroke, and an editor, an app or a page showing the file would show nothing at all while they type, where the one cell is all that is wrong.
- **Inference.** A column with no type is read from its values: every non-blank value an `int` is `int`, a `num` `num` (so integers beside one decimal are `num`), every one `true` or `false` `bool`, every one `yes` or `no` a boolean in those words (`yes/no`, a column of `yes` alone too), a `date` `date`, a `datetime` `datetime`, otherwise `text`. Nothing else is inferred: a time, a percentage, a URL, an address, an amount or an `id` is `text`, and so is a column mixing kinds (`true` beside `no`, a date beside an instant). A column with no value at all is `text`. An inferred column is always optional and its type is what its values are, so it is never reported but for a control character (below). An ambiguous value infers the plainer kind: `0042` is an integer, but `0042` beside `A12` is text. A cell with its own formula takes no part in what its column infers.
- **A blank cell** is missing: a problem unless the type is `?` or the column has a formula, its value null either way. A quoted empty field (`""`) is blank too.
- **A cell that breaks its type loads and is reported, never coerced.** The cell's text is kept as written, its value null, and a problem names the row, the column and why: `row 3, cost: "8x.50" is not a number`, its rows counted from 1 as a table numbers them (and as `#n` does), in every message. A bound broken is the type broken. The file always opens, so the tool that can fix it can show it.
- **A control character in a cell**, quoted or not, breaks it whatever its column's type, and before the type is read: the cell is reported as any broken cell is, its text kept as written, its value null, and the problem shows the text before the character and names the character by its code point: `row 2, invoice: holds U+0000, a control character, after "INV-0042"`. A `text?` column is no exception, `?` saying only that a cell may be blank. An inferred column reports it too, its one problem, since nothing invisible gets through unseen; the cell takes no part in what the column infers, so `1`, `2`, and `3` with a NUL after it are an `int` column, its third cell broken.
- **Keys.** Two rows hold the same key when their values are equal: `7` and `007` in an `int` key, `1.5` and `1.50` in a currency, two spellings of one instant. A duplicate key is reported on every row that holds it. A blank or broken key is missing or broken, and in no comparison: `INV-0042`, and `INV-0042` with a NUL after it, are no duplicate, the second key being broken.
- **References.** A reference's key that no table it names holds is a problem of kind `reference` (`row 2, to: "bar" is no key of accounts`), and so is one two of them hold, never a silent pick; its text and value are kept. A reference into a table that could not be reached is unchecked.
- **A computed cell that stores another value than its formula gives** is a problem of kind `stale` on its cell, saying both: `row 1, total: the formula gives 90.00, the cell stores 84.20`, the cell's text as the reader holds it (an editor's buffer before it is saved, too), never said as the disk's. Its value is what the formula gives, so every formula reading it reads that; the stored text is kept as written. A file edited where no Cells writer computes (an input changed in a plain editor, a value typed over its `=`) holds one until a write resolves it ([Writing](#writing)). A reader that knew the file before the edit tells the two apart: an input changed makes the formula give another value than it gave, so the cell is one the change reached, and stored; a value typed over its `=` leaves the formula giving what it gave, so which was meant only a person knows, and nothing resolves it unasked. A reader seeing a file for the first time cannot tell, and resolves nothing. What is stored left of a formula that does not compute is no problem of this kind: the formula's own is the cell's.
- **A computed cell that stores nothing yet** (a blank field of a computed column, a lone `=`, `=96.75 / 3`) where its formula gives a value is a problem of kind `unstored` on its cell, a note and no error: `row 1, total: not stored yet: the formula gives 36.90, the next write stores it`. It reads the same, its value the formula's, and the file is valid: a file written by hand leaves its computed cells blank. Said all the same, because the file does not yet hold its data whole ([A cell](#a-cell)), so a person who deleted a value in an editor that does not write, or a reader of the file sent as it is, learns that the number is only in the tools until a write stores it. A cell whose formula gives nothing (an input blank) stores nothing and is no problem. Considered: silence, which leaves a stored value that differs said (`stale`) and a missing one not; a warning, which would weigh every hand-written file as behind.
- **A stored value stands when its inputs cannot be reached.** A computed cell whose formula reads an import that is not reached (by key, through a reference, or a table whole in an aggregate) is never computed: its value is what the file stores left of its `=`, read by its column's type, and every formula reading the cell reads that value. It is never null for it and never `stale`, and a write leaves it as stored. It is a problem of kind `unreached` on its cell, a note and no error, naming the import: `row 2, rate: from ./rates.cells, not reachable: the value the file stores stands`; a cell storing nothing yet stays null, said the same way. So a file sent alone keeps every value it was sent with, and says why none of them is computed again; once the import is reached the formula is the truth again, and a cell it no longer gives is `stale`. Considered: null, the formula's honest answer with nothing to read, which flags every stored value of a file sent alone as stale and shows a person blanks where the file holds the numbers. An input not reached worked back from what the file stores (`total = price * qty`, `total` and `qty` stored, so `price`): it holds only for one row, one unknown and an operation that can be undone, and an aggregate, a rounding, a comparison, a lookup by key, two unknowns or a zero break it; rows that each work back their own from rounded values disagree; and an input nobody wrote would read as data. A reader never derives an input from an output: packing carries the inputs exactly ([Packed imports](#packed-imports)).
- A cell has at most one problem: missing, broken (a control character in it its one problem, whatever else its text breaks), a duplicate key, a formula that does not compute, a stored value its formula no longer gives, none stored yet, one standing for an import not reached, a reference to no row, or its record too long.

## Writing

A writer round-trips: a file read and written with nothing changed is the same bytes. A changed cell is written in its column's form (its format, its pair's words, a text's `=` as `'=`, quoted only when CSV needs it); everything else is written as it was read, the header's spelling (shorthand or explicit), each line's ending, the byte order mark and the prelude included. A cell set past the end of a short row writes the blank fields before it. A changed field keeps its line aligned when it fits, its padding taking up the difference; one wider pushes its line, and laying the table out again is formatting's ([Formatting](#formatting)).

- **A write stores what it changes.** A writer computes every computed cell its change reaches (the cells whose formulas read what it changed, and theirs in turn) and stores each value left of its `=`, in the column's form; a computed cell stored nowhere yet (a blank field of a computed column, a lone `=`, `=96.75 / 3`) is stored on any write. One pass is all of it: a cycle is refused, so the cells are computed in the order their reads need, each once, and what the pass stores is what a second would. A cell the change does not reach is written as read.
- **A change reaches every file that reads it.** A formula reading another file's table (an import, followed through every import after it) is reached by a change to that file, however the change was made: an app's edit, an editor's save, a script, git. Every file it reaches is written as its own write, in the order their reads need, so a total in `quarters.cells` follows a cost saved in `q4/money.cells` with nobody opening `quarters.cells`; a packed copy kept `with { pack: "auto" }` is written fresh in the same write ([Packed imports](#packed-imports)). Only a writer that knew each file before the change knows what it reached ([Reading](#reading)), so this is the engine's, which keeps the folder ([ENGINE.md](ENGINE.md)); a reader holding one file alone stores in that file only.
- **A stale cell is left as read** by a write that did not cause it ([Reading](#reading)): what a person wrote is never overwritten unasked. A cell a change reached is not one: its stored value is what the formula gave before the change, which nobody chose. It is resolved one of two ways, each one write: store the formula's value (a client's fix, and `cells fix`, for every stale cell of a file at once), or keep the stored value as the row's override, the `=` and its formula dropped.
- **A write never stores into a written cell**: an override stays as it was written, whatever the formula would give.

A writer changing a type writes the column's header cell in its standard form ([A header cell](#a-header-cell)), the type in shorthand, then its formula when it has one (`total: EUR>=0 = cost * qty * 2`), and rewrites no written cell: a cell the new type breaks is reported, not changed, the computed cells it reaches are stored again, and a change after which a formula no longer reads (one a column it reads no longer holds the kind it needs) is refused, naming the column changed. A writer never writes a control character into a cell it changes or adds: text holding one is refused, naming the row, and the file is left as it was. A cell read with one is written back as read, as every unchanged byte is. A file written as a plain one (a `.csv`) writes each cell's value alone, its `=` and its formula dropped, so the numbers a person saw are in the file.

### Formatting

`cells fmt` writes a file as the format's standard lays it out, and so does a language server's formatting on save: every table's columns aligned, each table on its own (the file's, and each written inline in the prelude).

- **A column starts where it starts on every line.** Each field but a line's last is its value, its comma, and spaces up to the next column's start. A column is as wide as its widest value plus its comma and one space, rounded up to a stop of four characters, so a value that grows within its stop moves nothing.
- **A value wider than the cap, 32 characters, widens no column.** The value after it on its line starts one space after its comma, each later one at its own column's start as soon as the line has room, and the rest of the table is laid out as if it were not there, so one long note never moves the table.
- **A typed header cell** is written `name: type = formula`, as a declaration is written ([A header cell](#a-header-cell)); a plain name is written as read, and a quoted cell keeps its quotes (`"r: EUR = ROUND(cost, 1)"`).
- **The prelude** is written as read, but its column declarations ([Columns](#columns)): `name: type = formula`, every type starting at one column and every formula's `=` at one, a comment after one kept a space after it.
- **A file formatted is a function of its content**: formatting twice is formatting once, and `cells fmt --check` holds in CI.
- **Alignment is a writer's setting, on by default.** `cells fmt --align off`, and the language server's `align` setting, write each field with no padding; reading never depends on which.

The cost: a value made wider than its column's stop re-pads that column on every line of its table, so a one-value edit can show as a whole-column diff. The format keeps that rare or invisible. The stops let a value grow by up to four characters before anything moves; the cap keeps one long value from moving the rest; and `cells diff`, git's textconv for `*.cells`, compares two versions with their padding taken out, so a diff shows only the values that changed ([EDITORS.md](EDITORS.md)). A host that runs no textconv (a code host's web view) has its own "ignore whitespace". A merge compares lines with their padding ([Merging](#merging)), so a column re-padded on one branch can conflict with an edit to the same lines on the other. A person who prefers the smallest diff to the aligned file turns alignment off.

### Merging

Two versions of a file merge by what was written, and a computed cell is never a conflict: what is right of its `=` merges as text, what is left of it is computed again from the merged rows, and only written cells, the header and the prelude can conflict. `cells merge` is that merge, as git's merge driver (`*.cells merge=cells` in `.gitattributes`, the driver `cells merge %O %A %B`): each version's stored values dropped, the three texts merged line by line by git's own merge, and the merged file's computed cells stored again; a conflict is left in the file with git's markers. Two branches that each add a movement to a running balance merge into one file with every balance right, where a text merge conflicts on every row below the first insertion. Merging row by row, each row by its identity, is [Open](#open).

The shorthand, as a writer writes it:

- the type: a base, a currency, a named type by its name, a choice's words joined by `|`, a pair's two words joined by `/` (`true/false` is `bool`), a reference's tables and keys joined by `|`;
- then its format, then its lower bound (`>=` or `>`), then its upper bound (`<=` or `<`), then `!` or `?`. A `num` with both bounds inclusive is its range alone: `0..10`, never `num>=0<=10`;
- a bound's number in plain decimal: `-` if negative, no exponent, no zero that changes nothing (`1e3` is `1000`, `0.50` is `0.5`, `-0` is `0`);
- a word bare unless it must be quoted: it holds a space, a tab or a reserved character, or it is a choice's only word.

So `cost:EUR min 0 key` is written `cost: EUR>=0!`, and `mood:one of 'so so' meh optional` is written `mood: 'so so'|meh?`.

## Running

**Opening never runs anything.** Reading, querying, charting and sending a file run no script: a file opens with the rows its last run stored, as a spreadsheet opens with its last values. A script runs when it is asked to (`cells run <file> [name]`), or on its schedule where the engine keeps it (ENGINE.md).

- **When a schedule is due.** Its next moment after the last run its runs block records, whatever came of that run; a script never run is due at once for `every`, and at its next moment for a time of day. A time of day is its zone's, daylight saving included: one that does not exist that day runs at the moment after the gap, one that happens twice runs the first time. A run missed while no machine kept the schedule runs once when one next does, never once per miss. A failed run is a run, so it is tried again at the schedule's next moment, never in a loop.

- **A script runs only as its person allowed it.** A file is data and formulas, which reach no disk and no network; a script is a program, and a file from anyone (sent, cloned, copied in) may carry one. So a runner runs a script only at a version its person allowed, after being shown its code whole: its file, its name and the SHA-256 of its language and code, one character changed another version, asked again. A script its person writes through a Cells writer (an app, an editor's language server) is allowed as it is written. Approvals are the person's and the machine's, never the file's or the folder's, so a file that travels carries none. Unallowed, the file still opens and computes with the rows its last run stored.

- **The contract is the process.** The script's module exports a function by default; the runner calls it with the context, `{ rows, last, now }`: the table's rows as the file writes them, by column name (the engine's `run` aside), the last of them or null, and the time of the run, an instant. It returns a list of rows, objects by column name: text as it is, a number, a boolean (written in the column's words), or null (blank). Under the function, the context is JSON on stdin, each row returned a JSON line on stdout, and what the script prints on stderr, kept as the run's words.
- **What a run writes.** A table with a key gains the keys it did not have and has the rows whose keys it returned replaced; a table without one is replaced whole. Each row names, in the engine's column `run: int?` (added to the header the first time), the run that last wrote it; a row written by hand names none. The runs block gains the run, `ok`, and keeps the runs some row names, the last one and the last failure.
- **A run that fails writes nothing but its failure.** A script that exits with an error, returns a column its table does not have, or returns a row its types break leaves the stored rows as they were; the runs block gains the run, `failed`, and why. A program's broken output never lands, where a person's broken cell opens to be fixed.
- **The time a run happened is the runs table's**; the time the data is about is the data's own column.

In v0 a script is a program with the whole access of its runner, its imports resolved as Bun resolves them, and the person who runs it carries its risk, as with any program they run: an approval makes sure they read it first, and says nothing of the packages it imports. Considered: hosts and secrets declared on the `run` line, enforced as Deno enforces its flags. With no sandbox nothing would enforce them, and a declaration nothing enforces is ceremony; Deno's runtime itself ends in 2027. Trusted folders: trusting a file says nothing of the packages it imports. Adapters compiled to wasm, given only the hosts they declare: the right shape for a sandbox once one is needed, premature before. A fetch described in the file (a URL template, a path into the JSON): it grows into a language of auth, paging and retries, puts keys beside files that are shared, and makes every file opened a beacon to whoever wrote the URL. Any language a fence names: a notebook's code runs only where its packages are, and of 1.4 million Jupyter notebooks on GitHub 24% ran without an error and 4% gave the results they stored; so the fence names its language, and a second one comes with its runner (`python`, through `uv run` and the dependencies its PEP 723 block declares), never by assumption. Python first: its `Decimal` suits money, but `Infer<"header">` in `ts/` already checks a script's rows against the header in the editor, and only JavaScript runs in a page and an app, if scripts ever run there. A Zod or Pydantic model of the rows: a second copy of the header that drifts from it; a language's types for the rows are generated from the header, never written.

## Open

### What it is for

The use Cells is designed against is **plain text accounting**: books kept as plain files (a `money.cells` per folder), readable, diffable and their owner's, that say what plain text ledgers cannot: that an amount is euros, that an account must exist, that VAT is derived from the amount, that an invoice appears once; and that answer any question across every file. Every decision below is judged by whether it makes those books better.

### Decided in direction, not yet written

- **The filesystem as a graph database.** A folder of `.cells` files is a graph of typed tables, its edges the references: a reference that breaks is a problem naming the file and row, and the language server (`cells lsp`) follows a rename through every file that references the key. What is left: tools that notice before a break lands (a warning on a key renamed in the other file, a writer refusing to save a dangling reference without saying so).
  ```text
  ~/books/
  ├── accounts.cells              account: text!,kind: assets|liabilities|expenses|income
  ├── vendors.cells               vendor: text!,name: text,vat_id: text?
  ├── home/money.cells            account_from/account_to → accounts, vendor → vendors
  ├── travel/money.cells
  └── studio/money.cells
  ```
- **Fixes a reference offers**: `did you mean "rent"?`, `add it to accounts.cells`, so a new account never means opening the other file by hand; and for a plain text column, a value seen once beside a near spelling seen often a hint, never a problem.
- **Every row has an identity.** A file's key (`!`) is its rows' identity. A file with none gets an `id` column the first time an edit through the engine writes it, eight characters from a base-32 alphabet, unique within the file, assigned only by the engine and never edited through it. Edits address a row by its identity, never by its position, so a line inserted by hand elsewhere never makes an edit land on the wrong row, and two versions of a file merge row by row. A row without one (added by a tool that bypasses the engine) is found by its text until an edit through the engine next writes the file, which gives it one; a copied row's repeated id is replaced on the later copy. Apps built on the engine never show the column. The engine does this for its intents, one file at a time ([ENGINE.md](ENGINE.md)); merging row by row is not built.
- **One fact, one row, one file.** A movement is one row holding both its sides (`account_from`, `account_to`), so recording it is one write. A change that must touch several files (an account renamed in every file that references it) is an intent the engine journals before applying file by file, and replays after a crash; every intent is safe to apply twice.
- **What books need**, the accounting primitives (ACCOUNTING.md drafts the books with them):
  - **An amount whose currency is another cell**, `amount: money in commodity`, or reached through a reference, `money in account.commodity`; and commodities beyond ISO 4217 (a share, a fund, points) declared in a file.
  - **`unique`** on a column that is not the key, among its non-blank cells, and over several columns together.
  - **`final`**: a column written once, never changed by an edit. A movement whose money columns are final is corrected by a new row, never edited, as TigerBeetle never edits a transfer.
  - **References to a file's own rows** (`pending: TABLE.id`, by the name formulas read the file's own table by, [The language](#the-language)), for a payment that settles a hold, a row that corrects another, rows that are one transaction.
  - **`check`**: a rule over many rows or files, a query in the prelude returning the rows that break it, checked before every write and by `cells check` (a balance that must hold on a date, an account open on a movement's date). The prelude is where it lives, beside the data it guards; it is SQL, the language formulas are a part of, whole, in DuckDB.
- **Go's checksum log** for pinned imports, proving everyone got the same bytes and not only you, once files are published widely.
- **Queries in standards, never a language of Cells' own**: SQL, and SQL/PGQ to walk references ([The query language](#the-query-language)).
- **Tooling in every editor**, each column coloured as Rainbow CSV colours one, by its type. The tree-sitter and TextMate grammars, the language server, Cells.app and ui's table are built ([EDITORS.md](EDITORS.md)); an editor that starts a server only from an extension (VS Code) has none yet.
- **Charts everywhere a file is shown** ([Charts](#charts)): Cells.app draws them (Swift Charts), `cells chart` in a terminal, as SVG for a page; ui's components do not yet. The graph of files drawn as a graph, and a row's own sparkline read from the files that reference it (each account's months, from the movements naming it), are undecided.

### The query language

No query language is part of the format yet. What exists is a tool, `cells sql <file|folder>... <query>`, which runs a query in DuckDB's SQL over the engine's typed columns; DuckDB is in a binary of its own, `cells-sql`, which `cells sql` runs.

- **Tables.** Every `.cells` file under the paths is a table named by its file's stem, and so is every file they import from the disk and every table a prelude writes, by its name. Tables of one name are one table: every `money.cells` under a folder is `money`, its columns joined by name, null in a file without them. Every table has one more column, `file`, the path its row is read from.
- **Values.** Each column has its type's SQL type: money `DECIMAL(38, s)` at its scale, a `num` `DECIMAL` too, a date `DATE`, a choice an `ENUM` of its words in their order, a reference its key's type ([rust/README.md](rust/README.md#cells-sql) has the whole mapping and what it costs). A computed cell is the value its formula gives; a broken cell is null. One column holding two kinds of value in two files (two currencies, two choices of other words) stops the query, both files named.
- **The query** is DuckDB's SQL, whole, and the answer is written as `cells show` writes a table. No DuckDB extension is installed or loaded, so a query fetches nothing. Over files with problems the answer is still written, stderr says how many, and it exits 1; a query that cannot run exits 2.

```console
$ cells sql ~/books "
  SELECT vendor, SUM(amount) FROM money
  WHERE account_to LIKE 'expenses:home:%' AND YEAR(date) = 2026
  GROUP BY vendor"
```

**Decided in direction: standards, never a language of Cells' own.** SQL, which every reader knows and which already runs over files: a folder of `.cells` files is typed tables, so `amount` is a number and `date` a date. **SQL/PGQ** (SQL:2023, property graph queries) to walk the references: GQL's `MATCH` inside SQL, so one query walks the graph and then sums and groups. GQL alone (ISO/IEC 39075, 2024, Cypher's successor) has the graph's shape (a movement to its account, an account to its parent, a vendor to everything it touched), but is weak at sums and grouping, which are most of accounting: so the two standards composed, nothing invented. `cells sql` does not run SQL/PGQ: DuckDB has it only as the community extension `duckpgq`, which is not built in ([rust/README.md](rust/README.md#cells-sql)). A query of the shape intended, which does not run:

```console
$ cells sql ~/books "
  FROM GRAPH_TABLE(books
    MATCH (m:money)-[:account_to]->(a:accounts)
    WHERE a.account LIKE 'expenses:%'
    COLUMNS (a.account, m.amount))
  SELECT account, SUM(amount) GROUP BY account"
```

**Undecided**:

- whether a query language becomes part of the format, so every reader answers a query the same way, or stays the tools'. `cells sql` is DuckDB's dialect, not the standard's, and differs from a formula where the two meet: DuckDB's `/` and `AVG` on decimals give a `DOUBLE`, where a formula's `/` is exact decimal to 15 significant digits (34 in an amount's column).
- how a folder is a property graph: its name, which tables are vertex tables and which edge tables, and how a reference column becomes an edge ([rust/README.md](rust/README.md#cells-sql) sketches one).

### Undecided

- **The command line beyond `check`, `fix`, `fmt`, `diff`, `merge`, `pin`, `pack`, `run`, `allow`, `history`, `show`, `sql`, `serve` and `lsp`**: what else a folder-as-database needs (a visualisation of the graph, a rename followed through every file from the shell).
- **The block's name.** "Prelude" reads well in prose, but to a programmer a prelude is the standard definitions a language loads into every file unasked (Haskell's `Prelude`, Rust's `std::prelude`), where this block is one file's own, explicit declarations. The candidates: frontmatter (the shape everyone knows and searches for; Astro puts code in its own, and calls it "component frontmatter" beside its "component script"; the one risk, a tool reading a `---` at a file's top as YAML, is met only in Markdown), preamble (LaTeX's exact role, imports and definitions before the body, but it needs explaining), or keeping prelude. A rename reaches the spec, the engine's messages, both grammars' names, the language server and ui, so it is made once and whole, with no alias kept, when it is decided.
- **Words about a column and a title for the table.** A header cell has no room for a description; a column's declaration in the prelude ([Columns](#columns)) is the likely place, its syntax open.
- **Strictness.** A broken cell and a broken grammar load and are reported ([Reading](#reading)); whether a reader may also be asked to refuse a file is not decided.
- **Narrowing a reference** to some of the rows it names (`from` an account of kind `assets` or `income`): in the header cell (`from: accounts.account(kind=assets|income)`), or as a `check`, which keeps the header readable where the rule spans two files.
- **Provenance: what was known on a date, and who may write what.** Two problems. A run replaces a corrected row, so the rate a report used before the correction lives only in git. And anyone can edit a block the engine owns, or a file a process owns: locks on a Mac are advisory, so the engine can only detect an edit, never prevent one. The candidates, none decided:
  - **Owners.** A block or a file names who writes it; an edit by anyone else is refused through the engine and found by its hash otherwise. Solves who may write what; does not keep what was known.
  - **Append-only with a hash chain.** `run …, append`: rows never replaced, each with two engine columns, `recorded` and `chain` (the SHA-256 of the previous row's `chain` and the row's bytes, the core's own hash, as the engine's log chains its lines), so "as known on" is the latest row of each key recorded by then, and an edit or a removal breaks every later link. Solves what was known and makes tampering evident; does not stop a whole chain written again from scratch, which stays valid unless each link is signed by a device's key, as ENGINE.md's Later plans for the log.
  - **Transitions as a type.** `phase: pending->post|void`: the values a cell may move to from the one it holds, checked on every write, replacing ACCOUNTING.md's `unique` on `pending` if adopted. Solves an illegal change of state; does not keep history or say who wrote it.
- **A folder sent as one file.** A file's own imports pack into it ([Packed imports](#packed-imports)); a folder of files that import one another sent as one file, each of them a table of it, is not decided.
- **Several values in one cell** (`tags: text[]`, split on `;`): useful (tags on a movement), and where a table starts to stretch; undecided.

### Brainstorm, not planned

- **A replicated mode.** Engines on several devices agreeing on the order of the intent log (Viewstamped Replication, as TigerBeetle does), for books where two writers must never both succeed; the files then the cluster's readable view. The engine's write path (deterministic intents, logged) keeps it a protocol on top, never a rewrite (ENGINE.md, Later).
