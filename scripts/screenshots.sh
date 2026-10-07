#!/usr/bin/env bash
# The README's screenshots and demo GIF, drawn by blackglass itself
# (tests/screenshots.rs drives the example vault and writes SVGs), turned
# into PNGs and a GIF in documentation/images/.
#
#   scripts/screenshots.sh
#
# Needs: a Chromium browser (SVG to PNG, color emoji included; brave-browser
# or chromium), ImageMagick's magick (the GIF), and the JetBrainsMono Nerd
# Font and Noto Color Emoji fonts.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v cargo >/dev/null || export PATH="$HOME/.cargo/bin:$PATH"

browser=$(command -v chromium || command -v chromium-browser || command -v brave-browser || command -v google-chrome)
# An SVG as a PNG of its own size.
png() {
  local size
  size=$(grep -o -m1 'width="[0-9.]*" height="[0-9.]*"' "$1" | tr -dc '0-9 ' | awk '{print int($1+0.5)","int($2+0.5)}')
  "$browser" --headless=new --disable-gpu --no-sandbox --hide-scrollbars \
    --window-size="$size" --screenshot="$2" "file://$(realpath "$1")" >/dev/null 2>&1
}

cargo test -q --test screenshots -- --ignored
out=documentation/images
for svg in "$out"/*.svg; do
  png "$svg" "${svg%.svg}.png"
done
# The demo: each frame named NNN-<hundredths of a second it shows>.svg.
frames=$(mktemp -d)
args=()
for svg in "$out"/demo-frames/*.svg; do
  name=$(basename "$svg" .svg)
  png="$frames/$name.png"
  png "$svg" "$png"
  args+=(-delay "${name#*-}" "$png")
done
magick "${args[@]}" -resize 80% -loop 0 -layers Optimize "$out/demo.gif"
rm -rf "$frames" "$out"/demo-frames "$out"/*.svg
ls -la "$out"
