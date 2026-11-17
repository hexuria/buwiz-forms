#!/usr/bin/env bash
# Opens every form's sample via BIR_OPEN_SAMPLE, screenshots, OCRs the main pane,
# and flags forms stuck loading or showing errors. Results -> /tmp/form-audit.txt
set -u
cd "$(dirname "$0")"
CODES=$(python3 -c "import json;print(' '.join(f['c'] for f in json.load(open('../app/forms/forms-schema.json'))))")
: > /tmp/form-audit.txt
for code in $CODES; do
  pkill -x cui-app-ebirfor 2>/dev/null; sleep 0.6
  BIR_OPEN_SAMPLE="$code" setsid nohup ./run-dev.sh >/tmp/audit-app.log 2>&1 < /dev/null &
  sleep 11
  DISPLAY=:1 import -window root "/tmp/audit-$code.png" 2>/dev/null
  convert "/tmp/audit-$code.png" -crop 1140x780+300+115 "/tmp/audit-$code-main.png" 2>/dev/null
  tesseract "/tmp/audit-$code-main.png" "/tmp/audit-$code" 2>/dev/null
  python3 - "$code" "/tmp/audit-$code.txt" <<'PYEOF' | tee -a /tmp/form-audit.txt
import sys, re
code = sys.argv[1]
try:
    txt = open(sys.argv[2], errors="replace").read().lower()
except OSError:
    txt = ""
parts = re.search(r"part\s+\d+\s+of\s+(\d+)", txt)
nparts = parts.group(1) if parts else "?"
flags = []
if "opening the return" in txt: flags.append("STILL-LOADING")
if "no on-screen layout" in txt: flags.append("NO-LAYOUT")
if "does not parse" in txt or "failed to" in txt: flags.append("ERRTXT")
status = "PASS" if not flags else "FAIL:" + ",".join(flags)
print(f"{code}\t{status}\tparts={nparts}")
PYEOF
done
pkill -x cui-app-ebirfor 2>/dev/null
echo "AUDIT DONE"
