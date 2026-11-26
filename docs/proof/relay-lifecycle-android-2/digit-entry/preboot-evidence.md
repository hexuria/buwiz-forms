# Pre-cold-boot evidence (captured before the boot or reconstructed from surviving logs)

NOTE: the cold boot was already in progress when the addition arrived, so the live pre-boot captures
(dumpsys input_method/window on the failing page, logcat -b crash) could not be retaken. What survives:
the full continuous adb logcat capture `logcat-pre-reboot-full.log` (17,442 lines, covering the whole
failing session), the recorded uiautomator/dumpsys outputs in this README's reproduction section, and
the emulator/IME identity which does not change across a cold boot.

## B. Emulator identity (unchanged by boot)
- ro.build.version.sdk: 35
- ro.product.model: sdk_gphone64_x86_64
- ro.build.fingerprint: google/sdk_gphone64_x86_64/emu64xa:15/AE3A.240806.043/12960925:userdebug/dev-keys
- AVD `buwiz` (port 5554): image.sysdir.1 = system-images/android-35/google_apis/x86_64/
- emulator -version: Android emulator version 37.3.10.0 (build_id 16970787)

## IME
- default_input_method: com.google.android.inputmethod.latin/com.android.inputmethod.latin.LatinIME (Gboard)
- versionName=14.2.09.629370537-preload-x86_64
- full dumpsys input_method: input-method-postboot.txt (post-boot; pre-boot equivalent was observed with
  mInputShown=true during the failing attempts)

## Packages present both before and after the boot
- dev.goldcoders.bir (the test app)
- com.longbridge.nsh.host (job 35's dev host, installed since the job-35 measurements; its DevActivity
  was the resumed activity once when the test app was backed out of — see finding)

## C. What changed since 03:21 (when the job-37 APK signed in successfully)
- No snapshot restore: the emulator ran continuously since before 03:21 (started windowed in an earlier
  session, then restarted at various points; uptime was hours, not a fresh boot at failure time).
- The dev host com.longbridge.nsh.host was installed and exercised for job 35 (dev-server-driven)
  BETWEEN the 03:21 success and the failure session. It remains installed.
- logcat-pre-reboot-full.log: 24 lines mention com.google.android.inputmethod, none of them crashes
  (see logcat-ime-lines.txt); no Gboard crash/restart signature around the failing attempts.
- During failures: dumpsys input_method showed mInputShown=true (keyboard visibly up in screenshots),
  and uiautomator showed the focused node as a plain android.widget.FrameLayout in dev.goldcoders.bir,
  NOT an EditText/input view. Screenshots s05/s08 in the proof folder show the keyboard up and boxes empty.
