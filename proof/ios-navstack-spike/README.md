# iOS NavStack spike proof — try/ios-navstack-spike @ 0179c620

Device: iPhone 17 (6.3-inch), iOS 27.0 Simulator, light appearance, status bar overridden (9:41, full battery/signal). macOS 26.6.2 arm64. App launched via `just nsh-run-agent ios` with the four safety variables; scratch database; demo taxpayer Juana Santos only. All gestures were REAL mouse drags on the Simulator screen (mouse down at the left edge, slow move, release) — no accessibility actions were used for any gesture.

## Files

- `spike-01-settings-list.png` — Settings list pushed onto Home (slides in as a NavPage).
- `spike-02-account.png` — Account pushed from Settings; header reads "Account" with one Back arrow.
- `spike-03-mid-swipe.png` — mid-drag of the interactive edge-pop: the Account page follows the finger to the right with the Settings list visible underneath it.
- `spike-04-after-swipe.png` — after a full swipe past half: landed on the Settings list, header reads "Settings".
- `spike-05-tab.png` — Home tab after pressing Forms then Home from inside Settings > Account: no leftover page, no Back arrow.
- `spike-swipe-back.mp4` — under 90 seconds: Home, Settings, Account, the slow edge drag that follows the finger and is cancelled, the full swipe back to the list, a swipe back to Home, then the header's arrow path once.

## Verdicts a–g

- a. WORKS — Settings slides in as a pushed page.
- b. WORKS — Account slides in; header reads "Account" with one Back arrow.
- c. WORKS — the gesture exists and the page follows the finger with the previous page beneath it (see spike-03). One deviation from the expected thresholds: a slow drag released at ~40% of the width COMPLETED the pop instead of cancelling; a slow drag released at ~30% cancelled correctly and returned to Account. The cancel/complete threshold on this device sits between 30% and 40%, or is velocity-weighted. A full drag past half releases onto the Settings list with the header reading "Settings".
- d. WORKS — swipe-back from the Settings list lands on Home; the header Back arrow path (Account → Settings → Home) works identically to the swipe. No visible difference between the two paths.
- e. WORKS — from Settings > Account, pressing the Forms tab and then the Home tab leaves no leftover page and no Back arrow on the tab root.
- f. PARTIAL — the swipe back from the open 2551Q form to the Forms list WORKS. However, the typed value (Year Ended "12/2026") was NOT kept on re-entry: tapping the form card again opens a fresh sample form with empty fields. Note this test used the library "sample" flow, not a saved draft — the sample does not persist its in-progress field values across a pop/re-enter.
- g. ODD (minor) — the Account page's Server row shows `http://localhost:3008`, a leftover from an earlier mock-server launch on this machine; harmless in this scratch setup but noting it. No blank pages, no double-drawn pages, no header/tab-bar jumping, no content under the status bar or home indicator, no crashes observed.

## Compile / tests

- `just nsh-check` at 0179c620: PASS.
- Nine named test files at the spike and at base 0043bc5b: 90/90 pass on both — the spike breaks nothing in those files.
