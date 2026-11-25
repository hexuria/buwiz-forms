# Android OTP input at nsh#39 final head 6149250d — proof (job 21)

Build: app `75bd78b8` (buwiz-forms main) · sibling NSH `6149250d0d650aad1c76ea44f4eb2c26478d08b8` (hexuria/nsh#39 head, clean checkout) · tool `nsh 0.6.1 (6149250)` · both Android runtimes real copies at the same commit in `~/BuwizNsh/tools-6149250d` with `nsh-ref` beside each classes.dex · classes.dex sha256 **taken from the installed APK**: `3f102613d7aa3b3e917d91abf6fd175cad865eb02dccc9598cbff0c484d52f8c` (byte-identical to the 62e436ff runtime — the Java side did not change between those commits) · CARGO_TARGET_DIR `~/BuwizNsh/target-6149250d` (the tools compile used `target-6149250d-tools` for the CLI only). Consistency proof: the APK's `libnsh.so` sha256 `9f32ecac…` is byte-identical to the build artifact in `target-6149250d` and to the file the packager copied — the package's native side is the sibling's compile. One build-folder failure along the way: `cargo xtask android` insists on the checkout-local `target/`, so the first tools build failed before any artifact was produced and was re-run without `CARGO_TARGET_DIR` for the xtask steps (reported per the every-attempt rule).

One emulator install at a time, one build at a time. Taps shown; screenrecord segments for cases 1 and 3 are `case1-rec-*.mp4` and `case3-rec-*.mp4` (170 s segments, concatenated coverage).

## 1. Select one digit and Cut — the fix DOES NOT WORK

The one-digit cut is the case the head commit is meant to repair. The evidence says it is not repaired.

**First digit.** Boxes `1 2 3` (c1a-before.png). Right after the long press on digit 1, BEFORE any dragging: only digit 1 is grey-highlighted with both teardrop handles exactly at box-1's edges and a Cut/Copy/Paste/Share menu (c1a-longpress.png) — so a long press does *not* visibly select the whole run as one word; it appears to select exactly the digit under the finger. Handle dragging is not a usable narrowing mechanism: drags either fail to move the selection or collapse it (c1a-drag-extend.png, c1a-dragged.png). Cutting the one-digit-looking selection **empties all three boxes** (c1a-after-cut.png), and the clipboard — pasted into Registered name — holds **`123`, the whole run** (c1a-clipboard-pasted.png). The visible one-digit selection is not the real selection; the Cut takes everything. Reproduced twice.

**Middle digit.** No person-like selection can be created on digit 2 at all: the long press produces no selection — only a caret near the digit-1/2 boundary with a Paste/Select-all menu (c1b-longpress.png) — and a clean fast double-tap produces no selection either (c1b-doubletap-clean.png). Slow handle-drag retries do not establish a digit-2 selection (c1b-handle-slow-extend-test.png, c1b-handle-slow-extend-settled.png). The cut case is therefore not executable for the middle digit; that is itself the result.

**Last digit.** Same: long press on digit 3 makes no selection (c1c-longpress-firsttry.png), double-tap makes none (c1c-doubletap-clean.png). Not executable.

## 2. Select all + Cut; caret after the second digit + Backspace — both wrong

**Select all + Cut.** After "Select all", the grey highlight still appears over digit 1 only, handles at box 1 (c2-selectall.png) — the visual never shows a full-run selection. Cut empties all boxes and the clipboard holds `123` (c2-selectall-after-cut.png, c2-selectall-clipboard-pasted.png) — the *result* is correct for select-all, but only because cutting everything coincides with selecting everything.

**Caret after digit 2, then Backspace — DOES NOT WORK, the decisive failure.** Boxes `1 2 3` (c2retry-typed123.png). Tap immediately right of digit 2 (c2retry-caret-after2.png). One numeric-pad Backspace removes **digit 3**, leaving `1 2` (c2retry-backspace-after-wide.png) — not digit 2, which would leave `1 3` if the caret truly sat where the user put it. Reproduced in two independent runs. The caret is still effectively pinned to the end.

## 3. Long-press Paste with four formats — WORKS, uniform

Clipboard loaded by typing into Registered name, selecting there, Copy from the menu. All four formats paste into the boxes with separators stripped, filling `123-456-789` left to right:

| clipboard | result in boxes |
|---|---|
| `123456789` (raw nine) | `1 2 3 - 4 5 6 - 7 8 9` (c3-raw-after-paste-wide.png) |
| `123-456-789` (dashed) | `1 2 3 - 4 5 6 - 7 8 9` (c3-dashed-after-paste2-wide.png) |
| `123 456 789` (spaced) | `1 2 3 - 4 5 6 - 7 8 9` (c3-spaced-after-paste-wide.png) |
| `12345678901234` (fourteen) | first nine only: `1 2 3 - 4 5 6 - 7 8 9`; trailing `01234` dropped (c3-fourteen-after-paste-wide.png) |

The TIN pad is the app's own numeric pad; Paste comes from the floating menu on a long press, not the Gboard chip (the chip only exists over Gboard, e.g. in Registered name — it was used to verify clipboard contents). Procedural notes: Android's smart actions add `Call` to the menu on long digit runs (two accidental navigations during setup, retaken — the `2`-suffixed files are the valid ones); rapid repeated backspace taps are unreliable in these boxes and had to be spaced.

## 4. Interrupted entry, 20 TINs — see tinrun/

Same machinery as job 14: hide-keyboard chevron after the 4th digit, tap the next box, type the rest; ~0.4–0.5 s real tap spacing. Results in tinrun/ with a screenshot per TIN — see the report message for the count.

## 5. The six-box sign-in code — skipped

This dev-host build signs in through the baked `BIR_AUTH_BYPASS` device flow; the six-box email-code page is not reachable, same as job 14.
