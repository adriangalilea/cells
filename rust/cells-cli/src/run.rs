//! `cells run <file> [script]`: each script of a file (or the one named) run once by the
//! engine, as its person allowed it, and what it returned written under it there
//! (`cells::run`, SPEC.md, A table from a script). The engine runs it so every run is one
//! kind of event: logged as a root of history, what it reached stored after it, whichever
//! surface asked.

use std::fs;
use std::path::Path;

use cells_daemon::Engine;
use cells_daemon::log::Outcome;

use cells_disk::Disk;

/// What each script's run did, a line each, and whether every one succeeded.
pub fn run(path: &Path, only: Option<&str>) -> Result<(Vec<String>, bool), String> {
    let file = fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let bytes = fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let doc = cells::read_with(bytes, &mut Disk::new(&file), 1).map_err(|e| format!("{} {e}", path.display()))?;
    let names: Vec<String> = doc.scripts().iter().map(|s| s.name.clone()).filter(|n| only.is_none_or(|o| o == n)).collect();
    if names.is_empty() {
        return Err(match only {
            Some(o) => format!("{} has no script {o}", path.display()),
            None => format!("{} has no script", path.display()),
        });
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let (engine, _) = Engine::connect_or_start("cells run", &exe)?;
    engine.serve(file.parent().expect("a file is in a folder"))?;
    let mut said = Vec::new();
    let mut all_ok = true;
    for name in names {
        match engine.run(&file, &name) {
            Ok((run, Outcome::Ok { added, changed })) => said.push(format!("{name}: run {run} ok, {added} added, {changed} changed")),
            Ok((run, Outcome::Failed { why })) => {
                all_ok = false;
                said.push(format!("{name}: run {run} failed: {why}"));
            }
            Err(why) => {
                all_ok = false;
                said.push(format!("{name}: not run: {why}"));
            }
        }
    }
    Ok((said, all_ok))
}
