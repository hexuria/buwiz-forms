# Keystroke-mangling fault — buwiz-forms money boxes, main @ 47ab5025 (Windows, real OS keys)

Report-only. Nothing fixed, no branch changed. All probes used real operating-system key events
(PowerShell `System.Windows.Forms.SendKeys]::SendWait`, SendInput-backed) into the live window of a
dev build of buwiz-forms main at 47ab5025 (kit pin 8c78cfd6), never the app's test harness.

## YES/NO

YES — a person's own keystrokes can silently save an amount different from what they typed, with no
visible error. Worst cases measured on main @ 47ab5025:

| keys typed (one by one, ~120 ms apart) | what the person meant | what was SAVED | tax due saved |
|---|---|---|---|
| `9 9 9 9 9 . 9 9` (99999.99) | 99,999.99 | **10,000.00** | 300.00 |
| `9 9 9 9 9 9 9 9 . 9 9` (99999999.99) | 99,999,999.99 | **10,000,000.00** | 300,000.00 |
| 1234567890, then Left, Left, Backspace | 123,456,790.00 | **23,456,790.00** (leading digit silently lost) | 703,703.70 |

Earlier runs (main 1b18d668 and the #370 head a70b3edd, same SendInput method) also saved
1000000.005 as **100000.01** — a 10× error. The fault is non-deterministic: the same probe sometimes
mangles differently or not at all (see table). Paste of the same strings is always correct.

## Where it starts (the ladder, 2551Q Schedule 1 row 1 taxable amount, keys at ~120 ms)

Distinct digits — correct at every width:

| keys | saved | verdict |
|---|---|---|
| 1234.56 | 1234.56 | correct |
| 12345.67 | 12345.67 | correct |
| 123456.78 | 123456.78 | correct |
| 1234567.89 | 1234567.89 | correct |
| 12345678.90 | 12345678.90 | correct |
| 123456789.12 | 123456789.12 | correct |

Repeated digits:

| keys | saved | verdict |
|---|---|---|
| 9999.99 | 9999.99 | correct |
| 99999.99 | **10000.00** | MANGLED — smallest failing input found |
| 999999.99, 9999999.99 | (batch-run focus was unreliable for these two; read crops before quoting) |

Digit count alone is NOT the trigger — the distinct-digit ladder passes at every width. The trigger is
**an adjacent repeated digit at the position where the live thousands-separator reformat moves the
caret** (first comma insertion at 5 whole digits). `cmp-t1-t2-big.png` shows the smoking gun: after the
8th key of 99999999 the caret sits one position early (before the last digit) while after the 8th key
of 12345678 it sits correctly at the end.

## Burst speed (all keys in one SendInput burst)

| keys | saved | verdict |
|---|---|---|
| 99999999.99 | **10000000.00** | MANGLED at burst speed too |
| 1000000.005 | 1000000.01 | correct this run (the fault is non-deterministic) |
| 12345678.90 | 12345678.90 | correct |

## Mid-string edits (typed 1234567890, then arrow keys, then one edit key, at ~120 ms)

| edit | correct result | saved | verdict |
|---|---|---|---|
| Left, Left, Backspace (delete the 8) | 123,456,790.00 | **23,456,790.00** | MANGLED — leading digit lost |
| Left, Left, 5 (insert before 9) | 12,345,678,590.00 | 12,345,678,590.00 | correct |

## Cause (read, not fixed)

`nsh/ui/mask.ts` (app repo): `reconcile` (L179-224) reformats the box on every keystroke while focused
and restores the caret by counting significant digits before the raw caret, then `caretAfter` (L72-83);
the prefix/suffix guess in `findEdit` (L93-126, comment at L111-126: "a typed digit equal to its
neighbour is ambiguous") cannot tell WHICH of two equal adjacent digits was typed, so after the comma
reformat the caret settles one position early. Subsequent digits then insert one position early,
displacing digits — silently. `nsh/ui/fields.tsx` `MaskedField` write-back (L294) restores the selection
and `MoneyInput` (L416-424) documents the design: "thousands separators while typing … leaving the box
writes two decimals" — live reformatting while focused, unlike the official program, which formats on
blur only. The orchestrator's guess is CONFIRMED as the design difference: no live reformat while
focused + format on blur (the official behaviour) would remove the ambiguity entirely — typed keys land
exactly where the caret is, in the order the person typed them. Smallest fix consistent with that:
while the box has focus keep the raw digit string unformatted (or at most reformat but never move the
caret backward), apply commas+decimals on blur. Report only — nothing was changed.

## Files

- `failing-run-99999.99.mp4` — 70 s desktop recording of a failing run. Keys, in order, into Schedule 1
  row 1 taxable amount: Ctrl+A, Backspace, then `9` `9` `9` `9` `9` `.` `9` `9` at ~100-120 ms each
  (person speed), then Tab. Box settles at 10,000.00 instead of 99,999.99. Recording is real-time
  (10 fps gdigrab); the probe occupies the middle of the clip.
- `370-one.ps1` — probe runner used per value (caller clicks the field first; real clicks, SendKeys typing,
  4 s save settle, crops + draft-DB read). `370-mid.ps1` — same for arrow-key/edit probes.
  `370-keyprobe.ps1`, `370-autoprobe.ps1`, `370-matrix.ps1`, `370-rest.ps1`, `370-batch.ps1`,
  `370-final.ps1` — earlier iterations kept for the fix-proof machine (the batch self-focus in
  matrix/rest/batch/final is UNRELIABLE — window-foreground race; use the hybrid: click via any
  OS-level input tool, then run 370-one.ps1/370-mid.ps1).
- `results-slow.txt`, `results2.txt`, `results-final.txt`, `results-batch.txt`, `results-rest.txt` —
  probe logs. NOTE: rows in the batch logs where consecutive probes read identical values are
  save-lag/self-focus artifacts of the unreliable batch method — trust only the `-aftertab.png` crops
  and the hybrid rows (lad*b-*, rep4/rep5, burstH-*, midh-*), each of which was visually verified.
- `*-aftertab.png` — drawer crop (x1440,y140,w480,h180) after Tab-out for each probe: the box content
  as formatted on blur. `lad*b-*-inbox.png` — box while focused.
- `cmp-t1-t2-big.png` — side-by-side of the caret position after key 8 for 99999999 vs 12345678.
- `zrep5.png`, `m2-a.png`, `m2-b.png`, `m-ladder*` — montage crops used for verification.

## Not done / caveats

- 2551Q item 15 (amended return), 1701 and 2550M money boxes: not yet probed (same shared MaskedField/
  MoneyInput component — fault is expected to be identical; the fix-proof machine should cover them).
- All probes were single runs; the fault is non-deterministic, so a correct row does not prove a value
  is safe.
