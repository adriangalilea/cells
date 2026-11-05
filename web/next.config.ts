import type { NextConfig } from "next"

/** A static site: every page is HTML at build, the engine runs in the reader's browser as
 *  WebAssembly, and nothing needs a server. `out/` is what the Worker serves. */
const nextConfig: NextConfig = {
  output: "export",
  trailingSlash: false,
  images: { unoptimized: true },
  // Paid @ag items ship built and bundled, never as source: no browser source maps.
  productionBrowserSourceMaps: false,
}

export default nextConfig
