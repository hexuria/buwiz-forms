#!/usr/bin/env bash
set -euo pipefail
OUT='' ARCH=''
while [ "$#" -gt 0 ]; do
    case "$1" in
        --out) OUT="${2:?}"; shift 2 ;;
        --arch) ARCH="${2:?}"; shift 2 ;;
        *) echo "Usage: $0 --out DIRECTORY --arch x64|arm64" >&2; exit 2 ;;
    esac
done
test -d "$OUT"
case "$ARCH" in
    x64) platform=linux/amd64; kinds=(deb rpm arch tarball installer) ;;
    arm64) platform=linux/arm64; kinds=(deb rpm tarball) ;;
    *) exit 2 ;;
esac
OUT="$(cd "$OUT" && pwd)"
(cd "$OUT" && sha256sum -c "SHA256SUMS-linux-${ARCH}.txt")
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EVIDENCE="${SMOKE_EVIDENCE:-$ROOT/target/linux-smoke}"
mkdir -p "$EVIDENCE"
EVIDENCE="$(cd "$EVIDENCE" && pwd)"
# The Ubuntu containers (deb, tarball, installer) install the same packages. They share one folder of downloaded
# files, so a slow mirror is paid for once in a run, not once per container.
APT_ARCHIVES="${SMOKE_APT_ARCHIVES:-$EVIDENCE-apt}"
mkdir -p "$APT_ARCHIVES"
APT_ARCHIVES="$(cd "$APT_ARCHIVES" && pwd)"
CONTAINER_TIMEOUT="${SMOKE_CONTAINER_TIMEOUT:-1200}"
in_time=(timeout --kill-after=30 "$CONTAINER_TIMEOUT")
shopt -s nullglob
archives=("$OUT/eBIRForms-Linux-$ARCH-"*.tar.gz)
[ "${#archives[@]}" = 1 ] || { echo 'Expected exactly one download tarball' >&2; exit 1; }
version="$(basename "${archives[0]}" .tar.gz)"
version="${version#eBIRForms-Linux-"$ARCH"-}"
base="eBIRForms-Linux-$ARCH-$version"
for kind in "${kinds[@]}"; do
    case "$kind" in
        deb|tarball|installer) image=ubuntu:24.04 ;;
        rpm) image=fedora:latest ;;
        arch) image=archlinux:latest ;;
    esac
    "${in_time[@]}" docker run --rm --platform "$platform" \
        -v "$OUT:/packages:ro" -v "$EVIDENCE:/evidence" \
        -v "$ROOT/packaging/linux:/smoke:ro" -v "$APT_ARCHIVES:/apt-archives" \
        -e KIND="$kind" -e BASE="$base" -e VERSION="$version" -e APT_MIRROR="${SMOKE_APT_MIRROR:-}" \
        -e BUWIZ_NO_KEYCHAIN=1 -e EBIR_TEST_ENV=1 -e BIR_AUTH_BYPASS=1 -e BIR_SFTP_DRY_RUN=1 \
        "$image" bash /smoke/smoke-container.sh
done
