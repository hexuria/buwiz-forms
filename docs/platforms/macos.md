# eBIRForms on macOS

Status: fully tested. Back to the [README](../../README.md). The same matrix
for all platforms is in [FEATURES.md](../FEATURES.md).

## Feature matrix

Legend: ✓ works and is covered by the platform's testing. ✗ does not exist or cannot work
on that platform. partial: works with limits that are named in the note. untested: the code is
there, but nobody has run it on that platform.

| Feature | Status | Notes |
|---|---|---|
| Profiles, tax form library, editors, Validate, Save draft | ✓ | All 43 forms. |
| Submission queue and dry run (SFTP upload, IMAP receipts) | ✓ |  |
| Encrypted database (SQLCipher) | ✓ |  |
| Database key storage | ✓ | macOS Keychain, through the `security` tool. |
| Close button (X) and quit shortcut | ✓ | X and Cmd+Q hide the app to the menu bar. Quit from the tray menu. |
| System tray icon | ✓ | Menu bar icon with Show, Hide and Quit. |
| Global show/hide hotkey | ✓ | Default Ctrl+Option+E suggestion. Off in Mac App Store builds. |
| Single-instance guard | ✓ | The app bundle plus a loopback ping (`instance.port`). |
| Window size and position memory | ✓ |  |
| Print preview | ✓ | Own window with a Print button (native print dialog). |
| Print an email receipt as PDF | ✓ | AppKit through a Swift helper; falls back to opening the file. |
| Export dialogs (save file, choose folder) | ✓ | Native dialog. |
| Unlock with the OS (Touch ID, Windows Hello) | ✓ | Touch ID, through robius-authentication. |
| PIN or authenticator app lock | ✓ |  |
| Desktop notifications | ✓ | System banner through GPUI in a `.app`; in-app toast otherwise. |
| Instant refresh when the database changes | ✓ | Distributed notification plus a 5 s poll. |
| Add deadlines to a calendar (.ics) | ✓ | Opens in Calendar. |
| Google Calendar credentials storage | ✓ | Keychain, through the `security` tool. |
| Keyboard shortcuts | ✓ | Cmd based; also Cmd+H, Cmd+M, Cmd+Ctrl+F. |
| Packaging | ✓ | Signed and notarized `.app` and `.dmg`; Mac App Store package. |
| Agent control (gpui-agent) | ✓ | Opt-in build feature. |
| Agent screenshots | ✓ | `screencapture`; needs Screen Recording permission. |
| Dev Mode panel (debug builds only) | ✓ |  |

## Install and run

- Download the `.dmg` from the
  [Releases](https://github.com/hexuria/buwiz-forms/releases) page and drag
  `eBIRForms.app` to Applications.
- From source: run `scripts/setup-macos.sh`, then `just run`. Build a universal
  (arm64 and x86_64) `.app` and DMG with `just _package-mac`.
- Without `create-dmg`, `just _package-mac` makes a `.zip` instead.

## Data

- Database: `~/Library/Group Containers/group.dev.goldcoders.bir/bir_data.db`.
  Override the path with `BIR_DATABASE_PATH`.
- The SQLCipher key is in the Keychain (service `com.ebir.rust`, account
  `sqlcipher_master_key`). The app reads it with the `security` tool, not the
  Keychain framework, because the framework call can deadlock the GPUI main
  loop. If the Keychain read fails for any reason other than "not found", the
  app refuses to make a new key. A new key would make the old database
  unreadable.
- Window size and position are saved in `window-geometry` in the data folder.
- `instance.port` for the single-instance ping is in the user's local data
  folder (chosen by the `directories` crate; exact path unverified).

## Close, quit and the menu bar

- The close button and Cmd+W hide the window. Cmd+Q and the app menu's
  "Close to Menu Bar" item do the same. The app keeps running so the
  background engine can poll for BIR receipts.
- To quit for real, use Quit in the tray (menu bar) menu.
- Clicking the Dock icon, or launching the app again, brings the window back.
- Cmd+H hides the app, Cmd+Alt+H hides other apps, Cmd+M minimizes and
  Cmd+Ctrl+F toggles full screen.

## Global hotkey

- No hotkey is registered until you record one in Settings. The suggested
  default for a bare key is Ctrl+Option plus the key.
- Mac App Store builds (`--features mas_build`) leave the hotkey out. The
  sandbox forbids the entitlements it needs.

## Print and export

- Print preview opens in its own 1200 by 900 window with a Print button. The
  button uses the native print operation, with A4 paper and zero margins. If
  the preview window cannot open, the app writes the HTML to a temporary file
  and opens it in the default browser.
- Receipt PDFs print through AppKit with a Swift helper. If the Swift runtime
  is not there, the file is opened instead.
- Export dialogs are always the native macOS dialogs.

## Security

- Touch ID unlock uses `robius-authentication`. If biometrics fail or are
  cancelled, the lock screen falls back to the PIN.

## Notifications

- The app posts system banners through GPUI's `UNUserNotificationCenter`
  support when it runs from a `.app`. A bare `cargo run` shows only the in-app
  toast.
- `bir-headless` (the background daemon) posts banners with `osascript`.
- Database changes made by the daemon reach the window at once through a
  distributed notification. A 5 second poll is the fallback.

## Packaging

- The release workflow builds `bir-shell` for arm64 and x86_64, joins them
  with `lipo`, makes the `.app` — `Contents/MacOS/bir` plus `app/`,
  `print-overrides/` and `assets/` under `Contents/Resources` — signs it,
  builds a DMG, notarizes it with `notarytool` and checks it with `spctl`.
  It needs the Apple secrets listed in
  [DEVELOPMENT.md](../DEVELOPMENT.md#cicd-secrets--codesigning).
- `just app` still builds the sandboxed Mac App Store package from the painted
  `bir-desktop` (feature `mas_build`): the shell has no `mas_build` equivalent
  or sandboxed JIT entitlement story yet (docs/plans/gpui-shell-migration.md,
  S7). It needs App Store Connect credentials for the build number.
- Entitlement files are in `assets/macos/` and at the repository root.

## Agent control

- The gpui-agent build feature is macOS-capable end to end, including
  screenshots (`screencapture -l`). The terminal needs Screen Recording
  permission for that exact binary path. See
  [AGENT.md](../../crates/bir-desktop/docs/AGENT.md).

## Unverified

- The exact `instance.port` path on macOS.
- That `just _package-mac` output is byte-identical to the CI release build.
