# Job 17 — sign-in re-proof on 140ea4ba (certified build)

## Build header

| piece | value |
|---|---|
| app commit | `140ea4bac783b389f7bcd4c75eed06c640b3de25` (feat/relay-filing-signin, main merged) |
| sibling NSH checkout | `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (pin, pristine worktree `~/j17/nsh`) |
| `nsh` tool | `nsh 0.6.1 (8c78cfd)` |
| runtime classes.dex sha256 | `a53652883f334dbb2844bd2844fa5246097d2f49acddf068341c43702c46943c` (re-hashed from the packaged APK) |
| CARGO_TARGET_DIR | `~/BuwizNsh/target-140ea4ba` |

Baked list exactly as docs/relay.md on that branch: `EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1;BUWIZ_SERVER_URL=http://localhost:3008;BUWIZ_TRUST_KEYS=dev-test-key-not-production;BIR_DATABASE_PATH=test-data/bir_data.db`, no `BIR_AUTH_BYPASS`. Phone emulator-5554 (API 35, 1080x2400, density 420). `adb reverse tcp:3008`. Fresh `pm clear` before the run. The mock was the one built from this branch (`--network-verifies`/`--network-starts`/`--busy-verify`, per-request codes counting up from 246810). Navigation mode set with `cmd overlay enable-exclusive` only, verified by screenshot (d02 shows the three buttons; gesture shots show the pill). The app was never left with Back between steps; `am force-stop` was used for relaunches.

## Results, one line per item

| item | verdict | evidence |
|---|---|---|
| A | WORKS | first wrong code "That code is not right. 4 tries left." (shots/a02), second "… 3 tries left." (shots/a03) |
| B | WORKS | swipe-away killed the process (pid 17583→18347): reopened on the code page, countdown continued (4:31, shots/b02); force-stop (→pid 18608): same, countdown 4:17 (shots/b03) |
| C | WORKS, with the timing note below | mock lines pasted under step C; same-address continue printed no new line, different address the second, first address the third; first code failed, third worked |
| D | WORKS | gesture+three-button, email and code pages: field and primary button visible above the keyboard (f01/f02, a02, d03/d04, d05); the Continue tap with the keyboard up pressed it (every continue in the flows was tapped with the keyboard up). Landscape: one picture (d01) — Android's full-screen keyboard extract UI, nothing to fix |
| E | WORKS | after the successful code the first-run question opens with no keyboard over it (e01) |
| F | WORKS | Change email returns with the address selected (f01, visible highlight); typing one letter "X" replaces the whole address (f02) |
| G | WORKS, all three sentences exact | g1b / g2a / g3a-g3b, details below |
| H | MIXED | one "Buwiz account" group and sign-out asks once then the sign-in page (h04/h05), BUT the page shows TWO Back controls: the top bar's back arrow+title and a second "‹ Settings" breadcrumb row under it (h03, resource ids `nb-back` and `settings-back` both visible) |
| I | WORKS | taxpayer created (i12), Back from the taxpayer page lands on Home (i13→i14), not back on the New taxpayer form |
| J | MIXED | PIN page after force-stop+reopen, but the keyboard does NOT rise by itself (`mInputShown=false` even after a wait, j03/j04); after one tap on the TIN boxes the keyboard is up and every field plus the Sign in button sits above it (j05). Clipboard chip on the code boxes: fills all six — see below |

## Step C detail — the third line needs the first code dead

The mock's lines from the run (verbatim):

```
[mock-account] sign-in code for maria@example.com: 246810
[mock-account] sign-in code for juan@example.com: 246811
[mock-account] sign-in code for maria@example.com: 246812
```

- Continue with maria, Change email, Continue with maria again: the code page, still ONE line (the app re-enters the live sign-in).
- juan@example.com: second line (246811).
- Back to maria **while her first code still lived**: NO third line. The mock refuses the start (`code_already_sent`, same install, code unexpired with tries left) and the email page shows "A sign-in code was already sent to this address. You can ask for a new one in 4 minutes." (shots/c01-c03 area).
- After the first code expired (its five minutes), the same continue printed the third line (246812). On the code page the FIRST code 246810 then failed with "That code is not right. 4 tries left." (shots/c04 — invalid against the newest request, costing one try of it), and the THIRD code 246812 signed in.

So the sequence works as specified only because steps A+B outlive the five-minute code life. If C is run fast, the first-address restart is refused instead. This is the mock's documented "one wait" rule; flagging it so the spec's expectation is calibrated.

## Step G detail

- `--network-verifies 1`: first wrong code → "That code is not right. 4 tries left." (g1a). Second wrong code → **"Too many codes were tried from this network. Try again in 60 minutes."** (g1b). The limited verify consumed no try: the sentence carries no tries count, and a third wrong code produced the same network sentence, never a "3 tries left" (g1c).
- `--network-starts 1`: first start went through (juan, 246810), the second start → **"Too many sign-in codes were asked for. Try again in 60 minutes."** on the email page (g2a). The surviving sign-in (force-stop+reopen restored juan's code page) then answered a wrong code with the full first decrement, "That code is not right. 4 tries left." (g2b) — the refused starts consumed nothing.
- `--busy-verify`: entering the code → **"Sign-in is busy right now. Try again in a moment."** (g3a); a second entry produced the same busy sentence and no tries count ever appeared (g3b).

(The mock's wait is a fixed hour, so N minutes renders as 60 whatever the flag's count; the flag only sets how many calls pass first. Flags used were `--network-verifies 1` / `--network-starts 1`.)

## Clipboard chip on the code boxes (question in J)

Copied "246810" to the clipboard from a text field. The numeric keyboard on the code page then shows the clipboard chip "246810" in its strip (k02). Tapping it filled all six boxes and fired the verify immediately — proof: the mock was running `--busy-verify` at that moment and the busy sentence appeared right after the chip tap with no typing (k03). So yes, it still fills all six.

## Method notes

- NSH controls on this build often did not react to zero-duration `input tap`; a 150 ms press-and-release (input swipe to the same point) registered reliably. Mentioned as method, not as a defect — but worth one look on a physical device.
- The countdown in uiautomator text lags the screen; screenshots are the truth.
- Error sentences on the code page do not appear in uiautomator text extraction; again the screenshots are the evidence.
- The recording `j17-a-to-c.mp4` is two concatenated segments (screenrecord caps at three minutes): segment one covers A, B and C up to the first-code refusal, segment two the post-expiry restart, the failing first code and the successful third. The wait for the first code's expiry is the gap between them; nothing else is cut.
- `b00`/`b01*` shots document the swipe-away mechanics (Recents needed a fast fling, slower swipes only scrolled or reopened).
- Shots j06/j07 document the TIN/PIN entry: the admin PIN is four digits; the PIN boxes do not take focus unless tapped directly.
