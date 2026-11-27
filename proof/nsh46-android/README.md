# Job 44 — Android device proof for kit PR hexuria/nsh#46 (OtpInput) at kit head 75a39332

## Builds

- App: hexuria/buwiz-forms main `642bfc78221c0a87b318c7c3378c3f5b0b2c9359` (main as it was at job start), both builds.
- AFTER: NSH tools at the kit head `75a393326c627b976e4a3a703f9297a82b698424` (`~/BuwizNsh/tools-75a39332`, sibling `../nsh` at exactly 75a39332, pinned toolchain's cargo, never `nsh update`), over the app's 8c78cfd6 pin as ordered. APK sha256 `80724531e45895d4a50c257d3ca1b7a3328c874f6b7f8e50997b03322329c10a`.
- BEFORE: the app's own pin 8c78cfd6 with `~/BuwizNsh/tools`. APK sha256 `f4673768a21cf009fd4d73451a787a873bcb701fd204b3994fb6c98fb6384182`.
- Each build in its own target folder, docs/relay.md dev recipe, x86_64. The sibling `../nsh` was put back to 8c78cfd6 afterwards.

## Safety

EBIR_TEST_ENV=1, BUWIZ_NO_KEYCHAIN=1, BIR_SFTP_DRY_RUN=1 throughout; the local mock account server only (`just mock-account --auto-approve`, adb reverse tcp:3008) with BIR_AUTH_BYPASS unset; the network stayed cut; never Queue; invented values only (example.com addresses, TIN digits 123456789); touches on. No BIR_SFTP_LIVE anywhere.

## Method

3 runs per cell, each from a cold arrival on the page (app relaunch; for the TIN page: relaunch → "Just me or my business" → New taxpayer). Keyboard settled before the gesture; taps inside the boxes' own bounds. "First long press" and "long press after one tap" are separate cells. A failure means failing 3/3 at AFTER while passing 3/3 at BEFORE; anything else is reported with all six results.

Clipboard notes (API 35): `cmd clipboard set-text` has no shell implementation, so 123456/654321 were put on the clipboard by typing into a real text field, select-all, copy. Digit readout is by a template classifier whose templates were harvested per build from known typed sequences; where the classifier is ambiguous (6/5, caret read as a digit), the video frames decide — all videos/frames are in this folder.

Boxes covered: the sign-in email code (6 digits, `otp-*`) and the TIN boxes on New taxpayer (9 digits, `tin-*`). The PIN pad's boxes were not reachable as a test surface here: reaching them needs a completed taxpayer setup and a sign-out, and the sign-in PIN pad reuses the same OtpInput component the OTP/TIN cells already exercise; no claim is made about it beyond that.

## Results

Pass/fail is about the PR's promised behavior at AFTER; "BEFORE" shows the old-kit behavior for contrast. No cell fails 3/3 at AFTER while passing 3/3 at BEFORE.

| cell | box | AFTER (3 runs) | BEFORE (3 runs) | verdict |
|---|---|---|---|---|
| 1 empty, first long press | OTP | Paste menu shows + inserts 123456, 3/3 (`otp-c1-r*-pasted.png` read 123456) | no menu at all, 3/3 | pass — fixed by the PR |
| 1 empty, first long press | TIN | Paste shows + inserts, 3/3 | no menu at all, 3/3 | pass — fixed by the PR |
| 2 empty, long press after one tap | OTP | Paste shows + inserts 123456, 3/3 | no menu, 3/3 | pass — fixed |
| 2 empty, long press after one tap | TIN | Paste shows + inserts, 3/3 | no menu, 3/3 | pass — fixed |
| 3 filled, long press FIRST cell | OTP | whole code selected, platform menu (Cut/Copy/Paste), paste 654321 replaces whole, 3/3 | no menu, 3/3 | pass — fixed |
| 3 filled, long press MIDDLE cell | OTP | same: whole select + paste replaces, 3/3 | no menu, 3/3 | pass — fixed |
| 3 filled, long press LAST cell | OTP | same, 3/3 | no menu, 3/3 | pass — fixed |
| 3 filled, long press FIRST cell | TIN | whole code selected + platform menu, paste replaces, 3/3 (`tin-c3-first-rec-menu.png`: all 9 boxes highlighted, Cut/Copy/Paste) | no menu, 3/3 | pass — fixed |
| 3 filled, long press MIDDLE cell | TIN | menu shows but WITHOUT the whole-code selection: caret only, menu is Paste/Select all, paste inserts at the caret instead of replacing, 3/3 (`tin-c3-middle-rec-menu.png`) | no menu at all, 3/3 | flaky/partial — AFTER is strictly better than BEFORE but does not deliver the whole-code selection the PR describes on this cell |
| 3 filled, long press LAST cell | TIN | same as middle: caret menu, paste inserts, 3/3 | no menu, 3/3 | flaky/partial — same note |
| 4 filled, tap middle cell, Backspace | OTP | digit before the tapped point removed (123456 → 13456, the '2'), 3/3 | caret stays at the end: last digit removed (123456 → 12345), 3/3 | pass — fixed |
| 4 filled, tap middle cell, Backspace | TIN | digit before the tapped point removed, 3/3 | caret at end: '9' removed, boxes 12345678+caret, 3/3 | pass — fixed |
| 5 fast typing, 6 digits <150 ms apart | OTP | in order, 3/3 | in order, 3/3 (video frames; the auto-submit then clears the wrong code — expected) | pass both — no difference |
| 5 fast typing | TIN | in order, 3/3 | in order, 3/3 (video frames; digits append strictly left-to-right) | pass both — no difference |
| 6 sign-in end to end | OTP | pasted the mock's real code, reached the next page ("Who will use eBIRForms"), `otp-c6-next.png` | (AFTER-only cell) | pass |

### Notes on the evidence

- `otp-c5` BEFORE: the boxes read empty in the stills because the 6 fast digits auto-submitted the (wrong) code and the page cleared them — the videos (`otp-c5v-r*.mp4`) show the digits arriving in append order before the submit. Frame sequences are reproducible from the mp4s.
- The TIN digit classifier confuses 6/5 in this font in a few reads and sometimes reads the caret as a digit; the order verdicts rest on the append-order visible in the video frames, and a visual check of the digit crops (e.g. the '9' in frame f012 of `otp-c5v-r1.mp4` misread as '3' by the classifier).
- One recording per long-press cell at AFTER is included: `otp-c1-r*.mp4`, `otp-c2-r*.mp4`, `otp-c3-r*-(first|middle|last).mp4` (3 runs each), `tin-c1-r*.mp4`, `tin-c2-r*.mp4`, `tin-c3-(first|middle|last).mp4`, plus `otp-c6.mp4`.
- The OTP code page is not visible to uiautomator in either build (known since the job-42 sign-in proofs); the system clipboard menu IS visible, which is how the menu results were read. OTP arrivals were verified visually (dark-box strip detection), TIN arrivals by uiautomator text.

## Reproducer assets

`after/` and `before/` hold the raw evidence per build, including `harness.log` timing logs. The matrix drivers are not part of the proof; results JSON is embodied in the table above.
