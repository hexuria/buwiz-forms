#!/bin/sh
# Stands in for `cargo` while nsh builds the app's generated hosts, so they carry the app's `agent` feature.
#
# The generated workspace (nsh/.nsh/rust) lists this crate with no features and nsh's CLI has no flag for one, but it runs the
# program named by $CARGO, and cargo takes `--features <dependency>/<feature>` for a dependency of the host it builds. Only a
# `cargo build` in that generated workspace is changed (the build of the crate itself, and every other command, pass through).
# Used by `just nsh-run-agent`; nothing else sets $CARGO.
real="${NSH_REAL_CARGO:-cargo}"
if [ "$1" = "build" ]; then
    for arg in "$@"; do
        # On Windows nsh names the manifest with either slash, even both (...\nsh\.nsh/rust\Cargo.toml): compare and
        # open it with forward slashes only, which every system's tools take.
        # shellcheck disable=SC1003
        case "$(printf '%s' "$arg" | tr '\\' '/')" in
            */.nsh/rust/Cargo.toml)
                # shellcheck disable=SC1003
                manifest="$(printf '%s' "$arg" | tr '\\' '/')"
                # The plug's crates name hexuria/nsh as Git dependencies. The generated workspace is its own workspace
                # root, so it has to redirect them to the app's checkout itself, or the host links a second NSH and the
                # window's listener never starts (no error). nsh writes that table since the pinned commit: it carries
                # the app's own [patch] tables over. An older nsh does not, so stop here instead of building that.
                if ! grep -q 'patch."https://github.com/hexuria/nsh"' "$manifest"; then
                    echo "cargo-with-agent: $manifest has no [patch.\"https://github.com/hexuria/nsh\"] table." >&2
                    echo "  This nsh is older than the app's NSH pin (NSH_PINNED_REF in .github/workflows/nsh.yml)." >&2
                    echo "  Rebuild it from the sibling checkout at that commit:" >&2
                    echo "  cargo install --locked --path ../nsh/crates/nsh-cli --root ~/BuwizNsh/tools" >&2
                    exit 1
                fi
                shift
                exec "$real" build "$@" --features app_nsh_app_ebirforms/agent
                ;;
        esac
    done
fi
exec "$real" "$@"
