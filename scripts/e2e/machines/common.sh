# Sourced by the per-platform scripts: the checks every machine makes before a smoke run, then the run.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)"
cd "$root"
need() { command -v "$1" >/dev/null 2>&1 || { echo "e2e: $1 is not installed: $2" >&2; exit 1; }; }
need just "install just (the setup script does)"
need gpui-agent "run the setup script: it installs gpui-agent-cli at the version Cargo.lock pins"
need python3 "install Python 3"
echo "app $(git rev-parse --short=8 HEAD), $(cargo install --list 2>/dev/null | grep '^gpui-agent-cli' || echo 'gpui-agent-cli: version unknown')"
just e2e check smoke
smoke() {
    # A dry-run test build only: the runner refuses the Queue recipe without these and --allow-send.
    EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1 BIR_SFTP_LIVE= just e2e "$1" smoke --allow-send "${@:2}"
}
