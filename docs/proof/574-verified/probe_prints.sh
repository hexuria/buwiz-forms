#!/bin/bash
# #574 corrected census probe — one key, one verdict.
# Usage: ./probe_prints.sh <slug> <writer_key> <fill_value> [control_key=control_value]
#
# Renders the form through the committed print path (print_path_render,
# same pipeline as the print preview) and reads the value back three ways:
# full text extraction in document order, per-cell comb slot spans
# (data-slot-index), and input value= attributes. A census readback that
# only checks input values or contiguous substrings misses comb digits
# entirely — that was blind spot #1.
set -e
SLUG="$1"; KEY="$2"; VAL="$3"; EXTRA="$4"
export PATH="$HOME/.rustup/toolchains/1.99.0-aarch64-apple-darwin/bin:/usr/bin:/bin"
export CARGO_TARGET_DIR="$PWD/target-483" RUSTC_WRAPPER="" CARGO_INCREMENTAL=0
export BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
OUT="/tmp/probe-574-$SLUG.html"
cargo run --locked -p bir-app-api --example print_path_render -- \
    "$SLUG" "$OUT" "$KEY=$VAL" $EXTRA 2>/dev/null | tail -1
python3 - "$OUT" "$VAL" <<'PYEOF'
import re, sys
html = open(sys.argv[1]).read()
val = sys.argv[2].upper()
# readback 1: full text, document order
t = re.sub(r'<style[^>]*>.*?</style>', '', html, flags=re.S)
t = re.sub(r'<[^>]+>', '', t)
t = re.sub(r'\s+', '', t).upper()
print('text scan:    ', 'FOUND' if val in t else 'absent')
# readback 2: comb slot spans (one digit per slot)
slots = re.findall(r'data-slot-index="\d+"[^>]*>([^<]*)</span>', html)
joined = ''.join(s for s in slots).strip().upper()
print('slot spans:   ', 'FOUND' if val in joined else ('absent — digits found: ' + joined[:40] if joined else 'no slots at all'))
# readback 3: input values (what the blind census used)
vals = ''.join(re.findall(r'value="([^"]*)"', html)).upper()
print('input values: ', 'FOUND' if val in vals else 'absent (the blind readback)')
PYEOF
