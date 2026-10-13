# Feature matrix

What works on each platform. The notes behind each cell are on the platform
pages: [macOS](platforms/macos.md), [Windows](platforms/windows.md),
[Linux](platforms/linux.md). Form coverage is in [FORMS.md](FORMS.md).

Legend: ✓ works and is covered by the platform's testing. ✗ does not exist or cannot work
on that platform. partial: works with limits that are named in the note. untested: the code is
there, but nobody has run it on that platform.

| Feature | macOS | Windows | Linux |
|---|---|---|---|
| Profiles, tax form library, editors, Validate, Save draft | ✓ | untested | ✓ |
| Submission queue and dry run (SFTP upload, IMAP receipts) | ✓ | untested | ✓ |
| Encrypted database (SQLCipher) | ✓ | untested | ✓ |
| Database key storage | ✓ | untested | partial |
| Close button (X) and quit shortcut | ✓ | untested | ✓ |
| System tray icon | ✓ | untested | partial |
| App menu launcher, icons and dock grouping (app id `dev.goldcoders.bir`) | n/a | n/a | ✓ |
| Global show/hide hotkey | ✓ | untested | partial |
| Single-instance guard | ✓ | untested | ✓ |
| Window size and position memory | ✓ | untested | ✓ |
| Print preview | ✓ | untested | partial |
| Print an email receipt as PDF | ✓ | untested | ✓ |
| Export dialogs (save file, choose folder) | ✓ | untested | partial |
| Unlock with the OS (Touch ID, Windows Hello) | ✓ | untested | ✗ |
| PIN or authenticator app lock | ✓ | untested | ✓ |
| Desktop notifications | ✓ | untested | untested |
| Instant refresh when the database changes | ✓ | untested | ✓ |
| Add deadlines to a calendar (.ics) | ✓ | untested | ✓ |
| Google Calendar credentials storage | ✓ | untested | untested |
| Keyboard shortcuts | ✓ | untested | ✓ |
| Packaging | ✓ | untested | ✓ |
| Agent control (gpui-agent) | ✓ | partial | ✓ |
| Agent screenshots | ✓ | ✗ | ✗ |
| Dev Mode panel (debug builds only) | ✓ | untested | ✓ |

## Reading this table

- macOS and Linux are fully tested. Windows has partial evidence from one
  Windows Server 2022 VM ([#90 comment 6119512665](https://github.com/hexuria/buwiz-forms/issues/90#issuecomment-6119512665));
  this is not full certification, and all Windows cells without specific
  evidence remain "untested".
- The table was derived from the code on branch `forms/integration` (commit
  `fe86f57d`): `cfg(target_os)` blocks, `crates/bir-desktop/src/platform/`,
  `gui.rs`, `export_target.rs`, `crates/bir-core/src/db/mod.rs` and the
  packaging files. It was not produced by running the app on each platform.
- The same data drives the matrix at the top of each platform page.

## Features that are the same everywhere

These have no platform code and behave the same in the shared Rust code:

- The 43 forms, their validation, XML output and the submission queue.
- The statuses Draft, Queued, Submitted, Confirmed and Paid, and Revert to Draft.
- Profiles, the PIN and authenticator-app lock, the calendar and the
  notifications drawer.
- The light and dark theme (Cmd/Ctrl+Shift+T).
- Dry-run and live sending (`BIR_SFTP_DRY_RUN`, `BIR_SFTP_LIVE`).
- The frozen printed sheets.

## Security and privacy

- **Offline first.** Profiles, TINs and drafts are stored in an encrypted SQLite
  (SQLCipher) database on your machine. See each platform page for the path.
- **App lock.** A 4-digit PIN or an authenticator app. On macOS and Windows the
  lock screen can also use Touch ID or Windows Hello.
- **Profile snapshot at save time.** A saved form keeps the profile data as it
  was when you saved. Changing the profile later does not change the saved form.
- **Post-submission lock.** Once a return is submitted its fields are read-only.
  Revert to Draft to change it.

## Automation

- **Background engine.** A cron engine runs on a dedicated thread inside the
  app. No separate daemon is needed.
- **Receipt tracking.** IMAP fetching scans your inbox for BIR receipts and
  moves returns from Submitted to Confirmed and Paid.
- **Filters.** The dashboard filters by period (Q1 to Q4, monthly, yearly) and
  by status.
