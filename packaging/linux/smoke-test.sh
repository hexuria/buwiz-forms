#!/usr/bin/env bash
set -euo pipefail

OUT=""
ARCH=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --out)
            OUT="${2:?missing value for --out}"
            shift 2
            ;;
        --arch)
            ARCH="${2:?missing value for --arch}"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 --out <directory> --arch <x64|arm64>"
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

if [ -z "$OUT" ] || [ ! -d "$OUT" ]; then
    echo "--out must name an existing package directory" >&2
    exit 2
fi
case "$ARCH" in
    x64) DOCKER_ARCH=amd64 ;;
    arm64) DOCKER_ARCH=arm64 ;;
    *) echo "--arch must be x64 or arm64" >&2; exit 2 ;;
esac
OUT="$(cd "$OUT" && pwd)"
prefix="eBIRForms-Linux-${ARCH}-"
shopt -s nullglob
tarballs=("$OUT"/"${prefix}"*.tar.gz)
debs=("$OUT"/"${prefix}"*.deb)
rpms=("$OUT"/"${prefix}"*.rpm)
if [ "${#tarballs[@]}" -ne 1 ] || [ "${#debs[@]}" -ne 1 ] || [ "${#rpms[@]}" -ne 1 ]; then
    echo "Expected exactly one tarball, deb, and rpm for $ARCH in $OUT" >&2
    exit 1
fi
version="$(basename "${tarballs[0]}" .tar.gz)"
version="${version#"$prefix"}"
platform="linux/$DOCKER_ARCH"

# Every container run gets a hard wall-clock cap, so one stalled package
# download fails this script instead of hanging the release job.
# SMOKE_CONTAINER_TIMEOUT overrides it (seconds). The cap wraps the docker
# client on the runner rather than running inside the image: as the
# container's PID 1, coreutils timeout exits 125 on fedora:latest.
CONTAINER_TIMEOUT="${SMOKE_CONTAINER_TIMEOUT:-600}"
in_time=(timeout --kill-after=30 "$CONTAINER_TIMEOUT")

# apt in the Debian/Ubuntu images: give up on a stalled mirror connection
# after 30 s and retry, rather than waiting minutes on one socket (the x64
# v0.1.0 run sat 390 s and 428 s on single archive.ubuntu.com downloads).
apt_ci_config="printf '%s\\n' 'Acquire::Retries \"5\";' 'Acquire::http::Timeout \"30\";' 'Acquire::https::Timeout \"30\";' > /etc/apt/apt.conf.d/80ci-network"

check_system_install=$(cat <<'SYSTEM_CHECK'
set -eu
test -x /usr/bin/ebirforms
test -x /usr/lib/ebirforms/eBIRForms
test -d /usr/lib/ebirforms/nsh
test -d /usr/lib/ebirforms/print-overrides
test -f /usr/share/applications/dev.goldcoders.bir.desktop
for size in 48 64 128 256 512; do
    test -f "/usr/share/icons/hicolor/${size}x${size}/apps/dev.goldcoders.bir.png"
done
ldd /usr/bin/ebirforms > /tmp/ebirforms-ldd.txt
if grep -q "not found" /tmp/ebirforms-ldd.txt; then
    cat /tmp/ebirforms-ldd.txt >&2
    exit 1
fi
echo "PASS: installed binary, dependencies, payload, desktop entry, and icons"
SYSTEM_CHECK
)

install_deb_image() {
    local image="$1"
    local file="$2"
    "${in_time[@]}" docker run --rm --platform "$platform" \
        -v "$OUT:/packages:ro" "$image" bash -euc "
            $apt_ci_config
            apt-get update
            apt-get install -y /packages/$file
            $check_system_install
        "
    printf 'PASS: %s installed %s\n' "$image" "$file"
}

# The deb requires libgtk-4-1 >= 4.14, so only distros shipping it can be
# smoke targets: debian:12 (4.8) and ubuntu:22.04 (4.6) cannot install it.
install_deb_image debian:trixie "$(basename "${debs[0]}")"
install_deb_image ubuntu:24.04 "$(basename "${debs[0]}")"

"${in_time[@]}" docker run --rm --platform "$platform" \
    -v "$OUT:/packages:ro" fedora:latest bash -euc "
        dnf install -y /packages/$(basename "${rpms[0]}")
        $check_system_install
    "
printf 'PASS: fedora:latest installed %s\n' "$(basename "${rpms[0]}")"

# No openSUSE leg: the spec's Requires use Fedora package names
# (gstreamer1-plugins-base, webkitgtk6.0, dbus-libs, google-noto-sans-fonts)
# that openSUSE does not provide, so zypper can never resolve the rpm there.

if [ "$ARCH" = x64 ]; then
    arch_packages=("$OUT"/"${prefix}"*.pkg.tar.zst)
    if [ "${#arch_packages[@]}" -ne 1 ]; then
        echo "Expected one Arch package for x64 in $OUT" >&2
        exit 1
    fi
    "${in_time[@]}" docker run --rm --platform linux/amd64 \
        -v "$OUT:/packages:ro" archlinux:latest bash -euc "
            pacman -Syu --noconfirm
            pacman -U --noconfirm /packages/$(basename "${arch_packages[0]}")
            $check_system_install
        "
    printf 'PASS: archlinux:latest installed %s\n' "$(basename "${arch_packages[0]}")"
fi

tarball_name="$(basename "${tarballs[0]}")"
"${in_time[@]}" docker run --rm --platform "$platform" \
    -v "$OUT:/packages:ro" debian:trixie bash -euc "
        $apt_ci_config
        apt-get update
        apt-get install -y \
            libgtk-4-1 libwebkitgtk-6.0-4 \
            gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
            libdbus-1-3 fonts-noto-core
        mkdir -p /opt/ebirforms
        tar -xzf /packages/$tarball_name -C /opt/ebirforms --strip-components=1
        test -x /opt/ebirforms/eBIRForms
        test -d /opt/ebirforms/nsh
        test -d /opt/ebirforms/print-overrides
        test -f /opt/ebirforms/dev.goldcoders.bir.desktop
        test -f /opt/ebirforms/dev.goldcoders.bir.png
        ldd /opt/ebirforms/eBIRForms > /tmp/ebirforms-ldd.txt
        if grep -q 'not found' /tmp/ebirforms-ldd.txt; then
            cat /tmp/ebirforms-ldd.txt >&2
            exit 1
        fi
        echo 'PASS: tarball binary, dependencies, payload, and desktop entry'
    "
printf 'PASS: debian:trixie extracted %s\n' "$tarball_name"

if [ ! -f "$OUT/SHA256SUMS-linux-${ARCH}.txt" ]; then
    echo "Missing per-architecture checksums in $OUT" >&2
    exit 1
fi
if [ "$ARCH" = x64 ]; then
    if [ ! -f "$OUT/install-linux.sh" ]; then
        echo "Missing install-linux.sh in $OUT" >&2
        exit 1
    fi
    for image in debian:trixie fedora:latest; do
        if [ "$image" = debian:trixie ]; then
            install_command="$apt_ci_config && apt-get update && apt-get install -y curl ca-certificates"
        else
            install_command='dnf install -y curl ca-certificates'
        fi
        "${in_time[@]}" docker run --rm --platform "$platform" \
            -v "$OUT:/packages:ro" -e INSTALL_VERSION="$version" "$image" bash -euc "
                $install_command
                sh /packages/install-linux.sh --from-dir /packages --version \"\$INSTALL_VERSION\"
                $check_system_install
            "
        printf 'PASS: install-linux.sh --from-dir in %s\n' "$image"
    done
fi
