# Android emulator matrix — main report (interim: phone complete, tablets blocked, see end)

Checkout: origin/main @ d7fc83be951abb46e422150022239f5de181213e. Package dev.goldcoders.bir, versionCode=27, versionName 0.2.1, minSdk 26, targetSdk 35. APK 137,443,462 bytes (~131 MiB) installed. flags=[HAS_CODE ALLOW_CLEAR_USER_DATA ALLOW_BACKUP] — **ALLOW_BACKUP is present on main**. POST_NOTIFICATIONS not granted. Everything through scripts\run-android-windows.ps1 (dry-run env baked in), mock account on :3008, invented taxpayer only.

## Result table

| item | phone (Pixel-7-profile, API 35) | small tablet (Nexus 7 2013) | large tablet (Pixel Tablet) | API 26 phone |
|---|---|---|---|---|
| a install/first launch | pass w/ notes (8.3 s cold start on swiftshader) | blocked — see note | blocked | not done — no budget/disk |
| b sign-in (mock) | pass — silent, no browser visible | blocked | blocked | — |
| c add taxpayer | partly — real defects found | blocked | blocked | — |
| d form 2551Q fill+check | partly | blocked | blocked | — |
| e print preview/PDF | pass (system print UI) | blocked | blocked | — |
| f settings | not completed | blocked | blocked | — |
| g rotation | pass | blocked | blocked | — |
| h back button/gesture | fail — wrong destination + ANR | blocked | blocked | — |
| i background/force-stop | partly — dev-config sign-in loss | blocked | blocked | — |
| j tablet width/split-screen | n/a | blocked | blocked | — |
| k reviewer trip-hazards | found — see list | — | — | — |

## Details (phone)

**a.** Cold start `am start -W`: TotalTime 8332 ms — on a swiftshader software-rendered emulator, so not representative of hardware, but slow even so. No blank/flashing frames; splash draws edge-to-edge covering the status-bar area. Launcher icon/label not verified (minor, unchecked).

**b.** Sign-in completes silently in seconds against the mock (auto-approve). No browser ever opens and no code/address fallback is shown on main — consistent with what PR #336 set out to fix.

**c.** Taxpayer creation fields are all reachable by scrolling; the soft keyboard (Gboard) DOES appear normally (an earlier dumpsys-based concern of mine was wrong). Defects found:
1. In the form chooser, search-result rows render underneath the open soft keyboard — tapping a result hits keyboard keys and types junk into the search box. You must dismiss the keyboard first. (screenshot d-row2551.png)
2. Tapping a result row's text does not select it — only tapping the small checkbox at the far left does.
3. Synthetic text input gets badly mangled in these fields (fast injected input drops/duplicates characters; a long name came out as garbage). Cannot yet say whether real-typing speed is affected — flagged for the app to check its input batching.
4. Display truncation: long registered names show clipped without ellipsis in the header ("Maria Clara Delos Reyes y Sa").

**d.** 2551Q opens cleanly: title, "2 pages · 85 fields", Print preview / Check / Queue all fine at phone width. Check works — floating bottom panel "1 to do · Page 1" with red badge and arrows (d-check.png). Full validation fill not completed in budget (needs business start date + income-tax election in profile first); the one-tax-code limit not re-verified.

**e.** "Print preview" does not open an in-app preview — it hands off to the Android system print activity. In words: printer dropdown offering "Save as PDF" and "All printers…", copies, paper size ISO A4, a paged preview showing "1/2". Save as PDF opens the system DocumentsUI picker defaulted to Downloads with a long auto-generated filename; SAVE writes a real PDF — verified 274,330 bytes in /sdcard/Download — and returns to the app.

**f.** Not completed — ran out of budget before a full pass. Honestly reported as unchecked.

**g.** Rotation passes: layout follows portrait↔landscape, typed-but-unsaved text survived both rotations, no crash (g-landscape.png).

**h.** Two defects: (1) Back from the taxpayer screen lands on the "New taxpayer" creation form with the old half-entered data still in it — the creation screen stays in the back stack instead of being replaced. (2) After further back presses the app hung: system dialog "eBIRForms isn't responding — Close app / Wait" (h-anr.png); logcat "Completed ANR of dev.goldcoders.bir in 32273ms". Wait did not recover within 30 s. Caveat: swiftshader emulators are slow and the host was loaded, but a 30-second main-thread stall on back-navigation is severe regardless.

**i.** After force-stop + relaunch the app returns to the sign-in gate — in this dev configuration (BUWIZ_NO_KEYCHAIN=1) credentials are memory-only, so this is at least partly a harness artifact; real keychain builds may persist. After re-sign-in, onboarding restarted and the previously created taxpayer did not reappear — confounded by the in-memory mock account server (accounts vanish on restart), so data persistence is honestly unresolved and needs a controlled retest.

## Five worst problems, in order

1. **ANR on back navigation** — "eBIRForms isn't responding", unrecoverable within 30 s. A store reviewer hitting this fails the app instantly.
2. **Back-stack pollution** — back from the main taxpayer screen drops you into the stale half-filled creation form instead of home.
3. **Form-chooser results render behind the soft keyboard** — search results untappable without dismissing the keyboard, and taps type garbage into the search.
4. **Input mangling under fast typing** — injected text loses/duplicates characters in app fields (needs verification with real keyboard timing, but if real, taxpayer names/addresses get corrupted silently).
5. **Sign-in/onboarding does not survive force-stop in practice** — even accounting for the dev keychain artifact, the user is dropped back into full re-onboarding with no sign their data still exists.

Honourable mentions: ALLOW_BACKUP on main (job 2's branch addresses this); titles ellipsized in lists; splash covers the status bar; POST_NOTIFICATIONS denied by default.

## Tablet/API-26 status — honest blocker

The two tablet AVDs (buwiz_tab7 Nexus-7-2013, buwiz_tab10 Pixel Tablet, both API 35 google_apis x86_64) were created and the app installed, but running 2–3 emulators concurrently on this nested-virtualization host wedges the software compositor (black screens, SystemUI ANRs, 20–40 minute boots). Both tablets are currently wedged/black after 25+ minutes. I killed the phone emulator to free CPU so they can finish booting; the tablet pass (items a–c quick check + item j) continues as soon as they come up, and I'll send a tablet supplement. The API 26 phone was not started — disk pressure (5 GB free) and time; image IS available for x86_64 (downloaded already).

Evidence in C:\Users\Administrator\BuwizNsh\scratch\android-evidence\ — key shots: d-row2551.png (keyboard hiding results), d-check.png (validation panel), h-anr.png (ANR dialog), g-landscape.png (rotation).

---

## Developer hand-off detail for the five problems (added after first report)

### 1. ANR on back navigation
- Steps from fresh launch: sign in (auto-approve), create/choose taxpayer, open any form, press system Back repeatedly (form -> taxpayer screen -> lands in stale New-taxpayer form -> back again).
- Reproduced once fully; second reproduction not attempted before the emulator was power-cycled. logcat: `Completed ANR of dev.goldcoders.bir in 32273ms`.
- ANR subject line (traces file, committed as anr-app-trace.txt): `Input dispatching timed out (dev.goldcoders.bir/com.longbridge.nsh.AppActivity (server) is not responding. Waited 5006ms for FocusEvent(hasFocus=true))`.
- Main thread (anr-main-thread.txt): blocked in `android::uirenderer::renderthread::RenderProxy::setStopped` -> `std::future<void>::get` — UI thread parked waiting on the render thread. On software rendering this may be partly environmental, but the 30s+ stall with no recovery needs a look; note the emulator also ANRs its own SystemUI and Google search app under load, so treat the environment as a confound and repro on hardware.

### 2. Back-stack pollution
- Steps: fresh launch -> sign in -> finish taxpayer creation -> land on taxpayer Forms screen -> press Back once. Expected: app home or exit. Actual: the half-filled "New taxpayer" creation form with the previously typed values still in it. Reproduced twice (both sign-in cycles). No logcat lines — navigation logic, not a crash.

### 3. Form-chooser results behind keyboard
- Steps: taxpayer -> Forms -> Choose forms -> tap search -> type 2551Q. Result row renders in the area covered by the open soft keyboard; tapping it types letters into the search instead (see committed d-row2551.png — empty white gap above keyboard where results should be). Reproduced every time. Workaround: dismiss keyboard, then tap.

### 4. Text input mangling
- Typed with `adb shell input text` (synthetic injection), NOT the on-screen keyboard — the on-screen keyboard could not be tested in the taxpayer form because it does not appear there at all (see problem 4b).
- New evidence since first report (kbd-test.png): (a) TIN segmented control dropped the digit '2' even when digits were injected one at a time with 300 ms spacing — shows "1 3 4 - 5 6 7 - 8 9" with validation error; (b) a long address injected while focus was ambiguous interleaved into the Registered-name field, producing "Maria Clara Delos R234 Tower B, Th" — text fragments merged across fields. So even slow deliberate input is corrupted and cross-field focus is unreliable. Cannot rule out an adb-injection artifact; needs a repro on hardware with a real keyboard, but the app-side input batching/focus handling is the place to look.
- 4b (new, maybe its own bug): in the New-taxpayer form the soft keyboard never appears even with a field focused and cursor visible (mInputShown=false), while in the form-chooser search the keyboard DOES appear. Inconsistent IME behavior between screens.

### 5. Sign-in/onboarding loss after force-stop
- Steps: sign in, create taxpayer, force-stop app, relaunch. Result: Welcome gate again (partly dev artifact: BUWIZ_NO_KEYCHAIN=1). After re-sign-in the onboarding restarts at an EMPTY New-taxpayer form; the earlier taxpayer never resurfaces. Reproduced twice. Confound: mock account server is in-memory — but it was NOT restarted between the cycles, so the account identity should have matched; suspect taxpayers are keyed to something ephemeral or the post-sign-in flow always pushes creation regardless of existing data.
