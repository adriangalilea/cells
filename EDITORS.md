# Editors

A `.cells` file in an editor takes two pieces from this repository: the tree-sitter grammar (`tree-sitter/`) for highlighting, and `cells lsp`, the language server, for everything that needs the engine, its colours included where the editor asks for them (Colour, below).

## The language server

`cells lsp` speaks the Language Server Protocol on stdin and stdout. It is the command line's (`rust/cells-cli`), built with `cargo build --release -p cells-cli` in `rust/` into `rust/target/release/cells`. It reads every file as the editor holds it, follows its imports from the folder the editor opened (a file the editor holds open is read as it holds it), and answers from the engine:

- **Diagnostics** as you type, in every open file: each break of the grammar and each problem, where it is (a cell's text, a header cell's type, the part of a formula that does not read, a prelude line). A change in one file that breaks a reference in another shows in the other.
- **Hover**: a column's header cell, its type, its formula, what it references, and why it breaks; a cell's value, computed or written, its problem, and the row its reference finds.
- **Completion**: in a reference's cell, the keys of the tables it names, each with its row; a choice's words, a boolean's; in a type, the named types, the tables' keys (`accounts.account`) and the bases; in a formula, the columns, `TABLE`, the tables, the functions and words in capitals (`SUM`, `FIRST`, `THIS`), after `.` the columns of what the name leads to (`accounts['rent'].`), after a column's `[` the rows by where they are (`[-1]`, `[FIRST:THIS]`), and inside `TABLE[` or `fees[` the table's rows, each by its key, found by its number too (`#3`).
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
| ui's `code` (`cells-code`) | the wasm's tokens: a colour per column, a formula's names in the colour of the column each reads, the rest ink | built; inline code (a fragment read by its shape) owed |

The same reading lays a table out as a table: `cells::tokens::layout` gives the spaces after each cell that bring a column's cells to one end, and with a cap the middle of each wider cell to fold. Never written into the file, since a cell is read exactly as written. `cells lsp` serves the spaces as inlay hints, which Helix, VS Code, Neovim and Zed draw between the real characters (built); ui's code blocks draw them, the folds opened by a click (owed). Folding in an editor has no way through the protocol, and waits for a plugin per editor.
| the C ABI, Swift | `cells_tokens`, as `Tokens.swift` draws them | built; Swift reads `reads` from the next Apple release |
| the TextMate and tree-sitter grammars | the text by pattern, where no engine runs (GitHub, an editor without the server): each column a scope its theme already colours, a name in a formula without its column | TextMate built; tree-sitter's rows still by kind |

A grammar is the reading without the engine, so it is held to the engine's: each grammar's scopes over the examples agree with the engine's kinds (owed).

## Helix

`~/.config/helix/languages.toml`:

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
command = "/path/to/cells/rust/target/release/cells"
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

Neovim 0.11 configures a server natively:

```lua
vim.filetype.add({ extension = { cells = "cells" } })

vim.lsp.config("cells", {
  cmd = { "/path/to/cells/rust/target/release/cells", "lsp" },
  filetypes = { "cells" },
  root_markers = { ".git" },
})
vim.lsp.enable("cells")
```

For highlighting, register the grammar with nvim-treesitter and copy `tree-sitter/queries/` (the queries outside `helix/`) to `queries/cells/` on the runtime path:

```lua
require("nvim-treesitter.parsers").get_parser_configs().cells = {
  install_info = { url = "/path/to/cells/tree-sitter", files = { "src/parser.c", "src/scanner.c" } },
  filetype = "cells",
}
```

## VS Code

VS Code starts a language server only from an extension, and none ships yet. One is small: a `package.json` contributing the language `cells` (extension `.cells`) and the TextMate grammar `syntaxes/cells.tmLanguage.json`, and an `extension.ts` whose `activate` starts `cells lsp` through `vscode-languageclient`:

```ts
import { LanguageClient } from "vscode-languageclient/node"

export function activate() {
  const client = new LanguageClient(
    "cells",
    "Cells",
    { command: "/path/to/cells/rust/target/release/cells", args: ["lsp"] },
    { documentSelector: [{ language: "cells" }] },
  )
  client.start()
}
```
