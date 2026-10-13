#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
VERSION=""
TARGET=""
OUT=""

usage() {
    echo "Usage: $0 --version <version> --target <triple> --out <directory>" >&2
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            VERSION="${2:?missing value for --version}"
            shift 2
            ;;
        --target)
            TARGET="${2:?missing value for --target}"
            shift 2
            ;;
        --out)
            OUT="${2:?missing value for --out}"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage
            exit 2
            ;;
    esac
done

if [[ ! "$VERSION" =~ ^[0-9][A-Za-z0-9.+~-]*$ ]]; then
    echo "Invalid package version: $VERSION" >&2
    exit 2
fi
if [ -z "$OUT" ]; then
    usage
    exit 2
fi

case "$TARGET" in
    x86_64-unknown-linux-gnu)
        ARCH=x64
        DO_ARCH_PACKAGE=1
        ;;
    aarch64-unknown-linux-gnu)
        ARCH=arm64
        DO_ARCH_PACKAGE=0
        ;;
    *)
        echo "Unsupported Linux target: $TARGET" >&2
        exit 2
        ;;
esac

OUT="$(mkdir -p "$OUT" && cd "$OUT" && pwd)"
BIN="$ROOT/target/$TARGET/release/bir"
if [ ! -x "$BIN" ]; then
    echo "Release binary is missing or not executable: $BIN" >&2
    exit 1
fi
for required in \
    "$ROOT/assets/linux/dev.goldcoders.bir.desktop" \
    "$ROOT/assets/linux/install-desktop-entry.sh" \
    "$ROOT/assets/linux/icons/hicolor/48x48/apps/dev.goldcoders.bir.png" \
    "$ROOT/assets/linux/icons/hicolor/64x64/apps/dev.goldcoders.bir.png" \
    "$ROOT/assets/linux/icons/hicolor/128x128/apps/dev.goldcoders.bir.png" \
    "$ROOT/assets/linux/icons/hicolor/256x256/apps/dev.goldcoders.bir.png" \
    "$ROOT/assets/linux/icons/hicolor/512x512/apps/dev.goldcoders.bir.png"; do
    test -f "$required" || { echo "Required Linux packaging file is missing: $required" >&2; exit 1; }
done

rpm_version="${VERSION//-/\~}"
base_name="eBIRForms-Linux-${ARCH}-${VERSION}"
deb="$OUT/${base_name}.deb"
rpm="$OUT/${base_name}.rpm"
tarball="$OUT/${base_name}.tar.gz"

ensure_cargo_tool() {
    local command_name="$1"
    local package_name="$2"
    local wanted_version="$3"
    local actual_version=""
    if command -v "$command_name" >/dev/null 2>&1; then
        actual_version="$("$command_name" --version | awk '{print $NF}')"
    fi
    if [ "$actual_version" != "$wanted_version" ]; then
        cargo install --locked "$package_name" --version "$wanted_version" --force
    fi
}

ensure_cargo_tool cargo-deb cargo-deb 3.7.0
ensure_cargo_tool cargo-generate-rpm cargo-generate-rpm 0.21.0

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
payload="$stage/eBIRForms-Linux-${ARCH}-${VERSION}"
mkdir -p "$payload"
install -m 755 "$BIN" "$payload/bir"
cp -R "$ROOT/assets" "$payload/assets"
install -m 755 "$ROOT/assets/linux/install-desktop-entry.sh" "$payload/install-desktop-entry.sh"
check_launcher() {
    local applications="$1"
    local icons="$2"
    test -f "$applications/dev.goldcoders.bir.desktop" || {
        echo "Missing Linux desktop entry in $applications" >&2
        return 1
    }
    for size in 48 64 128 256 512; do
        test -f "$icons/${size}x${size}/apps/dev.goldcoders.bir.png" || {
            echo "Missing ${size}x${size} Linux icon in $icons" >&2
            return 1
        }
    done
}

python3 "$ROOT/scripts/audit_no_legacy.py" --package-root "$payload"
check_launcher "$payload/assets/linux" "$payload/assets/linux/icons/hicolor"
tarball_root="$stage/tarball-extracted"
mkdir -p "$tarball_root"
tar -czf "$tarball" -C "$stage" "$(basename "$payload")"
tar -xzf "$tarball" -C "$tarball_root"
python3 "$ROOT/scripts/audit_no_legacy.py" --package-root "$tarball_root"
check_launcher \
    "$tarball_root/$(basename "$payload")/assets/linux" \
    "$tarball_root/$(basename "$payload")/assets/linux/icons/hicolor"

cargo deb --locked -p bir-desktop --no-build --target "$TARGET" \
    --deb-version "$VERSION" -o "$deb"
deb_root="$stage/deb-extracted"
mkdir -p "$deb_root"
dpkg-deb --extract "$deb" "$deb_root"
python3 "$ROOT/scripts/audit_no_legacy.py" --package-root "$deb_root"
test -x "$deb_root/usr/bin/ebirforms"
test -d "$deb_root/usr/share/ebirforms/assets"
check_launcher "$deb_root/usr/share/applications" "$deb_root/usr/share/icons/hicolor"

rpm_metadata="$stage/rpm-metadata.toml"
printf 'version = "%s"\n' "$rpm_version" > "$rpm_metadata"
cargo generate-rpm -p crates/bir-desktop --target "$TARGET" \
    --metadata-overwrite="$rpm_metadata" -o "$rpm"
if ! command -v rpm2archive >/dev/null 2>&1; then
    echo "rpm2archive is required to audit the generated RPM payload" >&2
    exit 1
fi
rpm_root="$stage/rpm-extracted"
mkdir -p "$rpm_root"
rpm2archive --nocompression - < "$rpm" | tar -xf - -C "$rpm_root"
python3 "$ROOT/scripts/audit_no_legacy.py" --package-root "$rpm_root"
test -x "$rpm_root/usr/bin/ebirforms"
test -d "$rpm_root/usr/share/ebirforms/assets"
check_launcher "$rpm_root/usr/share/applications" "$rpm_root/usr/share/icons/hicolor"

produced=("$deb" "$rpm" "$tarball")
if [ "$DO_ARCH_PACKAGE" -eq 1 ]; then
    if ! command -v docker >/dev/null 2>&1; then
        echo "Docker is required to build the Arch Linux package" >&2
        exit 1
    fi
    arch_build="$stage/arch-build"
    mkdir -p "$arch_build"
    cp "$tarball" "$arch_build/"
    aur_pkgver="${VERSION//-/.}"
    aur_pkgver="${aur_pkgver//+/.}"
    archive_name="$(basename "$tarball")"
    archive_sha256="$(sha256sum "$tarball" | awk '{print $1}')"
    cat > "$arch_build/PKGBUILD" <<PKGBUILD
pkgname=ebirforms
pkgver=${aur_pkgver}
pkgrel=1
pkgdesc='Philippine BIR tax return filing application'
arch=('x86_64')
url='https://github.com/hexuria/buwiz-forms'
license=('MIT')
depends=('gtk3' 'webkit2gtk-4.1' 'vulkan-icd-loader' 'libxkbcommon' 'libxkbcommon-x11' 'wayland' 'fontconfig' 'freetype2' 'dbus' 'noto-fonts')
options=('!strip' '!debug')
source=('${archive_name}')
sha256sums=('${archive_sha256}')

package() {
  local app_root="\$srcdir/eBIRForms-Linux-x64-${VERSION}"
  install -Dm755 "\$app_root/bir" "\$pkgdir/usr/bin/ebirforms"
  install -d "\$pkgdir/usr/share/ebirforms/assets"
  cp -a "\$app_root/assets/." "\$pkgdir/usr/share/ebirforms/assets/"
  install -Dm644 "\$app_root/assets/linux/dev.goldcoders.bir.desktop" "\$pkgdir/usr/share/applications/dev.goldcoders.bir.desktop"
  for size in 48 64 128 256 512; do
    install -Dm644 "\$app_root/assets/linux/icons/hicolor/\${size}x\${size}/apps/dev.goldcoders.bir.png" "\$pkgdir/usr/share/icons/hicolor/\${size}x\${size}/apps/dev.goldcoders.bir.png"
  done
}
PKGBUILD
    aur_url="https://github.com/hexuria/buwiz-forms/releases/download/v${VERSION}/${archive_name}"
    sed \
        -e 's/^pkgname=ebirforms$/pkgname=ebirforms-bin/' \
        -e "s|^source=('${archive_name}')$|source=('${aur_url}')|" \
        "$arch_build/PKGBUILD" | awk '
            /^pkgname=ebirforms-bin$/ {
                print
                print "provides=(" sprintf("%c", 39) "ebirforms" sprintf("%c", 39) ")"
                print "conflicts=(" sprintf("%c", 39) "ebirforms" sprintf("%c", 39) ")"
                next
            }
            { print }
        ' > "$OUT/ebirforms-bin.PKGBUILD"
    docker run --rm --platform linux/amd64 \
        -v "$arch_build:/work" archlinux:base-devel \
        bash -euc '
            pacman -Syu --noconfirm --needed base-devel gtk3 webkit2gtk-4.1 \
                vulkan-icd-loader libxkbcommon libxkbcommon-x11 wayland \
                fontconfig freetype2 noto-fonts
            useradd --create-home --uid 1000 builder
            chown -R builder:builder /work
            su builder -c "cd /work && makepkg --syncdeps --noconfirm --clean --cleanbuild"
        '
    arch_package=("$arch_build"/ebirforms-*.pkg.tar.zst)
    if [ "${#arch_package[@]}" -ne 1 ]; then
        echo "Expected one Arch package in $arch_build" >&2
        exit 1
    fi
    cp "${arch_package[0]}" "$OUT/${base_name}.pkg.tar.zst"
    arch_root="$stage/arch-extracted"
    mkdir -p "$arch_root"
    tar --zstd -xf "$OUT/${base_name}.pkg.tar.zst" -C "$arch_root"
    python3 "$ROOT/scripts/audit_no_legacy.py" --package-root "$arch_root"
    test -x "$arch_root/usr/bin/ebirforms"
    test -d "$arch_root/usr/share/ebirforms/assets"
    check_launcher "$arch_root/usr/share/applications" "$arch_root/usr/share/icons/hicolor"
    produced+=("$OUT/${base_name}.pkg.tar.zst" "$OUT/ebirforms-bin.PKGBUILD")
fi

if [ "$ARCH" = x64 ]; then
    install -m 755 "$ROOT/packaging/linux/install-linux.sh" "$OUT/install-linux.sh"
    produced+=("$OUT/install-linux.sh")
fi
(
    cd "$OUT"
    names=()
    for file in "${produced[@]}"; do
        names+=("$(basename "$file")")
    done
    sha256sum "${names[@]}" > "SHA256SUMS-linux-${ARCH}.txt"
)

printf 'Linux %s packages written to %s:\n' "$ARCH" "$OUT"
for file in "${produced[@]}"; do
    printf '  %s (%s bytes)\n' "$(basename "$file")" "$(stat -c '%s' "$file")"
done
printf '  SHA256SUMS-linux-%s.txt\n' "$ARCH"
