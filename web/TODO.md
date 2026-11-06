# TODO

## The landing, as it stands and what it owes (2026-11-06)

Built: the hero is someone at work in Cells (two app windows, ui's table-session, every commit the engine's); three tools one at a time (a spreadsheet beside the landed window, a database and a script each with its scene), then the one plain text file; the story from a CSV, the app first and the text after; the price ("Unapologetically incompatible with CSV."); what it opens, each lever with a proof the build checks; where it opens; your turn, the trip editable. "Programmable data." is the line, never reworded (the repository's CLAUDE.md).

Owed, in order:

- [ ] **The app's window, after Cells.app**: designed and validated in the Mac app first (untitled `apps/cells/TODO.md`, "The window, designed"; on its `cells-window` branch, under Adrian's review), then mirrored here as it settles there: the folder in the title's second line; the prelude first class, a panel along the work's foot as an editor's console, open at first (as controls or as its text, each script in a view of its own), its summary the window's foot; the file's facts and problems a trailing pane asked for; heads in their column's colour, their type and formula hidden until asked and grown in their band under the hand as the head itself; the folder a sidebar closed at first; table and chart split by the hand, the chart a card. Never designed here ahead of the app.
- [ ] **What the engine says now, said here too**: each problem's severity (`Kind::severity`: wrong red, absent yellow; the wasm build's problems carry only their kind, and `lib/diagnose.ts` calls every one an error); a row being written holding its warnings until the hand leaves it (ui's table-session, as Swift's `CellsTable` drafting does: a fact of the editing, never of the file); a cell typed into offering what it may hold (`Doc::offers`, through the wasm build); what completes at a place and what is said of it (`cells::complete`, `cells::hover`, the core's since they left `cells lsp`), for a formula typed in the page's table and the terminal editor's text.
- [ ] **The hero, told in order, once the window is**: `accounts.cells` full screen and fully written; `money.cells` opened, the screen split; its import added from its prelude; its data typed a cell at a time, each computed value landing; a link to an account that is not there flagged and fixed in the other window. ui's table-session needs the steps for it: a window opened and the screen arranged, a prelude line set, a column's type and formula set in its head.
- [ ] **The copy pass**, every line of the page and of the docs read again for density: each sentence a fact a reader needs, nothing that fills time.
- [ ] **The link card's title**: it leads with "Programmable data." (the rule), the name smaller.
- [ ] **Deploy**, once Adrian has seen it.

## The docs, finished

Built: every document a page (SPEC.md a page per section), links resolved at build, every `cells` block read by the engine, the ground quiet behind reading. Owed, in order:

- **Inline code coloured.** `` `total:EUR=cost * qty` `` in prose is Cells and draws plain. The grammar is built (`syntaxes/cells-inline.tmLanguage.json`, `source.cells.inline`, tested in `syntaxes/test.ts`). Waiting on ui: `highlightTree` colouring `inlineCode` ahead for a language it is given, `code-highlight` giving inline html, `Markdown` drawing it, and `scripts/cells.ts` vendoring the inline grammar beside the main one. Then `lib/docs.ts` passes `cells-inline`.
- **Read on a phone.** The sidebar folds into a menu under `lg`; today it stacks above the article, so a phone scrolls the whole list before the page.
- **Search.** ⌘K over every section's words (ui's `command`, already installed), the section's page and anchor as the result.
- **The suite beside the spec.** Each spec section shows the conformance cases that pin it, read from `tests/`, so a rule and its proof sit on one page.
- **Guides.** Task pages (keep a budget, import a bank's CSV, set up an editor) as files under `docs/`, read and checked as the rest.
- **Verified by eye.** Contrast of body text, code and the sidebar in both themes, on a real screen.

## The app becomes usable (Adrian's idea, worth a great deal of effort)

After the hero plays, its two app windows take the reader's hands: one folder in memory (`accounts.cells`, `money.cells`) that both windows read and write through the real wasm engine, every edit a write to the folder that both read again. ui's `table`, live, on that folder (`useTableDocument` with the `cells` codec, which shows a computed cell's value and stores what each edit reached). "Your turn" at the page's foot is Helix on the story's trip today; once the app takes hands it is the app first, the text beside it, as the story tells it.

The text's side, waiting on ui (its TODO, "Cells on the one engine"): `terminal` taking input with a command handler the page supplies (`cells check`, `cells fmt`, `ls`, `cat`, nothing leaving the page), and modal editing in `terminal-editor`'s look.
