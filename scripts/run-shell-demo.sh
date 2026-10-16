#!/usr/bin/env bash
# Launch the debug shell against a demo database with dummy TINs allowed.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/crates/bir-shell/target/debug/bir-shell"
if [[ ! -x "$BIN" ]]; then
  echo "Build the debug shell first: cargo build --locked --manifest-path crates/bir-shell/Cargo.toml --features gpui-fast --bin bir-shell" >&2
  exit 1
fi
mkdir -p "$ROOT/tmp/pr141-review"
exec env \
  EBIR_TEST_ENV=1 \
  EBIR_RELAXED_VALIDATION=1 \
  BIR_SFTP_DRY_RUN=1 \
  BIR_DATABASE_PATH="${BIR_DEMO_DATABASE_PATH:-$ROOT/tmp/pr141-review/manual.db}" \
  BIR_SHELL_APP="$ROOT/app" \
  "$BIN" "$@"
