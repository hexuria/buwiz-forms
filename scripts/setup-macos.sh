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
#     (the gpui-agent-cli crate), installed at the exact version Cargo.lock
#     pins so CLI and host speak the same protocol (v1 CLI can't talk to a
#     v2 host)
#   - NSH app (nsh/) on macOS and the iOS Simulator: the Rust iOS targets,
#     a checkout of hexuria/nsh at the commit CI pins (NSH_PINNED_REF in
#     .github/workflows/nsh.yml) next to this repo, and the `nsh` command line
#     built from it into ~/BuwizNsh/tools (where the just nsh-* recipes look).
#     The iOS Simulator needs Apple Silicon (the pinned NSH builds an arm64
#     simulator host only) and the full Xcode app with an iOS runtime, not
#     only the Command Line Tools. Intel Macs get the NSH macOS app only.
#   - impeccable CLI (verification tooling) and the impeccable-rust agent skill
#     in .agents/skills/impeccable-rust/ (git-ignored, so downloaded), both at
#     the versions scripts/impeccable.pin holds (see scripts/setup-impeccable.sh)
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
#   scripts/setup-macos.sh --extras     # also install cargo-audit/outdated/machete
#   scripts/setup-macos.sh --no-agent   # skip installing the gpui-agent CLI
#   scripts/setup-macos.sh --no-nsh     # skip the NSH app (iOS targets, nsh CLI)
#   scripts/setup-macos.sh --no-impeccable  # skip the impeccable CLI and skill
#
set -euo pipefail

CHECK_ONLY=0
WITH_EXTRAS=0
WITH_AGENT=1
WITH_NSH=1
WITH_IMPECCABLE=1
for arg in "$@"; do
    case "$arg" in
        --check)    CHECK_ONLY=1 ;;
        --extras)   WITH_EXTRAS=1 ;;
        --no-agent) WITH_AGENT=0 ;;
        --no-nsh)   WITH_NSH=0 ;;
        --no-impeccable) WITH_IMPECCABLE=0 ;;
        -h|--help)
            sed -n '2,40p' "$0"
            exit 0
            ;;
        *) echo "Unknown option: $arg (see --help)" >&2; exit 2 ;;
    esac
done

[ "$(uname)" = "Darwin" ] || { echo "This script is for macOS only." >&2; exit 1; }

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }
# shellcheck source=scripts/lib/prebuilt.sh
. "$REPO_ROOT/scripts/lib/prebuilt.sh"
# shellcheck source=scripts/setup-impeccable.sh
. "$REPO_ROOT/scripts/setup-impeccable.sh"

BREW_PREFIX="$(uname -m | grep -q arm64 && echo /opt/homebrew || echo /usr/local)"
NSH_TOOLS="$HOME/BuwizNsh/tools"
# nsh/Cargo.toml points at ../../nsh, so the checkout must sit next to this repo.
NSH_DIR="$(dirname "$REPO_ROOT")/nsh"
IOS_TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim)
# The pinned NSH CLI always builds the simulator host for aarch64-apple-ios-sim
# (Platform::triple in crates/nsh-cli/src/runtime.rs), which an Intel
# simulator cannot launch. hw.optional.arm64 is 1 even under Rosetta.
IOS_SIM=0
[ "$(sysctl -n hw.optional.arm64 2>/dev/null)" = "1" ] && IOS_SIM=1
# Homebrew rustup's proxies honour rust-toolchain.toml (the NSH checkout pins
# its own toolchain); links made by link_toolchain_bins do not, so the proxy
# dir goes first whenever it exists.
if [ -d "$BREW_PREFIX/opt/rustup/bin" ]; then
    export PATH="$BREW_PREFIX/opt/rustup/bin:$PATH"
fi

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
    if [ -d "$BREW_PREFIX/opt/rustup/bin" ]; then
        persist_line 'export PATH="'"$BREW_PREFIX"'/opt/rustup/bin:$PATH"' 'opt/rustup/bin'
        export PATH="$BREW_PREFIX/opt/rustup/bin:$PATH"
    fi

    # `just app`/`_package-mac` lipo aarch64 + x86_64 into a universal binary.
    # On Apple Silicon the second target is missing; on Intel the first is.
    log "Adding macOS targets for universal builds"
    rustup target add aarch64-apple-darwin x86_64-apple-darwin
}

# ---------------------------------------------------------------------------
# gpui-agent CLI — drives the app over the opt-in control plane
# (GPUI_AGENT=1 + GPUI_AGENT_TOKEN on the host, then `gpui-agent hello`).
# Protocol version must match the host: read the version Cargo.lock pins for
# the library dep and install the CLI at that same version.
# ---------------------------------------------------------------------------
gpui_agent_version() {
    # Cargo.lock has, on two lines:
    #   name = "gpui-agent"
    #   version = "0.4.0"
    awk '$0 == "name = \"gpui-agent\"" { getline; gsub(/version = |"/, ""); print; exit }' \
        Cargo.lock 2>/dev/null
}

install_gpui_agent() {
    local version
    version="$(gpui_agent_version)"
    if [ -z "$version" ]; then
        warn "could not read the gpui-agent version from Cargo.lock — run from the repo root"
        return 0
    fi
    local installed_version=""
    if command -v gpui-agent >/dev/null 2>&1; then
        # A crates.io install lists as `gpui-agent-cli v0.4.0:`; one from git
        # carries its source after the version and never matches.
        installed_version="$(prebuilt_installed_version gpui-agent-cli gpui-agent)"
        if [ "$installed_version" = "$version" ]; then
            log "gpui-agent already installed at pinned version $version"
            return 0
        fi
    fi
    log "Installing gpui-agent CLI at pinned version $version"
    install_prebuilt_or_build gpui-agent-cli "$version"
}

# ---------------------------------------------------------------------------
# NSH app: iOS targets, hexuria/nsh at the CI pin, and the nsh CLI built from
# that checkout. The CLI and the checkout must match, so a moved pin rebuilds.
# ---------------------------------------------------------------------------
NSH_STAMP="$NSH_TOOLS/.buwiz-nsh-ref"

nsh_pinned_ref() {
    sed -n 's/^ *NSH_PINNED_REF: *\([0-9a-fA-F]\{7,64\}\).*/\1/p' "$REPO_ROOT/.github/workflows/nsh.yml" 2>/dev/null | head -1 || true
}

# Tracked changes only: they reach the app through its path dependencies.
nsh_dirty() {
    ! git -C "$NSH_DIR" diff --quiet 2>/dev/null || ! git -C "$NSH_DIR" diff --cached --quiet 2>/dev/null
}

# The CLI counts as built from the pin only when this script built it from a
# clean checkout there (the stamp); its embedded HEAD alone says nothing about
# local edits.
nsh_cli_at() {
    "$NSH_TOOLS/bin/nsh" --version 2>/dev/null | grep -q "(${1:0:7})" \
        && [ "$(cat "$NSH_STAMP" 2>/dev/null)" = "$1" ]
}

# The toolchain the NSH checkout pins. Naming it with RUSTUP_TOOLCHAIN keeps
# rustup from adding the Android targets its rust-toolchain.toml also lists.
nsh_toolchain() {
    local channel
    channel="$(sed -n 's/^ *channel *= *"\([^"]*\)".*/\1/p' "$NSH_DIR/rust-toolchain.toml" 2>/dev/null | head -1 || true)"
    echo "${channel:-stable}"
}

install_nsh() {
    if [ "$IOS_SIM" -eq 0 ]; then
        warn "Intel Mac: skipping the iOS Simulator (the pinned NSH builds an arm64-only simulator host); the NSH macOS app still works"
    else
        if [ "$(xcode-select -p 2>/dev/null)" = "/Library/Developer/CommandLineTools" ]; then
            warn "Xcode.app is not selected: the iOS Simulator needs it (sudo xcode-select -s /Applications/Xcode.app)"
        elif ! xcrun simctl list runtimes available 2>/dev/null | grep -q '^iOS'; then
            warn "no available iOS Simulator runtime: install one with 'xcodebuild -downloadPlatform iOS'"
        fi
        log "Adding iOS targets: ${IOS_TARGETS[*]}"
        rustup target add "${IOS_TARGETS[@]}"
    fi

    local ref
    ref="$(nsh_pinned_ref)"
    [ -n "$ref" ] || die "could not read NSH_PINNED_REF from $REPO_ROOT/.github/workflows/nsh.yml"
    if [ ! -e "$NSH_DIR/.git" ]; then
        log "Cloning hexuria/nsh into $NSH_DIR"
        git clone https://github.com/hexuria/nsh "$NSH_DIR"
    fi
    if nsh_dirty; then
        die "$NSH_DIR has uncommitted changes to tracked files, and the app would build against them.
Keep them with 'git -C $NSH_DIR stash' (or commit them on a branch), then re-run this script."
    fi
    if [ "$(git -C "$NSH_DIR" rev-parse HEAD 2>/dev/null)" != "$ref" ]; then
        log "Checking out NSH at the CI pin ${ref:0:7}"
        git -C "$NSH_DIR" fetch origin "$ref"
        git -C "$NSH_DIR" checkout --detach "$ref"
    fi

    if nsh_cli_at "$ref"; then
        log "nsh CLI already built at ${ref:0:7}"
    else
        local toolchain
        toolchain="$(nsh_toolchain)"
        log "Building the nsh CLI into $NSH_TOOLS with Rust $toolchain (about 10 minutes cold)"
        rustup toolchain install "$toolchain" --profile minimal
        rm -f "$NSH_STAMP"
        (cd "$NSH_DIR" && RUSTUP_TOOLCHAIN="$toolchain" cargo install --locked --path crates/nsh-cli --root "$NSH_TOOLS")
        printf '%s\n' "$ref" > "$NSH_STAMP"
    fi
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
    # gpui-agent CLI — required for agent-driven control unless --no-agent;
    # its version must match the Cargo.lock pin so protocol versions agree.
    if [ "$WITH_AGENT" -eq 1 ]; then
        if command -v gpui-agent >/dev/null 2>&1; then
            printf '  %-14s %s\n' "gpui-agent" "$(command -v gpui-agent)"
        else
            printf '  %-14s MISSING (needed to drive the app over the control plane)\n' "gpui-agent"
            ok=0
        fi
    fi
    if [ "$WITH_NSH" -eq 1 ]; then
        local ref
        ref="$(nsh_pinned_ref)"
        if [ -z "$ref" ]; then
            printf '  %-14s MISSING (no NSH_PINNED_REF in %s/.github/workflows/nsh.yml)\n' "nsh-pin" "$REPO_ROOT"
            ok=0
        fi
        if [ ! -f "$NSH_DIR/crates/nsh-core/Cargo.toml" ]; then
            printf '  %-14s MISSING (%s, the app'"'"'s path dependencies)\n' "nsh-source" "$NSH_DIR"
            ok=0
        elif [ -n "$ref" ] && [ "$(git -C "$NSH_DIR" rev-parse HEAD 2>/dev/null)" != "$ref" ]; then
            printf '  %-14s %s is not at the CI pin %s\n' "nsh-source" "$NSH_DIR" "${ref:0:7}"
            ok=0
        elif nsh_dirty; then
            printf '  %-14s %s has uncommitted changes to tracked files\n' "nsh-source" "$NSH_DIR"
            ok=0
        else
            printf '  %-14s %s at %s\n' "nsh-source" "$NSH_DIR" "${ref:0:7}"
        fi
        if [ ! -x "$NSH_TOOLS/bin/nsh" ]; then
            printf '  %-14s MISSING (%s/bin/nsh, needed by the just nsh-* recipes)\n' "nsh" "$NSH_TOOLS"
            ok=0
        elif [ -n "$ref" ] && ! nsh_cli_at "$ref"; then
            printf '  %-14s %s is not built by this script from the CI pin %s\n' "nsh" "$NSH_TOOLS/bin/nsh" "${ref:0:7}"
            ok=0
        else
            printf '  %-14s %s\n' "nsh" "$("$NSH_TOOLS/bin/nsh" --version 2>/dev/null | head -1)"
        fi
    fi
    if [ "$WITH_NSH" -eq 1 ] && [ "$IOS_SIM" -eq 0 ]; then
        printf '  %-14s %s\n' "ios-sim" "skipped: Intel Mac (the pinned NSH builds an arm64-only simulator host)"
    elif [ "$WITH_NSH" -eq 1 ]; then
        if command -v rustup >/dev/null 2>&1; then
            local t missing_ios=""
            for t in "${IOS_TARGETS[@]}"; do
                rustup target list --installed 2>/dev/null | grep -qx "$t" || missing_ios="$missing_ios $t"
            done
            if [ -n "$missing_ios" ]; then
                printf '  %-14s MISSING:%s\n' "ios-targets" "$missing_ios"
                ok=0
            else
                printf '  %-14s %s\n' "ios-targets" "${IOS_TARGETS[*]}"
            fi
        fi
        if xcrun simctl list runtimes available 2>/dev/null | grep -q '^iOS'; then
            printf '  %-14s %s\n' "ios-runtime" "$(xcrun simctl list runtimes available 2>/dev/null | grep '^iOS' | tail -1 | cut -d' ' -f1-2)"
        else
            warn "no available iOS Simulator runtime (needs Xcode.app; xcodebuild -downloadPlatform iOS)"
        fi
    fi
    if [ "$WITH_IMPECCABLE" -eq 1 ]; then
        check_impeccable 14 || ok=0
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
    if [ "$WITH_AGENT" -eq 1 ]; then
        install_gpui_agent
    fi
    if [ "$WITH_NSH" -eq 1 ]; then
        install_nsh
    fi
    if [ "$WITH_IMPECCABLE" -eq 1 ]; then
        install_impeccable
        install_impeccable_skill
    fi

    if [ "$WITH_EXTRAS" -eq 1 ]; then
        log "Installing optional developer tools"
        install_prebuilt_or_build cargo-audit 0.22.1
        install_prebuilt_or_build cargo-outdated 0.19.0
        install_prebuilt_or_build cargo-machete 0.9.2
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

NSH app (nsh/), with the CLI built at the CI pin:

    just nsh-check && just nsh-build && just nsh-test
    just nsh-run                       # the NSH app on macOS

Packaging tools already on macOS: lipo, codesign, PlistBuddy, productbuild.
OpenSSL is vendored — no brew openssl needed; system perl + cc build it.

To drive the app from the CLI (gpui-agent was installed at the Cargo.lock
rev): build the host with the agent feature and launch with env vars —

    cargo build --locked -p bir-cli
    export GPUI_AGENT=1 GPUI_AGENT_TOKEN=dev-secret
    ./target/debug/bir serve &        # then, another shell:
    gpui-agent hello            # protocol, app, platform, ready
    gpui-agent keybindings      # {id, chord, scope, dangerous}

Full contract: docs/agent.md

EOF
    if [ "$WITH_NSH" -eq 1 ] && [ "$IOS_SIM" -eq 1 ]; then
        cat <<'EOF'
The NSH app on the iOS Simulator (Apple Silicon), from nsh/:

    just mock-bir & just mock-relay &
    PATH="$HOME/BuwizNsh/tools/bin:$PATH" CARGO_TARGET_DIR="$HOME/BuwizNsh/target-nsh" \
      SIMCTL_CHILD_BIR_RELAY_URL=http://127.0.0.1:8787 SIMCTL_CHILD_BUWIZ_NO_KEYCHAIN=1 \
      SIMCTL_CHILD_EBIR_TEST_ENV=1 SIMCTL_CHILD_BIR_AUTH_BYPASS=1 SIMCTL_CHILD_BIR_SFTP_DRY_RUN=1 \
      nsh run --target ios

EOF
    fi
}

main
