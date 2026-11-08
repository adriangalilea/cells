#!/usr/bin/env bash
# What Cells.app carries, in rust/target/apple/engine: the engine for CellsEngine.app, one
# static library for macOS (arm64 and x86_64, one universal slice) with its header and module
# map; and the `cells` command for Contents/Helpers, for this Mac's architecture (the app
# signs it). macOS only: on iOS an app holds the engine in its own process.
#
#   rust/cells-daemon/build-apple.sh
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$here/.."
cd "$root"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
# The deployment target of the app that links it.
export MACOSX_DEPLOYMENT_TARGET=27.0
targets=(aarch64-apple-darwin x86_64-apple-darwin)
for t in "${targets[@]}"; do
  rustup target add "$t" > /dev/null 2>&1
  cargo build -q --profile apple -p cells-daemon --lib --target "$t"
done
out="$root/target/apple/engine"
rm -rf "$out"
mkdir -p "$out/include"
cp "$here/include/cells_engine.h" "$here/include/module.modulemap" "$out/include/"
lipo -create "$root/target/aarch64-apple-darwin/apple/libcells_daemon.a" "$root/target/x86_64-apple-darwin/apple/libcells_daemon.a" -output "$out/libcells_daemon.a"
echo "libcells_daemon.a: $(( $(stat -f %z "$out/libcells_daemon.a") / 1024 )) KB, in $out"
cargo build -q --profile apple -p cells-cli
strip -S -x -o "$out/cells" "$root/target/apple/cells" 2> /dev/null
echo "cells: $(( $(stat -f %z "$out/cells") / 1024 )) KB, in $out"
