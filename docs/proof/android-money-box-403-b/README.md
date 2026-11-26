# Android hunt — money box rework (#403) at 7d491c57

Independent pass, written before reading the branch's own proof.

- Build: `7d491c57dd073a86d933361088500ab03bc74837` (fix/money-box-keep-text-on-focus, main e5f7b42b merged in). Folder `target-7d491c57`. Kit pin `8c78cfd6` (`~/BuwizNsh/tools`). Recipe: `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true BUWIZ_ANDROID_DEV=1`, x86_64 emulator, API 35.
- Baked env keys: `EBIR_TEST_ENV=1;BIR_DATABASE_PATH=test-data/moneybox.db;BIR_AUTH_BYPASS=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1`. `BIR_DATABASE_PATH` is used so drafts survive relaunch (as in job 39, for the reopen checks). Never Queue pressed; invented values only; outside network cut (`network-cut.txt`).
- Input: on-screen keypad taps only, inside the box's own uiautomator bounds; `show_touches 1` on. Fast input: the 8 key taps chained inside one adb shell — measured spacing well under 150 ms.
- Harness plumbing, disclosed: clearing a box between runs used keyevents MOVE_END+DEL (caret plumbing, not amount input). Every measured amount was typed with on-screen keypad taps only.
- Draft: 2551Q Q4 2026, Schedule 1 row 1 (PT010), Taxable Amount. Row summary read after collapsing the row; reopen check by leaving to Home and reopening the draft.

## Verdicts

**Fast input: misplaced in 0 of 10.** Ten scripted runs of `15000.50` into the zeroed box, each cleared first: after taps `15000.50`, after Done `15,000.50`, row `PT010 · 15,000.50 · 3.0 · 450.02` — every run. Video `j41-fast10.mp4` (taps shown) covers the batch; `fastlog4.txt` the per-run log.

**Every other amount saved as typed: NO** — three faults below (F1–F3), plus two design deviations (D1–D2).

## Case table

| case | keys | after taps | after Done | saved (row / after reopen) | verdict |
|---|---|---|---|---|---|
| 1 ×10 | 15000.50 fast | `15000.50` | `15,000.50` | `15,000.50` | OK 10/10 |
| 2 | 0.50 | `0.50` | `0.50` | `0.50` | OK |
| 2 | 1234.56 | `1234.56` | `1,234.56` | `1,234.56` | OK |
| 2 | 99999.99 | `99999.99` | `99,999.99` | `99,999.99` | OK |
| 2 | 100000 | `100000` | `100,000.00` | `100,000.00` | OK |
| 2 | 1000000.01 | `1000000.01` | `1,000,000.01` | `1,000,000.01` | OK |
| 2 | 12345678.90 | `12345678.90` | `12,345,678.90` | `12,345,678.90` | OK |
| 2 | 99999999.99 | `99999999.99` | `99,999,999.99` | `99,999,999.99` | OK |
| 3 | filled 1,234.56, then `8` | `81,234.56` (prepended!) | — | — | D1, see below |
| 3 | then `.` | `81,234.56` (unchanged) | — | — | second point rejected — correct strictness |
| 3 | then bksp | `1,234.56` | `1,234.56` | `1,234.56` | OK |
| 4a | `1 5 , 5 0` | `1550` — **comma never appears** | `1,550.00` | `1,550.00`, no sentence | **F1** |
| 4b | `1 , 5 0 0` | `1500` | `1,500.00` | `1,500.00` | same root cause, coincidentally correct result |
| 5a | tap zero box + `1` within ~100 ms, then `234.56` | first digit empty | `234.56` | `234.56` — the `1` was lost | **F2** |
| 5b | Next in row, then `9` | `91,234.56` | — | inserted, not replaced | D1 again |
| 5c | leave app focused, back after 5 s | box reads `12.35` — was `1,234.56`! | — | — | **F3** |
| 6 | sheets box `0`, type `4 2` | `42` | `42` | `42` — not `04` | OK |
| 7a | bksp to empty, Done | empty | `0.00`, **no sentence** | `0.00` | OK |
| 7b | two-row delete race | — | — | — | NOT MEASURED (time) |

"the box shows …, but the draft stored …" never appeared in any run.

## F1 — the keypad's comma key inserts nothing (case 4)

In a zero box, keys `1 5 , 5 0` produce the text `1550` — the comma never enters the box. On Done the box commits `1,550.00` with **no sentence**, because `1550` is a perfectly readable amount. The user who typed comma-decimal `15,50` (or the spec's unreadable case) silently files **1,550.00** — exactly the wrong-value-with-no-error class this rework was meant to kill. Case 4b (`1 , 5 0 0` → `1,500.00`) matches the spec's expectation only by accident: the comma is dropped either way, so `1,500` and `1500` are the same text. Whether the key is meant to insert a comma at all: the keypad ships the key, and the strict reading defines comma semantics — as measured, the key is dead and mistyped input is silently accepted.

## F2 — the first digit right after the focus tap is eaten (case 5a)

Tap into a box showing `0.00` and press `1` within ~100 ms of the tap: the box stays empty. The rest of the keys land (`234.56`), and `234.56` is what saves — the typed `1234.56` loses its first digit with no error. A focus-timing race: the tap's focus lands after the keypress.

## F3 — leaving the app with the box focused rewrites the amount (case 5c)

Box holding `1,234.56` committed, refocused, then Home key, 5 s away, relaunch: the box reads **`12.35`** — the text was rewritten while away, no keys pressed. The keypad is still up and the box still focused on return. Typing `7` then prepends (`712.35`) and `712.35` saves. An amount the user never typed is what the draft holds.

## D1 — no select-all anywhere; caret lands at the start of the text (cases 3, 5b)

Tapping into a filled `1,234.56` — anywhere in the box, even at its far right end — places the caret at the **start** of the text. The next digit prepends (`8` → `81,234.56`). The design note "the form's own focus selects the whole text" was not observed in any focus path (tap, or the keypad's Next key): no selection appears, and typing always inserts at the start rather than replacing. Combined with the backspace deleting backward, a filled box cannot be corrected without first moving the caret — and there is no keypad way to move it forward. (Long-press raised no selection menu either.)

## D2 — while focused, the box still shows grouping separators (observation, not a fault)

On focus a filled box keeps its text **with** separators (`1,234.56`), per the design. Typed text is not regrouped live (`15000.50` stays as typed until Done) — correct. But after a digit is inserted into the filled text, the display reads e.g. `81,234.56`, i.e. the kept-separator text with the insertion — consistent with the design, noted because it makes caret position hard to read visually.

## What passed clean

- All seven case-2 amounts, including `99,999,999.99` and `1,000,000.01`: typed exactly, grouped on Done, saved equal, tax due recomputed correctly (`3,000,000.00` on the largest).
- Zero box shows empty on focus (money and whole-number boxes).
- Half-up rounding and grouping on leaving: correct throughout.
- Whole-number Number of Sheet/s: `42` saved as `42` (not `04`).
- Backspace-to-empty then Done: `0.00`, no sentence — correct.
- Second `.` rejected while typing — strictness works.
- Reopen persistence: `91,234.56` row summary intact after leaving to Home and reopening the draft (`reopen-check.png`).
- Fast input: no misplacement in 10/10 at <150 ms spacing.

## Case 8 — look (phone and tablet width)

Screenshots: `case8-phone-rowdetail.png` (1080×2400), `case8-tablet-rowdetail.png` (1600×2560 @320), `case8-phone-reset.png` after reset. The amount box renders full-width at both sizes; see the images for the touch target. Not measured: the unreadable sentence's wrap — the comma fault (F1) means there was no way to raise the sentence through the keypad; the sentence requires typing text the keypad cannot produce.

## Files

- `j41-fast10.mp4` — the ten fast-input runs, taps shown (519 s; the runs are the first ~8 minutes).
- `fastlog4.txt`, `caselog.txt` — verbatim measurement logs.
- `case2-*.png`, `case3-*.png`, `case4a/b.png`, `case5a.png`, `case5c-return.png`, `case6-*.png`, `case7a.png`, `case8-*.png`, `reopen-check.png`.
- `network-cut.txt`.
