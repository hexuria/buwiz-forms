# eBIRForms on Windows

Status: not yet verified. Tracked in [#90](https://github.com/hexuria/buwiz-forms/issues/90). Back to the
[README](../../README.md). The same matrix for all platforms is in
[FEATURES.md](../FEATURES.md).

Every item below describes what the code is written to do. Nobody has run it
on Windows. "Untested" in the matrix means exactly that.

## Feature matrix

Legend: ✓ works and is covered by the platform's testing. ✗ does not exist or cannot work
on that platform. partial: works with limits that are named in the note. untested: the code is
there, but nobody has run it on that platform.

| Feature | Status | Notes |
|---|---|---|
| Profiles, tax form library, editors, Validate, Save draft | untested | Same code as the other platforms. |
| Submission queue and dry run (SFTP upload, IMAP receipts) | untested | Same code; Windows file locking for the database lock differs. |
| Encrypted database (SQLCipher) | untested |  |
| Database key storage | untested | Windows Credential Manager, with a key-file fallback. Never run. |
| Close button (X) and quit shortcut | untested | X and Alt+F4 hide the app to the tray. There is no Ctrl+Q binding. |
| System tray icon | untested | Same code through the `tray-icon` crate. |
| Global show/hide hotkey | untested | Win+Shift+E suggestion. |
| Single-instance guard | untested | Named mutex plus the loopback ping. |
| Window size and position memory | untested |  |
| Print preview | untested | Own window (WebView2) with a Print button. |
| Print an email receipt as PDF | untested | Windows `print` verb; opens the file if that fails. |
| Export dialogs (save file, choose folder) | untested | Native dialog. |
| Unlock with the OS (Touch ID, Windows Hello) | untested | Windows Hello, through robius-authentication. |
| PIN or authenticator app lock | untested |  |
| Desktop notifications | untested | In-app toast. System banner: unverified. |
| Instant refresh when the database changes | untested | 1 s poll. |
| Add deadlines to a calendar (.ics) | untested | Opens in the default calendar app. |
| Google Calendar credentials storage | untested | Credential Manager, through `keyring`. |
| Keyboard shortcuts | untested | Ctrl based; also F11 and Win+M. |
| Packaging | untested | `Setup.exe` and `.msi` from CI; MSIX with `just msix`. |
| Agent control (gpui-agent) | ✓ | Token-authenticated control works at the current pinned revision. |
| Agent screenshots | ✗ | macOS only. |
| Dev Mode panel (debug builds only) | untested |  |

## Install and run

- The release workflow builds `eBIRForms-Windows-x64-<version>-Setup.exe`
  (Inno Setup) and `eBIRForms-Windows-x64-<version>.msi` (WiX). Run either. The
  installer adds a Start menu entry and offers an optional desktop icon.
- From source: run `scripts\setup-windows.ps1`, then `just run`. Build the
  installers with `just exe` or `just msix`. The `justfile` uses PowerShell 7
  (`pwsh`) on Windows.
- Installing an MSIX needs a signed package. `just sign-dev` signs one for
  local use.
- The executable is `bir.exe`.

## Data

- Database: `%LOCALAPPDATA%\Taxman\eBIRForms\bir_data.db`. If `LOCALAPPDATA`
  is not set, `%USERPROFILE%\.taxman-ebir`. Override the path with
  `BIR_DATABASE_PATH`.
- The SQLCipher key goes to Windows Credential Manager through the `keyring`
  crate (`windows-native` backend). If the store cannot be used, the key is
  written to `bir_key.txt` in the data folder. If the store holds a key but
  cannot read it, the app refuses to make a new one.
- The database owner lock opens the lock file with exclusive sharing, so a
  second process gets "database in use".
- Window size and position are saved in `window-geometry` in the data folder.

## Close, quit and the tray

- The close button and Alt+F4 hide the window to the tray. The background
  engine keeps running. Quit from the tray menu.
- There is no Ctrl+Q binding. Ctrl+W also hides the window. Win+M minimizes.
  F11 toggles full screen.
- Hiding uses Win32 `ShowWindow` on this process's windows.

## Single instance

- A named mutex (`eBIRForms_Desktop_App_Mutex_Lock`) stops a second copy. A
  loopback ping to the first copy (`instance.port`) also asks it to show its
  window. The second copy then exits.

## Global hotkey

- No hotkey is registered until you record one in Settings. The suggested
  default for a bare key is Win+Shift plus the key. Not tested.

## Print and export

- Print preview opens in its own window that uses WebView2 (through `wry`),
  with a Print button that calls the WebView2 print UI. If the window cannot
  open, the HTML is written to a temp file and opened in the default browser.
  WebView2 runtime requirement: unverified.
- Receipt PDFs print with the Windows `print` verb. If the default PDF viewer
  does not support it, the file is opened and a message tells you to press
  Ctrl+P.
- Opening a file removes its `Zone.Identifier` stream (Mark of the Web) and
  uses backslashes, to avoid the Windows security prompt.
- Export dialogs use the native Windows dialogs.

## Security

- Windows Hello unlock uses `robius-authentication`. After it returns, the app
  releases a stuck Alt key and takes back keyboard focus
  (`reclaim_keyboard_focus`), because the Hello dialog can leave Alt pressed.
  That workaround is in the code because of a known library bug; whether it is
  enough has not been checked.

## Agent control

- The gpui-agent token handshake works on Windows at the current pinned
  revision. Set `GPUI_AGENT_TOKEN` for secure driving. The CLI install needs
  `scripts\setup-windows.ps1` or `-l bcrypt` until the #91 upstream fix is
  pinned. `GPUI_AGENT_INSECURE_NO_TOKEN=1` remains for local-only driving; never
  use it on a shared machine.
- Agent screenshots are macOS only.

## Packaging

- CI signs `bir.exe`, the Setup program and the MSI. The workflow stops if the
  signing inputs are missing.
- `just msix` makes an MSIX from `assets\windows\AppxManifest.xml`. The MSIX
  version is `Major.Minor.BuildNumber.0`; see
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
