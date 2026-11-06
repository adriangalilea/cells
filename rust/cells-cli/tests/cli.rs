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

    let fmt = Command::new(cells).args(["fmt", "--check"]).arg(dir.join("clean.cells")).arg(dir.join("trip/money.cells")).output().unwrap();
    assert_eq!(fmt.status.code(), Some(0));

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
    assert_eq!(String::from_utf8(show.stdout).unwrap(), "day\tusd\tto\teur\n2026-11-03\t100\thotel\t25\n2026-11-04\t10\tbar\t5\n");
    fs::remove_dir_all(&dir).unwrap();
}
