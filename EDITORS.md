# Editors

A `.cell` file in an editor takes two pieces from this repository: the tree-sitter grammar (`tree-sitter/`) for highlighting, and `cells lsp`, the language server, for everything that needs the engine.

## The language server

`cells lsp` speaks the Language Server Protocol on stdin and stdout. It is the command line's (`rust/cells-cli`), built with `cargo build --release -p cells-cli` in `rust/` into `rust/target/release/cells`. It reads every file as the editor holds it, follows its imports from the folder the editor opened (a file the editor holds open is read as it holds it), and answers from the engine:

- **Diagnostics** as you type, in every open file: each break of the grammar and each problem, where it is (a cell's text, a header cell's type, the part of a formula that does not read, a prelude line). A change in one file that breaks a reference in another shows in the other.
- **Hover**: a column's header cell, its type, its formula, what it references, and why it breaks; a cell's value, computed or written, its problem, and the row its reference finds.
- **Completion**: in a reference's cell, the keys of the tables it names, each with its row; a choice's words, a boolean's; in a type, the named types, the tables' keys (`accounts.account`) and the bases; in a formula, the columns, `this`, the tables and the functions, after `.` the columns of what the name leads to (`accounts['rent'].`), and inside `this[` or `fees[` the table's rows, each by its key, found by its number too (`#3`).
- **A row by its number**: a formula holding `this[#3]` breaks (a file never holds a row's place), and the code action writes the row's key in its place: `this['k7m2q9xa']`.
- **Go to definition** from a reference (the row it finds, in its file), a type's target (the key's header cell), a formula's name (its column, its table) and an import (its file).
- **Rename** of a key or a column, followed through every file of the folder that names it: the key's references and the formulas that find its row; the column's formulas, and in other files the references and formulas that reach it. A column takes a name formulas write bare (letters, digits and `_`).

Semantic tokens are not served: the grammar already colours every part.

## Helix

`~/.config/helix/languages.toml`:

```toml
[[language]]
name = "cells"
scope = "source.cells"
file-types = ["cell"]
comment-token = "//"
injection-regex = "^cells$"
roots = []
language-servers = ["cells"]

[[grammar]]
name = "cells"
source = { path = "/path/to/cells/tree-sitter" }

[language-server.cells]
command = "/path/to/cells/rust/target/release/cells"
args = ["lsp"]
```

Then `hx --grammar build`, and link the queries: `ln -s /path/to/cells/tree-sitter/queries/helix ~/.config/helix/runtime/queries/cells`. `hx --health cells` shows the server, the parser and the highlights. Hover is `space k`, the diagnostics `space d`, go to definition `g d`, rename `space r`, the code action `space a`.

## Neovim

Neovim 0.11 configures a server natively:

```lua
vim.filetype.add({ extension = { cell = "cells" } })

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

VS Code starts a language server only from an extension, and none ships yet. One is small: a `package.json` contributing the language `cells` (extension `.cell`) and the TextMate grammar `syntaxes/cells.tmLanguage.json`, and an `extension.ts` whose `activate` starts `cells lsp` through `vscode-languageclient`:

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
