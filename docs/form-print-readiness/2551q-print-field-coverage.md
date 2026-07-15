# Form 2551Q Print Field Coverage

Status: implemented contract reconciliation; release blockers remain

Form revision: January 2018 ENCS (`2551Qv2018`)

Branch: `codex/print-preview-parity`

This checklist is the release truth for the 2551Q HTML renderer. A field is
covered only when the application owns it in Rust, validates it where required,
serializes it consistently to the renderer and BIR XML, and renders it without
recomputing tax values in JavaScript.

## Pinned source and geometry

| Artifact | Expected value |
| --- | --- |
| Official source | `https://bir-cdn.bir.gov.ph/local/pdf/2551Q%20Jan%202018%20ENCS%20final%20rev%203_copy.pdf` |
| Official PDF SHA-256 | `1f270ecf66d778836a14697863e420ff65d5ed0a5576a6cf58b97c9a8e8c9b24` |
| Item 13 legal source | BIR RMO No. 23-2018, section II(7.1)(b): `https://bir-cdn.bir.gov.ph/local/pdf/RMO%20NO.23-2018.pdf` |
| Page count and size | 2 pages, 612 x 936 points |
| Pinned capture size | 1224 x 1872 pixels at 144 DPI |
| `form_structure.json` SHA-256 | `1132a9e78373f2701acddec1e06e10346e0117561b500fdbf12460d2790345f9` |
| `formtype.json` SHA-256 | `1e6beb14024a1335c22d8e0506af8dbb0b56084d8c877c7bf1e2e4efa35728f5` |
| Legacy page 1 SVG SHA-256 | `e62c392a3962ba4c2c31ffcb4b77a7798140473a2af99abf95173680536db599` |
| Legacy page 2 SVG SHA-256 | `377ec4cee07cbff674686926aa0d402ec068b9a70fe3e8dbfc9802e90902f47a` |
| Golden page 1 PNG SHA-256 | `c78f0724e2f320f1b306408008e9085ed36397c4e1add66bf5e77c322a3485ea` |
| Golden page 2 PNG SHA-256 | `d6ab5afbf6b3f4cbac7c69a01df231eaf6dcf7fde587e78c02ee20e3f2508d1a` |
| Discrete government-seal PNG SHA-256 | `d532deb6eff07393f0dd2360526805bbcf2680c727baedbb4510ed63c58fb3f4` (verified crop from official page 1) |
| Discrete page 1 barcode PNG SHA-256 | `ddb2025f8630575db15d43b855511f50821b25bc5fa05f767b2439bd9bc45279` (verified crop from official page 1) |
| Discrete page 2 barcode PNG SHA-256 | `dd1e8dd49782640a51fb7a69bae6662ca595f73384c371d9344a1448e3530e77` (verified crop from official page 2) |
| Six-row fixture SHA-256 | `f3d49ddab5cdd7c1d889a7b2cbd519babf7556c186702f0232b9f18257f7a5b7` |
| Ten-row fixture SHA-256 | `1d5c560fa7a87325e69a1092f283cf32d839b6954dc900ab7588b35a88aa0e4d` |

The SVG pages and PNG captures are calibration and legacy-renderer inputs only.
The HTML runtime bundle must not contain or load full-page official artwork. It
embeds only the three reviewed discrete seal/barcode crops above; their source
page, crop box, treatment, and decoded hash are recorded in
`packages/form-renderer/references/manifest.json`.

The production-default legacy Typst preview still uses fixed snapshot fields.
It now fails closed instead of opening a PDF that would truncate an over-capacity
name, address, contact number, or email, or omit a non-empty Item 12A/Item 17
specification. Those drafts remain viewable in the explicit experimental HTML
preview, whose adaptive text boxes preserve the complete value; this does not
promote HTML to normal print traffic.

## Page 1 dynamic fields

| Official field | Rust model and validation | Renderer contract | BIR XML | Current status |
| --- | --- | --- | --- | --- |
| Item 1 Calendar/Fiscal | `TaxPeriodBasis`; calendar/fiscal paths tested | `tax_period_basis` | Calendar/fiscal booleans | **Covered.** |
| Item 2 Year ended MM/YYYY | `taxable_year` plus validated `year_end_month` | Rust-derived period label | Month and year fields | **Covered.** Calendar requires month 12; fiscal allows 1-12. |
| Item 3 Quarter | Validated `quarter` | `period.quarter` | Four quarter booleans | **Covered.** |
| Item 4 Amended return | `is_amended` | `is_amended` | Yes/no booleans | **Covered.** |
| Item 5 Number of sheets attached | Bounded `number_of_attached_sheets` (0-99), with an editable `5. Number of attached sheets` control | Same-named integer | `txtSheets` | **Covered and rendered.** The Page 1 filing grid prints the two-character value beside `Attached`. |
| Item 6 TIN and branch | `tin` | `taxpayer.tin` | Split 3-3-3-5 groups | **Covered.** |
| Item 7 RDO code | `rdo_code` | `taxpayer.rdo_code` | `txtRDOCode` | **Covered.** |
| Item 8 Taxpayer name | `taxpayer_name`; 160-character defensive rendering limit | `taxpayer.name` | `registeredName` | **Covered.** Page 1 and Page 2 use the official comb while the value fits, then replace the dividers with one non-truncating plain-text box when it does not. |
| Item 9 Registered address | `registered_address`; 320-character defensive rendering limit | Same-named taxpayer value | `registeredAddress` | **Covered.** Values within the official 71-character capacity use the two comb rows; longer values replace those dividers with one non-truncating plain-text box while preserving the ZIP region. |
| Item 9A ZIP code | `zip_code` | Same-named taxpayer value | `zipCode` | **Covered.** |
| Item 10 Contact number | `contact_number`; 32-digit defensive rendering limit | Same-named taxpayer value | `telNo` | **Covered.** Uses the 12-cell comb when it fits and one plain-text box when it does not. |
| Item 11 Email address | `email`; 254-character defensive rendering limit | Same-named taxpayer value | `txtEmail` | **Covered.** Uses the 28-cell comb when it fits and one plain-text box when it does not. |
| Item 12 Tax relief yes/no | `tax_relief` | `tax_relief` | Yes/no booleans | **Covered.** |
| Item 12A Tax relief specification | Required when Item 12 is yes; 160-character defensive rendering limit | `tax_relief_specification` | `txtTaxReliefSpecify` | **Covered.** Uses the 26-cell comb when it fits and one plain-text box when it does not. |
| Item 13 income-tax-rate election | The editable return owns profile-synchronized taxpayer-type, business-start-date, and annual-election snapshots and exposes explicit Not applicable, Graduated, and 8% controls. Applicability is Individual + initial-quarter return + canonical `PT010` activity, including a NIL row: Q1 for an existing taxpayer, or the quarter containing business commencement for a new registrant. Missing later-quarter legacy snapshots fail closed. Applicable returns require graduated/eight-percent, recorded annual elections must agree, and a queued initial-quarter 8% choice is saved atomically to the profile ledger. The generic Graduated box does not fabricate OSD versus itemized-deduction ledger data. Subsequent quarters require NIL PT010 for an 8% year without removing other ATCs. | `item_13_election` | Two official booleans | **Covered and rendered on Page 1 for election semantics, exact 8% persistence, and queue/XML validation.** The selected Graduated or 8% box is driven only by the Rust-owned envelope value. The separate declaration-side selector is not yet carried in the render envelope. |
| Item 14 Total Tax Due | Rust-derived from Schedule 1 | `total_tax_due` | `txt14` | **Derived and covered.** |
| Item 15 Creditable percentage tax withheld | `creditable_tax_withheld` | Same-named decimal | `txt15` | **Covered.** |
| Item 16 Tax paid on previously filed return | `tax_paid_previous`; amended-return rules retained | Same-named decimal | `txt16` | **Covered.** |
| Item 17 description | Required when amount is positive | `other_tax_credit_description` | `txt17Specify` | **Covered.** |
| Item 17 amount | `other_tax_credit` | Same-named decimal | `txt17` | **Covered.** |
| Item 18 Total tax credits/payments | Rust-derived | `total_tax_credits` | `txt18` | **Derived and covered.** |
| Item 19 Tax still payable/(overpayment) | Rust-derived; negative values preserved | `tax_payable` | `txt19` | **Derived and covered.** |
| Items 20-22 penalties | Rust validates each component as finite, non-negative, cent-precise, and within the official amount comb. Automatic penalties are recalculated at both queue revalidation and XML generation; even a coherently altered surcharge/total chain is rejected. | Same-named decimals | `txt20`-`txt22`, emitted only after validation | **Covered and fail-closed.** React does not recalculate. |
| Item 23 Total penalties | Rust-derived | `total_penalties` | `txt23` | **Derived and covered.** |
| Item 24 Total amount payable/(overpayment) | Rust-derived | `total_amount_payable` | `txt24` | **Derived and covered.** |
| Overpayment disposition | Required only for a negative Item 24; otherwise forbidden | `overpayment_disposition` | Refund/TCC booleans | **Covered.** |

## Declaration, tax-agent, and payment regions

| Official field or region | Current application state | Classification and required resolution |
| --- | --- | --- |
| Individual/non-individual declaration side | The editable Rust draft retains and validates a profile-synchronized taxpayer-type snapshot, but `RenderTaxpayer` does not expose it and the semantic declaration keeps both legal sides blank | **Renderer-contract gap.** Carry the existing snapshot into the envelope and use it only to select the correct declaration side; do not infer from the taxpayer name or Item 13. |
| Printed signatory name, title/designation, and TIN | Not captured | **Missing.** Add explicit legal metadata; never synthesize a signature. |
| Tax-agent accreditation/roll number | Not captured; XML is blank | **Missing/optional.** Capture only when a tax agent is used. |
| Tax-agent issue and expiry dates | Not captured; XML leaves both legal values blank | **Missing/optional.** Capture explicit metadata when a tax agent is used; render time is never substituted. |
| Items 25-28 payment details | Receipt path exists, but structured bank/agency, number, date, amount, and Item 28 description do not | **Missing/conditional.** Add structured values before claiming paid-return print completeness. |
| Machine validation/ROR details | Not captured | **External.** Preserve the official blank region unless verified data is imported. |
| Receiving-office/AAB stamp and receipt date | Not captured | **External.** Preserve the official blank region. |
| Handwritten/digital signature mark | Not captured | **External.** Preserve signature space; do not invent a mark. |

The semantic renderer includes the complete declaration, signature, tax-agent,
payment, machine-validation, receiving-office, and privacy regions, but keeps
unowned legal values blank.

## Page 2 and Schedule 1

| Official field | Rust model | Renderer contract | BIR XML | Current status |
| --- | --- | --- | --- | --- |
| Page 2 TIN groups | `tin` | `taxpayer.tin` | Page-2 split fields | **Covered.** |
| Page 2 taxpayer name | `taxpayer_name` | `taxpayer.name` | `txtPg2TaxpayerName` | **Covered.** |
| Schedule rows 1-6 | All row fields owned; Rust validates the official ATC code, canonical description/rate, unique ATCs, and cent-rounded tax due. The draft editor provides a searchable canonical ATC selector plus Add/Remove controls and persists/reopens six distinct lines. | All rows preserved with stable keys | First six slots | **Covered and directly editable for the official base sheet.** The editor add path caps the schedule at six lines; duplicate ATCs are rejected with instructions to combine their taxable amounts. |
| Schedule Item 7 total | `total_tax_due` | Same final total as Page 1 Item 14 | `txtTotalSched1` | **Derived and covered for 0-6 rows.** |
| Rows after row 6 | Rust retains them, owns the page-2 carry subtotal, and blocks unsupported XML submission | Deterministic 12-slot continuation pages preserve all rows; final total appears only on the final page | Existing XML has only six slots | **Preview continuation covered; submission blocked.** Native print/PDF pagination proof still keeps the promotion flag false. |
| Static ATC reference table | Rust owns the exact 22-entry January 2018 order, codes, descriptions, and rates | `generate_render_contract` emits `2551q-atc-reference.json`; the renderer imports that generated artifact and owns only category/note layout | Not dynamic | **Cross-language registry covered; visual calibration still required.** |

## Required invariants

1. `total_tax_due == sum(schedule_1[].tax_due) == final Schedule Item 7 == Page 1 Item 14`.
2. `total_tax_credits == Item 15 + applicable Item 16 + Item 17`.
3. `tax_payable == Item 14 - Item 18`; negative values remain negative.
4. `total_penalties == Item 20 + Item 21 + Item 22`.
5. `total_amount_payable == Item 19 + Item 23`.
6. Each user-entered monetary input has at most two decimal places; Items 14
   and 18 are derived from the independently serialized cent values.
7. The queue-time fingerprint binds the exact BIR field map plus taxpayer type,
   business start date, annual election, EOPT tier, automatic-penalty mode,
   fraud basis, and original-return timing status. Missing or changed
   fingerprints fail closed.
8. React only formats values supplied by the envelope.
9. More than six Schedule 1 rows may be previewed only when every row is
   present; queuing/submission remains blocked until the BIR attachment
   protocol is proven.
10. A directly edited Schedule 1 contains at most six unique canonical ATCs;
    repeated ATCs must be combined into one line.
11. The unmapped declaration selector and missing signatory, tax-agent, and
   structured payment identities remain blank and keep `contract_complete`,
   `xml_complete`, `validation_complete`, and `release_ready` false.
12. A queue generation claimed at the FTP boundary is never automatically
    retried or overwritten after a crash or transport error. The resulting
    unknown outcome requires support-assisted reconciliation; an in-app claim
    resolution workflow is not implemented in this slice.

## Current promotion decision

The 2551Q HTML renderer is **not release ready**. The support manifest keeps
every promotion flag conservative. The final integrated comparison changed
270,714 of 2,291,328 pixels on page 1 (11.814720546338194%) and 220,932 of
2,291,328 pixels on page 2 (9.642094017094017%), so both pages fail the 1%
release ceiling. The renderer now uses the reviewed official-source seal and
page-specific barcode crops; remaining visible differences are chiefly
typography, spacing, and fine geometry calibration, and are not hidden with a
full-page runtime background. The native host can open the system print dialog experimentally
after a fresh nonce-bound geometry readiness check, but producer-bound packaged
macOS/Windows print proof does not exist and direct HTML PDF export is not
implemented. Packaged-offline and rollback producers are intentionally
untrusted until real drivers exist. Normal Print Preview therefore remains on
the legacy renderer; HTML is an explicit development-only action.
