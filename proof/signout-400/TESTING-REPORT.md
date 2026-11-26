# PR #400 Part C runtime proof report

S2 `92de021427aeb0c1d13b40e929e0b28185ca0218`; iPhone 17 simulator, iOS 27.0, UDID `E0224A22-F874-4B8C-83FB-EB084492C493`; debug dev host; local mock `127.0.0.1:3008` only. Safety env: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1`, no `BIR_SFTP_LIVE`, no `BIR_AUTH_BYPASS`; builds used `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`. A named disposable scratch DB was required for relaunch persistence (deviation from literal unset-DB wording). Hanging-server cases used SIGSTOP/SIGCONT, not killing the listener.

## Results

### 1. Server stopped before sign-out — PASS
Mock PID 79739 was SIGSTOPped before sign-out and SIGCONTed only after relaunch evidence. Video `partc-case1.mp4`. Video frames: last confirmation-dialog frame 6.920101s, first post-dialog frame 6.999642s, first gate frame 7.158725s. Thus gate was about 0.079s after the first visible press response and no more than 0.239s after the last pre-press frame: under 1s. Account file was absent after gate. Terminate was 1.233s after tap-call completion. Relaunch showed the gate. Data listing: `partc-case1-data-ls.txt`; timing JSON `partc-case1.json`.

### 2. Server up — gate and relaunch PASS, logout-order expectation CONTRADICTED
Gate appeared immediately and relaunch showed the gate; account file absent. However byte-polling of the mock log observed `POST /api/auth/logout` at epoch 1795688722.070827, before the first accessibility detection of the gate and consistent with video timing (dialog disappeared around video t=2.97s; first gate frame t=3.143s). This does **not** prove the requested “logout arrives after gate” ordering; observed evidence indicates the logout reaches the server before the visible gate on fast localhost. JSON `partc-case2.json`; video `partc-case2.mp4`; contact sheet `partc-case2-early-contact.png`.

### 3. Restore/refresh wait — partial PASS with stated deviation; kill-during-wait result UNEXPECTED
A valid hanging-refresh measurement with server SIGSTOPped showed the gate after 16.253s of polling (17.068s conservative upper bound including tap invocation), within the stated up-to-20s design. Account file was absent and relaunch showed the gate. Deviation: this measurement was taken in the already signed-in process rather than after a fresh stopped-server relaunch, because reliable navigation/setup time made the literal launch-based procedure impractical in this run. JSON `partc-case3b.json`.

A separate kill-during-wait attempt (`partc-case3a.json`) relaunched to the cloud email gate, i.e. it did **not** leave the user signed in. That contradicts the stated-limit expectation, but the attempt is marked questionable because the refresh may not have acquired the account-session lock before sign-out (the press happened very soon after Account opened). This was repeated; see the update below.

## Artifacts
Key evidence: `partc-case1.mp4`, `partc-case1-early-contact.png`, `partc-case1-relaunch-gate.png`, `partc-case2.mp4`, `partc-case2-early-contact.png`, `partc-case2-relaunch-gate.png`, `partc-case3-relaunch.png`, JSONs, data listings, and `mock-excerpt-no-tokens.txt` in this folder.

No real server was contacted; no tracked files were changed; nothing was pushed.


## Repeated kill-during-restore-wait stated-limit case

Repeat artifacts: `partc-case3a-repeat.json`, `partc-case3a-repeat-before-kill.png`, `partc-case3a-repeat-after-relaunch.png`, `partc-case3a-repeat-after-relaunch-late.png`, `partc-case3a-repeat-data-before-kill.txt`, `partc-case3a-repeat-lsof.txt`.

Method: signed-in app was relaunched with the mock server SIGSTOPped. Sign out was confirmed. For at least 2.5s after the confirming tap call completed, the gate did not appear; immediately before termination the account file was still present. This is direct behavioral proof that the kill happened while sign-out was still waiting and before local removal completed. The app was terminated 3.660s after tap-call completion.

Result: after relaunch, the first automatic 3s classification was too early because the stopped-server launch was still blank. A later inspection showed the local TIN/PIN Sign in screen, not the Buwiz email gate. In this app state that means the cloud account survived the kill. Thus the stated limit is confirmed in this repeat: killing during the restore/refresh wait can leave the person signed in after relaunch.

Caveat: the requested positive network proof was only partial. The pre-press `lsof` snapshot did not show an established app->mock connection, so it did not independently prove a hanging `/me`/token request at that exact instant. The proof of being in the wait is behavioral: no gate for at least 2.5s and the account file still present immediately before kill.
