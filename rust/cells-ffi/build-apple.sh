#!/usr/bin/env bash
# The core for the Apple platforms: static libraries for macOS (arm64 and x86_64, one
# universal slice), iOS, tvOS and their simulators (arm64), the C header from cbindgen, all
# in rust/target/apple/CellsCore.xcframework, a SwiftPM binaryTarget, and that framework
# zipped as SwiftPM downloads it (CellsCore.xcframework.zip) with its checksum beside it.
# Prints each slice's size as built and as a stripped link would carry it.
#
# No watchOS slice: a watch running watchOS 26 is arm64_32 (Series 6 to 8, SE), which Rust
# builds only on nightly (tier 3). No visionOS slice: nothing that links it claims visionOS.
#
#   rust/cells-ffi/build-apple.sh
#
# `--mac` builds this Mac's slice alone, incrementally, into rust/target/apple/mac/
# CellsCore.xcframework: what an app built against this checkout links while it is
# developed (apps set CELLS_CORE to it), seconds after an edit, no release in the loop.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$here/.."
cd "$root"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
# The deployment targets the apps that link it build for.
export MACOSX_DEPLOYMENT_TARGET=26.0 IPHONEOS_DEPLOYMENT_TARGET=26.0 TVOS_DEPLOYMENT_TARGET=26.0

# A target this toolchain cannot build stops the build, saying why: rustup's own words.
add() {
  local said
  if ! said=$(rustup target add "$1" 2>&1); then
    echo "build-apple.sh: rustup cannot add $1:" >&2
    echo "$said" >&2
    echo "(a newer stable may have it: rustup update stable)" >&2
    exit 1
  fi
}

if [ "${1:-}" = "--mac" ]; then
  t=aarch64-apple-darwin
  add "$t"
  cargo build -q --profile apple -p cells-ffi --lib --target "$t"
  out="$root/target/apple/mac"
  mkdir -p "$out/headers"
  cbindgen -q --config "$here/cbindgen.toml" --crate cells-ffi --output "$out/headers/cells.h" "$here"
  cp "$here/module.modulemap" "$out/headers/module.modulemap"
  strip -S -x -o "$out/libcells_ffi.a" "$root/target/$t/apple/libcells_ffi.a" 2> /dev/null
  rm -rf "$out/CellsCore.xcframework"
  xcodebuild -create-xcframework -library "$out/libcells_ffi.a" -headers "$out/headers" \
    -output "$out/CellsCore.xcframework" > /dev/null
  echo "CellsCore.xcframework (this Mac): $out/CellsCore.xcframework"
  exit 0
fi

targets=(aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-tvos aarch64-apple-tvos-sim)
for t in "${targets[@]}"; do
  add "$t"
  cargo build -q --profile apple -p cells-ffi --lib --target "$t"
done

out="$root/target/apple"
rm -rf "$out"
mkdir -p "$out/headers" "$out/macos"
cbindgen -q --config "$here/cbindgen.toml" --crate cells-ffi --output "$out/headers/cells.h" "$here"
# At the top of Headers, where SwiftPM's explicit module build finds it (a map in a
# subfolder, which keeps Xcode from merging it with another framework's, is not found).
cp "$here/module.modulemap" "$out/headers/module.modulemap"
size() { stat -f %z "$1"; }
kb() { echo "$(( $1 / 1024 )) KB"; }
# Debug info and local symbols out of each library (std's included); the global symbols a
# link resolves stay.
for t in "${targets[@]}"; do
  a="$root/target/$t/apple/libcells_ffi.a"
  mkdir -p "$out/$t"
  strip -S -x -o "$out/$t/libcells_ffi.a" "$a" 2> /dev/null
  echo "$t: $(kb "$(size "$a")") built, $(kb "$(size "$out/$t/libcells_ffi.a")") stripped"
done
lipo -create "$out/aarch64-apple-darwin/libcells_ffi.a" "$out/x86_64-apple-darwin/libcells_ffi.a" -output "$out/macos/libcells_ffi.a"
slices=()
for lib in macos aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-tvos aarch64-apple-tvos-sim; do
  slices+=(-library "$out/$lib/libcells_ffi.a" -headers "$out/headers")
done
xcodebuild -create-xcframework "${slices[@]}" -output "$out/CellsCore.xcframework" > /dev/null
echo "CellsCore.xcframework: $(du -sk "$out/CellsCore.xcframework" | cut -f1) KB"
# The archive a binaryTarget(url:checksum:) downloads: the framework at its top.
(cd "$out" && ditto -c -k --sequesterRsrc --keepParent CellsCore.xcframework CellsCore.xcframework.zip)
shasum -a 256 "$out/CellsCore.xcframework.zip" | cut -d' ' -f1 > "$out/CellsCore.xcframework.zip.sha256"
echo "CellsCore.xcframework.zip: $(kb "$(size "$out/CellsCore.xcframework.zip")"), sha256 $(cat "$out/CellsCore.xcframework.zip.sha256")"
