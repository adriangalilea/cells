// swift-tools-version: 6.2
// Proves the XCFramework is a SwiftPM binaryTarget: `swift run -c release cells-swift-check
// <file>` reads a file through the C ABI and walks its columns through the Arrow C Data
// Interface. Build the framework first: rust/cells-ffi/build-apple.sh.

import PackageDescription

let package = Package(
    name: "cells-swift-check",
    platforms: [.macOS(.v26), .iOS(.v26)],
    targets: [
        .binaryTarget(name: "CellsCore", path: "../../target/apple/CellsCore.xcframework"),
        .executableTarget(name: "cells-swift-check", dependencies: ["CellsCore"]),
    ]
)
