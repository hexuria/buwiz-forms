#!/usr/bin/env bash
# Regenerate every committed layout in crates/bir-core/data/official-xml from
# its own official_hta, using setup/<form_id>.js when present.
# Usage: tools/official-xml/regen_all.sh <buwiz-validation dir>
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; validation="${1:?validation dir}"
out="$here/../../crates/bir-core/data/official-xml"
for f in "$out"/*.json; do
  id=$(basename "$f" .json); case $id in *atc-options*) continue;; esac
  hta=$(node -e "console.log(require('$f').official_hta)")
  setup=""; [ -f "$here/setup/$id.js" ] && setup="$here/setup/$id.js"
  if node "$here/gen_layout.js" "$validation/package/forms/$hta" "$id" $setup > "$f.tmp" 2> "$f.err"; then
    cmp -s "$f.tmp" "$f" && echo "same $id" || echo "NEW  $id"; mv "$f.tmp" "$f"; rm -f "$f.err"
  else echo "FAIL $id: $(grep -m1 Error "$f.err")"; rm -f "$f.tmp"; fi
done
