// Installs @ag items from a ui checkout: its built registry (`public/r/*.json`) served on
// :4111, where components.json's `@ag` points, and that checkout's own shadcn, whose
// installer keeps every file whole (ui's patch: stock shadcn drops a file's opening
// comments, the licence line of a paid item among them). With no items named, the paid
// ones this site uses, which git ignores here (Adrian holds their licence; their source
// never enters this repository): run it before a build on a fresh clone.
//
//   UI=~/Developer/ui bun scripts/ui.ts [item...]

const PAID = ["macos", "player", "table"]

const ui = process.env.UI
if (!ui) throw new Error("UI must name a ui checkout (its public/r is served, its shadcn runs)")
if (!(await Bun.file(`${ui}/public/r/registry.json`).exists())) throw new Error(`${ui}/public/r holds no built registry`)
const shadcn = `${ui}/node_modules/.bin/shadcn`
if (!(await Bun.file(shadcn).exists())) throw new Error(`${shadcn} is missing: pnpm install in ${ui}`)

const server = Bun.serve({
  port: 4111,
  async fetch(req) {
    const path = new URL(req.url).pathname
    if (!/^\/r\/[a-z0-9-]+\.json$/.test(path)) return new Response("not found", { status: 404 })
    const file = Bun.file(`${ui}/public${path}`)
    if (!(await file.exists())) return new Response(`${path} is no item of ${ui}`, { status: 404 })
    return new Response(file, { headers: { "content-type": "application/json" } })
  },
})

const items = process.argv.slice(2)
const named = (items.length ? items : PAID).map((i) => `@ag/${i}`)
const run = Bun.spawnSync([shadcn, "add", ...named, "--yes", "--overwrite"], {
  cwd: `${import.meta.dir}/..`,
  stdout: "inherit",
  stderr: "inherit",
})
server.stop()
if (run.exitCode !== 0) throw new Error(`shadcn add exited ${run.exitCode}`)
