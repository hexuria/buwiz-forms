# Six-box sign-in code and sign-in PIN at the final kit head

Proof for the boxed digit input in the sign-in flow, app `feat/relay-filing-signin`
@ `e9bd76b1` (pull request #345), built against the kit sibling checkout at
`6149250d` ("otp input: Leave the caret and the selection where the user puts them").

**Build line:** app `e9bd76b1` · kit `6149250d` · nsh `0.6.1 (6149250)` · cargo
`1.100.0 (3acc79c0c 2026-11-05)` · RUSTC_WRAPPER empty · folder
`/Users/devin/rig/sixbox/target` (empty before, used by nothing else).

Run environment: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1
BIR_SFTP_DRY_RUN=1`, the mock account server on localhost:3008, a scratch
database, invented `@example.com` emails only. No real account, never
`BIR_SFTP_LIVE`, never Queue. Input was real OS key events (CGEventPost).

## Files

- `transcript.txt` — every run with its code, the field reads taken through the
  agent plane, the counts, and the mock-server log lines in order.
- `sixbox-cases.mp4` (118 s) — the three single-run cases in one take:
  wrong-digit-then-corrected (juana5), paste (juana3), select-all-delete-retype
  (juana4). Shows the email gate, the six boxes, and each sign-in closing the gate.
  Does not show the five burst runs (they preceded it; the mock log lines for
  codes 246810–246814 with their verifies are in the transcript).
- `pin-burst.mp4` (48 s) — five lock/unlock cycles: Lock app in the account menu,
  TIN boxes filled, the PIN typed as a 20 ms burst, the shell returning.
- `runN-locked.png` / `runN-unlocked.png` — stills of the PIN lock screen (empty
  TIN and PIN boxes) and the unlocked shell for runs 1 and 3.

## What the files do not show

- The five 20 ms burst sign-ins are evidenced by the mock-server log (a start,
  the code line, a verify, a GET /me per run, in order) and the gate closing;
  they are not in the recording.
- keystroke-level timing inside a burst is not visible at 30 fps; the 20 ms gap
  is a property of the injector, stated in the transcript.
- The PIN field is masked; correctness is proven by the unlock, not by reading digits.

## Counts

- correct code, 20 ms burst: 5/5 signed in
- wrong digit corrected by keyboard selection + type-over: 1/1 signed in
- six-digit paste: 1/1 signed in
- select all + Delete empties, retype signs in: 1/1 (field read empty before retype)
- PIN 1234 as a 20 ms burst: 5/5 unlocked
