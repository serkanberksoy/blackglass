#!/usr/bin/env bash
# Adds blackglass to the Linux desktop's applications (for the user): its
# launcher (which opens the window, --gui) and its icon. The blackglass
# program must be on the PATH (the window build, not the static terminal
# one).
#
#   scripts/install-desktop.sh            # into ~/.local/share
#   scripts/install-desktop.sh --remove   # take it out again
set -euo pipefail
cd "$(dirname "$0")/.."
share="${XDG_DATA_HOME:-$HOME/.local/share}"
apps="$share/applications"
icons="$share/icons/hicolor"
if [ "${1:-}" = "--remove" ]; then
  rm -f "$apps/blackglass.desktop"
  for size in 256 128 64 32; do rm -f "$icons/${size}x${size}/apps/blackglass.png"; done
  rm -f "$icons/scalable/apps/blackglass.svg"
  echo "removed"
  exit 0
fi
command -v blackglass >/dev/null || echo "note: blackglass isn't on the PATH yet" >&2
mkdir -p "$apps" "$icons/scalable/apps"
install -m 644 packaging/linux/blackglass.desktop "$apps/blackglass.desktop"
for size in 256 128 64 32; do
  mkdir -p "$icons/${size}x${size}/apps"
  install -m 644 "packaging/icon/blackglass-$size.png" "$icons/${size}x${size}/apps/blackglass.png"
done
install -m 644 packaging/icon/blackglass.svg "$icons/scalable/apps/blackglass.svg"
command -v update-desktop-database >/dev/null && update-desktop-database "$apps" >/dev/null 2>&1 || true
echo "blackglass is in the applications menu"
