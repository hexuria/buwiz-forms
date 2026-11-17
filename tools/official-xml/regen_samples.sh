#!/usr/bin/env bash
# Re-run every sample's steps through the official page (saveEncryptedProfile).
# Usage: tools/official-xml/regen_samples.sh <buwiz-validation dir>
here="$(cd "$(dirname "$0")" && pwd)"; validation="${1:?}"; P="$here/../../crates/bir-core/tests/sample-payloads"
for steps in "$P"/*.steps.json; do
  stem=$(basename "$steps" .steps.json)
  hta=$(grep -o '"hta"[^,}]*' "$steps.meta" 2>/dev/null | head -1)
  # The HTA comes from the sample's form id via forms.txt and the layout file.
  hta=$(python3 - "$stem" "$here" <<'PY'
import json,sys,glob,os
stem,here=sys.argv[1:3]
meta=os.path.join(here,'..','..','crates','bir-core','tests','sample-payloads',stem+'.hta')
if os.path.exists(meta): print(open(meta).read().strip()); sys.exit()
code=stem.split('-')[0].lower()
for line in open(os.path.join(here,'forms.txt')):
    fid,h=line.split()
    if fid.split('-v')[0]==code or fid.split('-')[0]==code: print('BIR-Form'+h+'.hta'); break
PY
)
  if [ -z "$hta" ]; then echo "NOHTA $stem"; continue; fi
  if node "$here/runtime.js" "$validation/package/forms/$hta" "$steps" > "$P/$stem.official.xml.tmp" 2> "$P/$stem.err"; then
    mv "$P/$stem.official.xml.tmp" "$P/$stem.official.xml"; echo "ok   $stem"
  else echo "FAIL $stem: $(head -c 200 "$P/$stem.err" | tr '\n' ' ')"; rm -f "$P/$stem.official.xml.tmp"; fi
  rm -f "$P/$stem.err"
done
