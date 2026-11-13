#!/usr/bin/env bash
# The engine behind its C header for the Swift binding (swift/Package.swift): a static
# library for the Mac (arm64, all macOS 27 runs on), the header from cbindgen and its module
# map, as rust/target/apple/CellsCore.xcframework. Incremental: seconds after an edit.
# Another platform is a slice added here when an app on it links the binding.
#
#   rust/cells-ffi/build-apple.sh
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$here/.."
cd "$root"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
# The deployment target of the apps that link it.
export MACOSX_DEPLOYMENT_TARGET=27.0
t=aarch64-apple-darwin
# a target this toolchain cannot build stops the build, in rustup's own words
if ! said=$(rustup target add "$t" 2>&1); then
  echo "build-apple.sh: rustup cannot add $t:" >&2
  echo "$said" >&2
  exit 1
fi
cargo build -q --profile apple -p cells-ffi --lib --target "$t"
out="$root/target/apple"
mkdir -p "$out/headers"
cbindgen -q --config "$here/cbindgen.toml" --crate cells-ffi --output "$out/headers/cells.h" "$here"
# At the top of Headers, where SwiftPM's explicit module build finds it.
cp "$here/module.modulemap" "$out/headers/module.modulemap"
# Debug info and local symbols out (std's included); the global symbols a link resolves stay.
strip -S -x -o "$out/libcells_ffi.a" "$root/target/$t/apple/libcells_ffi.a" 2> /dev/null
rm -rf "$out/CellsCore.xcframework"
xcodebuild -create-xcframework -library "$out/libcells_ffi.a" -headers "$out/headers" \
  -output "$out/CellsCore.xcframework" > /dev/null
echo "CellsCore.xcframework: $out/CellsCore.xcframework"
