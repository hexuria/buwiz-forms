#!/usr/bin/env bash
# Screenshot every print render in tmp/form-findings/print/*.html (worst-case
# filled sheets from `cargo run --example print_render`) with Playwright's
# headless Chromium shell: one PNG per form, every sheet stacked.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIR="$ROOT/tmp/form-findings/print"
H="${CHROME_HEADLESS_SHELL:-$(ls -d "$HOME"/Library/Caches/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-mac-arm64/chrome-headless-shell 2>/dev/null | tail -1)}"
[[ -x "$H" ]] || { echo "no chrome-headless-shell (set CHROME_HEADLESS_SHELL)" >&2; exit 2; }
PROFILE="$(mktemp -d)"; trap 'rm -rf "$PROFILE"' EXIT
for f in "$DIR"/*.html; do
  slug="$(basename "$f" .html)"
  n="$(grep -o 'class="sheet"' "$f" | wc -l | tr -d ' ')"; [[ "$n" -gt 0 ]] || n=1
  "$H" --headless --disable-gpu --user-data-dir="$PROFILE" --hide-scrollbars \
    --window-size=900,$((n * 1300)) --screenshot="$DIR/$slug.png" "file://$f" >/dev/null 2>&1
  echo "$slug $n"
done
