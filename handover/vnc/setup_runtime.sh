#!/bin/bash
# Download (no root) the runtime libs bir-desktop needs plus Mesa lavapipe, unpack into $RT.
set -e
RT=/home/agent/.cache/rtroot; DL=/home/agent/.cache/rtdebs
mkdir -p $RT $DL; cd $DL
PKGS="libgtk-3-0t64 libwebkit2gtk-4.1-0 libjavascriptcoregtk-4.1-0 libsoup-3.0-0 libayatana-appindicator3-1 libxdo3 mesa-vulkan-drivers libvulkan1 libxkbcommon-x11-0 libxcb1 libwayland-client0"
# resolve names that exist in this release
OK=""; for p in $PKGS; do apt-cache show $p >/dev/null 2>&1 && OK="$OK $p" || { alt=$(echo $p | sed 's/t64$//'); apt-cache show $alt >/dev/null 2>&1 && OK="$OK $alt" || echo "skip $p"; }; done
echo "pkgs:$OK"
NEED=$(apt-get -s install --no-install-recommends $OK 2>/dev/null | awk '/^Inst /{print $2}')
echo "$(echo $NEED | wc -w) packages to fetch"
apt-get download $NEED >/dev/null 2>&1 || for p in $NEED; do apt-get download $p >/dev/null 2>&1 || echo "download failed: $p"; done
for d in *.deb; do dpkg -x $d $RT; done
ls $RT/usr/share/vulkan/icd.d/ 2>/dev/null
ls $RT/usr/lib/aarch64-linux-gnu | grep -cE '\.so'
echo SETUP_DONE
