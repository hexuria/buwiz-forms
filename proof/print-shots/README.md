driver: gpui-agent-cli 0.5.0

# Print-preview shots for #495 and #499

Print-preview pictures taken in the REAL macOS app, through the app's real print
path (the print button on the form page opens the app's own print-preview window;
each PNG is the preview window on screen, captured with macOS `screencapture` —
observation only; every input went through gpui-agent-cli 0.5.0, pinned by the
repo's e2e recipes).

## Exact heads shot

| set | ref | head |
|---|---|---|
| before | main | 08e753f056e200f8d13f408ea555c0b3e395aac9 |
| after (folder 495/) | hexuria fix/rule6-band-uses-own-field (#495) | 5282f20fab7aad8e90d44ddceeefb2f72762921e |
| after (folder 499/) | hexuria fix/print-uniform-amount-sections (#499) | 2f4bd3f6c00e9ae584a0069d2a20b5f0944f2820 |

nsh pin at all three: 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf.

## Safety on every run

`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, a
scratch database, the demo taxpayer (TIN 000-123-453-00000, "SANTOS, JUANA",
RDO 044) and invented values. Nothing was ever queued or sent; `BIR_SFTP_LIVE`
was never set.

## Fills (through the data plane, `form.fill` — real validation enforced)

### 1701 (for #495)
- `frm1701:rdoPg1I7ATC_II012` = true first (Schedule 3 stays closed otherwise)
- item 19: `frm1701:txtPg2IShed3_19Desc` = "SPECIMEN OTHER NON-OPERATING INCOME LONG DESCRIPTION WRAPPING OVER SEVERAL LINES OF PRINT", `frm1701:txtPg2IShed3_19A` = 1000
- item 20: `frm1701:txtPg2IShed3_20Desc` = "SPECIMEN ITEM TWENTY LONG DESCRIPTION ALSO WRAPPING OVER SEVERAL LINES OF PRINT", `frm1701:txtPg2IShed3_20A` = 1000

### 1601C (for #495)
- item 20: `frm1601c:txt20Other` = "SPECIMEN LONG SPECIFY TEXT FOR OTHER MANNER OF PAYMENT WITH MANY WORDS TO EXERCISE WRAPPING", `frm1601c:txtTax20` = 1000

### 1702Q (for #499)
- Q2 overflow draft: period 2, `frm1702q:cbATC_2` = "IC010_25%" (the ATC the
  schedules key off), then Schedule 4 `frm1702q:Sched4:txtTax2` with txtTax1/3/4/5/6/7 = 100 each.
  NOTE: the job's example 12,345,678,901.00 (11 integer digits) does NOT overflow
  this sheet's comb column — the capture with it printed digits aligned to combs
  in both the before and the after build. The overflow shot therefore uses the
  larger 12,345,678,901,234.00 (the PR's own proof value), which does convert.
- Q1 zero draft: period 1, only the ATC above; no amounts — Schedule 1 column A
  must keep its combs.

## Recipe steps

1. `just nsh-run-agent macos <scratch>.db` with the safety environment above in
   a worktree at the head being shot.
2. Seed through the data plane (port 17422): `profile.save`, `profile.forms_set`
   (1701, 1601C, 1702Q for year 2026), then per form `form.open` → `form.fill`
   → `form.save_draft` with the fills above.
3. On the window plane (port 17421): click `fp-print`; the print-preview window
   opens. Click `print-zoom-fit` inside it (this also targets the preview window
   for input), then point-scroll (`scroll --x 470 --y 500 --dy 850`) to the
   target rows. `screencapture -x` after each step.
4. Each PNG in `before/` and `after/` is one capture; the row-by-row reading is
   in the job report.

Known gaps: none hit — every step needed (open, fill, print click, zoom,
scroll) is inside gpui-agent's coverage. The plug's own `screenshot` verb can
only capture the MAIN window (nsh-gpui-agent-macos shoots
`surface.root().ns().window()`; no window selection in the protocol), so window
pixels are observed with the OS's screen capture, which the proof rules allow
("recording the screen is fine"). No OS input was used anywhere.

## What the shots show (short version; the full row-by-row reading is in the job report)

- 495 after, 1701 page 2: item 19 fills its own field ("SPECIMEN OTHER NON-OPERATING
  INCOME LONG DESCRIPTION") and continues on ONE white strip line ("WRAPPING OVER
  SEVERAL LINES OF PRINT"); the strip's amount columns are grey with no comb ticks.
  Item 20 ends on a whole word with "..." ("...ALSO WRAPPING...") with no strip line.
  At main both items printed empty fields with the whole text overprinting full-width.
- 495 after, 1601C item 20: the specify text fills its own field and continues on one
  strip line; the strip is grey across the amount columns including the decimal-point
  column. At main the field was empty and the text overprinted full-width.
- 499 after, 1702Q Schedule 4 with the overflowing item 2: the WHOLE Schedule 4
  column prints as uniform right-aligned lines ("12,345,678,901,234"), no comb ticks —
  while Schedules 2 and 3 on the same page keep their combs. At main the digits
  printed spread across and beyond the comb positions.
- 499 after, zero draft: Schedule 1 column A rows 1-9 print WITHOUT comb ticks
  (plain right-aligned zeros), column B keeps its combs; at main BOTH columns carry
  combs. See the report — this difference is in these PNGs either way.
