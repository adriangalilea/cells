# The conformance suite

The spec's rules as data, read by every implementation. An implementation conforms when it passes every case here; a rule in SPEC.md with no case here is a rule nobody checks, so a rule and its cases land in one commit.

## A case

A case is a `.tcsv` file and a `.json` beside it, under the folder of what it checks (`header/`, `types/`, `prelude/`, `records/`, `inference/`, `problems/`, `write/`). The file's name says the rule: `types/money-at-least-zero.tcsv`.

The `.json` is one of two shapes.

**The file reads**: its columns, every row's values, every problem.

```json
{
  "columns": [
    { "name": "item", "type": "text!" },
    { "name": "cost", "type": "EUR>=0" },
    { "name": "total", "type": "num?", "computed": "cost * qty" }
  ],
  "rows": [
    { "item": { "text": "fuel" }, "cost": { "money": "84.20", "currency": "EUR" }, "total": { "num": "84.2" } }
  ],
  "problems": [
    { "row": 2, "column": "cost", "kind": "type" }
  ]
}
```

- `type` is the column's type in canonical shorthand, inferred types included (an inferred column is always `?`).
- A value is tagged with its kind: `text`, `int`, `num`, `pct`, `bool`, `date`, `time`, `datetime`, `url`, `email`, `money` (with `currency`), `choice`, or `null` for a blank or broken cell. Numbers are decimal strings, compared as decimals (`"84.20"` equals `"84.2"`), never as binary floats.
- A problem is a row (0-based, from the first record), a column, and a kind: `type` (the cell breaks its type), `missing` (blank where the type has no `?`), `key` (a duplicate key, one problem per row that holds it). Words are each implementation's own; the suite pins where and what kind, never the sentence.

**The file does not read**: the grammar is broken.

```json
{ "error": { "at": "prelude line 1" } }
```

`at` is one of `the header`, `column N` (1-based), `prelude line N` (1-based, inside the fences), `the prelude`, `row N` (0-based). An implementation may refuse the file (an exception) or open it with the error on it; either way it reports exactly this place.

## Writing

Every case that reads is also a write case: the file read and written with nothing changed is the same bytes. `write/` holds edits: `<name>.tcsv`, `<name>.edit.json` (`[{ "row": 0, "column": "item", "text": "fuel, diesel" }]` or `[{ "column": "cost", "type": "EUR>0" }]`), and `<name>.out.tcsv`, the bytes after them.

## Running it

- TypeScript: `bun run conformance` in `ts/`.
- Rust: `cargo test` in `rust/`.
- Swift: swift-utils' `TCSVConformance` tests read this folder from a checkout of this repo (`TCSV_SUITE`, else `../tcsv/tests`).

Each runner prints every case it fails with the case's path, and exits non-zero on any.
