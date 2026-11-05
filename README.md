# cellular.computer

The Cells site: one page, every example live on the real engine (the wasm build, copied into `lib/engine`), built from ui's @ag items, served as static files by a Cloudflare Worker.

| | |
|---|---|
| `lib/story.ts` | every step's words and files, as data |
| `lib/hero.ts` | the ten seconds on a Mac, as data: the edit, the terminal's session, one clock |
| `lib/product.ts` | the name, the extension, the command: change them here only |
| `scripts/examples.ts` | reads every file through the engine and fails on any claim it does not back |

```bash
pnpm install
UI=~/Developer/ui pnpm ui                 # the paid @ag items git ignores (macos, player, table)
CELLS=~/Developer/tcsv pnpm engine        # the engine and grammar, after they change
pnpm dev                                  # :3200; ?still=0.5 holds the hero at a moment
pnpm check                                # types and the story's claims
pnpm deploy                               # next build, then wrangler (workers.dev)
```

Paid items are never committed (`.gitignore`): the licence keeps their source out of any repository that may become public. A fresh clone needs `pnpm ui` before it builds.
