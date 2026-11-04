#!/usr/bin/env bash
# The core for the Apple platforms: static libraries for macOS (arm64 and x86_64, one
# universal slice), iOS and the iOS simulator (arm64), the C header from cbindgen, all in
# rust/target/apple/TCSVCore.xcframework, a SwiftPM binaryTarget. Prints each slice's size
# as built and as a stripped link would carry it.
#
#   rust/tcsv-ffi/build-apple.sh
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$here/.."
cd "$root"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
# The deployment targets the apps that link it build for.
export MACOSX_DEPLOYMENT_TARGET=26.0 IPHONEOS_DEPLOYMENT_TARGET=26.0
targets=(aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim)
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
xcodebuild -create-xcframework \
  -library "$out/macos/libtcsv_ffi.a" -headers "$out/headers" \
  -library "$out/aarch64-apple-ios/libtcsv_ffi.a" -headers "$out/headers" \
  -library "$out/aarch64-apple-ios-sim/libtcsv_ffi.a" -headers "$out/headers" \
  -output "$out/TCSVCore.xcframework" > /dev/null
echo "TCSVCore.xcframework: $(du -sk "$out/TCSVCore.xcframework" | cut -f1) KB"
