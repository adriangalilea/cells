#!/usr/bin/env bash
# The core for the Apple platforms: static libraries for macOS (arm64 and x86_64, one
# universal slice), iOS, tvOS and their simulators (arm64), the C header from cbindgen, all
# in rust/target/apple/TCSVCore.xcframework, a SwiftPM binaryTarget, and that framework
# zipped as SwiftPM downloads it (TCSVCore.xcframework.zip) with its checksum beside it.
# Prints each slice's size as built and as a stripped link would carry it.
#
# No watchOS slice: a watch running watchOS 26 is arm64_32 (Series 6 to 8, SE), which Rust
# builds only on nightly (tier 3). No visionOS slice: nothing that links it claims visionOS.
#
#   rust/tcsv-ffi/build-apple.sh
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$here/.."
cd "$root"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
# The deployment targets the apps that link it build for.
export MACOSX_DEPLOYMENT_TARGET=26.0 IPHONEOS_DEPLOYMENT_TARGET=26.0 TVOS_DEPLOYMENT_TARGET=26.0
targets=(aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-tvos aarch64-apple-tvos-sim)
for t in "${targets[@]}"; do
  rustup target add "$t" > /dev/null 2>&1
  cargo build -q --profile apple -p tcsv-ffi --lib --target "$t"
done

out="$root/target/apple"
rm -rf "$out"
mkdir -p "$out/headers" "$out/macos"
cbindgen -q --config "$here/cbindgen.toml" --crate tcsv-ffi --output "$out/headers/tcsv.h" "$here"
# At the top of Headers, where SwiftPM's explicit module build finds it (a map in a
# subfolder, which keeps Xcode from merging it with another framework's, is not found).
cp "$here/module.modulemap" "$out/headers/module.modulemap"
size() { stat -f %z "$1"; }
kb() { echo "$(( $1 / 1024 )) KB"; }
# Debug info and local symbols out of each library (std's included); the global symbols a
# link resolves stay.
for t in "${targets[@]}"; do
  a="$root/target/$t/apple/libtcsv_ffi.a"
  mkdir -p "$out/$t"
  strip -S -x -o "$out/$t/libtcsv_ffi.a" "$a" 2> /dev/null
  echo "$t: $(kb "$(size "$a")") built, $(kb "$(size "$out/$t/libtcsv_ffi.a")") stripped"
done
lipo -create "$out/aarch64-apple-darwin/libtcsv_ffi.a" "$out/x86_64-apple-darwin/libtcsv_ffi.a" -output "$out/macos/libtcsv_ffi.a"
slices=()
for lib in macos aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-tvos aarch64-apple-tvos-sim; do
  slices+=(-library "$out/$lib/libtcsv_ffi.a" -headers "$out/headers")
done
xcodebuild -create-xcframework "${slices[@]}" -output "$out/TCSVCore.xcframework" > /dev/null
echo "TCSVCore.xcframework: $(du -sk "$out/TCSVCore.xcframework" | cut -f1) KB"
# The archive a binaryTarget(url:checksum:) downloads: the framework at its top.
(cd "$out" && ditto -c -k --sequesterRsrc --keepParent TCSVCore.xcframework TCSVCore.xcframework.zip)
shasum -a 256 "$out/TCSVCore.xcframework.zip" | cut -d' ' -f1 > "$out/TCSVCore.xcframework.zip.sha256"
echo "TCSVCore.xcframework.zip: $(kb "$(size "$out/TCSVCore.xcframework.zip")"), sha256 $(cat "$out/TCSVCore.xcframework.zip.sha256")"
