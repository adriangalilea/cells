# The core

One reader and writer of TCSV, in Rust, that every surface uses: the command line, the page (as WebAssembly), the Apple platforms (a C ABI in an XCFramework), and next the daemon and the language server. [SPEC.md](../SPEC.md) is the law and [tests/](../tests) holds it to it.

| Crate | What it is | Depends on |
|---|---|---|
| `tcsv` | the reader, the writer, the columns, the Arrow export | nothing; `no_std` with `alloc`, the `std` feature adds threads |
| `tcsv-cli` | `tcsv check`, `tcsv fmt --check` | `tcsv` |
| `tcsv-wasm` | the core as WebAssembly, and `js/tcsv.ts`, a synchronous reader on it | `tcsv` without `std` |
| `tcsv-ffi` | the C ABI, `tcsv.h` (cbindgen), `TCSVCore.xcframework` | `tcsv` |
| `tcsv-bench` | budgets generated and read, against the csv crate | `tcsv`, `csv` (bench only) |

`cargo test` runs the conformance suite (`TCSV_SUITE`, else `../tests`), the properties (written back is the same bytes, an edit reads back as written, a tail read alone is the whole read, the parallel reading is the sequential one, columns reshaped keep their cells, a file written in another dialect reads back its cells, each for `.tcsv` files and for plain ones) and each module's own tests.

## `tcsv`

```rust
let mut doc = tcsv::read(bytes)?;          // or read_parallel(bytes, threads)
doc.rows(); doc.columns(); doc.problems(); // problems by row, then column
doc.value(row, column);                    // Value::Decimal, ::Date, ::Choice, ...
doc.describe(&doc.problems()[0]);          // row 3, cost: "8x.50" is not a number
doc.why(&doc.problems()[0]);               // "8x.50" is not a number
doc.set_cell(row, column, "90.00")?;       // or set_value, set_type (written in shorthand)
doc.write();                               // every unchanged byte as read
let doc = doc.extend(grown_bytes)?;        // only the appended records read
let doc = doc.rearranged(&[Row::Kept(2), Row::New(&["fuel", "1"])], threads)?; // rows moved, added, out
let doc = doc.reshaped(&[ColumnAt::Kept(1), ColumnAt::Renamed(0, "item"), ColumnAt::New("note", &[])], threads)?;
let csv = tcsv::read_as(bytes, Dialect::Plain(tcsv::detect(&bytes)), threads)?; // a .csv, a .tsv
csv.written_as(Dialect::Tcsv, Some(&["item:text", "cost:EUR"]));            // another dialect
let spans = tcsv::tokens::tokens(text);    // what each part of any text is, a broken one too
doc.apply(&Change::Cell { row: RowAt::Key("INV-7".into()), column: "amount".into(), was: Some("950.00".into()), text: "1000".into() })?;
let doc = doc.assign_ids(|| random_bits())?;
let (schema, array) = tcsv::arrow::export(&doc);
```

- **Reading.** Records are found 64 bytes at a time: quote and separator bitmasks (NEON, SSE2, wasm SIMD, else a portable loop), quoted stretches by prefix XOR; a block that breaks RFC 4180 is read again byte by byte, which names the record. Field spans are batched 4096 rows at a time and decoded column by column, each kind of column in a loop of its own. A batch's bytes are searched for control characters first, 64 at a time (a table lookup on NEON, compares elsewhere, a block with a byte 0xC2 read exactly against the byte after it); in a batch holding one, a cell is broken when one starts inside its span, so no cell's text is read twice (1M rows: 1% on one thread with none, 5% with one every thousand rows). `read_parallel` splits the body at record boundaries (quote parity), reads the parts on threads, joins each column on its own thread and checks keys on threads; its result is `read`'s byte for byte. The core starts no thread unless asked.
- **Columns** are Arrow's layout: `i64`; `Decimal128(38, s)` at one scale per column (the most digits after the point any value has); `Date32`; `Time32[s]`; `Timestamp[s|ms|us|ns, UTC]` at one unit per column; bits; `i16` dictionary indices; `utf8` offsets and bytes; a validity bitmap each. Buffers are `Arc`-shared: an export hands them out, an edit copies only what someone still holds.
- **Problems** by place and kind (`type`, `missing`, `key`), keys compared by value; a control character in a cell is a `type` problem (`Why::Control`, its code point), and `set_cell`, `set_value` and a new row's or column's cells refuse one (`Err` at the row, the doc untouched). **Errors** by place: `At::Header`, `Column(n)`, `PreludeLine(n)`, `Prelude`, `Row(n)`, `File`.
- **Computed columns** in exact decimals (`exact::Exact`): `+ - *` exact, `/` to 34 significant digits, half to even, stored at the column's scale within 38 digits.
- **Identity** (`RowAt`, `Change`, `Refused`, `Version`): a row by its key, its id, or its text; a change applies only to the cell it was made against. Ids come from bits the caller gives.
- **Rows rearranged** (`Doc::rearranged`): the file with exactly the rows asked, each kept one its record as it is now, each new one written from its cells, read again. Moving, adding and taking out rows; the reader is fast enough that it is the whole file read again (13.8 ms at 1M rows on 18 threads).
- **Columns reshaped** (`Doc::reshaped`, `ColumnAt`): the file with exactly the columns asked, each kept one its header cell and every row's field as written, a renamed one its type kept, a new one its header cell and cells, read again. Inserting, removing, moving and renaming columns; a `.tcsv` file whose columns would no longer read (two of one name, a computed column reading one taken out) is refused, the doc untouched.
- **Plain files** (`Dialect::Plain`, `detect`, `read_as`): a `.csv`, a `.tsv` or plain text, its separator (`,` `;` `\t` `|`) the one its first lines agree on, kept on write. The header is names alone, no prelude, and nothing is refused but bytes that are not UTF-8: a row of another width reads as blanks with a `ragged` problem (its fields past the header kept), a stray quote is text, a quote never closed runs to the end, a lone `\r` ends a line, and a blank or repeated name is a header problem (`Doc::naming`). An edited cell is quoted for the file's separator, and always in a file that quotes every field (`Doc::quotes_all`). The scanner takes the separator as a byte, so its speed is the comma's. tests/README.md, "Plain files", is the reading.
- **Computed columns changed** (`Doc::recomputed`, `Computing`): one computed column's prelude line rewritten (its comment kept), a new one added at the prelude's end, or one taken out, every other byte as it was; a prelude that no longer reads is refused at its line with the reason.
- **Another dialect** (`Doc::written_as`): a plain file written as TCSV (with the header cells a conversion types it by) or with another separator, a `.tcsv` file written plain (its computed columns' values a column each); a record kept as written where nothing changes it.
- **Spans** (`tokens::tokens`): the text read as what each part is, the fences, comments, the prelude's statements, every header cell's name and its type's parts (base, mark, bound, format, choice), and every cell by its column. Read from the text alone, never refused, a part with no meaning an `Error` span. What swift-utils' Syntax colours a `.tcsv` file by, and what the language server's semantic tokens will carry.

## Numbers

Apple M5 Max, 18 cores, macOS 27, other work running beside it (load near 7); best of five (three at 10M). The budget: `item:text!,category:travel|stay|food|gear,cost:EUR>=0,qty:int>=1,paid:yes/no,on:date,note:text?`, half the notes quoted with a comma. `cargo run --release -p tcsv-bench -- gen <rows> <file>`, then `read <file>`.

| Rows | File | tcsv, 1 thread | tcsv, 18 threads | csv crate, fields only | TS reader (bun) | wasm (bun) |
|---|---|---|---|---|---|---|
| 1k | 0.06 MB | 0.06 ms, 917 MB/s | 0.06 ms | 0.05 ms, 1073 MB/s | 1.5 ms, 38 MB/s | 0.4 ms |
| 100k | 5.6 MB | 7.5 ms, 747 MB/s | 2.3 ms, 2.5 GB/s | 4.3 ms, 1.3 GB/s | 130 ms, 43 MB/s | 12 ms, 464 MB/s |
| 1M | 55.8 MB | 78 ms, 718 MB/s, 12.9M rows/s | 13.8 ms, 4.0 GB/s, 72M rows/s | 44 ms | 1575 ms, 35 MB/s | 130 ms, 428 MB/s |
| 10M | 558 MB | 945 ms, 591 MB/s | 160 ms, 3.5 GB/s, 62M rows/s | 424 ms | | |

Peak memory at 10M rows (`/usr/bin/time -l tcsv-bench once <file> <threads>`): 1.37 GB on one thread, 1.64 GB on eighteen, for a 558 MB file kept whole (the source is the round trip) and about 700 MB of columns. The csv crate finds fields and decodes nothing; the core types every cell, checks the key and keeps the spans to write back. The wasm reader leaves values in its memory; building every row's object as the TypeScript reader does takes it to 339 ms at 1M rows.

## WebAssembly

`bun tcsv-wasm/build.ts` builds it (opt-level `z`, LTO, SIMD on, `wasm-opt -Oz` when installed) and embeds it in `js/wasm.ts` as base64, so `read(text)` compiles the module synchronously, no `await init()`. 169.3 KB raw, 72.3 KB gzip, **61.4 KB brotli**, no imports. `readAs(text, dialect)` reads a plain file (`detect` finds its separator), `reshape` and `writtenAs` are the columns reshaped and another dialect. A host that keeps its own rows (ui's table) takes every cell's text in one call (`doc.cells()`, one decode cut at offsets, never a call per cell), each header cell's type as spelled (`column.spelled`), the text before the header (`doc.prelude`), and writes a file back from its cells with `compose`, which never touches a doc; a problem says itself with its place (`why`) or without it, for the cell it sits on (`reason`). Chrome allows a synchronous `new WebAssembly.Module` and `new WebAssembly.Instance` on the main thread up to 8 MB since Chrome 115 (4 KB before), the limit taken from the largest function the spec allows ([Chrome 115 beta](https://developer.chrome.com/blog/chrome-115-beta/), [the intent to ship](https://groups.google.com/a/chromium.org/g/blink-dev/c/nJw2zwaiJ2s/m/EYPgC5D3LwAJ)). `bun tcsv-wasm/js/conformance.ts [--lib <a vendored copy>]` runs the suite through the wrapper (cells and compose included); `js/bench.ts` races it against `ts/src/tcsv.ts`.

## Apple

`bash tcsv-ffi/build-apple.sh` builds aarch64-apple-darwin and x86_64-apple-darwin (one universal slice), aarch64-apple-ios, aarch64-apple-tvos and their simulators, strips debug info and local symbols, and writes `target/apple/TCSVCore.xcframework` for a SwiftPM `binaryTarget`, zipped beside it with its checksum. Each static library is about 19 MB built and 2.3 MB stripped (std included); the framework 14 MB, its zip 4.4 MB; what a link keeps is about 0.5 MB (the check below, Swift and all, is 633 KB). No watchOS slice: watchOS 26 runs on arm64_32 (Series 6 to 8), which Rust builds only on nightly. The `apple` workflow (`gh workflow run apple -f version=X`) builds it on macos-26 and releases `apple-X` with the zip; swift-utils pins that asset by URL and checksum, and its `TCSV` is the binding over it, held to the suite. `tcsv-ffi/swift` is that package and a check that walks a file's columns through the Arrow C Data Interface.

A C header over uniffi, by measurement: the same surface through uniffi 0.32.2 links 591 KB stripped against 518 KB, pulls 59 crates where this pulls none, and moves strings and buffers by copy, while the columns must cross as the doc's own pointers.
