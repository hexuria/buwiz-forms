# Cases (a)-(d) at 1988536b — keys 15000.50, 2551Q Schedule 1 row 1 taxable amount

Real OS key events (PowerShell SendKeys / SendInput) at 120 ms per key into the live window,
app built at hexuria/buwiz-forms 1988536bfd41b22ee77d6f1e39ef29a61d4bbc87 (fix/money-box-no-live-format).
Saved values read from the draft SQLite database, not the screen.

Per-key crops: hX-key0-start (state before first key), hX-keyN-<char> (box right after key N), hX-aftertab (after leaving).
Montages: zhA.png/zhD.png. h1988-caseA.mp4: recording of case (a).

| case | entry | saved (draft) | verdict |
|---|---|---|---|
| (a) click 0.00 box, type | focused zero box shows EMPTY; digits appear exactly as typed | 15000.50 | right |
| (b) click 1,234.56, Ctrl+A, type over selection | digits exactly as typed | 15000.50 | right |
| (c) click 1,234.56, Ctrl+A, Backspace, type | digits exactly as typed | 15000.50 | right |
| (d) Tab into 1,234.56, type | NO select-all; caret before last digit (1234.5|6); keys splice in: 1234.51500050|6 | 1234.52 | WRONG |

Case (d): blur rounds the spliced text 1234.515000506 -> 1,234.52; the filer's typing silently becomes a different amount.
