# TODO

## The docs, finished

Built: every document a page (SPEC.md a page per section), links resolved at build, every `cells` block read by the engine, the ground quiet behind reading. Owed, in order:

- **Inline code coloured.** `` `total:EUR=cost * qty` `` in prose is Cells and draws plain. The grammar is built (`syntaxes/cells-inline.tmLanguage.json`, `source.cells.inline`, tested in `syntaxes/test.ts`). Waiting on ui: `highlightTree` colouring `inlineCode` ahead for a language it is given, `code-highlight` giving inline html, `Markdown` drawing it, and `scripts/cells.ts` vendoring the inline grammar beside the main one. Then `lib/docs.ts` passes `cells-inline`.
- **Read on a phone.** The sidebar folds into a menu under `lg`; today it stacks above the article, so a phone scrolls the whole list before the page.
- **Search.** ⌘K over every section's words (ui's `command`, already installed), the section's page and anchor as the result.
- **The suite beside the spec.** Each spec section shows the conformance cases that pin it, read from `tests/`, so a rule and its proof sit on one page.
- **Guides.** Task pages (keep a budget, import a bank's CSV, set up an editor) as files under `docs/`, read and checked as the rest.
- **Verified by eye.** Contrast of body text, code and the sidebar in both themes, on a real screen.

## The hero becomes usable (Adrian's idea, worth a great deal of effort)

After it plays, a "your turn" control pauses the clip, and the frame at that instant (the files as typed so far, the tab in front, the caret) becomes live state: one folder in memory (`accounts.cells`, `money.cells`) that every window reads and writes through the real wasm engine. Any window's edit is a write to the folder, and every window reads it again.

- **The Cells window:** ui's `table`, editable on that folder (`useTableDocument` with the `cells` codec, which shows a computed cell's value and stores what each edit reached). Buildable now.
- **The terminal:** a small shell over the folder running the real engine: `cells check`, `cells show`, `ls`, `cat`. Anything else answers that it is not in this demo. Nothing leaves the page.
- **The editor:** real modal editing, Helix's, with diagnostics, hover and completion from the engine as the language server gives them. The landing's window already takes typing as a text box (ui's `onEdit`).

Waiting on ui (its TODO, "Cells on the one engine"): `terminal` taking input with a command handler the page supplies, and modal editing in `terminal-editor`'s look. The folder, the commands and the wiring are this site's.
