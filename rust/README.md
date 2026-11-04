# The core

One reader and writer of TCSV, in Rust, that every surface uses: the command line, the page (as WebAssembly), the Apple platforms (a C ABI in an XCFramework), and next the daemon and the language server. [SPEC.md](../SPEC.md) is the law and [tests/](../tests) holds it to it.

| Crate | What it is | Depends on |
|---|---|---|
| `tcsv` | the reader, the writer, the columns, the Arrow export | nothing; `no_std` with `alloc`, the `std` feature adds threads |
| `tcsv-cli` | `tcsv check`, `tcsv fmt --check` | `tcsv` |
| `tcsv-wasm` | the core as WebAssembly, and `js/tcsv.ts`, a synchronous reader on it | `tcsv` without `std` |
| `tcsv-ffi` | the C ABI, `tcsv.h` (cbindgen), `TCSVCore.xcframework` | `tcsv` |
| `tcsv-bench` | budgets generated and read, against the csv crate | `tcsv`, `csv` (bench only) |

`cargo test` runs the conformance suite (`TCSV_SUITE`, else `../tests`), the properties (written back is the same bytes, an edit reads back as written, a tail read alone is the whole read, the parallel reading is the sequential one) and each module's own tests.

## `tcsv`

```rust
let mut doc = tcsv::read(bytes)?;          // or read_parallel(bytes, threads)
doc.rows(); doc.columns(); doc.problems(); // problems by row, then column
doc.value(row, column);                    // Value::Decimal, ::Date, ::Choice, ...
doc.describe(&doc.problems()[0]);          // row 3, cost: "8x.50" is not a number
doc.set_cell(row, column, "90.00");        // or set_value, set_type (written in shorthand)
doc.write();                               // every unchanged byte as read
let doc = doc.extend(grown_bytes)?;        // only the appended records read
doc.apply(&Change::Cell { row: RowAt::Key("INV-7".into()), column: "amount".into(), was: Some("950.00".into()), text: "1000".into() })?;
let doc = doc.assign_ids(|| random_bits())?;
let (schema, array) = tcsv::arrow::export(&doc);
```

- **Reading.** Records are found 64 bytes at a time: quote and separator bitmasks (NEON, SSE2, wasm SIMD, else a portable loop), quoted stretches by prefix XOR; a block that breaks RFC 4180 is read again byte by byte, which names the record. Field spans are batched 4096 rows at a time and decoded column by column, each kind of column in a loop of its own. `read_parallel` splits the body at record boundaries (quote parity), reads the parts on threads, joins each column on its own thread and checks keys on threads; its result is `read`'s byte for byte. The core starts no thread unless asked.
- **Columns** are Arrow's layout: `i64`; `Decimal128(38, s)` at one scale per column (the most digits after the point any value has); `Date32`; `Time32[s]`; `Timestamp[s|ms|us|ns, UTC]` at one unit per column; bits; `i16` dictionary indices; `utf8` offsets and bytes; a validity bitmap each. Buffers are `Arc`-shared: an export hands them out, an edit copies only what someone still holds.
- **Problems** by place and kind (`type`, `missing`, `key`), keys compared by value. **Errors** by place: `At::Header`, `Column(n)`, `PreludeLine(n)`, `Prelude`, `Row(n)`, `File`.
- **Computed columns** in exact decimals (`exact::Exact`): `+ - *` exact, `/` to 34 significant digits, half to even, stored at the column's scale within 38 digits.
- **Identity** (`RowAt`, `Change`, `Refused`, `Version`): a row by its key, its id, or its text; a change applies only to the cell it was made against. Ids come from bits the caller gives.

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

`bun tcsv-wasm/build.ts` builds it (opt-level `z`, LTO, SIMD on, `wasm-opt -Oz` when installed) and embeds it in `js/wasm.ts` as base64, so `read(text)` compiles the module synchronously, no `await init()`. 145.6 KB raw, 62.8 KB gzip, **53.4 KB brotli**, no imports. Chrome allows a synchronous `new WebAssembly.Module` and `new WebAssembly.Instance` on the main thread up to 8 MB since Chrome 115 (4 KB before), the limit taken from the largest function the spec allows ([Chrome 115 beta](https://developer.chrome.com/blog/chrome-115-beta/), [the intent to ship](https://groups.google.com/a/chromium.org/g/blink-dev/c/nJw2zwaiJ2s/m/EYPgC5D3LwAJ)). `bun tcsv-wasm/js/conformance.ts` runs the suite through the wrapper; `js/bench.ts` races it against `ts/src/tcsv.ts`.

## Apple

`bash tcsv-ffi/build-apple.sh` builds aarch64-apple-darwin and x86_64-apple-darwin (one universal slice), aarch64-apple-ios and aarch64-apple-ios-sim, strips debug info and local symbols, and writes `target/apple/TCSVCore.xcframework` for a SwiftPM `binaryTarget`. Each static library is about 19 MB built and 2.3 MB stripped (std included); the framework 9.1 MB (78 MB unstripped); what a link keeps is about 0.5 MB (the check below, Swift and all, is 633 KB). `tcsv-ffi/swift` is that package and a check that walks a file's columns through the Arrow C Data Interface.

A C header over uniffi, by measurement: the same surface through uniffi 0.32.2 links 591 KB stripped against 518 KB, pulls 59 crates where this pulls none, and moves strings and buffers by copy, while the columns must cross as the doc's own pointers.
