//! The command line on a folder written for the test: what it prints and how it exits.

use std::fs;
use std::process::Command;

#[test]
fn check_and_fmt() {
    let dir = std::env::temp_dir().join(format!("tcsv-cli-{}", std::process::id()));
    fs::create_dir_all(dir.join("trip")).unwrap();
    fs::create_dir_all(dir.join(".tcsv")).unwrap();
    fs::write(dir.join("clean.tcsv"), "item:text!,cost:EUR\nfuel,84.20\n").unwrap();
    fs::write(dir.join("trip/money.tcsv"), "item:text!,cost:EUR>=0\nfuel,8x.50\nfuel,-1\n").unwrap();
    fs::write(dir.join("trip/broken.tcsv"), "item:EURO\nfuel\n").unwrap();
    fs::write(dir.join(".tcsv/ignored.tcsv"), "a:EURO\n").unwrap();
    let tcsv = env!("CARGO_BIN_EXE_tcsv");

    let out = Command::new(tcsv).arg("check").arg(&dir).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let shown = |p: &str| dir.join(p).display().to_string();
    assert_eq!(
        lines,
        [
            format!("{} column 1: no type or currency \"EURO\"", shown("trip/broken.tcsv")),
            format!("{} row 0, item: \"fuel\" is the key of row 1 too", shown("trip/money.tcsv")),
            format!("{} row 0, cost: \"8x.50\" is not a number", shown("trip/money.tcsv")),
            format!("{} row 1, item: \"fuel\" is the key of row 0 too", shown("trip/money.tcsv")),
            format!("{} row 1, cost: \"-1\" is below 0", shown("trip/money.tcsv")),
        ]
    );

    let clean = Command::new(tcsv).arg("check").arg(dir.join("clean.tcsv")).output().unwrap();
    assert_eq!(clean.status.code(), Some(0));
    assert!(clean.stdout.is_empty());

    let fmt = Command::new(tcsv).args(["fmt", "--check"]).arg(dir.join("clean.tcsv")).arg(dir.join("trip/money.tcsv")).output().unwrap();
    assert_eq!(fmt.status.code(), Some(0));

    let missing = Command::new(tcsv).arg("check").arg(dir.join("nowhere")).output().unwrap();
    assert_eq!(missing.status.code(), Some(2));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn imports_formulas_and_a_script() {
    let dir = std::env::temp_dir().join(format!("tcsv-cli-books-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("accounts.tcsv"), "account:text!\ncard\nhotel\n").unwrap();
    fs::write(
        dir.join("money.tcsv"),
        "---\nimport accounts from \"./accounts.tcsv\"\nrates = run every hour\n```ts\nexport default async ({ last }) => [{ day: \"2026-11-04\", rate: 0.5 }]\n```\n```tcsv\nday:date!,rate:num>0\n2026-11-03,0.25\n```\n---\nday:date,usd:USD,to:accounts.account,eur:EUR=usd * rates[day].rate\n2026-11-03,100,hotel,\n2026-11-04,10,bar,\n",
    )
    .unwrap();
    let tcsv = env!("CARGO_BIN_EXE_tcsv");
    let money = dir.join("money.tcsv");
    let shown = money.display().to_string();

    let check = Command::new(tcsv).arg("check").arg(&money).output().unwrap();
    assert_eq!(String::from_utf8(check.stdout).unwrap(), format!("{shown} row 1, to: \"bar\" is no key of accounts\n"));

    let run = Command::new(tcsv).arg("run").arg(&money).output().unwrap();
    assert_eq!(String::from_utf8(run.stdout).unwrap(), "rates: run 1 ok, 1 added, 0 changed\n");
    let text = fs::read_to_string(&money).unwrap();
    assert!(text.contains("day:date!,rate:num>0,run:int?\n2026-11-03,0.25,\n2026-11-04,0.5,1\n```\n```tcsv runs\n"), "{text}");

    let show = Command::new(tcsv).arg("show").arg(&money).output().unwrap();
    assert_eq!(String::from_utf8(show.stdout).unwrap(), "day\tusd\tto\teur\n2026-11-03\t100\thotel\t25\n2026-11-04\t10\tbar\t5\n");
    fs::remove_dir_all(&dir).unwrap();
}
