# TODO

## The hero becomes usable (Adrian's idea, worth a great deal of effort)

After it plays, a "your turn" control pauses the clip, and the frame at that instant (the files as typed so far, the tab in front, the caret) becomes live state: one folder in memory (`accounts.cell`, `money.cell`) that every window reads and writes through the real wasm engine. Any window's edit is a write to the folder, and every window reads it again.

- **The Cells window:** ui's `table`, editable on that folder (`useTableDocument` with the `cells` codec, which shows a computed cell's value and stores what each edit reached). Buildable now.
- **The terminal:** a small shell over the folder running the real engine: `cells check`, `cells show`, `ls`, `cat`. Anything else answers that it is not in this demo. Nothing leaves the page.
- **The editor:** real modal editing, Helix's, with diagnostics, hover and completion from the engine as the language server gives them. The landing's window already takes typing as a text box (ui's `onEdit`).

Waiting on ui (its TODO, "Cells on the one engine"): `terminal` taking input with a command handler the page supplies, and modal editing in `terminal-editor`'s look. The folder, the commands and the wiring are this site's.
