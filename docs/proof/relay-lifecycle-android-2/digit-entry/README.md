# Digit-entry failure on Android — standalone finding (e5f7a504, job 19)

## Verdict

**POSSIBLE V1 BLOCKER: digit entry on Android.** On the sign-in code page (the kit's OtpInput), NO input
method reaches the boxes — not hardware keyevents, not `input text`, not taps on the on-screen keyboard
digits — on a cold-booted emulator, from a fresh install, with no long press ever issued. Plain text
fields (the email field on the preceding page) accept input normally in the same session. The fault is
specific to the boxed digit input, which is the same kit control family as the money box.

## (a) Exact order of the pre-boot attempts on e5f7a504 (timestamps UTC+8-ish, 2026-11-26 morning)

All times approximate, from the harness log (harness.log in the parent proof folder is partial; the
sequence is exact):

1. ~03:44 install + first launch, email `maria@example.com` typed with `input text` (worked), Continue.
2. First digit attempt: hold-tap 150 ms at the box strip + `input keyevent KEYCODE_2/4/6/8/1/0` — **FAILED. This was before ANY long press** (no long press had ever been issued in the session at that point).
3. tap (200,763) + keyevents — fail.
4. tap (300,763) + `input text 246810` — fail.
5. Back (which exited the app), tap, slow keyevents — fail.
6. remount code page via Change email; 5 s settle; single tap first box; single KEYCODE_2 — fail.
7. htap variants, double-tap, rapid keyevents — fail.
8. 800 ms long-press on a box — no selection menu, no effect (first long press of the session, long after the first failure).
9. blind full-code keyevents — fail. blind `input text` — fail.
10. force-stop + reopen — app returned to the email page; no recovery.
11. Reinstalled the job-37 APK (40464693 — the very build that signed in successfully at 03:21 the same day):
    SAME failures with blind keyevents and `input text` and taps along the strip's left edge.
    => not an e5f7a504 regression; the job-37 APK fails identically in the current environment.

**Answer: the very first failed attempt came BEFORE any long press.** The nsh#44 long-press mechanism
cannot be the cause of what is measured here, though it may be an additional fault.

## (b) Cold-boot retry — exact protocol

- Emulator cold-booted with `-no-snapshot-load` (sys.boot_completed=1 fresh; uptime minutes).
- `pm clear dev.goldcoders.bir` + reinstall of the e5f7a504 APK, fresh launch.
- NO long press anywhere in the run.
- Email typed (`input text` — worked, mock issued code 246813), Continue, code page reached.
- ONE plain tap on the first box, then in order:
  - **on-screen keyboard digit taps** ('2' at the number-row coordinates, then '4') — boxes stayed empty (cb03/cb04);
  - **`input text 24`** — boxes stayed empty (cb05);
  - **`input keyevent KEYCODE_6 KEYCODE_8`** — boxes stayed empty (cb06);
  - completing the correct code 246813 with KEYCODE_1/KEYCODE_3 — no submit, no error, nothing (cb07).
- **Verdict: cold boot did NOT cure digit entry. NO.** Same failure with all three methods.
- The long-press comparison run is moot: entry already fails completely without any long press.

## (c) Instrumentation captured per run (this folder)

- `run1-after-first-tap-logcat.txt` — logcat -c before the first box tap, dumped after.
- `run1-inputtext-logcat.txt`, `run1-keyevent-logcat.txt` — same around those steps.
- `run1-input-method-during.txt` — full `dumpsys input_method` while on the failing page (keyboard up).
- `run1-window-focus.txt` — full `dumpsys window`.
- `input-method-postboot.txt` — input_method right after the cold boot.
- `preboot-evidence.md` — what could be reconstructed of the pre-boot state (the boot had already started
  when the capture order arrived): emulator/IME identity, package list, logcat IME lines.

## Key facts

- Focused node while typing (uiautomator, pre-boot): a plain `android.widget.FrameLayout` in
  dev.goldcoders.bir — NOT any input view. mInputShown=true throughout (keyboard visibly up).
- IME: com.google.android.inputmethod.latin (Gboard) versionName=14.2.09.629370537-preload-x86_64.
- Emulator: sdk_gphone64_x86_64, API 35, fingerprint google/sdk_gphone64_x86_64/emu64xa:15/AE3A.240806.043/12960925:userdebug/dev-keys,
  system image android-35/google_apis/x86_64, emulator 37.3.10.0.
- Packages: dev.goldcoders.bir and com.longbridge.nsh.host (job 35's dev host) both installed.
- Pre-boot full logcat (17k lines) shows no Gboard crash/restart around the failures.
- Plain text fields work: the email field accepted `input text` in every session, including this one.

## Minimal reproduction from a fresh install

1. Install the e5f7a504 debug APK, pm clear, launch.
2. Type an email into the email field (works), tap Continue with email.
3. On the code page: tap the first box once, then type digits by any method.
4. Boxes stay empty; nothing is submitted; no error. Focus is on a FrameLayout, not the input.

## Note on the 03:21 success

The same failure now reproduces on the 40464693 APK whose sign-in succeeded at 03:21 (harness-log
evidence). Whatever made it work then is environmental and could not be re-established: not the app
version, not the emulator image, not a snapshot difference (no snapshot was restored). The dev host
(com.longbridge.nsh.host) was installed and exercised between the success and the failures.

## Mechanism-level evidence (from run1-input-method-during.txt)

While typing on the code page, `dumpsys input_method` shows:

- `mServedView=com.longbridge.nsh.NshView{... .F...... 0,0-1080,2400}` — the IME serves the whole custom
  NSH view, not any input widget;
- `mFallbackInputConnection=RemoteInputConnectionImpl{connection=android.view.inputmethod.BaseInputConnection@...}`
  — the active input connection is the system's FALLBACK BaseInputConnection, which discards committed text;
- `mInputShown=true`, `mCurrentFocus=dev.goldcoders.bir/com.longbridge.nsh.AppActivity`.

Reading: no real input connection is ever established for the digit input, so IME-committed text
(on-screen keyboard taps, `input text` commit channel) is discarded. This also explains why plain-text
fields appear to work: `adb shell input text` injects most ASCII as raw keyevents, which the NshView key
pipeline handles directly — but the kit's boxed inputs (OTP, money) take their input through the input
connection, which is the broken path. Consistent with the money-box symptoms ("." dropped, Backspace dead).
