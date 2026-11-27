#!/usr/bin/env bash
# The smoke suite on the iOS Simulator: one booted iPhone or iPad. --record keeps a Simulator recording per recipe.
source "$(dirname "$0")/common.sh"
need xcrun "install Xcode"
xcrun simctl list devices booted | grep -q Booted || { echo "e2e: boot a Simulator first (xcrun simctl boot <udid>)" >&2; exit 1; }
smoke ios --record "$@"
