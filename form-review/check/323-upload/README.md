# Check: the upload text #323 writes for its three named samples

Independent check of `feat/store-launch-locked-inputs` at its merged head
`4efeca2d` (the merge of origin/main `92b3e201`). Report-only branch; nothing
here changes the pull request.

## What every file is

For each of the three official samples the pull request names, two files sit
side by side:

- `<sample>.ours.xml` — the XML the app would send: every `.steps.json` edit
  entered through the page's own write path (`screen_form_fill`, keyed by the
  official DOM ids, as a filer's edits; never test setters), the sample's
  period set through `screen_form_sample_period`, saved with
  `screen_form_save`, then the payload builder `to_bir_xml_payload()` on the
  saved draft. Never a live send, never Queue. The exact bytes are one long
  line as the builder emits them; compare field-wise (each `<div>key=value
  key=</div>` segment).
- `<sample>.official.xml` — an exact copy of the official
  `crates/bir-core/tests/sample-payloads/<sample>.official.xml` at that head.

What they do not show: a live filing, the paper layout, or any edit a filer
could not make. Profile boxes (TIN, RDO, e-mail) are refused by the write
path by design — the sample's profile already carries those values, so the
refusals do not affect the comparison.

## How the entry went

- The write path is the sample/library path: for all three forms a
  transaction-dated or late-period return cannot be opened with
  `screen_form_open` (the period encoding does not fit its `u8`), so the
  sample was opened and its period set to the sample's own (2552 and 2200A:
  03/14/2025; 1701MS: December 2025), as the form library offers. The 2552
  transaction choice (Local Stock Exchange) was made through the same write
  path, clicking `frm2552:rdoPg1I11TransactionLSE` as the steps do; it was
  accepted and the draft carries `transaction=local_stock_exchange`.
- Penalty boxes (Surcharge/Interest/Compromise) are refused on a plain write
  with "computed from the due date; choose Edit penalties to enter the
  amounts BIR agreed to". They were then entered the sanctioned way — the
  Edit penalties path, which passes the box in `unlocked` — and accepted.
  After that, 2552's item 24 total penalties and item 25's penalty part
  match the official exactly.

## What matches

Number format, thousands separators and decimals match character for
character everywhere except the lines listed below: every compared value is
rendered with the same comma grouping and the same decimal places as the
official, and untouched boxes hold the same values in both files (blank
stays blank, `0`/`0.00` exactly as the official carries them).

**1701MS-122025** — the boxes this pull request opens and what is computed
from them match exactly: item 2A `txtLessNonTaxablea` = `90,000`, item 2B
`txtLessNonTaxableb` = `90,000`, and the downstream 3A/3B-chain boxes
(`txtTaxableCompensationIncomea/b` = `810,001`/`510,000`, `txtGrossa/b` =
`900,001`/`600,000`, `txtGrossIncomeNo9a` = `1,490,000`,
`txtNetIncomeLossNo12a` = `894,000`) are byte-identical to the official.

**2552-03142025** — items 17, 19, 20 and 25 do **not** match; see below.

**2200A-03142025** — Others rows 2 and 3 match exactly (blank, check false,
due `0.00`), row 1's rate/bracket/taxable/description/ATC and checkbox all
match; its tax due does **not**; the schedule total and Part III lines
inherit that one difference.

## Every line that differs

### 1701MS-122025 — 6 differing fields of 201, all penalties, none in the boxes this pull request opens

```
frm1701MS:txtSurchangeNo28a:            ours '49,625'   official '0'
frm1701MS:txtInterestNo28a:             ours '14,618'   official '0'
frm1701MS:txtCompromise28a:             ours '20,000'   official '0'
frm1701MS:txtTotalPenaltiesNo28a:       ours '84,243'   official '0'
frm1701MS:txtTotalAmountPayableNo29a:   ours '282,743'  official '198,500'
frm1701MS:txtAggregateAmountPayable30a: ours '282,743'  official '198,500'
```

Cause: the sample path starts from the form library's sample draft, which
carries pre-entered penalties (the sample is an illustration); the official
filer's steps never clear them because a blank return has none. A filer
opening a fresh return would not see them. All six are outside the boxes
this pull request opens.

### 2552-03142025 — 4 differing fields of 238

```
frm2552:txtPg1P2I17TaxPaidInPrevRtrn:  ours '0.00'      official '100.50'
frm2552:txtPg1P2I19TotTaxCredPmnt:     ours '1,000.00'  official '1,101.00'
frm2552:txtPg1P2I20TotTaxStllDueOvrPmnt: ours '12,422.00' official '12,321.00'
frm2552:txtPg1P2I25TotAmntPyable:      ours '12,458.00' official '12,357.00'
```

Item 17 — the box this pull request opens behind Amended Return = Yes — was
**refused by the write path**: "`frm2552:txtPg1P2I17TaxPaidInPrevRtrn` is
computed by BIR's own formulas". This with Amended Return = Yes accepted and
carried in the draft (`is_amended=true`, verified after the click and again
after the transaction choice), with the write retried alone, and even with
the box passed in `unlocked` (which lifts profile/penalty locks; it did not
lift this one). Items 19, 20 and 25 differ only by the missing 100.50 plus
its rounding — the cascade of the item-17 refusal; every other compared box,
including the penalties entered through Edit penalties, matches exactly.
Note: the pull request's own crate test fills this box successfully on a
draft built by `Form2552Draft::new_from_profile`; the refusal above is what
the sample/library path produces at the merged head. Which of the two a
filer's New-return page hits was not established here.

### 2200A-03142025 — 7 differing fields of 274

```
frm2200Av2020:txtSched1_xa_others1_Due: ours '0.00'      official '11.00'
frm2200Av2020:txtSched1_TotalDue:       ours '87,440.56' official '87,451.56'
frm2200Av2020:txtExciseDue:             ours '87,440.56' official '87,451.56'
frm2200Av2020:txtNetTaxDue:             ours '86,205.99' official '86,216.99'
frm2200Av2020:txtStillDue:              ours '86,205.99' official '86,216.99'
frm2200Av2020:txtAmtPayable:            ours '87,241.49' official '87,252.49'
frm2200Av2020:txtBalance:               ours '-111.00'   official '-100.00'
```

The Others row 1 tax due was **refused by the write path**:
"`frm2200Av2020:txtSched1_xa_others1_Due` is computed by BIR's own
formulas" — yet the recompute does not derive it either: the row carries
taxable `2.00` and rate `5.5` (both accepted and matching the official), and
2.00 x 5.5 = 11.00 never appears. The box is neither writable nor computed
at this head, so it is unfillable through the page; every other difference
is exactly that missing 11.00 cascading through the schedule total and the
Part III lines. (The row's checkbox is app-written by design — it follows
the row's content — and both files carry it `true`.) The sibling pull
request #326 is the one that touches unfillable rows; this head does not
include it.
