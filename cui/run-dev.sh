#!/usr/bin/env bash
# Dev launch for the CUI app: persistent database (data survives restarts), auth bypass,
# dry-run sending, and the dev start route selecting the seeded taxpayer.
set -euo pipefail
cd "$(dirname "$0")"
export DISPLAY="${DISPLAY:-:1}"
export BIR_DATABASE_PATH="${BIR_DATABASE_PATH:-$HOME/.local/share/buwiz-cui/bir_data.db}"
export BIR_AUTH_BYPASS=1 EBIR_TEST_ENV=1 BUWIZ_NO_KEYCHAIN=1 BIR_SFTP_DRY_RUN=1
export BIR_SHELL_START='tp=000123453;branch=00000;theme=dark'
exec ./target/debug/cui-app-ebirforms "$@"
