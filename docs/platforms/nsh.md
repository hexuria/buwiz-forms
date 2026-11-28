# eBIRForms NSH app: platforms

The NSH (TSX) app in `nsh/`, updated 2026-11-24. Plan and hard rules: [docs/plans/nsh-tsx-migration.md](../plans/nsh-tsx-migration.md) (a dated migration plan, kept as a record). Working notes and commands:
[nsh/AGENTS.md](../../nsh/AGENTS.md). The other pages here ([macos.md](macos.md), [windows.md](windows.md), [linux.md](linux.md)) describe this app.

## Logging

The app writes a daily log file, `ebirforms.YYYY-MM-DD.log`, inside the `logs` folder of its data
directory, and only there: it never writes its log to stdout, so a `just nsh-run` terminal shows
nothing of it. The file on each desktop platform:

- macOS: `~/Library/Application Support/dev.goldcoders.bir/logs/` (a release build inside its group container uses the group container's `logs/` instead)
- Windows: `%LOCALAPPDATA%\dev.goldcoders.bir\logs\`
- Linux: `~/.local/share/dev.goldcoders.bir/logs/`

The Logs screen in the app tails today's file.

## Status

| Platform | Status |
|---|---|
| macOS | Built and tested (NSH 0.6.1, hexuria/nsh `buwiz-next` @ `8c78cfd6`, pinned by `NSH_PINNED_REF` in [.github/workflows/nsh.yml](../../.github/workflows/nsh.yml)): type check, build, headless tests, real-window screenshots; CI on `macos-15`. |
| Windows | Built and tested in CI on `windows-2025` (nsh.yml `NSH app (Windows)` jobs: `nsh check`, `nsh build`, crate tests, headless TSX tests). A real window has not been tried. |
| Linux | Built and tested in CI on `ubuntu-24.04`, including a dev-host build and real-window screenshots (nsh.yml `NSH app (Linux)` jobs). |
| iOS | Builds and runs in the iOS Simulator (`nsh run --target ios` from `nsh/`, iPhone 17, Xcode 26). Print preview is the system print sheet. Sending goes through the local mock relay ([docs/relay.md](../relay.md)). Store packaging is in progress. |
| Android | Runs in development on emulators (`just nsh-run-agent android`, `scripts/run-android-windows.ps1`); the Google Play bundle is `just play-bundle` ([android.md](android.md)), not yet published. |

Cells below marked "written" were recorded before Windows and Linux CI
existed (2026-11-15): the code exists and compiles; the CI jobs named
above now exercise it headless, but a person has not run a real window
on those platforms.

## What each desktop has

| Feature | macOS | Windows | Linux |
|---|---|---|---|
| Forms, tax profile, filings, submission queue (`bir-core`, `bir-app-api`, plain SQLite) | built and tested | written | written |
| Send guard: debug, test and agent runs are dry or use a guarded local mock; normal releases use the filing transport | built and tested | written (plain Rust, its tests run on macOS) | written |
| Database guard: never opens the owner's data folders | built and tested (home folder from the password database; folders also known by device and inode) | built and tested (profile folder from the process's token, never `USERPROFILE`; folders and files also known by volume and file index) | written (the macOS code); names compared case-sensitively; never run |
| OS sign-in for admin recovery (Touch ID or password; Windows Hello) | written; no test shows the real prompt (a test environment answers "no" without asking) | written (`robius-authentication`), never run | stub: answers "no" (`this platform has no OS sign-in prompt`). PIN, authenticator and recovery key work as everywhere |
| Print preview window | built and tested | the TSX side is shared; never opened | the same |
| Print and Save as PDF | built and tested (WebKit makes the PDF, the system print panel prints) | built (WebView2 makes the PDF, tested in a desktop session; the system print dialog, then WebView2 prints) | stub: "Print and Save as PDF are available on macOS and Windows only for now." |
| One copy per database (a second launch brings the first forward) | built and tested | written (std file lock and a loopback port); never run | the same |
| App menu, tray, hide to tray, global hotkey, updater driver | built; a test cannot click the system menu bar or tray | not tried: NSH's `nsh/desktop` decides what exists there | not tried |
| Agent remote control (cargo feature `agent`) | built and tested; the window control plane comes from hexuria/nsh-gpui-agent on every native platform | data plane only; type-checks | the same |
| Private print folder (modes 0700/0600) and removal of stale ones at start | built and tested | per-user temp folder, no ACLs; stale ones removed at start (`OpenProcess`) | written (the Unix code); never run |

## Cross-check from the macOS machine (recorded 2026-11-15, before Windows and Linux CI existed)

Both targets are installed. Commands were `cargo check --locked --manifest-path nsh/Cargo.toml --target <triple>` with the toolchain's own cargo, one run per target, 2026-11-15.

- **As is, both stop in the build scripts of C dependencies** (no Linux or Windows C toolchain; no system packages were installed). Linux: first blocking error in `libz-sys` (`failed to find tool "x86_64-linux-gnu-gcc"`); `ring`, `openssl-sys` and `libsqlite3-sys` stop the same way, and `libdbus-sys` says `pkg-config has not been configured to support cross-compilation` (a real Linux build needs `pkg-config` and `libdbus-1-dev` for the credential-store crate). Windows: first blocking error in `libz-sys` (`'sys/types.h' file not found`: Apple's clang has no Windows headers); `ring` (`'assert.h' file not found`), `openssl-sys` and `libsqlite3-sys` stop too. The app's own crate was never reached.
- **With stand-ins that are not in the repository** the check goes through for both targets, with the crate, its tests (`--tests`) and `--features agent`, zero warnings from our crate: `zig cc` and `zig ar` as the C compiler and archiver (for Windows, a shim that turns `lib.exe` arguments into `zig ar`), a fake `dbus-1.pc` for Linux, and for Windows an empty `ssl.lib`/`crypto.lib` with Homebrew's OpenSSL headers. This is a type check only: nothing was linked or run, and the NSH Windows (Win32) and Linux (GTK 4) backends were never built.
- Changes made from the check and from reading the code (all `cfg` or guard rules, none weakens a test on macOS): the Windows database guard rule above; the save panel's start folder uses std's home folder (Windows has no `HOME`); dead-code allowances for the macOS-only PDF items and the Windows stubs; a Unix-only test helper gated; one test row that assumes a case-insensitive file system is now macOS-only (on Linux it would have failed; not run there).

## Prerequisites

All systems:

- Rust 1.99.0 with `rustfmt` and `clippy` (`rust-toolchain.toml`), and a C compiler (SQLite and `ring` are C; OpenSSL is built from source, which needs Perl).
- [huacnlee/cargo](https://github.com/huacnlee/cargo): what the owner's Mac and NSH's own CI use (`curl -fsSL https://huacnlee.github.io/cargo/install.sh | sh`). Building the app with stock cargo has not been tried.
- `just` 1.45.0 (the recipes are `[unix]`: bash).
- **The NSH checkout beside this repository**: `<parent>/buwiz-forms` and `<parent>/nsh`. `nsh/Cargo.toml` names the NSH crates as `../../nsh/crates/...`. The commit is `NSH_PINNED_REF` in `.github/workflows/nsh.yml` (hexuria/nsh branch `buwiz-next`, `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`).
- The `nsh` command line, built from that checkout: `cargo install --locked --path <parent>/nsh/crates/nsh-cli --root ~/BuwizNsh/tools` (10 minutes cold on the owner's Mac). The recipes call `~/BuwizNsh/tools/bin/nsh`.
- Network on the first `nsh check`: it downloads the TypeScript 7.0.2 compiler from npm.

macOS: macOS 14 or newer (NSH's floor) and the Xcode command line tools.

Windows (nothing here was tried): Windows 10 1809 or newer (NSH's floor), the MSVC build tools with a Windows SDK, and Perl and NASM for the OpenSSL build (the old app's CI installs OpenSSL with `choco` and sets `OPENSSL_DIR`; the same is probably needed).

Linux (nothing here was tried): GTK 4.14 or newer for the window (Ubuntu 24.04), `build-essential pkg-config perl libdbus-1-dev`, and what NSH's own CI installs for its Linux app (`libgtk-4-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev`; see the NSH checkout's `.github/workflows/rust-app.yml` and `linux.yml`).

## Build, test, run

From the repository root, with the four safety variables (the recipes set them): `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`.

```sh
just nsh-check                        # cargo check of the crate, then tsc (silent on success; add -v)
just nsh-build                        # bundle and compile; every warning is a finding
just nsh-test                         # cargo test, then every *.test.ts(x) headless, file by file
just nsh-test shell/frame.test.tsx    # one file
just nsh-run ~/BuwizNsh/agents/<track>.db    # a window with hot TSX edits (macOS: passes --target macos)
just nsh-shots base                   # real windows, PNGs into ~/BuwizNsh/evidence/base/
just core-plain-test                  # bir-core and bir-app-api tests, plain SQLite (no sqlcipher)
```

Without just: `export PATH="$HOME/BuwizNsh/tools/bin:$PATH"; cd nsh; nsh check; nsh build; nsh test`, and `cargo test --manifest-path nsh/Cargo.toml` for the Rust crate.
`nsh run --target windows` and `--target linux` are NSH's commands for those backends; CI builds and tests on `windows-2025` and `ubuntu-24.04`, while `nsh-run` and `nsh-shots` pass `--target macos` locally.
On Windows, as on macOS, a test environment's scratch database needs no `BIR_DATABASE_PATH` (the guard verifies there). Engines in one run cannot share one named file.

CI ([docs/CI.md](../CI.md)): a pull request that touches `nsh/` or the crates it uses runs `just nsh-check -v` on
Linux (ci.yml's `NSH app type check (Linux)`). `.github/workflows/nsh.yml` runs the steps above on `macos-15`,
`windows-2025` and `ubuntu-24.04`, but only when they are asked for: on a push to `test/...` (`just ci-test`), in a
manual run, and nightly on `main`. It checks out hexuria/nsh at `NSH_PINNED_REF` (with `NSH_REPO_TOKEN` when the
fork is private) and has run and passed.

## Parked

- Windows: a person at a real window (the matrix's untested cells), and
  Print at a printer.
- Linux: Print and Save as PDF (a stub today), and an OS sign-in prompt.
- iOS: a real relay (only the loopback mock exists), device signing and
  packaging, the keychain on a device. Android: the app itself. Screens
  are built wide and narrow with a test at 390x844.
