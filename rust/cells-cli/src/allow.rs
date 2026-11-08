//! `cells allow <file> [script]`: each script of the file not allowed to run as it is
//! written (or the one named) shown to its person whole, its name, schedule, language and
//! every line of its code, and allowed at that version once they say yes (`allowed`, in the
//! engine). It asks a person, so it runs only in a terminal: nothing piped or scripted
//! allows a script on anyone's behalf.

use std::fs;
use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;

use cells_daemon::Engine;
use cells_daemon::allowed::{Allowed, version};

use cells_disk::Disk;

/// What was allowed and what was not, a line each.
pub fn allow(path: &Path, only: Option<&str>) -> Result<Vec<String>, String> {
    if !std::io::stdin().is_terminal() {
        return Err("allowing a script asks its person: run `cells allow` in a terminal".into());
    }
    let file = fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let bytes = fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let doc = cells::read_with(bytes, &mut Disk::new(&file), 1).map_err(|e| format!("{} {e}", file.display()))?;
    let allowed = Allowed::read();
    let asked: Vec<&cells::Script> = match only {
        Some(name) => {
            vec![doc.scripts().iter().find(|s| s.name == name).ok_or_else(|| format!("{} has no script {name}", path.display()))?]
        }
        None => doc.scripts().iter().filter(|s| !allowed.allows(&file, s)).collect(),
    };
    if asked.is_empty() {
        return Ok(vec![format!("{}: every script may run as it is written", path.display())]);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let (engine, _) = Engine::connect_or_start("cli", &exe)?;
    let mut said = Vec::new();
    let stdin = std::io::stdin();
    for script in asked {
        let mut out = std::io::stdout();
        writeln!(out, "\n{} = run {}   ({}, {})", script.name, script.schedule.spelled, script.language, path.display())
            .map_err(|e| e.to_string())?;
        for (n, line) in script.code.lines().enumerate() {
            writeln!(out, "{:>4}  {line}", n + 1).map_err(|e| e.to_string())?;
        }
        write!(out, "\nAllow {} to run, as written above? [y/N] ", script.name).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        let mut answer = String::new();
        stdin.lock().read_line(&mut answer).map_err(|e| e.to_string())?;
        if matches!(answer.trim(), "y" | "Y" | "yes") {
            engine.allow(&file, &script.name, &version(script))?;
            said.push(format!("{}: allowed, as written", script.name));
        } else {
            said.push(format!("{}: not allowed", script.name));
        }
    }
    Ok(said)
}
