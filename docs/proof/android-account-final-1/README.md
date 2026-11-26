# android-account-final-1 — job 28: #345 final Android look (63743f3b)

## Header
- app commit: hexuria/buwiz-forms `63743f3b` (feat/relay-filing-signin; f95b6996 + main 1b18d668)
- sibling nsh: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (the pin, real copy; no nsh update)
- tool: `nsh 0.6.1` at pin (tools-8c78cfd-bak)
- runtime classes.dex sha256, re-hashed from the packaged APK: `a53652883f334dbb2844bd2844fa5246097d2f49acddf068341c43702c46943c`
- CARGO_TARGET_DIR: `~/BuwizNsh/target-63743f3b`
- baked: `EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1;BUWIZ_SERVER_URL=http://localhost:3008;BUWIZ_ACCOUNT_URL=http://localhost:3008;BUWIZ_TRUST_KEYS=dev-test-key-not-production;BIR_DATABASE_PATH=test-data/bir_data.db` — **no BIR_AUTH_BYPASS**. NOTE: `BUWIZ_ACCOUNT_URL` had to be added to job 17's list: at this head the account server is keyed by it (release default is the real `https://account.buwiz.com`); without it sign-in would have left the mock.
- mock: `mock_account` built from this commit, `--port 3008 --auto-approve`, adb reverse. Codes read from the mock's own lines (246810, 246811). Nothing went to a non-local host: the Account page itself displays `Server http://localhost:3008` (s25) and the mock printed both codes.
- device: emulator-5554, API 35, gesture nav; show_touches=1; 150 ms hold-taps.

## Results, one line per item

1. **Sign in email+code — WORKS.** Gate shows (s01), email + code from the mock signed in (s01–s04; code 246810). First run then asked the admin PIN + recovery key (s05–s07), then Home (s07).
2. **Settings > Account signed in — WORKS.** One "Buwiz account" group: server line `http://localhost:3008`, "Signed in as maria@example.com", Sign out of Buwiz, Delete account (s14, s25).
3. **Sign-out sheet vs status bar — CLEAR, but the keyboard half does not apply: the sheet has NO field.** Content: "You will go back to the sign-in page. Your drafts and returns stay on this device." + Sign out / Cancel (s15, s26). `mInputShown=false` with it open and there is nothing focusable — keyboard-down is the only state it has. Measurements: sheet panel's top edge at **y≈998 px** (pixel scan of s15; first white row of the panel inside the dimmed page, content text starts y1053 per uiautomator), status bar inset **top=74 px** (dumpsys window InsetsFrameProvider). The whole sheet sits ~920 px clear below the status bar's bottom.
4. **Sign out — WORKS, and then "Settings > Account signed out" is NOT REACHABLE on this build.** Tapping Sign out returns to the cloud email gate (s16). From there: no avatar/Settings access (s17), Back exits to the launcher (s18b), and force-stop + relaunch shows the same gate (s18c) — not the device TIN+PIN page — even though a PIN-protected database with drafts exists on the device. So there is no signed-out Account page to show; reported exactly as measured. (For contrast: while still signed in, force-stop + relaunch DOES lead to the device TIN+PIN sign-in, s10/s24 — the cloud gate passes first, then the device lock.)
5. **Back controls — exactly ONE on every Settings page passed.** Settings root: only `nb-back` (s13/s19). Account: only `nb-back` (s14/s25). Personal: only `nb-back` (s20-personal). The job-17-era double back control (`nb-back` + `settings-back` breadcrumb) is gone — #347's fix is in effect.

## Extra observations (exactly as measured, not part of the asks)

- **The lock card appears after Back-exiting the app and relaunching** ("eBIRForms is already open on this database…", s09b) and Try again did not open it (s09c); force-stop was the recovery, twice. Consistent with the lifecycle fix being in #363, not here.
- Once, the text keyboard was up on Home with no field focused (s08b) — imshown=true right after the device sign-in completed; the first avatar taps did not open the menu in that state, later taps did.
- The sweep harness's taps on category rows beyond Personal were unreliable (rows not found / backed out to Home) — harness limitation, not necessarily app behavior; the three pages reached all showed exactly one back control.

## Pictures, step by step
- s01 welcome/email gate; s02 email typed; s03 code page; s04 after correct code; s05 first-run choice result (admin PIN page); s06 PIN page; s07 Home after recovery key; s08–s08d avatar menu attempts (keyboard note); s09/s09b/s09c lock card + Try again; s10/s10b/s10c/s10d device TIN+PIN sign-in; s11/s12/s12 menu open; s13 Settings root; s14 Account signed in; s15 sign-out sheet (measurement shot); s16 after sign-out = email gate; s17/s18b/s18c signed-out reachability evidence; s19–s20 Settings root + Personal back-control sweep; s21–s23 second account visit; s24 device sign-in after force-stop while signed in; s25 Account signed in (clean); s26 sign-out sheet (clean); s27 after Cancel.
- videos: `j28.mp4` (first minutes: gate → email → code → first run, taps shown), `j28c-sheet.mp4` (Settings > Account > sign-out sheet > Cancel, taps shown).
- `harness.log` — timestamped harness transcript.
