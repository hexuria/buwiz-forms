# Job 12 — keyboard room before/after PR #351

**App commits:** BEFORE `00a20033` (parent, main), AFTER `36505288` (feat/android-store-keyboard-pad).
**NSH CLI:** `nsh 0.6.1 (8c78cfd)` — `~/BuwizNsh/tools-8c78cfd-bak/bin/nsh`, built from pin `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`.
**Android runtime:** clean pin build, real copied folders (not symlinks) — `~/BuwizNsh/runtime-pin-clean/` with a `nsh-ref` file holding the pin hash beside classes.dex. release classes.dex sha256 `a53652883f334dbb2844bd2844fa5246097d2f49acddf068341c43702c46943c` (dev and release dex are byte-identical copies of the same Java compile). Both APKs verified `strings classes.dex | grep -c NSHKBD` = 0 before installing; the dex inside each packaged APK re-hashed to the same sha256. Dev builds, env `EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1;BIR_AUTH_BYPASS=1`. Phone emulator API 35, keyboard docked; keyboard top edge y=1517 portrait (1080x2400).

## The verdicts first

| case | BEFORE 00a20033 | AFTER 36505288 |
|---|---|---|
| 1 New taxpayer: Email/Create above keyboard | **WORKS** — after swipes Email y1044–1154, Create y1240–1295, both fully above 1517 | **WORKS** — identical end positions (Email y1044–1154, Create y1240–1295); the spacer adds nothing visible here |
| 2 Choose forms 2551Q row with keyboard up | **STUCK** — row pinned at y1805–1856 (checkbox y1809–1851), byte-identical after 3 swipes; cannot be reached or tapped. **This is job 6's A1 hard fault, reproduced on the certified pin runtime** | **FIXED** — the same 3 swipes move the row to **y438–489** (checkbox y442–484), fully above the keyboard; tapping the checkbox with the keyboard still up sets checked=true |
| 3 Settings, keyboard up | rows already fit above the keyboard (last row Device y1235–1290); swipes move nothing | rows scroll further up (Device y1048–1103 after swipes); `keyboard-room` testID present in the dump |
| 4 form page | not reached — see below | not reached — same |
| 5 keyboard down | (shots: room-before-gesture*.mp4 and evidence files) | no extra space visible at page ends with the keyboard down (after-home/after-settings-kbd-down.png) |

**On question 3 from the owner (is job 6's fault reproducible on a clean pin runtime): YES for the chooser (A1)** — byte-identical immovability in BEFORE — **and NO for New taxpayer (A2)**: Email and Create taxpayer CAN be brought fully above the keyboard in plain main at 00a20033. Job 6's A2 measurement (hard limit with fields unreachable) does not reproduce; the difference vs job 6 is consistent with job 6 having run the contaminated ca39/3f8 runtime.

**On the owner's question (a) — does the AFTER spacer add anything at all: YES, decisively, exactly where NSH's padding does not reach.** The Choose forms page is the case where the pin runtime's own padding mechanism fails even when healthy (the list area is not the scroll that gets padded, or gets none) — and the app's own spacer is what makes the row reachable there. In New taxpayer, where NSH padding already worked, the spacer changes nothing measurable.

**`keyboard-room` testID**: present in the uiautomator dump on the Settings page with the keyboard up in AFTER (`keyboard-room present: True` in after-c3.txt); NOT present on the New taxpayer or Choose forms pages in the states captured — the spacer appears per-page or the testID is not always exposed; noted honestly rather than assumed.

## Case 4 — form page

Not reached in either build within the job's time: after setup, these commits' Home is a dashboard and the scripted taps on the "Each taxpayer at a glance" row did not open the taxpayer panel; the back stack does reach it (used for the threebutton pass), but no form was ever opened in this nav. What was tried: Forms library "Filed in 2026" row, Home dashboard rows, taxpayer glance rows.

## Nav modes and orientations

- **Gesture, portrait**: full BEFORE and AFTER passes above; recordings `room-before-gesture.mp4`, `room-before-gesture2.mp4`, `room-after-gesture.mp4`, `room-after-gesture2.mp4`.
- **Three-button, portrait**: AFTER case 1 scrolls (positions move freely with swipes, evidence after-tb-c1.txt) — recording `room-after-threebutton.mp4` covers the mode switch and navigation; the case-2 threebutton re-measurement was cut short by navigation cost (the back stack unwound to Home) — honest gap; the mechanism is nav-mode-independent (the spacer is layout, not system-inset driven), but it is not measured here.
- **Landscape**: on this phone image Gboard uses fullscreen extract mode in landscape (the keyboard covers the whole app; job 10 evidence) — there is no visible page/keyboard split to measure; the cases are portrait-meaningful only on a phone. Tablet landscape is different (docked keyboard) — see the tablet section.

## Tablet

(pending — being captured; see report message)

## Stuck-state reproduction (owner's item b)

(pending — captured in job 15 with the instrumented runtime; cross-referenced here when done)

## File index
- `before-c1-*.txt`, `before-c2-*.txt`, `before-c3*.txt` — uiautomator bounds before/after swipes, BEFORE build
- `after-c1-swipe.txt`, `after-c2-*.txt`, `after-c3.txt`, `after-tb-c*.txt` — same, AFTER build
- `after-c2-rowabove.png` — the 2551Q row above the keyboard, checked, keyboard up
- `after-home-kbd-down.png`, `after-settings-kbd-down.png` — case 5 comparison
- recordings as listed above
