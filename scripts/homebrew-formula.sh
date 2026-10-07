#!/usr/bin/env bash
# Writes the Homebrew formula for this version of blackglass (and the
# mdedit it builds against) from packaging/homebrew/blackglass.rb.in, with
# the checksums of their release tags' source archives on GitHub. Run it
# after the release's tags are pushed:
#
#   scripts/homebrew-formula.sh > ../homebrew-tap/Formula/blackglass.rb
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
mdedit=$(sed -n 's/^version = "\(.*\)"/\1/p' ../mdedit/Cargo.toml | head -1)
sha() {
  curl -fsSL "https://github.com/serkanberksoy/$1/archive/refs/tags/v$2.tar.gz" | sha256sum | cut -d' ' -f1
}
blackglass_sha=$(sha blackglass "$version") || { echo "no tag v$version of blackglass on GitHub yet" >&2; exit 1; }
mdedit_sha=$(sha mdedit "$mdedit") || { echo "no tag v$mdedit of mdedit on GitHub yet" >&2; exit 1; }
sed -e "s/@VERSION@/$version/" -e "s/@SHA256@/$blackglass_sha/" \
    -e "s/@MDEDIT_VERSION@/$mdedit/" -e "s/@MDEDIT_SHA256@/$mdedit_sha/" \
    packaging/homebrew/blackglass.rb.in
