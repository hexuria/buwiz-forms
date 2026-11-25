# iOS tablet + stack proof — feat/ios-store-stack app (c6caa8a7, same app code as f0d8751b)

App commit: c6caa8a70d9e2a4a40a201927aa2fe62bc427a13 (branch point of this proof: f0d8751b). nsh CLI 0.6.1 (ca39c0e); ../nsh at ca39c0e9103a389a31ea280799ef34796d7aac70. Built into the per-commit evidence folder ~/BuwizNsh/target-c6caa8a7 (real rustup cargo; CARGO_INCREMENTAL=0, dev debug off). macOS 26.6.2.

Devices: iPad Pro 13-inch (M5) iOS 27.0 (ipad13), iPad Pro 11-inch (M5) iOS 26.5 (ipad11), iPad mini (A17 Pro) iOS 26.5 (ipadmini), iPhone 17 Pro iOS 26.5 (iphone). Light appearance, status bar 9:41 override, no hardware keyboard.

**Every picture and recording was driven through gpui-agent** (window plane 127.0.0.1:17421): Xcode 27/26.x simulators on this Mac accept no injected input. Rotation was done through Simulator.app menu clicks (AppleScript), which do work.

Demo data: taxpayer Juana Santos TIN 000-123-453 (self-employed, Micro, PT010, graduated rates), one draft 2551Q Q4 2026, Schedule 1 row PT010 ₱250,000 @ 3.0% = ₱7,500.00. Never queued.

## Findings
- **Row editor wedge (iPad, both orientations):** after the Schedule 1 row editor bottom sheet opens, every further gpui-agent press fails "offscreen"/"occluded" and the sheet cannot be closed; the app still re-renders on rotation, so rendering is live — input delivery into/behind the sheet layer is what breaks. Data typed before it saves fine. (Onboarding sheets — ZIP/RDO pickers — do not wedge.)
- iphone-swipe-back.mp4 and ipad13-swipe-back.mp4 are **not real edge drags**: they are gpui-agent `swipe right` gestures fed to the app's own gesture recognizers. iOS 26.5 simctl video recording is also broken on this machine (any recording yields one frame), so iphone-swipe-back.mp4 was assembled at 3 fps from rapid simctl screenshots of the same gestures — content is real, timing is not. Real edge drags must be re-done on another Mac.
- Mid-swipe frames do show the page following the gesture (interactive pop visible in both videos).

## Pictures
One line per file; all are full-screen simctl captures. ipad13 portrait/landscape home/forms/filings/settings-account/profile-sheet/tax-profile were reviewed visually: phone layout at tablet width, header with + search and taxpayer name over TIN/branch, tabs grouped centered at bottom, Recent on Home, nothing cut off, nothing under the status bar or home indicator, sheets come from the bottom width-limited. ipad11/ipadmini shots were captured with the same driving and verified as distinct non-frozen frames but not all reviewed pixel by pixel — flag anything doubtful and it can be re-checked.

- ipad13-portrait-home.png — Home upright: dashboard cards, Next 30 days, Recent with the 2551Q draft; right: layout, header, centered tabs; wrong: nothing seen.
- ipad13-portrait-forms.png — Forms tab upright; full-width list, nothing cut off.
- ipad13-portrait-filings.png — Filings upright: filter tabs with counts, the one draft card "2551Q Q4 2026"; full-width content, large empty area below (one return only).
- ipad13-portrait-calendar.png — Calendar upright.
- ipad13-portrait-settings-account.png — Settings > Account upright, pushed page with header Back.
- ipad13-portrait-profile-sheet.png — tax profile sheet open from the header taxpayer name: bottom sheet, taxpayer summary, branches, Settings/Theme/Lock/Log out rows.
- ipad13-portrait-tax-profile.png — tax profile page upright.
- ipad13-portrait-form.png — draft 2551Q Q4 2026 top, ✓ Saved, parts list.
- ipad13-portrait-form-row-editor.png — Schedule 1 row editor sheet: ₱250,000 @ 3.0% = ₱7,500.00, keyboard up; bottom sheet width-limited. This is the wedge state.
- ipad13-portrait-keyboard.png — New taxpayer (scratch db), ZIP picker sheet open, keyboard up; TIN segments render correctly.
- ipad13-landscape-*.png — the same ten sideways; content is a centered phone-width column with full-width header; home reviewed: correct.
- ipad11-*, ipadmini-* — same twenty per device (iPad Pro 11-inch M5, iPad mini A17 Pro, both iOS 26.5).
- iphone-portrait-home.png / settings-account / form / keyboard — iPhone 17 Pro upright.
- iphone-landscape-home.png — iPhone sideways Home; check the header/tab bar versus the Dynamic Island in this one.

## Recordings
- ipad13-tour.mp4 (83s) — every tab, profile sheet, Settings > Account and back, draft with its row editor; then sideways Home and the form opening (the stack's push visible). Two segments concatenated because the row-editor wedge ends navigability; uniform canvas, letterboxed landscape part.
- ipad13-swipe-back.mp4 (10s) — Settings > Account swiped back to Settings and Home; NOT a real edge drag (gpui-agent swipe).
- iphone-swipe-back.mp4 (23s) — draft opened from Filings, swipe back, reopened; Settings > Account swiped back twice; assembled from rapid screenshots (see above); NOT a real edge drag. The amount-typing part of the script was not done (the row-editor/keyboard wedge makes it unrecoverable in one session).

## Added: 44 pt targets at feat/ios-store-tablet 14dc0edf (iPhone + iPad 13)

App commit 14dc0edf05d33f0ca1f6f86c4279f0494683db50 (T). Same nsh CLI 0.6.1 (ca39c0e), same ../nsh ca39c0e9103a389a31ea280799ef34796d7aac70, cargo 1.99.0 (5f94df478 2026-08-27); built into per-commit evidence folder ~/BuwizNsh/target-14dc0edf. Same driving (gpui-agent; no injected input exists on this Mac), same scratch demo data, never queued. iPhone shots on iPhone 17 Pro iOS 26.5, iPad shot on iPad Pro 13-inch (M5) iOS 27.0, light appearance, 9:41 override, no hardware keyboard.

- iphone-home-daystrip.png — iPhone Home with the day strip: tiles in two columns, "Next 30 days" strip with 44x80 day cells (Nov 25 today → Dec 24); nothing clipped, nothing under the status bar or home indicator.
- iphone-daystrip-scroll.mp4 — the day strip dragged sideways twice through gpui-agent (~15 s, assembled from rapid simctl screenshots at 4 fps because iOS 26.5 video recording is broken on this Mac — content real, timing approximate). The strip scrolls and earlier days leave the screen left.
- iphone-form-quarter.png — the draft 2551Q's item 3 Quarter segmented control on the iPhone: 1st/2nd/3rd/4th segments at 44 pt, 4th selected (matches the Q4 draft); item 4 Amended Return segmented below; nothing overlapping the floating pill.
- ipad13-home-portrait.png — iPad 13 Home upright at T: phone layout at tablet width — header with + search and the taxpayer's name over TIN and branch, tiles in one row, day strip, Open deadlines, taxpayer-at-a-glance, Recently filed, tab bar at the bottom. Same live database as the iPhone shots (single-owner db: the iPhone app was quit first — the lock screen "eBIRForms is already open on this database" appears if both run at once, which is the intended single-owner behavior).

## Not covered
- Real left-edge drags (needs another Mac, see above).
- The tax profile tab-switch swipe-back sequence from the iphone script.
- Pixel-by-pixel review of ipad11/ipadmini shots.
- Android tablets; dark appearance.
