#!/usr/bin/env bash
# Sign BIR's Rox v1 manifest (dev.goldcoders.bir) with the Goldcoders
# publisher key, for `bir-mini-app mcp` to serve through BIR_V1_MANIFEST.
#
#   BIR_MINI_APP=<path>/bir-mini-app ROX_BROKER_BIN=<path>/rox-broker \
#     crates/bir-shell/scripts/sign-v1-manifest.sh --keychain gc-2026-01 --out <signed.json>
#   ... sign-v1-manifest.sh --key-file <0600 test seed> --out <signed.json>   # tests
#
# The manifest comes from the binary that will serve it
# (`bir-mini-app print-v1-manifest`): its app fingerprints are the bytes
# that binary serves, so set BIR_MINI_APPS_DIR here exactly as the binary
# will run (unset for the apps built into it). `rox-broker sign-manifest`
# signs it (ROX_BROKER_BIN); until Rox drops its Node broker, the broker's
# v1/sign-manifest.mjs still can (ROX_BROKER_DIR: src/v1/ in rox/server,
# server/v1/ in a grok-shell checkout). Both take the same arguments. The
# key comes from the login
# Keychain (service dev.goldcoders.publisher) or a key file, and is never
# printed. Any later change to an app, a tool list or the binary's manifest
# text needs a new signature; the binary says so on stderr at start.
set -euo pipefail

bin=${BIR_MINI_APP:?set BIR_MINI_APP to the bir-mini-app binary that will serve the manifest}
if [ -n "${ROX_BROKER_BIN:-}" ]; then
  sign=("$ROX_BROKER_BIN" sign-manifest)
else
  broker=${ROX_BROKER_DIR:?set ROX_BROKER_BIN to rox-broker (or ROX_BROKER_DIR to rox/server)}
  mjs="$broker/src/v1/sign-manifest.mjs"
  [ -f "$mjs" ] || mjs="$broker/server/v1/sign-manifest.mjs"
  sign=(node "$mjs")
fi
if [ $# -eq 0 ]; then
  echo "usage: sign-v1-manifest.sh (--keychain <key_id> | --key-file <path>) [--out <file>]" >&2
  exit 2
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
"$bin" print-v1-manifest >"$tmp/manifest.json"
"${sign[@]}" "$tmp/manifest.json" "$@"
