// Written by swift/scripts/palette.sh from syntaxes/palette.json: never edited here.

extension Cells {
    /// A colour for a light ground and one for a dark, as 0xRRGGBB.
    public struct Ink: Sendable, Equatable {
        public let light: UInt32
        public let dark: UInt32
    }

    /// The Cells palette: colour means a column, six in turn (`column % 6`), what
    /// describes a column rather than holds it is muted, and a value a formula gave is
    /// computed. The site's and an editor's.
    public enum Palette {
        public static let columns: [Ink] = [
            Ink(light: 0x007a85, dark: 0x43d5dc),
            Ink(light: 0x6b4cae, dark: 0xc3aeff),
            Ink(light: 0x9e4700, dark: 0xf6ab6b),
            Ink(light: 0x107823, dark: 0x8bd28d),
            Ink(light: 0x943b84, dark: 0xeea0dc),
            Ink(light: 0x7e5f00, dark: 0xd6bd5c),
        ]
        public static let muted = Ink(light: 0x57606a, dark: 0x8b949e)
        public static let computed = Ink(light: 0x5d6275, dark: 0x9096aa)
    }
}
