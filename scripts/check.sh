#!/usr/bin/env bash
# Quality gate: must pass before a feature is considered done.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v cargo >/dev/null || export PATH="$HOME/.cargo/bin:$PATH"

echo "==> cargo fmt --check"
cargo fmt --check
echo "==> cargo clippy --all-targets -- -D warnings"
cargo clippy --quiet --all-targets -- -D warnings
echo "==> cargo clippy --no-default-features (the terminal-only build)"
cargo clippy --quiet --no-default-features -- -D warnings
# Windows keeps compiling (where its target is installed).
if rustup target list --installed 2>/dev/null | grep -q x86_64-pc-windows-gnu; then
  echo "==> cargo clippy --target x86_64-pc-windows-gnu (Windows, both programs)"
  cargo clippy --quiet --target x86_64-pc-windows-gnu --features windows-launcher -- -D warnings
fi
echo "==> cargo test"
cargo test --quiet
echo "==> cargo test -p crossterm --lib parse (the vendored crossterm's parser)"
cargo test --quiet -p crossterm --lib parse
echo "OK: format, lints and tests all pass"
