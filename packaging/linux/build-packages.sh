#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION='' TARGET='' OUT=''
while [ "$#" -gt 0 ]; do
    case "$1" in
        --version) VERSION="${2:?}"; shift 2 ;;
        --target) TARGET="${2:?}"; shift 2 ;;
        --out) OUT="${2:?}"; shift 2 ;;
        *) echo "Usage: $0 --version VERSION --target TRIPLE --out DIRECTORY" >&2; exit 2 ;;
    esac
done
[[ "$VERSION" =~ ^[0-9][A-Za-z0-9.+~-]*$ ]] && [ -n "$OUT" ] || exit 2
case "$TARGET" in
    x86_64-unknown-linux-gnu) ARCH=x64; DEB_ARCH=amd64; RPM_ARCH=x86_64 ;;
    aarch64-unknown-linux-gnu) ARCH=arm64; DEB_ARCH=arm64; RPM_ARCH=aarch64 ;;
    *) echo "Unsupported target: $TARGET" >&2; exit 2 ;;
esac
[ "$(uname -m)" = "$RPM_ARCH" ] || { echo 'Build on the target architecture' >&2; exit 1; }
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cd "$ROOT/nsh"
nsh package --target linux
name="$(python3 -c 'import json; print(json.load(open("nsh.json"))["name"])')"
app_version="$(python3 -c 'import json; print(json.load(open("nsh.json"))["version"])')"
payload="$stage/$name"
cp -a ".nsh/out/$name" "$payload"
test -x "$payload/$name"
test ! -e "$payload/ebirforms-no-self-update"
# Print preview reads these beside the executable, not from NSH assets.
cp -a "$ROOT/print-overrides" "$payload/print-overrides"
python3 "$ROOT/scripts/audit_no_legacy.py" --package-root "$payload"
base="eBIRForms-Linux-${ARCH}-${VERSION}"
tar -czf "$OUT/$base.tar.gz" -C "$stage" "$name"
updater="$name-$app_version-linux-$RPM_ARCH.tar.gz"
test -f ".nsh/out/$updater"
# Retain NSH's updater name and root, including the same print resources.
cp "$OUT/$base.tar.gz" "$OUT/$updater"
system="$stage/system"
mkdir -p "$system/usr/lib/ebirforms" "$system/usr/bin" "$system/usr/share/applications"
cp -a "$payload/." "$system/usr/lib/ebirforms/"
touch "$system/usr/lib/ebirforms/ebirforms-no-self-update"
ln -s "/usr/lib/ebirforms/$name" "$system/usr/bin/ebirforms"
install -m644 "$ROOT/assets/linux/dev.goldcoders.bir.desktop" "$system/usr/share/applications/"
cp -a "$ROOT/assets/linux/icons" "$system/usr/share/"
mkdir -p "$system/DEBIAN"
cat > "$system/DEBIAN/control" <<EOF
Package: ebirforms
Version: $VERSION
Architecture: $DEB_ARCH
Maintainer: Goldcoders Corp <support@goldcoders.dev>
Section: office
Priority: optional
Depends: libgtk-4-1 (>= 4.14), libwebkitgtk-6.0-4 (>= 2.42), gstreamer1.0-plugins-base, gstreamer1.0-plugins-good, libdbus-1-3, fonts-noto-core
Description: Philippine BIR tax return filing application
EOF
dpkg-deb --root-owner-group --build "$system" "$OUT/$base.deb"
rm -rf "$system/DEBIAN"
mkdir -p "$stage/rpm/"{BUILD,RPMS,SOURCES,SPECS,SRPMS,BUILDROOT}
tar -czf "$stage/rpm/SOURCES/payload.tar.gz" -C "$system" .
cat > "$stage/rpm/SPECS/ebirforms.spec" <<EOF
Name: ebirforms
Version: ${VERSION//-/\~}
Release: 1
Summary: Philippine BIR tax return filing application
License: MIT
URL: https://github.com/hexuria/buwiz-forms
BuildArch: $RPM_ARCH
Source0: payload.tar.gz
Requires: gtk4 >= 4.14, webkitgtk6.0 >= 2.42, gstreamer1-plugins-base, gstreamer1-plugins-good, dbus-libs, google-noto-sans-fonts
AutoReqProv: no
%description
Prepare, validate and file Philippine BIR tax returns.
%prep
%build
%install
mkdir -p %{buildroot}
tar -xzf %{SOURCE0} -C %{buildroot}
%files
/usr/lib/ebirforms
/usr/bin/ebirforms
/usr/share/applications/dev.goldcoders.bir.desktop
/usr/share/icons/hicolor/*/apps/dev.goldcoders.bir.png
EOF
rpmbuild --define "_topdir $stage/rpm" --define '__os_install_post %{nil}' -bb "$stage/rpm/SPECS/ebirforms.spec"
cp "$stage/rpm/RPMS/$RPM_ARCH/"*.rpm "$OUT/$base.rpm"
if [ "$ARCH" = x64 ]; then
    mkdir -p "$stage/arch"
    tar -czf "$stage/arch/payload.tar.gz" -C "$system" .
    cat > "$stage/arch/PKGBUILD" <<EOF
pkgname=ebirforms
pkgver=$(printf '%s' "$VERSION" | tr '+-~' '...')
pkgrel=1
pkgdesc='Philippine BIR tax return filing application'
arch=('x86_64')
url='https://github.com/hexuria/buwiz-forms'
license=('MIT')
depends=('gtk4>=4.14' 'webkitgtk-6.0' 'gst-plugins-base' 'gst-plugins-good' 'dbus' 'noto-fonts')
options=('!strip' '!debug')
source=('payload.tar.gz')
sha256sums=('$(sha256sum "$stage/arch/payload.tar.gz" | awk '{print $1}')')
package() {
  cp -a "\$srcdir/usr" "\$pkgdir/"
}
EOF
    cp -a "$system/." "$stage/arch/"
    cat > "$stage/arch/.PKGINFO" <<EOF
pkgname = ebirforms
pkgbase = ebirforms
pkgver = $(printf '%s' "$VERSION" | tr '+-~' '...')-1
pkgdesc = Philippine BIR tax return filing application
url = https://github.com/hexuria/buwiz-forms
builddate = $(date +%s)
packager = Goldcoders Corp <support@goldcoders.dev>
size = $(du -sb "$system" | awk '{print $1}')
arch = x86_64
license = MIT
depend = gtk4>=4.14
depend = webkitgtk-6.0
depend = gst-plugins-base
depend = gst-plugins-good
depend = dbus
depend = noto-fonts
EOF
    tar --zstd --owner=0 --group=0 -cf "$OUT/$base.pkg.tar.zst" -C "$stage/arch" .PKGINFO usr
    cp "$stage/arch/PKGBUILD" "$OUT/ebirforms-bin.PKGBUILD"
    sed -i "s|source=(.*|source=('https://github.com/hexuria/buwiz-forms/releases/download/v$VERSION/$base.pkg.tar.zst')|; s|sha256sums=(.*|sha256sums=('$(sha256sum "$OUT/$base.pkg.tar.zst" | awk '{print $1}')')|" "$OUT/ebirforms-bin.PKGBUILD"
    install -m755 "$ROOT/packaging/linux/install-linux.sh" "$OUT/install-linux.sh"
fi
cd "$OUT"
files=("$base.deb" "$base.rpm" "$base.tar.gz" "$updater")
if [ "$ARCH" = x64 ]; then files+=("$base.pkg.tar.zst" ebirforms-bin.PKGBUILD install-linux.sh); fi
sha256sum "${files[@]}" > "SHA256SUMS-linux-${ARCH}.txt"
