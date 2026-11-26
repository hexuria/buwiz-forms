# Money-box hunt — raw results (in progress)

Build: 7eacb1d2195b8dc1e1a1b65ad97728c15b50bc2f (fix/money-box-keep-text-on-focus), dev recipe CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true BUWIZ_ANDROID_DEV=1, x86_64, bake EBIR_TEST_ENV=1;BIR_AUTH_BYPASS=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1. Emulator-5554 API35, outside network cut, show_touches on. Where: 2551Q Schedule 1 row 1 Taxable Amount (PT010, rate 3.0). On-screen keypad taps only, taps inside box uiautomator bounds. Keypad keys measured: digits x135/405/675 rows y1721/1873/2026/2179, comma (135,2179), dot (675,2179), backspace (941,2026), check (941,2179).

## Zero-box-on-focus: CONFIRMED empty (h04-box-focused.png) — the box holding 0.00 shows empty (₱ only) when focused.

## Slow typing baseline: every key in order, 15000.50 -> per-key 1/15/150/1500/15000/15000./15000.5/15000.50 — correct.

## H1 fast runs (chained input tap, minimal delay) — focused-shown right after typing:
- 15000.50 -> "1500.050" WRONG (first ever fast run: "15.00050" WRONG)
- 99999.99 -> "99999.99" correct
- 100000.01 -> "10000.001" WRONG
- 1000000.005 -> "1000000.005" correct

## H2 1000000.005 x10 fast runs:
run1 100000.0005 WRONG; run2 correct; run3 correct; run4 correct; run5 correct; run6 correct; run7 correct; run8 100000.0005 WRONG; run9 correct; run10 100000.0005 WRONG. => 3/10 misordered. Pattern always the same: the dot lands one digit-position early (e.g. 1000000.005 -> 100000.0005; 15000.50 -> 1500.050 or 15.00050). Digits never reorder among themselves, never drop; the decimal point leapfrogs left past one (or two) zeros.

## Pipeline consistency (a correct-typed run of 12345678.90):
- focused shown: 12345678.90 (plain, no separators while typing)
- after keyboard check key: 12,345,678.90 (separators on leaving)
- row summary after Done: "PT010 · 12,345,678.90 · 3.0 · 370,370.37" — SAVED == SHOWN
- Tax Due 370,370.37 = correct 3.0% math

## H2b — 12345678.90 fast pipeline (one run, happened to type correctly):
- focused: 12345678.90 -> check key -> 12,345,678.90 -> saved row summary "PT010 · 12,345,678.90 · 3.0 · 370,370.37". SAVED == SHOWN. Tax due correct.

## Misorder consistency probe: inconclusive — while attempting to produce a wrong-order run after reopening the row, digits appended to the leftover formatted value ("100000000512,345,678.90"): refocusing the filled box placed the caret at the START and the backspaces before it did not clear the box. Consistent with the "Backspace dead / caret weirdness in a filled formatted box" family — NOT fully isolated, needs the list-2 protocol. No "the box shows …, but the draft stored …" error was ever seen in any run.

## H3 comma-decimal locale: NOT COMPLETED. Setting de-DE via cmd locale set-app-locales then force-stop+relaunch brought the app up at the FIRST-LAUNCH screen with all data gone (scratch database per launch? — job19 data had persisted across force-stops, so this needs explanation; flagged as an open observation). Locale was reset. Taxpayer recreated. The comma-key behavior in a comma locale is untested.

## H4-H7 and list 1-6: NOT RUN — session budget exhausted after H1/H2. The recordings: h1.mp4 (170 s cap, covers the first fast runs), h2.mp4 (170 s cap, covers H2 repetitions). Screenshots: h04-box-focused (zero shows empty on focus), h1a-typed, h1-slow-final, h1-fast-done, h2-rowsummary, h2-wrong-saved.

## HEADLINE: the fix does NOT cover fast input. At chained input-tap speed (~40-80 ms/key) the decimal point leapfrogs one digit-position left in roughly 30% of runs (3/10 on 1000000.005, plus 15000.50 and 100000.01 in H1). Digits never reorder among themselves and never drop; only the dot jumps. Slow typing (~600 ms/key) is always correct. While focused the box keeps the text as typed (plain digits); on leaving it formats with separators; saved equals shown in every completed pipeline check.
