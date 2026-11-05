# The conformance suite

The spec's rules as data, read by every implementation. An implementation conforms when it passes every case here; a rule in SPEC.md with no case here is a rule nobody checks, so a rule and its cases land in one commit.

## A case

A case is a `.tcsv` file and a `.json` beside it, under the folder of what it checks: `records/`, `header/`, `types/`, `choices/`, `constraints/`, `formats/`, `marks/`, `explicit/`, `prelude/`, `inference/`, `problems/`, `write/`, and `plain/` (below), the one folder whose files are not TCSV. The file's name says the rule: `constraints/money-at-least-zero.tcsv`. A case whose file does not read is named `error-…`.

The `.tcsv` is bytes, never text to be normalized: its line endings, its trailing break and its byte order mark are the case. `.gitattributes` here keeps git from touching them.

The `.json` is one of two shapes.

**The file reads**: its columns, every row's values, every problem.

```json
{
  "types": { "Category": "travel|stay|food|gear" },
  "columns": [
    { "name": "item", "type": "text!" },
    { "name": "category", "type": "Category" },
    { "name": "cost", "type": "EUR>=0" },
    { "name": "qty", "type": "int?", "inferred": true },
    { "name": "total", "type": "num?", "computed": "cost * qty" }
  ],
  "rows": [
    { "item": { "text": "fuel" }, "category": { "choice": "travel" }, "cost": { "money": "84.20", "currency": "EUR" }, "qty": { "int": "1" }, "total": { "num": "84.2" } },
    { "item": { "text": "maps" }, "category": { "choice": "gear" }, "cost": null, "qty": { "int": "2" }, "total": null }
  ],
  "problems": [
    { "row": 1, "column": "cost", "kind": "type" }
  ]
}
```

- `columns` are the stored columns in the header's order, then the computed ones in the prelude's. `columns`, `rows` and `problems` are always there, empty when there are none.
- `type` is the column's type in the shorthand a writer writes (SPEC.md, Writing): a named type by its name, inferred types included (an inferred column is always `?`). `inferred: true` marks a column the header gave no type. `computed` is a computed column's expression as the prelude writes it, without its comment and the spaces at its ends.
- `types` is present when the prelude names types: each name and its definition, in the same shorthand.
- Every row holds every column, computed ones included. A value is tagged with its kind: `text`, `int`, `num`, `pct`, `bool`, `date`, `time`, `datetime`, `url`, `email`, `id`, `money` (with `currency`), `choice`. A blank or broken cell is `null`, the JSON literal.
- Numbers are decimal strings, `-` or none, digits, then a fraction or none (`"84.20"`, `"-0.5"`, `"300000000"`), compared as decimals (`"84.20"` equals `"84.2"`), never as binary floats. `bool` is `true` or `false`. A `date` is `YYYY-MM-DD`, a `time` `HH:mm:ss`, a `datetime` the instant in UTC, `YYYY-MM-DDTHH:mm:ssZ` with a fraction only when it is not zero and no trailing zero in it (`2026-08-14T12:30:00.25Z`). `text`, `url`, `email`, `id` and `choice` are the cell's text.
- A problem is a row (0-based, from the first record), a column, and a kind: `type` (the cell breaks its type, its bounds included, or holds a control character, in any column, an inferred one too), `missing` (blank where the type has no `?`), `key` (a duplicate key, one problem per row that holds it). Problems are listed by row, then by column left to right; a cell has at most one. Words are each implementation's own; the suite pins where and what kind, never the sentence.

**The file does not read**: the grammar is broken.

```json
{ "error": { "at": "prelude line 1" } }
```

`at` is one of `the file`, `the header`, `column N` (1-based), `prelude line N` (1-based, inside the fences), `the prelude`, `row N` (0-based). `the file` is bytes that are not UTF-8, the one case whose `.tcsv` is not text; `the header` and `row N` are a record's own errors (its quoting, its line ending, a row too long); `column N` is a header cell's (its name, its type); `prelude line N` a statement's, a computed column's references included; `the prelude` a fence never closed. A case that does not read holds exactly one error, so no order between errors is pinned. An implementation may refuse the file (an exception) or open it with the error on it; either way it reports exactly this place.

## Writing

Every case that reads is also a write case: the file read and written with nothing changed is the same bytes.

`write/` holds edits, three files each and no `.json`: `<name>.tcsv` (which reads), `<name>.edit.json`, and `<name>.out.tcsv`, the bytes after the edits. `<name>.edit.json` is a list applied in order, each one of:

- `{ "row": 0, "column": "item", "text": "fuel, diesel" }`: the cell's text, written quoted only when CSV needs it;
- `{ "row": 0, "column": "paid", "value": { "bool": false } }`: a value tagged as in a read, or `null`, written in the column's form (its pair's words, its format);
- `{ "column": "cost", "type": "EUR min 1" }`: the column's type, in any spelling, written in shorthand; `""` makes it a plain column;
- `{ "insert": 1, "header": "bed:north|south", "cells": ["north", "south"] }`: a new column at that place among the stored ones, its header cell as given and a cell for each row (none: every cell blank);
- `{ "remove": "bed" }`: the column taken out, its header cell and every row's field;
- `{ "move": "qty", "to": 0 }`: the column taken out and put back at that place;
- `{ "rename": "qty", "to": "count" }`: the column under a new name, its type kept as spelled;
- `{ "compute": "vat", "expression": "total * tax / 100" }`: a new computed column, a line `vat = total * tax / 100` at the prelude's end (a file with no prelude gains one, its fences broken as the header's line);
- `{ "recompute": "total", "expression": "cost * qty * 2" }`, a `"name"` to rename it: the computed column's prelude line rewritten as `name = expression`, the spacing before its comment and the comment kept, every other byte as it was;
- `{ "uncompute": "total" }`: the computed column's prelude line taken out, the fences kept;
- any of these three with `"refused": "prelude line 1"`: the change is refused at that place (an expression that does not parse, a name taken, a column it reads missing or not a number, a `//` that would make the rest of it a comment), the file left as it was;
- a `text`, a `value` or an `insert` with `"refused": "row 0"`, and a `type` with `"refused": "column 1"`: refused at that place for a control character in the text, a cell or the type (SPEC.md, Writing), the file left as it was;
- `{ "as": ";" }` or `{ "as": "tcsv", "header": ["item:text!", "cost:EUR"] }`, always last: the file written in another dialect (`tcsv`, or a plain file with that separator, `","` `";"` `"\t"` `"|"`), and that text is the output. A plain file's header is the columns' names, computed ones included with their values a column after the stored ones (in the plain form of a decimal, `10`, never `10.00`); `header` gives a `.tcsv` file's header cells, one a stored column. A record is kept as written when the separator is the same and nothing joins it; otherwise it is written from its cells, quoted where the dialect needs it.

Edits name stored columns only. A column's fields move with it whole: a short row's missing fields are written blank, and a plain file's fields past its header stay after the columns.

## Plain files

`plain/` holds what a TCSV reader reads when a file is not TCSV: a `.csv`, a `.tsv`, plain text. Its cases are `.csv`, `.tsv` or `.txt` files, each with a `.json` of the same shape as a case that reads, or with an `.edit.json` and its output, `<name>.out.csv` (the case's own extension). An implementation reads them with the separator it finds in the first lines (the one splitting each into as many fields as the first, the most fields winning, a tie to the comma, then the tab, the semicolon, the pipe) and the plain reading:

- the header's cells are the columns' names, whatever they hold (`cost:EUR` is a name), so every column is inferred; no prelude is read (a first line `---` is the header);
- nothing is refused but bytes that are not UTF-8: a quote out of RFC 4180's places is text (`5" long`), a quoted field's text runs on after its closing quote to the separator (`"ab"c` is `abc`), a quote never closed runs to the end of the file, and a lone `\r` ends a line;
- a row of another width than the header reads as blanks with a problem of kind `ragged`, on the first cell it lacks, or on the last cell of a row too long, whose fields past the header are kept and written back;
- a cell holding a control character is broken as in a `.tcsv` file (SPEC.md, Reading): a problem of kind `type`, the cell no part of what its column infers;
- a blank name, or a name another column has already, is a header problem;
- an edited cell is quoted when it holds the file's separator, `"` or a line break, and in a file whose header and first row quote every field, always.

The `.json` also says `separator` (`","` `";"` `"\t"` `"|"`), and `names`, the header problems, when there are any: `{ "column": 1, "kind": "blank" }` or `"twice"`, columns from 0. A plain file's problems are `ragged`, and `type` for a control character. Rows are keyed by name, so a case with two columns of one name has no rows.

## The checker

`bun tests/validate.ts` checks that every case is well-formed by this contract: every `.tcsv` has its `.json` (or, in `write/`, its `.edit.json` and `.out.tcsv`; in `plain/`, every file one or the other), every `.json` has one of the two shapes, every tag and kind is known, every value names a column and every row holds every column. No implementation is involved.

## Running it

The Rust engine runs the suite (`cargo test` in `rust/`), and so does every binding of it: wasm under bun, Swift in swift-utils. Each reads this folder, or a vendored copy of it.

Each runner prints every case it fails with the case's path, and exits non-zero on any.
