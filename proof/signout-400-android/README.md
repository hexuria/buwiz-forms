# Job 39 — Android proof for PR #400 (sign-out fix)

- AFTER build: `92de021427aeb0c1d13b40e929e0b28185ca0218` (#400 head)
- BEFORE build: `d6cf0518ab8b8daffab1fad198f81476bb269b08` (main)
- Sibling/tool/runtimes: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (the apps' pin)
- Debug-assertions package recipe, x86_64 emulator, API 35, adb root, taps shown on.
- Baked env keys: `EBIR_TEST_ENV=1`, `BIR_DATABASE_PATH=test-data/signout.db`, `BUWIZ_NO_KEYCHAIN=1`, `BIR_SFTP_DRY_RUN=1`. **BIR_AUTH_BYPASS absent** — the string does not appear in the APKs at all (strings check on libnsh.so). The spec's keys-only check (`files/dev.env`) is not applicable: the dev env is baked into the binary, no dev.env file exists on device.
- **`BIR_DATABASE_PATH` deviation, disclosed mid-job and objected to by nobody:** the prescribed env alone gives every launch a fresh per-PID scratch database, so a force-stop + relaunch shows the gate *even without any sign-out* — the relaunch checks would have been vacuous and step 3's "launched signed in" impossible. `dev_env.rs` allowlists this variable for exactly this case ("an emulator check of a sign-in kept across the app being swiped away"), guarded to `<data_dir>/test-data/`.
- Mock account server rebuilt from the current app checkout, `--auto-approve --network-starts 500 --network-verifies 500`, `adb reverse tcp:3008`. Outside network cut (`svc wifi/data disable`); proof in `network-cut.txt` (`ping 8.8.8.8` → Network is unreachable). Mock log shows only `/auth/email/*`, `/me`, `/api/auth/logout` — nothing non-local. No returns ever queued.
- File lists: names and sizes only. Never contents.
- Times are epoch seconds; gates measured by screencap polling of the gate title band plus screenrecord videos (`j39-step1.mp4` step 1 AFTER, `j39-step3a.mp4` step 3a AFTER, taps shown).

## AFTER `92de0214` — all steps PASS

- **Step 1 (server down): PASS.** Gate shows under 1 s after confirm (video frames: first gate pixels ≈ confirm+0.6 s, `j39-step1.mp4`). Account file gone immediately. Force-stop 1 s after pressing, relaunch → gate.
- **Step 2 (server up): PASS.** `POST /api/auth/logout` hit the mock at confirm+0.335 s (`s2-logout-time.txt`); gate first pixels ≈ confirm+0.2 s, full render ≈ +0.6 s. The local sign-out does not wait for the server. File gone. Force-stop, relaunch → gate.
- **Step 3a (sign-out during hanging launch restore): PASS.** Reverse removed, launched signed in, Sign out pressed within the restore wait. Gate shows at launch+18.7 s (`GATE-SHOWN 1795688757.63` vs `launch 1795688738.97` in `s3a2-times.txt`) — consistent with the designed wait for the restore, up to its ~20 s timeout. Force-stop after the gate, relaunch → gate.
- **Step 3b (force-stop DURING the wait): PASS as measured.** Force-stop at confirm+2.7 s, before the gate. Relaunch → the gate, account file already gone (`s3b-files-after.txt` empty of `buwiz-cloud-account.json`). The local deletion happens at confirm, before the restore wait ends.
- **Control (no press, reverse off, force-stop, relaunch):** app stays on Home 30 s+, account file intact (321 bytes) — a failed restore never signs out by itself, so the relaunch-gate results above are the sign-out's own work, not a confound.

## BEFORE `d6cf0518` — what main does today

- **Step 1 (server down):** gate ≤ confirm+1.15 s, account file gone, relaunch → gate. Quick force-stop races at confirm+0.8 s and confirm+0.15 s both still relaunched to the gate with the file gone in these runs — the deletion lands fast when the server connection is refused; the predicted "can stay signed in" race did not reproduce in two attempts, recorded as measured.
- **Step 2 (server up):** logout POST at confirm+0.22 s (`b2-logout.txt`), gate at confirm+1.19 s (`b2-times.txt`). File gone, relaunch → gate.
- **Step 3a (the visible difference):** gate at **confirm+0.65 s** — BEFORE does NOT wait for the hanging restore at all; the sign-out races ahead of it. Relaunch → gate, file gone and never comes back (server down, the late restore can never finish to re-save).
- **Step 3b:** force-stop at confirm+0.78 s, relaunch → gate, account file gone (same as AFTER in this configuration).
- **Control:** failed restore leaves the app signed in on Calendar with the file intact — same as AFTER.

## Reading the difference

On device with the mock unreachable, BEFORE and AFTER both end signed out in every step. The behavioral change the PR makes is visible in step 3a's timing: AFTER deliberately waits for the in-flight restore (gate at launch+18.7 s, i.e. the restore timeout) so that a late-finishing refresh cannot re-save the sign-in it is racing; BEFORE gates at once (confirm+0.65 s) and lets the restore finish whenever it finishes — which is exactly the window in which a restore that *does* complete (server slow rather than unreachable) can write the account file back after sign-out on main. With the server hard-down that write can never happen, so the resurrection itself is not reproducible in this configuration on either build; the timing difference is the measurable proxy.

## Files

- `j39-step1.mp4` (31.7 s): step 1 AFTER — sign out with server down, gate under 1 s.
- `j39-step3a.mp4` (59.7 s): step 3a AFTER — sign-out during the hanging restore, gate at ~18.7 s.
- `*-times.txt`: epoch-second event logs per step per build (`s*` = AFTER, `b*` = BEFORE).
- `*-files-*.txt`: `find`/size listings before and after sign-out, names and sizes only.
- Screenshots: `gate1.png` (gate after sign-out), `home1.png`/`after-signin.png` (signed-in), `b1q-relaunch.png`, `b3a-relaunch.png`, `b3b-relaunch.png` (BEFORE relaunch states), `control-relaunch.png` (control run), `before-si.png` (BEFORE signed in).
- `network-cut.txt`: proof the outside network was unreachable.
