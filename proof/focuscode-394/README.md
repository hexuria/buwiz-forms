# PR #394 focus-on-arrival proof

Target commit: `0234be18a7edf59b9bbc48eee2c09bf6997620fc` (`0234be18`).  
Before comparison: `afc1c738349c051f973fcf580873e854b8227d37` (`afc1c738`).  
NSH pin: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`.  
Proof date: 2026-11-26. Machine: macOS 26.6.2. Simulator: iPhone 17, iOS 27.0, UDID `E0224A22-F874-4B8C-83FB-EB084492C493`.

## Safety

All runs were debug/dev builds against only the local mock account server on `127.0.0.1:3008`. The launch environment had `BUWIZ_NO_KEYCHAIN=1`, `EBIR_TEST_ENV=1`, and `BIR_SFTP_DRY_RUN=1`; `BIR_SFTP_LIVE` was never set and `BIR_AUTH_BYPASS` was unset for the sign-in/sign-out proofs. No production account server, BIR server, or relay was used. Data-folder evidence is names and sizes only; no database, account-file, key, token, certificate, or `.env` contents are included.

## Steps and timing

| Proof | Artifact | Time/result |
|---|---|---|
| iPhone after video | `ios-after.mp4` | 81.471 s: email → code typed with keys only and no tap on the code page → required first-run setup → Home |
| iPhone arrival picture | `ios-after-arrival.png` | Captured about 0.2–0.3 s after the code page arrived: box 1 has the accent border and caret; number pad is up |
| iPhone Home end frame | `ios-after-home-end.png` | Signed-in Home after first-run setup |
| macOS after video | `macos-after.mp4` | 21.200 s: email → keyboard-only code → required first-run setup → Home |
| macOS arrival picture | `macos-after-arrival.png` | Box 1 has the accent border and caret |
| macOS Home end frame | `macos-after-home-end.png` | Signed-in Home after first-run setup |
| iPhone before picture | `ios-before-arrival.png` | At main `afc1c738`: empty grey boxes, no caret in any box, no number pad up |

On a brand-new empty scratch database the correct code leads first to the required first-run setup, then Home. That is the app's documented first-run path.

## Sign-out persistence (A2)

A2 used named disposable scratch databases because the default `EBIR_TEST_ENV=1` mode with no named database creates a fresh random scratch database per process, making relaunch persistence impossible to test. This is a deliberate deviation from the literal "`BIR_DATABASE_PATH` unset" wording; no owner data was used. For busy-kill cases the mock was SIGSTOPped/SIGCONTinued rather than killed/restarted, because killing the listener made logout fail fast instead of hang.

| Case | Sign-out busy / gate timing | Account file when listed | Kill timing | Relaunch |
|---|---|---|---|---|
| iPhone immediate kill | Busy too fast for ≈0.3 s polling; first gate detection 1.196 s after tap start, conservative upper bound | Absent | 0.635 s after first gate detection | Gate |
| iPhone 10 s wait | Busy too fast; first gate detection 1.572 s after tap start, conservative upper bound | Absent | 10.007 s after first gate detection | Gate |
| iPhone kill while busy | `Signing out…` visible; server stopped, press-to-kill 2.036 s including control overhead | Present (322 bytes) | Killed while still busy | Gate |
| macOS immediate kill | Gate detected 0.087 s after click-call completion upper bound | Absent | 0.009 s after first gate detection | Gate |
| macOS 10 s wait | Account file disappeared 0.004 s after click-call completion upper bound | Absent | 10.006 s after completion proxy | Gate |
| macOS kill while busy | `Signing out…` visible; kill 1.129 s after click-call completion upper bound | Present | Killed while still busy | Gate |

Every valid relaunch ended at the Buwiz email gate, not signed-in Home. `POST /api/auth/logout` arrived in every valid case. In busy-kill cases it arrived after server resume and before relaunch; those runs do not cover the narrower race where relaunch finishes before the delayed logout reaches the server.

The detailed local evidence report is `TESTING-REPORT.md`. Data-folder listings are the `a2-*-data-ls.txt` files and contain names and sizes only.
