# iOS Paste rows for hexuria/nsh PR #46 — iPhone and iPad proof

Job I46. Kit under test `bd96100e779d332e2da8cdea936939194e6c9251` (PR #46),
comparison kit `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (1.0.0's pin, where Paste works).
App: hexuria/buwiz-forms `e318e4514d989f553a5ca315cf0df7301a954455` (origin/main) with `../nsh`
at each kit commit in turn, one CARGO_TARGET_DIR per kit. iOS simulator dev recipe with
`EBIR_TEST_ENV=1 BUWIZ_NO_KEYCHAIN=1 BIR_SFTP_DRY_RUN=1 BIR_AUTH_BYPASS=1`, driven with
gpui-agent only. Never Queue. Clipboard held `123456` (`xcrun simctl pbcopy`).

Devices: iPhone 17 Pro, iOS 27.0 (4773C6D7-080E-4816-846A-2BACA1B5B600) and
iPad Pro 13-inch (M5), iOS 27.0 (133028B5-1A78-4C51-87C0-866175BA466B).

Every run is a cold arrival: the app is terminated, relaunched, and driven
email → code page before the cell's touches. File naming:
`<device>-<kit>-<cell>-run<N>-menu.png` (the edit menu after the long press),
`-after.png` (after the choose attempt), `-fill-fast.png` / `-fill.png`
(the paste-verb probe, see below).

## The structural result: gpui-agent gap on every cell

The iOS edit menu is UIKit-native. It is not in gpui-agent's semantic tree at all,
and gpui-agent cannot activate it:

- `press --x --y` at the Paste item is refused host-side: `error: node 74 is not
  pressable` — the hit test lands on the app's own nodes *behind* the native menu,
  and the menu itself never receives the synthesized touch.
- `key` chords are unsupported on the iOS host (`native focus is unsupported on
  this platform`); `keybinding` has no paste action; there is no menu verb.

So for every cell: **gpui-agent gap: cannot activate the native edit menu item**
(the "choose it" half of the cell). gpui-agent *can* long-press, the menu *does*
open, and it is read here from the screenshots. No improvisation: no simctl-level
touch injection or other driving was used. Because choosing Paste is impossible in
all 24 runs, no cell can produce the spec's PASS or FAIL (which needs 3/3 fills at
one kit against 3/3 at the other); the table reports what the runs do prove.

## Results: menu shown / Paste visible (of 3 runs per cell)

| device | cell | kit bd96100e | kit 8c78cfd6 |
|---|---|---|---|
| iPhone 17 Pro | first long press | menu 3/3, Paste 3/3 | menu 3/3, Paste 3/3 |
| iPhone 17 Pro | after one tap | menu 3/3, Paste 3/3 | menu 3/3, Paste 3/3 |
| iPad Pro 13-inch | first long press | menu 3/3, Paste 3/3 | menu 3/3, Paste 3/3 |
| iPad Pro 13-inch | after one tap | menu 3/3, Paste 3/3 | menu 3/3, Paste 3/3 |

The menu opens directly under the pressed box every time, shows **Paste | AutoFill**,
and stays open (it does not close at once) at both kits, on both devices, in both
cells — no visible difference between the kits in anything gpui-agent can drive.
"After one tap" note: the tap lands on the box (caret/focus), then the long press
opens the same menu.

## Supplement, clearly labeled: the paste-verb probe

After each run's choose attempt, the run additionally fired gpui-agent's own
`paste` verb with the clipboard string `123456` into the OTP input — not the menu
path, not a spec step, included because it is the only paste driving gpui-agent has.
At **both** kits, on both devices: all six boxes fill and the input auto-submits;
the fake backend answers "That code is not right. 4 tries left." (the code is a
wrong one on purpose; the sign-in code is irrelevant to the fill test). The error
state is itself the evidence that the paste filled all six boxes, since the submit
fires only on complete input; the filled state is transient because validation is
fast. No difference between the kits here either.

Coverage note: the earliest iPhone bd runs (first 1–3, tap 1–2) and iPad bd
first-run1 predate the probe and have menu/after shots only; every other run has
all four files. The menu evidence — the spec's primary proof — is complete 24/24.

## Environment notes for reproduction

- The iOS dev host always connects `ws://127.0.0.1:8790/ws` (hardcoded fallback in
  nsh-ios-host; `simctl launch` passes no `--dev-server`), so the dev server must
  run on the default port; one dev server serves both devices.
- The agent plane reads its token from `Library/Caches/gpui-agent-token` in the
  app's data container once per surface open and deletes it; a writer loop must
  keep it present per device or the plane never binds after the first surface.
- `SIMCTL_CHILD_EBIR_TEST_ENV/BUWIZ_NO_KEYCHAIN/BIR_SFTP_DRY_RUN/BIR_AUTH_BYPASS`
  (plus the GPUI_AGENT plane vars) reach the simulator child; plain exports do not.
- The gate page was driven through the repo's AuthDemo harness (`gate: true`)
  because the mandatory `BIR_AUTH_BYPASS=1` opens the real app's gate.
