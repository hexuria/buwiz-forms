# android-otp-input-3 — OTP/TIN selection-fix proof (measure only)

## Header
- app commit: hexuria/buwiz-forms `ee3e649d` (main)
- sibling nsh: hexuria/nsh `fix/otp-input-android-selection` @ `151dcf3d1a6bf7edce316a1acbe20ea34417471a` (real copy, nsh-ref; no nsh update)
- tool: nsh CLI built at 151dcf3d in `tools-151dcf3d`
- runtime classes.dex sha256 (re-hashed from the packaged APK): `f8dc8452e8e7cbe6d2ce0c0522588709c648ac8ba2d1d779f02fae9969255920`
- app CARGO_TARGET_DIR: `~/BuwizNsh/target-ee3e649d`
- safety env baked: EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BUWIZ_NO_KEYCHAIN=1 BIR_SFTP_DRY_RUN=1 (never BIR_SFTP_LIVE)
- device: emulator-5554, API 35 phone, gesture nav. 150 ms hold-taps throughout.
- nothing queued; demo taxpayer / invented digits only; scratch database.

## Case results (one line each)

1. **Cut one digit — WORKS.** Type 123, long-press digit 1: Android selects the whole run — all three boxes draw selected (grey), floating menu offers Cut. Cut empties all boxes; clipboard holds "123", proven by pasting into Registered name (field shows `123`). See c1-longpress-digit1.png, c1-after-cut.png, c1-name-pasted.png.
2. **Long-press on the middle and on the last digit — DOES NOT WORK.** Long-press over box 2 or box 3 finds no word: the menu that opens is the no-selection variant (Paste / Select all, no Cut/Copy), nothing is selected in any box, and the caret handle sits at box 1. The digits of the hidden field are still laid out in a strip at its left edge, so only presses over the leftmost glyphs hit text. See c2-longpress-digit2.png, c2-longpress-digit3.png.
3. **Select all, then Cut — WORKS.** Select all (from the no-selection menu) selects the whole run — boxes 1–3 grey, menu now offers Cut/Copy — and Cut empties the boxes; the clipboard chip shows "123". See c3-selectall.png, c3-after-cut.png.
4. **Caret — WORKS.** Type 123, tap right after digit 2, one Backspace: digit 2 is deleted, boxes show `1` `_` `3` (not last-digit deletion). Then tapping a box and typing a digit places it there. See c4-caret-after-digit2.png, c4-after-backspace.png, c4-typed-in-box.png.
5. **Paste — PARTIAL: nine digits raw WORKS; the other three formats NOT MEASURED.** Clipboard set by typing 123456789 into the TIN, select-all + Copy inside the TIN itself (the only reliable clipboard channel the harness has — Android gives no clipboard-set on unrooted emulators, and non-digit formats cannot be produced this way). Boxes cleared to empty, long-press Paste → boxes fill `1 2 3 - 4 5 6 - 7 8 9` complete and in order. Evidence: c5-tin-pastemenu.png (empty boxes + Paste menu), c5-pasted-final.png (filled). Dashed, spaced and fourteen-digit pastes need a clipboard-setting test hook or manual verification — reported open, not faked.
6. **Interrupted entry — WORKS.** 20 consecutive TINs typed digit-by-digit at ~0.4–0.5 s per digit (natural keyevent latency); at iterations 1, 10 and 20 the boxes are complete and in order `123-456-789` (c6-tin1.png, c6-tin10.png, c6-tin20.png), no lost or reordered digits.

## The author's two picture questions

(a) **Where the selection handles are drawn:** confirmed misplaced as the author expected — in c1-longpress-digit1.png the two teal handles hang at the far LEFT edge below box 1 (at the hidden digits' strip), not under the boxes whose digits they delimit. They work (selection itself is correct) but are misplaced.

(b) **Dragging a handle:** NOT MEASURED — the handles sit at the left edge strip where the hidden digits are; drag gestures there did not map cleanly onto box digits in the harness, and time ran out before a reliable drag measurement. Left open rather than faked.

## Artifacts
- mp4: j30-part1.mp4 (cases 1–early 5), j30-part2.mp4 (remainder) — taps shown by system.
- notes: red error in several shots is the 9-digit validation firing on the partial 3-digit TIN; expected with invented short input.
