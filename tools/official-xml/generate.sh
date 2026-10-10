#!/usr/bin/env bash
# Regenerate crates/bir-core/data/official-xml/<form_id>.json for every form in
# forms.txt from a hexuria/buwiz-validation checkout.
#
# Usage: tools/official-xml/generate.sh <buwiz-validation dir>
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
validation="${1:?path to a hexuria/buwiz-validation checkout}"
out="$here/../../crates/bir-core/data/official-xml"
mkdir -p "$out"
cd "$here"
[ -d node_modules ] || npm ci --silent
status=0
while read -r form_id hta; do
  [ -z "$form_id" ] && continue
  if node gen_layout.js "$validation/package/forms/BIR-Form$hta.hta" "$form_id" > "$out/$form_id.json.tmp" 2> "$out/$form_id.err"; then
    mv "$out/$form_id.json.tmp" "$out/$form_id.json"; rm -f "$out/$form_id.err"
    echo "ok   $form_id"
  else
    rm -f "$out/$form_id.json.tmp"
    echo "FAIL $form_id: $(grep -m1 -E 'Error' "$out/$form_id.err" || true)"
    rm -f "$out/$form_id.err"
    status=1
  fi
done < forms.txt
exit $status
