#!/bin/sh
# Stands in for `cargo` while `nsh package` builds the Mac App Store app, so the generated hosts carry the app's `mas_build`
# feature (no global hotkey, no shell cron runner, no updater).
#
# Same route as nsh/agent/cargo-with-agent.sh: nsh's CLI has no flag for a feature of the app crate, but it runs the program named
# by $CARGO, and cargo takes `--features <dependency>/<feature>` for a dependency of the host it builds. Only a `cargo build` in
# the generated workspace (nsh/.nsh/rust) is changed; every other command passes through. NSH_MAS_AGENT=1 adds the `agent`
# feature too (`just testflight-agent`). Used by `just _package-mac-appstore`; nothing else sets $CARGO to it.
real="${NSH_REAL_CARGO:-cargo}"
if [ "$1" = "build" ]; then
    features="app_nsh_app_ebirforms/mas_build"
    if [ "${NSH_MAS_AGENT:-}" = "1" ]; then
        features="$features,app_nsh_app_ebirforms/agent"
    fi
    for arg in "$@"; do
        case "$arg" in
            */.nsh/rust/Cargo.toml)
                shift
                exec "$real" build "$@" --features "$features"
                ;;
        esac
    done
fi
exec "$real" "$@"
