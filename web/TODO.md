# TODO


## The hero becomes usable (Adrian's idea, worth a great deal of effort)

After it plays, a "your turn" control pauses the clip, and the frame at that instant (the files as typed so far, the tab in front, the caret) becomes live state: one folder in memory (`accounts.cell`, `money.cell`) that every window reads and writes through the real wasm engine. Any window's edit is a write to the folder, and every window reads it again.

- **The Cells window:** ui's `table`, editable on that folder (`useTableDocument` with a codec over the engine). Nearly free.
- **The terminal:** a small shell over the folder running the real engine: `cells check`, `cells show`, `ls`, `cat`. Anything else answers that it is not in this demo. Nothing leaves the page.
- **The editor:** real modal editing. CodeMirror 6 with its Vim mode, styled as Helix (a Helix keymap of our own later), rather than writing an editor. Diagnostics, hover and completion from the engine, as the language server gives them.

What each needs from ui, to be ui items:

- `terminal`: an interactive mode (a prompt that takes keys, a command handler the page supplies), beside the played session it draws today.
- An editor skin backed by CodeMirror that matches `terminal-editor`'s look (gutter, status line, command line, bufferline, the phosphor palette), so a played edit and a live one are the same picture.
- `terminal-editor`: a diagnostics layer and a bufferline of its own (the site draws both today, components/helix-diagnostics.tsx and the bufferline in components/hero.tsx).
