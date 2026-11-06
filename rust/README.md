# The core

One reader and writer of Cells, in Rust, that every surface uses: the command line, the page (as WebAssembly), the Apple platforms (a C ABI in an XCFramework), the language server (`cells lsp`, [EDITORS.md](../EDITORS.md)), and next the daemon. [SPEC.md](../SPEC.md) is the law and [tests/](../tests) holds it to it.

| Crate | What it is | Depends on |
|---|---|---|
| `cells` | the reader, the writer, the columns, formulas, imports, the Arrow export | nothing; `no_std` with `alloc`, the `std` feature adds threads |
| `cells-cli` | `cells check`, `fmt` (and `--check`), `diff`, `pin`, `run`, `show`, and `lsp`, the language server (tower-lsp); imports followed from the disk, git and a cache, a file an editor holds open as it holds it | `cells`; `git`, `curl` and `bun` when a file asks |
| `cells-wasm` | the core as WebAssembly, and `js/cells.ts`, a synchronous reader on it | `cells` without `std` |
| `cells-ffi` | the C ABI, `cells.h` (cbindgen), `CellsCore.xcframework` | `cells` |
| `cells-bench` | budgets generated and read, against the csv crate | `cells`, `csv` (bench only) |

`cargo test` runs the conformance suite (`Cells_SUITE`, else `../tests`), the properties (written back is the same bytes, an edit reads back as written, a tail read alone is the whole read, the parallel reading is the sequential one, columns reshaped keep their cells, a file written in another dialect reads back its cells, each for `.cells` files and for plain ones) and each module's own tests.

## `cells`

```rust
let mut doc = cells::read(bytes)?;          // or read_parallel(bytes, threads)
let doc = cells::read_with(bytes, &mut cells::files::Files::new(&held, "books/money.cells"), 1)?; // imports followed
doc.rows(); doc.columns(); doc.problems(); // problems by row, then column
doc.value(row, column);                    // Value::Decimal, ::Date, ::Choice, ...
doc.columns()[k].origin(row);              // Origin::Written, ::Formula (the column's), ::Cell (its own)
doc.tables(); doc.scripts(); doc.named();  // the prelude's tables, scripts and types, imported ones too
doc.describe(&doc.problems()[0]);          // row 3, cost: "8x.50" is not a number
doc.why(&doc.problems()[0]);               // "8x.50" is not a number
doc.set_cell(row, column, "90.00")?;       // or set_value, set_type ("EUR=cost * qty", written in shorthand)
doc.write();                               // every unchanged byte as read
let doc = doc.extend(grown_bytes)?;        // only the appended records read
let doc = doc.rearranged(&[Row::Kept(2), Row::New(&["fuel", "1"])], threads)?; // rows moved, added, out
let doc = doc.reshaped(&[ColumnAt::Kept(1), ColumnAt::Renamed(0, "item"), ColumnAt::New("note", &[])], threads)?;
let csv = cells::read_as(bytes, Dialect::Plain(cells::detect(&bytes)), threads)?; // a .csv, a .tsv
csv.written_as(Dialect::Cells, Some(&["item:text", "cost:EUR"]));            // another dialect
let spans = cells::tokens::tokens(text);    // what each part of any text is, a broken one too
doc.apply(&Change::Cell { row: RowAt::Key("INV-7".into()), column: "amount".into(), was: Some("950.00".into()), text: "1000".into() })?;
let doc = doc.assign_ids(|| random_bits())?;
let (schema, array) = cells::arrow::export(&doc);
```

- **Reading.** Records are found 64 bytes at a time: quote and separator bitmasks (NEON, SSE2, wasm SIMD, else a portable loop), quoted stretches by prefix XOR; a block that breaks RFC 4180 is read again byte by byte, which names the record. Field spans are batched 4096 rows at a time and decoded column by column, each kind of column in a loop of its own. A batch's bytes are searched for control characters first, 64 at a time (a table lookup on NEON, compares elsewhere, a block with a byte 0xC2 read exactly against the byte after it); in a batch holding one, a cell is broken when one starts inside its span, so no cell's text is read twice (1M rows: 1% on one thread with none, 5% with one every thousand rows). `read_parallel` splits the body at record boundaries (quote parity), reads the parts on threads, joins each column on its own thread and checks keys on threads; its result is `read`'s byte for byte. The core starts no thread unless asked.
- **Columns** are Arrow's layout: `i64`; `Decimal128(38, s)` at one scale per column (the most digits after the point any value has); `Date32`; `Time32[s]`; `Timestamp[s|ms|us|ns, UTC]` at one unit per column; bits; `i16` dictionary indices; `utf8` offsets and bytes; a validity bitmap each. Buffers are `Arc`-shared: an export hands them out, an edit copies only what someone still holds.
- **Problems** by place and kind (`type`, `missing`, `key`, `ragged`, `formula`, `reference`), keys compared by value; a control character in a cell is a `type` problem (`Why::Control`, its code point), and `set_cell`, `set_value` and a new row's or column's cells refuse one (`Err` at the row, the doc untouched). **Errors** by place: `At::Header`, `Column(n)`, `PreludeLine(n)`, `Prelude`, `Row(n)`, `File`.
- **The prelude** (`prelude::parse`, `Statement`): types, imports, tables in `` ```cells `` fences, scripts with their schedules (`Schedule`, `Trigger`), read in order, fence-aware, so a `---` inside a fence closes nothing. A table in a fence is read as a file of its own with the types and tables above it.
- **Formulas** (`expr`): parsed, resolved against a `Scope` (every name a place, every kind checked before anything runs) and evaluated against a `Source`, lazily, each computed cell once, so formulas read each other in any order and a cell that reads itself is found. Exact decimals (`exact::Exact`): `+ - *` exact, `/` to 34 significant digits, half to even. What a formula gives is written as text and read by its column's decoder, so a result is checked exactly as a typed cell is.
- **Imports** (`Imports`, `read_with`): a reader is handed what follows them. `files::Files` follows them through files a host holds by path (the suite, a page), checking a pin's SHA-256 (`sha256`); `Alone`, `read`'s, follows none, so what a file imports reads as unchecked. A doc read again after an edit takes its imports as they were followed.
- **Identity** (`RowAt`, `Change`, `Refused`, `Version`): a row by its key, its id, or its text; a change applies only to the cell it was made against. Ids come from bits the caller gives.
- **Rows rearranged** (`Doc::rearranged`): the file with exactly the rows asked, each kept one its record as it is now, each new one written from its cells, read again. Moving, adding and taking out rows; the reader is fast enough that it is the whole file read again (13.8 ms at 1M rows on 18 threads).
- **Columns reshaped** (`Doc::reshaped`, `ColumnAt`): the file with exactly the columns asked, each kept one its header cell and every row's field as written, a renamed one its type and formula kept, a new one its header cell and cells, read again. Inserting, removing, moving and renaming columns; a `.cells` file whose columns would no longer read (two of one name, a formula reading one taken out) is refused, the doc untouched.
- **Plain files** (`Dialect::Plain`, `detect`, `read_as`): a `.csv`, a `.tsv` or plain text, its separator (`,` `;` `\t` `|`) the one its first lines agree on, kept on write. The header is names alone, no prelude, and nothing is refused but bytes that are not UTF-8: a row of another width reads as blanks with a `ragged` problem (its fields past the header kept), a stray quote is text, a quote never closed runs to the end, a lone `\r` ends a line, and a blank or repeated name is a header problem (`Doc::naming`). An edited cell is quoted for the file's separator, and always in a file that quotes every field (`Doc::quotes_all`). The scanner takes the separator as a byte, so its speed is the comma's. tests/README.md, "Plain files", is the reading.
- **Another dialect** (`Doc::written_as`): a plain file written as Cells (with the header cells a conversion types it by, its text that would read as a formula escaped) or with another separator, a `.cells` file written plain (every computed cell its value); a record kept as written where nothing changes it.
- **Spans** (`tokens::tokens`): the text read as what each part is, the fences, comments, the prelude's statements (imports, a schedule's words, a script's code), every header cell's name, its type's parts (base, mark, bound, format, choice, reference) and its formula's, a table written inline as a table, and every cell by its column, a cell's formula by its parts. Read from the text alone, never refused, a part with no meaning an `Error` span. What swift-utils' Syntax colours a `.cells` file by.
- **Places** (`Doc::place`, `header_span`, `cell_span`, `expr::names`): what is at a byte of the text, where each header cell and cell is, every name a formula reads with each part where it is written. What the language server finds, follows and renames by.
- **Breaks and spans** (`Doc::broken`, `Doc::spans`, `Doc::broken_spans`): where the grammar breaks, each break taking out only what it breaks, and where every problem and break is in the text written now.

A formula column costs its evaluation: 1M rows with `total:EUR=cost * qty` read in 236 ms on one thread and 170 ms on eighteen (the file without it in 74 and 12), the formula evaluated on one thread, its results written as text and read again by the column's decoder.

## Numbers

Apple M5 Max, 18 cores, macOS 27, other work running beside it (load near 7); best of five (three at 10M). The budget: `item:text!,category:travel|stay|food|gear,cost:EUR>=0,qty:int>=1,paid:yes/no,on:date,note:text?`, half the notes quoted with a comma. `cargo run --release -p cells-bench -- gen <rows> <file>`, then `read <file>`.

| Rows | File | cells, 1 thread | cells, 18 threads | csv crate, fields only | TS reader (bun) | wasm (bun) |
|---|---|---|---|---|---|---|
| 1k | 0.06 MB | 0.06 ms, 917 MB/s | 0.06 ms | 0.05 ms, 1073 MB/s | 1.5 ms, 38 MB/s | 0.4 ms |
| 100k | 5.6 MB | 7.5 ms, 747 MB/s | 2.3 ms, 2.5 GB/s | 4.3 ms, 1.3 GB/s | 130 ms, 43 MB/s | 12 ms, 464 MB/s |
| 1M | 55.8 MB | 78 ms, 718 MB/s, 12.9M rows/s | 13.8 ms, 4.0 GB/s, 72M rows/s | 44 ms | 1575 ms, 35 MB/s | 130 ms, 428 MB/s |
| 10M | 558 MB | 945 ms, 591 MB/s | 160 ms, 3.5 GB/s, 62M rows/s | 424 ms | | |

Peak memory at 10M rows (`/usr/bin/time -l cells-bench once <file> <threads>`): 1.37 GB on one thread, 1.64 GB on eighteen, for a 558 MB file kept whole (the source is the round trip) and about 700 MB of columns. The csv crate finds fields and decodes nothing; the core types every cell, checks the key and keeps the spans to write back. The wasm reader leaves values in its memory; building every row's object as the TypeScript reader does takes it to 339 ms at 1M rows.

## WebAssembly

`bun cells-wasm/build.ts` builds it (opt-level `z`, LTO, SIMD on, `wasm-opt -Oz` when installed) and embeds it in `js/wasm.ts` as base64, so `read(text)` compiles the module synchronously, no `await init()`. 335.0 KB raw, 143.5 KB gzip, **118.5 KB brotli**, no imports (the formulas, the prelude's tables and imports, SHA-256 and Unicode's case tables for `lower` and `upper` took it from 61.4 KB). `read(text, { at, files })` follows the file's imports through the files a page holds, by path; `readAs(text, dialect)` reads a plain file (`detect` finds its separator), `reshape` and `writtenAs` are the columns reshaped and another dialect; `column.formula`, `column.origin(row)` and `column.targets` say what computes and what a reference names. A host that keeps its own rows (ui's table) takes every cell's text in one call (`doc.cells()`, one decode cut at offsets, never a call per cell), each header cell's type as spelled (`column.spelled`), the text before the header (`doc.prelude`), and writes a file back from its cells with `compose`, which never touches a doc; a problem says itself with its place (`why`) or without it, for the cell it sits on (`reason`), and where it is (`span`, in UTF-16 units); `doc.broken` is where the grammar breaks, and `doc.unread` the bytes after a record that does not read, which `compose` writes back. Chrome allows a synchronous `new WebAssembly.Module` and `new WebAssembly.Instance` on the main thread up to 8 MB since Chrome 115 (4 KB before), the limit taken from the largest function the spec allows ([Chrome 115 beta](https://developer.chrome.com/blog/chrome-115-beta/), [the intent to ship](https://groups.google.com/a/chromium.org/g/blink-dev/c/nJw2zwaiJ2s/m/EYPgC5D3LwAJ)). `bun cells-wasm/js/conformance.ts [--lib <a vendored copy>]` runs the suite through the wrapper (cells and compose included); `js/bench.ts` races it against `ts/src/cells.ts`.

## Apple

`bash cells-ffi/build-apple.sh` builds aarch64-apple-darwin and x86_64-apple-darwin (one universal slice), aarch64-apple-ios, aarch64-apple-tvos and their simulators, strips debug info and local symbols, and writes `target/apple/CellsCore.xcframework` for a SwiftPM `binaryTarget`, zipped beside it with its checksum. Each static library is about 19 MB built and 2.3 MB stripped (std included); the framework 14 MB, its zip 4.4 MB; what a link keeps is about 0.5 MB (the check below, Swift and all, is 633 KB). No watchOS slice: watchOS 26 runs on arm64_32 (Series 6 to 8), which Rust builds only on nightly. The `apple` workflow (`gh workflow run apple -f version=X`) builds it on macos-26 and releases `apple-X` with the zip; swift-utils pins that asset by URL and checksum, and its `Cells` is the binding over it, held to the suite. `cells-ffi/swift` is that package and a check that walks a file's columns through the Arrow C Data Interface. The header carries `cells_origin`, `CellsBaseRef`, the `formula` and `reference` problem kinds, `CellsPartFormula`, `cells_read_with` and `cells_read_at`, a file read with its imports followed, and the `stale` kind with `cells_fix_stale`, `cells_keep_stale` and `cells_stored_since`, a write storing what it reached; swift-utils pins `apple-0.3.0`, the first release with them. Built since, for `apple-0.4.0`: `cells_complete`, what may be written at a byte of a doc's text (`cells::complete`, the completions `cells lsp` gives and the prelude's statements, types, schedules and declarations), each read with `cells_completion` (its kind and the bytes it replaces) and `cells_completion_part` (its label, detail, insert, filter and sort), the tables it names the ones the doc holds.

A C header over uniffi, by measurement: the same surface through uniffi 0.32.2 links 591 KB stripped against 518 KB, pulls 59 crates where this pulls none, and moves strings and buffers by copy, while the columns must cross as the doc's own pointers.
