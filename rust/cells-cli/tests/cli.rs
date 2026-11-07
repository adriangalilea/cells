//! The command line on a folder written for the test: what it prints and how it exits.

use std::fs;
use std::process::Command;

#[test]
fn check_and_fmt() {
    let dir = std::env::temp_dir().join(format!("cells-cli-{}", std::process::id()));
    fs::create_dir_all(dir.join("trip")).unwrap();
    fs::create_dir_all(dir.join(".cells")).unwrap();
    fs::write(dir.join("clean.cells"), "item:text!,cost:EUR\nfuel,84.20\n").unwrap();
    fs::write(dir.join("trip/money.cells"), "item:text!,cost:EUR>=0\nfuel,8x.50\nfuel,-1\n").unwrap();
    fs::write(dir.join("trip/broken.cells"), "item:EURO\nfuel\n").unwrap();
    fs::write(dir.join(".cells/ignored.cells"), "a:EURO\n").unwrap();
    let cells = env!("CARGO_BIN_EXE_cells");

    let out = Command::new(cells).arg("check").arg(&dir).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let shown = |p: &str| dir.join(p).display().to_string();
    assert_eq!(
        lines,
        [
            format!("{} column 1: item: no type or currency \"EURO\"", shown("trip/broken.cells")),
            format!("{} row 1, item: \"fuel\" is the key of row 2 too", shown("trip/money.cells")),
            format!("{} row 1, cost: \"8x.50\" is not a number", shown("trip/money.cells")),
            format!("{} row 2, item: \"fuel\" is the key of row 1 too", shown("trip/money.cells")),
            format!("{} row 2, cost: \"-1\" is below 0", shown("trip/money.cells")),
        ]
    );

    let clean = Command::new(cells).arg("check").arg(dir.join("clean.cells")).output().unwrap();
    assert_eq!(clean.status.code(), Some(0));
    assert!(clean.stdout.is_empty());

    // `fmt --check` says which files it would change; `fmt` changes them, and then nothing.
    let clean = dir.join("clean.cells");
    let check = Command::new(cells).args(["fmt", "--check"]).arg(&clean).output().unwrap();
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(String::from_utf8(check.stdout).unwrap(), format!("{}: not formatted (cells fmt)\n", clean.display()));
    let fmt = Command::new(cells).arg("fmt").arg(&clean).output().unwrap();
    assert_eq!(fmt.status.code(), Some(0));
    assert_eq!(fs::read_to_string(&clean).unwrap(), "item:text!, cost:EUR\nfuel,       84.20\n");
    let check = Command::new(cells).args(["fmt", "--check"]).arg(&clean).output().unwrap();
    assert_eq!(check.status.code(), Some(0));
    // `cells diff`, git's textconv, prints the file with no padding; `--align off` writes it so.
    let diff = Command::new(cells).arg("diff").arg(&clean).output().unwrap();
    assert_eq!(String::from_utf8(diff.stdout).unwrap(), "item:text!,cost:EUR\nfuel,84.20\n");
    let off = Command::new(cells).args(["fmt", "--align", "off"]).arg(&clean).output().unwrap();
    assert_eq!(off.status.code(), Some(0));
    assert_eq!(fs::read_to_string(&clean).unwrap(), "item:text!,cost:EUR\nfuel,84.20\n");

    let missing = Command::new(cells).arg("check").arg(dir.join("nowhere")).output().unwrap();
    assert_eq!(missing.status.code(), Some(2));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn imports_formulas_and_a_script() {
    let dir = std::env::temp_dir().join(format!("cells-cli-books-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("accounts.cells"), "account:text!\ncard\nhotel\n").unwrap();
    fs::write(
        dir.join("money.cells"),
        "---\nimport accounts from \"./accounts.cells\"\nrates = run every hour\n```ts\nexport default async ({ last }) => [{ day: \"2026-11-04\", rate: 0.5 }]\n```\n```cells\nday:date!,rate:num>0\n2026-11-03,0.25\n```\n---\nday:date,usd:USD,to:accounts.account,eur:EUR=usd * rates[day].rate\n2026-11-03,100,hotel,\n2026-11-04,10,bar,\n",
    )
    .unwrap();
    let cells = env!("CARGO_BIN_EXE_cells");
    let money = dir.join("money.cells");
    let shown = money.display().to_string();

    let check = Command::new(cells).arg("check").arg(&money).output().unwrap();
    assert_eq!(String::from_utf8(check.stdout).unwrap(), format!("{shown} row 2, to: \"bar\" is no key of accounts\n"));

    let run = Command::new(cells).arg("run").arg(&money).output().unwrap();
    assert_eq!(String::from_utf8(run.stdout).unwrap(), "rates: run 1 ok, 1 added, 0 changed\n");
    let text = fs::read_to_string(&money).unwrap();
    assert!(text.contains("day:date!,rate:num>0,run:int?\n2026-11-03,0.25,\n2026-11-04,0.5,1\n```\n```cells runs\n"), "{text}");

    let show = Command::new(cells).arg("show").arg(&money).output().unwrap();
    assert_eq!(String::from_utf8(show.stdout).unwrap(), "day\tusd\tto\teur\n2026-11-03\t100\thotel\t25.00\n2026-11-04\t10\tbar\t5.00\n");

    // A computed amount at its currency's digits, its column declared in the prelude too.
    let vat = dir.join("vat.cells");
    fs::write(&vat, "---\nvat: USD = usd * 21 / 100\n---\nusd:USD>0,vat,total:USD=usd + vat\n200,42.00=,242.00=\n38.40,,\n").unwrap();
    let show = Command::new(cells).arg("show").arg(&vat).output().unwrap();
    assert_eq!(String::from_utf8(show.stdout).unwrap(), "usd\tvat\ttotal\n200\t42.00\t242.00\n38.40\t8.06\t46.46\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// `cells sql` over `tests/books`: what it prints, on stdout and stderr, and how it exits.
fn sql(paths: &[&std::path::Path], query: &str) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_cells")).arg("sql").args(paths).arg(query).output().unwrap();
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
    let dir = std::env::temp_dir().join(format!("cells-cli-sql-{}", std::process::id()));
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
