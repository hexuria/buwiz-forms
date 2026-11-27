# Shared pinned-tool installer for the Bash setup scripts. Sourced, not run:
# gpui-agent-cli and impeccable may be downloaded from their own hexuria GitHub
# release archives after a sha256 check against the digest pinned in this
# repository; everything else and every failure is a pinned cargo install from
# source.

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

prebuilt_binary_name() {  # prebuilt_binary_name <crate> → the binary cargo installs
    case "$1" in
        gpui-agent-cli) printf '%s\n' gpui-agent ;;
        *) printf '%s\n' "$1" ;;
    esac
}

# The fixed release table. Only these two repositories are ours; no other crate
# may take the download route.
prebuilt_release_repo() {  # prebuilt_release_repo <crate> → owner/repo
    case "$1" in
        gpui-agent-cli) printf '%s\n' hexuria/gpui-agent ;;
        impeccable) printf '%s\n' hexuria/impeccable-rust ;;
        *) return 1 ;;
    esac
}

prebuilt_machine() {
    case "$(uname -m)" in
        amd64|x86_64) printf '%s\n' x86_64 ;;
        arm64|aarch64) printf '%s\n' aarch64 ;;
        *) return 1 ;;
    esac
}

prebuilt_release_target() {  # prebuilt_release_target <crate> → target triple in its table
    local crate="$1" system machine
    system="$(uname -s)"
    machine="$(prebuilt_machine)" || return 1
    case "$crate" in
        gpui-agent-cli)
            case "$system:$machine" in
                Darwin:aarch64) printf '%s\n' aarch64-apple-darwin ;;
                Darwin:x86_64) printf '%s\n' x86_64-apple-darwin ;;
                Linux:aarch64) printf '%s\n' aarch64-unknown-linux-gnu ;;
                Linux:x86_64) printf '%s\n' x86_64-unknown-linux-gnu ;;
                *) return 1 ;;
            esac
            ;;
        impeccable)
            case "$system:$machine" in
                Darwin:aarch64) printf '%s\n' aarch64-apple-darwin ;;
                Darwin:x86_64) printf '%s\n' x86_64-apple-darwin ;;
                Linux:aarch64) printf '%s\n' aarch64-unknown-linux-musl ;;
                Linux:x86_64) printf '%s\n' x86_64-unknown-linux-musl ;;
                *) return 1 ;;
            esac
            ;;
        *)
            return 1 ;;
    esac
}

prebuilt_release_archive() {  # prebuilt_release_archive <crate> <target> <version>
    local crate="$1" target="$2" version="$3"
    case "$crate:$target" in
        gpui-agent-cli:aarch64-apple-darwin|gpui-agent-cli:x86_64-apple-darwin|\
        gpui-agent-cli:aarch64-unknown-linux-gnu|gpui-agent-cli:x86_64-unknown-linux-gnu|\
        gpui-agent-cli:x86_64-pc-windows-msvc)
            printf '%s\n' "gpui-agent-cli-$target-v$version.tgz" ;;
        impeccable:aarch64-apple-darwin|impeccable:x86_64-apple-darwin|\
        impeccable:aarch64-unknown-linux-musl|impeccable:x86_64-unknown-linux-musl)
            printf '%s\n' "impeccable-$target-v$version.tar.gz" ;;
        impeccable:x86_64-pc-windows-msvc|impeccable:aarch64-pc-windows-msvc)
            printf '%s\n' "impeccable-$target-v$version.zip" ;;
        *)
            return 1 ;;
    esac
}

prebuilt_sha256_file() {  # prebuilt_sha256_file <file> → lowercase hex digest
    local line digest
    if command -v sha256sum >/dev/null 2>&1; then
        line="$(sha256sum "$1" 2>/dev/null)" || return 1
    elif command -v shasum >/dev/null 2>&1; then
        line="$(shasum -a 256 "$1" 2>/dev/null)" || return 1
    else
        return 1
    fi
    digest="${line%%[[:space:]]*}"
    printf '%s\n' "$digest" | tr 'A-F' 'a-f'
}

prebuilt_download() {  # prebuilt_download <url> <output>: one retry at most, 60 s each
    curl -fsSL --max-time 60 --retry 1 -o "$2" "$1"
}

# Locate a binary in an archive listing. Accept it at the top level or exactly
# one folder down, and only when exactly one such entry exists.
prebuilt_archive_binary_path() {  # prebuilt_archive_binary_path <archive> <binary>
    local archive="$1" binary="$2" entries entry stripped dir candidate="" count=0
    case "$archive" in
        *.zip)
            command -v unzip >/dev/null 2>&1 || return 1
            entries="$(unzip -Z1 "$archive" 2>/dev/null)" || return 1
            ;;
        *)
            entries="$(tar -tzf "$archive" 2>/dev/null)" || return 1
            ;;
    esac
    while IFS= read -r entry; do
        [ -n "$entry" ] || continue
        stripped="${entry#./}"
        case "$stripped" in
            */) continue ;;
        esac
        if [ "$stripped" = "$binary" ]; then
            candidate="$stripped"
            count=$((count + 1))
        elif [ "${stripped##*/}" = "$binary" ]; then
            dir="${stripped%/*}"
            case "$dir" in
                ""|*/*) ;;
                *)
                    candidate="$stripped"
                    count=$((count + 1))
                    ;;
            esac
        fi
    done <<EOF
$entries
EOF
    [ "$count" -eq 1 ] || return 1
    printf '%s\n' "$candidate"
}

# prebuilt_write_stamp <crate> <version> <route> <binary>: three lines beside the
# binary — version, route, sha256 of the binary bytes installed just now.
prebuilt_write_stamp() {
    local crate="$1" version="$2" route="$3" binary="$4"
    local bin path stamp digest tmp
    bin="$(prebuilt_cargo_bin)"
    path="$bin/$binary"
    stamp="$bin/.$crate.buwiz-installed"
    [ -f "$path" ] || return 1
    digest="$(prebuilt_sha256_file "$path")" || return 1
    tmp="$stamp.tmp.$$"
    if printf '%s\n%s\n%s\n' "$version" "$route" "$digest" > "$tmp" 2>/dev/null && mv -f "$tmp" "$stamp" 2>/dev/null; then
        return 0
    fi
    rm -f "$tmp" 2>/dev/null || true
    return 1
}

# installed_pinned_version <crate> <binary>: the stamp answers only while the
# binary bytes are still the bytes it describes. Otherwise a plain crates.io line
# in cargo's install list answers; a bracketed local/git source and missing
# evidence answer nothing.
installed_pinned_version() {
    local crate="$1" binary="${2:-$1}"
    local bin path stamp version stamped_digest actual_digest installed
    bin="$(prebuilt_cargo_bin)"
    path="$bin/$binary"
    stamp="$bin/.$crate.buwiz-installed"
    [ -f "$path" ] || return 0
    if [ -s "$stamp" ]; then
        version="$(sed -n '1p' "$stamp" 2>/dev/null)"
        stamped_digest="$(sed -n '3p' "$stamp" 2>/dev/null)"
        actual_digest="$(prebuilt_sha256_file "$path" 2>/dev/null || true)"
        if [ -n "$version" ] && [ -n "$stamped_digest" ] && [ "$stamped_digest" = "$actual_digest" ]; then
            printf '%s\n' "$version"
            return 0
        fi
    fi
    installed="$( (cargo install --list 2>/dev/null || true) | sed -n \
        "s/^$crate v\([0-9][0-9.]*\):$/\1/p" | head -1)"
    if [ -n "$installed" ]; then
        printf '%s\n' "$installed"
    fi
}

# prebuilt_report_pinned <label> <crate> <binary> <pin>: check-mode row, true
# only when installed_pinned_version proves the pinned version.
prebuilt_report_pinned() {
    local label="$1" crate="$2" binary="$3" pin="$4" installed
    installed="$(installed_pinned_version "$crate" "$binary")"
    [ -n "$installed" ] || installed="unknown"
    printf '%s  %s  (pin %s)\n' "$label" "$installed" "$pin"
    [ "$installed" = "$pin" ]
}

prebuilt_cargo_source_install() {  # prebuilt_cargo_source_install <crate> <version> <binary> [extra args]
    local crate="$1" version="$2" binary="$3" rc
    shift 3
    cargo install "$crate" --version "=$version" "$@"
    rc=$?
    [ "$rc" -eq 0 ] || return "$rc"
    prebuilt_write_stamp "$crate" "$version" source "$binary"
}

# Try the direct release download. On return 1, PREBUILT_FAILED_STEP names the
# failed step; every temporary file or directory has already been removed.
prebuilt_direct_install() {  # prebuilt_direct_install <crate> <version> <binary>
    local crate="$1" version="$2" binary="$3"
    local repo target archive url tmp downloaded extract
    local tool_key target_key version_pin digest_generation expected actual binary_path source bin
    PREBUILT_FAILED_STEP=""

    repo="$(prebuilt_release_repo "$crate")" || {
        PREBUILT_FAILED_STEP="release table lookup"
        return 1
    }
    target="$(prebuilt_release_target "$crate")" || {
        PREBUILT_FAILED_STEP="target lookup"
        return 1
    }
    archive="$(prebuilt_release_archive "$crate" "$target" "$version")" || {
        PREBUILT_FAILED_STEP="archive lookup"
        return 1
    }
    tool_key="$(printf '%s\n' "$crate" | tr 'a-z-' 'A-Z_')"
    version_pin="$(prebuilt_pin "${tool_key}_VERSION")"
    digest_generation="$(prebuilt_pin "${tool_key}_SHA256_FOR")"
    if [ "$digest_generation" != "$version_pin" ] || [ "$version" != "$version_pin" ]; then
        PREBUILT_FAILED_STEP="pin version"
        return 1
    fi
    target_key="${target//-/_}"
    expected="$(prebuilt_pin "${tool_key}_SHA256_${target_key}")"
    if [[ ! "$expected" =~ ^[0-9a-f]{64}$ ]]; then
        PREBUILT_FAILED_STEP="digest pin"
        return 1
    fi
    url="https://github.com/$repo/releases/download/v$version/$archive"
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/buwiz-prebuilt.XXXXXX")" || {
        PREBUILT_FAILED_STEP="temporary directory"
        return 1
    }
    downloaded="$tmp/$archive"
    extract="$tmp/extract"

    if ! prebuilt_download "$url" "$downloaded"; then
        rm -rf "$tmp"
        PREBUILT_FAILED_STEP="archive download"
        return 1
    fi
    actual="$(prebuilt_sha256_file "$downloaded" 2>/dev/null || true)"
    if [ "$actual" != "$expected" ]; then
        rm -rf "$tmp"
        PREBUILT_FAILED_STEP="digest check"
        return 1
    fi
    binary_path="$(prebuilt_archive_binary_path "$downloaded" "$binary")" || {
        rm -rf "$tmp"
        PREBUILT_FAILED_STEP="archive layout"
        return 1
    }
    if [ "$crate" = "gpui-agent-cli" ] && [ "$binary_path" != "gpui-agent-cli/$binary" ]; then
        rm -rf "$tmp"
        PREBUILT_FAILED_STEP="archive layout"
        return 1
    fi
    mkdir -p "$extract" || {
        rm -rf "$tmp"
        PREBUILT_FAILED_STEP="archive unpack"
        return 1
    }
    case "$archive" in
        *.zip)
            if ! unzip -q -o "$downloaded" -d "$extract"; then
                rm -rf "$tmp"
                PREBUILT_FAILED_STEP="archive unpack"
                return 1
            fi
            ;;
        *)
            if ! tar -xzf "$downloaded" -C "$extract"; then
                rm -rf "$tmp"
                PREBUILT_FAILED_STEP="archive unpack"
                return 1
            fi
            ;;
    esac
    source="$extract/$binary_path"
    if [ ! -f "$source" ]; then
        rm -rf "$tmp"
        PREBUILT_FAILED_STEP="archive layout"
        return 1
    fi
    bin="$(prebuilt_cargo_bin)"
    # The archive's own mode is not trusted to carry the executable bit.
    if ! mkdir -p "$bin" || ! mv -f "$source" "$bin/$binary" || ! chmod +x "$bin/$binary"; then
        rm -rf "$tmp"
        PREBUILT_FAILED_STEP="binary install"
        return 1
    fi
    hash -r 2>/dev/null || true
    if ! prebuilt_write_stamp "$crate" "$version" prebuilt "$binary"; then
        rm -rf "$tmp"
        PREBUILT_FAILED_STEP="stamp"
        return 1
    fi
    rm -rf "$tmp"
    return 0
}

# install_pinned_tool <crate> <version> [extra cargo-install args]
# Returns the install status explicitly. The download route exists only for the
# fixed table above; any failure is announced once and retried as a pinned cargo
# install from source with the caller's extra arguments.
install_pinned_tool() {
    local crate="$1" version="$2" binary installed step
    shift 2
    prebuilt_use_cargo_bin
    binary="$(prebuilt_binary_name "$crate")"
    installed="$(installed_pinned_version "$crate" "$binary")"
    if [ "$installed" = "$version" ]; then
        printf '%s already at %s\n' "$crate" "$version"
        return 0
    fi
    if [ "${BUWIZ_SETUP_NO_PREBUILT:-0}" = "1" ]; then
        printf 'prebuilt installs disabled for %s; building from source\n' "$crate"
        prebuilt_cargo_source_install "$crate" "$version" "$binary" "$@"
        return
    fi
    if prebuilt_direct_install "$crate" "$version" "$binary"; then
        return 0
    fi
    step="$PREBUILT_FAILED_STEP"
    printf '%s prebuilt %s failed; building from source\n' "$crate" "$step"
    prebuilt_cargo_source_install "$crate" "$version" "$binary" "$@"
}

# cargo-binstall is still needed by impeccable's host setup in the Linux Devin
# blueprint. It is downloaded only for the two Linux musl targets and only when
# the archive matches the digest pinned in scripts/impeccable.pin.
prebuilt_binstall_asset() {
    case "$(uname -s):$(uname -m)" in
        Linux:x86_64|Linux:amd64)
            printf '%s\n' "cargo-binstall-x86_64-unknown-linux-musl.tgz" ;;
        Linux:aarch64|Linux:arm64)
            printf '%s\n' "cargo-binstall-aarch64-unknown-linux-musl.tgz" ;;
        *)
            return 1 ;;
    esac
}

prebuilt_binstall_at_pin() {  # prebuilt_binstall_at_pin <version>
    local output
    output="$(cargo-binstall -V 2>/dev/null)" || return 1
    [ "$output" = "$1" ] || [ "$output" = "cargo-binstall $1" ]
}

# ensure_cargo_binstall: true when the pinned cargo-binstall is usable. A
# download or digest failure is deliberately non-fatal; the blueprint warns and
# impeccable's host setup falls back as it did before.
ensure_cargo_binstall() {
    prebuilt_use_cargo_bin
    [ "${BUWIZ_SETUP_NO_PREBUILT:-0}" != "1" ] || return 1

    local pin asset target key expected actual url bin tmp extract source
    pin="$(prebuilt_pin CARGO_BINSTALL_VERSION)"
    [ -n "$pin" ] || return 1
    if prebuilt_binstall_at_pin "$pin"; then
        return 0
    fi
    asset="$(prebuilt_binstall_asset)" || return 1
    target="${asset#cargo-binstall-}"
    target="${target%.tgz}"
    key="CARGO_BINSTALL_SHA256_${target//-/_}"
    expected="$(prebuilt_pin "$key")"
    [ -n "$expected" ] || return 1
    bin="$(prebuilt_cargo_bin)"
    mkdir -p "$bin" || return 1
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/cargo-binstall.XXXXXX")" || return 1
    url="https://github.com/cargo-bins/cargo-binstall/releases/download/v$pin/$asset"
    if ! prebuilt_download "$url" "$tmp/$asset"; then
        rm -rf "$tmp"
        return 1
    fi
    actual="$(prebuilt_sha256_file "$tmp/$asset" 2>/dev/null || true)"
    if [ "$actual" != "$expected" ]; then
        rm -rf "$tmp"
        return 1
    fi
    extract="$tmp/extract"
    mkdir -p "$extract"
    if ! tar -xzf "$tmp/$asset" -C "$extract"; then
        rm -rf "$tmp"
        return 1
    fi
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
