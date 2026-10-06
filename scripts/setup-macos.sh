#!/usr/bin/env bash
#
# setup-macos.sh — install everything needed to build and run E-BIRForms on macOS.
#
# What it installs:
#   - Xcode Command Line Tools (cc/clang, lipo, codesign, PlistBuddy,
#     productbuild — the universal-binary and .app packaging recipes need them)
#   - Homebrew (if missing), then: just, uv, python@3.13, create-dmg
#   - Rust toolchain via rustup (if cargo is missing), plus BOTH darwin
#     targets — `just app` / `just _package-mac` build aarch64 + x86_64 and
#     lipo them into a universal binary
#   - gpui-agent CLI — the control-plane client that drives the app
#     (github.com/hexuria/gpui-agent), installed at the exact rev Cargo.lock
#     pins so CLI and host speak the same protocol (v1 CLI can't talk to a
#     v2 host)
#
# What it does NOT need (unlike Windows/Linux):
#   - No system OpenSSL: openssl-src compiles from vendored source with the
#     system perl + cc that macOS already ships
#   - No WebKitGTK/GTK: previews render through the platform WKWebView
#   - No font packages: the UI font resolves from the system font set
#
# Usage:
#   scripts/setup-macos.sh              # install everything
#   scripts/setup-macos.sh --check      # verify only, install nothing
#   scripts/setup-macos.sh --extras     # also install cargo-audit/outdated/machete + node
#
set -euo pipefail

CHECK_ONLY=0
WITH_EXTRAS=0
for arg in "$@"; do
    case "$arg" in
        --check)   CHECK_ONLY=1 ;;
        --extras)  WITH_EXTRAS=1 ;;
        -h|--help)
            sed -n '2,30p' "$0"
            exit 0
            ;;
        *) echo "Unknown option: $arg (see --help)" >&2; exit 2 ;;
    esac
done

[ "$(uname)" = "Darwin" ] || { echo "This script is for macOS only." >&2; exit 1; }

log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

BREW_PREFIX="$(uname -m | grep -q arm64 && echo /opt/homebrew || echo /usr/local)"

# ---------------------------------------------------------------------------
# Xcode Command Line Tools — cc, lipo, codesign, PlistBuddy, productbuild all
# come from here. `xcode-select --install` opens a GUI dialog and returns
# immediately, so a fresh machine has to finish that install and re-run us.
# ---------------------------------------------------------------------------
ensure_xcode_clt() {
    if xcode-select -p >/dev/null 2>&1 && command -v cc >/dev/null 2>&1; then
        log "Xcode Command Line Tools present: $(xcode-select -p)"
        return 0
    fi
    log "Installing Xcode Command Line Tools (GUI dialog)..."
    xcode-select --install || true
    cat >&2 <<'EOF'
Finish the "Command Line Developer Tools" install dialog, then re-run
this script. If the dialog claims the tools are already installed but
`xcode-select -p` fails, run `sudo xcode-select --reset` first.
EOF
    exit 1
}

# ---------------------------------------------------------------------------
# Homebrew
# ---------------------------------------------------------------------------
ensure_brew() {
    if command -v brew >/dev/null 2>&1; then
        log "Homebrew already installed: $(brew --version | head -1)"
        return 0
    fi
    log "Installing Homebrew (will ask for your password)"
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    # The installer does not touch the current shell; add brew to PATH now
    # and persist it for future shells.
    eval "$("$BREW_PREFIX/bin/brew" shellenv)"
    persist_line 'eval "$('"$BREW_PREFIX"'/bin/brew shellenv)"' 'brew shellenv'
    command -v brew >/dev/null 2>&1 || die "brew still not on PATH after install"
}

# ---------------------------------------------------------------------------
# Shell profile persistence — append a line to ~/.zprofile once.
# persist_line <line> <marker-substring>
# ---------------------------------------------------------------------------
persist_line() {
    local line="$1" marker="$2" profile="$HOME/.zprofile"
    touch "$profile"
    if ! grep -qF "$marker" "$profile"; then
        printf '%s\n' "$line" >> "$profile"
        log "Added to ~/.zprofile: $line"
    fi
}

# ---------------------------------------------------------------------------
# Rust via rustup. Two sources are supported:
#   1. rustup.rs installer (README's documented path) — creates ~/.cargo/bin
#      proxies, so `cargo` lands on PATH by itself.
#   2. Homebrew `rustup` — installs toolchains under ~/.rustup but creates
#      NO proxies: symlinked calls dispatch as `rustup`, not the toolchain.
#      We link the toolchain's real binaries into ~/.cargo/bin instead.
# ---------------------------------------------------------------------------
link_toolchain_bins() {
    # Only needed when `rustup` exists but `cargo` is not on PATH.
    command -v rustup >/dev/null 2>&1 || return 1
    command -v cargo  >/dev/null 2>&1 && return 0
    local toolchain_bin
    toolchain_bin="$(rustup which cargo 2>/dev/null | tail -1 | xargs dirname 2>/dev/null)"
    [ -n "$toolchain_bin" ] && [ -d "$toolchain_bin" ] || return 1
    mkdir -p "$HOME/.cargo/bin"
    local bin
    for bin in "$toolchain_bin"/*; do
        ln -sf "$bin" "$HOME/.cargo/bin/$(basename "$bin")"
    done
    hash -r
    command -v cargo >/dev/null 2>&1
}

install_rust() {
    if command -v cargo >/dev/null 2>&1; then
        log "Rust already installed: $(cargo --version)"
    elif command -v rustup >/dev/null 2>&1; then
        log "rustup present ($(rustup --version 2>/dev/null | head -1)); installing stable toolchain"
        rustup default stable
        link_toolchain_bins || warn "cargo still not on PATH — open a new shell or check rustup"
    else
        log "Installing Rust via rustup"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi

    # ~/.cargo/bin must precede any rustup shim dir in every future shell.
    persist_line 'export PATH="$HOME/.cargo/bin:$PATH"' '.cargo/bin'
    export PATH="$HOME/.cargo/bin:$PATH"

    # `just app`/`_package-mac` lipo aarch64 + x86_64 into a universal binary.
    # On Apple Silicon the second target is missing; on Intel the first is.
    log "Adding macOS targets for universal builds"
    rustup target add aarch64-apple-darwin x86_64-apple-darwin
}

# ---------------------------------------------------------------------------
# gpui-agent CLI — drives the app over the opt-in control plane
# (GPUI_AGENT=1 + GPUI_AGENT_TOKEN on the host, then `gpui-agent hello`).
# Protocol version must match the host: read the rev Cargo.lock pins for the
# library dep and install the CLI at that same rev.
# ---------------------------------------------------------------------------
gpui_agent_rev() {
    # Cargo.lock line looks like:
    #   source = "git+https://github.com/hexuria/gpui-agent?rev=<sha>#<sha>"
    sed -n 's/.*gpui-agent?rev=\([0-9a-fA-F]\{7,64\}\)#.*/\1/p' Cargo.lock 2>/dev/null | head -1
}

install_gpui_agent() {
    local rev
    rev="$(gpui_agent_rev)"
    if [ -z "$rev" ]; then
        warn "could not read gpui-agent rev from Cargo.lock — run from the repo root"
        return 0
    fi
    local installed_rev=""
    if command -v gpui-agent >/dev/null 2>&1; then
        installed_rev="$(cargo install --list 2>/dev/null | sed -n 's/.*gpui-agent?rev=\([0-9a-fA-F]\{7,64\}\)#.*/\1/p' | head -1)"
        if [ "$installed_rev" = "$rev" ]; then
            log "gpui-agent already installed at pinned rev ${rev:0:7}"
            return 0
        fi
    fi
    log "Installing gpui-agent CLI at pinned rev ${rev:0:7}"
    cargo install --git https://github.com/hexuria/gpui-agent --rev "$rev" gpui-agent-cli
}

# ---------------------------------------------------------------------------
# Brew packages. python@3.13 is versioned: it ships `python3.13` plus an
# unversioned bin dir at <prefix>/opt/python@3.13/libexec/bin — prepend that
# to PATH when you want `python3` to be 3.13 (Xcode's 3.9 wins otherwise).
# ---------------------------------------------------------------------------
install_brew_packages() {
    local pkgs=(just uv python@3.13 create-dmg)
    local missing=()
    local p
    for p in "${pkgs[@]}"; do
        brew list --versions "$p" >/dev/null 2>&1 || missing+=("$p")
    done
    if [ "${#missing[@]}" -eq 0 ]; then
        log "Brew packages already installed: ${pkgs[*]}"
    else
        log "Installing brew packages: ${missing[*]}"
        brew install "${missing[@]}"
    fi
    if command -v python3 >/dev/null 2>&1; then
        local pyver
        pyver="$(python3 -c 'import sys; print("%d.%d" % sys.version_info[:2])' 2>/dev/null || echo "?")"
        if [ "$(python3 -c 'import sys; print(sys.version_info < (3, 13))' 2>/dev/null)" = "True" ]; then
            warn "python3 is $pyver (Xcode's); freeze/stamp CI scripts expect 3.13."
            warn "  To make python3.13 the default, add to your PATH:"
            warn "    export PATH=\"$BREW_PREFIX/opt/python@3.13/libexec/bin:\$PATH\""
        fi
    fi
}

# ---------------------------------------------------------------------------
# --check mode: verify the toolchain end to end
# ---------------------------------------------------------------------------
check() {
    local ok=1 ver=""
    if xcode-select -p >/dev/null 2>&1; then
        printf '  %-14s %s\n' "xcode-clt" "$(xcode-select -p)"
    else
        printf '  %-14s MISSING (run xcode-select --install)\n' "xcode-clt"
        ok=0
    fi
    for tool in cc cargo rustc just uv perl lipo codesign; do
        if command -v "$tool" >/dev/null 2>&1; then
            # perl prints a blank line first; lipo/codesign have no --version.
            case "$tool" in
                perl)              ver="$(perl -e 'print $^V')" ;;
                lipo|codesign)     ver="$(command -v "$tool")" ;;
                *)                 ver="$("$tool" --version 2>/dev/null | head -1)" ;;
            esac
            printf '  %-14s %s\n' "$tool" "$ver"
        else
            printf '  %-14s MISSING\n' "$tool"
            ok=0
        fi
    done
    # python3: warn-level only — `just run`/`just check` don't call it, but
    # freeze/stamp/packaging scripts do and CI pins 3.13.
    if command -v python3 >/dev/null 2>&1; then
        printf '  %-14s %s\n' "python3" "$(python3 --version 2>&1) ($(command -v python3))"
        [ "$(python3 -c 'import sys; print(sys.version_info < (3, 13))' 2>/dev/null)" = "True" ] \
            && warn "python3 < 3.13 — freeze/stamp scripts expect 3.13 (brew python@3.13 ships python3.13)"
    else
        printf '  %-14s MISSING\n' "python3"
        ok=0
    fi
    # Universal-build targets (only needed for `just app`, not `just run`).
    if command -v rustup >/dev/null 2>&1; then
        local t missing_targets=""
        for t in aarch64-apple-darwin x86_64-apple-darwin; do
            rustup target list --installed 2>/dev/null | grep -qx "$t" || missing_targets="$missing_targets $t"
        done
        if [ -n "$missing_targets" ]; then
            warn "rust targets missing (needed by 'just app'):$missing_targets"
        else
            printf '  %-14s %s\n' "rust-targets" "aarch64 + x86_64 apple-darwin"
        fi
    fi
    # create-dmg is optional — `just _package-mac` falls back to .zip.
    if command -v create-dmg >/dev/null 2>&1; then
        printf '  %-14s %s\n' "create-dmg" "$(command -v create-dmg)"
    else
        warn "create-dmg not installed — 'just app' will produce a .zip instead of a .dmg"
    fi
    # gpui-agent CLI — required for agent-driven control; rev must match the
    # Cargo.lock pin so protocol versions agree.
    if command -v gpui-agent >/dev/null 2>&1; then
        printf '  %-14s %s\n' "gpui-agent" "$(command -v gpui-agent)"
    else
        printf '  %-14s MISSING (needed to drive the app over the control plane)\n' "gpui-agent"
        ok=0
    fi
    [ "$ok" -eq 1 ] && log "All required tooling present" || warn "Missing pieces — run scripts/setup-macos.sh"
    return $((1 - ok))
}

# ---------------------------------------------------------------------------
main() {
    if [ "$CHECK_ONLY" -eq 1 ]; then
        check
        exit $?
    fi

    ensure_xcode_clt
    ensure_brew
    install_brew_packages
    install_rust
    install_gpui_agent

    if [ "$WITH_EXTRAS" -eq 1 ]; then
        log "Installing optional developer tools"
        cargo install cargo-audit cargo-outdated cargo-machete
        brew list --versions node >/dev/null 2>&1 || brew install node
    fi

    log "Verifying toolchain"
    check

    cat <<'EOF'

Done. Next steps:

    just run      # build + launch the desktop app (dev-tools feature)
    just check    # fmt + cargo check + clippy
    just test     # workspace tests
    just app      # universal .app + DMG (ad-hoc signed; needs App Store
                  # Connect creds in .env for the build-number lookup)

Packaging tools already on macOS: lipo, codesign, PlistBuddy, productbuild.
OpenSSL is vendored — no brew openssl needed; system perl + cc build it.

To drive the app from the CLI (gpui-agent was installed at the Cargo.lock
rev): build the host with the agent feature and launch with env vars —

    cargo build --locked --bin bir --features agent
    export GPUI_AGENT=1 GPUI_AGENT_TOKEN=dev-secret
    ./target/debug/bir &        # then, another shell:
    gpui-agent hello            # protocol, app, platform, ready
    gpui-agent keybindings      # {id, chord, scope, dangerous}

Full contract: crates/bir-desktop/docs/AGENT.md

EOF
}

main
