# candidate (b) - reselect-once after outside write + focus collapse
Diff: candidate-b.diff (33 lines, no logging). h4-trace-v2.txt: instrumented trace.

## measured timings
- collapse of the platform select-all arrives 41-43 ms after focus (consistent, 8+ runs) when the write lands within ~20ms of focus; with a write-to-focus gap of 85-110ms the collapse does not happen at all (selection survives natively). bound chosen: 150 ms = 3.5x the measured latency; a filer's own End/arrow after a deliberate pause comes 300-1000ms+ after focus (6x+ margin the other side). End/arrows do not reach onKeyPress, so the time bound is unavoidable.
- second failure shape found in burst runs: sometimes NO select-all event at all, the first selection is already caret-at-end. v2 fires on any end-collapsed caret within the window, covering both shapes.

## results table (2550M Nov 2026, 13A base -> 13B editable recomputed output tax, saved from draft/onBlur-write)
| case | runs | result |
|---|---|---|
| Tab, wait, 999 | 5/5 | reselect fires, keys replace, saved 999.00 |
| Tab, burst 999 no delay | 5/5 | same, incl. shape-2 runs, saved 999.00 |
| variant 13A 1000->2000 alternating | included above | 240.00/120.00 recomputed shown at rest; override 999.00 saved |
| Tab, wait 1s, End, type 9 | 1 | reselect fired at +50ms, user's End collapsed later, 9 APPENDED -> 240.01 (constraint satisfied: user caret move wins) |
| Tab, leave without typing | 2 | recomputed amount kept shown, saved 120.00/240.00, edited=false |
| Shift+Tab path | 1 | 999.00 saved (first entry forward-tab armed; re-entry natural select-all) |
| zero-recompute: clear 13A, Tab into 13B(0.00), type 5 | 1 | box shows EMPTY on focus (zero-to-empty write), len 0 so no reselect, 5 lands as first digit -> saved 5.00 |
| rest display after Tab | all | recomputed amount is what the box shows (write lands before focus) - never stale |
| 2551Q regression W1 Tab into 2,000.00 type 15000.50 | 1 | saved 15000.5; no reselect-fire logged on 2551Q (no outside writes -> candidate inert). W2/W7/W8 not rerun: no code path difference (armReselect never set without an outside write) |

## not done
click-into-recomputed-box explicit run; full 5x Shift+Tab; 20-run timing statistics formalized (8+ consistent samples).
