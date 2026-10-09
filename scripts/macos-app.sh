#!/usr/bin/env bash
# Builds blackglass.app (macOS): the window build in an app bundle, with
# its icon, that opens the window (--gui) from the Dock or Finder.
# Run on a Mac (it uses sips and iconutil):
#
#   scripts/macos-app.sh            # dist/blackglass.app (this Mac's arch)
#   scripts/macos-app.sh universal  # Apple Silicon and Intel in one
set -euo pipefail
cd "$(dirname "$0")/.."
command -v cargo >/dev/null || export PATH="$HOME/.cargo/bin:$PATH"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
if [ "${1:-}" = "universal" ]; then
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    rustup target add "$t" >/dev/null
    cargo build --release --target "$t"
  done
  bin=dist/blackglass-universal
  mkdir -p dist
  lipo -create -output "$bin" \
    target/aarch64-apple-darwin/release/blackglass target/x86_64-apple-darwin/release/blackglass
else
  cargo build --release
  bin=target/release/blackglass
fi
app=dist/blackglass.app
rm -rf "$app" && mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/blackglass"
# Launched from the Dock: the window (a terminal still runs `blackglass`
# as it is, from the PATH).
cat > "$app/Contents/MacOS/blackglass-launcher" <<'SH'
#!/bin/sh
exec "$(dirname "$0")/blackglass" --gui "$@"
SH
chmod +x "$app/Contents/MacOS/blackglass-launcher"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist.in > "$app/Contents/Info.plist"
iconset=$(mktemp -d)/blackglass.iconset
mkdir -p "$iconset"
for size in 16 32 64 128 256 512; do
  sips -z $size $size packaging/icon/blackglass-256.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/blackglass.icns"
echo "$app (open it, or drag it to Applications)"
