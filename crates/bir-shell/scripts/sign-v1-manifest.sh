#!/usr/bin/env bash
# Sign BIR's Rox v1 manifest (dev.goldcoders.bir) with the Goldcoders
# publisher key, for `bir-mini-app mcp` to serve through BIR_V1_MANIFEST.
#
#   BIR_MINI_APP=<path>/bir-mini-app ROX_BROKER_DIR=<rox>/server \
#     crates/bir-shell/scripts/sign-v1-manifest.sh --keychain gc-2026-01 --out <signed.json>
#   ... sign-v1-manifest.sh --key-file <0600 test seed> --out <signed.json>   # tests
#
# The manifest comes from the binary that will serve it
# (`bir-mini-app print-v1-manifest`): its app fingerprints are the bytes
# that binary serves, so set BIR_MINI_APPS_DIR here exactly as the binary
# will run (unset for the apps built into it). The broker's
# v1/sign-manifest.mjs signs it (src/v1/ in rox/server, server/v1/ in a
# grok-shell checkout): the key comes from the login
# Keychain (service dev.goldcoders.publisher) or a key file, and is never
# printed. Any later change to an app, a tool list or the binary's manifest
# text needs a new signature; the binary says so on stderr at start.
set -euo pipefail

bin=${BIR_MINI_APP:?set BIR_MINI_APP to the bir-mini-app binary that will serve the manifest}
broker=${ROX_BROKER_DIR:?set ROX_BROKER_DIR to rox/server (or a grok-shell checkout)}
sign="$broker/src/v1/sign-manifest.mjs"
[ -f "$sign" ] || sign="$broker/server/v1/sign-manifest.mjs"
if [ $# -eq 0 ]; then
  echo "usage: sign-v1-manifest.sh (--keychain <key_id> | --key-file <path>) [--out <file>]" >&2
  exit 2
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
"$bin" print-v1-manifest >"$tmp/manifest.json"
node "$sign" "$tmp/manifest.json" "$@"
