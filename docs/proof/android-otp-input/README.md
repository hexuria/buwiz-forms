# Job 14 — TIN digit loss and long-press paste, before/after hexuria/nsh#39

**READ THE REDO SECTION.** The first AFTER run recorded here was a mixed build (native from
8c78cfd6, Java runtime from 62e436ff) and is INVALID; it is kept only because the crash it
produced is what diagnosed the mix. The valid evidence is under `redo/`.

## Builds

BEFORE: app `75bd78b8` (origin/main), sibling nsh `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`
(clean worktree), nsh tool and runtime from the same pin; dev classes.dex sha256
`73b3126db70bdca7e03f0ee3acb39c63437efc1f606c7232d0324445a1788e17`; CARGO_TARGET_DIR
`~/BuwizNsh/target-8c78cfd6` (this build only).

AFTER: app `75bd78b8` (same commit), sibling nsh `62e436ff170ed6652b05815deb6799077285cad2`
(clean worktree, PR #39 head), nsh tool and runtime from 62e436ff; dev classes.dex sha256
`3f102613d7aa3b3e917d91abf6fd175cad865eb02dccc9598cbff0c484d52f8c`; CARGO_TARGET_DIR
`~/BuwizNsh/target-62e436f` (this build only). The shared `~/BuwizNsh/tools` never left the pin.

History: the FIRST BEFORE run (and the first AFTER) were built in the SHARED
`~/BuwizNsh/target-nsh`; the orchestrator ruled both void because cargo cannot distinguish two
checkouts of the same package version in one target dir. The first AFTER additionally had the
Java runtime from 62e436ff against native code from 8c78cfd6 — SetSelection gained a third int
in #39 — which is exactly the `BufferUnderflowException` recorded in
`paste-after-crash.logcat.txt`.

## Consistency proof for the AFTER build (what was checked)

#39 adds no new string literal or exported function name in non-test code, so a plain
`strings`/symbol check has nothing unique to find — checked the diff of otp_input.rs, ops.rs,
core/mod.rs, protocol.rs, element_ref.rs and encode.rs for one. What was verified instead:
1. The packaged `lib/x86_64/libnsh.so` (352MB, unstripped dev) was built in the pair checkout
   whose `../../nsh` is a clean worktree at 62e436ff (`git -C pair/nsh log -1` output), see
   build log evidence.
2. Behavioural: the edit menu on long-press — the PR's own purpose, per its diff comment
   "disabling it (caret(false)) ... cost the field its edit menu — a long press showed no
   Paste" — appears in the AFTER build and never in BEFORE.
3. The `BufferUnderflowException` never occurred in the consistent build across all runs
   (logcat checked after each).
4. The first-run AFTER screenshots in the root of this folder are the INVALID mixed build.

## Results (redo/)

- TIN digits, interrupted sequence, BEFORE: **20/20 complete and in order** (`before-tin-20-sheet.png`) — the loss did NOT reproduce in the consistent BEFORE build at n=20; saying so plainly, that is a result. No failures, so nothing to list per failure.
- TIN digits, interrupted sequence, AFTER: **20/20 complete and in order**
  (`after-tin-20-sheet.png`).
- Tap spacing: `sleep 0.12` between taps batched inside one `adb shell`, but each adb `input`
  invocation costs ~300-400ms, so real spacing was ~0.4-0.5s in BOTH builds, same scripts.
- The interruption after the 4th digit: keyboard-hide chevron then tap on the next TIN box,
  because the numeric pad has no "?123"/"ABC" layout key (documented in the first run).
- Paste, BEFORE: long-press of the first TIN box shows NO menu at all — confirmed again on the
  consistent redo BEFORE build (`paste-before-check.png`, one long-press with content in the
  boxes: no menu, no selection), matching the first-run evidence.
- Paste, AFTER: the Android floating edit menu APPEARS. With text selected: Cut/Copy/Share
  (plus smart actions in the overflow). On empty boxes with an empty clipboard: only
  "Autofill". With a non-empty clipboard: **Paste appears** (`paste-after-menu-proof.png`)
  and tapping it fills the boxes (`paste-after-result-proof.png`).
- Paste formats: NOT measurable per format on this emulator — see the caveat.

## Caveats (read before trusting)

- Clipboard loading is the limit of this evidence. ctrl+a/ctrl+c/ctrl+x key combinations do
  NOTHING in these fields (in `paste-after-ctrlx-fails.png` the name field still shows the
  text after ctrl+x). The four "Paste appears" menu shots for the spec strings
  (`paste-after-strings-menu.png`) prove only that the clipboard was non-empty — it still held
  the earlier "777" — not which string. The one clean end-to-end paste was loaded with the
  field's own Cut menu item: paste filled boxes 1-3 with the cut digits. Whether the input
  strips dashes or spaces from `000-123-453`, `000 123 453` or truncates `00012345300000`
  is UNMEASURED — no way to place those exact strings on the emulator clipboard was found
  (long-press on the Registered name field shows no menu; double-tap starts a selection but
  offers nothing; there is no clipboard-chip on the app's own numeric pad, and GBoard showed
  none either).
- The TIN pad is the app's own numeric pad, not a system keyboard; there is no clipboard chip
  on it in either build.
- Sign-in code boxes: not tested — these dev-host builds don't bake BUWIZ_SERVER_URL/
  BUWIZ_TRUST_KEYS and the mock sign-in is the device-approval flow, so the six-box email
  code page is unreachable (same as the first run).
- Two harness desyncs happened mid-run (the pad closed and later taps cascaded into other
  fields — a test-driver failure, not an app defect); the affected TINs were re-run. The
  batches after the first used a pixel-check that re-navigates if the pad is not open.
- Recordings: `tin-after.mp4` covers the first AFTER TIN run including the desynced stretch;
  `tin-after-batches.mp4` covers the batch re-runs; `paste-after.mp4` the paste investigation.
  All are emulator `screenrecord` segments (175s cap) concatenated with `ffmpeg -c copy`.
  BEFORE recordings are `redo/tin-before*.mp4`.
- One oddity noticed in passing, not investigated: cutting one selected digit from a 3-digit
  entry emptied all three boxes.

## First-run files (mixed/void evidence, kept for the record)

`tin-before.mp4`, `tin-after.mp4` (root), `paste-before.mp4`, `paste-after.mp4` (root),
`*-sheet.png` in the root — the first run. The BEFORE half of it was consistent in pin terms
(sibling, tool, runtime all 8c78cfd6) but is void under the one-target-per-checkout rule; the
AFTER half was both shared-target AND pin-mixed. `paste-after-crash.logcat.txt` is the
BufferUnderflowException that mixed build produced — the crash that diagnosed everything.
