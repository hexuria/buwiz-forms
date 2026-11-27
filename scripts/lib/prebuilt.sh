# Shared prebuilt-tool installer for the Bash setup scripts. Sourced, not run:
# cargo-binstall is downloaded at scripts/impeccable.pin's CARGO_BINSTALL_VERSION
# and crates use only their own crate-meta-data release artifacts. Every failure
# falls back to the caller's normal cargo install from source.

PREBUILT_PIN_FILE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/impeccable.pin"

prebuilt_pin() {  # prebuilt_pin <KEY> → its value in scripts/impeccable.pin
    sed -n "s/^$1=\([^[:space:]]*\).*/\1/p" "$PREBUILT_PIN_FILE" 2>/dev/null | head -1 || true
}

# cargo install writes to $CARGO_INSTALL_ROOT, else $CARGO_HOME, else ~/.cargo.
prebuilt_cargo_bin() {
    printf '%s\n' "${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin"
}

prebuilt_use_cargo_bin() {
    local bin
    bin="$(prebuilt_cargo_bin)"
    case ":$PATH:" in
        *":$bin:"*) ;;
        *) export PATH="$bin:$PATH" ;;
    esac
}

prebuilt_binstall_asset() {
    case "$(uname -s):$(uname -m)" in
        Linux:x86_64|Linux:amd64)
            printf '%s\n' "cargo-binstall-x86_64-unknown-linux-musl.tgz" ;;
        Linux:aarch64|Linux:arm64)
            printf '%s\n' "cargo-binstall-aarch64-unknown-linux-musl.tgz" ;;
        Darwin:x86_64)
            printf '%s\n' "cargo-binstall-x86_64-apple-darwin.zip" ;;
        Darwin:arm64|Darwin:aarch64)
            printf '%s\n' "cargo-binstall-aarch64-apple-darwin.zip" ;;
        *)
            return 1 ;;
    esac
}

prebuilt_binstall_at_pin() {  # prebuilt_binstall_at_pin <version>
    local output
    output="$(cargo-binstall -V 2>/dev/null)" || return 1
    [ "$output" = "$1" ] || [ "$output" = "cargo-binstall $1" ]
}

# ensure_cargo_binstall: true when the pinned cargo-binstall is usable. Download
# or extraction failure is deliberately non-fatal so callers can build from source.
ensure_cargo_binstall() {
    prebuilt_use_cargo_bin
    [ "${BUWIZ_SETUP_NO_PREBUILT:-0}" != "1" ] || return 1

    local pin asset url bin tmp extract source
    pin="$(prebuilt_pin CARGO_BINSTALL_VERSION)"
    [ -n "$pin" ] || return 1
    if prebuilt_binstall_at_pin "$pin"; then
        return 0
    fi
    asset="$(prebuilt_binstall_asset)" || return 1
    bin="$(prebuilt_cargo_bin)"
    mkdir -p "$bin" || return 1
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/cargo-binstall.XXXXXX")" || return 1
    url="https://github.com/cargo-bins/cargo-binstall/releases/download/v$pin/$asset"
    if ! curl -fsSL -o "$tmp/$asset" "$url"; then
        rm -rf "$tmp"
        return 1
    fi
    extract="$tmp/extract"
    mkdir -p "$extract"
    case "$asset" in
        *.tgz)
            if ! tar -xzf "$tmp/$asset" -C "$extract"; then
                rm -rf "$tmp"
                return 1
            fi
            ;;
        *.zip)
            if ! command -v unzip >/dev/null 2>&1 || ! unzip -q -o "$tmp/$asset" -d "$extract"; then
                rm -rf "$tmp"
                return 1
            fi
            ;;
    esac
    if [ -f "$extract/cargo-binstall" ]; then
        source="$extract/cargo-binstall"
    else
        source="$(find "$extract" -type f -name cargo-binstall -print -quit)"
    fi
    if [ -z "$source" ] || ! cp "$source" "$bin/cargo-binstall"; then
        rm -rf "$tmp"
        return 1
    fi
    chmod +x "$bin/cargo-binstall" 2>/dev/null || true
    rm -rf "$tmp"
    hash -r
    prebuilt_binstall_at_pin "$pin"
}

prebuilt_write_stamp() {  # prebuilt_write_stamp <crate> <version>
    local crate="$1" version="$2" bin
    bin="$(prebuilt_cargo_bin)"
    [ -d "$bin" ] || return 0
    printf '%s\n' "$version" > "$bin/.$crate.buwiz-prebuilt-version" 2>/dev/null || true
}

# prebuilt_installed_version <crate> <binary>: cargo-binstall versions at least
# some installs in Cargo's own install list, but the stamp beside the binary keeps
# detection working where it does not. Cargo's list wins so a later source build
# can never be hidden by an older stamp.
prebuilt_installed_version() {
    local crate="$1" binary="${2:-$1}" installed bin stamp
    installed="$( (cargo install --list 2>/dev/null || true) | sed -n \
        "s/^$crate v\([0-9][0-9.]*\):$/\1/p" | head -1)"
    if [ -n "$installed" ]; then
        printf '%s\n' "$installed"
        return 0
    fi
    bin="$(prebuilt_cargo_bin)"
    stamp="$bin/.$crate.buwiz-prebuilt-version"
    if [ -x "$bin/$binary" ] && [ -s "$stamp" ]; then
        cat "$stamp"
    fi
}

# install_prebuilt_or_build <crate> <version> [extra cargo-install args]
# Tries only the crate's own release metadata; cargo-binstall is never allowed to
# use a third-party prebuilt store or to compile implicitly here.
install_prebuilt_or_build() {
    local crate="$1" version="$2"
    shift 2
    prebuilt_use_cargo_bin
    if [ "${BUWIZ_SETUP_NO_PREBUILT:-0}" = "1" ]; then
        printf 'prebuilt installs disabled for %s; using cargo install\n' "$crate"
        cargo install "$crate" --version "$version" "$@"
        return
    fi
    if ! ensure_cargo_binstall; then
        printf 'cargo-binstall unavailable for %s; falling back to cargo install\n' "$crate"
        cargo install "$crate" --version "$version" "$@"
        return
    fi
    if cargo binstall --no-confirm --strategies crate-meta-data --version "$version" "$crate"; then
        prebuilt_write_stamp "$crate" "$version"
        return 0
    fi
    printf 'cargo binstall failed for %s; falling back to cargo install\n' "$crate"
    cargo install "$crate" --version "$version" "$@"
}
