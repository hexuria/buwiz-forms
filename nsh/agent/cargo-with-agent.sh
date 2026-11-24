#!/bin/sh
# Stands in for `cargo` while nsh builds the app's generated hosts, so they carry the app's `agent` feature.
#
# The generated workspace (nsh/.nsh/rust) lists this crate with no features and nsh's CLI has no flag for one, but it runs the
# program named by $CARGO, and cargo takes `--features <dependency>/<feature>` for a dependency of the host it builds. Only a
# `cargo build` in that generated workspace is changed (the build of the crate itself, and every other command, pass through).
# Used by `just nsh-run-agent`; nothing else sets $CARGO.
real="${NSH_REAL_CARGO:-cargo}"
if [ "$1" = "build" ]; then
    features="app_nsh_app_ebirforms/agent"
    for arg in "$@"; do
        case "$arg" in
            */.nsh/rust/Cargo.toml)
                manifest="$arg"
                shift
                # The generated workspace ignores the app crate's [patch]: patch entries apply only to the
                # workspace root, which is this generated manifest. The plug's crates name hexuria/nsh as git
                # dependencies, so without the patch the host links a second NSH and `install_window_plane`
                # registers its surface hook on the wrong copy. Re-append it, rooted at .nsh/rust.
                if ! grep -q 'patch."https://github.com/hexuria/nsh"' "$manifest"; then
                    cat >>"$manifest" <<'PATCH'

[patch."https://github.com/hexuria/nsh"]
nsh-core = { path = "../../../../nsh/crates/nsh-core" }
nsh-dev = { path = "../../../../nsh/crates/nsh-dev" }
nsh-testing = { path = "../../../../nsh/crates/nsh-testing" }
nsh-base = { path = "../../../../nsh/crates/nsh-base" }
nsh-cli = { path = "../../../../nsh/crates/nsh-cli" }
nsh-ios-host = { path = "../../../../nsh/crates/nsh-ios-host" }
nsh-linux-host = { path = "../../../../nsh/crates/nsh-linux-host" }
nsh-macos-host = { path = "../../../../nsh/crates/nsh-macos-host" }
nsh-windows-host = { path = "../../../../nsh/crates/nsh-windows-host" }
nsh-platform-android = { path = "../../../../nsh/crates/nsh-platform-android" }
nsh-platform-ios = { path = "../../../../nsh/crates/nsh-platform-ios" }
nsh-platform-linux = { path = "../../../../nsh/crates/nsh-platform-linux" }
nsh-platform-macos = { path = "../../../../nsh/crates/nsh-platform-macos" }
nsh-platform-windows = { path = "../../../../nsh/crates/nsh-platform-windows" }
PATCH
                fi
                exec "$real" build "$@" --features "$features"
                ;;
        esac
    done
fi
exec "$real" "$@"
