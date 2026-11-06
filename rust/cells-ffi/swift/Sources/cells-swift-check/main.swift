// Reads a .cells file through the core's C ABI and walks its columns through the Arrow C
// Data Interface, no copy: the rows, the problems, each column's Arrow format and nulls,
// and the sum of every decimal column, read from the shared buffers.

import Foundation
import CellsCore

func string(_ s: CellsStr) -> String {
    String(decoding: UnsafeRawBufferPointer(start: s.ptr, count: s.len), as: UTF8.self)
}

let path = CommandLine.arguments.dropFirst().first ?? { fatalError("cells-swift-check <file>") }()
let bytes = try Data(contentsOf: URL(fileURLWithPath: path))
let threads = ProcessInfo.processInfo.activeProcessorCount
var failure = CellsFailure()
let start = Date()
let doc: OpaquePointer = bytes.withUnsafeBytes { raw in
    guard let d = cells_read(raw.bindMemory(to: UInt8.self).baseAddress, raw.count, threads, &failure) else {
        let why = String(decoding: UnsafeRawBufferPointer(start: failure.why, count: failure.why_len), as: UTF8.self)
        fatalError("\(path) does not read (\(failure.at), \(failure.number)): \(why)")
    }
    return d
}
let read = Date().timeIntervalSince(start)
print("\(cells_rows(doc)) rows, \(cells_problem_count(doc)) problems, read in \(String(format: "%.1f", read * 1000)) ms on \(threads) threads")
if cells_problem_count(doc) > 0 { print("  first: \(string(cells_describe(doc, 0)))") }

var schema = ArrowSchema()
var array = ArrowArray()
cells_export(doc, &schema, &array)
cells_free(doc)  // the exported arrays keep the buffers they point into

for k in 0..<Int(array.n_children) {
    let field = schema.children[k]!.pointee
    let column = array.children[k]!.pointee
    let format = String(cString: field.format)
    var line = "  \(String(cString: field.name)): \(format), \(column.null_count) nulls"
    if format.hasPrefix("d:38,") {
        // Decimal128: little-endian i128 at the column's scale; the low word is enough here.
        let scale = Int(format.split(separator: ",")[1])!
        let values = column.buffers[1]!.assumingMemoryBound(to: Int64.self)
        let valid = column.buffers[0]?.assumingMemoryBound(to: UInt8.self)
        var sum = 0.0
        for i in 0..<Int(column.length) where valid.map({ $0[i >> 3] >> UInt8(i & 7) & 1 == 1 }) ?? true {
            sum += Double(values[2 * i])
        }
        line += String(format: ", sum %.2f", sum / pow(10, Double(scale)))
    }
    print(line)
}
schema.release(&schema)
array.release(&array)
