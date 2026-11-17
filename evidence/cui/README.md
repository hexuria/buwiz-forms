# CUI audit-fix evidence

Screenshots of the audit findings applied in commit `b1097c9`
(`form: apply round-2 audit findings to the CUI session and renderer`),
captured live on `DISPLAY=:1` against the seeded taxpayer
(Juana Santos, TIN 000-123-453-00000).

## Shot index

| File | What it shows | Findings |
|---|---|---|
| `01-form-top-item-chips.png` | Form 1601C (sample) top: printed item-number chips (`1.`, `2`, `3.`), required `*` markers, per-field sub-labels (`Return period month` / `year`), empty select showing **Choose…** | I-8, R-4, I-3, I-14 |
| `02-tin-segments-profile-fold.png` | Part I expanded: TIN rendered as segmented 3-3-3-5 boxes (`TIN segment 1 000 - … branch code 00000`), "From tax profile" fold header, radio groups with plain choice labels | I-2, I-13, I-11, I-19 |
| `03-profile-fold-expanded.png` | Same fold expanded: tucked profile items (RDO code, withholding agent name, registered address) each with "From tax profile. Edit changes this return only" + Edit action | I-13 |
| `04-profile-fold-collapsed.png` | Same fold collapsed: tucked items skipped, only "> From tax profile 6 filled boxes" remains — proves hidden state actually hides rows | I-13 |
| `05-schedule-part.png` | Part IV Schedule: "Schedule I - Adjustment of Taxes Withheld…", "No rows yet." + "+ Add row", computed "Total Adjustment 0.00" card, "Not on the printed sheet" section | I-15, I-16 |
| `06-schedule-row-editor.png` | Schedule row drawer ("Row 1") with stacked per-column fields and computed cells (Adjustments = 6 less 5) | I-15, I-18 |
| `07-multifield-penalties-computed.png` | 1601C Part II: multi-field items with sub-labels (item 20 "Other non-taxable compensation description / amount"), penalty items with "Edit penalties" action, computed totals ("Total taxable compensation 0.00"), conditional "Needs Amended Return = Yes" marker | I-3, I-9-adjacent, I-16 |
| `08-form-1701-tin-atc-dob.png` | Form 1701 Part I: TIN segments again, ATC select with real option list, "10 Date of Birth (MM/DD/YYYY) *" date field, "12 Citizenship *" | I-2, I-14, I-7-adjacent |

## Findings covered by these shots

I-1/I-13 (folds), I-2 (TIN segments), I-3 (multi-field rows + sub-labels),
I-8 (item chips), I-11 (Y/on handling — visible via radios), I-13, I-14
(Choose…), I-15/I-16 (schedule cells + totals), I-18 (row editor state),
R-4 (required markers).

## Findings verified but not pictured

These were verified in the earlier interactive sweep
(`cui/FORM-PORT.md`, commit `9f6cc32`) or need a specific click path:

- **Calendar picker** (real day/month/year popover): open any `t:"date"`
  field, e.g. 1701 → Part I → item 10 "Date of Birth" → calendar icon
  button beside the box.
- **Searchable select** (I-6): any select with >12 options, e.g. 1701
  item 7 "Alphanumeric Tax Code (ATC)".
- **Election hints** (I-9) and **disabled-until-pick Save** (I-17): forms
  with election items (e.g. 1701Q/2551Q amendment elections).
- **Locked ReadBox** (I-10/I-12): open a *real* return (not a library
  sample) so profile-locked fields render with lock icon + ₱/% units.
- **Per-field invalid styling** (I-4): run Check on a form with a
  validation message naming a specific box.
- **"Additional details" fold** (I-1): forms with unplaced fields —
  look for "N more fields not yet placed on the paper layout".
- **Max-length enforcement** (I-5): type past `f.m` in any capped text
  field (e.g. ATC, bank codes).

## Reproduce

```bash
cd cui && ./run-dev.sh   # DISPLAY=:1, dev env vars preset
```

Open Form library → pick a form → expand parts via the left nav.
