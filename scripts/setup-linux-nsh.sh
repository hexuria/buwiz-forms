#!/usr/bin/env bash
#
# setup-linux-nsh.sh — NSH app toolchain on Linux: the NSH CLI, the Android SDK
# and emulator, and a wrapper that runs NSH commands with the host display.
# Run after scripts/setup-linux.sh (it needs rustup).
#
# NSH needs GTK >= 4.14 and WebKitGTK 6.0, which Ubuntu 22.04 does not ship, so
# everything that builds NSH runs in an Ubuntu 24.04 Docker image
# (buwiz-nsh-2404) with $HOME mounted. Needs Docker usable without sudo.
#
# What it sets up (idempotent; finished pieces are skipped):
#   - ../nsh checked out at NSH_PINNED_REF (.github/workflows/nsh.yml)
#   - the buwiz-nsh-2404 image (GTK 4, WebKitGTK 6.0, JDK 17, Noto fonts)
#   - the nsh CLI in ~/BuwizNsh/tools/bin, built in the image at the pin
#   - Android (x86_64 host): JDK 17, SDK command-line tools, platform-tools, emulator,
#     android-35, build-tools 35.0.1, NDK 27, the API 35 x86_64 image, an AVD
#     named "buwiz", the Rust Android targets and NSH's Android dev and release runtimes
#     (copies, each with the NSH commit it was built from in nsh-ref)
#   - ~/BuwizNsh/nsh2404.sh <cmd>: runs <cmd> in the image from nsh/ with
#     $DISPLAY (default :0), host networking (`just mock-account` on 127.0.0.1:3008) and
#     the local-test safety variables (baked into Android dev builds)
#
# Usage:
#   scripts/setup-linux-nsh.sh               # everything
#   scripts/setup-linux-nsh.sh --no-android  # desktop NSH only
#   scripts/setup-linux-nsh.sh --check       # verify only, install nothing
#
# Then:
#   just mock-account &
#   ~/BuwizNsh/nsh2404.sh nsh run --target linux
#   emulator -avd buwiz & ~/BuwizNsh/nsh2404.sh nsh run --target android
#
set -euo pipefail

CHECK_ONLY=0
WITH_ANDROID=1
for arg in "$@"; do
    case "$arg" in
        --check)      CHECK_ONLY=1 ;;
        --no-android) WITH_ANDROID=0 ;;
        -h|--help)
            sed -n '2,31p' "$0"
            exit 0
            ;;
        *) echo "Unknown option: $arg (see --help)" >&2; exit 2 ;;
    esac
done

log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/.." && pwd)"
NSH="$(cd "$REPO/.." && pwd)/nsh"
WORK="$HOME/BuwizNsh"
IMAGE="buwiz-nsh-2404"
ANDROID_HOME="$(realpath -m -- "${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}")"
CMDLINE_TOOLS_URL="https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip"
NDK_VERSION="27.2.12479018"
ANDROID_RUNTIME_DEBUG=0
SYSTEM_IMAGE="system-images;android-35;google_apis;x86_64"
AVD="buwiz"

REF="$(sed -n 's/^ *NSH_PINNED_REF: *//p' "$REPO/.github/workflows/nsh.yml" | head -1)"
[ -n "$REF" ] || die "NSH_PINNED_REF not found in .github/workflows/nsh.yml"

SUDO=""
[ "$(id -u)" -ne 0 ] && SUDO="sudo"

# $HOME plus the checkouts when they live outside it, mounted at the same
# paths so builds and the wrapper see the host layout.
outside_home() {
    case "$1/" in "$HOME"/*) return 1 ;; esac
}
MOUNTS=(-v "$HOME:$HOME")
for dir in "$REPO" "$NSH"; do
    outside_home "$dir" && MOUNTS+=(-v "$dir:$dir")
done
# The SDK is mounted only for Android work, once the host install has created
# it: a missing bind source would be created by Docker, owned by root.
sdk_mount() {
    if [ -d "$ANDROID_HOME" ] && outside_home "$ANDROID_HOME"; then
        printf '%s\n' -v "$ANDROID_HOME:$ANDROID_HOME"
    fi
}

# One-off command in the image as the calling user.
in_image() {  # in_image <workdir> <docker -e args...> -- <cmd...>
    local workdir="$1"; shift
    local envs=()
    while [ "$1" != "--" ]; do envs+=("$1"); shift; done
    shift
    # The same git settings as the wrapper (write_wrapper): a host git proxy does not authenticate in the container.
    if git config --global --get-regexp '^(url\..*\.insteadof|credential\..*helper)$' >/dev/null 2>&1; then
        envs+=(-e CARGO_NET_GIT_FETCH_WITH_CLI=true -e GIT_CONFIG_GLOBAL=/dev/null)
    fi
    docker run --rm -u "$(id -u):$(id -g)" \
        -e HOME="$HOME" -e PATH="$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin" \
        -e CARGO_INCREMENTAL=0 "${envs[@]}" \
        "${MOUNTS[@]}" -w "$workdir" "$IMAGE" "$@"
}

nsh_cli_current() {
    [ -x "$WORK/tools/bin/nsh" ] &&
        in_image "$NSH" -- "$WORK/tools/bin/nsh" --version 2>/dev/null | grep -q "(${REF:0:7})"
}

checkout_nsh() {
    [ -e "$NSH/.git" ] || git clone https://github.com/hexuria/nsh "$NSH"
    if [ "$(git -C "$NSH" rev-parse HEAD)" != "$REF" ]; then
        log "Checking out NSH at ${REF:0:7}"
        git -C "$NSH" fetch origin "$REF"
        git -C "$NSH" checkout --detach "$REF"
    fi
}

build_image() {
    log "Building the $IMAGE image (cached after the first run)"
    mkdir -p "$WORK"
    cat > "$WORK/Dockerfile" <<'DOCKER_EOF'
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config perl python3 ca-certificates curl git \
    clang libclang-dev libgtk-4-dev libwebkitgtk-6.0-dev \
    libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
    libdbus-1-dev libssl-dev \
    xvfb xauth at-spi2-core libxtst6 \
    fonts-noto-core fonts-noto-color-emoji \
    && rm -rf /var/lib/apt/lists/*
# Android builds (javac + d8) for `nsh run --target android`.
RUN apt-get update && apt-get install -y --no-install-recommends openjdk-17-jdk-headless \
    && ln -s "$(dirname "$(dirname "$(readlink -f /usr/bin/javac)")")" /opt/java \
    && rm -rf /var/lib/apt/lists/*
ENV JAVA_HOME=/opt/java
DOCKER_EOF
    docker build -q -t "$IMAGE" - < "$WORK/Dockerfile" >/dev/null
}

write_wrapper() {
    cat > "$WORK/nsh2404.sh" <<'NSH_EOF'
#!/usr/bin/env bash
# Runs a command in the buwiz-nsh-2404 image from nsh/, with the host display.
# Written by scripts/setup-linux-nsh.sh.
set -euo pipefail
NSH_EOF
    printf 'app_dir=%q\nandroid_home=%q\nndk_version=%q\n' \
        "$REPO/nsh" "$ANDROID_HOME" "$NDK_VERSION" >> "$WORK/nsh2404.sh"
    cat >> "$WORK/nsh2404.sh" <<'NSH_EOF'
cd "$app_dir"
display="${DISPLAY:-:0}"
DISPLAY="$display" xhost +SI:localuser:"$(id -un)" >/dev/null 2>&1 || true
tty=(); [ -t 0 ] && [ -t 1 ] && tty=(-t)
# NSH's CLI sets CC/AR for Android but not RANLIB, which vendored OpenSSL needs.
ndk="$android_home/ndk/$ndk_version"
RANLIB="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib"
# Unset, the NSH app's own workspace builds full debuginfo: a 4 GB dev APK.
# The Android app reads test settings baked in at build time, not from the
# environment it runs in (nsh/src/dev_env.rs).
# A value from the caller is taken only if it keeps the app a test build that talks to this machine alone. The app
# honours every allowed key baked into a development build (nsh/src/dev_env.rs), so each entry is checked here:
# only keys that file allows, each at most once, EBIR_TEST_ENV=1 and BIR_SFTP_DRY_RUN=1 both present and exactly 1,
# and a server, account or relay address only when it is this machine's own. Anything else is refused and the safe
# default is baked in. (The function's body is in parentheses, not braces: this text sits inside a here-document of
# a brace-bodied function, and scripts/tests/test_linux_nsh_setup.py reads that function up to its closing brace.)
dev_env_default="EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1"
dev_env_refusal() (
  local entry key value seen=";" test_env="" dry_run=""
  local local_url='^https?://(localhost|127\.0\.0\.1|\[::1\])(:[0-9]+)?(/|$)'
  case "$1" in *$'\n'*|*$'\r'*) echo "a line break in the value"; return 0;; esac
  local IFS=';'
  for entry in $1; do
    [ -n "$entry" ] || continue
    case "$entry" in *=*) ;; *) echo "an entry without '='"; return 0;; esac
    key="${entry%%=*}"; value="${entry#*=}"
    case "$seen" in *";$key;"*) echo "$key given twice"; return 0;; esac
    seen="$seen$key;"
    case "$key" in
      EBIR_TEST_ENV) [ "$value" = 1 ] || { echo "EBIR_TEST_ENV must be 1"; return 0; }; test_env=1;;
      BIR_SFTP_DRY_RUN) [ "$value" = 1 ] || { echo "BIR_SFTP_DRY_RUN must be 1"; return 0; }; dry_run=1;;
      BUWIZ_SERVER_URL|BUWIZ_ACCOUNT_URL|BIR_RELAY_URL)
        [[ "$value" =~ $local_url ]] || { echo "$key is not a local address"; return 0; };;
      BIR_AUTH_BYPASS|BUWIZ_NO_KEYCHAIN|GPUI_AGENT|GPUI_AGENT_ADDR|BUWIZ_TRUST_KEYS|BIR_DATABASE_PATH) ;;
      *) echo "$key is not a key a development build takes"; return 0;;
    esac
  done
  [ -n "$test_env" ] || { echo "EBIR_TEST_ENV=1 is missing"; return 0; }
  [ -n "$dry_run" ] || { echo "BIR_SFTP_DRY_RUN=1 is missing"; return 0; }
  return 1
)
dev_env="${BUWIZ_ANDROID_DEV_ENV:-$dev_env_default}"
if why="$(dev_env_refusal "$dev_env")"; then
  echo "nsh2404: BUWIZ_ANDROID_DEV_ENV refused ($why); using the default" >&2
  dev_env="$dev_env_default"
fi
android=(); case " $* " in *android*) android=(-e CARGO_PROFILE_DEV_DEBUG=0
  -e "BUWIZ_ANDROID_DEV_ENV=$dev_env");; esac
# The account and relay addresses reach the container only when they are this machine's own (a local mock). A debug
# build pointed at a real server would sign in there, and on Android the address is compiled into the APK.
urls=()
for name in BUWIZ_SERVER_URL BUWIZ_ACCOUNT_URL; do
  [ -n "${!name:-}" ] || continue
  if [[ "${!name}" =~ ^https?://(localhost|127\.0\.0\.1|\[::1\])(:[0-9]+)?(/|$) ]]; then
    urls+=(-e "$name")
  else
    echo "nsh2404: $name is not a local address; not passed to the build" >&2
  fi
done
uid="$(id -u)"
# A host git config that rewrites GitHub URLs or names a credential helper (a Devin machine's git proxy) does not
# authenticate inside the container, and Cargo's first git fetch fails there. Then the container fetches with the git
# CLI and without the host's global config; NSH's git dependencies are public. Off such a machine nothing changes.
git_env=()
if git config --global --get-regexp '^(url\..*\.insteadof|credential\..*helper)$' >/dev/null 2>&1; then
  git_env=(-e CARGO_NET_GIT_FETCH_WITH_CLI=true -e GIT_CONFIG_GLOBAL=/dev/null)
fi
# In a mock-account e2e the gate stays on: pass the mode, exact mock URL and rooms-mode device folder through, with
# the bypass exactly 0 so the app's dotenv cannot restore it. Every other launch is exactly the old development bypass.
accounts_env=()
if [ "${E2E_ACCOUNTS:-bypass}" = mock ]; then
  [ "${E2E_MOCK_ACCOUNT_URL:-}" = "http://127.0.0.1:3008" ] || {
    echo "nsh2404: E2E_MOCK_ACCOUNT_URL must be http://127.0.0.1:3008 for --accounts mock" >&2
    exit 1
  }
  accounts_env=(-e E2E_ACCOUNTS -e E2E_MOCK_ACCOUNT_URL -e BIR_AUTH_BYPASS=0 -e BIR_DEVICE_DIR)
else
  accounts_env=(-e BIR_AUTH_BYPASS=1)
fi
NSH_EOF
    local mounts=("${MOUNTS[@]}") sdk
    # Kept on --no-android runs too, so they don't break an existing Android setup.
    mapfile -t sdk < <(sdk_mount); mounts+=("${sdk[@]}")
    printf 'mounts=(%s)\n' "$(printf '%q ' "${mounts[@]}")" >> "$WORK/nsh2404.sh"
    cat >> "$WORK/nsh2404.sh" <<'NSH_EOF'
exec docker run --rm -i "${tty[@]}" "${android[@]}" "${git_env[@]}" --network host --security-opt apparmor=unconfined -u "$uid:$(id -g)" \
  -e HOME="$HOME" \
  -e PATH="${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin:$HOME/.cargo/bin:$android_home/platform-tools:/usr/local/bin:/usr/bin:/bin" \
  -e CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/BuwizNsh/target-nsh}" \
  -e NSH_RUNTIME \
  -e CARGO_INCREMENTAL=0 \
  -e ANDROID_HOME="$android_home" -e ANDROID_NDK_HOME="$ndk" \
  -e RANLIB_x86_64_linux_android="$RANLIB" -e RANLIB_aarch64_linux_android="$RANLIB" -e RANLIB_armv7_linux_androideabi="$RANLIB" \
  -e DISPLAY="$display" -e XDG_RUNTIME_DIR="/run/user/$uid" \
  -e DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$uid/bus" \
  -e BUWIZ_NO_KEYCHAIN=1 -e EBIR_TEST_ENV=1 -e BIR_SFTP_DRY_RUN=1 \
  "${accounts_env[@]}" \
  -e GPUI_AGENT -e GPUI_AGENT_ADDR -e GPUI_AGENT_TOKEN -e BIR_DATABASE_PATH -e BUWIZ_ANDROID_DEV \
  -e CARGO -e NSH_REAL_CARGO \
  "${urls[@]}" -e BUWIZ_TRUST_KEYS \
  "${mounts[@]}" \
  -v /tmp/.X11-unix:/tmp/.X11-unix \
  -v "/run/user/$uid:/run/user/$uid" \
  -w "$app_dir" \
  buwiz-nsh-2404 "$@"
NSH_EOF
    chmod +x "$WORK/nsh2404.sh"
}

install_nsh_cli() {
    if nsh_cli_current; then
        log "nsh CLI already at ${REF:0:7}"
        return 0
    fi
    log "Building the nsh CLI at ${REF:0:7} in the image"
    in_image "$NSH" -e CARGO_TARGET_DIR="$WORK/target-nsh" -- \
        cargo install --locked --path crates/nsh-cli --root "$WORK/tools"
}

find_java_home() {
    local java version java_home
    for java in "${JAVA_HOME:+$JAVA_HOME/bin/java}" "$(command -v java || true)"; do
        [ -x "$java" ] || continue
        version="$("$java" -version 2>&1 | sed -n 's/.*version "\([0-9]*\).*/\1/p' | head -1)" || continue
        if [[ "$version" =~ ^[0-9]+$ ]] && [ "$version" -ge 17 ]; then
            java_home="$("$java" -XshowSettings:properties -version 2>&1 | sed -n 's/^ *java.home = //p' | head -1)" || continue
            if [ -n "$java_home" ]; then
                printf '%s\n' "$java_home"
                return 0
            fi
        fi
    done
    return 1
}

install_android_sdk() {
    local java_home
    if ! java_home="$(find_java_home)"; then
        command -v apt-get >/dev/null 2>&1 || die "install a JDK 17 or newer and set JAVA_HOME, then re-run"
        log "Installing JDK 17"
        $SUDO apt-get install -y openjdk-17-jdk-headless
        java_home="$(dpkg-query -L openjdk-17-jdk-headless | sed -n 's#/bin/javac$##p')"
        [ -x "$java_home/bin/java" ] || die "set JAVA_HOME to the installed JDK 17, then re-run"
    fi
    export JAVA_HOME="$java_home" ANDROID_HOME ANDROID_SDK_ROOT="$ANDROID_HOME"
    if ! command -v unzip >/dev/null 2>&1; then
        command -v apt-get >/dev/null 2>&1 || die "install unzip, then re-run"
        $SUDO apt-get install -y unzip
    fi
    local sdkmanager="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
    if [ ! -x "$sdkmanager" ]; then
        log "Installing Android command-line tools in $ANDROID_HOME"
        local tmp; tmp="$(mktemp -d)"
        curl -fsSL -o "$tmp/cmdline-tools.zip" "$CMDLINE_TOOLS_URL"
        unzip -q "$tmp/cmdline-tools.zip" -d "$tmp"
        mkdir -p "$ANDROID_HOME/cmdline-tools"
        rm -rf "$ANDROID_HOME/cmdline-tools/latest"
        mv "$tmp/cmdline-tools" "$ANDROID_HOME/cmdline-tools/latest"
        rm -rf "$tmp"
    fi
    log "Installing Android SDK packages (NDK $NDK_VERSION, API 35)"
    yes | "$sdkmanager" --licenses >/dev/null 2>&1 || true
    "$sdkmanager" "platform-tools" "emulator" "platforms;android-35" "build-tools;35.0.1" \
        "ndk;$NDK_VERSION" "$SYSTEM_IMAGE" >/dev/null
    if ! "$ANDROID_HOME/emulator/emulator" -list-avds 2>/dev/null | grep -qx "$AVD"; then
        log "Creating the $AVD AVD (Pixel 7, API 35)"
        echo no | "$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager" create avd \
            -n "$AVD" -k "$SYSTEM_IMAGE" -d pixel_7 --force >/dev/null
    fi
    rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
}

# NSH's Android runtimes (classes.dex + libnsh.so per ABI): dev for `nsh run`, release for `nsh package`, which takes
# the release one even for a development build. xtask reads its own target/ dir, so no CARGO_TARGET_DIR; the CLI looks
# for them next to its bin/. Each is a COPY with the NSH commit it was built from in `nsh-ref` (the recipes check it), never
# a link into a checkout's build folder: such a link silently follows whatever that checkout builds next.
android_runtime_stamp() {
    printf '%s\n' "$REF" "$NDK_VERSION" "$ANDROID_RUNTIME_DEBUG"
}

nsh_android_runtime_current() {
    local variant="${1:-dev}" abi
    local dest="$WORK/tools/runtime/android/$variant"
    [ ! -L "$dest" ] && [ -s "$dest/classes.dex" ] &&
        [ "$(cat "$dest/nsh-build" 2>/dev/null)" = "$(android_runtime_stamp)" ] &&
        [ "$(cat "$dest/nsh-ref" 2>/dev/null)" = "$REF" ] || return 1
    for abi in arm64-v8a armeabi-v7a x86_64; do
        [ -s "$dest/lib/$abi/libnsh.so" ] || return 1
    done
}

install_android_runtime() {
    local variant flag built
    for variant in dev release; do
        if nsh_android_runtime_current "$variant"; then
            log "NSH Android $variant runtime already at ${REF:0:7}"
            continue
        fi
        if [ "$variant" = dev ]; then flag=--dev built=debug-dev; else flag=--release built=release; fi
        log "Building NSH's Android $variant runtime in the image"
        local sdk; mapfile -t sdk < <(sdk_mount)
        in_image "$NSH" "${sdk[@]}" -e ANDROID_HOME="$ANDROID_HOME" \
            -e ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION" \
            -e CARGO_PROFILE_DEV_DEBUG="$ANDROID_RUNTIME_DEBUG" -- \
            cargo xtask android "$flag"
        local dest="$WORK/tools/runtime/android/$variant"
        rm -rf "$dest" && mkdir -p "$WORK/tools/runtime/android"
        cp -r "$NSH/target/android/$built" "$dest"
        android_runtime_stamp > "$dest/nsh-build"
        printf '%s\n' "$REF" > "$dest/nsh-ref"
        nsh_android_runtime_current "$variant" || die "NSH Android $variant runtime is incomplete"
    done
}

check() {
    local ok=1
    report() { printf '  %-22s %s\n' "$1" "$2"; [ "$3" -eq 1 ] || ok=0; }
    if docker info >/dev/null 2>&1; then report docker "$(docker --version)" 1
    else report docker "MISSING or not usable without sudo" 0; fi
    if docker image inspect "$IMAGE" >/dev/null 2>&1; then report "image:$IMAGE" present 1
    else report "image:$IMAGE" MISSING 0; fi
    if [ -e "$NSH/.git" ] && [ "$(git -C "$NSH" rev-parse HEAD)" = "$REF" ]; then report "../nsh" "${REF:0:7}" 1
    else report "../nsh" "MISSING or not at ${REF:0:7}" 0; fi
    if nsh_cli_current; then report "nsh CLI" "${REF:0:7}" 1
    else report "nsh CLI" "MISSING or not at ${REF:0:7}" 0; fi
    # The wrapper embeds the checkout and SDK paths; a move makes it stale.
    if [ -x "$WORK/nsh2404.sh" ] &&
        grep -qxF "$(printf 'app_dir=%q' "$REPO/nsh")" "$WORK/nsh2404.sh" &&
        { [ "$WITH_ANDROID" -eq 0 ] ||
            { grep -qxF "$(printf 'android_home=%q' "$ANDROID_HOME")" "$WORK/nsh2404.sh" &&
              grep -qxF "$(printf 'ndk_version=%q' "$NDK_VERSION")" "$WORK/nsh2404.sh"; }; }; then
        report "nsh2404.sh wrapper" present 1
    else report "nsh2404.sh wrapper" "MISSING or stale" 0; fi
    if [ "$WITH_ANDROID" -eq 1 ]; then
        if [ -d "$ANDROID_HOME/ndk/$NDK_VERSION" ]; then report "android ndk" "$NDK_VERSION" 1
        else report "android ndk" MISSING 0; fi
        if "$ANDROID_HOME/emulator/emulator" -list-avds 2>/dev/null | grep -qx "$AVD"; then report "avd:$AVD" present 1
        else report "avd:$AVD" MISSING 0; fi
        if nsh_android_runtime_current dev; then report "nsh android dev runtime" "${REF:0:7}" 1
        else report "nsh android dev runtime" "MISSING, incomplete, stale or a link" 0; fi
        if nsh_android_runtime_current release; then report "nsh android release runtime" "${REF:0:7}" 1
        else report "nsh android release runtime" "MISSING, incomplete, stale or a link" 0; fi
    fi
    [ "$ok" -eq 1 ] && log "NSH toolchain ready" || warn "Missing pieces — run scripts/setup-linux-nsh.sh"
    return $((1 - ok))
}

main() {
    if [ "$CHECK_ONLY" -eq 1 ]; then
        check
        exit $?
    fi
    if [ "$WITH_ANDROID" -eq 1 ] && [ "$(uname -m)" != "x86_64" ]; then
        die "Android setup requires an x86_64 Linux host; use --no-android for desktop only"
    fi
    command -v docker >/dev/null 2>&1 || die "Docker is required (NSH builds in an Ubuntu 24.04 image)"
    docker info >/dev/null 2>&1 || die "Docker is not usable as $(id -un); add the user to the docker group"
    # setup-linux.sh may have just installed rustup in another process.
    if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi
    command -v cargo >/dev/null 2>&1 || die "run scripts/setup-linux.sh first (needs rustup)"

    checkout_nsh
    build_image
    install_nsh_cli
    if [ "$WITH_ANDROID" -eq 1 ]; then
        install_android_sdk
        install_android_runtime
    fi
    write_wrapper

    log "Verifying"
    check
    if [ "$WITH_ANDROID" -eq 1 ]; then
        printf '\nAdd to your shell profile:\n  export ANDROID_HOME=%q\n  export ANDROID_SDK_ROOT=%q\n' \
            "$ANDROID_HOME" "$ANDROID_HOME"
    fi
}

main
