#!/usr/bin/env bash
set -euo pipefail
case "$KIND" in
    deb|tarball|installer)
        # Give up on a stalled mirror connection after 30 s and retry, rather than waiting on one socket: these images
        # download about 200 MB, and archive.ubuntu.com stalls often enough to use up the container's time limit.
        printf '%s\n' 'Acquire::Retries "5";' 'Acquire::http::Timeout "30";' 'Acquire::https::Timeout "30";' \
            > /etc/apt/apt.conf.d/80ci-network
        # SMOKE_APT_MIRROR replaces archive.ubuntu.com (the x64 images' mirror; arm64 uses ports.ubuntu.com and is left
        # alone). The release workflow sets it to the mirror GitHub's own runners use.
        if [ -n "${APT_MIRROR:-}" ] && [ -f /etc/apt/sources.list.d/ubuntu.sources ]; then
            sed -i "s|http://archive.ubuntu.com/ubuntu|$APT_MIRROR|g" /etc/apt/sources.list.d/ubuntu.sources
        fi
        # Downloaded packages go to the folder smoke-test.sh shares between its Ubuntu containers, so the second and
        # third install from files the first one fetched. (Not /var/cache/apt/archives: the image empties that one
        # after every install.)
        apt_get=(apt-get)
        if [ -d /apt-archives ]; then apt_get+=(-o Dir::Cache::Archives=/apt-archives); fi
        apt-get update
        DEBIAN_FRONTEND=noninteractive "${apt_get[@]}" install -y \
            xvfb xauth dbus-x11 xdotool imagemagick libgtk-4-1 libwebkitgtk-6.0-4 \
            gstreamer1.0-plugins-base gstreamer1.0-plugins-good fonts-noto-core
        ;;
    rpm) dnf install -y xorg-x11-server-Xvfb xorg-x11-xauth dbus-x11 xdotool ImageMagick ;;
    arch) pacman -Syu --noconfirm xorg-server-xvfb xorg-xauth dbus xdotool imagemagick ;;
esac
case "$KIND" in
    deb) "${apt_get[@]}" install -y "/packages/$BASE.deb" ;;
    rpm) dnf install -y "/packages/$BASE.rpm" ;;
    arch) pacman -U --noconfirm "/packages/$BASE.pkg.tar.zst" ;;
    installer) sh /packages/install-linux.sh --from-dir /packages --version "$VERSION" ;;
    tarball)
        mkdir -p "$HOME/.local/share/ebirforms"
        tar -xzf "/packages/$BASE.tar.gz" --strip-components=1 -C "$HOME/.local/share/ebirforms"
        ;;
esac
exe=/usr/bin/ebirforms
app=/usr/lib/ebirforms
if [ "$KIND" = tarball ]; then
    app="$HOME/.local/share/ebirforms"
    exe="$app/eBIRForms"
    test ! -e "$app/ebirforms-no-self-update"
else
    test -f "$app/ebirforms-no-self-update"
    test ! -s "$app/ebirforms-no-self-update"
    test "$(readlink -f "$exe")" = "$app/eBIRForms"
    test -f /usr/share/applications/dev.goldcoders.bir.desktop
    for size in 48 64 128 256 512; do
        test -f "/usr/share/icons/hicolor/${size}x${size}/apps/dev.goldcoders.bir.png"
    done
fi
test -f "$app/nsh/app.json"
test -d "$app/print-overrides"
ldd "$exe" | tee "/evidence/$KIND-ldd.txt"
if grep -q 'not found' "/evidence/$KIND-ldd.txt"; then exit 1; fi
export EXE="$exe" GSK_RENDERER=cairo
export XDG_DATA_HOME=/scratch/data XDG_CACHE_HOME=/scratch/cache BIR_DATABASE_PATH=/scratch/filing.db
mkdir -p "$XDG_DATA_HOME" "$XDG_CACHE_HOME"
# shellcheck disable=SC2016
dbus-run-session -- xvfb-run -a -s '-screen 0 1600x1000x24' bash -euc '
    "$EXE" > "/evidence/$KIND-app.log" 2>&1 &
    pid=$!
    trap "kill $pid 2>/dev/null || true" EXIT
    window="$(timeout 90 xdotool search --sync --onlyvisible --name eBIRForms | head -1)"
    kill -0 "$pid"
    import -pause 3 -window "$window" "/evidence/$KIND-app-open.png"
'
case "$KIND" in
    deb|installer) apt-get remove -y ebirforms ;;
    rpm) dnf remove -y ebirforms ;;
    arch) pacman -Rns --noconfirm ebirforms ;;
    tarball) rm -rf "$app" ;;
esac
test ! -e "$app"
test ! -L "$exe"
test ! -e "$exe"
if [ "$KIND" = tarball ]; then
    test ! -e "$HOME/.local/share/applications/dev.goldcoders.bir.desktop"
else
    test ! -e /usr/share/applications/dev.goldcoders.bir.desktop
    for size in 48 64 128 256 512; do
        test ! -e "/usr/share/icons/hicolor/${size}x${size}/apps/dev.goldcoders.bir.png"
    done
fi
echo "PASS: $KIND install, visible app window, uninstall (executable, resources, launcher and icons removed)" | tee "/evidence/$KIND-uninstall.txt"
