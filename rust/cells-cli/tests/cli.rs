//! The command line on a folder written for the test: what it prints and how it exits.

use std::fs;
use std::path::PathBuf;
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
        "---\nimport accounts from \"./accounts.cells\"\nrates = run every day at 03:00 in UTC\n```ts\nexport default async ({ last }) => [{ day: \"2026-11-04\", rate: 0.5 }]\n```\n```cells\nday:date!,rate:num>0\n2026-11-03,0.25\n```\n---\nday:date,usd:USD,to:accounts.account,eur:EUR=usd * rates[day].rate\n2026-11-03,100,hotel,\n2026-11-04,10,bar,\n",
    )
    .unwrap();
    let cells = env!("CARGO_BIN_EXE_cells");
    let money = dir.join("money.cells");
    let shown = money.display().to_string();

    let check = Command::new(cells).arg("check").arg(&money).output().unwrap();
    assert_eq!(String::from_utf8(check.stdout).unwrap(), format!("{shown} row 2, to: \"bar\" is no key of accounts\n"));

    // A script runs only at a version its person allowed: not yet, so the engine does not run
    // it. The engine and its approvals are this test's own (`HOME` a folder of its own, short
    // enough for the engine's socket path), never the person's.
    let home = PathBuf::from(format!("/tmp/cells-cli-{}", std::process::id()));
    let run = Command::new(cells).env("HOME", &home).arg("run").arg(&money).output().unwrap();
    assert_eq!(run.status.code(), Some(1));
    let said = String::from_utf8(run.stdout).unwrap();
    assert!(said.starts_with("rates: not run: rates is not allowed on this Mac as it is written"), "{said}");
    // allowed at this version, as the engine keeps an approval
    let doc = cells::read(fs::read(&money).unwrap()).unwrap();
    let approval = cells_daemon::allowed::Approval {
        file: fs::canonicalize(&money).unwrap(),
        script: "rates".into(),
        version: cells_daemon::allowed::version(&doc.scripts()[0]),
        by: "cli".into(),
        at: 1,
    };
    let kept = home.join("Library/Application Support/Cells/allowed.jsonl");
    fs::create_dir_all(kept.parent().unwrap()).unwrap();
    fs::write(&kept, format!("{}\n", cells_daemon::allowed::line(&approval))).unwrap();
    let run = Command::new(cells).env("HOME", &home).arg("run").arg(&money).output().unwrap();
    assert_eq!(String::from_utf8(run.stdout).unwrap(), "rates: run 1 ok, 1 added, 0 changed\n");
    let text = fs::read_to_string(&money).unwrap();
    assert!(text.contains("day:date!,rate:num>0,run:int?\n2026-11-03,0.25,\n2026-11-04,0.5,1\n```\n```cells runs\n"), "{text}");
    // the run is history's: a root, by who asked
    let history = Command::new(cells).env("HOME", &home).arg("history").arg(&money).output().unwrap();
    let history = String::from_utf8(history.stdout).unwrap();
    assert!(history.contains("money.cells: rates run 1 by cells run, 1 added, 0 changed"), "{history}");
    // the engine this test started, stopped: `cells serve` names the one running
    let serve = Command::new(cells).env("HOME", &home).arg("serve").output().unwrap();
    let said = String::from_utf8(serve.stderr).unwrap();
    let pid: i32 =
        said.split("pid ").nth(1).and_then(|s| s.split(')').next()).and_then(|p| p.parse().ok()).unwrap_or_else(|| panic!("{said}"));
    Command::new("kill").arg(pid.to_string()).status().unwrap();
    fs::remove_dir_all(&home).unwrap();

    let show = Command::new(cells).arg("show").arg(&money).output().unwrap();
    assert_eq!(String::from_utf8(show.stdout).unwrap(), "day\tusd\tto\teur\n2026-11-03\t100\thotel\t25.00\n2026-11-04\t10\tbar\t5.00\n");

    // A computed amount at its currency's digits, its column declared in the prelude too.
    let vat = dir.join("vat.cells");
    fs::write(&vat, "---\nvat: USD = usd * 21 / 100\n---\nusd:USD>0,vat,total:USD=usd + vat\n200,42.00=,242.00=\n38.40,,\n").unwrap();
    let show = Command::new(cells).arg("show").arg(&vat).output().unwrap();
    assert_eq!(String::from_utf8(show.stdout).unwrap(), "usd\tvat\ttotal\n200\t42.00\t242.00\n38.40\t8.06\t46.46\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// `cells sql` is `cells-sql`'s (its own tests are there): handed over with its arguments,
/// its exit its own.
#[test]
fn sql_is_handed_to_cells_sql() {
    let out = Command::new(env!("CARGO_BIN_EXE_cells")).args(["sql", "/nowhere", "SELECT 1"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.starts_with("cells sql: "), "{err}");
}
