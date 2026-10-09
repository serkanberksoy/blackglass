#!/usr/bin/env bash
# Builds the Windows release on Linux, cross-compiled with MinGW, checked
# under Wine when it's there, and packed into dist/ as a zip with the
# README, license, version history and the example vault:
#   blackglass.exe          the terminal version, and the window (--gui)
#   blackglass-window.exe   the window without a console (double-click it)
#
#   scripts/release-windows.sh        # dist/blackglass-<version>-x86_64-windows.zip
#
# Needs: rustup's x86_64-pc-windows-gnu target and MinGW
# (Fedora: dnf install mingw64-gcc mingw64-winpthreads-static); zip.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v cargo >/dev/null || export PATH="$HOME/.cargo/bin:$PATH"

target=x86_64-pc-windows-gnu
rustup target add "$target" >/dev/null
cargo build --release --target "$target" --features windows-launcher

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
exe="target/$target/release/blackglass.exe"
if command -v wine >/dev/null; then
  WINEDEBUG=-all wine "$exe" --version | grep -q "$version" \
    || { echo "release: $exe isn't $version" >&2; exit 1; }
fi

name="blackglass-$version-x86_64-windows"
rm -rf "dist/$name" "dist/$name.zip" && mkdir -p "dist/$name"
cp "$exe" "target/$target/release/blackglass-window.exe" "dist/$name/"
cp README.md LICENSE VERSION.md "dist/$name/"
git archive HEAD example_vault | tar -x -C "dist/$name"
(cd dist && zip -qr "$name.zip" "$name")
(cd dist && sha256sum "$name.zip" > "$name.zip.sha256")
echo "dist/$name.zip"
