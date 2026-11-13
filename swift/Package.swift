// swift-tools-version: 6.4
// Cells in Swift: the binding over the engine (rust/) behind its C ABI, and only the binding:
// a table or a chart drawn over it belongs to the app that draws it. Depended on by path from
// a checkout, so it always links the engine built from that same checkout: build it first
// with rust/cells-ffi/build-apple.sh (an app's build runs it when the engine changed).
import Foundation
import PackageDescription

let core = "../rust/target/apple/CellsCore.xcframework"
guard
    FileManager.default.fileExists(
        atPath: URL(filePath: #filePath).deletingLastPathComponent().appending(path: core).path)
else {
    fatalError("cells: the engine is not built; run rust/cells-ffi/build-apple.sh")
}

let package = Package(
    name: "cells",
    platforms: [.macOS(.v27)],
    products: [
        .library(name: "Cells", targets: ["Cells"]),
        // A script's TypeScript (SPEC.md, Tables a script fills) read as TypeScript reads it: its language
        // service in JavaScriptCore, in process, with what a script sees declared beside it
        // (Script/cells-script.d.ts), so an editor completes and explains a script as the
        // engine will run it.
        .library(name: "CellsScripts", targets: ["CellsScripts"]),
    ],
    targets: [
        .binaryTarget(name: "CellsCore", path: core),
        .target(name: "Cells", dependencies: ["CellsCore"]),
        .target(name: "CellsScripts", resources: [.copy("TypeScript"), .copy("Script")]),
        // The binding's laws, and the conformance suite read from tests/ where it lives.
        .testTarget(name: "CellsTests", dependencies: ["Cells"]),
        .testTarget(name: "CellsScriptsTests", dependencies: ["CellsScripts"]),
    ]
)
