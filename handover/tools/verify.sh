#!/usr/bin/env bash
# Full integration verification in the current repo root.
S=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad
source $S/env.sh
set -o pipefail
cargo fmt --all
( export CARGO_TARGET_DIR=/home/agent/.cache/codegen-main
  cargo run -q -p bir-rules-codegen -- generate >/dev/null 2>&1; rm -rf crates/bir-rules/src/generated.bir-rules-codegen-backup-*
  echo "--- rules check"; cargo run -q -p bir-rules-codegen -- check 2>&1 | tail -1
  echo "--- boundaries"; cargo run -q -p bir-rules-codegen -- status --boundaries-only 2>&1 | grep -E '^\s*\[ \]|^error' | cut -c1-300
  echo "--- codegen status tests"; cargo test -q -p bir-rules-codegen status:: 2>&1 | grep -E 'test result|FAILED' )
echo "--- bir-core tests"; cargo test -p bir-core 2>&1 | grep -E 'panicked|FAILED|test result: F|^error' | head -20; echo "(done)"
echo "--- bir-print tests"; cargo test -p bir-print --lib 2>&1 | grep -E 'panicked|FAILED|test result' | head -5
echo "--- clippy core"; cargo clippy -p bir-core -- -D warnings 2>&1 | grep -E '^(error|warning)' -A4 | grep -v imap | head -20
echo "--- clippy desktop"; cargo clippy -p bir-desktop --all-targets --features dev-tools,agent -- -D warnings 2>&1 | grep -E '^(error|warning)' -A4 | grep -v 'imap\|future-incompat' | head -30
echo "--- clippy desktop default"; cargo clippy -p bir-desktop -- -D warnings 2>&1 | grep -E '^error' -A4 | head -10
