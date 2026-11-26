# Job 46 — money-box FINAL candidate 542e4844, Android device proof

- AVD `buwiz`, system image `android-35/google_apis/x86_64`, `ro.build.version.sdk` 35 (Android 15), Gboard `14.2.09.629370537-preload-x86_64` — matches job 41's repro image.
- Build: hexuria/buwiz-forms PR #403 at exactly `542e484401494623a7ccbae9e25832985739a3a8`, debug APK, kit pin 8c78cfd6, docs/relay.md recipe, x86_64, fresh folder `target-542e4844`.
- APK sha256: `cd7627635ca76e1b4bfed3f28d681936a755fa001e1467cceba26e19cfeb5fb7`
- Safety: EBIR_TEST_ENV=1, BUWIZ_NO_KEYCHAIN=1, BIR_SFTP_DRY_RUN=1, BIR_AUTH_BYPASS as in job 36/41, BIR_DATABASE_PATH=test-data/…. NEVER queued. Invented values only (DEMO, TIN 123-456-788/00000 — valid check digit). Network cut. `pm clear` used after the job-42 install; the taxpayer was recreated through the UI. On-screen keypad taps inside the box's bounds; touches shown.
- Target box: 2551Q, Schedule 1, row 1, Taxable Amount (ATC PT010, rate 3.0).

**"the box shows …, but the draft stored …" never appeared in any run.**

## Item lines

1. **F3 changed in 0 of 24** (8 launcher + 8 Recents + 8 with the tax-due cascade live). Box read before any key each time; committed 1,234.56 in the box, focus + keyboard up, Home 5 s, back. Some `post=None` reads in the log are a11y-read timing artifacts (the value reads back on the next trial's pre-read); every trial with two good reads shows unchanged 1,234.56, and the cascade trials additionally verified Tax Due unchanged. Evidence: `f3-trials.mp4`, `f3-*.png`, `evidence-f3.log`.
2. **7b Delete race: PASS** — rows 1,000.00 and 3,000.00, typed 7777 into row 2, tapped row 1's Delete at once with the keyboard up: exactly one row left holding **7,777.00**. Delete does not blur the box first — the row was removed with the keyboard still up; the typed value survived in row 2. Evidence: `7b-race.mp4`, `7b-after-race-type.png`, `7b-after-delete.png`.
4. **First digit after the focus tap into a zero box: lost in 10 of 10 at 100 ms, 5 of 10 at 200 ms, 0 of 10 at 400 ms.** At 200 ms the "lost" reads were wrong digits (`2` instead of `5`): the keypad is still sliding up at that point, so the tap either misses (100 ms — nothing there yet) or lands on a key one row up (200 ms). Once the keypad has settled (400 ms) every digit lands correctly. Full readings in `evidence-cases.log`.
5. **Comma-locale pad (de-DE): the pad drops the comma.** Typed `1 5 0 0 , 5 0` under de-DE — per-key readings `1, 15, 150, 1500, 1500(comma ignored), 15005, 150050` → committed **150,050.00**, not 1,500.50. Same in en-US (case 4). Locale reset to en-US after.
6. **Placeholder: confirmed.** The focused untouched zero box shows a grey `0.00` placeholder (`item6c-empty-focused.png` — grey, vs the black committed `0.00` in the Tax Due read-only below it). Typing `0` shows a black `0` with caret (`item6c-typed-0.png`). After the typed `0` is left, the row summary stores `0.00`; the after-left refocus capture did not land on the box (an ATC picker intercepted the reopen tap), so the placeholder-returning half is supported by the a11y reads (box reads empty of typed text, summary 0.00) but not by a clean picture — stated honestly.

## Cases table (keys → box after each key → after Done → saved → saved after reopen)

Slow per-key typing (≈500 ms apart) is correct everywhere below; per-key readings in `evidence-cases.log`. "saved" = row summary after the sheet closes; "reopen" = box text after reopening the row sheet.

| case | keys | box after Done | saved | after reopen | verdict |
|---|---|---|---|---|---|
| 1 — fast 15000.50 ×10, taps <150 ms | see below | — | — | — | **dot misordered in 2 of 10** |
| 2 — 0.50 | 0 . 5 0 → `0`,`0.`,`0.5`,`0.50` | 0.50 | — | — | pass |
| 2 — 1234.56 | digits in order, `1234.` after the dot | 1,234.56 | — | — | pass |
| 2 — 99999.99 | in order | 99,999.99 | — | — | pass |
| 2 — 100000 | in order | 100,000.00 | — | — | pass |
| 2 — 1000000.01 | in order | 1,000,000.01 | — | — | pass |
| 2 — 12345678.90 | in order | 12,345,678.90 | — | — | pass |
| 2 — 99999999.99 | in order | 99,999,999.99 | 99,999,999.99 | 99,999,999.99 | pass |
| 3 — tap into filled, then `7` `.` ⌫ | `7`→`7123,456.00` (prepends at caret-0), `.` no-op (already has one), `⌫`→`123,456.00` | 123,456.00 | 123,456.00 | 123,456.00 | as designed at this head (caret at start; digits prepend) |
| 4 — `1 5 , 5 0` | `1`,`15`,`15`(comma dropped),`155`,`1550` | 1,550.00 | — | — | **comma key is dropped, not decimal** |
| 4 — `1 , 5 0 0` | `1`,`1`(dropped),`15`,`150`,`1500` | 1,500.00 | 1,500.00 | 1,500.00 | same |
| 5a — tap into 0.00 + digit at once (~150 ms) | digit lost — box stayed empty | 0.00 | 0.00 | — | consistent with item 4's 100 ms result (keypad not yet up) |
| 5b — Next into a filled amount then a digit | — | — | — | — | not reachable: the row sheet has a single amount box and the keypad's ✓ is Done, there is no Next; digit-into-filled is measured in case 3 |
| 5c — type then leave without Done, return | `7` typed; the scrim tap did not close the sheet (sheet stayed open); value `7` still present | — | — | 7 persists | typed value survives in-memory; leave-gesture note honest |
| 6 — Number of Sheet/s 0 → 42 | field read `0`, typed `42` via ctrl+a replace | — | 42 in field | — | pass (`case6-before.png`, `case6-after42.png`) |
| 7 — Backspace to empty, then Done | after ctrl+a + `1234`, five ⌫ reads all show the empty/placeholder state; no partial text visible in a11y | empty | 0.00 | empty (placeholder) | pass — see note below |

Case-7 note: per-key a11y reads during the backspaces show `0.00` throughout — the a11y tree exposes the grey placeholder as text `0.00`, so reads can't distinguish "empty showing placeholder" from literal text; the screenshots (`item6c-empty-focused.png`) show the placeholder is grey, and the committed summary is `0.00` with the box empty. The first ⌫ cleared the whole typed content at once rather than digit-by-digit — consistent with the select-all still being replaced as a unit in this build; no wrong value is ever committed.

## Case 1 detail — the fast-input ordering bug at the FINAL head

`15000.50` typed with chained adb taps (<150 ms apart, typically 40–80 ms) ten times, into a zero box, verified per-run:

- 8 of 10 correct: `15000.50` → `15,000.50`.
- run 3: shown **15.00050** → committed `15.00`.
- run 5: shown **1500.050** → committed `1,500.05`.

The decimal point leapfrogs one digit-position LEFT; digits never reorder or drop. Committed always equals what the box showed — no hidden divergence. Log: `evidence-case1-fast.log`.

## Method notes

- The keypad floats over the form but the row sheet slides up above it; taps used the measured NSH pad positions (digits at x 135/405/675, rows y 1721/1873/2026/2179; `,` bottom-left, `.` bottom-right, ⌫ and ✓ right column).
- ctrl+a (keycombination 113 29) selects all in the money box; typing replaces — used to reset between amounts.
- The 200 ms wrong-digit effect in item 4 is the keypad-rise animation, not a value bug: the tapped point is a moving key.
- Money-box values survive reopen exactly (BIR_DATABASE_PATH test-data backed).
