# Money-box real-key round — hexuria/buwiz-forms @ 93c575187749d8fa121c032bc489765c857810b3

Pull request #403 (fix/money-box-keep-text-on-focus), rev-parse confirmed. Desktop build, fresh clone,
own build folder. Real window, real hardware-keycode key events and real mouse clicks (no harness value
setter). 2551Q Schedule 1 row 1 taxable amount unless said; demo taxpayer; invented values. Saved values
read from the draft sqlite after each case. Keys at 120 ms unless said. Four safety variables set
(BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1); Queue never pressed.

Rig notes: gpui-agent snapshot for reading the tree; sentences do NOT appear in the agent tree — they are
captured by screencapture and read visually (the pngs here). Letters/comma/space typed with hardware
keycodes (keyalpha). One rig lesson: all driving must happen inside one continuous process — the window
loses key focus between separate invocations. The app was relaunched once with BIR_DATABASE_PATH pointed
at the same scratch db; draft data preserved (evidence values unaffected).

## A. ENTRY into 0.00 — every amount saved as typed: YES

| case | after leaving | saved | verdict |
|---|---|---|---|
| click 15000.50 | 15,000.50 | 15000.5 | RIGHT |
| click 99999.99 | 99,999.99 | 99999.99 | RIGHT |
| click 1000000.005 | 1,000,000.01 | 1000000.01 | RIGHT (half-up on leaving) |
| Tab + same three | identical | identical | RIGHT |
| Shift+Tab + same three | identical | identical | RIGHT |
| 1234567890, Left Left Backspace | 123,456,790.00 | 123456790.0 | strict caret arithmetic — see report (differs from the 12,345,689.00 expectation) |
| burst 15000.50, click / Tab | 15,000.50 | 15000.5 | RIGHT |
| count box (Number of Sheets) 0, click, 42 | 42 | 42 | RIGHT — old `04` fault gone |
| count box, Tab in, 42 | 42 | 42 | RIGHT |

Hit-test scan of first clicks on 1,234.56 (x → where 9 lands): ≤1314 front · 1321 before 2 · 1335 between
2 and 3 · 1342 3|4 · 1349 4|point · 1356 point|5 · 1363 5|6 · 1370 end. Glyph-accurate.

## B. FILLED AMOUNT 1,234.56

| case | box after keys | after leaving | saved | verdict |
|---|---|---|---|---|
| Tab in, 7 | whole replaced | 7.00 | 7.0 | RIGHT |
| Tab in, Right, 9 | `1,234.569` while typing | 1,234.57 | 1234.57 | RIGHT |
| click between 2 and 3 (x1335), 9 | `1,2934.56` | 12,934.56 | 12934.56 | RIGHT |
| Cmd+A, Backspace, Tab away | empty | 0.00 | 0.0 | RIGHT, no sentence |
| caret end, 4×Backspace, Tab | `1,23` | 123.00 | 123.0 | RIGHT |

No sentence under the box in any A/B case.

## C. COMMAS into 0.00 then Tab

| input | after leaving | saved | note |
|---|---|---|---|
| `1500,50` typed | 1,500.50 | 1500.5 | comma-as-decimal accepted when typed |
| `1,500` | 1,500.00 | 1500.0 | RIGHT |
| `15,00,50` | 0.00 | 0.0 | red sentence, see below |
| paste `1500,50` over selected 1,234.56 | reverts to 1,234.56 | 1234.56 unchanged | pasted comma-decimal is NOT read, typed comma-decimal is — flagged |

C3 sentence word for word (shot-c4r / caseC): "Taxable Amount: 15,00,50 is not an amount. The box kept
0.00. Use a point for centavos, commas only between thousands."

## D. UNREADABLE (paste over selected 1,234.56, then Tab)

All four: box shows the pasted text while editing; leaving reverts to 1,234.56; draft unchanged 1234.56.

- `1500x`, `12a3`, `15000..50` — sentence word for word (shot-pd1.png): "Taxable Amount: 1500x is not an
  amount. The box kept 1,234.56." (the typed text substituted; prefix is the field label, not "Item N")
- twenty digits — sentence word for word (shot-pd4.png): "Taxable Amount: you typed
  12,345,678,901,234,567,890.00; the return holds 1,234.56. Exceeds the official 12-character input limit
  (at most 999,999,999,999 without centavos, or 999,999,999.99 with centavos); enter a smaller amount."
- LETTER PROBE: letters DO insert into the amount box as text (`12` + a → `12a`, + `3` → `12a3`) — they are
  not refused at key time, so typed unreadable text reaches the same not-an-amount path as paste.

## E. THE SAME INSTANT — click into 0.00, burst 15000.50, ten runs per delay

wrong 0/10 at 0 ms · wrong 0/10 at 50 ms · wrong 0/10 at 100 ms — every run: box `15000.50`, after leaving
15,000.50, saved 15000.5 exact. Never a leading `0.00`.

## F. RECOMPUTED BOX — 2550M: 1000 in 13A, Tab (lands in recomputed 13B), 999

0 ms: 10/10 REPLACED — typing-end `999`, after leaving 999.00, saved output_tax_government 999.0,
sales_to_government intact 1000.0.
120 ms: 10/10 REPLACED — identical.
The Mac replaces at both delays (Linux appended at 120 ms at the last head — the Mac does NOT).
Focus fact measured on the way: Tab out of 13A lands directly in recomputed 13B; a probe digit replaces
the recomputed value wholesale.

## G. 1601FQ TAX RATE box

Could not reach an editable rate: with a row added and the ATC chosen, the Tax Rate row renders only the
hint text "Shows the ATC's rate once this row's ATC is chosen." for every ATC tried (WI226, WI240, WI250,
WI260, WI310, WI330) — no input, no auto-filled rate. The page shows in red: "Selecting an ATC is not
necessary when item no. 4 is set to ' NO '." — so with item 4 = NO the rows may be inert by design; which
rows let you type a rate could not be established in this round. Evidence: caseG-scan.log, shot-g-click.png.

## H. AWAY AND BACK — Cmd+Tab away 5 s and back, then type 7 (five runs each)

H1 filled 1,234.56 with focus: box unchanged after back, 5/5. Typing 7 inserts at the caret — which a
center click in the empty left area of the right-aligned box had placed at the FRONT: `71,234.56` →
71,234.56 saved 71234.56, 5/5 consistent. The app switch itself disturbs nothing; the caret is preserved
across it, and typing continues exactly where the caret is (no reselect, no replacement).
H2 `1500` typed and not yet left: box unchanged after back (`15001,234.56` — the 1500 had prepended at the
front-position caret), 5/5; typing 7 continues mid-text: `150071,234.56` → 150,071,234.56 saved
150071234.56, 5/5.

## I. A BOX THAT GOES AWAY — `12,3 4` typed, part folded without leaving, reopened

2551Q note: Part II of 2551Q has no editable amount in this state (items are computed or "Filled in when
the return is paid"), so the case ran on 2550M Part II 13A — an amount-kind box of a Part II row.
Typed `12,3 4` (space accepted verbatim, box shows `12,3 4`), folded Part II from the navigator without
leaving the box. Reopened: the box shows the last saved 1,000.00 with a red border and the sentence
word for word (shot-i.png): "Item 13A: 12,3 4 is not an amount. The box kept 1,000.00. Use a point for
centavos, and commas only between thousands." — folding the part commits as a leave: the uncommitted text
is rejected, the saved value restored, the sentence shown. Draft still 1000.0. Note this sentence uses the
"Item 13A:" prefix (unlike the Schedule-1 drawer's "Taxable Amount:" label).

## Files

- caseA/B/C/D5/E/F/H/I .log+.json — full transcripts with per-key states
- shot-pd1..4.png — unreadable sentences · shot-c4r.png — comma paste · shot-i.png / shot-i2.png — fold case
- shot-letterprobe.png — letters-in-box probe · shot-g-click.png — 1601FQ rate row · shot-demo-end.png
- rec-demo.mp4 — short recording of the app window: typing 15000.50 into 13A, leaving (grouping), then
  pasting 1500,50 and the revert with its sentence
