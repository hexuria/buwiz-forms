# Feature matrix

What works on each platform. The notes behind each cell are on the platform
pages: [macOS](platforms/macos.md), [Windows](platforms/windows.md),
[Linux](platforms/linux.md). Form coverage is in [FORMS.md](FORMS.md).

Legend: ✓ works and is covered by the platform's testing. ✗ does not exist or cannot work
on that platform. partial: works with limits that are named in the note. untested: the code is
there, but nobody has run it on that platform.

| Feature | macOS | Windows | Linux |
|---|---|---|---|
| Profiles, tax form library, editors, Check, Save draft | ✓ | ✓ | ✓ |
| Submission queue (in shipped builds Queue is refused — the return stays a draft; a return queued by an older version stays Queued) | ✓ | ✓ | ✓ |
| Database storage (plain SQLite, not encrypted) | ✓ | ✓ | ✓ |
| Account sign-in (Buwiz account, device flow in the browser) | ✓ | partial | ✓ |
| Close button (X) and quit shortcut | ✓ | untested | ✓ |
| System tray icon | ✓ | untested | partial |
| App menu launcher, icons and dock grouping (app id `dev.goldcoders.bir`) | n/a | n/a | ✓ |
| Global show/hide hotkey | ✓ | untested | partial |
| Single-instance guard | ✓ | untested | ✓ |
| Window size and position memory | ✓ | untested | ✓ |
| Print preview | ✓ | untested | partial |
| Export profile (Settings → Transfer) and Save as PDF (print preview) | ✓ | untested | partial |
| PIN or authenticator app lock | ✓ | ✓ | ✓ |
| In-app notifications drawer and toasts | ✓ | ✓ | ✓ |
| Background refresh when the database changes | ✓ | ✓ | ✓ |
| Gmail connect for BIR receipt tracking (Settings → Account → Google; host opens the browser) | ✓ | partial | ✓ |
| Keyboard shortcuts | ✓ | ✓ | ✓ |
| Store builds never self-update (Mac App Store, Microsoft Store) | ✓ | ✓ | n/a |
| Packaging | ✓ | untested | ✓ |
| Agent control (gpui-agent) | ✓ | partial | ✓ |
| Agent screenshots | ✓ | ✗ | ✗ |

## Reading this table

- macOS and Linux are fully tested. Windows shared features are ✓ when the
  CI covers them: the `NSH app (Windows)` job in
  [.github/workflows/nsh.yml](../.github/workflows/nsh.yml) runs `nsh check`,
  `nsh build`, the Rust crate tests and the headless TSX tests on
  `windows-2025`, and `check` in
  [.github/workflows/ci.yml](../.github/workflows/ci.yml) runs
  `cargo test --locked --workspace` on `windows-latest`. They run nightly and
  when a `test/...` push or a manual run asks for Windows, not on every pull
  request ([CI.md](CI.md)). Those jobs exercise
  the shared Rust and TypeScript code, not a real Windows window, so every
  Windows cell that needs a person looking at a desktop stays "untested".
- The macOS cells were checked against the running app on this commit's
  ancestor line (a dev host window with a scratch database), including the
  menus, the editor header and the print preview window.
- The system tray row: on macOS the app sets an `NSStatusItem`
  (`nsh/shell/desktop/Desktop.tsx`, `Tray.set` → `nsh-platform-macos/src/tray.rs`).
  Hide and Close keep the app running in the menu bar without a Dock icon;
  a click on the menu-bar icon brings the window back and a secondary
  click opens the Show / Hide / Quit menu (accessibility-press check on a
  real Mac, 2026-11-24). See [macos.md](platforms/macos.md).
- The table was re-derived from `main`: the retired JS desktop shell and
  its crate were deleted; the app is the NSH app in `nsh/`.
- Features whose code exists but which no screen offers were removed from
  this table: adding deadlines to a calendar (the .ics builder,
  `crates/bir-core/src/calendar_ics.rs`, is not called from `nsh/screens`
  or `nsh/shell`), Google Calendar credentials storage, unlocking with the
  OS (Touch ID / Windows Hello — `nsh/src/auth.rs` `os_authenticate` has no
  caller), and printing an email receipt as PDF (the receipt code in
  `crates/bir-core/src/email/` is not reached from a screen).

## Features that are the same everywhere

These have no platform code and behave the same in the shared Rust and
TypeScript code:

- The 43 forms, their validation, XML output and the submission queue.
- The statuses Draft, Queued, Submitted, Confirmed and Paid, and Revert to Draft.
- Profiles, the PIN and authenticator-app lock, the calendar and the
  notifications drawer.
- The light and dark theme (Cmd/Ctrl+T).
- Dry-run mode for development (`BIR_SFTP_DRY_RUN`). Live filing is not in
  any shipped build: the `direct-bir` cargo feature that would send to the
  BIR itself is never enabled in them.
- The frozen printed sheets.

## Security and privacy

- **Offline first.** Profiles, TINs and drafts are stored in a plain SQLite
  database on your machine; it is not encrypted. See each platform page for
  the path, and [PRIVACY_POLICY.md](../PRIVACY_POLICY.md) for what is stored
  and sent.
- **App lock.** A 4-digit PIN or an authenticator app (Settings → Account →
  Security). OS unlock (Touch ID / Windows Hello) exists in
  `nsh/src/auth.rs` but no screen calls it, so the app does not offer it.
- **Profile snapshot at save time.** A saved form keeps the profile data as
  it was when you saved. Changing the profile later does not change the
  saved form.
- **Post-submission lock.** Once a return is submitted its fields are
  read-only. Revert to Draft to change it.

## Automation

- **Background engine.** A submission worker runs inside the app and keeps
  the queue. No separate daemon is needed. In shipped builds it cannot
  send to the BIR itself.
- **Receipt tracking.** Reading your inbox for BIR receipts moves returns
  from Submitted to Confirmed and Paid.
- **Filters.** The library and calendar filter by period (Q1 to Q4,
  monthly, yearly) and by status.
