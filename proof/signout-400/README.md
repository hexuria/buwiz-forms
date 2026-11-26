# PR #400 sign-out proof

Target commit: `92de021427aeb0c1d13b40e929e0b28185ca0218` (`92de0218`).  
Proof date: 2026-11-26. Machine: macOS 26.6.2. Simulator: iPhone 17, iOS 27.0.

## Safety

Debug/dev build only, local mock account server on `127.0.0.1:3008` only, `BUWIZ_NO_KEYCHAIN=1`, `EBIR_TEST_ENV=1`, `BIR_SFTP_DRY_RUN=1`; `BIR_SFTP_LIVE` never set and `BIR_AUTH_BYPASS` unset. A named disposable scratch database inside the simulator container was used because relaunch persistence cannot be tested with the default fresh-random scratch database. No database/account/key/token contents are included; data evidence is names and sizes only.

## Results

| Case | Timing/result |
|---|---|
| 1. server stopped, sign out | PASS: gate in about 0.079–0.239 s by video-frame bounds (<1 s); account file absent; app terminated after press; relaunch gate. Video `partc-case1.mp4`. |
| 2. server up | PASS for immediate gate, account-file removal, and relaunch gate; CONTRADICTED ordering: mock observed `POST /api/auth/logout` before the first visible/detectable gate, not after it. Video `partc-case2.mp4`. |
| 3. hanging restore, sign out | PASS with deviation: 16.253 s to gate detection (17.068 s conservative upper bound), within up-to-20 s design; kill 1 s after gate; relaunch gate. Valid timing used the already signed-in process rather than a fresh stopped-server relaunch. |
| 3 limit: kill during wait | PASS/stated limit confirmed behaviorally: no gate for at least 2.5 s, account file still present before kill, killed 3.660 s after tap completion, late relaunch showed local TIN/PIN Sign in rather than cloud email gate, so cloud sign-in survived. Caveat: no simultaneous network-level proof of hanging `/me`/token at exact pre-press instant. |

The mock server was SIGSTOPped/SIGCONTinued where a hanging server connection was required, because killing the listener makes connections fail fast. Full detail: `TESTING-REPORT.md`.
