#!/usr/bin/env bash
# Adds eBIRForms to the app menu / dock for the current user when running from
# the Linux tarball (the .deb installs these files system-wide instead).
#
#   ./install-desktop-entry.sh            install for this user
#   ./install-desktop-entry.sh --remove   undo
set -euo pipefail

APP_ID=dev.goldcoders.bir
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
ENTRY="$DATA/applications/$APP_ID.desktop"
SIZES=(48 64 128 256 512)

# Works both from the tarball root (bir + assets/) and from assets/linux/ itself.
if [ -d "$HERE/assets/linux" ]; then
    LINUX="$HERE/assets/linux"; BIN="$HERE/bir"
else
    LINUX="$HERE"; BIN="$(cd "$HERE/../.." && pwd)/bir"
fi

refresh() {
    command -v update-desktop-database >/dev/null && update-desktop-database "$DATA/applications" 2>/dev/null || true
    command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$DATA/icons/hicolor" 2>/dev/null || true
}

if [ "${1:-}" = "--remove" ]; then
    rm -f "$ENTRY"
    for n in "${SIZES[@]}"; do rm -f "$DATA/icons/hicolor/${n}x${n}/apps/$APP_ID.png"; done
    refresh
    echo "Removed eBIRForms launcher."
    exit 0
fi

[ -x "$BIN" ] || { echo "eBIRForms binary not found at $BIN" >&2; exit 1; }

for n in "${SIZES[@]}"; do
    install -Dm644 "$LINUX/icons/hicolor/${n}x${n}/apps/$APP_ID.png" "$DATA/icons/hicolor/${n}x${n}/apps/$APP_ID.png"
done
mkdir -p "$(dirname "$ENTRY")"
# Point Exec at this copy of the binary; quote it in case the path has spaces.
sed "s|^Exec=.*|Exec=\"$BIN\"|" "$LINUX/$APP_ID.desktop" > "$ENTRY"
chmod 644 "$ENTRY"
refresh
echo "Installed eBIRForms launcher: $ENTRY"
