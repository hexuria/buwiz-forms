# Job 19b — relay lifecycle proof, PR #363 at 007d2af1

Build: hexuria/buwiz-forms `feat/relay-filing-lifecycle` at **007d2af1fe0f2bfb8b47d471eb37ce5ef63c923d**
(2f4a1e99 + main c6d27f1d merged in). Debug APK baked per docs/relay.md dev recipe
(`CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true BUWIZ_ANDROID_DEV=1`, x86_64), own target folder
`~/BuwizNsh/target-007d2af1` (since deleted per cleanup rules).
APK sha256: `cfc50bb93006c69e71d1c791c644247d6359dd10f00af0a63c2966aee5667400`.

Baked env: `EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1;BUWIZ_SERVER_URL=http://localhost:3008;BUWIZ_ACCOUNT_URL=http://localhost:3008;BUWIZ_TRUST_KEYS=dev-test-key-not-production;BIR_DATABASE_PATH=test-data/bir_data.db;BIR_RELAY_URL=http://127.0.0.1:8787` — no `BIR_AUTH_BYPASS`, never `BIR_SFTP_LIVE`.

Servers: local mock account (`:3008`) and local mock relay (`:8787`) only, reached through
`adb reverse`. Outside network cut the whole run (`svc wifi`/`svc data` off). Public-host proof
before the first queue: `ping 8.8.8.8` → `connect: Network is unreachable`, same for `1.1.1.1`.
Nothing non-local appears in either mock's log; nothing reached api.buwiz.com or the BIR.

Setup: fresh install (old e5f7a504 APK uninstalled first), real sign-in through the email code
(demo@example.com, mock-issued code), taxpayer created through the app UI: TIN **123-456-788**,
branch **00000**, name "Demo Taxpayer", RDO **050** picked in the create form (the new head
refuses bad TIN check digits — 123-456-788 passes). The reinstall wiped the old taxpayer, so it
was recreated as ordered.

## Step 0 — new return inherits the profile RDO

`19b-item7-rdo.png` (phone width): a NEW 1601C (October 2026), created after the profile already
held RDO 050:

- item 7 "RDO Code" shows **050**;
- the box is **locked** (padlock in the box, group note "From tax profile");
- an **"Edit"** control (pencil) is present at the row's right;
- **no "Differs from your profile" banner** anywhere on the form (the return agrees with the profile).

The return was filled through the UI until Check passes: `19b-check-pass.png` shows the black
**"No problems found."** toast. Filled: month October 2026 (a December 2026 draft was impossible —
validation rejects months later than the current month, correct behavior), item 2 No, item 3 No
(answering No also drops the Part II compensation requirements), address, ZIP 1002, phone, email,
item 11 Private, item 13 No, and the EOPT tier "Micro" — the tier surfaces inside Part II item 32's
penalties block as "Missing in tax profile" and is required even with no tax due. Once set on the
first return it did not reappear on the second.

## (b) relay cut before the send — PASS

Timeline (device clock, UTC+8):

- 12:27:19 `adb reverse --remove tcp:8787` (mock relay itself left running; the device simply cannot reach it)
- 12:27:42 return queued → app shows **"Queued · read-only"** (`b-queued-relay-down.png`)
- ~60 s down: app shows **"Not sent yet: Could not connect to send this return. It tries again by itself. Next try at 12:28 PM."** (`b-not-sent-yet.png`); never anything like "filed"
- 12:28:49 reverse restored
- 12:34:5x the worker's retry lands at the mock relay — **exactly one** `HTTP 202 (979 bytes)` for this return
- the app's status flips to **"Submitted · read-only — Sent to the mock relay Nov 26, 2026, not to BIR"**: the change is on screen by **12:35:0x** (a uiautomator dump at that time reads `Submitted · read-only` plus the toast "1601C for October 2026 was submitted"), i.e. within seconds of the mock's receipt. `b-just-before-send.png` (12:35, mislabeled in the first push) still shows the pre-flip "Queued · read-only — Next try at 12:28 PM". **No frame of the flip moment itself exists**: the b2 video's last frame (12:34) predates it and the (c) recording never shows the October return. What there is: `b-oct-return-submitted-later-capture.png`, the same October return in "Submitted · read-only" state, re-captured from the live app afterwards (same install, same database, still mock-only).

Mock inbox afterwards: exactly one file for the period —
`inbox/1601Cv2018/12345678800000-1601Cv2018-102026#demo@example.com#.xml` — and exactly one receipt.
Watched a further 60 s: no second submission.

Video: `j19b-b-queue-down.mp4` (queue + down period), `j19b-b2-send.mp4` (retry-wait window; the
actual send moment lands just after this segment ends — the states before/after are in the screenshots).

Note on the mock, for honesty: the original mock-relay process (started in the e5f7a504 job) had its
inbox inside a since-deleted build tree, so it was restarted at 12:33 with an absolute inbox path
(`~/j19b-relay-inbox`). Its earlier log shows no submission of any kind before the restart (only
0-byte health responses), so nothing was lost or double-counted. The mock's full log for both
returns, no tokens present:

```
[mock-relay] HTTP 202 (979 bytes)   <- October return, one line
[mock-relay] HTTP 202 (979 bytes)   <- November return, one line
```
(all other lines are `HTTP 200 (0 bytes)` health responses)

## (c) relay down mid-session, second return queued while down — PASS

- 12:36:30 reverse removed again, app open and signed in
- a second NEW 1601C (**November 2026**) was created, filled and checked while the relay was down
- 12:40:25 queued while still down → same honest status: "Not sent yet: Could not connect … Next try at 12:41 PM." (`c-queued-relay-down.png`); no crash, no "filed"
- 12:40:37 reverse restored
- 12:41:2x the retry sends it — second `HTTP 202 (979 bytes)` in the mock log
- app shows **"Submitted · read-only"** (`c-after-restore.png`); `dumpsys` confirms the app activity still top-resumed — no crash anywhere in the run

Final exactly-once count after a 75 s watch:

- inbox files: **2** (one per period: `-102026` and `-112026`)
- receipts: **2**
- `HTTP 202` lines in the mock log: **2**

No return was ever received twice.

Video: `j19b-c-queue-down.mp4`, `j19b-c2-send.mp4`.

## PASS/FAIL table

| step | result | mock-log count |
|---|---|---|
| 0 — item 7 inherits 050, locked, Edit control, no differs-banner; Check passes | PASS | — |
| (b) relay cut before send | PASS | return received exactly 1×; app status matches |
| (c) relay down mid-session, second return queued while down | PASS | exactly 1×; no crash; never "filed" while down |

## Notes

- `obsolete-e5f7a504-*.png` are the equivalent item-7 shots taken on the superseded e5f7a504 build
  before the head moved; kept for reference, not part of the verdict.
- One harness-level observation, not a product finding: Gboard autocorrect mangles digits typed
  into text-search fields via `input text` (e.g. "050" → "05o"); keyevent digit entry avoids it.
- Sign-in OTP entry works fine when tapping the boxes' true center (y≈652); documented in
  relay-lifecycle-android-2's digit-entry checks.
