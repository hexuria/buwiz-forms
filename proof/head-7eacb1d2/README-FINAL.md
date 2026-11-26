# Real-key money-box fault hunt — head 7eacb1d2

Exact commit verified: `7eacb1d2195b8dc1e1a1b65ad97728c15b50bc2f` (`fix/money-box-keep-text-on-focus`, draft PR #403). Real visible maximized Linux X11 window; real `xdotool key` events, 120 ms where ordered and zero delay where required; real Ctrl+V paste; saved values read from scratch SQLite drafts. Safety variables only; no Queue; no live submission; invented values; Android emulator/watch untouched.

## Second-line verdict

Faults shown: **H2, H3, H5 and H8**. E1, H1, H4, H6 and H7 behaved as expected/captured, with the H7 malformed paste silently refused back to zero.

## Table

| Case | Keys/input | Box after keys | After leaving | Saved draft | Result |
|---|---|---:|---:|---:|---|
| E1 Tab into `1,234.56` | `15000.50` @120ms | `15000.50` | `15,000.50` | `15000.5` | RIGHT |
| E1 Shift+Tab into `1,234.56` | `15000.50` @120ms | `15000.50` | `15,000.50` | `15000.5` | RIGHT |
| E1 click, Ctrl+A | `15000.50` @120ms | `15000.50` | `15,000.50` | `15000.5` | RIGHT |
| H1 click zero box | `15000.50` no delay, 10× | `15000.50` 10/10 | `15,000.50` 10/10 | `15000.5` 10/10 | RIGHT |
| H1 Tab zero box | `15000.50` no delay, 10× | `15000.50` 10/10 | `15,000.50` 10/10 | `15000.5` 10/10 | RIGHT |
| H2 typo mid-burst | `15000..50` no delay | empty | `0.00` | `0.0` | FAULT |
| H2 typo mid-burst | `150a00.50` no delay | empty | `0.00` | `0.0` | FAULT |
| H2 diagnostic | same strings @120ms | `15000.50` | `15,000.50` | `15000.5` | speed-dependent contrast |
| H3 form focus into filled surcharge | page Check/to-do focus, then `2500` | `1,500.002500` | `1,500.00` | computed `surcharge` stayed `1500.0`; raw `penalty_overrides.surcharge` became `1500.0025` | FAULT |
| H4 2550M recomputed editable Item 18F | Tab, `999`, 5× @120ms | `999` 5/5 | `999.00` 5/5 | `999.0` 5/5 | RIGHT |
| H4 same | Tab, `999`, 5× no delay | `999` 5/5 | `999.00` 5/5 | `999.0` 5/5 | RIGHT |
| H5 percent, 1601FQ Schedule 1 row 1 Tax Rate from 0 | click, `42` | `420` | `0.00` | `420.0` | FAULT |
| H5 percent, same | Tab, `42` | `42` | `0.00` | `42.0` | FAULT: silent visible/draft mismatch |
| H5 whole number, 2551Q Item 5 sheets from 0 | click, `42` | `04` | `04` | `4` | FAULT |
| H5 whole number, same | Tab, `42` | `42` | `42` | `42` | RIGHT |
| H6 commas | `1500,50` | `1500,50` | `150,050.00` | `150050.0` | RIGHT per review expectation |
| H6 commas | `1.234,56` | `1.234,56` | `1.23` | `1.23` | RIGHT per review expectation |
| H6 commas | `1,2,3` | `1,2,3` | `123.00` | `123.0` | RIGHT per review expectation |
| H7 paste | `₱ 15,000.50` | `15000.50` | `15,000.50` | `15000.5` | RIGHT/normalized |
| H7 paste | ` 15000.50 ` | `15000.50` | `15,000.50` | `15000.5` | RIGHT/normalized |
| H7 paste | `15,000.50` | `15000.50` | `15,000.50` | `15000.5` | RIGHT/normalized |
| H7 paste | `PHP 15000.50` | `15000.50` | `15,000.50` | `15000.5` | RIGHT/normalized |
| H7 paste | `15000.50.25` | empty | `0.00` | `0.0` | refused silently; behavior captured |
| H8 exact mismatch message | observed during H5/H3 mismatch states | — | visible `0.00` or `1,500.00` while draft retained another value | no exact `the box shows ..., but the draft stored ...` message appeared | FAULT: silent mismatch |

## Notes

- H1 caveat: every immediate post-key state/screenshot showed no leading `0.00`; every transient frame was not programmatically asserted, but the 60 fps recording is included.
- H3 focus method: 2551Q Item 20 Surcharge was filled with `1,500.00`; the form's own Check/to-do step moved focus to that row, then `2500` was typed without clicking the box. The row was a validation/to-do row through the penalty/EOPT prerequisite; an intrinsically invalid amount variant was not separately proven.
- H3 draft nuance: the computed `surcharge` field read `1500.0`, while raw `penalty_overrides.surcharge` was `1500.0025`. Thus the wrong raw amount was present silently even though the displayed/computed amount reverted to `1,500.00`.
- H6 first-pass `saved:null` rows in `h5-h8-results.jsonl` were a measurement-script path mistake (`schedule1` vs `schedule_1`); use the rows marked `(correct draft read)`.

## Evidence

- E1-H4 report: `REPORT.md`; recording `money-focus-validation.mp4`; rows `focus-results.jsonl` and `focus-results.csv`.
- H5-H8 report: `REPORT-H5-H8.md`; recording `money-focus-h5-h8-validation.mp4`; rows `h5-h8-results.jsonl`.
- Key fault screenshots: `h2-dots-after-keys.png`, `h2-letter-after-keys.png`, `h3-after-keys.png`, `h3-after-leaving.png`, `h5-pct-click-after-keys.png`, `h5-pct-click-after-leaving.png`, `h5-int-click-after-keys.png`, `h7-twodots-after-leaving.png`.
- Scratch DB included for independent draft inspection: `money-focus-validation.db`.
