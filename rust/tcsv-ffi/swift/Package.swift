// swift-tools-version: 6.2
// Proves the XCFramework is a SwiftPM binaryTarget: `swift run -c release tcsv-swift-check
// <file>` reads a file through the C ABI and walks its columns through the Arrow C Data
// Interface. Build the framework first: rust/tcsv-ffi/build-apple.sh.

import PackageDescription

let package = Package(
    name: "tcsv-swift-check",
    platforms: [.macOS(.v26), .iOS(.v26)],
    targets: [
        .binaryTarget(name: "TCSVCore", path: "../../target/apple/TCSVCore.xcframework"),
        .executableTarget(name: "tcsv-swift-check", dependencies: ["TCSVCore"]),
    ]
)
