#!/usr/bin/env bash
# Proof that the lock-pages debug entry is compiled out of the release macOS host built from nsh's generated
# workspace. Cargo's JSON artifact line, never a directory scan, names the exact binary this build produced.
# This checks the macOS host; the other hosts use the same debug_assertions gate.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"
mkdir -p "$target"
json="$target/release-lockdebug-artifacts.jsonl"
cd "$root"
nsh build >/dev/null
cargo build --manifest-path "$root/.nsh/rust/Cargo.toml" -p nsh-app-nsh-app-ebirforms-macos-app --message-format=json >"$json"
debug_artifact="$(python3 - "$json" <<'PY'
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
    raise SystemExit('debug macOS host artifact not found in cargo JSON output')
print(found[-1])
PY
)"
if grep -qa 'lock_pages_debug' "$debug_artifact"; then
  echo "found in debug: $debug_artifact"
else
  echo "positive control failed: debug macOS host lacks lock_pages_debug"
  exit 1
fi
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
if grep -qa 'lock_pages_debug' "$artifact"; then
  echo "release build contains the lock-pages debug route entry"
  exit 1
fi
echo "absent in release: $artifact"
