# Sourced by the per-platform scripts: the checks every machine makes before a smoke run, then the run.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)"
cd "$root"
# shellcheck source=scripts/lib/prebuilt.sh
. "$root/scripts/lib/prebuilt.sh"
need() { command -v "$1" >/dev/null 2>&1 || { echo "e2e: $1 is not installed: $2" >&2; exit 1; }; }
need just "install just (the setup script does)"
need gpui-agent "run the setup script: it installs gpui-agent-cli at the scripts/impeccable.pin version"
need python3 "install Python 3"
gpui_agent_version="$(installed_pinned_version gpui-agent-cli gpui-agent)"
echo "app $(git rev-parse --short=8 HEAD), gpui-agent-cli ${gpui_agent_version:-unknown}"
just e2e check smoke
just e2e check rdo-row-press
smoke() {
    # A dry-run test build only: the runner refuses the Queue recipe without these and --allow-send.
    EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1 BIR_SFTP_LIVE= just e2e "$1" smoke --allow-send "${@:2}"
}
# A desktop machine's default run: smoke, then rdo-row-press (the SearchPick combobox under a person's presses, the
# #425 Windows case and #458). Both run; the run fails if either does.
desktop_suites() {
    local status=0
    smoke "$@" || status=$?
    just e2e "$1" rdo-row-press "${@:2}" || status=$?
    return "$status"
}
