#!/usr/bin/env bash
# Builds the Linux release: one static x86_64 binary (musl, so it runs on
# any distribution), checked, and packed into dist/.
#
#   scripts/release-linux.sh          # dist/blackglass-<version>-x86_64-linux{,.tar.gz} and SHA256SUMS
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
cargo build --release --target "$target"

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
(cd dist && sha256sum "$name.tar.gz" blackglass > SHA256SUMS)
echo "dist/$name.tar.gz, dist/blackglass, dist/SHA256SUMS"
