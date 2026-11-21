# Devin environment recipes

Copy-paste setup for a Devin environment (or any fresh machine) that builds buwiz-forms and runs the NSH app in the
Android emulator and the iOS simulator. Each platform is one block of a Devin blueprint; paste the blocks you need into
the repository's blueprint (Settings > Environment, or ask Devin to "update the blueprint with
docs/platforms/devin-environment.md").

| Platform | Desktop app | Android emulator | iOS simulator |
| --- | --- | --- | --- |
| Windows | `scripts/setup-windows.ps1` | `scripts/setup-android-windows.ps1` | not available |
| macOS | `scripts/setup-macos.sh` + NSH CLI | not covered here yet | Xcode + iOS Rust targets |
| Linux | `scripts/setup-linux.sh` + NSH sidecar | not covered here yet | not available |

The NSH checkout is a sibling of this repository (`../nsh`), at `NSH_PINNED_REF` from `.github/workflows/nsh.yml`.
It is read-only: never edit it.

## Local account server (all platforms)

Start the in-memory account mock before launching NSH; debug builds use
`http://localhost:3008` without overrides. See `docs/cloud-account.md` for the
browser approval flow and `--auto-approve` for headless tests. No Postgres or
secrets are needed. Never expose the mock publicly.

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

## Windows (Android)

`scripts/setup-android-windows.ps1` installs JDK 17, the Android command-line tools, `platform-tools`, the emulator,
`platforms;android-35`, `build-tools;35.0.1`, NDK r27, the API 35 Google APIs x86_64 image, an AVD named
`buwiz_api35`, LLVM (libclang for bindgen), and the Rust targets `x86_64-linux-android` and `aarch64-linux-android`.
It sets `ANDROID_HOME`, `ANDROID_SDK_ROOT`, `ANDROID_NDK_HOME`, `JAVA_HOME` and `LIBCLANG_PATH` machine-wide and enables
the Windows Hypervisor Platform feature, which the emulator needs for acceleration. That feature takes effect after a
reboot: on a fresh snapshot it is active; on the machine that ran the script, reboot before starting the emulator.

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
  - name: NSH checkout at the pinned commit and the nsh CLI
    run: |
      $env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
      $pin = ((Select-String -Path .github\workflows\nsh.yml -Pattern 'NSH_PINNED_REF:\s*(\S+)').Matches[0].Groups[1].Value)
      if (-not (Test-Path ..\nsh)) { git clone https://github.com/hexuria/nsh ..\nsh }
      git -C ..\nsh fetch origin $pin
      git -C ..\nsh checkout --detach $pin
      $env:RUSTUP_TOOLCHAIN = '1.99.0'; $env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'; $env:RUST_MIN_STACK = '268435456'
      cargo install --locked --path ..\nsh\crates\nsh-cli --root "$HOME\BuwizNsh\tools"
      exit $LASTEXITCODE
knowledge:
  - name: startup
    contents: |
      GPUI app: $env:BUWIZ_NO_KEYCHAIN=1; $env:EBIR_TEST_ENV=1; $env:BIR_AUTH_BYPASS=1; $env:BIR_SFTP_DRY_RUN=1; just run
      Android (from nsh/, see docs/relay.md "Android development APK"):
        # Start `just mock-account` in another terminal first.
        Start-Process emulator '-avd buwiz_api35'; adb wait-for-device
        $env:Path = "$HOME\BuwizNsh\tools\bin;$env:Path"; $env:RUSTUP_TOOLCHAIN = '1.99.0'; $env:RUST_MIN_STACK = '268435456'
        $env:CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS = 'true'; $env:BUWIZ_ANDROID_DEV = '1'
        $env:BUWIZ_ANDROID_DEV_ENV = 'EBIR_TEST_ENV=1;BIR_AUTH_BYPASS=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1;BIR_RELAY_URL=http://127.0.0.1:8787'
        adb reverse tcp:8787 tcp:8787   # before launch, so the app reaches the host relay
        adb reverse tcp:3008 tcp:3008   # app and browser reach the host account mock
        nsh package --target android --abi x86_64 --run
  - name: test
    contents: |
      cargo test -p bir-core ; $env:Path = "$HOME\BuwizNsh\tools\bin;$env:Path" ; cd nsh ; nsh check -v ; nsh build
```

The Android app does not read the shell's safety variables at runtime: they are baked into the APK through
`BUWIZ_ANDROID_DEV_ENV` (or a private `dev.env`), as `docs/relay.md` describes. Never bake secrets into it.

Each step is a new process, so steps after the first reload `Path` from the registry to see what the earlier steps
installed. The scripts self-elevate only when not already elevated; Devin builds run as Administrator.

## Without Devin

Run the same scripts by hand in an administrator PowerShell, then open a new window so the new PATH applies:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup-windows.ps1
powershell -ExecutionPolicy Bypass -File scripts\setup-android-windows.ps1
```
