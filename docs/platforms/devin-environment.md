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

## Windows (Android)

`scripts/setup-android-windows.ps1` installs JDK 17, the Android command-line tools, `platform-tools`, the emulator,
`platforms;android-35`, `build-tools;35.0.1`, NDK r27, the API 35 Google APIs x86_64 image, an AVD named
`buwiz_api35`, LLVM (libclang for bindgen), and the Rust targets `x86_64-linux-android` and `aarch64-linux-android`.
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
