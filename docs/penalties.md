# BIR Penalties Reference

Source captured: 2026-05-16 Asia/Manila

Primary source:
- BIR penalties page: https://www.bir.gov.ph/penalties
- Linked compromise schedule on that page: RMO No. 7-2015 Annex A PDF, https://www.bir.gov.ph/images/bir_files/internal_communications_3/Full%20Text%20of%20RMO%202015/RMO%20No.%207-2015%20Annex%20A.pdf

Related current micro/small taxpayer sources:
- RR No. 6-2024: https://bir-cdn.bir.gov.ph/BIR/pdf/RR%206-2024%20%28final%29.pdf
- BIR Form 1701-MS August 2024 guidelines: https://bir-cdn.bir.gov.ph/BIR/pdf/1701-MS%20Guide%20August%202024%20ENCS_Final.pdf

## Tax Returns With Tax Due

The BIR penalties page lists three additions when a return is filed late with tax due:

1. Surcharge under NIRC Sec. 248.
2. Interest under NIRC Sec. 249.
3. Compromise penalty under NIRC Sec. 255 and RMO No. 7-2015 Annex A.

The page states a 25% surcharge applies when a taxpayer fails to file and pay by the prescribed date, files with the wrong revenue officer unless authorized, fails to pay a deficiency tax within the assessment notice period, or fails to pay the full or partial tax shown on a return by the payment deadline.

The same page still describes interest as 20% per annum. That should be treated as page text, not necessarily the current computation rule. RR No. 6-2024 and the 1701-MS guide reflect the EOPT/current micro/small framework: micro and small taxpayers use 10% surcharge, 6% interest, 50% surcharge for fraud or willful neglect, and 50% of the applicable compromise amount. For regular, medium, and large taxpayers, the current implementation uses 25% surcharge, 50% fraud/willful-neglect surcharge, and 12% annual interest.

Compromise penalty for late filing/payment with tax due:

| Amount of tax unpaid | Compromise |
| ---: | ---: |
| up to P5,000 | P1,000 |
| over P5,000 up to P10,000 | P3,000 |
| over P10,000 up to P20,000 | P5,000 |
| over P20,000 up to P50,000 | P10,000 |
| over P50,000 up to P100,000 | P15,000 |
| over P100,000 up to P500,000 | P20,000 |
| over P500,000 up to P1,000,000 | P30,000 |
| over P1,000,000 up to P5,000,000 | P40,000 |
| over P5,000,000 | P50,000 |

## Tax Returns With No Tax Due

For late filing of tax returns with no tax due, the BIR penalties page applies compromise penalties based on gross sales, earnings, receipts, gross estate, or gift from the subject return or information filing.

| Gross sales/receipts/etc. | Compromise |
| ---: | ---: |
| up to P50,000 | P1,000 |
| over P50,000 up to P100,000 | P3,000 |
| over P100,000 up to P500,000 | P5,000 |
| over P500,000 up to P5,000,000 | P10,000 |
| over P5,000,000 up to P10,000,000 | P15,000 |
| over P10,000,000 up to P25,000,000 | P20,000 |
| over P25,000,000 | P25,000 |

## Statements Or Reports With No Tax Due

For late filing of statements or reports with no tax due, the BIR penalties page points to NIRC Sec. 275 for violations where no specific penalty is provided.

For information returns, statements, lists, records, or supplied information required by the Code or Commissioner, the page also cites NIRC Sec. 250: P1,000 per failure, capped at P25,000 per calendar year. RR No. 6-2024 reduces this for covered micro and small taxpayers to P500 per failure, capped at P12,500 per calendar year.

## Implementation Notes

Reviewed implementation:
- `crates/bir-core/src/penalties/engine.rs`
- `crates/bir-core/src/penalties/config.rs`
- `crates/bir-core/src/penalties/compromise.rs`
- `crates/bir-core/src/forms/form_2551q.rs`

Findings:
- Surcharge and interest configuration now distinguishes pre-EOPT and current EOPT micro/small penalty treatment. Micro/small reductions apply only for due dates on or after the EOPT effective date.
- Interest now preserves the TRAIN-era transition: liabilities due before January 1, 2018 accrue 20% interest before that date and 12% interest from January 1, 2018 onward.
- Fraud/willful-neglect surcharge at 50% is represented.
- The amended-return waiver now applies only to covered micro/small taxpayers when the original return and tax due were filed/paid on time and the return is not fraudulent or willfully neglected. The 2551Q draft no longer assumes every amended return qualifies.
- The 2551Q view exposes an amended-return follow-up toggle for whether the original return was filed and paid on time, so the waiver is no longer hidden in core state only.
- The tax-due compromise table already matched the BIR page.
- The no-tax-due compromise table did not match the BIR page thresholds. It has been corrected in `crates/bir-core/src/penalties/compromise.rs`.
- Automatic penalty computation is currently wired through `crates/bir-core/src/forms/form_2551q.rs`. Other implemented forms expose surcharge, interest, and compromise as manual draft fields and only aggregate the totals.
