# Android form-picker row tap proof — PR #350 at 4ac7688f

## Build header

- App commit: `4ac7688fa6d05c7974605dda893f4d84fcac3ff2` (PR #350 exactly; not main)
- Sibling NSH commit: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (clean checkout beside the app)
- Tool / runtime: `~/BuwizNsh/tools` — real copies at pin 8c78cfd6, `nsh 0.6.1 (8c78cfd)`; never moved
- classes.dex sha256 (runtime used): `73b3126db70bdca7e03f0ee3acb39c63437efc1f606c7232d0324445a1788e17`
- CARGO_TARGET_DIR: `~/BuwizNsh/target-4ac7688f` (this build only)
- Device: emulator-5554, API 35, gesture navigation, density 420. Safety env: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1 BIR_AUTH_BYPASS=1`
- Taps: 150 ms hold-taps (`input swipe x y x y 150`). Drags: real 600 ms swipes.

## Widths

- **Phone**: default emulator size 1080x2400 (411 dp wide — compact layout).
- **Tablet**: `wm size 1600x2560` (override, density unchanged 420 → 609 dp wide — non-compact layout), reset after. No tablet AVD was used.

## Results — every item WORKS

### 1. Tap each part of a row (row 0605) — phone and tablet

| tap target | phone result | tablet result |
|---|---|---|
| text ("Payment Form") | toggled exactly once (0→1 selected; box ticked, "No template" chip appeared) | toggled exactly once (ON→cleared) |
| form code ("0605") | toggled exactly once (cleared) | toggled exactly once (selected) |
| empty part of the row | toggled exactly once (selected) | toggled exactly once (selected) |
| the row's chip ("No template") | toggled exactly once (cleared) | toggled exactly once (cleared) |
| the small box itself | still works — toggled exactly once (selected) | still works — toggled exactly once (cleared) |

Before/after per tap: `phone-0-baseline.png` → `phone-1-after-text.png` → … → `phone-5-after-box.png`; tablet: `tablet-0-baseline.png` → `tablet-1-after-text.png` → … → `tablet-5-after-box.png` (the sequence on tablet started with the row already on, so each step is the inverse — every step shows exactly one toggle of the same row).

### 2. Drags never select — phone and tablet

Five 600 ms drags, each starting on a row: **0 of 5 selected anything** at each width. The list scrolled every time (`phone-drag1.png` shows rows 0619F–1601C after starting at 0605/0619E; `tablet-drag2.png` shows the list down at 1600VT–1604C). After all five, the counter still read the pre-drag value: phone "1 selected" (`phone-after-5-drags.png`), tablet "3 selected" (`tablet-after-5-drags.png`), with exactly the same rows ticked.

### 3. Save, reopen, stored set exact — phone and tablet

Set {0605, 0619E, 0619F} saved ("2026 forms updated: 3 added, 0 removed", Forms badge 3 — `phone-post-save.png`, `tablet-post-save.png`). Reopened the picker: "3 selected" and exactly those three rows ticked, nothing else (`phone-reopen.png`, `tablet-reopen.png`). PASS at both widths.

## Recordings

- `picker-phone.mp4` — phone width, whole session from onboarding through the tap matrix, drags, save and reopen. Show taps on (pointer-location overlay visible at top).
- `picker-tablet.mp4` — tablet width (wm size 1600x2560), same coverage.

## Notes

- The "chip" tested is the row's trailing Template/No-template chip (present once the row is on). At phone width (411 dp, compact) rows have no frequency chip column — it only exists in the non-compact layout per `FormsTab.tsx`; the tablet width runs non-compact and the same chip was tapped there.
- No step failed; nothing needed a re-run. One harness note, not an app defect: on phone the first recording expired mid-onboarding and was restarted before the first measured tap; `picker-phone.mp4` covers every measured step.
