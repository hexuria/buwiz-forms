# Devin environment recipes

Copy-paste setup for a Devin environment (or any fresh machine) that builds buwiz-forms and runs the NSH app in the
Android emulator and the iOS simulator. Each platform is one block of a Devin blueprint; paste the blocks you need into
the repository's blueprint (Settings > Environment, or ask Devin to "update the blueprint with
docs/platforms/devin-environment.md").

## On another Devin account

[`devin-blueprint.yml`](devin-blueprint.yml) is the whole blueprint (Linux, macOS and Windows blocks) and is all you
need to pass on. On the other account:

1. Connect GitHub and give Devin access to the buwiz-forms repository (for example `hexuria/buwiz-forms`).
   `hexuria/nsh` is public, so it needs no access.
2. Settings > Environment > buwiz-forms > Blueprint: paste `devin-blueprint.yml`, delete the blocks for platforms you
   don't use, and save. Each block builds its own snapshot; the Windows Android build and the Linux NSH sidecar take
   the longest.
3. Start sessions on the platform you want (Linux is the default; pick macOS or Windows for the others). The
   blueprint's `knowledge` entries tell each session how to start the apps, the emulator or Simulator, and
   `just mock-account` for Sign in.

No secrets are needed. The blueprint reads `scripts/setup-*.sh`/`.ps1`, `just mock-account` and `NSH_PINNED_REF`
from the checked-out branch, so that branch must include them.

| Platform | Desktop app | Android emulator | iOS simulator |
| --- | --- | --- | --- |
| Windows | `scripts/setup-windows.ps1` | `scripts/setup-android-windows.ps1` | not available |
| macOS | `scripts/setup-macos.sh` + NSH CLI | not covered here yet | Xcode + iOS Rust targets |
| Linux | `scripts/setup-linux.sh` + `scripts/setup-linux-nsh.sh` (NSH sidecar) | `scripts/setup-linux-nsh.sh` (port 8792; see below) | not available |

The NSH checkout is a sibling of this repository (`../nsh`), at `NSH_PINNED_REF` from `.github/workflows/nsh.yml`.
It is read-only: never edit it.

## Verification tooling (all platforms)

Every setup script (`setup-linux.sh`, `setup-macos.sh`, `setup-windows.ps1`) installs the `impeccable` CLI with
`cargo install --locked` and downloads the `impeccable-rust` agent skill (`SKILL.md`, `performance.md`) from
`hexuria/impeccable-rust` into `.agents/skills/impeccable-rust/` at the repository root, where agents look for skills.
`.agents/` is git-ignored because agent artifacts are never committed, so the skill is downloaded at a pinned commit
instead; a `.commit` file there records which one. `--check` reports both; `--no-impeccable` (`-NoImpeccable` on
Windows) skips both. The general scripts do not install the verification tools themselves.

The Linux blueprint block adds them: it installs `valgrind` in `initialize`, and its `maintenance` installs a prebuilt
`cargo-binstall` (the newest release needs a newer Rust than this repo pins) and runs `impeccable setup host
--with-kani`. Maintenance also sets `IMPECCABLE_BACKEND=host` in the environment on every build because impeccable's Docker toolbox ships an older Rust than `rust-toolchain.toml`, so toolbox runs
cannot build this workspace. `impeccable doctor` then shows every tool on the host except TLC, which needs Java and
the TLA+ tools jar and is not installed.

To move to a new version, edit `scripts/impeccable.pin` (CLI version, skill commit, cargo-binstall version); nothing
else hard-codes them. Linux maintenance re-downloads `cargo-binstall` when its pin moves, runs
`scripts/setup-impeccable.sh`, and re-syncs the host tools when the CLI or `cargo-binstall` version changes; elsewhere
re-run the platform's setup script. On Linux and macOS, if Cargo's install directory (`$CARGO_INSTALL_ROOT/bin`, else
`$CARGO_HOME/bin`, else `~/.cargo/bin`) is not on `PATH`, the setup scripts add it for their own run and warn you to
add it to your shell profile.

## Local account server (all platforms)

The standard Linux/macOS desktop and iOS Simulator startup recipes bypass
the account gate deliberately and do not start the account mock. To exercise
first-launch sign-in instead, start the mock below and launch without
`BIR_AUTH_BYPASS` (or `SIMCTL_CHILD_BIR_AUTH_BYPASS` on iOS). On macOS use
`EBIR_TEST_ENV=1 BUWIZ_NO_KEYCHAIN=1 BIR_AUTH_BYPASS=0 BIR_SFTP_DRY_RUN=1 nsh run --target macos`
from `nsh/` directly instead of `just nsh-run`. `EBIR_TEST_ENV=1` keeps mock
credentials beside a scratch database rather than overwriting the saved real
account. On Linux use
`~/BuwizNsh/nsh2404.sh env GSK_RENDERER=cairo BIR_AUTH_BYPASS=0 nsh run --target linux --port 8791`.
Android's baked development settings do not bypass sign-in and need the mock.

Start the in-memory account mock before launching NSH; debug builds use
`http://localhost:3008` without overrides. See `docs/cloud-account.md` for the
browser approval flow and `--auto-approve` for headless tests. No Postgres or
secrets are needed. Never expose the mock publicly.

Verification boundary: `bir-core::mock_account` is an in-memory development
fixture, not the production account service or submission worker. Its pending,
denied, expired and one-time device grants are checked by the module's Rust
tests (`cargo test -p bir-core mock_account`), including the real HTTP client
flow. The system model is unaffected: this fixture does not change production
poll scheduling, retries, ownership handoff or submission liveness, and is not
a model of the production account service's concurrency guarantees.

Linux and macOS (separate terminal, or background it):

```sh
just mock-account &
# Linux NSH Docker sidecars must use --network host.
# Then launch the NSH app with the platform's usual command.
```

Windows (separate PowerShell window):

```powershell
just mock-account
```

Before Android launch on any platform, with the emulator/device connected:

```sh
adb reverse tcp:3008 tcp:3008
```

The iOS Simulator shares the host's localhost; start `just mock-account &`
before `nsh run --target ios`, with no port forwarding. Keep the account mock
running alongside `mock-bir` and `mock-relay` when testing both sign-in and filing.

## Linux (desktop and Android)

Use separate explicit development ports so starting Android does not replace
the desktop's remembered NSH development session. With the blueprint's sidecar
installed, launch each in its own terminal:

```sh
# Cairo avoids stale pixels in the NSH GTK window.
~/BuwizNsh/nsh2404.sh env GSK_RENDERER=cairo nsh run --target linux --port 8791
```

With the Android emulator running:

The blueprint provisions the SDK/emulator and installs the pinned NSH Android
development runtime under `~/BuwizNsh/tools/runtime/android/dev`. The sidecar
bakes `BUWIZ_ANDROID_DEV_ENV='EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1'`
into the Android app so it uses a scratch database, disables the keychain, and
does not send live filings. Android uses NSH port 8792, distinct from desktop's
8791.

```sh
just mock-account &
~/Android/Sdk/platform-tools/adb reverse tcp:3008 tcp:3008
~/BuwizNsh/nsh2404.sh nsh run --target android --port 8792
```

### A Linux emulator machine from nothing

What a new Linux machine needs before its first Android check (written from setting up `buwiz-forms-linux-5`, which
booted API 35 in about two minutes and reached the app's first screen in about 14 minutes from a cold build):

1. `ls -l /dev/kvm` and `test -r /dev/kvm -a -w /dev/kvm`: without KVM the emulator is software-only and too slow
   for these checks; stop there.
2. At least 40 GB free on the home volume: the NSH, app and three-ABI Android builds fill a smaller disk part way
   through.
3. Bring the checkout's `main` up to date (`git fetch && git merge --ff-only origin/main`) first: the setup script reads
   the NSH pin from the checkout.
4. `scripts/setup-linux-nsh.sh`. A machine set up before this script passed the caller's `CARGO_TARGET_DIR`, `NSH_TOOLS`
   and `NSH_RUNTIME` into its container must run it again: until then every NSH command there builds in the shared folder. It ends with every line green, including `nsh android dev runtime` and
   `nsh android release runtime` at the pin. Both are copies with the commit in `nsh-ref`. `nsh package` takes the
   release one even for a development build. Never link a runtime into an NSH checkout's build folder, and never run
   `nsh update`, which replaces the tool with Longbridge's latest release: either one makes every later check a check
   of another NSH than the pin. On a Devin machine, whose `~/.gitconfig` sends GitHub through Devin's git proxy, the
   container fetches with the git CLI and without that file (`CARGO_NET_GIT_FETCH_WITH_CLI=true`,
   `GIT_CONFIG_GLOBAL=/dev/null`): the proxy's sign-in does not work in the container, and Cargo's first fetch failed
   there. The script sets this itself; a wrapper patched by hand is replaced when the script runs again.
5. `emulator -accel-check`, then `emulator -avd buwiz -no-window -no-audio -gpu swiftshader_indirect &` and wait for
   `getprop sys.boot_completed` = 1.
6. `just mock-account --auto-approve &` for unattended runs: without it a sign-in waits for an approval in a browser.
7. `adb reverse tcp:3008 tcp:3008`. If the app stays on "Connecting…", the development host is dialling a port other
   than `--port`: read it from `adb logcat` (`ws://127.0.0.1:<port>/ws`) and reverse that one to `--port` too.
8. For recordings: `adb shell settings put system show_touches 1` (and `pointer_location 1` when coordinates matter).
   `adb shell input text` takes `%s` for a space.

A check that builds an older app commit, or a branch of NSH, uses one NSH commit for three things: the NSH checkout
beside the app checkout (the app's Rust compiles NSH's crates from `../../nsh` relative to `nsh/`), the `nsh` tool,
and both Android runtimes. Build the tool and runtimes at that commit into a separate tools folder
(`~/BuwizNsh/tools-<commit>`, with `nsh-ref`), use a separate pair of checkouts whose `nsh` is at that commit, and point
the build at the folder with `PATH` and `NSH_RUNTIME`, leaving `~/BuwizNsh/tools` and the shared checkouts at the current
pin. A build that mixes commits can fail far from the cause: when the Rust side and the Java host disagree on a command's
fields, the host stops with `BufferUnderflowException` in `Mounter.command`. Every report names the app commit, the
sibling NSH commit, the NSH commit of the `nsh` tool and the SHA-256 of the runtime `classes.dex` the package used.

Each checkout builds into its own folder (`CARGO_TARGET_DIR=~/BuwizNsh/target-<commit>`) for anything reported as
evidence. Cargo cannot tell two checkouts of the same package version apart in one shared build folder, so a "before"
build can link crates compiled from the "after" checkout, form definitions included (they are compiled into
`bir-core`), with no error. A before and after is two checkouts and two build folders, and the report names the folder
and the commit for each binary. A machine whose first `cargo` is a build that ignores `CARGO_TARGET_DIR` (huacnlee/cargo, as on the owner's
Mac: it prints "ignoring the configured target directory") puts every checkout of a repository into ONE folder under
`~/.cargo/targets/`, so the trap applies to every build there. The recipes whose output is evidence or a package
(`nsh-test`, `nsh-shots`, `nsh-agent-check`, `nsh-run-agent`, `play-bundle`, and the Apple store recipes) put the pinned
toolchain's own cargo (`rustup which cargo`) first on `PATH`, which the `nsh` tool then uses too, and refuse to build
if `cargo --version` still names the fork. The everyday recipes use whatever cargo comes first.

## Windows (Android)

`scripts/setup-android-windows.ps1` installs JDK 17, the Android command-line tools, `platform-tools`, the emulator,
`platforms;android-35` and `platforms;android-36` (the app targets API 36, `nsh/nsh.json`), `build-tools;35.0.1` and
`build-tools;36.0.0`, NDK r27, the API 35 Google APIs x86_64 image, an AVD named
`buwiz_api35`, LLVM (libclang for bindgen), and the Rust targets `x86_64-linux-android` and `aarch64-linux-android`.
A machine that only builds and packages Android (no nested virtualization, so no emulator) runs it with `-NoEmulator`:
unavailable emulator acceleration is then a warning, and the NSH command line and the Android runtime are still set up.
It sets `ANDROID_HOME`, `ANDROID_SDK_ROOT`, `ANDROID_NDK_HOME`, `JAVA_HOME` and `LIBCLANG_PATH` machine-wide and enables
the Windows Hypervisor Platform feature, which the emulator needs for acceleration. That feature takes effect after a
reboot. The blueprint's `initialize` steps run setup and launch during the same build boot, so the launch step
uses `-SkipIfRebootPending`: when acceleration is unavailable and WHPX awaits a reboot, it prints a restart notice
and succeeds without packaging or launching. False or missing CPU virtualization flags are inconclusive under a
hypervisor: the launcher warns that VT-x could not be verified before the reboot, rather than rejecting the build.
`HypervisorPresent` alone does not prove nested virtualization is exposed. The post-reboot acceleration check remains
mandatory; deferral is not proof that the emulator will work. Setup records the current Windows boot time, so this deferral expires
after a reboot (including a session VM booting from the snapshot). Run the normal launcher from the startup knowledge
after boot. For a direct launch on the setup machine, run `Restart-Computer` first, then rerun
`powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run-android-windows.ps1`.
Without the build-only switch, a pending reboot stops the launcher with that instruction; other acceleration failures
remain errors, even with the switch.

```yaml
runs-on: windows
shell: powershell
initialize:
  - name: desktop toolchain (VS Build Tools, Rust, OpenSSL, just, Perl, NASM)
    run: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\setup-windows.ps1
  - name: Android SDK, NDK, emulator and AVD
    run: |
      $env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
      powershell -NoProfile -ExecutionPolicy Bypass -File scripts\setup-android-windows.ps1
      exit $LASTEXITCODE
  - name: package and launch Android with the mock account server
    run: |
      $env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
      powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run-android-windows.ps1 -SkipIfRebootPending
      exit $LASTEXITCODE
knowledge:
  - name: startup
    contents: |
      Android: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run-android-windows.ps1
      Desktop app (separate process, repository root):
        $env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
        $env:OPENSSL_DIR = [Environment]::GetEnvironmentVariable('OPENSSL_DIR','User')
        $env:OPENSSL_LIB_DIR = [Environment]::GetEnvironmentVariable('OPENSSL_LIB_DIR','User')
        $env:BUWIZ_NO_KEYCHAIN=1; $env:EBIR_TEST_ENV=1; $env:BIR_SFTP_DRY_RUN=1; just run
  - name: test
    contents: |
      cargo test -p bir-core ; $env:Path = "$HOME\BuwizNsh\tools\bin;$env:Path" ; cd nsh ; nsh check -v ; nsh build
```

The Android app does not read the shell's safety variables at runtime: they are baked into the APK through
`BUWIZ_ANDROID_DEV_ENV` (or a private `dev.env`), as `docs/relay.md` describes. Never bake secrets into it.

Each step is a new process, so steps after the first reload `Path` from the registry to see what the earlier steps
installed. The scripts self-elevate only when not already elevated; Devin builds run as Administrator.

Setup checks out `NSH_PINNED_REF`, rebuilds the CLI when the pin changes, installs all three
Android Rust targets on toolchain 1.99.0, and runs `cargo xtask android --dev` and
`cargo xtask android --release` in `../nsh`. The outputs are copied into
`~/BuwizNsh/tools/android/bin-dev` and `android/bin`, respectively, and the CLI's
`NSH_RUNTIME/android/dev` and `NSH_RUNTIME/android/release` layout under `tools/runtime`.
Packaging requires the real release runtime, not a renamed development runtime.

MSYS2 (`choco install msys2`, `pacman -S --needed --noconfirm perl make`) lives at
`C:\tools\msys64`. Android builds put its `usr\bin` and NDK LLVM bin first on PATH,
set `PERL=C:/tools/msys64/usr/bin/perl.exe`, unset `PERL5LIB`, set
`GNUMAKEFLAGS='CC=clang.exe AR=llvm-ar.exe RANLIB=llvm-ranlib.exe'`, and set
`RANLIB_<triple>` to NDK llvm-ranlib for each ABI. `CARGO_PROFILE_DEV_DEBUG=0`
limits debug artifacts. Strawberry/Git Perl do not work for Android OpenSSL builds.
Host-side dependencies use `X86_64_PC_WINDOWS_MSVC_OPENSSL_NO_VENDOR=1` and the user-scope
OpenSSL directories. New Windows shells must reload both directories, not only PATH.
The ENVRC generator converts Devin's Git Bash `/c/...` filename to a native Windows path
before writing quoted exports with forward-slash values because Devin's envrc
is sourced by Git Bash; bare Windows paths containing spaces or backslashes are not shell-safe.

`setup-android-windows.ps1 -Run` chains setup and launch. The run helper removes non-x86_64
libraries, strips libnsh.so, zipaligns, signs with the standard debug key, then uses
`adb push` and `pm install` instead of streaming the oversized multi-ABI APK.
It cold-boots with `-accel on -feature WindowsHypervisorPlatform -gpu swiftshader_indirect
-cores 4 -memory 4096 -no-snapshot -no-boot-anim` and waits for `sys.boot_completed=1`
(about eight minutes; twenty-minute timeout). `emulator -accel-check` must pass first.
It starts `just mock-account --auto-approve` on loopback port 3008 and sets up
`adb reverse tcp:3008 tcp:3008` before launch. The sign-in helper locates the native
Sign in control by its accessibility label, presses it, and waits for the account gate
to clear. The synthetic mock account is automatically approved; authentication bypass
is deliberately not baked in. Already signed-in installs keep their session. The emulator
screen timeout is extended to thirty minutes for recording. Local taxpayer onboarding
remains a user choice after cloud account sign-in.
Never use real credentials with the mock. Logs and APKs persist under
`~/BuwizNsh/android-windows`. Both emulator and mock server remain running.

## Without Devin

Run the same scripts by hand in an administrator PowerShell, then open a new window so the new PATH applies:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup-windows.ps1
powershell -ExecutionPolicy Bypass -File scripts\setup-android-windows.ps1
```
