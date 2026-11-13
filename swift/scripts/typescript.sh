#!/usr/bin/env bash
# CellsScripts' TypeScript, fetched whole from the `typescript` npm package (Apache-2.0)
# at the version and tarball digest pinned here: its language service (lib/typescript.js,
# run as it ships in JavaScriptCore) and the standard library declarations a script
# targets (ES2022, no DOM: what a script uses of the web is cells-script.d.ts's), with its
# licence and notices. Nothing here is edited by hand: change the pin and run it again.
# 6.x is the last line with the language service in JavaScript (7 is the native compiler).
#
#   swift/scripts/typescript.sh
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

version=6.0.3
sha256=33cd0ee1beaa8c9e9d15a9da836c62ddea4c34a42d7c2d349dbc80d94165d22a
target=Sources/CellsScripts/TypeScript

work=$(mktemp -d)
trap 'trash "$work"' EXIT
(cd "$work" && npm pack --silent "typescript@$version" > /dev/null)
tarball="$work/typescript-$version.tgz"
got=$(shasum -a 256 "$tarball" | cut -d' ' -f1)
[ "$got" = "$sha256" ] || { echo "typescript: $version's tarball is $got, pinned $sha256" >&2; exit 1; }
tar -xzf "$tarball" -C "$work"
package="$work/package"
[ "$(jq -r .version "$package/package.json")" = "$version" ] || { echo "typescript: not $version" >&2; exit 1; }

[ -e "$target" ] && trash "$target"
mkdir -p "$target"
cp "$package/lib/typescript.js" "$package/LICENSE.txt" "$package/ThirdPartyNoticeText.txt" "$target/"
# the ES2022 chain without the DOM (the `full` libs are ES plus the DOM)
for lib in "$package"/lib/lib.es5.d.ts "$package"/lib/lib.es20*.d.ts "$package"/lib/lib.decorators*.d.ts; do
  case "$lib" in *full*|*es2023*|*es2024*|*es2025*|*esnext*) continue ;; esac
  cp "$lib" "$target/"
done
echo "$version" > "$target/VERSION"
echo "typescript: $version into $target ($(du -sh "$target" | cut -f1))"
