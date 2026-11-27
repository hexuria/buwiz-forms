#!/usr/bin/env bash
# Proof that the lock-pages debug entry is compiled out of the release macOS host built from nsh's generated
# workspace. Cargo's JSON artifact line, never a directory scan, names the exact binary this build produced.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"
mkdir -p "$target"
json="$target/release-lockdebug-artifacts.jsonl"
cd "$root"
nsh build --release >/dev/null
cargo build --release --manifest-path "$root/.nsh/rust/Cargo.toml" -p nsh-app-nsh-app-ebirforms-macos-app --message-format=json >"$json"
artifact="$(python3 - "$json" <<'PY'
import json,sys
found=[]
for line in open(sys.argv[1]):
    try: row=json.loads(line)
    except json.JSONDecodeError: continue
    if row.get('reason')!='compiler-artifact': continue
    target=row.get('target') or {}
    if target.get('name')=='nsh-app-nsh-app-ebirforms-macos-app' and row.get('executable'):
        found.append(row['executable'])
if not found:
    raise SystemExit('release macOS host artifact not found in cargo JSON output')
print(found[-1])
PY
)"
if strings "$artifact" | grep -q 'lock_pages_debug'; then
  echo "release build contains the lock-pages debug route entry"
  exit 1
fi
echo "release build has no lock-pages debug route entry: $artifact"
