//! `cells sql` over `tests/books`: what it prints, on stdout and stderr, and how it exits.

use std::fs;
use std::process::Command;

fn sql(paths: &[&std::path::Path], query: &str) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_cells-sql")).args(paths).arg(query).output().unwrap();
    (out.status.code(), String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

#[test]
fn sql_over_the_books() {
    let books = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/books");
    let ask = |query: &str| {
        let (code, out, err) = sql(&[&books], query);
        assert_eq!((code, err.as_str()), (Some(0), ""), "{query}");
        out
    };
    // README.md's query: `money` is the three money.cells, one table, summed exactly.
    assert_eq!(
        ask("SELECT account_to, SUM(amount) FROM money GROUP BY account_to ORDER BY account_to"),
        "account_to\tsum(amount)\nassets:bank:main\t120.00\nexpenses:home:power\t122.40\nexpenses:home:rent\t1900.00\nexpenses:studio:paper\t0.30\nexpenses:travel:ferry\t50.00\nexpenses:travel:food\t12.60\n"
    );
    // SPEC.md's: dates are dates, and the 2025 power bill is left out.
    assert_eq!(
        ask(
            "SELECT vendor, SUM(amount) FROM money WHERE account_to LIKE 'expenses:home:%' AND YEAR(date) = 2026 GROUP BY vendor ORDER BY vendor"
        ),
        "vendor\tsum(amount)\nlandlord\t1900.00\npower\t64.30\n"
    );
    // A join through the reference, a choice filtered, a computed column summed.
    assert_eq!(
        ask(
            "SELECT a.kind, SUM(m.amount) AS total FROM money m JOIN accounts a ON m.account_to = a.account WHERE a.kind = 'expenses' GROUP BY a.kind"
        ),
        "kind\ttotal\nexpenses\t2085.30\n"
    );
    assert_eq!(ask("SELECT SUM(vat) AS vat FROM money WHERE date >= DATE '2026-07-12'"), "vat\n16.86\n");
    assert_eq!(ask("SELECT 0.10 + 0.20 = (SELECT SUM(amount) FROM money WHERE vendor = 'paper') AS exact"), "exact\ntrue\n");
    // Each row says its file; a column a file does not have is null there.
    assert_eq!(
        ask("SELECT file, COUNT(*) AS n, COUNT(vat) AS vat FROM money GROUP BY file ORDER BY file"),
        format!("file\tn\tvat\n{0}/home/money.cells\t4\t4\n{0}/studio/money.cells\t3\t0\n{0}/travel/money.cells\t2\t2\n", books.display())
    );
    // The types, as the table in sql.rs maps them.
    assert_eq!(
        ask("SELECT typeof(amount), typeof(date), typeof(tax_rate), typeof(vat), typeof(account_to) FROM money LIMIT 1"),
        "typeof(amount)\ttypeof(date)\ttypeof(tax_rate)\ttypeof(vat)\ttypeof(account_to)\nDECIMAL(38,2)\tDATE\tDECIMAL(38,0)\tDECIMAL(38,2)\tVARCHAR\n"
    );
    assert_eq!(ask("SELECT DISTINCT typeof(kind) FROM accounts"), "typeof(kind)\nENUM('assets', 'liabilities', 'expenses', 'income')\n");
    // A file asked alone brings the files it imports.
    let (_, out, _) = sql(&[&books.join("travel/money.cells")], "SELECT COUNT(*) AS n FROM accounts");
    assert_eq!(out, "n\n9\n");

    // A broken cell is null, and said on stderr; a query that ran over it exits 1.
    let dir = std::env::temp_dir().join(format!("cells-sql-{}", std::process::id()));
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    fs::write(
        dir.join("a/money.cells"),
        "item:text!,cost:EUR,on:date,mode:ferry|bus\nferry,5O.00,2026-08-17,ferry\nbus,3.00,2026-08-18,bus\n",
    )
    .unwrap();
    let (code, out, err) = sql(&[&dir.join("a")], "SELECT item, cost, mode FROM money WHERE mode = 'ferry' OR cost IS NOT NULL");
    assert_eq!(code, Some(1));
    assert_eq!(out, "item\tcost\tmode\nferry\t\tferry\nbus\t3.00\tbus\n");
    assert_eq!(err, format!("cells sql: 1 problem in 1 file read, a broken cell read as null: cells check {}\n", dir.join("a").display()));
    // Two files of one name whose column holds two currencies: never one column.
    fs::write(dir.join("b/money.cells"), "item:text!,cost:USD\ntaxi,12.00\n").unwrap();
    let (code, out, err) = sql(&[&dir], "SELECT * FROM money");
    assert_eq!((code, out.as_str()), (Some(2), ""));
    let shown = |p: &str| dir.join(p).display().to_string();
    assert_eq!(err, format!("cells sql: money.cost: EUR in {} and USD in {}\n", shown("a/money.cells"), shown("b/money.cells")));
    // A query DuckDB refuses cannot run.
    let (code, _, err) = sql(&[&dir.join("a")], "SELEC 1");
    assert_eq!(code, Some(2));
    assert!(err.starts_with("cells sql: Parser Error"), "{err}");
    fs::remove_dir_all(&dir).unwrap();
}
