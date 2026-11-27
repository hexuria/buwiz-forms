#!/usr/bin/env bash
# The smoke suite on the Android emulator: one online emulator. The run starts from an empty test database on it
# (the development build's guarded test-data file); --record keeps an emulator recording per recipe.
source "$(dirname "$0")/common.sh"
adb="${ANDROID_HOME:+$ANDROID_HOME/platform-tools/}adb"
command -v "$adb" >/dev/null 2>&1 || { echo "e2e: no adb: set ANDROID_HOME" >&2; exit 1; }
[ "$("$adb" devices | grep -c $'\tdevice$')" = 1 ] || { echo "e2e: exactly one emulator must be online (adb devices)" >&2; exit 1; }
smoke android --record "$@"
