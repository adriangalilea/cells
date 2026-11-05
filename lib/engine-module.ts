// THE ENGINE'S MODULE, compiled before the engine is imported: in a browser streamed from
// `/cells.wasm` (WebAssembly.compileStreaming, compiled while it arrives, compressed on the
// wire), at build and under bun read from the file. `loadEngine()` compiles it once and
// then imports the wrapper, which takes it from `engineModule()`; nothing imports the
// wrapper any other way, so it can never run before its module is there.

type Engine = typeof import("@/lib/engine/cells.ts")

let compiled: WebAssembly.Module | null = null
let loading: Promise<Engine> | null = null

/** The compiled module. Screams if asked before `loadEngine()` has compiled it. */
export function engineModule(): WebAssembly.Module {
  if (!compiled) throw new Error("engine: its module is not compiled yet: reach the engine through loadEngine()")
  return compiled
}

async function compile(): Promise<WebAssembly.Module> {
  if (typeof window !== "undefined") return WebAssembly.compileStreaming(fetch("/cells.wasm"))
  // At build (Node) and under bun: the file in public/, read by path. A bundle for the
  // browser never takes this branch.
  const { readFile } = await import("node:fs/promises")
  const { join } = await import("node:path")
  return new WebAssembly.Module(await readFile(join(process.cwd(), "public/cells.wasm")))
}

/** The engine, once its module is compiled: one load for the page, however many ask. */
export function loadEngine(): Promise<Engine> {
  loading ??= compile().then((m) => {
    compiled = m
    return import("@/lib/engine/cells.ts")
  })
  return loading
}
