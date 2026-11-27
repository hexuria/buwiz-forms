#!/usr/bin/env bash
# Proof that the lock-pages debug entry is compiled out of a release build of the app's Rust crate: the method name is
# a registered host string in a debug build, and a release rlib must not contain it at all.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"
cargo build --release --manifest-path "$root/Cargo.toml" >/dev/null
rlib="$(find "$target/release/deps" -maxdepth 1 -name 'libnsh_app_ebirforms-*.rlib' | head -1)"
[ -n "$rlib" ] || { echo "release rlib not found"; exit 1; }
if strings "$rlib" | grep -q 'lock_pages_debug'; then
  echo "release build contains the lock-pages debug route entry"
  exit 1
fi
echo "release build has no lock-pages debug route entry"
