#!/usr/bin/env bash
# The smoke suite in a real Linux desktop window (GTK). Without a display it runs under Xvfb. gpui-agent has no Linux
# window screenshots (screenshot_unavailable): record the run with the machine's own recorder.
source "$(dirname "$0")/common.sh"
if [ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
    need xvfb-run "apt install xvfb, or run on a machine with a display"
    exec xvfb-run -a env GDK_BACKEND=x11 GSK_RENDERER=cairo EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1 BIR_SFTP_LIVE= \
        just e2e linux smoke --allow-send "$@"
fi
smoke linux "$@"
