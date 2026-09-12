#!/usr/bin/env bash
# Wrap the debug `bir` binary in a minimal .app so a development run is a real
# bundle: macOS then shows the app's notifications natively (own name, own
# icon) instead of dropping them or routing through Script Editor.
#
#   cargo build --locked --bin bir --features dev-tools,agent
#   scripts/dev_bundle_macos.sh            # -> target/debug/eBIRForms.app
#   open target/debug/eBIRForms.app        # or run Contents/MacOS/bir directly
#
# Environment variables are inherited when launching Contents/MacOS/bir from a
# shell; `open` needs them exported via `open --env` or `launchctl setenv`.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
profile="${1:-debug}"
binary="$root/target/$profile/bir"
app="$root/target/$profile/eBIRForms.app"

[[ -x "$binary" ]] || { echo "no binary at $binary (build it first)" >&2; exit 1; }

version="$(grep -m1 '^version' "$root/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/bir"
cp "$root/assets/AppIcon.icns" "$app/Contents/Resources/AppIcon.icns"
# Its own identifier, not the release one: LaunchServices resolves a bundle
# id to whichever registered copy it prefers, so a notification posted under
# dev.goldcoders.bir would activate an installed release / TestFlight build
# instead of this process.
sed -e 's/BUNDLE_ID_PLACEHOLDER/dev.goldcoders.bir.dev/' \
    -e 's/APP_NAME_PLACEHOLDER/eBIRForms Dev/' \
    -e 's/<string>e-BIRForms<\/string>/<string>e-BIRForms (dev)<\/string>/' \
    -e "s/VERSION_PLACEHOLDER/$version/" \
    -e 's/BUILD_NUMBER_PLACEHOLDER/1/' \
    "$root/assets/macos/Info.plist" > "$app/Contents/Info.plist"

# Ad-hoc signature: unsigned bundles get no notification identity.
codesign --force --sign - "$app" >/dev/null

echo "$app"
