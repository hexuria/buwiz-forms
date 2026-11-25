# iOS shell proof: one Back on phone Settings, sheets below the status bar

Proof pictures and a recording for branch `feat/ios-store-shell` at `0043bc5b` (two commits on main `6bb90ad1`: `46a960b5` one Back control on a phone's Settings pages; `0043bc5b` a sheet stays below the status bar when the keyboard is up). BEFORE is main `6bb90ad1`, AFTER is `0043bc5b`. Capture only: no app code changed.

Devices: iPhone 17 Pro, iOS 26.5 (1206x2622); iPad Pro 13-inch (M5), iOS 26.5 (2064x2752 portrait, 2752x2064 landscape), Settings > Multitasking & Gestures = Full Screen Apps. Light appearance, status bar overridden to 9:41 / charged / full signal, hardware keyboard off so the on-screen keyboard shows. Safety: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1` (with `SIMCTL_CHILD_` prefixes) on every run; fresh scratch database; the demo taxpayer Juana Santos, TIN 000-123-453; nothing queued, no return created.

## Files

iPhone, before/after pairs:
- `before-iphone-settings-list.png` / `after-iphone-settings-list.png` — the Settings list from the avatar menu. Same in both.
- `before-iphone-settings-account.png` / `after-iphone-settings-account.png` — Settings > Account. BEFORE: the header Back arrow and title "Settings" AND a second "< Settings" line above a second big title "Account" (two Back controls). AFTER: one Back arrow in the header, the header reads "Account", no second Back line, no second title.
- `before-iphone-settings-general.png` / `after-iphone-settings-general.png` — same for General.
- `before-iphone-settings-device.png` / `after-iphone-settings-device.png` — same for Device.
- `after-iphone-settings-back-to-list.png` — after pressing the header Back on Account: the Settings list again. A second Back leaves Settings for the page you came from (Home).
- `before-iphone-zip-picker-keyboard.png` / `after-iphone-zip-picker-keyboard.png` — the ZIP picker in New taxpayer (Add branch) with "10" typed and the keyboard up. BEFORE: the sheet runs under the status bar and the Dynamic Island; the search field is hidden behind them, rows reach the top edge behind the status bar text. AFTER: the whole sheet — title, search field with "10", and rows — is below the status bar and bounded above the keyboard.
- `before-iphone-rdo-picker-keyboard.png` / `after-iphone-rdo-picker-keyboard.png` — the same for the RDO picker (AFTER has "10" typed, mirroring the ZIP shot). BEFORE: rows behind the status bar, search hidden. AFTER: everything below the status bar.
- `before-iphone-profile-sheet.png` / `after-iphone-profile-sheet.png` — the Tax profile sheet from the avatar menu. No keyboard: the same in both, bottom edge clear of the home indicator.
- `before-iphone-notifications-sheet.png` / `after-iphone-notifications-sheet.png` — the Notifications sheet. Same in both.

iPad (wide layout: the list beside the cards):
- `before-ipad-settings-account-portrait.png` / `after-ipad-settings-account-portrait.png` — Settings > Account, portrait. Same before and after, as expected: the wide layout was never doubled.
- `before-ipad-settings-account-landscape.png` / `after-ipad-settings-account-landscape.png` — the same in landscape.
- `before-ipad-zip-picker-keyboard.png` / `after-ipad-zip-picker-keyboard.png` — the ZIP picker in Add branch with the on-screen keyboard up. BEFORE: the popover list runs past the bottom edge of the screen behind the keyboard. AFTER: the list is bounded above the keyboard, the search field visible at its top.

Recording:
- `after-settings-and-sheets.mp4` (h264, 58 s, iPhone 17 Pro) — Home, avatar menu > Settings, open Account (one Back control, "Account" in the header), Back to the list, open General, Back, Back again to Home; then the taxpayer sheet > Add branch, the ZIP picker opened with the keyboard up, "10" typed, row 1100 picked, closed without creating anything.

## What this proof does not cover

Android, desktop layouts, dark appearance, the behavior of Forward/Android-back mentioned in commit 46a960b5, and every other picker beyond ZIP and RDO (the fix is shared in `nsh/ui`, but only those two are pictured). The "10" in the RDO after-shot is typed input for parity with the ZIP shot, not a requirement. No return was created and nothing was queued or sent.
