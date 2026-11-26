# Job 38: PR #394 — code page takes focus on open (Android proof)

Free disk at report time: see session report. Measure only; no code changed.

## Header

- AFTER: hexuria/buwiz-forms `0234be18a7edf59b9bbc48eee2c09bf6997620fc` (feat/relay-filing-focuscode, PR #394 head), folder `target-0234be18`.
- BEFORE: main `afc1c738349c051f973fcf580873e854b8227d37` (0234be18's parent), folder `target-afc1c738`.
- Sibling nsh, tool and both runtimes at the pin `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (`~/BuwizNsh/tools`, unmoved).
- Build: `nsh package --target android --abi x86_64` with `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true BUWIZ_ANDROID_DEV=1` (the debug-assertions recipe — the debug profile makes every baked server URL loopback: `http://localhost:3008`).
- Baked dev env: `EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1` — **BIR_AUTH_BYPASS absent** (the string does not appear in the AFTER APK's classes.dex at all). `BIR_SFTP_LIVE` never set anywhere.
- Mock: `just mock-account --auto-approve` built and run from the 0234be18 checkout on 127.0.0.1:3008; `adb reverse tcp:3008 tcp:3008`. First sign-in code 246810. (The mock left over from job 31 was a stale binary that 404s `/auth/email/start`; it was killed and rebuilt — one failed first attempt discarded, not an app defect.)
- Network cut: `adb shell svc wifi disable; svc data disable` before any run. Public-host proof in `network-cut.txt`: `ping 8.8.8.8` → `connect: Network is unreachable`. Every request the app made hit the local mock (`/auth/email/start`, `/auth/email/verify` in the mock log); nothing non-local was observed.
- Emulator: AVD `buwiz`, API 35 x86_64, 1080×2400, density 420. No returns queued; drafts not even involved — sign-in proof only.

## One line per item

1. **Step 1 — PASS.** Fresh install, email typed, Continue pressed: the mock logged `/auth/email/start` and the code page arrived (`step1-email-typed.png`).
2. **Step 2 — PASS.** Arrival record, no tap on the code page:
   - screenshot within ~1.5 s of the tap/`~1 s` of arrival: `after-arrival-within-1s.png` — box 1 shows the blue ring, the number pad is already up;
   - `dumpsys input_method` (`dumpsys-after.txt`): `mInputShown=true`, `mServedView=com.longbridge.nsh.InputBox$1` (the code field's input box, 0,0-788,126), `mServedInputConnection` bound to `InputBox$KeyConnection` on the same view;
   - inputType: observed `0x2` / `0x2002` in the editor-info lines (TYPE_CLASS_NUMBER) — **not** the 0xc001 the job note expected, but functionally the number pad is what the IME shows;
   - caret: visible in box 1 in the video frames (`swap-frames.png`, blinking in and out across frames), blue ring in every frame;
   - keyboard: the number pad, not QWERTY — settled within ~1–1.5 s of the page arriving.
3. **Step 3 — PASS.** `adb shell input text 246810` with no tap anywhere on the code page: the mock logged `/auth/email/verify` and the app reached the post-sign-in "Who will use eBIRForms on this device?" page (`after-step3-home.png`).
4. **Step 4 — PASS.** `pm clear`, same flow, code typed with six number-pad taps (2-4-6-8-1-0), no tap on the boxes: same post-sign-in page (`after-step4-home.png`).
5. **Step 5 — PASS.** `j38-email-code-home.mp4` (120 s, full email → code → Home flow of the step-4 run; taps visible). Note: the step-3 run's recording was lost to the known premature-pull moov corruption; the step-4 recording is the same flow end to end and is the delivered video.
6. **Step 6 — no jump, no flicker, no scroll.** Frame analysis at 4 fps across the QWERTY→number-pad swap (`swap-frames.png`): the code page arrives with QWERTY still up; over the next ~1 s the IME swaps in place to the number pad (one mixed transition frame), never closing or bouncing in between; the page itself never moves — the boxes, buttons and card stay at identical pixels in every frame. Nothing to stop for.

## BEFORE (afc1c738) — matches the prediction

`before-arrival.png` + `dumpsys-before.txt`: the same arrival record shows **no caret and no blue ring** in any box, **QWERTY still up**, and `mServedView=com.longbridge.nsh.NshView` — focus sits on the app container view, not the code input. The contrast with the AFTER record is exactly what #394 claims.

## Harness notes

- `adb shell input text` types into the focused box without any tap because the box owns the IME focus — itself evidence the fix works.
- The number-pad swap lag (~1 s of QWERTY after the page arrives) is the IME's own layout switch, visible in the frames; it never produces a wrong-keyboard moment long enough to type wrong.
