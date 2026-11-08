//! Which scripts may run. A Cells file is data and formulas, which reach no disk and no
//! network; a script is a program, and a file from anyone (sent, cloned, copied in) may
//! carry one. So a script runs only once its person allowed that very version of it: its
//! file, its name and the SHA-256 of its language and code. One character changed is
//! another version, never allowed until asked again.
//!
//! The approvals are this Mac's and this person's, never the folder's: `allowed.jsonl` in
//! the engine's own folder, so a folder copied or cloned carries none. The engine is its
//! one writer (`ToEngine::Allow`, from the command line, an editor or the app, after the
//! person saw the code); every client reads it as it is.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::wire;

/// A script's version: the SHA-256 of its language, a line break and its code.
pub fn version(script: &cells::Script) -> String {
    let said = format!("{}\n{}", script.language, script.code);
    cells::sha256::hex(said.as_bytes())
}

/// Where the approvals are kept.
pub fn path() -> PathBuf {
    wire::folder().join("allowed.jsonl")
}

/// One approval, as a line of `allowed.jsonl`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub file: PathBuf,
    pub script: String,
    pub version: String,
    /// Who asked the person: `cli`, `lsp`, `app`.
    pub by: String,
    /// Milliseconds since the Unix epoch.
    pub at: u64,
}

/// Every approval kept, by file, script and version.
#[derive(Default)]
pub struct Allowed {
    kept: BTreeSet<(PathBuf, String, String)>,
}

impl Allowed {
    /// The approvals as they are now; none when nothing was ever allowed.
    pub fn read() -> Allowed {
        let Ok(text) = std::fs::read_to_string(path()) else { return Allowed::default() };
        let kept = text
            .lines()
            .map(|l| {
                serde_json::from_str::<Approval>(l).unwrap_or_else(|e| panic!("{}: a line is no approval ({e}): {l}", path().display()))
            })
            .map(|a| (a.file, a.script, a.version))
            .collect();
        Allowed { kept }
    }

    /// Whether `script` of the file at `file` (canonical) may run as it is written now.
    pub fn allows(&self, file: &Path, script: &cells::Script) -> bool {
        self.kept.contains(&(file.to_path_buf(), script.name.clone(), version(script)))
    }
}

/// `approval` as its line of `allowed.jsonl`, which the engine alone appends.
pub fn line(approval: &Approval) -> String {
    serde_json::to_string(approval).expect("an approval serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str =
        "---\nrates = run every hour\n```ts\nexport default async () => []\n```\n```cells\nday:date!,rate:num\n```\n---\na\n1\n";

    #[test]
    fn a_version_is_the_code_and_one_character_is_another() {
        let doc = cells::read(FILE).unwrap();
        let script = &doc.scripts()[0];
        // pinned: every client computes the same (swift-utils' `Cells.Approvals.version`)
        // `shasum -a 256` of "ts\nexport default async () => []\n": a script's code ends with
        // its line break
        assert_eq!(version(script), "417af8deda266572d175f2fcc48b7429b87322668d511232a25569815a21010d");
        let changed = cells::read(FILE.replace("[]", "[ ]")).unwrap();
        assert_ne!(version(script), version(&changed.scripts()[0]));
        let mut allowed = Allowed::default();
        allowed.kept.insert((PathBuf::from("/w/money.cells"), "rates".into(), version(script)));
        assert!(allowed.allows(Path::new("/w/money.cells"), script));
        assert!(!allowed.allows(Path::new("/w/money.cells"), &changed.scripts()[0]), "another version");
        assert!(!allowed.allows(Path::new("/w/other.cells"), script), "another file");
    }
}
