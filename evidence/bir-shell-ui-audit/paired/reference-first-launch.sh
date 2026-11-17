#!/usr/bin/env bash
set -euo pipefail
for name in ${!BIR_SFTP_@}; do unset "$name"; done
unset EBIRFORMS_SEED BIR_AUTH_BYPASS ZED_HEADLESS
export BIR_SFTP_DRY_RUN=1
export EBIRFORMS_DATA_DIR=/audit/headed-data HOME=/audit/headed-home
export XDG_DATA_HOME="$HOME/.local/share" XDG_CONFIG_HOME="$HOME/.config" XDG_CACHE_HOME="$HOME/.cache"
export DISPLAY=:0 XAUTHORITY=/audit.xauth LIBGL_ALWAYS_SOFTWARE=1 VK_ICD_FILENAMES=/dev/null
export XDG_RUNTIME_DIR=/audit/headed-runtime WAYLAND_DISPLAY=wayland-audit
export PATH=/bun:/usr/bin:/bin
test ! -e "$EBIRFORMS_DATA_DIR"
test ! -e "$HOME"
printf '%s\n' "Before headed first launch $(date -u --iso-8601=seconds)" 'ABSENT /audit/headed-data (/tmp/ebirforms-paired-audit-20261101/headed-data)' 'ABSENT /audit/headed-home' 'ABSENT /audit/headed-data/ebirforms.sqlite' 'No seed/bypass; all inherited BIR_SFTP_* stripped; only BIR_SFTP_DRY_RUN=1' > /audit/reference-headed-fresh-state.txt
mkdir -p "$HOME"
cd /app/apps/desktop
exec /bun/bun src/app.tsx
