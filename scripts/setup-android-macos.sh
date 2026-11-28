#!/usr/bin/env bash
#
# setup-android-macos.sh — what `just play-bundle` needs on a Mac: the nsh CLI and NSH's Android release runtime at the
# NSH pin, checked against the Android SDK, the NDK, a JDK and the Rust Android targets. The macOS counterpart of
# scripts/setup-android-windows.ps1, for building the Google Play bundle only (no emulator: Android runs are done on Linux
# and Windows machines, docs/platforms/android.md).
#
# Idempotent: a piece already at the pin is kept. It installs nothing system-wide: a missing SDK package, NDK, JDK or Rust
# target is reported with the command that adds it, and the script stops.
#
# The tools go to $NSH_TOOLS (default ~/BuwizNsh/tools, the folder every nsh recipe uses). On a Mac whose shared tool must
# stay as it is (other work builds with it), put the pinned one beside it and point play-bundle there:
#
#   NSH_TOOLS=~/BuwizNsh/store-tools scripts/setup-android-macos.sh
#   NSH_TOOLS=~/BuwizNsh/store-tools just play-bundle <n> --allow-dotenv
#   (--allow-dotenv: the checkout's .env is meant; the bundle bakes nothing from it, and the Google client only with
#   STORE_GMAIL=1: docs/platforms/android.md)
#
# Usage:
#   scripts/setup-android-macos.sh           # check, then build what is missing
#   scripts/setup-android-macos.sh --check   # check only, build nothing
#   scripts/setup-android-macos.sh --rebuild # build the CLI and the runtime again even if they are recorded at the pin
set -euo pipefail

check_only=0 rebuild=0
case "${1:-}" in --check) check_only=1 ;; --rebuild) rebuild=1 ;; esac

repo="$(cd "$(dirname "$0")/.." && pwd -P)"
checkout="$(dirname "$repo")/nsh"
tools="${NSH_TOOLS:-$HOME/BuwizNsh/tools}"
pin="$(sed -n 's/^ *NSH_PINNED_REF: *//p' "$repo/.github/workflows/nsh.yml" | head -1)"
[ -n "$pin" ] || { echo "setup-android-macos: NSH_PINNED_REF not found in .github/workflows/nsh.yml" >&2; exit 1; }

step() { printf '\n==> %s\n' "$*"; }
fail() { echo "setup-android-macos: $*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || fail "this script is for macOS"

# --- What only the person can install -----------------------------------------------------------------------------------
step "Android SDK, NDK, JDK and Rust targets"
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
[ -d "$sdk" ] || fail "no Android SDK at $sdk: install Android Studio's SDK, or the command-line tools, and set ANDROID_HOME"
target_sdk="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["android"]["targetSdk"])' "$repo/nsh/nsh.json")"
[ -d "$sdk/platforms/android-$target_sdk" ] \
    || fail "no platforms;android-$target_sdk (nsh/nsh.json android.targetSdk): sdkmanager \"platforms;android-$target_sdk\""
build_tools="$(ls "$sdk/build-tools" 2>/dev/null | sort -V | tail -1)"
[ -n "$build_tools" ] || fail "no build-tools: sdkmanager \"build-tools;$target_sdk.0.0\""
ndk="${ANDROID_NDK_HOME:-}"
[ -n "$ndk" ] || ndk="$(ls -d "$sdk"/ndk/* 2>/dev/null | sort -V | tail -1)"
[ -n "$ndk" ] && [ -d "$ndk" ] || fail "no NDK: sdkmanager \"ndk;27.3.13750724\" (or set ANDROID_NDK_HOME)"
jdk=""
for bin in "${JAVA_HOME:+$JAVA_HOME/bin}" /opt/homebrew/opt/openjdk/bin "/Applications/Android Studio.app/Contents/jbr/Contents/Home/bin"; do
    if [ -n "$bin" ] && [ -x "$bin/jarsigner" ] && "$bin/jarsigner" -help >/dev/null 2>&1; then jdk="$bin"; break; fi
done
[ -n "$jdk" ] || fail "no JDK (jarsigner): brew install openjdk"
installed="$(rustup target list --installed 2>/dev/null || true)"
for triple in aarch64-linux-android armv7-linux-androideabi; do
    grep -qx "$triple" <<<"$installed" || fail "Rust target $triple missing: rustup target add $triple"
done
echo "SDK $sdk, android-$target_sdk, build-tools $build_tools, NDK $(basename "$ndk"), JDK $jdk"

# --- NSH at the pin --------------------------------------------------------------------------------------------------------
step "NSH checkout ($checkout) at $pin"
[ -d "$checkout/.git" ] || [ -f "$checkout/.git" ] \
    || fail "no NSH checkout beside this repository at $checkout: git clone https://github.com/hexuria/nsh \"$checkout\""
head="$(git -C "$checkout" rev-parse HEAD)"
if [ "$head" != "$pin" ]; then
    [ "$check_only" = 0 ] || fail "the NSH checkout is at $head, not the pin $pin"
    [ -z "$(git -C "$checkout" status --porcelain)" ] || fail "the NSH checkout has local changes: leave it clean, it is moved to the pin"
    git -C "$checkout" fetch origin "$pin"
    git -C "$checkout" checkout --detach "$pin"
fi
echo "at $(git -C "$checkout" log --oneline -1)"

# The NSH checkout's own toolchain cargo builds the tool and the runtime, never a cargo ahead of rustup's proxy that
# shares one build folder between checkouts (a huacnlee/cargo build on the owner's Mac): another NSH checkout's crates
# could end up in the store tool.
nsh_cargo="$(cd "$checkout" && rustup which cargo 2>/dev/null)" || fail "rustup cannot name the NSH checkout's cargo"
nsh_cargo_dir="$(dirname "$nsh_cargo")"
export PATH="$nsh_cargo_dir:$PATH"
case "$(cargo --version)" in *huacnlee/cargo*) fail "cargo is still $(cargo --version), which shares one build folder between checkouts" ;; esac
echo "building with $(cargo --version) ($(command -v cargo))"

step "nsh CLI in $tools"
have="$("$tools/bin/nsh" --version 2>/dev/null || true)"
if [ "$rebuild" = 0 ] && [ "$have" != "" ] && [ "$(cat "$tools/nsh-ref" 2>/dev/null || true)" = "$pin" ] && [[ "$have" == *"(${pin:0:7})"* ]]; then
    echo "$have: at the pin"
else
    [ "$check_only" = 0 ] || fail "the nsh CLI in $tools is '${have:-missing}', not the pin ${pin:0:7}"
    cargo install --locked --path "$checkout/crates/nsh-cli" --root "$tools" --force
    echo "$pin" > "$tools/nsh-ref"
    "$tools/bin/nsh" --version
fi

step "NSH's Android release runtime in $tools/runtime/android/release"
runtime="$tools/runtime/android/release"
if [ "$rebuild" = 0 ] && [ -f "$runtime/classes.dex" ] && [ "$(cat "$runtime/nsh-ref" 2>/dev/null || true)" = "$pin" ]; then
    echo "at the pin"
else
    [ "$check_only" = 0 ] || fail "no Android release runtime at the pin in $runtime"
    (cd "$checkout" && ANDROID_HOME="$sdk" ANDROID_NDK_HOME="$ndk" cargo xtask android --release)
    built="${CARGO_TARGET_DIR:-$checkout/target}/android/release"
    [ -f "$built/classes.dex" ] || built="$checkout/target/android/release"
    [ -f "$built/classes.dex" ] || fail "cargo xtask android --release left no classes.dex in $built"
    rm -rf "$runtime"
    mkdir -p "$runtime"
    cp -R "$built/." "$runtime/"
    echo "$pin" > "$runtime/nsh-ref"
fi
ls "$runtime"

step "Ready"
echo "NSH_TOOLS=$tools just play-bundle <n> --allow-dotenv   (bakes nothing from .env; unsigned: the owner signs it with just play-sign)"
