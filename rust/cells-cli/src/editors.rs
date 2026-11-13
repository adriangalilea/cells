//! `cells editors install helix|neovim|zed`: an editor set up for `.cells` from this binary
//! alone.
//!
//! The grammar's source and the editor's queries are carried inside `cells`, so the parser
//! an editor builds, the queries it reads and the server it runs (`cells lsp`) are always
//! one version. The grammar is written to `<data>/cells/tree-sitter` (`$XDG_DATA_HOME`, else
//! `~/.local/share`), the queries where the editor reads them; an editor's own config is
//! only added to, never rewritten: a `cells` language it already declares is kept as it is.
//! Neovim needs no config at all: a package on its default packpath is a plugin it loads.
//! Zed takes a language only from an extension, so the extension (`editors/zed`) is written
//! as a folder, and Zed installs it from there: the one step left to the person, since Zed
//! installs a dev extension from its own window alone.

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

/// Neovim's queries: the canonical ones, in the capture names Neovim reads.
const NEOVIM_QUERIES: &[(&str, &[u8])] = &[
    ("highlights.scm", include_bytes!("../../../tree-sitter/queries/highlights.scm")),
    ("injections.scm", include_bytes!("../../../tree-sitter/queries/injections.scm")),
];

/// The Neovim package, beside its `parser/cells.so` and `queries/cells`.
const NEOVIM_PACKAGE: &[(&str, &[u8])] = &[
    ("plugin/cells.lua", include_bytes!("../../../editors/neovim/plugin/cells.lua")),
    ("lsp/cells.lua", include_bytes!("../../../editors/neovim/lsp/cells.lua")),
];

/// The Zed extension, as `editors/zed` holds it; Zed builds `extension.wasm` and the
/// grammar's wasm into the folder when it installs it.
const ZED_EXTENSION: &[(&str, &[u8])] = &[
    ("extension.toml", include_bytes!("../../../editors/zed/extension.toml")),
    ("Cargo.toml", include_bytes!("../../../editors/zed/Cargo.toml")),
    ("LICENSE", include_bytes!("../../../editors/zed/LICENSE")),
    ("src/lib.rs", include_bytes!("../../../editors/zed/src/lib.rs")),
    ("languages/cells/config.toml", include_bytes!("../../../editors/zed/languages/cells/config.toml")),
    ("languages/cells/highlights.scm", include_bytes!("../../../editors/zed/languages/cells/highlights.scm")),
    ("languages/cells/injections.scm", include_bytes!("../../../editors/zed/languages/cells/injections.scm")),
    ("languages/cells/brackets.scm", include_bytes!("../../../editors/zed/languages/cells/brackets.scm")),
    ("languages/cells/outline.scm", include_bytes!("../../../editors/zed/languages/cells/outline.scm")),
    ("languages/cells/semantic_token_rules.json", include_bytes!("../../../editors/zed/languages/cells/semantic_token_rules.json")),
];

pub const EDITORS: &[&str] = &["helix", "neovim", "zed"];

pub fn install(editor: &str) -> Result<Vec<String>, String> {
    match editor {
        "helix" => helix(),
        "neovim" => neovim(),
        "zed" => zed(),
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

/// The grammar's source written to `<data>/cells/tree-sitter`, where every editor builds it.
fn grammar(said: &mut Vec<String>) -> Result<PathBuf, String> {
    let grammar = xdg("XDG_DATA_HOME", ".local/share")?.join("cells/tree-sitter");
    write_all(&grammar, GRAMMAR)?;
    said.push(format!("grammar written to {}", grammar.display()));
    Ok(grammar)
}

/// What an editor's `cells lsp` runs: the `cells` on PATH.
fn server(editor: &str) -> String {
    match on_path() {
        Some(p) => format!("language server: {} lsp", p.display()),
        None => format!(
            "`cells` is not on PATH, so {editor} cannot start `cells lsp`: link it onto PATH (Cells.app carries it, Contents/Helpers/cells)"
        ),
    }
}

fn helix() -> Result<Vec<String>, String> {
    let mut said = Vec::new();
    let grammar = grammar(&mut said)?;

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
    said.push(server("Helix"));
    said.push("check with: hx --health cells".to_owned());
    Ok(said)
}

/// A package on Neovim's default packpath (`<data>/nvim/site/pack/cells/start/cells`):
/// Neovim loads its `plugin/` at start and finds its `parser/`, `queries/` and `lsp/` on
/// the runtimepath, so the person's config is never touched, and a `vim.lsp.config("cells",
/// …)` of theirs is laid over the package's `lsp/cells.lua`.
fn neovim() -> Result<Vec<String>, String> {
    let mut said = Vec::new();
    let grammar = grammar(&mut said)?;
    let package = xdg("XDG_DATA_HOME", ".local/share")?.join("nvim/site/pack/cells/start/cells");
    write_all(&package, NEOVIM_PACKAGE)?;
    write_all(&package.join("queries/cells"), NEOVIM_QUERIES)?;
    said.push(format!("plugin and queries written to {}", package.display()));

    // Neovim holds a parser it loaded mapped while it runs, so the new one is built beside
    // it and moved over it: a new file, never the running one rewritten.
    let parser = package.join("parser/cells.so");
    let building = package.join("parser/cells.so.building");
    fs::create_dir_all(parser.parent().expect("parser/")).map_err(|e| format!("{}: {e}", parser.display()))?;
    let src = grammar.join("src");
    let out = Command::new("cc")
        .args(["-shared", "-fPIC", "-O2", "-I"])
        .arg(&src)
        .arg(src.join("parser.c"))
        .arg(src.join("scanner.c"))
        .arg("-o")
        .arg(&building)
        .output()
        .map_err(|e| format!("cc, which builds Neovim's parser, did not run: {e}"))?;
    if !out.status.success() {
        return Err(format!("cc did not build {}:\n{}", parser.display(), String::from_utf8_lossy(&out.stderr)));
    }
    fs::rename(&building, &parser).map_err(|e| format!("{}: {e}", parser.display()))?;
    said.push(format!("parser built: {}", parser.display()));

    // Neovim itself reads the package: a `.cells` file's filetype, its parser and
    // highlights, and the server's config, as a session with the person's plugins finds them.
    let probe = std::env::temp_dir().join(format!("cells-neovim-{}.cells", std::process::id()));
    fs::write(&probe, "a:text!\nx\n").map_err(|e| format!("{}: {e}", probe.display()))?;
    let check = "lua io.stdout:write(vim.bo.filetype == 'cells' \
                 and vim.treesitter.get_parser(0, 'cells'):parse()[1]:root():type() == 'source_file' \
                 and vim.treesitter.highlighter.active[vim.api.nvim_get_current_buf()] ~= nil \
                 and vim.treesitter.query.get('cells', 'highlights') ~= nil \
                 and vim.lsp.config.cells.cmd[1] == 'cells' \
                 and 'cells: loaded' or 'cells: not loaded') vim.cmd('qa!')";
    let run = Command::new("nvim").args(["--headless", "-u", "NORC", "-i", "NONE"]).arg(&probe).args(["-c", check]).output();
    fs::remove_file(&probe).map_err(|e| format!("{}: {e}", probe.display()))?;
    match run {
        Ok(o) if String::from_utf8_lossy(&o.stdout).contains("cells: loaded") => {
            said.push("checked in nvim --headless: the filetype, the parser, its highlights and the server's config".to_owned())
        }
        Ok(o) => {
            return Err(format!(
                "nvim did not load the package:\n{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ));
        }
        Err(_) => said.push("nvim is not on PATH: the package loads in the Neovim (0.11 or later) that finds it".to_owned()),
    }
    said.push(server("Neovim"));
    said.push("check with: :checkhealth vim.lsp vim.treesitter, in a .cells file".to_owned());
    Ok(said)
}

/// Where Zed keeps its extensions, each a folder or, for a dev extension, a link to one.
fn zed_installed() -> Result<PathBuf, String> {
    Ok(if cfg!(target_os = "macos") {
        home()?.join("Library/Application Support/Zed/extensions/installed")
    } else {
        xdg("XDG_DATA_HOME", ".local/share")?.join("zed/extensions/installed")
    })
}

/// The extension written to `<data>/cells/zed`. Zed fetches the grammar from the commit its
/// `extension.toml` names and builds the server's launcher with Rust (rustup's), both when
/// the extension is installed or rebuilt, which only Zed's window does.
fn zed() -> Result<Vec<String>, String> {
    let mut said = Vec::new();
    let extension = xdg("XDG_DATA_HOME", ".local/share")?.join("cells/zed");
    write_all(&extension, ZED_EXTENSION)?;
    said.push(format!("extension written to {}", extension.display()));

    let installed = zed_installed()?.join("cells");
    match fs::symlink_metadata(&installed) {
        Ok(m) if m.file_type().is_symlink() => {
            let to = fs::read_link(&installed).map_err(|e| format!("{}: {e}", installed.display()))?;
            if to == extension {
                said.push("installed in Zed as a dev extension: Zed › Extensions › Cells › Rebuild takes this version".to_owned());
            } else {
                said.push(format!(
                    "Zed has a dev extension cells from {}: kept as it is; Zed › Extensions › Install Dev Extension › {} installs this one in its place",
                    to.display(),
                    extension.display()
                ));
            }
        }
        Ok(_) => said.push(format!("Zed has Cells from its extensions ({}): kept as it is", installed.display())),
        Err(_) => said.push(format!("in Zed: Extensions › Install Dev Extension › {}", extension.display())),
    }
    if Command::new("rustup").arg("--version").output().is_err() {
        said.push("rustup is not on PATH: Zed builds a dev extension with Rust installed by rustup (rustup.rs)".to_owned());
    }
    said.push(server("Zed"));
    said.push(
        "Zed asks no server for colours or inlay hints until a setting says so, in settings.json: \
         \"languages\": { \"Cells\": { \"semantic_tokens\": \"combined\", \"inlay_hints\": { \"enabled\": true } } }"
            .to_owned(),
    );
    Ok(said)
}
