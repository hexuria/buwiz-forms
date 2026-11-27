#!/usr/bin/env bash
# Job 57 diagnostic runner: cold-launch the app, run prep/type and exactly one x/y hold recipe, then save the J57 logcat tags.
# adb is used here only for logcat; the app is driven by just e2e/gpui-agent.
set -u
cell=${1:-probe}
run=${2:-1}
outroot=${DIAG_OUTROOT:-$HOME/j57/diag-evidence}
out="$outroot/$cell-run$run"
adb="$HOME/Android/Sdk/platform-tools/adb"
mkdir -p "$outroot"
rm -rf "$out"
cd "$(dirname "$0")/../.." || exit 2
export ANDROID_HOME="$HOME/Android/Sdk"
export NSH_TOOLS="$HOME/BuwizNsh/tools-j57diag"
export NSH_RUNTIME="$HOME/BuwizNsh/tools-j57diag/runtime"
export CARGO_TARGET_DIR="$HOME/BuwizNsh/target-j57diag"
export RUSTC_WRAPPER=
export BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
unset BIR_SFTP_LIVE BIR_SFTP_HOST BIR_SFTP_PORT BIR_SFTP_USERNAME BIR_SFTP_PASSWORD BIR_SFTP_ACCEPT_ANY_HOST_KEY BIR_RELAY_URL BUWIZ_RELAY_SEND
"$adb" forward --remove tcp:17421 >/dev/null 2>&1 || true
"$adb" forward --remove tcp:17422 >/dev/null 2>&1 || true
"$adb" logcat -c >/dev/null 2>&1 || true
if [ "$cell" = probe ]; then
  set -- --only prep --only type
else
  set -- --only prep --only type --only "hold-$cell"
fi
just e2e android otp-diag "$@" --record --out "$out" > "$outroot/$cell-run$run.driver.log" 2>&1
rc=$?
"$adb" logcat -d -v threadtime 2>/dev/null | grep -E 'J57(STOCK|OURS)' > "$out/logcat-j57.txt" || true
echo "$cell run$run rc=$rc $(tail -1 "$outroot/$cell-run$run.driver.log" 2>/dev/null)"
exit "$rc"
