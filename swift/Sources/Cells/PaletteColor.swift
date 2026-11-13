#if canImport(SwiftUI)
    import SwiftUI

    extension Cells.Ink {
        /// The ink as a colour that follows the appearance: its light one on a light ground,
        /// its dark one on a dark. The table's heads, the chart's marks and a file's text
        /// all draw a column in it.
        public var color: Color {
            func rgb(_ v: UInt32) -> (Double, Double, Double) {
                (Double(v >> 16 & 0xff) / 255, Double(v >> 8 & 0xff) / 255, Double(v & 0xff) / 255)
            }
            let (l, d) = (rgb(light), rgb(dark))
            #if os(macOS)
                return Color(
                    nsColor: NSColor(name: nil) { appearance in
                        let dark = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
                        let c = dark ? d : l
                        return NSColor(srgbRed: c.0, green: c.1, blue: c.2, alpha: 1)
                    })
            #else
                return Color(
                    uiColor: UIColor { traits in
                        let c = traits.userInterfaceStyle == .dark ? d : l
                        return UIColor(red: c.0, green: c.1, blue: c.2, alpha: 1)
                    })
            #endif
        }
    }

    extension Cells.Palette {
        /// Column `i`'s colour.
        public static func column(_ i: Int) -> Color { columns[i % columns.count].color }
    }
#endif
