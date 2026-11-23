#!/usr/bin/env bash
# The Android half of `just nsh-run-agent`: which adb and which device, and the hand-off of the data plane's token.
#
#   android-handoff.sh device                    prints adb's path, then the device's serial
#   android-handoff.sh token ADB SERIAL PID      writes $GPUI_AGENT_TOKEN into the host's private files on SERIAL, for as
#                                                long as the launcher (PID) lives
#
# adb is the SDK's own, found as nsh finds the SDK (crates/nsh-cli/src/android.rs, Sdk::find): ANDROID_HOME, else
# ANDROID_SDK_ROOT, else Android Studio's default place. The device is the one nsh launches on: the first that
# `adb devices` lists as `device`. nsh takes no serial, so an ANDROID_SERIAL naming another device is refused here, before
# anything is built: the token would go to a device the app is not started on.
set -euo pipefail

host=com.longbridge.nsh.host
dir=files/nsh/dev.goldcoders.bir

fail() {
    echo "nsh-run-agent: $*" >&2
    exit 1
}

case "${1:-}" in
device)
    sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
    if [ -z "$sdk" ]; then
        if [ -n "${LOCALAPPDATA:-}" ]; then sdk="$LOCALAPPDATA/Android/Sdk"; else sdk="$HOME/Library/Android/sdk"; fi
    fi
    [ -d "$sdk" ] || fail "Android SDK not found at $sdk: install it and set ANDROID_HOME"
    adb="$sdk/platform-tools/adb"
    [ -x "$adb" ] || [ ! -x "$adb.exe" ] || adb="$adb.exe"
    [ -x "$adb" ] || fail "no adb at $adb: install the SDK's platform-tools"
    "$adb" version >/dev/null 2>&1 || fail "$adb does not run"
    serial="$("$adb" devices | awk -F '\t' 'NR > 1 && $2 == "device" { print $1; exit }')"
    [ -n "$serial" ] || fail "no Android device or emulator connected (see \`adb devices\`)"
    if [ -n "${ANDROID_SERIAL:-}" ] && [ "$ANDROID_SERIAL" != "$serial" ]; then
        fail "ANDROID_SERIAL is $ANDROID_SERIAL, but nsh launches on $serial (the first device \`adb devices\` lists): disconnect the others"
    fi
    printf '%s\n%s\n' "$adb" "$serial"
    ;;
token)
    adb="$2" serial="$3" launcher="$4"
    [ -n "${GPUI_AGENT_TOKEN:-}" ] || fail "set GPUI_AGENT_TOKEN first"
    tries="${NSH_AGENT_TOKEN_TRIES:-1800}"
    for _ in $(seq "$tries"); do
        # A launch that failed or was cancelled installs nothing more: stop, and leave the device as it is.
        kill -0 "$launcher" 2>/dev/null || exit 0
        if "$adb" -s "$serial" shell run-as "$host" true >/dev/null 2>&1; then
            printf %s "$GPUI_AGENT_TOKEN" | "$adb" -s "$serial" shell -T "run-as $host sh -c 'umask 077 && mkdir -p $dir && cat > $dir/agent-token.new && mv $dir/agent-token.new $dir/agent-token'" && exit 0
        fi
        sleep "${NSH_AGENT_TOKEN_PAUSE:-0.5}"
    done
    fail "the data plane's token was not stored on $serial (is the host installed?)"
    ;;
*)
    fail "android-handoff.sh: device | token ADB SERIAL PID"
    ;;
esac
