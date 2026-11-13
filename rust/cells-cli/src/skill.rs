//! `cells skill`: the description (`describe.rs`) offered to the agents on this Mac, as
//! swift-utils' TerminalPrograms offers a program's (its README, "The skill"). Claude Code's
//! and Codex's base skill folders are where it goes, for whichever agent is here. Inside
//! Cells.app the skill is the app's own copy (`Contents/Resources/skill/SKILL.md`, rendered at
//! build from `cells skill show`) and installed as a link to it, so the app's update is the
//! skill's; a `cells` anywhere else writes a copy. An earlier link or a copy this program
//! wrote is replaced; any other folder there is the person's own and is left alone. Nothing
//! installs it unasked.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::describe;

/// Where the skill goes: `skills/cells` in Claude Code's and Codex's base homes
/// (`~/.claude`, `~/.codex`), each only where that agent is on this Mac. The base homes,
/// never `$CODEX_HOME` or `$CLAUDE_CONFIG_DIR`: those name one profile, and every profile
/// links into its base home, so a skill there reaches them all (TerminalPrograms' `Agent`).
fn targets() -> Vec<PathBuf> {
    let home = home();
    [home.join(".claude"), home.join(".codex")]
        .into_iter()
        .filter(|base| base.is_dir())
        .map(|base| base.join("skills").join(describe::NAME))
        .collect()
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME is set"))
}

/// A path as a person reads it, `~` for the home folder.
fn shown(path: &Path) -> String {
    match path.strip_prefix(home()) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// The skill the app carries, when this binary is the app's: `Contents/Resources/skill`
/// beside `Contents/Helpers/cells`, found through the binary's real path (a link on the
/// PATH points into the bundle).
fn bundled() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let dir = exe.parent()?.parent()?.join("Resources").join("skill");
    dir.join("SKILL.md").is_file().then_some(dir)
}

/// What a skill folder holds now.
enum Held {
    Absent,
    /// A link, to where it points.
    Link(PathBuf),
    /// A copy this program wrote (its `generated-by` names `cells`).
    Copy,
    /// A folder of the person's own.
    Theirs,
}

fn held(target: &Path) -> Held {
    let Ok(meta) = fs::symlink_metadata(target) else { return Held::Absent };
    if meta.file_type().is_symlink() {
        return Held::Link(fs::read_link(target).unwrap_or_else(|e| panic!("{}: a link that cannot be read: {e}", target.display())));
    }
    let ours =
        fs::read_to_string(target.join("SKILL.md")).is_ok_and(|text| text.contains(&format!("  generated-by: \"{} ", describe::NAME)));
    if ours { Held::Copy } else { Held::Theirs }
}

/// The skill there says what this program says now.
fn current(target: &Path) -> bool {
    fs::read_to_string(target.join("SKILL.md")).is_ok_and(|text| text == describe::skill())
}

/// `cells skill`: where it is installed and whether it is current. Exit 0 when every agent
/// here reads the current skill, 1 when one does not.
pub fn status() -> ExitCode {
    let targets = targets();
    if targets.is_empty() {
        println!("no agent on this Mac: neither ~/.claude nor ~/.codex is here");
        return ExitCode::from(1);
    }
    let mut all = true;
    for target in &targets {
        let fresh = current(target);
        let state = if fresh { "current" } else { "stale: cells skill install" };
        let line = match held(target) {
            Held::Absent => {
                all = false;
                "not installed: cells skill install".to_string()
            }
            Held::Link(to) => format!("linked to {}, {state}", shown(&to)),
            Held::Copy => format!("a copy, {state}"),
            Held::Theirs => {
                all = false;
                "a folder of its own, not this program's: left alone".to_string()
            }
        };
        all &= fresh;
        println!("{}: {line}", shown(target));
    }
    if all { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

/// `cells skill install`: the skill put where each agent here reads it, replacing an earlier
/// link or copy of this program's. Exit 2 when a folder of the person's own is in the way.
pub fn install() -> ExitCode {
    let targets = targets();
    if targets.is_empty() {
        eprintln!("cells skill: no agent on this Mac: neither ~/.claude nor ~/.codex is here");
        return ExitCode::from(2);
    }
    let bundled = bundled();
    let mut refused = false;
    for target in &targets {
        match held(target) {
            Held::Theirs => {
                eprintln!("cells skill: {} is a folder of its own, not this program's: left alone", shown(target));
                refused = true;
                continue;
            }
            Held::Link(_) => fs::remove_file(target).unwrap_or_else(|e| panic!("{}: {e}", target.display())),
            Held::Copy => fs::remove_dir_all(target).unwrap_or_else(|e| panic!("{}: {e}", target.display())),
            Held::Absent => {}
        }
        let parent = target.parent().expect("a skill folder has a parent");
        fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{}: {e}", parent.display()));
        match &bundled {
            Some(dir) => {
                std::os::unix::fs::symlink(dir, target).unwrap_or_else(|e| panic!("{}: {e}", target.display()));
                println!("{}: linked to {}", shown(target), shown(dir));
            }
            None => {
                fs::create_dir(target).unwrap_or_else(|e| panic!("{}: {e}", target.display()));
                fs::write(target.join("SKILL.md"), describe::skill()).unwrap_or_else(|e| panic!("{}: {e}", target.display()));
                println!("{}: written by {}", shown(target), describe::generated_by());
            }
        }
    }
    if refused { ExitCode::from(2) } else { ExitCode::SUCCESS }
}

/// `cells skill remove`: the link or copy taken out of each agent's folder; a folder of the
/// person's own is left alone.
pub fn remove() -> ExitCode {
    for target in targets() {
        let line = match held(&target) {
            Held::Absent => "not installed",
            Held::Link(_) => {
                fs::remove_file(&target).unwrap_or_else(|e| panic!("{}: {e}", target.display()));
                "removed"
            }
            Held::Copy => {
                fs::remove_dir_all(&target).unwrap_or_else(|e| panic!("{}: {e}", target.display()));
                "removed"
            }
            Held::Theirs => "a folder of its own, not this program's: left alone",
        };
        println!("{}: {line}", shown(&target));
    }
    ExitCode::SUCCESS
}
