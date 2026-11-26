# relay-drop measurements — job 19 addendum (e5f7a504 debug APK, emulator-5554 API 35)

Guards held throughout: outside network cut (ping 8.8.8.8 = "Network is unreachable", re-verified
06:47:58 before (a)); every baked URL loopback (account page displays `Server http://localhost:3008`);
BIR_RELAY_URL = http://127.0.0.1:8787 the local mock; mocks reached only through adb reverse; no
BIR_SFTP_LIVE anywhere; no non-local request observed in the mock logs.

## (a) Probe decided at start — CONFIRMED, refused

Times (device):
- app process start, reverse 8787 DOWN: 06:48:05
- `adb reverse tcp:8787` re-added, NO app restart: 06:48:12
- sign-in attempted: 06:50:43; code submitted, outcome: 06:50:59

Result: REFUSED. Exact red line on the email page after the accepted code:
"Not opening the session: The mock relay at http://127.0.0.1:8787 is not listening or /v1/health
did not identify a mock; the submission worker was not started." (`a-after-signin.png`)

Proof the refusal is the cached verdict, not the network: reverse was UP for the whole sign-in
(the account mock was reached through it — code 246816 issued and accepted), and a device-side
raw-TCP probe of 127.0.0.1:8787 through the same reverse returns the correct health JSON in
~300 ms. The process's door had booted at 06:48:05 while 8787 was down; the verdict never
re-evaluated. Matches `guards.rs relay_target` cached in `door.rs process()`.

Side observation: after sign-out + force-stop + relaunch, the app opened directly into Home
signed in again — sign-out does not clear the local session token. Worth a look; unexpected.

## (b) and (c) — BLOCKED, root cause is a probable v1 finding: the RDO picker on Android
## offers no valid RDO

A 1601C cannot be queued: validation has exactly one open item, "RDO is required /
Please enter a valid RDO Code on Item 7." Every other field is filled (EOPT tier, amounts,
radios, TIN, address, ZIP, phone, month, ATC). The RDO control is the blockage:

- Opening it shows a small bottom sheet containing exactly TWO rows: "Choose…" (the empty
  option) and "000" (the placeholder). No search field renders, no keyboard ever appears
  (mInputShown=false with the sheet open). Screenshots: `rdo-sheet-open.png`,
  `rdo-sheet-detail.png`.
- Picking "000" is rejected by validation ("Please enter a valid RDO Code on Item 7.") —
  as designed, since 000 is the placeholder.
- No scroll, fling, drag, DPAD or typed input reveals further rows; aggressive flings dismiss
  the sheet instead.
- Code read (not a patch, orientation only): the rows come from `bir.list_rdos()` via
  `shell/store.ts rdoOptions()`, which on ANY throw returns `[]` and caches that forever —
  so a sheet of just the placeholder is exactly what an empty/failed list produces. Whether
  `list_rdos` throws or returns empty on Android is not directly measurable from the harness.
  Note validation itself is independent (`reference::get_rdo` in Rust): if validation still
  knows the official list, any correct value reaching the box would pass — but the UI offers
  no way to input one.

Consequence: (b) relay-drops-mid-send and (c) relay-drops-while-idle cannot run — no return
can be queued through the Android UI in this build. If the same sheet works on desktop,
this is Android-specific and v1-blocking for phones; recommend a quick check of `list_rdos`
in the packaged Android runtime.

Files: `a-notes.txt` (timeline), screenshots listed above.
