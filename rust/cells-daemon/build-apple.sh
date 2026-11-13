#!/usr/bin/env bash
# What Cells.app carries, in rust/target/apple/engine: the engine for CellsEngine.app, one
# static library for macOS (arm64 and x86_64, one universal slice) with its header and module
# map; and for Contents/Helpers, for this Mac's architecture (the app signs them), the
# `cells` command and `cells-sql`, which holds DuckDB and runs `cells sql`. macOS only: on iOS an app holds the engine in its own process.
#
#   rust/cells-daemon/build-apple.sh
#
# `--mac` builds the engine for this Mac's architecture alone: what a development build of
# the app links, half the work of the universal one a release carries.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$here/.."
cd "$root"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
# The deployment target of the app that links it.
export MACOSX_DEPLOYMENT_TARGET=27.0
targets=(aarch64-apple-darwin x86_64-apple-darwin)
[ "${1:-}" = "--mac" ] && targets=(aarch64-apple-darwin)
for t in "${targets[@]}"; do
  # a target this toolchain cannot build stops the build, in rustup's own words
  if ! said=$(rustup target add "$t" 2>&1); then
    echo "build-apple.sh: rustup cannot add $t:" >&2
    echo "$said" >&2
    exit 1
  fi
  cargo build -q --profile apple -p cells-daemon --lib --target "$t"
done
out="$root/target/apple/engine"
rm -rf "$out"
mkdir -p "$out/include"
cp "$here/include/cells_engine.h" "$here/include/module.modulemap" "$out/include/"
libs=()
for t in "${targets[@]}"; do libs+=("$root/target/$t/apple/libcells_daemon.a"); done
lipo -create "${libs[@]}" -output "$out/libcells_daemon.a"
echo "libcells_daemon.a: $(( $(stat -f %z "$out/libcells_daemon.a") / 1024 )) KB, in $out"
cargo build -q --profile apple -p cells-cli -p cells-sql
for bin in cells cells-sql; do
  strip -S -x -o "$out/$bin" "$root/target/apple/$bin" 2> /dev/null
  echo "$bin: $(( $(stat -f %z "$out/$bin") / 1024 )) KB, in $out"
done
