#!/usr/bin/env bash
# preview-form — render ONE form worst-case-filled through the print pipeline
# (fill -> A4 prepare -> print-overrides/<slug>.css -> static HTML) and open it.
# Overrides are read at render time: edit the CSS and rerun, no rebuild.
#
#   scripts/preview-form.sh <FORM_CODE> [--no-override] [--no-open]
#
# Writes $TMPDIR/bir-preview-form/<FORM_CODE>/{after,before}.html
# (before = --no-override).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CODE="${1:-}"; shift || true
[[ -z "$CODE" || "$CODE" == -* ]] && { echo "usage: $0 <FORM_CODE> [--no-override] [--no-open]" >&2; exit 2; }
OV=""; OPEN=1
for a in "$@"; do case "$a" in --no-override) OV="--no-override";; --no-open) OPEN=0;; *) echo "unknown option $a" >&2; exit 2;; esac; done
SLUG=$(awk -v c="\"$CODE\"" '/fn print_slug_for/{f=1} f && $1==c {gsub(/[",]/,"",$3); print $3; exit} f && /^}/{exit}' "$ROOT/crates/bir-shell/src/bir_module.rs")
[[ -z "$SLUG" ]] && { echo "unknown form code: $CODE" >&2; exit 3; }
TAG=$([[ -n "$OV" ]] && echo before || echo after)
OUT="${TMPDIR:-/tmp}/bir-preview-form/$CODE"; mkdir -p "$OUT"
(cd "$ROOT/crates/bir-shell" && cargo +1.99.0 run -q --example print_render -- --out "$OUT" --only "$SLUG" $OV >/dev/null)
mv -f "$OUT/$SLUG.html" "$OUT/$TAG.html"
echo "$OUT/$TAG.html"
echo "override: $([[ -n "$OV" ]] && echo off || { [[ -f "$ROOT/print-overrides/$SLUG.css" ]] && echo "print-overrides/$SLUG.css" || echo "none (print-overrides/$SLUG.css absent)"; })"
if [[ $OPEN == 1 ]]; then open "$OUT/$TAG.html" 2>/dev/null || xdg-open "$OUT/$TAG.html"; fi
