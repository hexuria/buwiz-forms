# eBIRForms on macOS

Status: fully tested. Back to the [README](../../README.md). The same matrix
for all platforms is in [FEATURES.md](../FEATURES.md).

## Feature matrix

Legend: ✓ works and is covered by the platform's testing. ✗ does not exist or cannot work
on that platform. partial: works with limits that are named in the note. untested: the code is
there, but nobody has run it on that platform.

| Feature | Status | Notes |
|---|---|---|
| Profiles, tax form library, editors, Check, Save draft | ✓ | All 43 forms. |
| Submission queue (in shipped builds Queue is refused — the return stays a draft; a return queued by an older version stays Queued) | ✓ |  |
| Database storage (plain SQLite, not encrypted) | ✓ |  |
| Close button (X) and quit shortcut | ✓ | X and Cmd+W hide the window; Cmd+Q quits for real. |
| System tray icon | ✓ | The `NSStatusItem` click handler restores the window; a secondary click opens the Show / Hide / Quit menu (hands-on check, 2026-11-24). |
| Global show/hide hotkey | ✓ | Default Ctrl+Option+E suggestion. Off in Mac App Store builds. |
| Single-instance guard | ✓ | The app bundle plus a loopback ping (`instance.port`). |
| Window size and position memory | ✓ |  |
| Print preview | ✓ | Own window with a Print button (native print dialog). |
| Export profile (Settings → Transfer) and Save as PDF (print preview) | ✓ | Native save dialog (`pick_save_path`). |
| PIN or authenticator app lock | ✓ |  |
| In-app notifications drawer and toasts | ✓ | The bell in the top bar (⌘⇧N); in-app toasts. |
| Background refresh when the database changes | ✓ | The paced watch in `nsh/shell/app.ts` re-reads. |
| Gmail connect for BIR receipt tracking | ✓ | Settings → Account → Google, or the taxpayer inbox card, in a build given a Google client (none in 1.0.0: an app password instead); the host opens the browser. |
| Keyboard shortcuts | ✓ | Cmd based; also Cmd+H, Cmd+M, Cmd+Ctrl+F. |
| Packaging | ✓ | Signed and notarized `.app` and `.dmg`; Mac App Store package. |
| Agent control (gpui-agent) | ✓ | Opt-in build feature. |
| Agent screenshots | ✓ | `screencapture`; needs Screen Recording permission. |


## Install and run

- Download the `.dmg` from the
  [Releases](https://github.com/hexuria/buwiz-forms/releases) page and drag
  `eBIRForms.app` to Applications.
- From source: run `scripts/setup-macos.sh`, then `just run` (a development
  window on a scratch database). `just _package-mac` builds the universal
  `.app` and DMG (it runs `scripts/package_macos_release.sh`); `just app`
  builds the Mac App Store `.pkg` (it resolves the next App Store build
  number, then runs `just _package-mac-appstore` with `mas_build`); and
  `just testflight` is `just app` plus the upload.

## Data

- Database: `~/Library/Application Support/dev.goldcoders.bir/bir_data.db`
  (sandboxed Mac App Store builds use the group container
  `~/Library/Group Containers/group.dev.goldcoders.bir/`).
  Override the path with `BIR_DATABASE_PATH`.
- The database is plain SQLite; nothing is encrypted and there is no key
  to store. The Buwiz account refresh token is the only keychain item
  (service `com.buwiz.bir`, account `desktop-refresh`; see
  [cloud-account.md](../cloud-account.md)).
- Window size and position are saved in `window-geometry` in the data folder.
- `instance.port` for the single-instance ping is in the user's local data
  folder (chosen by the `directories` crate; exact path unverified).

## Close, quit and the menu bar

- The close button and Cmd+W hide the window. The app keeps running so the
  submission worker can poll for BIR receipts.
- Cmd+Q and the app menu's Quit item quit for real
  (`nsh/shell/desktop/Desktop.tsx`).
- While hidden or closed the app lives on as a menu-bar item only — the
  Dock icon goes away (`nsh/shell/desktop/Desktop.tsx`, `Tray.set` →
  `nsh-platform-macos/src/tray.rs`). A click on the menu-bar icon brings
  the window back (verified 2026-11-24 by an accessibility `AXPress` on
  the status item after ⌘H: window count 0 → 1), and a right or
  Control-click opens the Show / Hide / Quit menu.
- Launching the app again brings the window back (single instance).
- Cmd+H hides the app, Cmd+Alt+H hides other apps, Cmd+M minimizes and
  Cmd+Ctrl+F toggles full screen.

## Global hotkey

- No hotkey is registered until you record one in Settings. The suggested
  default for a bare key is Ctrl+Option plus the key.
- Mac App Store builds (`--features mas_build`) leave the hotkey out. The
  sandbox forbids the entitlements it needs.

## Print and export

- Print preview opens in its own window (verified 1440x900 resizable) with
  a Print button. If the preview window cannot open, the app writes the
  HTML to a temporary file and opens it in the default browser.
- Export dialogs are always the native macOS dialogs.
- No screen offers calendar export (.ics), Google Calendar, unlock with
  Touch ID, or printing a receipt as PDF; that code exists in the core
  crates and `nsh/src/auth.rs` but has no control in `nsh/screens` or
  `nsh/shell`.

## Notifications

- Alerts and receipts appear in the app's notifications drawer
  (the bell in the top bar; ⌘⇧N). Delivery is in-app; there is no OS
  notification-banner code in `nsh/`.
- The headless `bir` daemon posts banners with `osascript`.

## Packaging

- The release workflow builds the NSH app for arm64 and x86_64, joins them
  with `lipo`, makes the `.app` — `Contents/MacOS/bir` plus `nsh/`,
  `print-overrides/` and `assets/` under `Contents/Resources` — signs it,
  builds a DMG, notarizes it with `notarytool` and checks it with `spctl`.
  It needs the Apple secrets listed in
  [DEVELOPMENT.md](../DEVELOPMENT.md#cicd-secrets--codesigning).
- The sandboxed Mac App Store package is built by `just app` (which runs
  `just _package-mac-appstore`) on the owner's Mac; no workflow builds it.
  It adds the app's `mas_build` feature (no global hotkey or self-updater)
  to the generated host's build, embeds
  `assets/macos/embedded.provisionprofile`, signs with the store
  entitlements that `scripts/store_gmail.py entitlements` writes from
  `entitlements.plist` (sandbox), adds `print-overrides/` and the privacy
  manifest under `Contents/Resources` and wraps the app with
  `productbuild`. A store build never offers self-updates
  (`store_managed()` in `nsh/src/desktop.rs`).
- **Gmail Connect is out of the store builds unless `STORE_GMAIL=1`**
  (`just app` and `just ios-appstore`; scripts/store_gmail.py). Connect
  asks Google for the restricted `https://mail.google.com/` scope, which a
  public app may use only after Google verifies the consent screen. Without
  the switch the recipes drop `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET`
  from the build, a check after packaging fails the build if the app holds
  either (with the switch on it fails if the app lacks either), and the Mac
  App Store package has no `com.apple.security.network.server` (kept only
  for Connect's one-shot 127.0.0.1 OAuth listener or an `--agent` build).
  The app then offers no Connect and says to use an app password. The
  direct downloads (`release-macos.yml`, `package_macos_release.sh`) are
  not affected.
- Entitlement files are in `assets/macos/` and at the repository root.

## Agent control

- The gpui-agent build feature is macOS-capable end to end, including
  screenshots (`screencapture -l`). The terminal needs Screen Recording
  permission for that exact binary path. See
  [agent.md](../agent.md).

## Unverified

- The exact `instance.port` path on macOS.
- Whether the `bir` headless daemon still posts `osascript` banners.
- Whether real-mouse clicks (not accessibility actions) reach the
  menu-bar item on remote/dev machines — a raw mouse click can be
  swallowed by the remote desktop while `AXPress` works (2026-11-24).
