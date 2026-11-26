# android-pin-focus-probe-1 — job 35: #364 PINPROBE diagnostic (b073de28)

## Header
- app commit: hexuria/buwiz-forms `b073de28` (try/pin-focus-probe = #364's a3aaa36f + PINPROBE console.log lines; diagnosis only)
- sibling nsh: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (pin, real copy)
- tool: `nsh 0.6.1 (8c78cfd)` (tools-8c78cfd-bak)
- runtime: the pin's own dev runtime (see "How it was run" — no packaged APK this time)
- CARGO_TARGET_DIR: `~/BuwizNsh/target-b073de28`
- baked: `EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1;BIR_AUTH_BYPASS=1`
- device: emulator-5554 (API 35 phone), 150 ms hold-taps

## How it was run — DIFFERS from job 25's packaged path, as the spec anticipated

Job 25 was `nsh package --target android` (packaged APK, release runtime). Here `console.log` only reaches the **dev-server console**, so the app ran as `nsh run --target android --port 8792` (dev runtime, development host `com.longbridge.nsh.host` installed on the emulator, adb reverse tcp:8790→host 8792; PINPROBE lines collected from the dev server's log). Two setup notes: the CLI's component check needed `components/android.json` written into the pin's tools distribution (record `{"version":"0.6.1","commit":"8c78cfd"}`) plus the dev runtime copied in as real files — it had been a dangling symlink; `nsh update` was NOT run, per the standing rule. With the bypass baked, the welcome sign-in gate does not appear: the walk is first-run choice → "I help other taxpayers" → PIN page in both passes (same as job 23/25 at this era).

## Result — the mechanism WORKED in both passes here

**`mInputShown=true` at 10 s in BOTH passes** (pass 1 = first launch after the host install, pass 2 = force-stop + reopen), and the probe lines show the same shape both times: the try at 0 ms finds `hadFocus=false`, the focus event lands ~40–50 ms later, and the 500/1500/3000 ms retries all confirm `hadFocus=true gone=false`. No cleanup lines (nothing navigated away), no blur. Pass 1 id `bsca`, pass 2 id `p601`. Contrast with job 25's packaged build at a3aaa36f, where the keyboard never self-rose in four passes — the difference between the dev host and the packaged runtime is itself the diagnostic signal.

## Raw PINPROBE lines, in order (epoch-ms timestamps; verbatim from the dev-server log)

pass 1 (first launch after install):
```
Android Emulator: PINPROBE body bsca t=1795662230182
Android Emulator: PINPROBE ref bsca autoFocus=true t=1795662230182
Android Emulator: PINPROBE try bsca 0 hadFocus=false gone=false t=1795662230220
Android Emulator: PINPROBE focus bsca t=1795662230258
Android Emulator: PINPROBE try bsca 500 hadFocus=true gone=false t=1795662230683
Android Emulator: PINPROBE try bsca 1500 hadFocus=true gone=false t=1795662231683
Android Emulator: PINPROBE try bsca 3000 hadFocus=true gone=false t=1795662233183
```

pass 2 (force-stop + reopen):
```
PINPROBE body p601 t=1795662349529
PINPROBE ref p601 autoFocus=true t=1795662349531
PINPROBE try p601 0 hadFocus=false gone=false t=1795662349552
PINPROBE focus p601 t=1795662349601
PINPROBE try p601 500 hadFocus=true gone=false t=1795662350032
PINPROBE try p601 1500 hadFocus=true gone=false t=1795662351031
PINPROBE try p601 3000 hadFocus=true gone=false t=1795662352533
```

(`pinprobe-raw.txt` has the same lines with the dev-server log's line numbers.)

## Screenshots
- `p1-pinpage-10s.png` — PIN page at 10 s, pass 1 (numeric keyboard up)
- `p2-pinpage-10s.png` — PIN page at 10 s, pass 2 (numeric keyboard up)
- `p1-launch.png`, `p1-pinpage-arrival.png`, `p2-launch.png` — walk states
