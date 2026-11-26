# Acceptance F/G/negative-C plus caret/digit validation — exact `7d491c57`

Date: 2025-11-26

Commit under test: `7d491c57dd073a86d933361088500ab03bc74837`  
Worktree: `/home/ubuntu/forms-validation/money-accept-7d491c57`  
Target: `/home/ubuntu/forms-validation/target-money-accept-7d491c57`  
Scratch DB: `/home/ubuntu/forms-validation/money-accept-proof/money-accept-validation.db`  
Recording: `/home/ubuntu/forms-validation/money-accept-proof/money-accept-fgc-validation.mp4`  
Curated machine rows: `/home/ubuntu/forms-validation/money-accept-proof/fgc-final-results.jsonl`

Runtime was a real visible maximized Linux X11 NSH window. Character entry used real `xdotool key` events at 120 ms unless a case explicitly says no-delay. Saved values were read from the scratch SQLite draft after autosave. No Queue, live submission, token exposure, commit, or push. Android was not touched.

Setup disclosure: direct scratch SQLite edits were used only for fixture setup/reset. In particular, 2551Q Schedule 1 was reset to rows PT010 `1000.00` and PT040 `3000.00` before each G attempt. For 1604C, an initially hand-created invalid row was replaced by the app's canonical annual draft row (`quarter=0`, `period_key='A'`); the negative tests themselves were typed in the real window after resetting the Adjustment box to zero through the UI. The app-created canonical 1604C row is id 5.

## F — recomputed 2550M Item 13B after changing 13A

Procedure: replace Item 13A, then press Tab and type `999` into recomputed Item 13B. The table uses `base_saved` to confirm 13A actually saved.

| run | 13A base | after keys in 13B | after leaving 13B | saved 13B | result |
|---|---:|---|---|---:|---|
| 120 ms 1 | 1000.0 | `120.00999` | `120.01` | 120.01 | FAULT |
| 120 ms 2 | 2000.0 | `240.00999` | `240.01` | 240.01 | FAULT |
| 120 ms 3 | 3000.0 | `360.00999` | `360.01` | 360.01 | FAULT |
| 120 ms 4 | 4000.0 | `480.00999` | `480.01` | 480.01 | FAULT |
| 120 ms 5 | 5000.0 | `600.00999` | `600.01` | 600.01 | FAULT |
| no-delay 1 | 1100.0 | `999` | `999.00` | 999.0 | RIGHT |
| no-delay 2 | 2200.0 | `999` | `999.00` | 999.0 | RIGHT |
| no-delay 3 | 3300.0 | `999` | `999.00` | 999.0 | RIGHT |
| no-delay 4 | 4400.0 | `999` | `999.00` | 999.0 | RIGHT |
| no-delay 5 | 5500.0 | `999` | `999.00` | 999.0 | RIGHT |

No sentence appeared in the F cases. Interpretation: with real typing at 120 ms per key, recomputation of 13B raced/concatenated with the typed `999`; the digits were appended after the newly computed value and then amount normalization effectively discarded the tail. At no-delay, the typed replacement was atomic enough to win and was preserved.

Representative screenshot: `/home/ubuntu/forms-validation/money-accept-proof/f-120ms-1-keys.png` and `/home/ubuntu/forms-validation/money-accept-proof/f-120ms-1-leave.png`.

## G — row delete with typing pending in 2551Q Schedule 1

Initial rows before both corrected runs:

```json
[
  {"atc":"PT010","taxable_amount":1000.0,"tax_due":30.0},
  {"atc":"PT040","taxable_amount":3000.0,"tax_due":90.0}
]
```

`7777` was typed into row 2 Taxable Amount. Because row 1 cannot be opened while row 2's sheet is open, the fastest correct path was: first row-1 click closes row 2, the same row is clicked again as soon as exposed, then row 1 Delete is pressed as soon as row 1's drawer exists. No pre-delete semantic reads or screenshots were taken in the corrected runs because those would add artificial delay.

### Corrected fast run

- Type end → Delete click: **853.9 ms**
- First row-1 click after typing: 145.1 ms
- Row-2 sheet close detected: 173.9 ms after first click
- Row-1 drawer ready: 712.8 ms after typing
- Delete click: 853.9 ms after typing
- Exact rows afterward:

```json
[
  {"atc":"","taxable_amount":0.0,"tax_due":0.0},
  {"atc":"PT040","taxable_amount":7777.0,"tax_due":233.31}
]
```

- `schedule1_total_tax_due`: `233.31`
- Visible row: `Row 2, PT040 · 7,777.00 · 3.0 · 233.31. Open`
- Screenshot: `/home/ubuntu/forms-validation/money-accept-proof/g-fast-corrected-after-delete.png`

### Corrected one-second run

- Type end → Delete click: **1909.9 ms** (1.0 s deliberate wait plus drawer/delete traversal)
- Exact rows afterward:

```json
[
  {"atc":"","taxable_amount":0.0,"tax_due":0.0},
  {"atc":"PT040","taxable_amount":7777.0,"tax_due":233.31}
]
```

- `schedule1_total_tax_due`: `233.31`
- Visible row: `Row 2, PT040 · 7,777.00 · 3.0 · 233.31. Open`
- Screenshot: `/home/ubuntu/forms-validation/money-accept-proof/g-delay-corrected-after-delete.png`

Result: typed `7777` survived both timings and was not overwritten; row 1's original `1000.00` did not overwrite row 2. However, the strict expected outcome “one row holding `7,777.00`” was **not** met in the draft representation: the array retained a blank first row and the filled second row rather than compacting to a single row. Therefore this is a PARTIAL/FAULT result.

The raw `fgc-results.jsonl` also contains earlier superseded G rows in which reading/screenshotting before Delete made even the “fast” attempt slower than intended. The corrected rows above are the evidence to use.

## Negative-capable C — 1604C Part II January Adjustment

Exact box: 1604C Part II, Row 1 January, **Adjustment** (`fp-frm1604c:txtSched1Adj1`). `crates/bir-core/src/forms/form_1604c.rs` documents this adjustment as “May be negative.”

| typed | after keys | after leaving | saved adjustment | total adjustment | row total remitted | result |
|---|---|---|---:|---:|---:|---|
| `(1,000.00)` | `(1,000.00)` | `-1,000.00` | -1000.0 | -1000.0 | -1000.0 | RIGHT |
| `-1500.50` | `-1500.50` | `-1,500.50` | -1500.5 | -1500.5 | -1500.5 | RIGHT |

No negative-amount rejection/explanation sentence appeared. The row did show the unrelated exact red completeness note: `Incomplete values on Part II (Continuation), Row 1.` because the intentionally minimal fixture lacked other row fields.

Screenshots: `/home/ubuntu/forms-validation/money-accept-proof/c-neg-parentheses-leave.png`, `/home/ubuntu/forms-validation/money-accept-proof/c-neg-minus-leave.png`.

## Added bare-click caret checks — PR head only

Surface: 2551Q Schedule 1 row drawer Taxable Amount initially `1,234.56`; fresh box each time. Coordinates below are logical points in the 2560×1920 window. Caret positions are inferred exactly from where the typed `9` appeared.

| click point and intended location | inferred caret | after typing `9` | after leaving | saved | sentence |
|---|---|---|---|---:|---|
| logical `(2500,173)`, between `2` and `3` | after `2` / before `3` | `1,2934.56` | `1,234.56` | 1234.56 | `Taxable Amount: 1,2934.56 is not an amount. The box kept 1,234.56.` |
| logical `(2532,173)`, just after final `6` | end after final `6` | `1,234.569` | `1,234.57` | 1234.57 | none |
| logical `(2200,173)`, empty right-aligned area left of number | before first digit | `91,234.56` | `91,234.56` | 91234.56 | none |

Finding: clicks at the end and in the empty left part behaved coherently. The between-`2`/`-3` click did place the caret in the intended character gap, but because grouping commas remained fixed while editing, insertion produced malformed-looking `1,2934.56`; on blur this was judged unreadable and reverted. This is an important caret/editing fault or at least strongly confusing behavior.

MAIN comparison: **not available**. Both `/home/ubuntu/forms-validation/main-134c2e4e` and `/home/ubuntu/forms-validation/target-main-134c2e4e` were absent from disk at the time of this check, so the same three clicks could not be run on MAIN.

## Added digit-limit checks

All four cases began from a held amount of `1,234.56`; after leaving, the held amount was restored and saved value remained `1234.56`.

Schedule cases were edited **inside the row drawer** (`editing_inside_row_drawer=true`). While the oversized digits were still being edited, the background row line omitted the amount: `Row 1, PT010 · 3.0 · 37.04. Open`. After leaving/restoring, the row line again showed it: `Row 1, PT010 · 1,234.56 · 3.0 · 37.04. Open`. No real toast text was observed; the semantic `toaster` container exists even when empty. The schedule input visibly had a red border and red explanatory sentence.

Exact sentences:

- Schedule, 20 digits `12345678901234567890`:
  `Taxable Amount: you typed 12,345,678,901,234,567,890.00; the return holds 1,234.56. Exceeds the official 12-character input limit (at most 999,999,999,999 without centavos, or 999,999,999.99 with centavos); enter a smaller amount.`
- Item 15, 20 digits:
  `Item 15: you typed 12,345,678,901,234,567,890.00; the return holds 1,234.56. Exceeds the official 12-digit peso field (at most 999,999,999,999.99); enter a smaller amount.`
- Schedule, 13 digits `1234567890123`:
  `Taxable Amount: you typed 1,234,567,890,123.00; the return holds 1,234.56. Exceeds the official 12-character input limit (at most 999,999,999,999 without centavos, or 999,999,999.99 with centavos); enter a smaller amount.`
- Item 15, 13 digits:
  `Item 15: you typed 1,234,567,890,123.00; the return holds 1,234.56. Exceeds the official 12-digit peso field (at most 999,999,999,999.99); enter a smaller amount.`

Thus the plain item sentence starts with exact `Item N:` wording (`Item 15:`), while the schedule-cell sentence uses the field label prefix `Taxable Amount:`.

Note: the earlier A/C/D pass reported no captured sentence for twenty digits. This rerun kept the row drawer visibly open and captured the sentence after blur; the sentence definitely exists on this exact build in this state. Treat the earlier no-sentence observation as a capture/timing limitation, superseded by this direct evidence.

Representative screenshots:
- `/home/ubuntu/forms-validation/money-accept-proof/x-schedule-20digits-leave.png`
- `/home/ubuntu/forms-validation/money-accept-proof/x-item15-20digits-leave.png`
- `/home/ubuntu/forms-validation/money-accept-proof/x-schedule-13digits-leave.png`
- `/home/ubuntu/forms-validation/money-accept-proof/x-item15-13digits-leave.png`

