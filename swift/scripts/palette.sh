#!/usr/bin/env bash
# The Cells palette (a column's colour, the muted ink, the computed one) written into
# Sources/Cells/Palette.swift from syntaxes/palette.json, its one source. Run after a change
# to the palette; the file it writes is never edited by hand.
#
#   swift/scripts/palette.sh
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
from=../syntaxes/palette.json
hex() { jq -r "$1" "$from" | tr -d '#'; }
{
  echo "// Written by swift/scripts/palette.sh from syntaxes/palette.json: never edited here."
  echo
  echo "extension Cells {"
  echo "    /// A colour for a light ground and one for a dark, as 0xRRGGBB."
  echo "    public struct Ink: Sendable, Equatable {"
  echo "        public let light: UInt32"
  echo "        public let dark: UInt32"
  echo "    }"
  echo
  echo "    /// The Cells palette: colour means a column, six in turn (\`column % 6\`), what"
  echo "    /// describes a column rather than holds it is muted, and a value a formula gave is"
  echo "    /// computed. The site's and an editor's."
  echo "    public enum Palette {"
  echo "        public static let columns: [Ink] = ["
  n=$(jq '.columns | length' "$from")
  for i in $(seq 0 $((n - 1))); do
    echo "            Ink(light: 0x$(hex ".columns[$i].light"), dark: 0x$(hex ".columns[$i].dark")),"
  done
  echo "        ]"
  echo "        public static let muted = Ink(light: 0x$(hex .muted.light), dark: 0x$(hex .muted.dark))"
  echo "        public static let computed = Ink(light: 0x$(hex .computed.light), dark: 0x$(hex .computed.dark))"
  echo "    }"
  echo "}"
} > Sources/Cells/Palette.swift
echo "palette: $(jq '.columns | length' "$from") columns into Sources/Cells/Palette.swift"
