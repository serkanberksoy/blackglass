#!/usr/bin/env bash
# Builds the Linux release: one static x86_64 binary (musl, so it runs on
# any distribution), the terminal version (no window: `--no-default-features`),
# checked, and packed into dist/.
#
#   scripts/release-linux.sh          # dist/blackglass-<version>-x86_64-linux{,.tar.gz},
#                                     # …-linux-gui.tar.gz (the window) and SHA256SUMS
#
# Needs: rustup's x86_64-unknown-linux-musl target and gcc (for mimalloc,
# the static build's allocator; see Cargo.toml).
set -euo pipefail
cd "$(dirname "$0")/.."
command -v cargo >/dev/null || export PATH="$HOME/.cargo/bin:$PATH"

target=x86_64-unknown-linux-musl
rustup target add "$target" >/dev/null
# mimalloc is C: the system gcc builds it for musl (C11: glibc's C23
# headers would ask for symbols musl doesn't have).
export CC_x86_64_unknown_linux_musl=gcc
export CFLAGS_x86_64_unknown_linux_musl="-std=gnu11"
# The terminal build: the window (the gui feature) needs the system's
# graphics libraries, which a static program can't load.
cargo build --release --target "$target" --no-default-features

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
bin="target/$target/release/blackglass"
"$bin" --version | grep -q "$version" || { echo "release: $bin isn't $version" >&2; exit 1; }
file "$bin" | grep -q "static" || { echo "release: $bin isn't static" >&2; exit 1; }

name="blackglass-$version-x86_64-linux"
rm -rf "dist/$name" && mkdir -p "dist/$name"
cp "$bin" "dist/$name/blackglass"
cp README.md LICENSE VERSION.md "dist/$name/"
# The example vault as git keeps it (the program has it built in too:
# blackglass --example).
git archive HEAD example_vault | tar -x -C "dist/$name"
tar -C dist -czf "dist/$name.tar.gz" "$name"
cp "$bin" "dist/$name"-bin && mv "dist/$name"-bin "dist/blackglass"

# The window build (the gui feature): for this system's C library (glibc),
# with the desktop launcher and icons (scripts/install-desktop.sh adds
# them to the applications menu).
cargo build --release
gui_bin=target/release/blackglass
"$gui_bin" --version | grep -q "$version" || { echo "release: $gui_bin isn't $version" >&2; exit 1; }
gui="blackglass-$version-x86_64-linux-gui"
rm -rf "dist/$gui" && mkdir -p "dist/$gui/scripts" "dist/$gui/packaging"
cp "$gui_bin" "dist/$gui/blackglass"
cp README.md LICENSE VERSION.md "dist/$gui/"
cp scripts/install-desktop.sh "dist/$gui/scripts/"
cp -r packaging/linux packaging/icon "dist/$gui/packaging/"
tar -C dist -czf "dist/$gui.tar.gz" "$gui"

(cd dist && sha256sum "$name.tar.gz" blackglass "$gui.tar.gz" > SHA256SUMS)
echo "dist/$name.tar.gz, dist/blackglass, dist/$gui.tar.gz, dist/SHA256SUMS"
