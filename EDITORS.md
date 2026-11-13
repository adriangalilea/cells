# Editors

A `.cells` file in an editor takes two pieces from this repository: the tree-sitter grammar (`tree-sitter/`) for highlighting, and `cells lsp`, the language server, for everything that needs the engine, its colours included where the editor asks for them (Colour, below).

## The language server

`cells lsp` speaks the Language Server Protocol on stdin and stdout. It is the command line's (`rust/cells-cli`): on a Mac with Cells.app, the signed `cells` the app carries and links onto PATH (`~/.local/bin/cells`), the one its engine admits; an editor runs `cells`, never a path into a build. A `cargo build --release -p cells-cli` build is unsigned: it serves an editor where no engine is installed, and an installed engine refuses it. It reads every file as the editor holds it, follows its imports from the folder the editor opened (a file the editor holds open is read as it holds it), and answers from the engine:

- **Diagnostics** as you type, in every open file: each break of the grammar and each problem, where it is (a cell's text, a header cell's type, the part of a formula that does not read, a prelude line). A change in one file that breaks a reference in another shows in the other, and so does a file written by anything else: the server is a client of the engine ([ENGINE.md](ENGINE.md), `cells serve`), which it starts when none answers, asks to serve the folder the editor names and each open file's own, and hears say what changed. The engine stores what a save reaches in every file reading it, and an editor that watches the disk reloads those files.
- **Hover**: a column's header cell, its type, its formula, what it references, and why it breaks; a cell's value, computed or written, its problem, and the row its reference finds.
- **Completion**: in a reference's cell, the keys of the tables it names, each with its row; a choice's words, a boolean's; in a type, the named types, the tables' keys (`accounts.account`) and the bases; in a formula, the columns, `TABLE`, the tables, the functions and words in capitals (`SUM`, `FIRST`, `THIS`), after `.` the columns of what the name leads to (`accounts['rent'].`), after a column's `[` the rows by where they are (`[-1]`, `[FIRST:THIS]`), and inside `TABLE[` or `fees[` the table's rows, each by its key, found by its number too (`#3`).
- **Looks** (SPEC.md, How a value looks): a look rule that does not read is a diagnostic on its line; completion offers the ten tints after a choice's `#` or `tint`, `look` as a statement, what a look is for (a column, a named type, `row`), each rule's next word and a condition's names; hover says a tint, what a rule does, and a column's looks. Every tint is a colour (`textDocument/documentColor`), so an editor with a picker (VS Code's) draws its swatch and writes back the colour picked, a tint's name where it is one, else its hex.
- **Computed values stored on save**: formatting, which the editor runs as it writes the file, stores every computed value the edit reaches beside its `=` (SPEC.md, Writing), so `:w` after changing a cost writes its total and every running total below it.
- **A stale cell's two fixes**: a computed cell that stores another value than its formula gives (typed over, or left by an edit outside the editor) is a diagnostic naming both, with two code actions, each one edit: use the formula's value, or keep the stored one as the row's override.
- **A row by its number**: a formula holding `TABLE[#3]` breaks (a file never holds a row's place), and the code action writes the row's key in its place: `TABLE['k7m2q9xa']`.
- **A spreadsheet's formula**: typed as a spreadsheet writes it (`=C2*D2`, `=SUM($E$2:E2)`), the code action writes it in names (`=cost * qty`, `=SUM(total[FIRST:THIS])`); a function in lower case (`sum(`) is a diagnostic whose action writes it in capitals.
- **Go to definition** from a reference (the row it finds, in its file), a type's target (the key's header cell), a formula's name (its column, its table) and an import (its file).
- **Rename** of a key or a column, followed through every file of the folder that names it: the key's references and the formulas that find its row; the column's formulas, and in other files the references and formulas that reach it. A column takes a name formulas write bare (letters, digits and `_`).

## Colour

A table is read by its columns, so a file is coloured the way Rainbow CSV colours one: a column's name in the header and every cell under it share a colour, six in turn, whatever the cell spells (its kind is its header's type). A formula reads as a formula: its operators, functions and numbers in their own colours, and each name it reads in the colour of the column it names, so `=cost * qty` shows what it is computed from.

What each part of a file is comes from one place, the engine's reading of it (`cells::tokens`): every span, what it is, the column it stands in, and for a name in a formula the column it reads. Every surface draws that reading:

| Surface | Reads | State |
|---|---|---|
| the engine's `tokens` | the text alone, never refused: a broken file still has its spans | built |
| `cells lsp` | semantic tokens: a part with a standard type served as it (keyword, type, number, operator), so any theme colours it; a header's name, a cell and a formula's name that reads a column as `column` or `cell`, types no theme colours, so the grammar's colour stays, with the column as a modifier (`column1` to `column6`) | built |
| wasm | `cells_tokens`, `cells_token_kinds` and `cells_layout` | built |
| ui's `code` (`cells-code`) | the wasm's tokens: a colour per column, a formula's names in the colour of the column each reads, the rest ink | built; inline code (a fragment read by its shape) not built |
| the C ABI, Swift | `cells_tokens`, as `Tokens.swift` draws them, a formula's names by the column each `reads` | built |
| the TextMate and tree-sitter grammars | the text by pattern, where no engine runs (GitHub, an editor without the server): each column a scope its theme already colours, a name in a formula without its column | built; tree-sitter colours a row's text cells by column, its numbers, dates, booleans and quoted text by kind |

A grammar is the reading without the engine, so it is held to the engine's: each grammar's scopes over the examples are to agree with the engine's kinds (not checked yet).

The same reading lays a table out as a table: `cells::tokens::layout` gives the spaces after each cell that bring a column's cells to one end, and with a cap the middle of each wider cell to fold. Never written into the file, since a cell is read exactly as written. `cells lsp` serves the spaces as inlay hints, which Helix, VS Code, Neovim and Zed draw between the real characters (built); ui's code blocks are to draw them, the folds opened by a click (not built). The protocol has no way to fold part of a line, so folding in an editor needs a plugin per editor.

## Helix

```sh
cells editors install helix
```

writes the grammar and Helix's queries that this `cells` carries (`~/.local/share/cells/tree-sitter`, `~/.config/helix/runtime/queries/cells`), adds the language and its server to `~/.config/helix/languages.toml`, and builds the parser, so the parser, its queries and the server are one version. A `cells` language the config already declares is kept as it is, and so is a queries folder that is a link (a checkout's). Run it again after updating Cells. By hand, the same in `~/.config/helix/languages.toml`:

```toml
[[language]]
name = "cells"
scope = "source.cells"
file-types = ["cells"]
comment-token = "//"
injection-regex = "^cells$"
roots = []
language-servers = ["cells"]
auto-format = true

[[grammar]]
name = "cells"
source = { path = "/path/to/cells/tree-sitter" }

[language-server.cells]
command = "cells"
args = ["lsp"]
```

Then `hx --grammar build`, and link the queries: `ln -s /path/to/cells/tree-sitter/queries/helix ~/.config/helix/runtime/queries/cells`. `hx --health cells` shows the server, the parser and the highlights. After a pull that changes `tree-sitter/src`, `hx --grammar build` again: a parser older than its queries makes Helix drop the highlights whole and say nothing, and `--health` still shows ✓. Hover is `space k`, the diagnostics `space d`, go to definition `g d`, rename `space r`, the code action `space a`. A cell is an argument (`mia` and `maa` select it, `]a` and `[a` go to the next and the one before), a row an entry (`]e`, `[e`), a formula a function's body (`mif`), and the symbol picker (`space s`) lists every column, named type and table. A formula's brackets wear rainbow colours where `rainbow-brackets` is on. `auto-format` is what stores the computed values on `:w` and lays the file out as `cells fmt` does, its columns aligned; without it, `:format` does. A file nobody formatted still shows aligned: the server's inlay hints draw the spaces `cells fmt` would write. To write no padding, `config = { align = false }` under `[language-server.cells]`.

## Git

`cells merge` merges two versions of a file as SPEC.md, Merging, says: what was written merges as text, every computed value is computed again from the merged rows, so two branches that each add a row to a running balance never conflict on the balances. `cells diff` is git's textconv: a file with its padding taken out, so a diff shows the values that changed, never a column re-padded by `cells fmt` (SPEC.md, Formatting). In the repository's `.gitattributes`:

```text
*.cells merge=cells diff=cells
```

and once per machine:

```sh
git config --global merge.cells.name "cells"
git config --global merge.cells.driver "cells merge %O %A %B"
git config --global diff.cells.textconv "cells diff"
```

A conflict in what was written is left in the file with git's markers, and the merge exits 1. `git diff --no-textconv` shows the bytes, padding included.

## Neovim

```sh
cells editors install neovim
```

writes a package on Neovim's own packpath, `~/.local/share/nvim/site/pack/cells/start/cells` (`$XDG_DATA_HOME`), that Neovim 0.11 or later loads at start with no line of config: `plugin/cells.lua` (the `cells` filetype, the server enabled, tree-sitter highlighting started), `lsp/cells.lua` (the server, `cells lsp`), `queries/cells` (the queries outside `tree-sitter/queries/helix`) and `parser/cells.so`, which it builds with `cc` from the grammar this `cells` carries. Then it opens a `.cells` file in `nvim --headless` and checks the filetype, the parser, its highlights and the server's config. Your config is never touched: a `vim.lsp.config("cells", { ... })` of yours is laid over `lsp/cells.lua`. Run it again after updating Cells; the package is written whole each time.

The package turns on what Helix's `auto-format` and inlay hints do: a `.cells` buffer is formatted by the server on `:w`, which stores the computed values, and draws the server's inlay hints, its columns aligned. A name the server reads as a column wears that column's colour (`@lsp.typemod.column.column1.cells` to `column6`, linked to the queries' six), so `=cost * qty` shows what it reads. `:checkhealth vim.lsp vim.treesitter` in a `.cells` file shows the server and the parser.

## Zed

Zed takes a language only from an extension, and starts a language server only from an extension's code: `settings.json` can change how a server Zed knows is run, never add one. The extension is `editors/zed`: the grammar, fetched by Zed from this repository at the commit `extension.toml` names, the queries (highlights, brackets, outline, injections), and a launcher of a dozen lines that runs the `cells` on PATH as `cells lsp`.

```sh
cells editors install zed
```

writes the extension this `cells` carries to `~/.local/share/cells/zed` and says the one step Zed keeps for its own window: Extensions › Install Dev Extension › that folder (or the `zed: install dev extension` action). Zed builds it there, the launcher with Rust from rustup and the grammar with the wasi-sdk it downloads. Run again after updating Cells, it rewrites the folder and says to press Rebuild on Cells in Zed's extensions. A checkout installs `editors/zed` the same way.

Zed asks a server for colours and inlay hints only where a setting says so; in `settings.json`:

```json
"languages": {
  "Cells": { "semantic_tokens": "combined", "inlay_hints": { "enabled": true } }
}
```

`combined` lays the server's colours over the grammar's: a name that reads a column wears its column's (`semantic_token_rules.json` maps `column1` to `column6` onto six theme colours), and the inlay hints draw the columns aligned. Formatting on save is on by default, and stores the computed values.

`tree-sitter/scripts/queries.ts` writes the extension's highlights from the canonical queries, in Zed's theme names, and sets `rev` to the last commit that changed the parser, so a change to the grammar reaches Zed once that commit is pushed. Zed's registry (a pull request to zed-industries/extensions adding this repository as a submodule, `path = "editors/zed"`) would install it with one click and no Rust; it needs a licence file inside `editors/zed`, which this repository does not have yet.

## VS Code

VS Code starts a language server only from an extension, and none ships yet. One is small: a `package.json` contributing the language `cells` (extension `.cells`) and the TextMate grammar `syntaxes/cells.tmLanguage.json`, and an `extension.ts` whose `activate` starts `cells lsp` through `vscode-languageclient`:

```ts
import { LanguageClient } from "vscode-languageclient/node"

export function activate() {
  const client = new LanguageClient(
    "cells",
    "Cells",
    { command: "cells", args: ["lsp"] },
    { documentSelector: [{ language: "cells" }] },
  )
  client.start()
}
```
