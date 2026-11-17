//! The folders macOS keeps from a process until its person allows them (System Settings ›
//! Privacy & Security › Files & Folders): the home folder's Desktop, Documents and Downloads,
//! and every volume but the startup disk's. The engine never touches one it is not allowed:
//! no walk into it, no event under it followed, no file there read, so a hosted engine
//! starting at login asks nobody anything. What it reaches is said to it: by its host at its
//! start, read in its own process (true at launch), and afterwards by a client over its socket
//! (`ToEngine::Reach`), Cells.app reading each switch fresh; a folder that becomes reached is
//! walked then. `cells engine` run from a shell reaches every one: the terminal's own access
//! applies, and the person typed it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A folder macOS guards, by the switch that guards it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protected {
    Desktop,
    Documents,
    Downloads,
    /// A volume of the Mac's own other than its startup disk: a USB stick, an external disk.
    Removable,
    /// A volume the network serves.
    Network,
}

impl Protected {
    pub const ALL: [Protected; 5] =
        [Protected::Desktop, Protected::Documents, Protected::Downloads, Protected::Removable, Protected::Network];

    /// Its word, as the wire and the trace say it.
    pub fn word(self) -> &'static str {
        match self {
            Protected::Desktop => "desktop",
            Protected::Documents => "documents",
            Protected::Downloads => "downloads",
            Protected::Removable => "removable",
            Protected::Network => "network",
        }
    }

    /// The protected folder `word` names (`word`'s inverse).
    pub fn named(word: &str) -> Result<Protected, String> {
        Protected::ALL.into_iter().find(|p| p.word() == word).ok_or_else(|| format!("no protected folder is said {word:?}"))
    }

    /// The folder it is in `home`, for the three that are one there.
    pub fn in_home(self, home: &Path) -> Option<PathBuf> {
        match self {
            Protected::Desktop => Some(home.join("Desktop")),
            Protected::Documents => Some(home.join("Documents")),
            Protected::Downloads => Some(home.join("Downloads")),
            Protected::Removable | Protected::Network => None,
        }
    }
}

/// Which of the home folder's protected folders `path` is in, by its words alone.
pub fn in_home(home: &Path, path: &Path) -> Option<Protected> {
    Protected::ALL.into_iter().find(|p| p.in_home(home).is_some_and(|f| path.starts_with(f)))
}

/// The protected folders the engine is allowed into.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reach(BTreeSet<Protected>);

impl Reach {
    /// Every protected folder: `cells engine` from a shell.
    pub fn all() -> Reach {
        Reach(Protected::ALL.into_iter().collect())
    }

    pub fn of(allowed: impl IntoIterator<Item = Protected>) -> Reach {
        Reach(allowed.into_iter().collect())
    }

    pub fn allows(&self, folder: Protected) -> bool {
        self.0.contains(&folder)
    }

    /// The folders allowed, in order.
    pub fn allowed(&self) -> impl Iterator<Item = Protected> + '_ {
        self.0.iter().copied()
    }

    /// The reach a host says, as `cells_engine.h` writes it: a JSON list of words,
    /// `["desktop","documents"]`.
    pub fn parse(json: &str) -> Result<Reach, String> {
        let words: Vec<Protected> = serde_json::from_str(json).map_err(|e| format!("no reach: {e}: {json}"))?;
        Ok(Reach::of(words))
    }

    /// As the trace says it: `desktop documents`, or `none`.
    pub fn said(&self) -> String {
        if self.0.is_empty() {
            return "none".into();
        }
        self.0.iter().map(|p| p.word()).collect::<Vec<_>>().join(" ")
    }
}

/// The protected folder a mount at `at` is, from the kernel's mount table: none on the
/// startup disk (`/`, `/System/Volumes/…`); a network volume when the network serves it, any
/// other under `/Volumes` removable.
pub fn volume(at: &str, local: bool) -> Option<Protected> {
    if !at.starts_with("/Volumes/") {
        return None;
    }
    Some(if local { Protected::Removable } else { Protected::Network })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_in_a_home_folder_by_its_words() {
        let home = Path::new("/h");
        assert_eq!(in_home(home, Path::new("/h/Documents/a.cells")), Some(Protected::Documents));
        assert_eq!(in_home(home, Path::new("/h/Documents")), Some(Protected::Documents));
        assert_eq!(in_home(home, Path::new("/h/Desktop/w/a.cells")), Some(Protected::Desktop));
        assert_eq!(in_home(home, Path::new("/h/DocumentsOld/a.cells")), None);
        assert_eq!(in_home(home, Path::new("/h/Music/a.cells")), None);
    }

    #[test]
    fn a_reach_is_said_in_words() {
        let reach = Reach::parse(r#"["documents","desktop"]"#).unwrap();
        assert!(reach.allows(Protected::Desktop) && !reach.allows(Protected::Downloads));
        assert_eq!(reach.said(), "desktop documents");
        assert_eq!(Reach::default().said(), "none");
        assert!(Reach::parse(r#"["attic"]"#).is_err());
        assert_eq!(volume("/Volumes/USB", true), Some(Protected::Removable));
        assert_eq!(volume("/Volumes/share", false), Some(Protected::Network));
        assert_eq!(volume("/", true), None);
        assert_eq!(volume("/System/Volumes/Data", true), None);
    }
}
