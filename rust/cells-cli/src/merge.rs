//! `cells merge`: git's merge driver for `.cells` files (SPEC.md, Merging). Two versions
//! merge as what was written: each version's computed values are dropped first, so a
//! value that follows from the rows (a running balance below an inserted movement) never
//! conflicts, the three texts are merged line by line by git's own merge, and the merged
//! file's computed cells are computed and stored again. What still conflicts is what
//! people wrote: a cell, the header, the prelude.
//!
//!   .gitattributes   *.cells merge=cells
//!   .git/config      [merge "cells"] driver = cells merge %O %A %B

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::disk::Disk;

/// How the merge ended: clean, or with conflicts left marked in `ours`.
pub enum Merged {
    Clean,
    Conflicts(u32),
}

/// `base`, `ours` and `theirs` merged into `ours`, as git's driver contract asks.
pub fn merge(base: &Path, ours: &Path, theirs: &Path) -> Result<Merged, String> {
    let bare = |p: &Path| -> Result<String, String> {
        let bytes = fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let doc = cells::read_with(bytes, &mut Disk::new(p), 1).map_err(|e| format!("{} {e}", p.display()))?;
        Ok(doc.without_stored())
    };
    let dir = std::env::temp_dir().join(format!("cells-merge-{}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let at = |name: &str| dir.join(name);
    for (name, p) in [("base", base), ("ours", ours), ("theirs", theirs)] {
        fs::write(at(name), bare(p)?).map_err(|e| format!("{}: {e}", at(name).display()))?;
    }
    // git's own three-way merge of the written texts, into `ours`: its exit is the count of
    // conflicts, or below zero when it cannot run.
    let status = Command::new("git")
        .args(["merge-file", "-L", "ours", "-L", "base", "-L", "theirs"])
        .arg(at("ours"))
        .arg(at("base"))
        .arg(at("theirs"))
        .status()
        .map_err(|e| format!("git merge-file: {e}"))?;
    let merged = fs::read_to_string(at("ours")).map_err(|e| format!("{}: {e}", at("ours").display()))?;
    let _ = fs::remove_dir_all(&dir);
    let conflicts = match status.code() {
        Some(0) => 0,
        Some(n) if n > 0 => n as u32,
        _ => return Err(format!("git merge-file failed: {status}")),
    };
    if conflicts > 0 {
        // The conflicts as git marks them, for a person to resolve; the next write after
        // stores the computed values again.
        fs::write(ours, merged).map_err(|e| format!("{}: {e}", ours.display()))?;
        return Ok(Merged::Conflicts(conflicts));
    }
    let mut doc = cells::read_with(merged.into_bytes(), &mut Disk::new(ours), 1).map_err(|e| format!("{} {e}", ours.display()))?;
    doc.store_unstored();
    fs::write(ours, doc.write()).map_err(|e| format!("{}: {e}", ours.display()))?;
    Ok(Merged::Clean)
}
