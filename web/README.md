# cellular.computer

The Cells site: the landing, every example live on the real engine (this repository's wasm build, copied into `lib/engine`), and the docs at `/docs`, drawn from this repository's own markdown. Built from ui's @ag items, served as static files by a Cloudflare Worker.

| | |
|---|---|
| `lib/story.ts` | every step's words and files, as data |
| `lib/hero.ts` | the ten seconds on a Mac, as data: the edit, the terminal's session, one clock |
| `lib/product.ts` | the name, the extension, the command: change them here only |
| `lib/docs.ts` | the docs: which files, where each lives on the site, SPEC.md cut a page per section, every link resolved at build (a broken one throws) |
| `scripts/examples.ts` | reads every file through the engine and fails on any claim it does not back |
| `scripts/docs.ts` | builds every docs page and reads every `cells` block with the engine: it must read whole, and a `cells draft` must not yet |

```bash
UI=~/Developer/ui pnpm ui                 # first, every time: each @ag item as ui main has it
pnpm install
pnpm engine                               # the engine and grammar from this repo, after they change
pnpm dev                                  # :3200; ?still=0.5 holds the hero at a moment
pnpm check                                # ui's items fresh, types, the story's claims, the docs
pnpm deploy                               # next build, then wrangler (workers.dev)
```

`pnpm ui` installs every item in `scripts/ui-items.ts` from the ui checkout, and `pnpm check` fails while any installed file differs from that checkout's registry (`scripts/ui-fresh.ts`), so a stale copy cannot ship. A patch the site needs inside an installed file sits between `// site-patch: <why>` and `// end site-patch`, named on every check, and goes when ui ships its replacement.

Paid and studio items are never committed (`.gitignore`): the licence keeps their source out of any repository that may become public.
