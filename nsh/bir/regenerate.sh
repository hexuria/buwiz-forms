#!/usr/bin/env bash
# Regenerates nsh/bir/types.d.ts and nsh/bir/functions.ts.
#
# types.d.ts comes from the Rust modules (the TypeScript declarations bir-app-api writes for the bir module).
# functions.ts wraps every host function the old app calls: the names are the string literals and bir.<name>(
# calls of app/main.js and app/shell/*.js that are host functions (comments skipped); the example keeps the
# ones the module has. Run it from anywhere; it needs only cargo and grep.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo"
names="$(
  cat app/main.js app/shell/*.js \
    | grep -vE '^[[:space:]]*(//|\*|/\*)' \
    | grep -oE "[\"'][a-z][a-z0-9_]*[\"']|bir\.[a-z][a-z0-9_]*\(" \
    | sed -E "s/^bir\.//; s/\($//; s/[\"']//g" \
    | sort -u
)"
# Names a new screen needs that the old app never calls (bir/extra-functions.txt).
extra="$(grep -vE '^[[:space:]]*(#|$)' nsh/bir/extra-functions.txt || true)"
names="$(printf '%s\n%s\n' "$names" "$extra" | sort -u)"
# shellcheck disable=SC2086
cargo run --quiet --manifest-path nsh/Cargo.toml --example door_types -- nsh/bir $names
