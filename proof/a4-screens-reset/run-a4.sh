#!/usr/bin/env bash
set -euo pipefail
label="$1"; target="$2"; db="$3"; port="$4"; session="$5"
cd /Users/devin/jobs/a4screens/app
export PATH="/Users/devin/.rustup/toolchains/1.99.0-aarch64-apple-darwin/bin:/Users/devin/BuwizNsh/tools/bin:/Users/devin/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export RUSTUP_TOOLCHAIN=1.99.0 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
export BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
unset BIR_SFTP_LIVE GOOGLE_CLIENT_ID GOOGLE_CLIENT_SECRET BIR_SFTP_HOST BIR_SFTP_PORT BIR_SFTP_USERNAME BIR_SFTP_PASSWORD BIR_SFTP_ACCEPT_ANY_HOST_KEY BIR_RELAY_URL BUWIZ_RELAY_SEND
export GPUI_AGENT_TOKEN="a4proof-local-token"
export GPUI_AGENT_ADDR="127.0.0.1:$port"
out=/Users/devin/jobs/a4screens/proof/logs/${label}-run.log
rm -f "$out" /Users/devin/jobs/a4screens/proof/logs/${label}.ready
if [ "$target" = ios ]; then
  /Users/devin/.cargo/bin/just nsh-run-agent ios "$db" --device E0224A22-F874-4B8C-83FB-EB084492C493 A4Proof.tsx >"$out" 2>&1 &
else
  /Users/devin/.cargo/bin/just nsh-run-agent macos "$db" A4Proof.tsx >"$out" 2>&1 &
fi
pid=$!
echo $pid > /Users/devin/jobs/a4screens/proof/logs/${label}.pid
for i in $(seq 1 240); do
  if GPUI_AGENT_TOKEN="a4proof-local-token" /Users/devin/.cargo/bin/gpui-agent --session "$session" attach "127.0.0.1:$port" >/tmp/${session}-attach.out 2>/tmp/${session}-attach.err; then
    echo attached >/Users/devin/jobs/a4screens/proof/logs/${label}.ready
    exit 0
  fi
  kill -0 "$pid" 2>/dev/null || exit 1
  sleep 3
done
exit 1
