#!/usr/bin/env bash
# bir-shell's scroll bench: a release build (with the measurement-only
# `bench` feature, which lets it open a throwaway database without signing
# in) opens 1702RT with N parts expanded, waits for the page to settle,
# scrolls it down and back up at a fixed number of points per frame, and
# writes every over-budget frame plus a summary to a jank log (see
# crates/bir-shell/src/perf.rs). It prints the summary of each run.
#
#   scripts/bench-shell-scroll.sh                 # 12 parts, 40 pt/frame
#   PARTS="1 4 12" SPEEDS="40 160" scripts/bench-shell-scroll.sh
#   BIN=path/to/bir-shell scripts/bench-shell-scroll.sh   # skip the build
#   SIZE=1440x900 (default) is the window size under test
#
# Everything is isolated: a throwaway HOME, database and key, never the
# keychain or the real data. The window goes to AeroSpace workspace
# $BIR_WORKSPACE (2) when AeroSpace is installed; the app is quit after each
# run. Logs are kept in $OUT (default target/bench-shell-scroll).
set -euo pipefail
cd "$(dirname "$0")/.."
export RUSTC_WRAPPER=

PARTS="${PARTS:-12}"
SPEEDS="${SPEEDS:-40}"
FORM="${FORM:-1702RT/2025/0}"
WORKSPACE="${BIR_WORKSPACE:-2}"
SIZE="${SIZE:-1440x900}"
OUT="${OUT:-target/bench-shell-scroll}"
MANIFEST=crates/bir-shell/Cargo.toml
KEY=0000000000000000000000000000000000000000000000000000000000000000
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

TARGET="${CARGO_TARGET_DIR:-crates/bir-shell/target}"
if [ -z "${BIN:-}" ]; then
    cargo build -q --locked --release --manifest-path "$MANIFEST" --features bench --bin bir-shell --example seed_bench
    BIN="$TARGET/release/bir-shell"
fi
SEED="${SEED:-$TARGET/release/examples/seed_bench}"

# A release binary looks for app/ beside itself.
mkdir -p "$TMP/bin"
cp "$BIN" "$TMP/bin/bir-shell"
ln -s "$(pwd)/app" "$TMP/bin/app"

isolated() {
    exec env -i PATH=/usr/bin:/bin HOME="$TMP/home" USER="${USER:-bench}" TMPDIR="$TMP" \
        BIR_DATABASE_PATH="$TMP/run.db" BIR_DB_KEY=$KEY BIR_NO_KEYCHAIN=1 \
        BIR_LOG_DIR="$TMP/logs" EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1 "$@"
}

for parts in $PARTS; do
    for speed in $SPEEDS; do
        rm -rf "$TMP/home" "$TMP/logs" "$TMP"/run.db*
        mkdir -p "$TMP/home" "$TMP/logs"
        (isolated "$SEED" >/dev/null)
        log="$OUT/jank-${parts}parts-${speed}pt.log"
        : >"$log"
        last=$((parts - 1))
        # First launch: help desk mode with a throwaway PIN, then the form.
        route="setup=helpdesk;setuppin=1357;keydone;wait=2000;tp=000123453;form=$FORM;parts=0-$last"
        # `isolated` execs, so $! is the app itself.
        (isolated BIR_SHELL_START="$route" BIR_SHELL_SCROLL_BENCH="$speed" BIR_SHELL_JANK_LOG="$log" \
            BIR_SHELL_WINDOW_SIZE="$SIZE" \
            ${BIR_SHELL_FPS:+BIR_SHELL_FPS=$BIR_SHELL_FPS} "$TMP/bin/bir-shell" >"$TMP/app.out" 2>&1) &
        pid=$!
        moved=0
        for _ in $(seq 1 180); do
            if [ $moved = 0 ] && command -v aerospace >/dev/null; then
                id=$(aerospace list-windows --all --format '%{window-id}|%{app-pid}' 2>/dev/null | awk -F'|' -v p="$pid" '$2==p{print $1; exit}')
                if [ -n "$id" ]; then
                    aerospace move-node-to-workspace --window-id "$id" "$WORKSPACE" >/dev/null 2>&1 || true
                    # Floating, so tiling cannot change the size under test
                    # (BIR_SHELL_WINDOW_SIZE re-applies it for a few seconds).
                    aerospace layout floating --window-id "$id" >/dev/null 2>&1 || true
                    moved=1
                fi
            fi
            grep -q 'bench end' "$log" 2>/dev/null && break
            kill -0 "$pid" 2>/dev/null || break
            sleep 1
        done
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
        echo "== $FORM, $parts part(s) open, $speed pt/frame ($log)"
        grep -E 'scroll bench|bench:|frame gaps:|bench end' "$log" | cut -c14- || echo "   no summary: the bench did not finish"
    done
done
