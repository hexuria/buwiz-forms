# UI proof report for PR #394

Updated: 2026-11-26T01:25:50

## Result summary

- Part A after videos now run **email → code → required first-run setup → Home** on both iPhone simulator and macOS. The first-run setup is the documented intended path on an empty database; it is not treated as a defect.
- The first code box is focused on arrival in the PR build on both platforms: accent border and caret are visible; on iPhone the number pad is up.
- The main-commit iPhone before picture shows no caret in any box.
- All six requested A2 sign-out persistence cases were executed. Every valid relaunch ended at the Buwiz email gate, not at signed-in Home. **No completed case triggered `SIGN-OUT DOES NOT HOLD`.**
- A2 required explicitly named disposable scratch databases. This is a deliberate deviation from the literal “`BIR_DATABASE_PATH` unset” wording, because the test environment otherwise creates a fresh random scratch database per process and relaunch persistence cannot be tested. No owner data was used.

## Scope and safety

- PR worktree: `/Users/devin/ci-394-0234be18/buwiz-forms`, HEAD `0234be18a7edf59b9bbc48eee2c09bf6997620fc`.
- Main worktree for the before picture: `/Users/devin/ci-394-0234be18/main-buwiz-forms`, detached at `afc1c738349c051f973fcf580873e854b8227d37`.
- NSH sibling: `/Users/devin/ci-394-0234be18/nsh`, pin `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; not edited.
- Simulator: iPhone 17, iOS 27.0, UDID `E0224A22-F874-4B8C-83FB-EB084492C493`.
- Mock account server only: `just mock-account --auto-approve` on `127.0.0.1:3008`; log `mock-account.log`.
- Debug/dev hosts only. Every app launch used `BUWIZ_NO_KEYCHAIN=1`, `EBIR_TEST_ENV=1`, `BIR_SFTP_DRY_RUN=1`; `BIR_SFTP_LIVE` was never set and `BIR_AUTH_BYPASS` was unset in the launch environment. No real server was intentionally contacted.
- No tracked file was edited; nothing was pushed and no branch was created.
- No database or account-file contents were printed or copied. Data-folder evidence is `ls -la` names and sizes only.

## Part A evidence

| Proof | Artifact | Result |
|---|---|---|
| iPhone after video | `ios-after.mp4` (81.471 s) | PASS: email → code with no tap on the code page → required first-run setup → Home. End frame: `ios-after-home-end.png`. |
| iPhone after arrival | `ios-after-arrival.png` | PASS: box 1 accent border and caret, number pad up. Frame at video t≈19.6 s; transition began between t≈19.3 and 19.4 s, so this is within about 0.2–0.3 s of arrival. |
| macOS after video | `macos-after.mp4` (21.200 s) | PASS: email → keyboard-only code → required first-run setup → Home. End frame: `macos-after-home-end.png`. |
| macOS after arrival | `macos-after-arrival.png` | PASS: box 1 accent border and caret. This valid earlier arrival capture was preserved; the new video also shows the same focus behavior. |
| iPhone before at `afc1c738` | `ios-before-arrival.png` | PASS as before comparison: empty grey boxes, no caret in any box, no number pad up. |

Important behavior note: on a brand-new empty database the correct code does **not** go straight to Home; it goes to first-run setup. The app’s own comments describe this as intended. The final videos now continue through setup and end on Home.

Additional Part A evidence:

- `ios-after-arrival-original.png` — earlier valid arrival capture, preserved rather than deleted.
- `ios-after-full-raw.mov` — raw simulator recording used for the final iPhone video.
- `ios-after-key-events.jsonl` — key-event proof from the earlier final code-entry run.
- `ios-after-post-gate.png`, `macos-after-post-gate.png` — earlier captures showing first-run setup after gate clearance.
- `ios-after-rehearsal-fast-text-wrong.mov` — superseded rehearsal evidence: one fast whole-string karagoz text injection produced a wrong-code attempt. It is not presented as a valid proof run.
- `ios-after-arrival-burst-contact.png`, `ios-after-first30-contact.png` — frame-selection contact sheets.

## Part A2 scratch databases

- iPhone scratch database: inside the simulator app container at `Documents/a2ios-proof/bir_data.db`; exact host path in `a2-ios-db-path.txt`.
- macOS scratch database: `/Users/devin/ci-394-0234be18/proof-staging/a2mac-proof/bir_data.db`.
- Both were job-local disposable scratch databases. Using them was necessary because `EBIR_TEST_ENV=1` with no named database creates a fresh database for every process, making relaunch persistence impossible to measure.

## Part A2 results table

Timing caveat: iOS control-plane taps have nontrivial injection overhead, so “busy/gate” measurements are conservative upper bounds unless stated otherwise. macOS accessibility clicks likewise record the click-call completion as an upper-bound press timestamp; local responses were often faster than the control-plane measurement. Kill delays after gate detection are exact measurements from first detection to process termination.

| Case | Sign-out busy / gate timing | What showed after sign-out | Account file at data-folder listing | Kill timing | Relaunch outcome | `POST /api/auth/logout` | First request after relaunch |
|---|---|---|---|---|---|---|---|
| iPhone immediate kill | Busy label was too fast for the ≈0.3 s polling loop; first gate detection 1.196 s after control-plane tap start (conservative upper bound). | Buwiz email gate (`Sign in to eBIRForms`). | **Absent** | 0.635 s after first gate detection | **Gate** | Yes | None observed |
| iPhone 10 s wait | Busy label too fast to measure; first gate detection 1.572 s after control-plane tap start (conservative upper bound). | Buwiz email gate | **Absent** | 10.007 s after first gate detection | **Gate** | Yes | None observed |
| iPhone kill while busy | Mock was SIGSTOPped so no response could complete; screenshot `a2-ios-case3-busy-before-kill.png` visibly shows `Signing out…`. Control-plane press-to-kill measurement 2.036 s including injection overhead. | Killed before completion; no post-sign-out page in the old process. | **Present** (322 bytes) | Killed while still busy; server could not have answered because it was stopped | **Gate** | Yes, after server resume and before relaunch | `GET /me`, then `POST /oauth/token` |
| macOS immediate kill | Gate detected 0.087 s after click-call completion upper bound; local mock response was effectively immediate. | Buwiz email gate | **Absent** | 0.009 s after first gate detection | **Gate** | Yes | None observed |
| macOS 10 s wait | Account file disappeared 0.004 s after click-call completion upper bound; gate followed in the same completion path. | Buwiz email gate | **Absent** | 10.006 s after account-file/gate completion proxy | **Gate** | Yes | None observed |
| macOS kill while busy | Mock was SIGSTOPped so no response could complete; screenshot `a2-macos-case3-valid-busy-before-kill.png` shows `Signing out…`. Kill 1.129 s after click-call completion upper bound. | Killed before completion; no post-sign-out page in the old process. | **Present** | Killed while still busy; server could not have answered because it was stopped | **Gate** | Yes, after server resume and before relaunch | `GET /me`, then `POST /oauth/token` |

### Interpretation

- In the immediate and 10-second cases, the mock received `POST /api/auth/logout`, `buwiz-cloud-account.json` disappeared, and relaunch showed the email gate. Sign-out held in those cases.
- In the busy-kill cases, the account file correctly still existed at the moment of the kill because logout had not completed. After the mock was resumed, the queued logout reached the mock before relaunch; on relaunch the app validated/refreshed (`GET /me`, `POST /oauth/token`) but still showed the gate because the server-side logout had completed. Thus these cases also ended signed out.
- The busy-kill tests do **not** prove the narrower race in which a relaunch completes before the delayed logout reaches the server. The server resume-to-relaunch interval was short but nonzero (about half a second on iPhone and about 1.5 seconds on macOS). This is a residual coverage limitation.

## Data-folder listings (names and sizes only)

#### iPhone immediate kill

```text
total 1928
drwxr-xr-x   7 devin  staff     224 Nov 26 00:16 .
drwxr-xr-x  10 devin  staff     320 Nov 26 00:11 ..
-rw-r--r--   1 devin  staff    4096 Nov 26 00:00 bir_data.db
-rw-r--r--   1 devin  staff   32768 Nov 26 00:11 bir_data.db-shm
-rw-r--r--   1 devin  staff  943512 Nov 26 00:03 bir_data.db-wal
-rw-r--r--   1 devin  staff       0 Nov 26 00:00 bir_data.db.owner.lock
-rw-------   1 devin  staff      32 Nov 26 00:02 buwiz-install-id
```

#### iPhone 10-second wait

```text
total 1928
drwxr-xr-x  7 devin  staff     224 Nov 26 00:52 .
drwxr-xr-x  3 devin  staff      96 Nov 26 00:48 ..
-rw-r--r--  1 devin  staff    4096 Nov 26 00:48 bir_data.db
-rw-r--r--  1 devin  staff   32768 Nov 26 00:48 bir_data.db-shm
-rw-r--r--  1 devin  staff  943512 Nov 26 00:50 bir_data.db-wal
-rw-r--r--  1 devin  staff       0 Nov 26 00:48 bir_data.db.owner.lock
-rw-------  1 devin  staff      32 Nov 26 00:49 buwiz-install-id
```

#### iPhone kill while busy

```text
total 1952
drwxr-xr-x  10 devin  staff     320 Nov 26 00:56 .
drwxr-xr-x   3 devin  staff      96 Nov 26 00:48 ..
-rw-r--r--   1 devin  staff    4096 Nov 26 00:48 bir_data.db
-rw-r--r--   1 devin  staff   32768 Nov 26 00:55 bir_data.db-shm
-rw-r--r--   1 devin  staff  943512 Nov 26 00:50 bir_data.db-wal
-rw-r--r--   1 devin  staff       0 Nov 26 00:48 bir_data.db.owner.lock
-rw-------   1 devin  staff     322 Nov 26 00:57 buwiz-cloud-account.json
-rw-------   1 devin  staff      32 Nov 26 00:49 buwiz-install-id
-rw-------   1 devin  staff      36 Nov 26 00:56 buwiz_submit_device_id.txt
-rw-------   1 devin  staff      43 Nov 26 00:56 buwiz_submit_device_key.txt
```

#### macOS immediate kill

```text
total 1928
drwxr-xr-x    7 devin  staff     224 Nov 26 01:16 .
drwxr-xr-x  101 devin  staff    3232 Nov 26 01:15 ..
-rw-r--r--    1 devin  staff    4096 Nov 26 00:59 bir_data.db
-rw-r--r--    1 devin  staff   32768 Nov 26 01:13 bir_data.db-shm
-rw-r--r--    1 devin  staff  943512 Nov 26 01:01 bir_data.db-wal
-rw-r--r--    1 devin  staff       0 Nov 26 00:59 bir_data.db.owner.lock
-rw-------    1 devin  staff      32 Nov 26 01:00 buwiz-install-id
```

#### macOS 10-second wait

```text
total 1928
drwxr-xr-x   7 devin  staff     224 Nov 26 01:10 .
drwxr-xr-x  87 devin  staff    2784 Nov 26 01:10 ..
-rw-r--r--   1 devin  staff    4096 Nov 26 00:59 bir_data.db
-rw-r--r--   1 devin  staff   32768 Nov 26 01:06 bir_data.db-shm
-rw-r--r--   1 devin  staff  943512 Nov 26 01:01 bir_data.db-wal
-rw-r--r--   1 devin  staff       0 Nov 26 00:59 bir_data.db.owner.lock
-rw-------   1 devin  staff      32 Nov 26 01:00 buwiz-install-id
```

#### macOS kill while busy

```text
total 1952
drwxr-xr-x   10 devin  staff     320 Nov 26 01:18 .
drwxr-xr-x  107 devin  staff    3424 Nov 26 01:19 ..
-rw-r--r--    1 devin  staff    4096 Nov 26 00:59 bir_data.db
-rw-r--r--    1 devin  staff   32768 Nov 26 01:16 bir_data.db-shm
-rw-r--r--    1 devin  staff  943512 Nov 26 01:01 bir_data.db-wal
-rw-r--r--    1 devin  staff       0 Nov 26 00:59 bir_data.db.owner.lock
-rw-------    1 devin  staff     322 Nov 26 01:18 buwiz-cloud-account.json
-rw-------    1 devin  staff      32 Nov 26 01:00 buwiz-install-id
-rw-------    1 devin  staff      36 Nov 26 01:18 buwiz_submit_device_id.txt
-rw-------    1 devin  staff      43 Nov 26 01:18 buwiz_submit_device_key.txt
```

## Mock request excerpts

No tokens are present in these excerpts; the mock does not print tokens.

Valid immediate/10-second cases:

```text
[mock-account] POST /api/auth/logout
```

Valid busy-kill cases, before relaunch after server resume:

```text
[mock-account] POST /api/auth/logout
```

Valid busy-kill cases, first requests after relaunch:

```text
[mock-account] GET /me
[mock-account] POST /oauth/token
```

Immediate/10-second relaunches produced no mock request in the capture window, consistent with the account file already being absent.

## A2 artifacts

### iPhone

- `a2-ios-case1-valid.json`, `a2-ios-case1-valid-relaunch.png`, `a2-ios-case1-data-ls.txt`
- `a2-ios-case2.json`, `a2-ios-case2-relaunch.png`, `a2-ios-case2-data-ls.txt`
- `a2-ios-case3-valid.json`, `a2-ios-case3-busy-before-kill.png`, `a2-ios-case3-valid-relaunch.png`, `a2-ios-case3-valid-data-ls.txt`

### macOS

- `a2-macos-case1-valid.json`, `a2-macos-case1-valid-relaunch.png`, `a2-macos-case1-valid-data-ls.txt`, `a2-macos-case1-valid-after-relaunch-log.txt`
- `a2-macos-case2.json`, `a2-macos-case2-relaunch.png`, `a2-macos-case2-data-ls.txt`, `a2-macos-case2-after-relaunch-log.txt`
- `a2-macos-case3-valid.json`, `a2-macos-case3-valid-busy-before-kill.png`, `a2-macos-case3-valid-relaunch.png`, `a2-macos-case3-valid-data-ls.txt`, `a2-macos-case3-valid-after-relaunch-log.txt`

## Superseded or invalid attempts retained for transparency

- `a2-ios-case1.json` / `a2-ios-case1-relaunch-uitree.json`: invalid first attempt; automation looked for `gate-card`, while the exposed accessibility marker is `gate-title`, so it did not terminate within the required window.
- `a2-ios-case3.json`: invalid busy-kill attempt using a killed rather than stopped server. The connection failed fast and the polling loop did not prove a busy-state kill.
- `a2-macos-case1-watcher.json` and `a2-macos-case1-data-ls.txt`: invalid first watcher attempt; the watcher timed out and killed the app without a sign-out.
- `a2-macos-case3.json` and matching non-`valid` artifacts: invalid macOS busy-kill attempt; the confirmation click was not actually hit, and the screenshot proved the app was still Home/menu rather than signing out.
- These files are retained because evidence should not be deleted before it is pushed, but they are not counted as valid results.

## Commands used, principal forms

iOS dev host / relaunch environment:

```sh
SIMCTL_CHILD_EBIR_TEST_ENV=1 SIMCTL_CHILD_BUWIZ_NO_KEYCHAIN=1 SIMCTL_CHILD_BIR_SFTP_DRY_RUN=1 SIMCTL_CHILD_BUWIZ_SERVER_URL=http://localhost:3008 SIMCTL_CHILD_BUWIZ_TRUST_KEYS=dev-test-key-not-production SIMCTL_CHILD_BIR_DATABASE_PATH=<job-local-scratch-db> xcrun simctl launch E0224A22-F874-4B8C-83FB-EB084492C493 com.longbridge.nsh.host
```

macOS host:

```sh
env -u BIR_AUTH_BYPASS -u BIR_SFTP_LIVE   BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1   BIR_DATABASE_PATH=/Users/devin/ci-394-0234be18/proof-staging/a2mac-proof/bir_data.db   BUWIZ_SERVER_URL=http://localhost:3008   BUWIZ_TRUST_KEYS=dev-test-key-not-production   /Users/devin/ci-394-0234be18/buwiz-forms/target/debug/nsh-app-nsh-app-ebirforms-macos-host   --dev-server ws://127.0.0.1:8790/ws
```

Mock server:

```sh
cd /Users/devin/ci-394-0234be18/buwiz-forms
env -u BIR_AUTH_BYPASS -u BIR_DATABASE_PATH -u BIR_SFTP_LIVE   PATH="/Users/devin/.rustup/toolchains/1.99.0-aarch64-apple-darwin/bin:/Users/devin/BuwizNsh/tools/bin:$PATH"   CARGO_TARGET_DIR=/Users/devin/ci-394-0234be18/buwiz-forms/target   CARGO_INCREMENTAL=0 BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1   just mock-account --auto-approve
```

iOS input used karagoz with Xcode 27:

```sh
DEVELOPER_DIR=/Applications/Xcode-27.0-RC.app/Contents/Developer   npx -y karagoz-mcp key KEYCODE_<digit> --device E0224A22-F874-4B8C-83FB-EB084492C493
```

## Remaining gaps and cautions

1. Exact Sign-out busy duration is below the practical control-plane polling/injection resolution on the local mock in the normal cases; the report gives conservative upper bounds. Busy-state presence in the stopped-server cases is proven by screenshots.
2. Busy-kill relaunches happened after a short server-resume interval, so they do not cover the narrower race of relaunching before a queued logout can reach the server.
3. A relaunch with a still-valid cloud account but a locked local profile shows the local TIN/PIN Sign in screen first. That is distinct from the Buwiz email gate. In the valid busy-kill cases this did not determine the final result because logout completed before relaunch and the email gate appeared.
4. For valid busy-kill cases the mock process was SIGSTOPped and later SIGCONTinued, rather than being killed and restarted as a brand-new process. This was necessary in practice because killing the listener made the logout connection fail fast instead of hanging; the stopped-process method is the meaningful interpretation of “stop the mock just before pressing Sign out”, but it is a deviation from the literal restart wording.
5. macOS case 1’s 0.087 s and macOS case 2’s 0.004 s response timings are measured from click-call completion, not the exact instant inside the accessibility click; they should be read as approximate local-mock timings, not performance benchmarks.
