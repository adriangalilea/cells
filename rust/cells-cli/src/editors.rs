//! `cells editors install helix`: an editor set up for `.cells` from this binary alone.
//!
//! The grammar's source and the editor's queries are carried inside `cells`, so the parser
//! an editor builds, the queries it reads and the server it runs (`cells lsp`) are always
//! one version. The grammar is written to `<data>/cells/tree-sitter` (`$XDG_DATA_HOME`, else
//! `~/.local/share`), the queries where the editor reads them; an editor's own config is
//! only added to, never rewritten: a `cells` language it already declares is kept as it is.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The grammar as a C compiler builds it: `src/` with its headers.
const GRAMMAR: &[(&str, &[u8])] = &[
    ("src/parser.c", include_bytes!("../../../tree-sitter/src/parser.c")),
    ("src/scanner.c", include_bytes!("../../../tree-sitter/src/scanner.c")),
    ("src/tree_sitter/alloc.h", include_bytes!("../../../tree-sitter/src/tree_sitter/alloc.h")),
    ("src/tree_sitter/array.h", include_bytes!("../../../tree-sitter/src/tree_sitter/array.h")),
    ("src/tree_sitter/parser.h", include_bytes!("../../../tree-sitter/src/tree_sitter/parser.h")),
];

/// Helix's queries, as `runtime/queries/cells` holds them.
const HELIX_QUERIES: &[(&str, &[u8])] = &[
    ("highlights.scm", include_bytes!("../../../tree-sitter/queries/helix/highlights.scm")),
    ("indents.scm", include_bytes!("../../../tree-sitter/queries/helix/indents.scm")),
    ("injections.scm", include_bytes!("../../../tree-sitter/queries/helix/injections.scm")),
    ("rainbows.scm", include_bytes!("../../../tree-sitter/queries/helix/rainbows.scm")),
    ("tags.scm", include_bytes!("../../../tree-sitter/queries/helix/tags.scm")),
    ("textobjects.scm", include_bytes!("../../../tree-sitter/queries/helix/textobjects.scm")),
];

pub const EDITORS: &[&str] = &["helix"];

pub fn install(editor: &str) -> Result<Vec<String>, String> {
    match editor {
        "helix" => helix(),
        other => Err(format!("no setup for {other}: cells editors install {}", EDITORS.join("|"))),
    }
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from).ok_or_else(|| "HOME is not set".to_owned())
}

/// An XDG base directory: its variable when set to an absolute path, else `~/<fallback>`.
fn xdg(var: &str, fallback: &str) -> Result<PathBuf, String> {
    match std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()) {
        Some(p) => Ok(p),
        None => Ok(home()?.join(fallback)),
    }
}

fn write_all(root: &Path, files: &[(&str, &[u8])]) -> Result<(), String> {
    for (name, bytes) in files {
        let path = root.join(name);
        fs::create_dir_all(path.parent().expect("a file under the root")).map_err(|e| format!("{}: {e}", path.display()))?;
        fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// When the grammar's parser source was last written.
fn grammar_written(grammar: &Path) -> std::time::SystemTime {
    fs::metadata(grammar.join("src/parser.c")).and_then(|m| m.modified()).expect("the parser source, written above")
}

/// Whether `cells` on PATH is a command a shell finds, so an editor's `command = "cells"` runs.
fn on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join("cells")).find(|p| p.is_file())
}

fn helix() -> Result<Vec<String>, String> {
    let mut said = Vec::new();
    let grammar = xdg("XDG_DATA_HOME", ".local/share")?.join("cells/tree-sitter");
    write_all(&grammar, GRAMMAR)?;
    said.push(format!("grammar written to {}", grammar.display()));

    let config = xdg("XDG_CONFIG_HOME", ".config")?.join("helix");
    let queries = config.join("runtime/queries/cells");
    match fs::symlink_metadata(&queries) {
        Ok(m) if m.file_type().is_symlink() => {
            let to = fs::read_link(&queries).map_err(|e| format!("{}: {e}", queries.display()))?;
            said.push(format!("queries kept: {} is a link to {}", queries.display(), to.display()));
        }
        _ => {
            write_all(&queries, HELIX_QUERIES)?;
            said.push(format!("queries written to {}", queries.display()));
        }
    }

    let languages = config.join("languages.toml");
    let text = match fs::read_to_string(&languages) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", languages.display())),
    };
    let kept = text.lines().any(|l| l.split('#').next().unwrap_or("").replace(' ', "") == "name=\"cells\"");
    if kept {
        said.push(format!("{} already declares cells: kept as it is", languages.display()));
    } else {
        let block = format!(
            "\n# Cells (github.com/adriangalilea/cells), added by `cells editors install helix`.\n\
             [[language]]\n\
             name = \"cells\"\n\
             scope = \"source.cells\"\n\
             file-types = [\"cells\"]\n\
             comment-token = \"//\"\n\
             injection-regex = \"^cells$\"\n\
             roots = []\n\
             language-servers = [\"cells\"]\n\
             auto-format = true\n\
             \n\
             [[grammar]]\n\
             name = \"cells\"\n\
             source = {{ path = {:?} }}\n\
             \n\
             [language-server.cells]\n\
             command = \"cells\"\n\
             args = [\"lsp\"]\n",
            grammar.display().to_string()
        );
        let mut out = text.clone();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&block);
        fs::create_dir_all(&config).map_err(|e| format!("{}: {e}", config.display()))?;
        fs::write(&languages, out).map_err(|e| format!("{}: {e}", languages.display()))?;
        said.push(format!("cells added to {}", languages.display()));
    }

    // Helix builds every grammar it declares; a parser older than its queries drops the
    // highlights whole, so it is built now, from what was just written. Helix links into
    // `runtime/grammars` without making it, and exits 0 whatever failed, so the folder is
    // made first and the parser's file is what says it was built.
    let built = config.join("runtime/grammars");
    fs::create_dir_all(&built).map_err(|e| format!("{}: {e}", built.display()))?;
    let parser = built.join(if cfg!(target_os = "macos") { "cells.dylib" } else { "cells.so" });
    match Command::new("hx").args(["--grammar", "build"]).output() {
        Ok(o) => {
            // built from our source: newer than it; from a source the config already named
            // (kept): there at all
            let after = fs::metadata(&parser).and_then(|m| m.modified()).ok();
            let fresh = match after {
                None => false,
                Some(t) => kept || t >= grammar_written(&grammar),
            };
            if !fresh {
                let log = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
                let ours: Vec<&str> = log.lines().skip_while(|l| !l.contains(" cells ")).take(6).collect();
                return Err(format!("hx --grammar build did not build {}:\n{}", parser.display(), ours.join("\n")));
            }
            said.push(format!("parser built: {}", parser.display()));
        }
        Err(_) => said.push("hx is not on PATH: run `hx --grammar build` where it is".to_owned()),
    }
    match on_path() {
        Some(p) => said.push(format!("language server: {} lsp", p.display())),
        None => said.push("`cells` is not on PATH, so Helix cannot start `cells lsp`: link it onto PATH (Cells.app carries it, Contents/Helpers/cells)".to_owned()),
    }
    said.push("check with: hx --health cells".to_owned());
    Ok(said)
}
