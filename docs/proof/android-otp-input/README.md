# Job 14 — TIN digit loss and long-press paste, before/after hexuria/nsh#39

App commit both builds: `75bd78b8` (origin/main). Emulator: AVD buwiz, API 35 x86_64, show-taps on.
BEFORE: nsh tool + runtime from pin `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`, dev classes.dex sha256 `73b3126db70bdca7e03f0ee3acb39c63437efc1f606c7232d0324445a1788e17`.
AFTER: nsh tool + runtime from `62e436ff170ed6652b05815deb6799077285cad2` (PR #39 head), dev classes.dex sha256 `3f102613d7aa3b3e917d91abf6fd175cad865eb02dccc9598cbff0c484d52f8c`. Tools in `~/BuwizNsh/tools-62e436f` with nsh-ref beside each classes.dex.

## Items

- TIN digits, stress round (WORKS as evidence): ten 9-digit TINs typed with real taps on the on-screen numeric pad, a keyboard hide+reshow after the 4th digit. BEFORE 9/10 complete-and-in-order: tin-05 `246813579` came out `246-813-555` (7 and 9 lost, 5 repeated). AFTER 10/10.
- TIN digits, fast round (WORKS): ten more with no keyboard interruption, as fast as adb taps go. BEFORE 10/10, AFTER 10/10.
- Long-press paste (WORKS as evidence — negative result): clipboard loaded by typing into Registered name then ctrl+a ctrl+c (`input keycombination 113 29/33`). For `000123453`, `000-123-453`, `000 123 453`, `00012345300000` in turn: long-press of the first TIN box shows NO paste menu in EITHER build, and the numeric pad shows no clipboard chip. Nothing to tap Paste on, in either build.
- Sign-in code boxes: DOES NOT APPLY — skipped. The dev host does not bake BUWIZ_SERVER_URL/BUWIZ_TRUST_KEYS and the mock sign-in is the device-approval flow; the six-box email code page is not reachable in these builds.

## Substitutions and caveats (read before trusting)

- The TIN field opens a NUMERIC pad that has no "?123"/"ABC" layout key. The keyboard interruption after the 4th digit was the keyboard's hide chevron followed by a tap on the next empty box, which restarts the input connection the way a layout switch does. Documented because the spec named a layout key this keyboard does not have.
- Taps were scripted `adb shell input tap` in one shell per batch with `sleep 0.12` between them, but each `input` invocation costs ~300-400ms, so real spacing was ~0.4-0.5s in the stress round and ~0.3-0.4s in the fast round — "as fast as taps allow" holds for both.
- AFTER build crashed ONCE during the first automated paste run: `java.nio.BufferUnderflowException` at `com.longbridge.nsh.Mounter.command(Mounter.java:2521)`, 14:13:01, logcat captured in paste-after-crash.logcat.txt. Step-by-step reproduction did NOT crash; a full re-run of the same automated paste script did NOT crash. One occurrence, not reproduced. The paste-after evidence here is from the clean re-run.
- tin-before.mp4 is two screenrecord segments concatenated with `ffmpeg -c copy` (the run exceeds the 180s platform cap). tin-after.mp4 is one segment covering roughly the first half of the run; the systematic proof is the per-TIN screenshots in the sheets, not the video. paste-before.mp4/paste-after.mp4 cover their full runs.
- The first paste-before attempt was invalidated by the back key navigating out of onboarding (the back key is not a safe keyboard-dismiss); evidence here is the corrected chevron-based re-run.

## Files

- before-slow-sheet.png / after-slow-sheet.png: contact sheet of the TIN boxes after each of the 10 stress-round TINs, before/after. Read top-to-bottom tin-01..tin-10; the typed TIN is in the file label on each row.
- before-fast-sheet.png / after-fast-sheet.png: same for the 10 fast-round TINs.
- tin-before-05-full.png / tin-after-05-full.png: full screens of the failing BEFORE tin-05 and the correct AFTER tin-05.
- paste-before-sheet.png / paste-after-sheet2.png: the four paste attempts before/after; every one shows no paste menu (boxes empty with the validation tint because the field is empty).
- tin-before.mp4, tin-after.mp4, paste-before.mp4, paste-after.mp4: screenrecord captures; caveats above.
- paste-after-crash.logcat.txt: the one-time AFTER crash stack.
