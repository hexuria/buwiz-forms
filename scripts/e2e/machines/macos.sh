#!/usr/bin/env bash
# The smoke suite, then rdo-row-press, in a real macOS window (a Devin macOS machine, never the owner's Mac). Screen Recording must be
# granted to the app's host binary for failure screenshots; record the run with the machine's own recorder.
source "$(dirname "$0")/common.sh"
desktop_suites macos "$@"
