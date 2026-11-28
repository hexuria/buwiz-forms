#!/usr/bin/env bash
# The iOS half of `just nsh-run-agent`: the hand-off of the window plane's token.
#
#   ios-handoff.sh token PID [DEVICE]   writes $GPUI_AGENT_TOKEN into the host's data container on DEVICE
#                                       (default booted: the simulator nsh launches on, or --device's udid)
#                              (Library/Caches/gpui-agent-token), for as long as the launcher (PID) lives
#
# The window plane (the plug's surface hook, nsh-gpui-agent's Attachment::start) reads $HOME's
# Library/Caches/gpui-agent-token once per surface open, and deletes it: this writer keeps it present until the
# launch ends, so a surface opened after the first attach still finds one. The token's value leaves this machine's
# disk in no file — the container is the Simulator's own sandbox and the file is private to the host (mode 0600).
# It is written only while the launcher lives, as with the Android hand-off.
set -euo pipefail

fail() {
    echo "nsh-run-agent: $*" >&2
    exit 1
}

case "${1:-}" in
token)
    launcher="$2"
    device="${3:-booted}"
    [ -n "${GPUI_AGENT_TOKEN:-}" ] || fail "set GPUI_AGENT_TOKEN first"
    command -v xcrun >/dev/null 2>&1 || fail "no xcrun: install Xcode's command line tools"
    tries="${NSH_AGENT_TOKEN_TRIES:-1800}"
    for _ in $(seq "$tries"); do
        # A launch that failed or was cancelled installs nothing more: stop, and leave the container as it is.
        kill -0 "$launcher" 2>/dev/null || exit 0
        data="$(xcrun simctl get_app_container "$device" com.longbridge.nsh.host data 2>/dev/null || true)"
        if [ -n "$data" ]; then
            file="$data/Library/Caches/gpui-agent-token"
            # Written when absent, and when it holds another token: a run that ended can leave its own behind, and
            # the host would read that one instead of this run's (#449).
            if [ ! -f "$file" ] || [ "$(cat "$file" 2>/dev/null)" != "$GPUI_AGENT_TOKEN" ]; then
                umask 077
                mkdir -p "$data/Library/Caches"
                printf %s "$GPUI_AGENT_TOKEN" > "$file.new" && mv "$file.new" "$file"
            fi
        fi
        sleep "${NSH_AGENT_TOKEN_PAUSE:-0.5}"
    done
    fail "the window plane's token was not stored (is the host installed on the booted Simulator?)"
    ;;
*)
    fail "ios-handoff.sh: token PID [DEVICE]"
    ;;
esac
