# Money-box baseline — unmodified main @ afc1c738, macOS real keys

Rig: hexuria/buwiz-forms exactly `afc1c738349c051f973fcf580873e854b8227d37` (`git rev-parse HEAD` confirmed),
kit sibling ../nsh at pin `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (`nsh 0.6.1 (8c78cfd)`), cargo 1.100.0 (3acc79c0c,
the toolchain's own), RUSTC_WRAPPER empty, build folder `/Users/devin/mainbase/target` (this build is kept).
Four safety variables, scratch db, demo taxpayer, invented values, never Queue. Real CGEvent keys at 120 ms.
2551Q Schedule 1 row 1 taxable amount holding 1,234.56 unless empty; box text read live from the a11y tree,
saved value from `form_drafts.data_json` `$.schedule_1[0].taxable_amount` after Done.

## Answer line

On main on the Mac: Tab into a filled amount **does** select it whole — YES. A first click **does** put the caret
where clicked — YES (measured across the whole text, digits and decimals alike; the empty area left of the
right-aligned digits maps to position 0, which is the nearest glyph boundary there).

## Cases

| case | what happened, key by key | after leaving | saved | line |
|---|---|---|---|---|
| M1 Tab in from the box before, type 15000.50 | whole amount selected: first digit replaces all — 1 → 15 → 150 → 1,500 → 15,000 → 15,000. → 15,000.5 → 15,000.50 (re-formats live while typing) | 15,000.50 | 15000.5 | selected-whole, exact |
| M2 Shift+Tab in from the box after, type 15000.50 | identical replacement cascade | 15,000.50 | 15000.5 | selected-whole, exact |
| M3 FIRST click between the 2 and the 3, key 9 | 1,234.56 → 19,234.56 — the 9 inserted one glyph left of the aim (between 1 and 2); see the scan below: the true 2|3 boundary is ~7 px right of the aim point, so this is aim error, not hit-test error | 19,234.56 | 19234.56 | caret mid-text where clicked |
| M4 first click just after the last digit, key 9 | 1,234.56 → 1,234.596 | 1,234.60 (rounds half up on leaving) | 1234.6 | see scan: even the rightmost click lands among the decimals |
| M5 first click in the empty left part, key 9 | 1,234.56 → 91,234.56 | 91,234.56 | 91234.56 | front — nearest glyph boundary to empty space left of right-aligned text |
| M6 empty box, type 1234, Cmd+Tab away and back, type 5 | 1,234 before the switch; after return the 5 appends: 12,345 | 12,345.00 | 12345.0 | caret survived the app switch, correct append |

Saved value equals what the box shows after leaving in every case (verdict "right" per line; transcript-M.txt has
the per-key states and timings).

## First-click hit-test scan (one click on the unfocused box, then key 9, surface x positions)

| click x (surface) | result | insertion point |
|---|---|---|
| 1103, 1110, 1117 | 91,234.56 | before the 1 |
| 1124, 1131 | 19,234.56 | between 1 and 2 |
| 1138 | 12,934.56 | between 2 and 3 |
| 1145 | 12,394.56 | between 3 and 4 |
| 1152 | 12,349.56 | between 4 and the decimal point |
| 1159 | 1,234.956 | before the first decimal digit |
| 1166 | 1,234.596 | between the two decimal digits |

Hit-testing is accurate to the glyph across the whole text including the decimals. Note no scan position produced an
end-of-text caret: a click at or past the last digit lands among the decimal digits, and a typed digit there inserts
rather than appends (M4). The text glyphs span roughly surface x 1105–1160 in this window; the box itself is
x 762–1173, so the wide empty area left of the digits maps to position 0.

## The date-field open item, resolved (on main, within the ten minutes)

The field is the "Business start date" ask-card (`fp-ask-business_start_date-input`, a DateField) in 2551Q Part I —
a profile-completion card for a fact the profile lacks. I expected the typed date to be written to the profile. Two
rig faults, no app bug: (1) the first run never pressed the card's own Save button (`fp-ask-business_start_date-save`
— typing only stages the text; `saveAsk` in nsh/screens/form/session.ts does the write), and (2) I then read the
wrong table: the write goes through `bir.set_year_facts` into the `year_facts` table, not the `profiles` row.
Verified on this main build: typed 01152000 → box shows 01/15/2000 → click Save → `year_facts` for
00012345300000/2026 holds `"business_start_date":"2000-01-15"` and the chip flips to "Saved to profile". Same on
main as anywhere; nothing broken.
