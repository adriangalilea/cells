// Every installed @ag file is the ui checkout's, byte for byte: each item the site uses
// (scripts/ui-items) and everything it depends on, read from the checkout's built
// registry (`public/r/<item>.json`, tracked in ui), its imports moved where the installer
// moves them, compared with the file here. A difference fails `pnpm check`, so a stale
// copy never ships: run `pnpm ui` and build again.
//
// A site patch inside an installed file is a block between `// site-patch: <why>` and
// `// end site-patch`, stripped before comparing: it is named, and any other difference
// still fails. Delete the block when ui ships what it stands in for.
//
//   UI=~/Developer/ui bun scripts/ui-fresh.ts

import { UI_ITEMS } from "./ui-items"

const ui = process.env.UI ?? `${process.env.HOME}/Developer/ui`
const site = `${import.meta.dir}/..`

interface Item {
  name: string
  registryDependencies?: string[]
  files: { path: string; type: string; target?: string; content: string }[]
}

/** Where the installer writes a registry file. */
function destOf(f: Item["files"][number]): string {
  if (f.target) return f.target
  const m = /^registry\/base-nova\/(ui|lib|hooks)\/(.+)$/.exec(f.path)
  if (!m) throw new Error(`ui-fresh: no rule for where ${f.path} installs`)
  return `${m[1] === "ui" ? "components/ui" : m[1]}/${m[2]}`
}

/** A registry file's text as the installer writes it: its imports at the site's paths. */
const asInstalled = (text: string) => text.replaceAll("@/registry/base-nova/ui/", "@/components/ui/").replaceAll("@/registry/base-nova/lib/", "@/lib/").replaceAll("@/registry/base-nova/hooks/", "@/hooks/")

const PATCH = /^[ \t]*\/\/ site-patch: (.+)\n[\s\S]*?^[ \t]*\/\/ end site-patch\n/gm

const seen = new Set<string>()
const stale: string[] = []
const patches: string[] = []
let files = 0
async function visit(name: string) {
  if (seen.has(name)) return
  seen.add(name)
  const json = Bun.file(`${ui}/public/r/${name}.json`)
  if (!(await json.exists())) throw new Error(`ui-fresh: ${name} is no item of ${ui}/public/r`)
  const item = (await json.json()) as Item
  for (const f of item.files) {
    const dest = destOf(f)
    installed.add(dest)
    const here = Bun.file(`${site}/${dest}`)
    if (!(await here.exists())) {
      stale.push(`${dest} (${name}) is not installed`)
      continue
    }
    files++
    const text = await here.text()
    for (const m of text.matchAll(PATCH)) patches.push(`${dest}: ${m[1]}`)
    if (text.replace(PATCH, "") !== asInstalled(f.content)) stale.push(`${dest} (${name}) differs from ui's`)
  }
  for (const d of item.registryDependencies ?? []) if (d.startsWith("@ag/")) await visit(d.slice(4))
}

const installed = new Set<string>()
for (const name of UI_ITEMS) await visit(name)
// components/ui is ui's alone: a file there that no item installs is left from an item
// renamed or dropped, and goes (lib/ holds the site's own files too, so it is not asked).
for (const file of new Bun.Glob("*").scanSync(`${site}/components/ui`))
  if (!installed.has(`components/ui/${file}`)) stale.push(`components/ui/${file} is no item's: trash it`)
for (const p of patches) console.log(`! site patch in ${p}`)
if (stale.length) {
  for (const s of stale) console.error(`✗ ${s}`)
  console.error(`${stale.length} installed ui file(s) are not ui's: UI=${ui} pnpm ui`)
  process.exit(1)
}
console.log(`✓ ui: ${files} installed files are ui's, from ${seen.size} items`)
