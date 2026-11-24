#!/usr/bin/env bash
# Tests android-handoff.sh against a stand-in adb (no device needed): `bash nsh/agent/android-handoff.test.sh`.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
handoff="$here/android-handoff.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failed=0

check() { # name, then a command that must succeed
    local name="$1"
    shift
    if "$@"; then echo "ok   $name"; else
        echo "FAIL $name"
        failed=1
    fi
}

# A stand-in SDK: its adb logs every call, lists $FAKE_DEVICES, and has the host installed once $work/installed exists.
sdk="$work/sdk"
mkdir -p "$sdk/platform-tools"
cat >"$sdk/platform-tools/adb" <<'ADB'
#!/usr/bin/env bash
echo "$*" >>"$FAKE_LOG"
case "$1" in
version) exit 0 ;;
devices)
    printf 'List of devices attached\n%b' "${FAKE_DEVICES:-}"
    exit 0
    ;;
esac
[ "$1" = -s ] || exit 64
case "$*" in
*"run-as com.longbridge.nsh.host true") [ -e "$FAKE_STATE/installed" ] ;;
*"shell -T"*) cat >"$FAKE_STATE/token-on-$2" ;;
esac
ADB
chmod +x "$sdk/platform-tools/adb"
export FAKE_LOG="$work/log" FAKE_STATE="$work"
two='emulator-5554\toffline\nR58M123\tdevice\nemulator-5556\tdevice\n'
run() { env -u ANDROID_HOME -u ANDROID_SDK_ROOT -u ANDROID_SERIAL -u LOCALAPPDATA "$@"; }

out="$(run ANDROID_HOME="$sdk" FAKE_DEVICES="$two" "$handoff" device)"
check "the device is the first one ready, as nsh picks it" [ "$out" = "$sdk/platform-tools/adb"$'\n'"R58M123" ]

out="$(run ANDROID_SDK_ROOT="$sdk" FAKE_DEVICES="$two" ANDROID_SERIAL=R58M123 "$handoff" device)"
check "ANDROID_SDK_ROOT and a matching ANDROID_SERIAL are accepted" [ "${out##*$'\n'}" = R58M123 ]

err="$(run ANDROID_HOME="$sdk" FAKE_DEVICES="$two" ANDROID_SERIAL=emulator-5556 "$handoff" device 2>&1)"
check "an ANDROID_SERIAL nsh would not launch on is refused" [ $? -ne 0 ]
check "the refusal names both devices" grep -q "emulator-5556.*R58M123" <<<"$err"

run ANDROID_HOME="$sdk" FAKE_DEVICES='emulator-5554\tunauthorized\n' "$handoff" device >/dev/null 2>&1
check "no ready device is refused" [ $? -ne 0 ]

home="$work/home"
mkdir -p "$home/Library/Android" && cp -R "$sdk" "$home/Library/Android/sdk"
out="$(run HOME="$home" FAKE_DEVICES="$two" "$handoff" device)"
check "macOS: Android Studio's default SDK is found" [ "${out%%$'\n'*}" = "$home/Library/Android/sdk/platform-tools/adb" ]

local_app="$work/AppData/Local"
mkdir -p "$local_app/Android" && cp -R "$sdk" "$local_app/Android/Sdk"
mv "$local_app/Android/Sdk/platform-tools/adb" "$local_app/Android/Sdk/platform-tools/adb.exe"
out="$(run HOME="$work/nowhere" LOCALAPPDATA="$local_app" FAKE_DEVICES="$two" "$handoff" device)"
check "Windows: %LOCALAPPDATA%\\Android\\Sdk and adb.exe are found" [ "${out%%$'\n'*}" = "$local_app/Android/Sdk/platform-tools/adb.exe" ]

err="$(run HOME="$work/nowhere" "$handoff" device 2>&1)"
check "no SDK is refused before anything is launched" [ $? -ne 0 ]
check "and says where it looked" grep -q "Android SDK not found" <<<"$err"

cp "$sdk/platform-tools/adb" "$work/adb.keep"
printf '#!/bin/sh\nexit 1\n' >"$sdk/platform-tools/adb"
run ANDROID_HOME="$sdk" FAKE_DEVICES="$two" "$handoff" device >/dev/null 2>&1
check "an adb that does not run is refused" [ $? -ne 0 ]
cp "$work/adb.keep" "$sdk/platform-tools/adb"

# The token: only to the chosen device, once the host is installed, and only while the launcher lives.
adb="$sdk/platform-tools/adb"
export GPUI_AGENT_TOKEN=handoff-test-token NSH_AGENT_TOKEN_PAUSE=0.05
: >"$FAKE_LOG"
sleep 30 &
launcher=$!
(sleep 0.3 && : >"$work/installed") &
"$handoff" token "$adb" R58M123 "$launcher"
check "the token is stored once the host is installed" [ $? -eq 0 ]
check "it is the token, on the chosen device" [ "$(cat "$work/token-on-R58M123" 2>/dev/null)" = handoff-test-token ]
check "every adb call names the serial" [ -z "$(grep -v '^-s R58M123 ' "$FAKE_LOG")" ]
check "the token is in no adb argument" [ -z "$(grep handoff-test-token "$FAKE_LOG")" ]

# A launch that fails or is cancelled: the writer stops without writing, though the host is installed.
kill "$launcher" && wait "$launcher" 2>/dev/null
rm -f "$work/token-on-R58M123"
: >"$FAKE_LOG"
started=$SECONDS
"$handoff" token "$adb" R58M123 "$launcher"
check "a writer whose launcher is gone ends quietly" [ $? -eq 0 ]
check "and at once" [ $((SECONDS - started)) -lt 5 ]
check "nothing is written after a cancelled launch" [ ! -e "$work/token-on-R58M123" ]
check "nor is the device asked anything" [ ! -s "$FAKE_LOG" ]

# The launcher dies while the writer waits for the host.
rm -f "$work/installed"
sleep 30 &
launcher=$!
(sleep 0.3 && kill "$launcher") &
started=$SECONDS
"$handoff" token "$adb" R58M123 "$launcher"
check "a launcher that dies mid-wait ends the writer" [ $? -eq 0 ] && [ $((SECONDS - started)) -lt 5 ]
check "with no token written" [ ! -e "$work/token-on-R58M123" ]

# A host that never installs is reported.
sleep 30 &
launcher=$!
err="$(NSH_AGENT_TOKEN_TRIES=3 "$handoff" token "$adb" R58M123 "$launcher" 2>&1)"
check "a host that never installs is reported" [ $? -ne 0 ]
check "by name" grep -q "was not stored on R58M123" <<<"$err"
kill "$launcher" 2>/dev/null

# plug mode, against a fake adb whose pidof answers the sequence in $work/pids.
plugsdk="$work/plugsdk"
mkdir -p "$plugsdk"
cat >"$plugsdk/adb" <<'ADB'
#!/usr/bin/env bash
echo "$*" >>"$FAKE_LOG"
[ "$1" = -s ] || exit 64
shift 2
case "$*" in
"shell run-as com.longbridge.nsh.host true") [ -e "$FAKE_STATE/installed" ] ;;
"shell pidof com.longbridge.nsh.host")
    pid="$(head -1 "$FAKE_STATE/pids" 2>/dev/null)"
    [ -n "$pid" ] && tail -n +2 "$FAKE_STATE/pids" >"$FAKE_STATE/pids.tmp" && mv "$FAKE_STATE/pids.tmp" "$FAKE_STATE/pids" || true
    [ -n "$pid" ] || exit 1
    echo "$pid"
    ;;
"shell -T"*) cat >>"$FAKE_STATE/writes" ;;
"forward --remove "*) exit 0 ;;
"forward "*) exit 0 ;;
"shell am start "*) touch "$FAKE_STATE/am-started" ; exit 0 ;;
esac
ADB
chmod +x "$plugsdk/adb"
printf '' >"$work/writes"

# A stale host that `nsh run` force-stops and replaces: the relaunch fires on the pid change, once.
touch "$work/installed"
printf '111\n222\n222\n222\n' >"$work/pids"
sleep 30 &
launcher=$!
GPUI_AGENT_TOKEN=t "$handoff" plug "$plugsdk/adb" R58M123 "$launcher" 17421 8347
check "a new pid relaunches the activity" [ -e "$work/am-started" ]
check "with the agent extras and the dev port" grep -q "am start -S -n com.longbridge.nsh.host/com.longbridge.nsh.DevActivity --es url ws://127.0.0.1:8347/ws --es GPUI_AGENT 1 --es GPUI_AGENT_ADDR 127.0.0.1:17421" "$FAKE_LOG"
check "and the window token was stored" [ -s "$work/writes" ]
kill "$launcher" 2>/dev/null; rm -f "$work/am-started"; : >"$FAKE_LOG"; : >"$work/writes"

# No host ever comes up while the launcher lives: nothing is launched.
printf '\n\n\n' >"$work/pids"
sleep 30 &
launcher=$!
NSH_AGENT_TOKEN_TRIES=4 NSH_AGENT_TOKEN_PAUSE=0.01 GPUI_AGENT_TOKEN=t "$handoff" plug "$plugsdk/adb" R58M123 "$launcher" 17421 8347
check "no host means no relaunch" [ ! -e "$work/am-started" ]
kill "$launcher" 2>/dev/null; rm -f "$work/am-started"; : >"$FAKE_LOG"

# The launcher dies mid-watch: the job removes the forwards it made.
printf '111\n' >"$work/pids"
sleep 30 &
launcher=$!
(sleep 0.3 && kill "$launcher") &
NSH_AGENT_TOKEN_TRIES=100 "$handoff" plug "$plugsdk/adb" R58M123 "$launcher" 17421 8347
check "a dead launcher removes both forwards" bash -c 'grep -q "forward --remove tcp:17421" "$FAKE_LOG" && grep -q "forward --remove tcp:17422" "$FAKE_LOG"'

# A launcher that dies before the host installs removes nothing it did not create.
rm -f "$work/installed"; : >"$FAKE_LOG"
sleep 30 &
launcher=$!
(sleep 0.3 && kill "$launcher") &
NSH_AGENT_TOKEN_TRIES=100 "$handoff" plug "$plugsdk/adb" R58M123 "$launcher" 17421 8347
check "a launch that never installed removes no forwards" bash -c '! grep -q "forward --remove" "$FAKE_LOG"'

exit "$failed"
