//! `tcsv-bench gen <rows> <file>` writes a typed budget; `tcsv-bench read <file> [threads]`
//! reads it with the core, once on one thread and once on `threads`, and with the csv
//! crate as the untyped ceiling (every field found, nothing decoded). Each prints MB/s and
//! rows/s, the best of five runs. Peak memory: run under `/usr/bin/time -l`.

use std::fs;
use std::hint::black_box;
use std::io::Write;
use std::time::Instant;

const HEADER: &str = "item:text!,category:travel|stay|food|gear,cost:EUR>=0,qty:int>=1,paid:yes/no,on:date,note:text?";

/// xorshift64*, seeded, so every machine writes the same file.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

fn generate(rows: u64, path: &str, header: &str) {
    let mut rng = Rng(0x7C5_0001);
    let mut out = Vec::with_capacity(rows as usize * 72);
    out.extend_from_slice(header.as_bytes());
    out.push(b'\n');
    let categories = ["travel", "stay", "food", "gear"];
    let notes = ["the market, on the square", "two nights, one late", "fuel, then tolls", "a gift, wrapped"];
    for n in 0..rows {
        let r = rng.next();
        let cost = r % 100_000;
        let days = (r >> 20) % 2500;
        let (y, m, d) = civil(20_000 + days as i32 - 1_800);
        write!(
            out,
            "item-{n:08},{},{}.{:02},{},{},{y:04}-{m:02}-{d:02},",
            categories[(r >> 40) as usize % 4],
            cost / 100,
            cost % 100,
            1 + (r >> 44) % 9,
            if (r >> 50) & 1 == 1 { "yes" } else { "no" }
        )
        .unwrap();
        if (r >> 51) & 1 == 1 {
            write!(out, "\"{} {}\"", notes[(r >> 52) as usize % 4], n % 97).unwrap();
        }
        out.push(b'\n');
    }
    fs::write(path, out).unwrap();
}

fn civil(z: i32) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe as i32 + era * 400 + (m <= 2) as i32;
    (y, m, d)
}

fn best(runs: usize, mut f: impl FnMut() -> usize) -> (f64, usize) {
    let mut t = f64::MAX;
    let mut rows = 0;
    for _ in 0..runs {
        let start = Instant::now();
        rows = f();
        t = t.min(start.elapsed().as_secs_f64());
    }
    (t, rows)
}

fn report(what: &str, bytes: usize, (secs, rows): (f64, usize)) {
    println!(
        "{what:<28} {:>9.1} ms {:>9.0} MB/s {:>12.0} rows/s  ({rows} rows)",
        secs * 1e3,
        bytes as f64 / secs / 1e6,
        rows as f64 / secs
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        // A fourth argument is another header for the same rows (`text,text,...` isolates
        // the scan from the decoding).
        Some("gen") => generate(args[2].parse().unwrap(), &args[3], args.get(4).map_or(HEADER, String::as_str)),
        Some("read") => {
            let bytes = fs::read(&args[2]).unwrap();
            let threads: usize = args.get(3).map_or_else(|| std::thread::available_parallelism().unwrap().get(), |t| t.parse().unwrap());
            let runs = if bytes.len() > 200_000_000 { 3 } else { 5 };
            let only = args.get(4).map(String::as_str);
            println!("{} ({:.1} MB)", args[2], bytes.len() as f64 / 1e6);
            if only.is_none_or(|o| o == "tcsv") {
                report("tcsv, 1 thread", bytes.len(), best(runs, || black_box(tcsv::read(bytes.clone()).unwrap()).rows()));
                report(
                    &format!("tcsv, {threads} threads"),
                    bytes.len(),
                    best(runs, || black_box(tcsv::read_parallel(bytes.clone(), threads).unwrap()).rows()),
                );
                let copy = best(runs, || black_box(bytes.clone()).len());
                report("(the input's copy alone)", bytes.len(), copy);
            }
            if only.is_none_or(|o| o == "csv") {
                report(
                    "csv crate, fields only",
                    bytes.len(),
                    best(runs, || {
                        let mut r = csv::ReaderBuilder::new().has_headers(true).from_reader(&bytes[..]);
                        let mut rec = csv::ByteRecord::new();
                        let mut rows = 0;
                        let mut sum = 0usize;
                        while r.read_byte_record(&mut rec).unwrap() {
                            sum += rec.len();
                            rows += 1;
                        }
                        black_box(sum);
                        rows
                    }),
                );
            }
        }
        Some("once") => {
            // For peak memory: the file read once, the bytes moved into the reader.
            let bytes = fs::read(&args[2]).unwrap();
            let threads: usize = args.get(3).map_or(1, |t| t.parse().unwrap());
            let doc = tcsv::read_parallel(bytes, threads).unwrap();
            println!("{} rows, {} problems", doc.rows(), doc.problems().len());
        }
        Some("loop") => {
            // For a sampling profiler: read on one thread, again and again.
            let bytes = fs::read(&args[2]).unwrap();
            let times: usize = args[3].parse().unwrap();
            let threads: usize = args.get(4).map_or(1, |t| t.parse().unwrap());
            for _ in 0..times {
                black_box(tcsv::read_parallel(bytes.clone(), threads).unwrap());
            }
        }
        Some("edit") => {
            // One cell set in each column, the file read as its extension says (a `.csv`
            // plain, by its separator): what an edit costs a table.
            let bytes = fs::read(&args[2]).unwrap();
            let threads = std::thread::available_parallelism().unwrap().get();
            let dialect = if args[2].ends_with(".tcsv") { tcsv::Dialect::Tcsv } else { tcsv::Dialect::Plain(tcsv::detect(&bytes)) };
            let doc = tcsv::read_as(bytes, dialect, threads).unwrap();
            println!("{} ({} rows, {dialect:?})", args[2], doc.rows());
            for c in 0..doc.stored() {
                let mut d = doc.clone();
                let row = doc.rows() / 2;
                let text = doc.text(row, c).into_owned();
                let t = Instant::now();
                d.set_cell(row, c, &text).unwrap();
                println!("{:<20} {:>8.2} ms", doc.columns()[c].name, t.elapsed().as_secs_f64() * 1e3);
            }
        }
        _ => eprintln!("tcsv-bench gen <rows> <file> | read <file> [threads] [tcsv|csv] | loop <file> <times> | edit <file>"),
    }
}
