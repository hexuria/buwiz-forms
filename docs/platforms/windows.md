# eBIRForms on Windows

Status: built and tested in CI nightly and whenever a `test/...` push or a
manual run asks for Windows ([CI.md](../CI.md)): the `NSH app
(Windows)` job in
[.github/workflows/nsh.yml](../../.github/workflows/nsh.yml) runs
`nsh check`, `nsh build`, the Rust crate tests and the headless TSX tests
on `windows-2025`, and `check` in
[.github/workflows/ci.yml](../../.github/workflows/ci.yml) runs
`cargo test --locked --workspace` on `windows-latest`. Tracked in
[#90](https://github.com/hexuria/buwiz-forms/issues/90). Back to the
[README](../../README.md). The same matrix for all platforms is in
[FEATURES.md](../FEATURES.md).

A real window has been run on Windows Server 2022 with the agent
(`nsh run --target windows`, `nsh-windows-handover.md` §4.6; store
screenshots on `try/store-shots-windows`). Cells that need a real Windows
window stay "untested" or "partial"; the pull request that changed this
page carries the exact steps to check them.

## Feature matrix

Legend: ✓ works and is covered by the platform's testing. ✗ does not exist or cannot work
on that platform. partial: works with limits that are named in the note. untested: the code is
there, but nobody has run it on that platform.

| Feature | Status | Notes |
|---|---|---|
| Profiles, tax form library, editors, Check, Save draft | ✓ | Shared code; covered by the Windows CI jobs. |
| Submission queue (in shipped builds Queue is refused — the return stays a draft; a return queued by an older version stays Queued) | ✓ | Shared code; Windows file locking for the database lock differs. |
| Database storage (plain SQLite, not encrypted) | ✓ |  |
| Account sign-in (Buwiz account, device flow) | partial | The screens' logic runs in the headless Windows CI tests (`nsh/screens/auth/gate.test.tsx`, `nsh/screens/settings/account.test.tsx`), but opening the default browser (`open::that` in `crates/bir-core/src/cloud_account.rs`) and the credential store (Windows Credential Manager via `keyring`, plus the file fallback) are unchecked on a real Windows. |
| Close button (X) and quit shortcut | untested | Code: X, Alt+F4 and Close Window (Ctrl+W) ask "Hide to tray or Quit?" by default ("Don't ask again" remembers the choice; Settings > Keyboard changes it); Ctrl+Q quits for real (`closePolicy` in `nsh/shell/desktop/menu_model.ts`). |
| System tray icon | untested | `Tray.set` is shared; the NSH Windows backend hosts it. |
| Global show/hide hotkey | untested | Win+Shift suggestion. |
| Single-instance guard | untested | Named mutex plus the loopback ping. |
| Window size and position memory | untested |  |
| Print preview | untested | In-app window showing one picture per sheet, drawn by Blitz + `vello_cpu` (`nsh/src/print.rs`, `nsh/src/sheets.rs`). |
| Export profile (Settings → Transfer) and Save as PDF (print preview) | untested | Native dialog. |
| PIN or authenticator app lock | ✓ | Shared code. |
| In-app notifications drawer and toasts | ✓ | Shared code. |
| Background refresh when the database changes | ✓ | Shared code. |
| Gmail connect for BIR receipt tracking | partial | Settings → Account → Google, or the taxpayer inbox card, in a build given a Google client (none in 1.0.0: an app password instead). The host opens the default browser — an OS call untested on real Windows. |
| Keyboard shortcuts | ✓ | Shared code; Ctrl based; the menu keymap is covered by `nsh/shell/desktop/menu_model.test.ts` in CI. |
| Store build never self-updates (Microsoft Store) | ✓ | `ebirforms-store-package` marker (`store_managed()` in `nsh/src/desktop.rs`). |
| Packaging | untested | `Setup.exe`, `.msi` and `.msix` from CI, or locally with `scripts\package_nsh_windows.ps1`. |
| Agent control (gpui-agent) | partial | A real window ran on Windows Server 2022 with the agent (docs/platforms/nsh-windows-handover.md §4.6; store screenshots on `try/store-shots-windows`, 2026-11-24); not covered by CI. |
| Agent screenshots | ✗ | macOS only. |

## Install and run

- Run the Windows release from GitHub Actions → **Release (Windows)** →
  **Run workflow**. It also runs on pull requests that touch Windows packaging
  files.
- `windows-release` contains `eBIRForms-Windows-x64-<version>-Setup.exe`
  (Inno Setup) and `eBIRForms-Windows-x64-<version>.msi` (WiX). The installer
  adds a Start menu entry and offers an optional desktop icon.
- The `.msix` is the package for the Microsoft Store. A test run puts it in `windows-release`. A release builds it
  in its own job, `build-windows-store`, as the `windows-store` artifact with `SHA256SUMS-windows-store-x64.txt`
  (`package_nsh_windows.ps1 -MsixOnly`). The direct job then leaves it out (`-NoMsix`). Neither job gets the Google
  OAuth client unless the repository variable `STORE_GMAIL` is `1` (1.0.0 ships none). Otherwise each checks its package, the `.msix` or
  the updater ZIP, holds no client id or secret (`scripts/audit_google_client.py --absent`, after its `--self-test` finds planted ones). Built without a
  signing certificate it is unsigned and keeps the Store identity from `assets\windows\AppxManifest.xml`, which is what Partner Center takes (the
  Store signs it). A run with `WINDOWS_CERTIFICATE_PFX` puts the certificate's subject in as publisher, which makes a
  sideload package that the Store refuses. The package carries `ebirforms-store-package` beside the executable, so the
  app in it never asks GitHub for a newer version (`nsh/src/desktop.rs` `store_managed`).
- The CI MSIX smoke verifies registration, packaged VC runtime files, and an
  isolated unpacked-layout launch with a visible window and 10-second process
  persistence. On GitHub's `windows-latest` runner (UAC enabled) all three
  stages run, including packaged `shell:AppsFolder` activation. On a host with
  UAC disabled (`EnableLUA=0`) activation is skipped with a warning;
  `-RequireActivation` turns that into a failure. To check activation locally,
  run `powershell.exe -NoProfile -File scripts\smoke_msix.ps1 -Package <msix> -RequireActivation`
  on Windows 11 with UAC enabled; add `-Certificate <cer>` when testing an
  unsigned package.
- To install an unsigned tester package, import its `.cer` into **Local
  Machine → Trusted People** (administrator required), then double-click the
  sideload `.msix`. An MSIX will not start on a machine with UAC disabled.
- From source: run `scripts\setup-windows.ps1`, then run the app from `nsh\`
  with `nsh run --target windows`, with the safety variables set as in
  [the Windows handover](nsh-windows-handover.md) (`just nsh-run` is the macOS
  form). Build the installers with `nsh package --target windows` and then
  `scripts\package_nsh_windows.ps1` (see [CONTRIBUTING](../../CONTRIBUTING.md)).
  The script and the `justfile` use PowerShell 7 (`pwsh`) on Windows.
- Installing an MSIX needs a signed package. `scripts\package_nsh_windows.ps1
  -SmokeMsix` signs one with a throwaway certificate for a local check; a
  release run signs with the real certificate.
- The executable is `bir.exe`.

## Data

- Database: `%LOCALAPPDATA%\dev.goldcoders.bir\bir_data.db` (the
  `directories` crate with app id `dev.goldcoders.bir`). Override the path
  with `BIR_DATABASE_PATH`.
- The database is plain SQLite; nothing is encrypted and there is no key
  to store. The Buwiz account refresh token goes to Windows Credential
  Manager through the `keyring` crate (service `com.buwiz.bir`, account
  `desktop-refresh`; see [cloud-account.md](../cloud-account.md)).
- The database owner lock opens the lock file with exclusive sharing, so a
  second process gets "database in use".
- Window size and position are saved in `window-geometry` in the data folder.

## Close, quit and the tray

- The close button, Alt+F4 and Close Window (Ctrl+W) ask "Hide to tray or
  Quit?" by default; "Don't ask again" remembers the choice (Settings >
  Keyboard). Ctrl+Q quits for real (`closePolicy` in
  `nsh/shell/desktop/menu_model.ts`).
- A quit while returns are still sending warns first: Quit anyway / Wait /
  Hide to tray.
- When hidden, the window leaves the taskbar; Show in the tray menu brings
  it back. Hiding uses Win32 `ShowWindow` on this process's windows.

## Single instance

- A named mutex (`eBIRForms_Desktop_App_Mutex_Lock`) stops a second copy. A
  loopback ping to the first copy (`instance.port`) also asks it to show its
  window. The second copy then exits.

## Global hotkey

- No hotkey is registered until you record one in Settings. The suggested
  default for a bare key is Win+Shift plus the key. Not tested.

## Print and export

- Print preview opens in the app's own window: each sheet is laid out by
  Blitz (stylo, taffy, parley) and painted by `vello_cpu` into a picture
  (`nsh/src/print.rs`, `nsh/src/sheets.rs`); no browser is involved.
- The PDF and the printout come from the web engine: a hidden WebView2
  prints with `PrintToPdf` (`nsh/src/print.rs`).
- Opening a file removes its `Zone.Identifier` stream (Mark of the Web) and
  uses backslashes, to avoid the Windows security prompt.
- Export dialogs use the native Windows dialogs: Settings → Transfer →
  Export (profile) and the print preview's Save as PDF.
- No screen offers calendar export (.ics), Google Calendar, unlock with
  Windows Hello, or printing a receipt as PDF; that code exists in the
  core crates and `nsh/src/auth.rs` but has no control in `nsh/screens` or
  `nsh/shell`.

## Agent control

- The gpui-agent token handshake works on Windows at the current pinned
  revision. Set `GPUI_AGENT_TOKEN` for secure driving.
  `gpui-agent --connect bir-shell` finds a running app, and bare commands go
  to the default address. `GPUI_AGENT_INSECURE_NO_TOKEN=1` remains for
  local-only driving; never use it on a shared machine.
- Agent screenshots are macOS only.

## Packaging

- The `Release (Windows)` workflow is defined in
  `.github/workflows/release-windows.yml`. Release runs require
  `WINDOWS_CERTIFICATE_PFX` (base64-encoded `.pfx`) and
  `WINDOWS_CERTIFICATE_PASSWORD`; pull-request and manually dispatched builds
  may be unsigned. `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET` are used only when the repository variable
  `STORE_GMAIL` is `1`; 1.0.0 leaves it unset, so no build carries them.
- `windows-release` contains only the Setup program and MSI for public GitHub
  releases. The MSIX packages are CI artifacts for testers and Store upload,
  not public release attachments.
- `scripts\package_msix.ps1` (called by `scripts\package_nsh_windows.ps1`) makes
  the MSIX from `assets\windows\AppxManifest.xml`. The MSIX
  version is `Major.Minor.Patch.0` (the caller passes `BuildNumber = [int]$patch`); see
  [Versioning](../DEVELOPMENT.md#versioning--build-numbers).
- `just bump-build` needs the App Store Connect credentials, so it works on
  macOS only. On Windows, `git pull` to get the latest `BUILD_NUMBER`.

## What needs testing

The test steps are collected in the Windows verification issue
([#90](https://github.com/hexuria/buwiz-forms/issues/90)).

## Unverified

- Everything in this page that is marked "untested" in the matrix.
- Whether the WebView2 runtime must be installed separately.
- The exact `instance.port` path on Windows.
