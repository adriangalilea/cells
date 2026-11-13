#!/usr/bin/env bash
# What Cells.app carries, in rust/target/apple/engine, for the Mac (arm64, all macOS 27 runs
# on): the engine for CellsEngine.app, a static library with its header and module map; and
# for Contents/Helpers (the app signs them) the `cells` command and `cells-sql`, which holds
# DuckDB and runs `cells sql`. Incremental: an unchanged engine builds in seconds.
#
#   rust/cells-daemon/build-apple.sh
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$here/.."
cd "$root"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
# The deployment target of the app that links it.
export MACOSX_DEPLOYMENT_TARGET=27.0
t=aarch64-apple-darwin
# a target this toolchain cannot build stops the build, in rustup's own words
if ! said=$(rustup target add "$t" 2>&1); then
  echo "build-apple.sh: rustup cannot add $t:" >&2
  echo "$said" >&2
  exit 1
fi
cargo build -q --profile apple -p cells-daemon --lib --target "$t"
out="$root/target/apple/engine"
mkdir -p "$out/include"
cp "$here/include/cells_engine.h" "$here/include/module.modulemap" "$out/include/"
cp "$root/target/$t/apple/libcells_daemon.a" "$out/libcells_daemon.a"
echo "libcells_daemon.a: $(( $(stat -f %z "$out/libcells_daemon.a") / 1024 )) KB, in $out"
cargo build -q --profile apple -p cells-cli -p cells-sql
for bin in cells cells-sql; do
  strip -S -x -o "$out/$bin" "$root/target/apple/$bin" 2> /dev/null
  echo "$bin: $(( $(stat -f %z "$out/$bin") / 1024 )) KB, in $out"
done
