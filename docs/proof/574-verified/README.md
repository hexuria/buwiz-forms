# #574 verified: printed returns and the values they hold

Verdict on hexuria/buwiz-forms#574 ("Printed returns show every value the
return holds (boxes still printed blank)"), checked key by key on main
(`b983b2a4e`). **Nearly every census entry was an artifact: the values print
correctly.** The census's two blind spots explain why:

1. **Comb digits print as one-per-slot `<span>` text**, never as a
   contiguous string and never as input `value=` attributes — a readback
   that searches for the digit string or scans input values sees nothing.
2. **Many print keys are derived** (e.g. `txtDateYear` is
   `put("txtDateYear", format!("{:02}", death_year))` — two digits by
   design, matching the comb). A direct synthetic fill of the derived key
   is overwritten by derivation in real use and proves nothing either way.

## Method

Each key was filled directly through the committed print path
(`print_path_render` example on `try/n4-band-cap-evidence`:
`filled_document` → `prepare_html_for(slug, …, true)` → `staticize`,
the same pipeline the print preview uses) with a distinctive value
(`ZZPROBE…` for text, unusual digit strings for money), and the output
was read back three ways: full text extraction of every element in
document order, per-cell comb slot spans (`data-slot-index`), and input
`value=` attributes. Physical absence claims were verified by rendering
the blank sheets and reading them row by row.

## Key-by-key results

| form | key | verdict |
|---|---|---|
| 2551Q | txt22 / txt23 / txt24 | **prints** — peso digits in the row's comb slots (p1c84/88/92 + cent combs); the census draft simply never populates penalties |
| 1801 | telephoneNumber | **prints** — digits in the contact comb |
| 1801 | txtInstallmentYear | **prints** |
| 1801 | registeredAddress_1 | **prints** |
| 1801 | txtDateYear | derived (`{:02}` of death_year) — two-digit by design; not probeable by direct fill, correct in real use |
| 1800 | DonorTaxDue, txtTaxPayable, TotalAmountPayable, TotalNetGifts* | **prints** — including negatives, with the minus sign in the comb (e.g. `−15000` across slots 4–9 of p1c70) |
| 1800 | txtPg2TaxpayerName | **prints** |
| 1707 | txtPg1I6SellerAddr0/1/2 (registered address) | **prints** — span text in p1c26 ff., page 1 (page 2 carries only TIN + name) |
| 1707 | ProfileAddress1/2, ProfileZipCode | storage keys upstream of the SellerAddr writer keys; the print pipeline writes the SellerAddr keys, which print |
| 0605 / 2553 | txtTelNum | **prints** |
| 2200AN | txtAddress | **prints** |
| 1600PT / 1604C | txtPg1I1Year / txtYear | **prints** |
| 2550M | txtRDOCode, txtZipCode | **prints** |
| 1707A | txtS1I20TotalCost, txtS1I20TotalSellingPrice, txtS2I21TotalCost, txtS2I21TotalSellingPrice | **no box on the official sheet** — the physical TOTAL rows total only column E (and F): the labels read "5 TOTAL (Total of Column E, to Part II Item 13) / (Total of Column F, to Part II Item 18A)" and "(Total of Column E, to Part II Item 14)"; verified on the rendered page 2 |
| 1707A / 1701A / 2200P | txtLineOfBusiness / txtLineBus / txtLOB | **no box on the official sheet** — no line-of-business item anywhere on the rendered sheets (1701A page 1 runs TIN → name → registered address → birth → contact → civil status with no LOB) |
| 1600WP | derived:att_from/to mm/dd/yyyy | **open — the one unresolved entry.** The writer-cells joins exist (p1c62–p1c67) and a control key (txtTaxpayerName) prints on the same render, but even a direct fill of the derived keys leaves the six small input combs empty. Either the fill path mishandles these multi-input cells or the derivation never reaches them in real use; needs one focused look. |

## Decision applied

Per the owner: values the official sheet has no box for stay off the
print, because the official program doesn't print them either; positions
are never invented. On that rule the cost/selling-price totals and the
line-of-business keys are correct as they are, and the reviewed
`not_joined` reasons in `writer-cells.json` already say why.

## What this means for the issue

Every probeable entry in the original blank-value census prints
correctly on current main. The print coverage gates (`unprinted_keys`,
the reviewed `not_joined` lists) are doing their job. The single open
thread is the 1600WP attestation row; everything else is verified
correct or correct-by-design.

Probe script: `probe_prints.sh` in this directory — usage and the exact
render/readback commands are inside it.
