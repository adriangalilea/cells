//! What `cells` is, said once: the program, when an agent reaches for it, what it never
//! does with it, and every command's usage and sentence. The usage a wrong argument prints,
//! `cells help` and the agent skill (`cells skill show`) are three renderings of this one
//! table, so none can say what the others do not. The conventions are swift-utils'
//! TerminalPrograms (its README, "TerminalPrograms"): `says`, `useWhen` and `never` make the
//! skill's description, and the skill is every command in full.

/// One command: how it is written, and what it does in a sentence or two.
pub struct Command {
    pub usage: &'static str,
    pub says: &'static str,
}

pub const NAME: &str = "cells";

pub const SAYS: &str = "Cells files (`.cells`) from the shell: typed tables whose header cells carry each column's type and formula, checked, formatted, queried with SQL, charted and kept computed by the engine";

/// When an agent reaches for it.
pub const USE_WHEN: &[&str] = &[
    "reading, writing or checking a `.cells` file or a folder of them",
    "a question over their rows (`cells sql`, DuckDB's SQL over every file and its imports)",
    "a table's values as plain text (`cells show`) or its charts in a terminal (`cells chart`)",
    "a file whose computed cells are stale after an edit (`cells fix`)",
    "an editor set up for the format (`cells editors install`)",
];

/// What an agent never does with it.
pub const NEVER: &[&str] = &[
    "type a computed cell's stored value by hand (a cell ending in `=`, `73.6=`): `cells fix` writes it from the formula",
    "convert a `.cells` file to CSV or rewrite it with another tool: Cells is its own format, and `cells fmt` lays it out",
    "run `cells allow` for the person: allowing a script is their yes, answered in their terminal",
];

/// How a folder is read and what the exit codes mean, for every command.
pub const ABOUT: &str = "A folder means every `.cells` file under it, hidden folders (`.git`, the engine's `.cells`) left out. Exit 0 when nothing is found (or every run succeeds), 1 when something is (or a run fails, or a query read files with problems: its answer is printed, and how many problems on stderr), 2 when the command cannot run (a path that does not exist, a file that cannot be read, a wrong argument, tables whose columns clash, a query DuckDB refuses). The format is SPEC.md in the Cells repository.";

pub const COMMANDS: &[Command] = &[
    Command {
        usage: "cells check <file|folder>...",
        says: "Every break and problem, `path column 3: to: why`, `path row N, column: why`, its imports followed, its own tables checked too.",
    },
    Command {
        usage: "cells fmt [--check] [--align on|off] <file|folder>...",
        says: "Each file as the format's standard lays it out, every table's columns aligned (`--align off`: no padding); `--check` writes nothing and says which files it would change.",
    },
    Command {
        usage: "cells diff <file>",
        says: "git's textconv: the file with its padding taken out, so a diff shows the values that changed.",
    },
    Command {
        usage: "cells fix <file|folder>...",
        says: "Every stale computed cell given its formula's value, every one not stored yet stored, each file written.",
    },
    Command {
        usage: "cells merge <base> <ours> <theirs>",
        says: "git's merge driver: what was written merged, what is computed computed again (SPEC.md, Merging).",
    },
    Command { usage: "cells pin <file>...", says: "Every import from a URL pinned by its hash now." },
    Command {
        usage: "cells run <file> [script]",
        says: "Its scripts run once by the engine, their rows written under them; a script runs only as its person allowed it, and the engine runs it on its schedule anyway.",
    },
    Command {
        usage: "cells allow <file> [script]",
        says: "Each script not allowed as written shown whole and allowed once its person says yes, in a terminal.",
    },
    Command {
        usage: "cells show <file>",
        says: "Its table as tab-separated text, computed cells their values as the file stores them (`0.00`).",
    },
    Command {
        usage: "cells chart <file> [n …] [--watch] [--svg] [--glyphs fine|blocks|ascii] [--colors true|256|16]",
        says: "Its charts listed, or charts n drawn in the terminal (`--watch`: held, drawn again as the file changes; `--svg`: as SVG); chart 0 is the one a file with none is drawn from.",
    },
    Command {
        usage: "cells sql <file|folder>... <query>",
        says: "The query, DuckDB's SQL, over every file and the files they import, each a table by its stem; its answer written as `show` writes a table.",
    },
    Command { usage: "cells lsp", says: "The language server, on stdin and stdout." },
    Command {
        usage: "cells serve [--verbose] <folder>...",
        says: "The engine kept running over the folders: a change written to a file stores every computed cell it reaches, in that file and every file importing it, each version logged in `.cells/log.jsonl`, each step said on stderr (`--verbose`: every read, timed).",
    },
    Command {
        usage: "cells history <file|folder> [<row> <column>]",
        says: "What changed, by whom, when, and what each change reached, newest first; or a cell's every value, each beside the change that wrote it (ENGINE.md, History).",
    },
    Command {
        usage: "cells editors install helix|neovim|zed",
        says: "The editor set up for `.cells`: the grammar and queries this binary carries written where it reads them, the language and its server added to its config, the parser built; Zed's extension written as the folder Zed installs (EDITORS.md).",
    },
    Command {
        usage: "cells skill [show|install|remove]",
        says: "This description as an agent skill: where it is installed and whether it is current, printed whole (`show`), linked into Claude Code's and Codex's skill folders (`install`; inside Cells.app a link to the app's copy, so the app's update is the skill's), or taken out (`remove`).",
    },
    Command { usage: "cells help", says: "This, every command in full." },
];

/// Every command's usage, a line each: what a wrong argument prints.
pub fn usage() -> String {
    COMMANDS.iter().map(|c| c.usage).collect::<Vec<_>>().join("\n")
}

/// `cells help`: what it is, then every command's usage with its sentence under it, then
/// how a folder is read and the exit codes.
pub fn help() -> String {
    let mut out = vec![format!("{NAME}: {SAYS}."), String::new()];
    for c in COMMANDS {
        out.push(format!("  {}", c.usage));
        out.push(format!("      {}", c.says));
    }
    out.push(String::new());
    out.push(ABOUT.to_string());
    out.join("\n")
}

/// The line agents choose skills by: what it is, when to use it, what never to do.
fn description() -> String {
    format!("{SAYS}. Use it for: {}. Never {}.", USE_WHEN.join("; "), NEVER.join("; "))
}

/// The skill as agents read it: the description as one Markdown document, every command in
/// full. Its `generated-by` names this program, which is how `cells skill` knows a copy as
/// its own.
pub fn skill() -> String {
    let mut out = vec![
        "---".to_string(),
        format!("name: {NAME}"),
        format!("description: {}", yaml(&description())),
        "metadata:".to_string(),
        format!("  generated-by: {}", yaml(&generated_by())),
        "---".to_string(),
        String::new(),
        format!("# {NAME}"),
        String::new(),
        format!("{SAYS}."),
        String::new(),
        "## Use it for".to_string(),
        String::new(),
    ];
    out.extend(USE_WHEN.iter().map(|w| format!("- {w}")));
    out.extend([String::new(), "## Never".to_string(), String::new()]);
    out.extend(NEVER.iter().map(|n| format!("- {n}")));
    out.extend([String::new(), "## Commands".to_string(), String::new()]);
    out.extend(COMMANDS.iter().map(|c| format!("- `{}`: {}", c.usage, c.says)));
    out.extend([String::new(), "## Folders and exit status".to_string(), String::new(), ABOUT.to_string(), String::new()]);
    out.join("\n")
}

/// Who wrote a skill: this program and its version.
pub fn generated_by() -> String {
    format!("{NAME} {}", env!("CARGO_PKG_VERSION"))
}

/// A YAML scalar in double quotes, so a colon or a `#` in it is text.
fn yaml(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
