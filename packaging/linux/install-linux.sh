#!/bin/sh
set -eu

REPOSITORY="hexuria/buwiz-forms"
RELEASE_API="https://api.github.com/repos/$REPOSITORY/releases/latest"
RELEASES="https://github.com/$REPOSITORY/releases/download"
APP_VERSION=""
USE_TARBALL=0
UNINSTALL=0
FROM_DIR=""
TEMP_DIR=""

usage() {
    cat <<'USAGE'
Usage: install-linux.sh [--version <tag>] [--tarball] [--uninstall]
       install-linux.sh --from-dir <directory> [--version <version>]
USAGE
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            [ "$#" -ge 2 ] || { echo "--version needs a value" >&2; exit 2; }
            APP_VERSION=$2
            shift 2
            ;;
        --tarball)
            USE_TARBALL=1
            shift
            ;;
        --uninstall)
            UNINSTALL=1
            shift
            ;;
        --from-dir)
            [ "$#" -ge 2 ] || { echo "--from-dir needs a directory" >&2; exit 2; }
            FROM_DIR=$2
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage
            exit 2
            ;;
    esac
done

cleanup() {
    if [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
}
trap cleanup 0 HUP INT TERM

if [ "$UNINSTALL" -eq 1 ]; then
    . /etc/os-release 2>/dev/null || true
    ID=${ID:-unknown}
    ID_LIKE=${ID_LIKE:-}
    case "$ID $ID_LIKE" in
        *debian*|*ubuntu*)
            if [ "$(id -u)" -eq 0 ]; then apt-get remove -y ebirforms; else sudo apt-get remove -y ebirforms; fi
            ;;
        *fedora*|*rhel*|*centos*|*suse*)
            if command -v dnf >/dev/null 2>&1; then
                if [ "$(id -u)" -eq 0 ]; then dnf remove -y ebirforms; else sudo dnf remove -y ebirforms; fi
            elif command -v zypper >/dev/null 2>&1; then
                if [ "$(id -u)" -eq 0 ]; then zypper --non-interactive remove ebirforms; else sudo zypper --non-interactive remove ebirforms; fi
            fi
            ;;
        *arch*|*manjaro*|*endeavouros*)
            if [ "$(id -u)" -eq 0 ]; then pacman -Rns --noconfirm ebirforms; else sudo pacman -Rns --noconfirm ebirforms; fi
            ;;
    esac
    rm -f "$HOME/.local/bin/ebirforms"
    if [ -x "$HOME/.local/share/ebirforms/install-desktop-entry.sh" ]; then
        "$HOME/.local/share/ebirforms/install-desktop-entry.sh" --remove
    fi
    rm -rf "$HOME/.local/share/ebirforms"
    echo "Removed eBIRForms."
    exit 0
fi

if [ -n "$FROM_DIR" ]; then
    [ -d "$FROM_DIR" ] || { echo "Package directory does not exist: $FROM_DIR" >&2; exit 1; }
    FROM_DIR=$(CDPATH='' cd -- "$FROM_DIR" && pwd)
fi

if [ -z "$APP_VERSION" ]; then
    command -v curl >/dev/null 2>&1 || {
        echo "curl is required to resolve the latest GitHub release" >&2
        exit 1
    }
    APP_VERSION=$(curl -fsSL "$RELEASE_API" | sed -n 's/^[[:space:]]*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
    [ -n "$APP_VERSION" ] || { echo "Could not resolve the latest eBIRForms release tag" >&2; exit 1; }
fi
APP_VERSION=${APP_VERSION#v}
case "$APP_VERSION" in
    ""|*[!A-Za-z0-9.+~-]*)
        echo "Invalid release version: $APP_VERSION" >&2
        exit 2
        ;;
esac
TAG="v$APP_VERSION"

MACHINE=$(uname -m)
case "$MACHINE" in
    x86_64|amd64) ARCH=x64 ;;
    aarch64|arm64) ARCH=arm64 ;;
    *) echo "Unsupported Linux architecture: $MACHINE" >&2; exit 1 ;;
esac

. /etc/os-release 2>/dev/null || true
ID=${ID:-unknown}
ID_LIKE=${ID_LIKE:-}
case "$ID $ID_LIKE" in
    *debian*|*ubuntu*) FAMILY=debian ;;
    *fedora*|*rhel*|*centos*|*suse*) FAMILY=rpm ;;
    *arch*|*manjaro*|*endeavouros*) FAMILY=arch ;;
    *) FAMILY=unknown ;;
esac

if [ "$FAMILY" = arch ] && [ "$ARCH" != x64 ] && [ "$USE_TARBALL" -eq 0 ]; then
    echo "The Arch package is available for x86_64 only; use --tarball on $ARCH." >&2
    exit 1
fi

FONT_COMMAND=""
case "$FAMILY" in
    debian) FONT_COMMAND="sudo apt-get install fonts-noto-core" ;;
    rpm)
        case "$ID" in
            *opensuse*|suse) FONT_COMMAND="sudo zypper install google-noto-sans-fonts" ;;
            *) FONT_COMMAND="sudo dnf install google-noto-sans-fonts" ;;
        esac
        ;;
    arch) FONT_COMMAND="sudo pacman -S noto-fonts" ;;
    *) FONT_COMMAND="install Noto Sans using your distribution's package manager" ;;
esac
warn_missing_font() {
    if ! command -v fc-list >/dev/null 2>&1 || ! fc-list : family 2>/dev/null | grep -qi 'Noto Sans'; then
        echo "Warning: Noto Sans was not found. Install it with: $FONT_COMMAND" >&2
    fi
}

run_privileged() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    elif command -v sudo >/dev/null 2>&1; then
        sudo "$@"
    else
        echo "Run as root or install sudo to install a system package." >&2
        exit 1
    fi
}

TEMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/ebirforms-install.XXXXXX")
SUMS="SHA256SUMS-linux-${ARCH}.txt"
if [ -n "$FROM_DIR" ]; then
    [ -f "$FROM_DIR/$SUMS" ] || { echo "Missing checksum file: $FROM_DIR/$SUMS" >&2; exit 1; }
    cp "$FROM_DIR/$SUMS" "$TEMP_DIR/$SUMS"
else
    command -v curl >/dev/null 2>&1 || { echo "curl is required to download the release" >&2; exit 1; }
    curl -fsSL "$RELEASES/$TAG/$SUMS" -o "$TEMP_DIR/$SUMS"
fi

download_and_verify() {
    asset=$1
    if [ -n "$FROM_DIR" ]; then
        [ -f "$FROM_DIR/$asset" ] || { echo "Missing package asset: $FROM_DIR/$asset" >&2; exit 1; }
        cp "$FROM_DIR/$asset" "$TEMP_DIR/$asset"
    else
        curl -fsSL "$RELEASES/$TAG/$asset" -o "$TEMP_DIR/$asset"
    fi
    expected=$(awk -v name="$asset" '$2 == name { print $1; exit }' "$TEMP_DIR/$SUMS")
    [ -n "$expected" ] || { echo "No checksum entry for $asset" >&2; exit 1; }
    printf '%s  %s\n' "$expected" "$asset" | (CDPATH='' cd -- "$TEMP_DIR" && sha256sum -c -)
}

install_tarball() {
    asset="eBIRForms-Linux-${ARCH}-${APP_VERSION}.tar.gz"
    download_and_verify "$asset"
    install_root="$HOME/.local/share/ebirforms"
    bin_dir="$HOME/.local/bin"
    mkdir -p "$install_root" "$bin_dir"
    tar -xzf "$TEMP_DIR/$asset" -C "$install_root" --strip-components=1
    ln -sf "$install_root/bir" "$bin_dir/ebirforms"
    "$install_root/install-desktop-entry.sh"
    echo "Installed eBIRForms to $install_root"
    echo "Add $bin_dir to PATH if it is not already present."
}

case "$FAMILY:$USE_TARBALL" in
    *:1|unknown:*)
        install_tarball
        ;;
    debian:0)
        asset="eBIRForms-Linux-${ARCH}-${APP_VERSION}.deb"
        download_and_verify "$asset"
        run_privileged apt-get update
        run_privileged apt-get install -y "$TEMP_DIR/$asset"
        ;;
    rpm:0)
        asset="eBIRForms-Linux-${ARCH}-${APP_VERSION}.rpm"
        download_and_verify "$asset"
        if command -v dnf >/dev/null 2>&1; then
            run_privileged dnf install -y "$TEMP_DIR/$asset"
        elif command -v zypper >/dev/null 2>&1; then
            run_privileged zypper --non-interactive --no-gpg-checks install --allow-unsigned-rpm "$TEMP_DIR/$asset"
        else
            echo "Neither dnf nor zypper is available to install the RPM." >&2
            exit 1
        fi
        ;;
    arch:0)
        asset="eBIRForms-Linux-${ARCH}-${APP_VERSION}.pkg.tar.zst"
        download_and_verify "$asset"
        run_privileged pacman -Syu --noconfirm
        run_privileged pacman -U --noconfirm "$TEMP_DIR/$asset"
        ;;
esac

warn_missing_font
