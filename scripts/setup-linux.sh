#!/usr/bin/env bash
#
# setup-linux.sh — install everything needed to build and run E-BIRForms on Linux.
#
# Supported package managers / distro families:
#   apt     Debian, Ubuntu, Linux Mint, Pop!_OS, ...
#   dnf     Fedora, RHEL, CentOS Stream, Rocky, Alma, ...
#   pacman  Arch, Manjaro, EndeavourOS, ...
#   zypper  openSUSE Tumbleweed / Leap
#
# What it installs:
#   - Build tools (cc/make, pkg-config, perl — openssl-src compiles from source)
#   - GPUI windowing/graphics/audio dev libraries (X11, XCB, Wayland, Vulkan,
#     xkbcommon, ALSA, udev, fontconfig, freetype, polkit)
#   - GTK 3 + WebKitGTK 4.1 (bundled offline HTML preview / print / PDF export)
#   - Noto fonts — the app renders every glyph through 'Noto Sans'; without it
#     the window is a blank white frame (cosmic_text finds no family)
#   - OpenSSL headers, mesa Vulkan drivers, libxdo, appindicator, librsvg,
#     patchelf (packaging), Xvfb (headless GTK tests), python3
#   - Rust toolchain via rustup (if cargo is missing)
#   - `just` command runner (distro package when available, else cargo install)
#
# Usage:
#   scripts/setup-linux.sh              # install everything
#   scripts/setup-linux.sh --check      # verify only, install nothing
#   scripts/setup-linux.sh --extras     # also install cargo-audit/outdated/machete + node
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

log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    command -v sudo >/dev/null 2>&1 || die "not root and sudo is not installed"
    SUDO="sudo"
fi

# ---------------------------------------------------------------------------
# Detect the package manager
# ---------------------------------------------------------------------------
PM=""
for candidate in apt-get dnf pacman zypper; do
    if command -v "$candidate" >/dev/null 2>&1; then
        PM="$candidate"
        break
    fi
done
[ -n "$PM" ] || die "no supported package manager found (apt/dnf/pacman/zypper)"

DISTRO_ID="$(. /etc/os-release 2>/dev/null && echo "${ID:-unknown}")"
log "Detected package manager: $PM (distro: $DISTRO_ID)"

# ---------------------------------------------------------------------------
# Package name helpers — package names differ per distro, and some libraries
# (WebKitGTK above all) moved API versions across releases.
# ---------------------------------------------------------------------------
pkg_exists() {  # pkg_exists <package>
    case "$PM" in
        apt-get) apt-cache show "$1" >/dev/null 2>&1 ;;
        dnf)     dnf info "$1" >/dev/null 2>&1 ;;
        pacman)  pacman -Si "$1" >/dev/null 2>&1 ;;
        zypper)  zypper info "$1" >/dev/null 2>&1 ;;
    esac
}

pick_pkg() {  # pick_pkg <candidate...> → first candidate that exists
    local p
    for p in "$@"; do
        if pkg_exists "$p"; then
            echo "$p"
            return 0
        fi
    done
    warn "none of these packages exist: $* — skipping"
    return 0
}

# ---------------------------------------------------------------------------
# Dependency lists per package manager
# ---------------------------------------------------------------------------
pkgs_apt() {
    echo "
        build-essential curl git pkg-config perl python3
        libx11-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev
        libxcb-xfixes0-dev libxkbcommon-dev libxkbcommon-x11-dev
        libwayland-dev libwayland-client0 libasound2-dev libudev-dev
        libvulkan-dev libfontconfig1-dev libfreetype-dev libssl-dev
        libpolkit-gobject-1-dev libgtk-3-dev libxdo-dev
        mesa-vulkan-drivers libappindicator3-dev librsvg2-dev
        patchelf xvfb fonts-noto-core fonts-noto-color-emoji
        $(pick_pkg libwebkit2gtk-4.1-dev libwebkit2gtk-4.0-dev)
    "
}

pkgs_dnf() {
    echo "
        gcc gcc-c++ make curl git pkgconf-pkg-config perl python3
        libX11-devel libxcb-devel libxkbcommon-devel libxkbcommon-x11-devel
        wayland-devel alsa-lib-devel systemd-devel vulkan-devel
        fontconfig-devel freetype-devel openssl-devel polkit-devel
        gtk3-devel libxdo-devel mesa-vulkan-drivers
        libappindicator-gtk3-devel librsvg2-devel patchelf
        xorg-x11-server-Xvfb google-noto-sans-fonts
        google-noto-emoji-color-fonts
        $(pick_pkg webkit2gtk4.1-devel webkit2gtk4.0-devel)
    "
}

pkgs_pacman() {
    echo "
        base-devel curl git pkgconf python3
        libx11 libxcb libxkbcommon libxkbcommon-x11 wayland
        alsa-lib systemd-libs vulkan-headers vulkan-icd-loader
        fontconfig freetype2 openssl polkit gtk3 xdotool
        mesa vulkan-swrast libappindicator-gtk3 librsvg patchelf
        xorg-server-xvfb noto-fonts noto-fonts-emoji
        $(pick_pkg webkit2gtk-4.1 webkit2gtk)
    "
}

pkgs_zypper() {
    echo "
        gcc gcc-c++ make curl git pkg-config perl python3
        libX11-devel libxcb-devel libxkbcommon-devel libxkbcommon-x11-devel
        wayland-devel alsa-devel libudev-devel vulkan-devel libvulkan1
        fontconfig-devel freetype2-devel libopenssl-devel polkit-devel
        gtk3-devel libxdo-devel Mesa
        libappindicator3-devel librsvg-devel patchelf xorg-x11-Xvfb
        $(pick_pkg noto-sans-fonts) $(pick_pkg google-noto-coloremoji-fonts)
        $(pick_pkg webkit2gtk4-devel webkit2gtk3-devel)
    "
}

# Refresh package metadata before resolving candidate names — apt-cache knows
# nothing until `apt-get update` has run at least once.
refresh_metadata() {
    case "$PM" in
        apt-get) $SUDO apt-get update ;;
        dnf)     $SUDO dnf check-update >/dev/null 2>&1 || true ;;
        pacman)  $SUDO pacman -Sy --noconfirm ;;
        zypper)  $SUDO zypper --non-interactive refresh ;;
    esac
}

install_pkgs() {
    local list="$1"
    # shellcheck disable=SC2086
    case "$PM" in
        apt-get)
            $SUDO apt-get install -y $list
            ;;
        dnf)
            $SUDO dnf install -y $list
            ;;
        pacman)
            $SUDO pacman -Sy --needed --noconfirm $list
            ;;
        zypper)
            $SUDO zypper --non-interactive install --no-recommends $list
            ;;
    esac
}

# ---------------------------------------------------------------------------
# just — prefer the distro package, fall back to cargo install
# ---------------------------------------------------------------------------
install_just() {
    if command -v just >/dev/null 2>&1; then
        log "just already installed: $(just --version)"
        return 0
    fi
    local candidate=""
    case "$PM" in
        apt-get) candidate="$(pick_pkg just)" ;;
        dnf)     candidate="$(pick_pkg just)" ;;
        pacman)  candidate="$(pick_pkg just)" ;;
        zypper)  candidate="$(pick_pkg just)" ;;
    esac
    if [ -n "$candidate" ]; then
        install_pkgs "$candidate"
    fi
    if ! command -v just >/dev/null 2>&1; then
        log "Installing just via cargo"
        cargo install just
    fi
}

# ---------------------------------------------------------------------------
# rustup — needed before cargo install works
# ---------------------------------------------------------------------------
install_rust() {
    if command -v cargo >/dev/null 2>&1; then
        log "Rust already installed: $(cargo --version)"
        return 0
    fi
    log "Installing Rust via rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
    # rust-toolchain.toml pins components (clippy, rustfmt); a no-op sync
    # inside the repo pulls them if the default profile skipped them.
}

# ---------------------------------------------------------------------------
# --check mode: verify the toolchain end to end
# ---------------------------------------------------------------------------
check() {
    local ok=1
    for tool in cargo rustc just python3 pkg-config perl cc; do
        if command -v "$tool" >/dev/null 2>&1; then
            printf '  %-10s %s\n' "$tool" "$("$tool" --version 2>/dev/null | head -1)"
        else
            printf '  %-10s MISSING\n' "$tool"
            ok=0
        fi
    done
    for mod in "webkit2gtk-4.1" "gtk+-3.0" "xkbcommon" "wayland-client" "alsa" "vulkan"; do
        if pkg-config --exists "$mod" 2>/dev/null; then
            printf '  %-18s %s\n' "pkg:$mod" "$(pkg-config --modversion "$mod")"
        else
            printf '  %-18s MISSING\n' "pkg:$mod"
            ok=0
        fi
    done
    if fc-match "Noto Sans" 2>/dev/null | grep -qi noto; then
        printf '  %-18s %s\n' "font:Noto Sans" "$(fc-match 'Noto Sans' --format '%{family}')"
    else
        printf '  %-18s MISSING (app window renders blank without it)\n' "font:Noto Sans"
        ok=0
    fi
    [ "$ok" -eq 1 ] && log "All required tooling present" || warn "Missing pieces — run scripts/setup-linux.sh"
    return $((1 - ok))
}

# ---------------------------------------------------------------------------
main() {
    if [ "$CHECK_ONLY" -eq 1 ]; then
        check
        exit $?
    fi

    refresh_metadata

    local pkgs
    case "$PM" in
        apt-get) pkgs="$(pkgs_apt)" ;;
        dnf)     pkgs="$(pkgs_dnf)" ;;
        pacman)  pkgs="$(pkgs_pacman)" ;;
        zypper)  pkgs="$(pkgs_zypper)" ;;
    esac
    log "Installing system packages"
    install_pkgs "$pkgs"

    install_rust
    install_just

    if [ "$WITH_EXTRAS" -eq 1 ]; then
        log "Installing optional developer tools"
        cargo install cargo-audit cargo-outdated cargo-machete
        case "$PM" in
            apt-get) install_pkgs "nodejs npm" ;;
            dnf)     install_pkgs "nodejs npm" ;;
            pacman)  install_pkgs "nodejs npm" ;;
            zypper)  install_pkgs "nodejs npm" ;;
        esac
    fi

    # Freeze-inventory / stamp CI scripts expect Python 3.13; warn if older.
    local pyver
    pyver="$(python3 -c 'import sys; print("%d.%d" % sys.version_info[:2])')"
    if [ "$(python3 -c 'import sys; print(sys.version_info < (3, 13))')" = "True" ]; then
        warn "python3 is $pyver; freeze/stamp CI scripts expect 3.13 (fine for just run/check/test)"
    fi

    log "Verifying toolchain"
    check

    cat <<'EOF'

Done. Next steps:

    just run      # build + launch the desktop app (dev-tools feature)
    just check    # fmt + cargo check + clippy
    just test     # workspace tests (GTK print tests need a display or xvfb)

Headless machine? Run tests under the virtual framebuffer:

    xvfb-run -a just test

Blank white window on a VM / software GPU? llvmpipe's Vulkan path can
present stale frames — force the Gl backend by hiding the Vulkan ICD:

    VK_ICD_FILENAMES=/dev/null just run

EOF
}

main
