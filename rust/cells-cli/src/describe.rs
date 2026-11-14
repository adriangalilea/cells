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
    "a file to send alone, the files it imports packed into it (`cells pack`)",
    "an editor set up for the format (`cells editors install`)",
];

/// What an agent never does with it.
pub const NEVER: &[&str] = &[
    "type a computed cell's stored value by hand (a cell ending in `=`, `73.6=`): `cells fix` writes it from the formula",
    "edit a packed copy (`packed …` in a prelude): it is another file's, edited there and refreshed with `cells pack`",
    "convert a `.cells` file to CSV or rewrite it with another tool: Cells is its own format, and `cells fmt` lays it out",
    "run `cells allow` for the person: allowing a script is their yes, answered in their terminal",
];

/// How a folder is read and what the exit codes mean, for every command.
pub const ABOUT: &str = "A folder means every `.cells` file under it, hidden folders (`.git`, the engine's `.cells`) left out. Exit 0 when nothing is found (or every run succeeds, or `check` found notes alone: a stored value standing for an import not reached), 1 when something is (or a run fails, or a query read files with problems: its answer is printed, and how many problems on stderr), 2 when the command cannot run (a path that does not exist, a file that cannot be read, a wrong argument, tables whose columns clash, a query DuckDB refuses).";

/// The repository, where the documents `DOCS` names live.
const REPO: &str = "https://github.com/adriangalilea/cells/blob/main";

/// Where to read more: each a file at the repository's root and what it says. A test holds
/// that each is there, so a moved document fails the build before a link lies.
pub const DOCS: &[(&str, &str)] = &[
    ("SPEC.md", "the format whole: every rule, and why"),
    ("EDITORS.md", "each editor's setup and what the language server gives it"),
    ("README.md", "what Cells is for, and where the code lives"),
];

/// The file the format's brief shows, `FORMAT`'s example: a test reads it with the engine
/// and holds that it has no problem, so the brief never shows a file that does not read.
pub const EXAMPLE: &str =
    "item: text!,    cost: EUR>=0,   qty: int>=1,    paid: yes/no,   on: date,   kind: travel|stay|food, total: EUR = cost * qty
fuel,           84.20,          1,              yes,            2026-08-14, travel,                 84.20=
cabin,          310.00,         1,              no,             2026-08-15, stay,                   310.00=
";

/// The format in brief, enough to read and write a file right, after the example: `cells
/// help format` prints it and the skill holds it. Every rule here is SPEC.md's, which says
/// each one whole.
pub const FORMAT: &str = r#"A Cells file is one table, a CSV whose header cells carry each column's type and, for a computed column, its formula; its rows hold every value, the computed ones too. One file, never a sidecar.

A header cell is `name: type`, or `name: type = formula`, the form a column declared in the prelude has. A cell with no `:` is a plain column, its type read from its values, so every CSV is a Cells file with no types written. Spaces at a field's ends are padding, never part of it (`cells fmt` aligns the columns with them).

Types: `text`, `int`, `num`, `pct`, `bool`, `date` (`2026-08-14`), `time` (`14:30`), `datetime` (`2026-08-14T14:30:00Z`), `url`, `email`, a currency by its ISO code (`EUR`, `USD`: an amount), `id`. A choice is its words joined by `|` (`travel|stay|food`, a word's colour after `#`: `income#green`); a boolean in the file's own words is `yes/no`; a reference to another table's key is `table.column` (`to: accounts.account`). After the type: bounds (`>=0`, `<100`, `0..10`; on text its length), a date's or time's format (`date(DD/MM/YYYY)`), then `!` for the key (present and unique) or `?` for may be blank. Without `?` a blank cell is reported missing.

A cell is one of four forms: `84.20` written; `84.20=` computed by its column's formula, the value it gave stored left of the `=`; `32.25=96.75 / 3` computed by its own formula, right of the `=`; `=96.75 / 3` its own formula not computed yet. A text holding `=` is written `'=`. A field holding a `,` or a `"` is quoted as CSV quotes it, so a formula with a comma (`ROUND(cost, 2)`) quotes its whole header cell.

Formulas are SQL's scalar expressions in exact decimal, the language in capitals only (`SUM`, `ROUND`, `CASE WHEN … THEN … ELSE … END`, `TRUE`, `NULL`); a word in lower case is always a column the file names (`cost * qty`; `"price net"` quoted). `to.kind` follows a reference; `fees[kind].pct` is a table's row by its key; `total[-1]` the row before, `SUM(total[FIRST:THIS])` a running sum; `TABLE` the file's own table.

A file may open with a prelude between `---` lines: `import accounts from "./accounts.cells"`, `type Kind = card|transfer` (a named type, capitalised), a column declared as `total: EUR = cost * qty` (the header then names it alone), a table written inline (`fees = ```cells` … ```` ``` ````), a script that fills a table, a `look` (how values are drawn) and charts (```` ```chart bars ````). A `//` starts a comment there. A `packed` statement (`packed accounts = ```cells` … ```` ``` ````, `packed type Kind = …`) is a copy of what an import names, read only where the import cannot be reached: never edit one, `cells pack` writes them.

After editing by hand: `cells check` says every problem by row and column, `cells fix` stores the computed values, `cells fmt` lays the file out."#;

/// The editors `cells editors install` sets up, from its own list, and the server each runs.
fn editors() -> String {
    format!(
        "`cells lsp` is the language server, and `cells editors install {}` sets an editor up with it and the grammar's highlighting. Another editor that speaks the protocol runs `cells lsp` itself; what the server gives each editor is EDITORS.md's.",
        crate::editors::EDITORS.join("|")
    )
}

/// Where to read more, as links.
fn docs() -> Vec<String> {
    DOCS.iter().map(|(file, says)| format!("- [{file}]({REPO}/{file}): {says}")).collect()
}

/// `cells help format`: the example, the format in brief, the editors, where to read more.
pub fn format_guide() -> String {
    let mut out = vec!["```cells".to_string(), EXAMPLE.trim_end().to_string(), "```".to_string(), String::new()];
    out.push(FORMAT.to_string());
    out.extend([String::new(), editors(), String::new()]);
    out.extend(docs());
    out.join("\n")
}

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
        usage: "cells pack [--remove] [--out <path>] <file>",
        says: "Every import written into the prelude as a packed copy (`packed accounts = ```cells`, `packed type Kind = …`), refreshed from its source, so the file reads whole where its imports cannot be reached; `--out`: a self-contained copy written there, the file untouched; `--remove`: every copy taken out. A copy of a source not reachable is kept; an import with neither exits 1. `with { pack: \"auto\" }` on an import has every write refresh its copy instead.",
    },
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
        usage: "cells chart <file> [n …] [--watch] [--svg] [--glyphs fine|blocks|ascii] [--colors true|256|16] [--edges on|off]",
        says: "Its charts listed, or charts n drawn in the terminal (`--watch`: held, drawn again as the file changes, `q` or Esc leaving; `--svg`: as SVG); chart 0 is the one a file with none is drawn from. A chart of several series names each above its plot, in its colour and with its mark. A terminal that answers the cell border protocol gets the frame, gridlines and axis as lines on the cells' edges, costing no row or column (`--edges on|off` says so instead of asking).",
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
        usage: "cells editors status",
        says: "Each editor's standing, a line each, `<editor> <state>`: `absent`, `available` (on this machine, not set up), `current`, `older`, `uninstalled` (Zed's extension written, not yet installed in Zed) or `failed <why>`; what Cells.app's Settings shows.",
    },
    Command {
        usage: "cells editors update",
        says: "Every editor set up brought to this binary's version, one already holding it left alone; Zed's extension, which only Zed's window rebuilds, said when older. Cells.app runs it on every launch, so an update of Cells is its editors' (EDITORS.md).",
    },
    Command {
        usage: "cells skill [show|install|remove]",
        says: "This description as an agent skill: where it is installed and whether it is current, printed whole (`show`), linked into Claude Code's and Codex's skill folders (`install`; inside Cells.app a link to the app's copy, so the app's update is the skill's), or taken out (`remove`).",
    },
    Command {
        usage: "cells help [format]",
        says: "This, every command in full; `format`: the format in brief, an example file, the editors and where to read more.",
    },
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
    out.push(String::new());
    out.push("The format in brief, an example and where to read more: cells help format".to_string());
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
    out.extend([String::new(), "## The format".to_string(), String::new()]);
    out.extend(["```cells".to_string(), EXAMPLE.trim_end().to_string(), "```".to_string(), String::new()]);
    out.push(FORMAT.to_string());
    out.extend([String::new(), "## Commands".to_string(), String::new()]);
    out.extend(COMMANDS.iter().map(|c| format!("- `{}`: {}", c.usage, c.says)));
    out.extend([String::new(), "## Folders and exit status".to_string(), String::new(), ABOUT.to_string()]);
    out.extend([String::new(), "## Editors".to_string(), String::new(), editors()]);
    out.extend([String::new(), "## Read more".to_string(), String::new()]);
    out.extend(docs());
    out.push(String::new());
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The brief's example is a file that reads with no problem.
    #[test]
    fn the_example_reads() {
        let doc = cells::read(EXAMPLE.as_bytes().to_vec()).expect("the example is UTF-8");
        assert!(doc.broken().is_empty(), "the example breaks: {:?}", doc.broken());
        let problems: Vec<String> = doc.problems().iter().map(|p| doc.describe(p)).collect();
        assert!(problems.is_empty(), "the example has problems: {problems:?}");
    }

    /// Every document a link names is in the repository.
    #[test]
    fn every_link_is_there() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (file, _) in DOCS {
            assert!(root.join(file).is_file(), "{file} is linked from the skill and not in the repository");
        }
    }

    /// The usage of `editors install` names the editors it installs, no more and no fewer.
    #[test]
    fn the_editors_usage_is_the_list() {
        let usage = format!("cells editors install {}", crate::editors::EDITORS.join("|"));
        assert!(COMMANDS.iter().any(|c| c.usage == usage), "no command's usage is `{usage}`");
    }
}

/// Who wrote a skill: this program and its version.
pub fn generated_by() -> String {
    format!("{NAME} {}", env!("CARGO_PKG_VERSION"))
}

/// A YAML scalar in double quotes, so a colon or a `#` in it is text.
fn yaml(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
