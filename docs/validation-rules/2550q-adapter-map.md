# 2550Q v2024 adapter inventory

Status: inventory plus non-authorizing adapter foundation. This document does
not approve the source-pinned v2 candidate as a reviewed rule snapshot, select
official-versus-filing-safe production behavior, define a complete
serialization contract, or change any form capability or release status.

## Implementation checkpoint

The core draft now persists strict monotonic stable IDs for all seven repeating
row families and a versioned raw-editor state that distinguishes missing,
explicitly absent, present blank, exact text, and malformed input. The
non-authorizing `Form2550QFieldValueSource` has a closed inventory of 66
rule-backed singleton buffers (39 ordinary controls plus 27 raw-only candidate
buffers, including the twelve filing-basis/quarter/classification/treaty
choices, Item 2 taxable year, Items 10-12 identity/contact fields, and the raw
TIN, branch, RDO, and taxpayer-name captures),
20 separately classified
local-print buffers, and the exact 28 members in seven logical groups. It
preserves raw values, sorts group instances by stable identity, reports
incomplete live-control coverage, and fails closed on unknown keys, unsupported
state versions, malformed IDs, duplicates, and orphan repeated values.

Checked draft validation and checked XML generation apply the same read-only
raw-binding checks. All seventeen raw-only candidate buffers begin missing
instead of fabricating typed or profile defaults and restore only existing raw
state. The twelve choice buffers materialize complete mutually exclusive
groups as exact `true`/`false` text on an explicit click; Item 2, Items 10-12,
Item 14A, and the Item 19, Item 42, Item 47, and Item 56
description/amount pairs capture exact live text before typed parsing, while
exact XML import seeds their persisted raw text.
Each exposes a visible semantic focus target and advances the input revision
once per user action. A missing raw group renders with no authoritative
selection even if the typed draft has a legacy default. Checked XML requires
all twenty-six raw-authority keys and rejects missing, partially captured,
malformed, or raw/typed-incoherent candidate values instead of synthesizing
them from typed state. Stable IDs and
raw state do not enter the reviewed 160-key XML map, additional-item rows
remain non-serializable, and no rule identity, provider, artifact, release
flag, or queue capability is selected. The sections below retain the original
inventory observations where useful, with current implementation checkpoints
called out explicitly.

The executable candidate field surface contains 66 singleton identities, all
closed by the core/GPUI inventory. Amended-return Yes/No and short-period
Yes/No use independent raw buffers and visible focus targets; no typed fallback
fabricates their package values.

The test-only v2 candidate now contains four typed workflow states:
`edit`, `validated`, `submission-enrollment`, and `submission-attempted`.
The two source-ordered official transitions between `edit` and `validated`
remain executable only in candidate tests. `validate-success` requires the
exact valid Validate request/result and emits the source-exact success alert.
`edit-after-validation` consumes that same unchanged Validate result, returns
to `edit`, and emits the exact Edit alert.

Two further source landmarks are machine-readable but deliberately
non-executable. `final-copy-open-enrollment` records the pinned reachable
combined Final Copy/Submit button branch and consumes the prior Validate
phase. `submit-after-enrollment` records the later credential, fresh Save
preflight, artifact-staging, encryption, and transport sequence and declares
Save as its evaluated phase. Both official branches are `documented_only`;
both filing-safe branches are unresolved. Each transition declares its
evaluated phase, and the action name is not used to invent one. The generic
core dispatcher therefore rejects the Final Copy/Submit edges while
preserving the shadow/trusted provider boundary. The desktop validation
controller can retain the exact validated semantic result while it is current,
clears it on raw edits, context changes, replacement evaluations, or
unavailable/incomplete state, and does not itself disable controls, select a
production provider, or authorize Final Copy, Upload, or Submit.

## Source-bound conflict decisions

The following inventory conflicts are now resolved for an **official-profile
candidate only** by direct inspection of the pinned
`official-hta-runtime` (`sha256
3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`).
They do not approve a filing-safe branch or change current UI behavior.

| Decision | Official source | Candidate treatment |
| --- | --- | --- |
| Item 35B `lessOutputVat` and Item 36B `addOutputVat` | HTA lines 1886–1911 render enabled numeric inputs; `compute37AB` consumes both | Raw decimal inputs. The v1 `computed: true` flags are extraction metadata errors. |
| Schedule 1 allowable input tax and balance | `computeSched1`, lines 6013–6083, derives `input tax / recognized life * 3` and the remaining balance; dynamic controls at lines 6112–6113 are disabled | Per-row derived outputs, not authoritative raw inputs. |
| Schedule 3 `txtTotalTaxWithHeld3{N}` | `computeSched3`, lines 6358–6386, sums the row values; dynamic control at line 6413 is enabled | Raw per-row decimal input. The v1 `computed: true` flags are extraction metadata errors. |
| Items 44B, 45B, and 46B | HTA lines 2124–2180 render the input-tax controls disabled; `compute44AB`–`compute46AB`, lines 7578–7593, multiply the corresponding purchase amount by 12% | Derived decimal outputs using exact reviewed 12% arithmetic. |
| Item 47B `otherSpecify47B` | HTA lines 2212–2216 render it disabled; `compute47AB`, lines 7595–7599, derives it from Item 47A at 12% | Derived decimal output. The unprefixed key is the exact concrete field ID; the prefixed calculation token is an alias, not a second field. |

These decisions resolve the role/alias ambiguity only. Exact decimal
rounding, trigger order, invalid-input behavior, group cardinality, profile
differences, and artifact serialization still require executable fixtures.

The inventory compares the 188 v1 field records in
`rules/forms/2550q-v2024/fields.json` with the calculation, validation, and
workflow observations beside them and with the current 2550Q core/XML/view
implementation. A "binding" below means only that the current handwritten code
has a traceable access path. It does **not** prove that the v1 behavior is
correct, that raw editor text is preserved, or that either XML artifact can be
materialized losslessly.

## Inputs inspected

| Input | SHA-256 at inventory time |
| --- | --- |
| `rules/forms/2550q-v2024/fields.json` | `5e7b5a3bfe175c4ca1cf1ffc33db216174926ae97b89ddde0bcd9e4513c1ed06` |
| `rules/forms/2550q-v2024/calculations.json` | `285a3ad8c28788ed375f6d4c0e3244eaf8df8cbc4d040ed3f53367706357b65b` |
| `rules/forms/2550q-v2024/validations.json` | `9de9215c9c4dfc70f21987e9a624957efe104727dab7afabc4abf9f6b387a374` |
| `rules/forms/2550q-v2024/workflow.json` | `b22249da8bd7d49c16a279618cd77fbacc1c2b817f59c83a78ce2e572b062c85` |
| `crates/bir-core/src/forms/form_2550q.rs` | `b9bbdf9d9af9587d3d89ccb7e1085b97c3fb93e22ea68261ed4b841c0cafda71` |
| `crates/bir-core/src/forms/form_2550q_xml.rs` | `a82f6a44534201717688fe4a884e9f80ad269c9e52c13bb492c63fcf8c8acb64` |
| `crates/bir-core/src/form_rules/form_2550q.rs` | `2c0514fb3e30aa601cbfa41314d9e6d85e4bb3e2804692b93fa948ede2c342fc` |
| `crates/bir-desktop/src/views/form_2550q_view.rs` | `9d791fa73e162e252aeb98acfe6010fd1a706af5a781dfb6fe28a44faeacbea8` |

Supporting count/order evidence was read from the already-pinned
`plaintext-field-audit-v796.json` and `encrypted-field-audit-v796.json`
fixtures. Those fixtures are evidence inputs, not runtime contracts.

## Classification rule and complete count

The categories are mutually exclusive and use this precedence:

1. an unkeyed `runtime-indexed-family` record is a logical repeating-group
   member descriptor;
2. a concrete record marked `computed: true` in v1 but backed by an editable
   current `InputState` and not derived by `recompute()` is a UI raw-buffer
   mismatch;
3. a concrete record emitted from a current recomputed field/total and checked
   on import is a computed output;
4. a concrete record that is a serializer literal, transport context,
   metadata, or preserved serializer-only value is
   serialized-only/context/default;
5. a remaining concrete record with a traceable semantic draft/XML path is an
   exact draft/XML binding;
6. anything left would be currently unmapped.

This is an inventory classification, not a review decision. In particular,
category 2 deliberately records conflicts instead of deciding whether the v1
flag or the current editor is right.

| Stable category | Records |
| --- | ---: |
| Exact draft/XML binding | 103 |
| Serialized-only/context/default | 14 |
| Computed output | 35 |
| Seven logical repeating groups | 28 |
| UI raw-buffer mismatch | 8 |
| Currently unmapped residual | 0 |
| **Total** | **188** |

Concrete source records are `160 = 103 + 14 + 35 + 8`. The remaining 28
records have no `serialized_key`; they are member descriptors for seven
logical groups. All 160 concrete records have a unique serialized key and
`serialized_occurrence: 1`.

The v1 `computed` flag is not itself a coherent computed-output partition:

| Reconciliation | Count |
| --- | ---: |
| All v1 `computed: true` records | 34 |
| Concrete v1-computed records also derived by the current core | 24 |
| Concrete v1-computed records exposed as editable raw buffers | 8 |
| V1-computed repeating member descriptors | 2 |
| Current core-derived concrete outputs whose v1 record says `computed: false` | 11 |
| Current core-derived concrete outputs (`24 + 11`) | 35 |

## 103 exact draft/XML bindings

These records have an exact source key and a direct current semantic path.
Formatting, raw lexical preservation, group identity, and artifact order are
separate concerns addressed later.

### Filing period and background information (31)

| Count | Source key(s) | Current core access path | Current live raw control |
| ---: | --- | --- | --- |
| 2 | `frm2550qv2024:calendarNo1`; `frm2550qv2024:fiscalNo1` | `draft.filing_basis` through a checked boolean pair | Yes; two raw-only candidate choice buffers, materialized atomically on click |
| 1 | `frm2550qv2024:selectedMonthNo2` | `draft.year_end_month` | `fields[YEAR_END_MONTH]` |
| 1 | `frm2550qv2024:txtYearNo2` | `draft.taxable_year` | Yes; raw-only `fields[RAW_TAXABLE_YEAR]`, never initialized from the typed year |
| 4 | `frm2550qv2024:OptQuarter1`; `frm2550qv2024:OptQuarter2`; `frm2550qv2024:OptQuarter3`; `frm2550qv2024:OptQuarter4` | `draft.quarter` through a checked one-of-four | Yes; four raw-only candidate choice buffers, materialized atomically on click |
| 2 | `frm2550qv2024:RtnPeriodFromNo4`; `frm2550qv2024:RtnPeriodToNo4` | `draft.return_period_from`; `draft.return_period_to` | `fields[RETURN_PERIOD_FROM]`; `fields[RETURN_PERIOD_TO]` |
| 2 | `frm2550qv2024:amendedReturnYesNo5`; `frm2550qv2024:amendedReturnNo5` | `draft.is_amended` through a checked boolean pair | No; one typed toggle |
| 2 | `frm2550qv2024:OptShortPrd1`; `frm2550qv2024:OptShortPrd2` | `draft.is_short_period_return` through a checked boolean pair | No; one typed toggle |
| 4 | `frm2550qv2024:txtTIN1`; `frm2550qv2024:txtTIN2`; `frm2550qv2024:txtTIN3`; `frm2550qv2024:branchCode` | split/join projection of `draft.tin` | Yes; four raw-only `candidate_raw_text_fields` controls, never initialized from the typed TIN |
| 1 | `frm2550qv2024:txtRDOCode` | `draft.rdo_code` | Yes; raw-only `candidate_raw_text_fields[RAW_RDO_CODE]`, never initialized from the typed RDO |
| 1 | `frm2550qv2024:taxpayerName` | `draft.taxpayer_name` | Yes; raw-only `candidate_raw_text_fields[RAW_TAXPAYER_NAME]`, never initialized from the typed name |
| 1 | `frm2550qv2024:taxpayerAddress` | `draft.registered_address` | Yes; raw-only `candidate_raw_text_fields[RAW_TAXPAYER_ADDRESS]` |
| 1 | `frm2550qv2024:taxpayerZip` | `draft.zip_code` | Yes; raw-only `candidate_raw_text_fields[RAW_TAXPAYER_ZIP]` |
| 1 | `frm2550qv2024:taxpayerContactNumber` | `draft.contact_number` | Yes; raw-only `candidate_raw_text_fields[RAW_TAXPAYER_CONTACT_NUMBER]` |
| 1 | `frm2550qv2024:taxpayerEmailAddress` | `draft.email` | Yes; raw-only `candidate_raw_text_fields[RAW_TAXPAYER_EMAIL_ADDRESS]` |
| 4 | `frm2550qv2024:taxPayerClassification1`; `frm2550qv2024:taxPayerClassification2`; `frm2550qv2024:taxPayerClassification3`; `frm2550qv2024:taxPayerClassification4` | `draft.taxpayer_classification` through a checked one-of-four | Yes; four raw-only candidate choice buffers, materialized atomically on click |
| 2 | `frm2550qv2024:internationalTreatyYn`; `frm2550qv2024:specialRateYn` | `draft.is_availing_tax_relief` through a checked boolean pair | Yes; two raw-only candidate choice buffers, materialized atomically on click |
| 1 | `frm2550qv2024:specifyInternationalTreaty` | `draft.tax_relief_details` | `fields[TAX_RELIEF_DETAILS]`; exact pre-parse raw authority is required |

### Part II user inputs (6)

| Source key | Current core access path | Current live raw control |
| --- | --- | --- |
| `frm2550qv2024:vatPaidReturn` | `draft.part_ii.item_18_paid_on_previous_return` | `fields[ITEM_18]` |
| `frm2550qv2024:addSpecifyNo19` | `draft.part_ii.item_19_description` | `fields[ITEM_19_DESCRIPTION]`; exact pre-parse raw authority is required |
| `frm2550qv2024:otherCreditsNo19` | `draft.part_ii.item_19_other_credit_or_payment` | `fields[ITEM_19]`; exact pre-parse raw authority is required before money parsing |
| `frm2550qv2024:surcharge` | `draft.part_ii.item_22_surcharge` | `fields[ITEM_22]` |
| `frm2550qv2024:interest` | `draft.part_ii.item_23_interest` | `fields[ITEM_23]` |
| `frm2550qv2024:compromise` | `draft.part_ii.item_24_compromise` | `fields[ITEM_24]` |

### Page-two identity duplicates (5)

| Count | Source key(s) | Current core access path | Current live raw control |
| ---: | --- | --- | --- |
| 4 | `frm2550qv2024:txtPg2TIN1`; `frm2550qv2024:txtPg2TIN2`; `frm2550qv2024:txtPg2TIN3`; `frm2550qv2024:txtPg2BranchCode` | duplicate projections of `draft.tin`; import requires equality with the page-one pieces | No independent controls |
| 1 | `frm2550qv2024:Pg2TaxPayer` | duplicate projection of `draft.taxpayer_name`; import requires equality with page one | No independent control |

### Part IV user inputs (24)

| Source key | Current core access path | Current live raw control |
| --- | --- | --- |
| `frm2550qv2024:addInputVat` | `draft.part_iv.item_58b_input_vat_on_settled_payables` | `fields[ITEM_58B]` |
| `frm2550qv2024:addSpecifyNo42` | `draft.part_iv.item_42_description` | `fields[ITEM_42_DESCRIPTION]`; exact pre-parse raw authority is required |
| `frm2550qv2024:addSpecifyNo47` | `draft.part_iv.item_47_description` | `fields[ITEM_47_DESCRIPTION]`; exact pre-parse raw authority is required |
| `frm2550qv2024:addSpecifyNo56` | `draft.part_iv.item_56_description` | `fields[ITEM_56_DESCRIPTION]`; exact pre-parse raw authority is required |
| `frm2550qv2024:domesticInputTax` | `draft.part_iv.item_44b_domestic_input_tax` | `fields[ITEM_44B]` |
| `frm2550qv2024:domesticPurchase` | `draft.part_iv.item_44a_domestic_purchases` | `fields[ITEM_44A]` |
| `frm2550qv2024:domesticPurchaseNoTax` | `draft.part_iv.item_48a_domestic_purchases_no_input_tax` | `fields[ITEM_48A]` |
| `frm2550qv2024:exemptSales` | `draft.part_iv.item_33a_exempt_sales` | `fields[ITEM_33A]` |
| `frm2550qv2024:importInputTax` | `draft.part_iv.item_46b_import_input_tax` | `fields[ITEM_46B]` |
| `frm2550qv2024:importPurchase` | `draft.part_iv.item_46a_importations` | `fields[ITEM_46A]` |
| `frm2550qv2024:inputTaxCarried` | `draft.part_iv.item_38b_input_tax_carried` | `fields[ITEM_38B]` |
| `frm2550qv2024:inputVatUnpaid` | `draft.part_iv.item_55b_input_vat_on_unpaid_payables` | `fields[ITEM_55B]` |
| `frm2550qv2024:otherSpecify42` | `draft.part_iv.item_42b_other_input_tax` | `fields[ITEM_42B]`; exact pre-parse raw authority is required before money parsing |
| `frm2550qv2024:otherSpecify47` | `draft.part_iv.item_47a_other_purchases` | `fields[ITEM_47A]`; exact pre-parse raw authority is required before money parsing |
| `frm2550qv2024:otherSpecify56` | `draft.part_iv.item_56b_other_deduction` | `fields[ITEM_56B]`; exact pre-parse raw authority is required before money parsing, including captured exact blank |
| `frm2550qv2024:presumptiveInputTax` | `draft.part_iv.item_41b_presumptive_input_tax` | `fields[ITEM_41B]` |
| `frm2550qv2024:serviceInputTax` | `draft.part_iv.item_45b_nonresident_service_input_tax` | `fields[ITEM_45B]` |
| `frm2550qv2024:servicesPurchase` | `draft.part_iv.item_45a_nonresident_services` | `fields[ITEM_45A]` |
| `frm2550qv2024:transitionalInputTax` | `draft.part_iv.item_40b_transitional_input_tax` | `fields[ITEM_40B]` |
| `frm2550qv2024:vatableSales` | `draft.part_iv.item_31a_vatable_sales` | `fields[ITEM_31A]` |
| `frm2550qv2024:vatExemptImports` | `draft.part_iv.item_49a_vat_exempt_importations` | `fields[ITEM_49A]` |
| `frm2550qv2024:vatRefund` | `draft.part_iv.item_54b_vat_refund_or_tcc_claimed` | `fields[ITEM_54B]` |
| `frm2550qv2024:zeroRatedSales` | `draft.part_iv.item_32a_zero_rated_sales` | `fields[ITEM_32A]` |
| `otherSpecify47B` | `draft.part_iv.item_47b_other_input_tax` | `fields[ITEM_47B]` |

The calculations corpus describes Item 44-46 input-tax amounts and Item 47B as
calculation outputs, but does not use exact resolvable field IDs for the former
and uses the non-existent prefixed key
`frm2550qv2024:otherSpecify47B` for the latter. The v1 field records above say
`computed: false`, and the current core/view treat all four as raw inputs.
They remain in this exact-binding category; the calculation conflict is
unresolved and must not be silently converted into executable behavior.

### Schedule 2 user inputs (3)

| Source key | Current core access path | Current live raw control |
| --- | --- | --- |
| `frm2550qv2024:sched2InputTaxDirect` | `draft.schedule_2.input_tax_directly_attributable_to_exempt_sales` | `fields[SCHEDULE_2_DIRECT]` |
| `frm2550qv2024:sched2VatExemptSale` | `draft.schedule_2.vat_exempt_sales` | `fields[SCHEDULE_2_EXEMPT_SALES]` |
| `frm2550qv2024:sched2AmountInputTax` | `draft.schedule_2.input_tax_not_directly_attributable` | `fields[SCHEDULE_2_NOT_DIRECT]` |

### Concrete Schedule 1 rows (14)

The current checked XML contract binds only suffixes 10 and 11. The two
allowable/balance fields per row are listed in the mismatch category, not here.

| Source key | Current core access path | Current live raw control |
| --- | --- | --- |
| `txtDatePurchase10` | `draft.schedule_1[0].purchase_or_import_date` | `schedule_1_inputs[0].date` |
| `txtSourceCode10` | `draft.schedule_1[0].source_code` | `schedule_1_inputs[0].source_code` |
| `txtDescription10` | `draft.schedule_1[0].description` | `schedule_1_inputs[0].description` |
| `txtAmountPurchase10` | `draft.schedule_1[0].purchase_or_import_amount` | `schedule_1_inputs[0].purchase_amount` |
| `txtInputTax10` | `draft.schedule_1[0].input_tax` | `schedule_1_inputs[0].input_tax` |
| `txtEstimatedLife10` | `draft.schedule_1[0].estimated_life_months` | `schedule_1_inputs[0].estimated_life` |
| `txtRecognizedLife10` | `draft.schedule_1[0].recognized_life_months` | `schedule_1_inputs[0].recognized_life` |
| `txtDatePurchase11` | `draft.schedule_1[1].purchase_or_import_date` | `schedule_1_inputs[1].date` |
| `txtSourceCode11` | `draft.schedule_1[1].source_code` | `schedule_1_inputs[1].source_code` |
| `txtDescription11` | `draft.schedule_1[1].description` | `schedule_1_inputs[1].description` |
| `txtAmountPurchase11` | `draft.schedule_1[1].purchase_or_import_amount` | `schedule_1_inputs[1].purchase_amount` |
| `txtInputTax11` | `draft.schedule_1[1].input_tax` | `schedule_1_inputs[1].input_tax` |
| `txtEstimatedLife11` | `draft.schedule_1[1].estimated_life_months` | `schedule_1_inputs[1].estimated_life` |
| `txtRecognizedLife11` | `draft.schedule_1[1].recognized_life_months` | `schedule_1_inputs[1].recognized_life` |

### Concrete Schedule 3 rows (8)

The two `txtTotalTaxWithHeld3*` records are listed in the mismatch category.

| Source key | Current core access path | Current live raw control |
| --- | --- | --- |
| `txtDateCovered30` | `draft.schedule_3[0].period_from` | `schedule_3_inputs[0].period_from` |
| `txtDateCovered3To0` | `draft.schedule_3[0].period_to` | `schedule_3_inputs[0].period_to` |
| `txtNameWithHoldingAgent30` | `draft.schedule_3[0].withholding_agent_name` | `schedule_3_inputs[0].agent_name` |
| `txtIncomePayment30` | `draft.schedule_3[0].income_payment` | `schedule_3_inputs[0].income_payment` |
| `txtDateCovered31` | `draft.schedule_3[1].period_from` | `schedule_3_inputs[1].period_from` |
| `txtDateCovered3To1` | `draft.schedule_3[1].period_to` | `schedule_3_inputs[1].period_to` |
| `txtNameWithHoldingAgent31` | `draft.schedule_3[1].withholding_agent_name` | `schedule_3_inputs[1].agent_name` |
| `txtIncomePayment31` | `draft.schedule_3[1].income_payment` | `schedule_3_inputs[1].income_payment` |

### Concrete Schedule 4 rows (12)

| Source key | Current core access path | Current live raw control |
| --- | --- | --- |
| `txtDate40` | `draft.schedule_4[0].period_from` | `schedule_4_inputs[0].period_from` |
| `txtDate4To0` | `draft.schedule_4[0].period_to` | `schedule_4_inputs[0].period_to` |
| `txtNameOfMiller40` | `draft.schedule_4[0].miller_name` | `schedule_4_inputs[0].miller_name` |
| `txtNameOfTaxpayer40` | `draft.schedule_4[0].taxpayer_name` | `schedule_4_inputs[0].taxpayer_name` |
| `txtOfficialReceiptNumber40` | `draft.schedule_4[0].official_receipt_number` | `schedule_4_inputs[0].receipt_number` |
| `txtAmountPaid40` | `draft.schedule_4[0].amount_paid` | `schedule_4_inputs[0].amount_paid` |
| `txtDate41` | `draft.schedule_4[1].period_from` | `schedule_4_inputs[1].period_from` |
| `txtDate4To1` | `draft.schedule_4[1].period_to` | `schedule_4_inputs[1].period_to` |
| `txtNameOfMiller41` | `draft.schedule_4[1].miller_name` | `schedule_4_inputs[1].miller_name` |
| `txtNameOfTaxpayer41` | `draft.schedule_4[1].taxpayer_name` | `schedule_4_inputs[1].taxpayer_name` |
| `txtOfficialReceiptNumber41` | `draft.schedule_4[1].official_receipt_number` | `schedule_4_inputs[1].receipt_number` |
| `txtAmountPaid41` | `draft.schedule_4[1].amount_paid` | `schedule_4_inputs[1].amount_paid` |

## 14 serialized-only/context/default records

| Count | Source key(s) | Current access/default behavior |
| ---: | --- | --- |
| 2 | `frm2550qv2024:txtCurrentPage`; `frm2550qv2024:txtMaxPage` | No draft field; writer emits `"2"` and importer requires `"2"` |
| 4 | `resultOtherCreditsNo19`; `resultOtherCreditsNo42`; `resultOtherCreditsNo47`; `resultOtherCreditsNo56` | `draft.preserved_unmodeled_xml_fields[key]`; imported text is replayed, otherwise writer defaults to `"0.00"` |
| 1 | `txtFinalFlag` | `draft.xml_final_flag`; reviewed values are editable `"1"` and encrypted companion `"0"` |
| 1 | `txtEnroll` | No draft field; writer emits `"Y"` and importer requires `"Y"` |
| 3 | `ebirOnlineConfirmUsername`; `ebirOnlineUsername`; `ebirOnlineSecret` | No persisted draft fields; writer emits blank and importer requires blank |
| 1 | `txtEmail` | `draft.xml_contact_email`; distinct from taxpayer Item 12 email |
| 1 | `driveSelectTPExport` | No draft field; writer emits `"0"` and importer requires `"0"` |
| 1 | `dateFiled` | `draft.date_filed`; `YYYY/MM/DD` metadata with artifact-specific placement |

The four `resultOtherCredits*` values are semantically unmodeled, but not lost
by the current key map. They are therefore serializer-only bindings rather
than residual-unmapped records. Their meaning, formatter, and allowed values
remain unresolved.

## 35 current computed outputs

`to_bir_field_map()` recomputes a clone before emission.
`from_bir_field_map()` separately parses source computed values, recomputes,
and compares them within the current floating-point tolerance. That is useful
handwritten consistency checking, but it is not the exact-decimal v2 contract.

### Part II and Part IV outputs (22)

| Source key | Current computed access path |
| --- | --- |
| `frm2550qv2024:excessInputTax` | `draft.part_ii.item_15_net_vat_payable_or_excess` |
| `frm2550qv2024:creditableVat` | `draft.part_ii.item_16_creditable_vat_withheld` |
| `frm2550qv2024:advVatPayment` | `draft.part_ii.item_17_advance_vat_payments` |
| `frm2550qv2024:totalTaxCredits` | `draft.part_ii.item_20_total_credits_or_payments` |
| `frm2550qv2024:excessCredits` | `draft.part_ii.item_21_tax_payable_or_excess_credits` |
| `frm2550qv2024:penalties` | `draft.part_ii.item_25_total_penalties` |
| `frm2550qv2024:totalPayable` | `draft.part_ii.item_26_total_amount_payable_or_excess` |
| `frm2550qv2024:outputVatSales` | `draft.part_iv.item_31b_output_tax` |
| `frm2550qv2024:totalSales` | `draft.part_iv.item_34a_total_sales` |
| `frm2550qv2024:outputTaxDue` | `draft.part_iv.item_34b_output_tax_due` |
| `frm2550qv2024:totalAdjOutput` | `draft.part_iv.item_37b_adjusted_output_tax_due` |
| `frm2550qv2024:inputTaxDeferred` | `draft.part_iv.item_39b_input_tax_deferred` |
| `frm2550qv2024:total43` | `draft.part_iv.item_43b_total_prior_input_tax` |
| `frm2550qv2024:totalCurPurchase` | `draft.part_iv.item_50a_total_current_purchases` |
| `frm2550qv2024:totalCurInputTax` | `draft.part_iv.item_50b_total_current_input_tax` |
| `frm2550qv2024:totalAvailInputTax` | `draft.part_iv.item_51b_total_available_input_tax` |
| `frm2550qv2024:importCapitalInputTax` | `draft.part_iv.item_52b_deferred_capital_goods_input_tax` |
| `frm2550qv2024:inputTaxAttr` | `draft.part_iv.item_53b_input_tax_attributable_to_exempt_sales` |
| `frm2550qv2024:totalDeductions` | `draft.part_iv.item_57b_total_deductions` |
| `frm2550qv2024:adjDeductions` | `draft.part_iv.item_59b_adjusted_deductions` |
| `frm2550qv2024:totalAllowInputTax` | `draft.part_iv.item_60b_total_allowable_input_tax` |
| `frm2550qv2024:netVatPayable` | `draft.part_iv.item_61b_net_vat_payable_or_excess` |

### Schedule totals and allocation outputs (8)

| Source key | Current computed access path |
| --- | --- |
| `sched1TotalBalPrev` | `draft.schedule_1_previous_total()` |
| `sched1TotalBalNext` | `draft.schedule_1_next_total()` |
| `frm2550qv2024:sched2TotalSales` | `draft.schedule_2.total_sales` |
| `frm2550qv2024:sched2TotalRatable` | `draft.schedule_2.ratable_input_tax` |
| `frm2550qv2024:sched2TotalAttr` | `draft.schedule_2.total_input_tax_attributable_to_exempt_sales` |
| `sched3TotalIncome` | `draft.schedule_3_income_total()` |
| `sched3TotalTax` | `draft.schedule_3_tax_total()` |
| `sched4AmountPaid` | `draft.schedule_4_amount_total()` |

### Duplicate serialized totals (5)

| Source key | Current computed access path |
| --- | --- |
| `txtTotalAmountOfBalanceofInputTaxFromPrevious` | `draft.schedule_1_previous_total()` |
| `txtTotalAmountOfBalanceofInputTaxToBeCarried` | `draft.schedule_1_next_total()` |
| `txtTotalAmountofIncomePayment` | `draft.schedule_3_income_total()` |
| `txtTotalAmoungOfTaxWithHeld` | `draft.schedule_3_tax_total()` |
| `txtAmountPaidSched4` | `draft.schedule_4_amount_total()` |

The following 11 records are in this category because the current core derives
them even though their v1 field record says `computed: false`:

`frm2550qv2024:adjDeductions`,
`frm2550qv2024:advVatPayment`,
`frm2550qv2024:creditableVat`,
`frm2550qv2024:excessInputTax`,
`frm2550qv2024:importCapitalInputTax`,
`frm2550qv2024:inputTaxAttr`,
`frm2550qv2024:inputTaxDeferred`,
`frm2550qv2024:outputTaxDue`,
`frm2550qv2024:penalties`,
`sched4AmountPaid`, and
`txtAmountPaidSched4`.

## Seven logical repeating groups: all 28 member descriptors

These are descriptors, not 28 group objects. `N>=0` is retained exactly as v1
evidence; this inventory does not select a v2 instance-ID scheme, maximum row
count, or artifact key projection.

| # | Logical group | V1 member descriptor | Current core access path, if present |
| ---: | --- | --- | --- |
| 1 | Schedule 1 capital-good row | `txtDatePurchase1{N>=0}` | `draft.schedule_1[index].purchase_or_import_date` |
| 2 | Schedule 1 capital-good row | `txtSourceCode1{N>=0}` | `draft.schedule_1[index].source_code` |
| 3 | Schedule 1 capital-good row | `txtDescription1{N>=0}` | `draft.schedule_1[index].description` |
| 4 | Schedule 1 capital-good row | `txtAmountPurchase1{N>=0}` | `draft.schedule_1[index].purchase_or_import_amount` |
| 5 | Schedule 1 capital-good row | `txtInputTax1{N>=0}` | `draft.schedule_1[index].input_tax` |
| 6 | Schedule 1 capital-good row | `txtEstimatedLife1{N>=0}` | `draft.schedule_1[index].estimated_life_months` |
| 7 | Schedule 1 capital-good row | `txtRecognizedLife1{N>=0}` | `draft.schedule_1[index].recognized_life_months` |
| 8 | Schedule 1 capital-good row | `txtAllowedInputTax1{N>=0}` | `draft.schedule_1[index].allowable_input_tax_for_period`; currently manual |
| 9 | Schedule 1 capital-good row | `txtBalanceInputTax1{N>=0}` | `draft.schedule_1[index].balance_to_next_period`; currently manual |
| 10 | Schedule 3 creditable-VAT row | `txtDateCovered3{N>=0}` | `draft.schedule_3[index].period_from` |
| 11 | Schedule 3 creditable-VAT row | `txtDateCovered3To{N>=0}` | `draft.schedule_3[index].period_to` |
| 12 | Schedule 3 creditable-VAT row | `txtNameWithHoldingAgent3{N>=0}` | `draft.schedule_3[index].withholding_agent_name` |
| 13 | Schedule 3 creditable-VAT row | `txtIncomePayment3{N>=0}` | `draft.schedule_3[index].income_payment` |
| 14 | Schedule 3 creditable-VAT row | `txtTotalTaxWithHeld3{N>=0}` | `draft.schedule_3[index].tax_withheld`; currently manual |
| 15 | Schedule 4 advance-VAT row | `txtDate4{N>=0}` | `draft.schedule_4[index].period_from` |
| 16 | Schedule 4 advance-VAT row | `txtDate4To{N>=0}` | `draft.schedule_4[index].period_to` |
| 17 | Schedule 4 advance-VAT row | `txtNameOfMiller4{N>=0}` | `draft.schedule_4[index].miller_name` |
| 18 | Schedule 4 advance-VAT row | `txtNameOfTaxpayer4{N>=0}` | `draft.schedule_4[index].taxpayer_name` |
| 19 | Schedule 4 advance-VAT row | `txtOfficialReceiptNumber4{N>=0}` | `draft.schedule_4[index].official_receipt_number` |
| 20 | Schedule 4 advance-VAT row | `txtAmountPaid4{N>=0}` | `draft.schedule_4[index].amount_paid` |
| 21 | Item 19 additional row | `frm2550qv2024:totalTaxPayableNo19Description{N>=0}` | None; the core has only the non-group singleton Item 19 description |
| 22 | Item 19 additional row | `frm2550qv2024:totalTaxPayableNo19Amount{N>=0}` | None; the core has only the non-group singleton Item 19 amount |
| 23 | Item 42 additional row | `frm2550qv2024:totalTaxPayableNo42Description{N>=0}` | None; the core has only the non-group singleton Item 42 description |
| 24 | Item 42 additional row | `frm2550qv2024:totalTaxPayableNo42Amount{N>=0}` | None; the core has only the non-group singleton Item 42 amount |
| 25 | Item 47 additional row | `frm2550qv2024:totalTaxPayableNo47Description{N>=0}` | None; the core has only the non-group singleton Item 47 description |
| 26 | Item 47 additional row | `frm2550qv2024:totalTaxPayableNo47Amount{N>=0}` | None; the core has only the non-group singleton Item 47 amounts |
| 27 | Item 56 additional row | `frm2550qv2024:totalTaxPayableNo56Description{N>=0}` | None; the core has only the non-group singleton Item 56 description |
| 28 | Item 56 additional row | `frm2550qv2024:totalTaxPayableNo56Amount{N>=0}` | None; the core has only the non-group singleton Item 56 amount |

Current Schedule 1, 3, and 4 vectors are required by the handwritten
validator to contain exactly two rows and are serialized only with suffixes
10/11, 30/31, and 40/41. The view labels those rows by vector index. That is
not an implementation of the unbounded descriptors and does not provide stable
`RepeatedGroupInstance` IDs. The four additional-item groups have no core
vector and no UI at all.

### Group-scoped IR prerequisite

The 2550Q package cannot become executable by copying these descriptors into
`field_groups`. It first requires the closed group-scoped runtime contract:

- every calculation and rule explicitly selects `singleton` or
  `each-group(group_id)` execution; source JSON has no implicit singleton
  default;
- derived output identity is
  `(calculation_id, output_id, group_instance)`, and rule execution/violation
  identity is `(rule_id, group_instance)`;
- a current-row calculation can read current-row fields and derived outputs,
  while a singleton calculation can aggregate an expression evaluated once per
  stable group instance; and
- serialization projections select singleton, current-row, or one reviewed
  stable derived instance explicitly.

This is needed directly for the Schedule 1 allowable-input-tax and
balance-to-next-period row outputs and for any singleton total that consumes
those derived rows. Schedule 3 and Schedule 4 totals likewise need
aggregate-over-row expressions, and nine observed validation rules need either
per-group execution/effect targeting or an explicitly reviewed singleton
group-quantifier identity. Missing, duplicate, or wrong-row result coverage
must be an error.

Traversal must use persisted stable instance identity, never a transient vector
index. If visible insertion order is legally significant, the adapter's
stable-ID allocation must encode that order deterministically; lexical sorting
of arbitrary UUID-like IDs is deterministic but does not prove official row
order. The source-pinned v2 candidate contains only a test-only official
validation subset; groups, calculations, complete serialization artifacts,
filing-safe behavior, and reviewed registry activation remain absent. Landing
generic primitives or candidate rules therefore does not make 2550Q
production-executable or registrable.

## Eight UI raw-buffer mismatches

Each record below says `computed: true` in `fields.json`, but the current core
imports it as source data, the view exposes an editable `InputState`, and
`recompute()` does not derive that field. No side is promoted by this
inventory.

| Source key | Current core access path | Current live raw control | Conflict |
| --- | --- | --- | --- |
| `frm2550qv2024:lessOutputVat` | `draft.part_iv.item_35b_less_output_vat_uncollected` | `fields[ITEM_35B]` | V1 says computed and even types it as `string`; core/view treat money input |
| `frm2550qv2024:addOutputVat` | `draft.part_iv.item_36b_add_output_vat_recovered` | `fields[ITEM_36B]` | V1 says computed and even types it as `string`; core/view treat money input |
| `txtAllowedInputTax10` | `draft.schedule_1[0].allowable_input_tax_for_period` | `schedule_1_inputs[0].allowable_input_tax` | V1 schedule-row calculation output; current view explicitly labels it manual |
| `txtAllowedInputTax11` | `draft.schedule_1[1].allowable_input_tax_for_period` | `schedule_1_inputs[1].allowable_input_tax` | Same conflict for row 2 |
| `txtBalanceInputTax10` | `draft.schedule_1[0].balance_to_next_period` | `schedule_1_inputs[0].balance_next_period` | V1 schedule-row calculation output; current core preserves source value |
| `txtBalanceInputTax11` | `draft.schedule_1[1].balance_to_next_period` | `schedule_1_inputs[1].balance_next_period` | Same conflict for row 2 |
| `txtTotalTaxWithHeld30` | `draft.schedule_3[0].tax_withheld` | `schedule_3_inputs[0].tax_withheld` | V1 field says computed; v1 schedule-total calculation consumes it as an input |
| `txtTotalTaxWithHeld31` | `draft.schedule_3[1].tax_withheld` | `schedule_3_inputs[1].tax_withheld` | Same conflict for row 2 |

## Currently unmapped residual: zero records

There is no unclassified v1 field record after applying the precedence above.
This does not mean the future adapter is complete. In particular, group-member
records with no core path remain explicit group blockers, and
serializer-only records remain semantically unresolved; they are not counted
again as a residual.

## RawInputSnapshot blockers

### Current live-buffer inventory

Code inspection gives the following view-level buffer count:

| Current `Entity<InputState>` source | V1-backed buffers | Non-v1 local-print buffers | Total |
| --- | ---: | ---: | ---: |
| Singleton parsed/local `fields` map | 40 | 20 | 60 |
| Candidate raw TIN, branch, RDO, name, address, ZIP, contact, and email | 10 | 0 | 10 |
| Candidate filing-basis, quarter, amended, short-period, classification, and treaty choices | 16 | 0 | 16 |
| Two Schedule 1 rows, 9 inputs each | 18 | 0 | 18 |
| Two Schedule 3 rows, 5 inputs each | 10 | 0 | 10 |
| Two Schedule 4 rows, 6 inputs each | 12 | 0 | 12 |
| **Total** | **106** | **20** | **126** |

The 106 v1-backed live buffers include the prior 102 controls plus the four
independently captured amended-return and short-period flags. A draft-only
adapter would not be lossless:

- numeric, integer, and date synchronization parses the `InputState` text into
  typed fields; on a parse error the typed target retains its prior value while
  the malformed text remains only in the `InputState`;
- `sync_from_inputs()` still calls `recompute()` after collecting parse errors,
  so a derived value can reflect the prior typed value rather than the visible
  malformed buffer;
- money values are initialized back into controls with `"{value:.2}"` and dates
  with the core formatter, so reopening a typed draft does not reconstruct an
  earlier lexical spelling;
- all 16 serialized flag records now have independent raw `InputState` values
  with exact mutually exclusive group materialization. The amended and
  short-period controls update their typed booleans only after the explicit
  Yes/No raw group is captured;
- the five page-two identity duplicates remain derived projections with no
  independent `InputState`; the six page-one TIN/RDO/name fields now have
  candidate-only raw controls that start blank for profile-derived drafts;
- computed values, defaults, transport metadata, and preserved unknowns are
  not raw editor buffers and need explicit v2 derived/context/default
  projections rather than fabricated raw text;
- the three view vectors still expose exactly two rows, although adapter
  capture now resolves those controls through persisted stable group-instance
  IDs; insertion, deletion, and reviewed visible-order behavior remain
  unimplemented; and
- the four additional-item groups have neither raw buffers nor core storage.

The non-authorizing `Form2550QFieldValueSource` now reads all 106 v1-backed live
controls before lossy synchronization, distinguishes present blank from
absent, and exposes persisted stable group instances. It deliberately reports
the remaining collapsed-choice, computed/default, and additional-group
coverage gaps instead of fabricating values. Reading `Form2550QDraft` or
`to_bir_field_map()` alone still cannot meet `RawInputSnapshot`'s lossless
boundary.

## Focus-handle blockers

The current `Form2550QV2View` has a checked semantic-field-to-`InputState`
registry. Duplicate semantic targets make candidate validation unavailable
instead of selecting an arbitrary control.

- The 106 v1-backed `InputState` entities are mapped to semantic
  `FieldInstance`s, including repeated controls resolved through persisted
  stable row identities.
- Filing basis, quarter, classification, and treaty choices track the
  corresponding raw `InputState` focus handles on their visible clickable
  elements. Items 6-12 TIN segments, branch, RDO, taxpayer name, address, ZIP,
  contact number, and email also have raw text focus handles. Amended and
  short-period Yes/No controls now track four independent raw focus targets.
- Page-two identity duplicates and computed rows render without independent raw
  focus targets.
- Input parse errors use view-local keys such as `item_18` and
  `schedule_1[0].date`; core validation uses keys such as
  `item_18_paid_on_previous_return` and
  `schedule_1[0].purchase_or_import_amount`; v1 rules use serialized source
  keys. There is no checked bijection among those three namespaces.
- Repeating-row semantic focus uses stable `RepeatedGroupInstance` identity,
  but the fixed two-row UI still has no insertion/deletion behavior to
  exercise identity preservation interactively.
- First-blocking-issue focus selection exists, but no reviewed 2550Q provider
  can invoke it in production while the generated registry remains empty.

Consequently an adapter cannot truthfully advertise complete focus coverage
until any legally focusable profile/computed targets have reviewed semantic
bindings and a reviewed provider can produce the trusted validation report.

## Editable XML versus encrypted Final Copy

The two artifact shapes must not be collapsed:

| Artifact observation | Pseudo-div occurrences | Other relevant node |
| --- | ---: | --- |
| Reviewed plaintext finalized save | 160 unique pseudo-divs, including trailing `dateFiled` | Ends in `All Rights Reserved BIR 2012.0`; serialized `txtFinalFlag = "1"` is not the finality classifier |
| Decrypted encrypted Final Copy | 159 unique pseudo-divs; `dateFiled` is absent from this sequence | standalone `<dateFiled>...</dateFiled>` metadata element; `txtFinalFlag = "0"` |

`fields.json` does not preserve artifact order: its 160 concrete records are
lexicographically sorted and have no artifact-global ordinal. The value-free
plaintext audit now carries the exact observed 160-key order and a separate
ordered hash. Its first 159 occurrences exactly equal the encrypted audit's
ordered sequence; its sole suffix is pseudo-div `dateFiled`. The same
`saveXML` source loop governs editable and finalized plaintext saves, but only
the finalized `.0` marker is pinned by the reviewed plaintext sample. Both
artifact identities therefore remain documented-only until their complete
node and byte contracts are reviewed.

`serialization-binding-inventory-v796.json` turns those ordered audits into a
reproducible value-free projection plan. It binds every observed occurrence to
its static control or one of seven dynamic groups, partitions all 28 unbounded
families, and records plaintext-versus-encrypted body codecs separately.
Taxpayer name and address use legacy JavaScript `escape()` only in normal
plaintext Save; encrypted staging concatenates them raw. The inventory also
proves the current boundary: all 159 live serialized-control occurrences have
candidate identities (119 static plus 40 materialized repeated-family
occurrences). The executable surface contains 66 singleton fields and 28
repeated-family descriptors. Forty-four derived/alias controls and nine
workflow/credential/UI-state controls remain identity-only documented
bindings. Generated `dateFiled` is projected from the required immutable
`local-current-date` validation context, closing all 160 observed occurrence
value-source identities without adding an editable field. Its production clock
and timezone provider remains unresolved. The
group indices are package evidence, while app-owned rows use
`assigned-stable-id`. Executable serialization still needs a reviewed relation
between stable-instance order and official live DOM/display order.

The handwritten XML layer also collapses the distinction:

- `expected_xml_keys()` is a 160-key `BTreeSet`;
- `to_reviewed_field_map()` returns a `BTreeMap`, losing insertion/source
  order;
- `from_bir_xml_payload()` extracts a standalone `dateFiled`, when present,
  and inserts it into the same 160-key map;
- `to_bir_xml_payload()` emits the editable map shape and is not an encrypted
  Final Copy materializer.

The current round-trip tests prove the checked semantic map and the two
reviewed `txtFinalFlag` values. They do not prove ordered artifact parity,
artifact-specific presence, per-node codec selection, the Final Copy envelope,
or encryption materialization. `QUEUE_SUBMISSION_SUPPORTED` remains false and
`transition_to_queued()` fails closed.

## Combined Final Copy and Submit call graph

The source-pinned call graph and exact hashes are recorded in
`rules/forms/2550q-v2024/v2-candidate-final-copy-submit-workflow-review.md`.
The combined button is not a local-only finalization command:

1. `openAlertEmail()` checks save-file/amended/version state and asks whether
   the user wants to submit.
2. The loaded `checkNetConnection()` returns `true` immediately, so the
   source-present no-connection local-copy branch is unreachable in this
   package.
3. The reachable path opens enrollment/credential UI without writing an
   artifact.
4. `sendEmail()` later calls `saveEncryptedProfile(true)`, which reruns the
   three-rule Save preflight, writes the editable save, stages and externally
   encrypts the Final Copy artifact, and only then invokes external transport.

Validate success is insufficient for that later step: full Validate accepts
RDO `000`, while the fresh Save preflight rejects it. The candidate therefore
does not use one prior valid report as artifact or submission authorization.
The source also contains three pinned defects: the false connectivity branch
is unreachable, retry calls undefined `emailResend()` instead of loaded
`reSendEmail()`, and encryption status is ignored. A failed
`saveXML(true)` can additionally leave `sendEmail()` dereferencing
`undefined` before its transport `try` block.

No online submission was performed. All three v2 serialization artifacts
retain empty node lists, both new transitions remain non-executable, and the helper
hashes prove identity only—not encryption semantics, transport behavior, or
filing authorization.

## Calculation comparison

`calculations.json` contains 27 calculation records. The current
`recompute()`/total helpers have recognizable conceptual counterparts for 24,
but there is no calculation-ID binding and no exact-decimal equivalence proof.
The current code uses `f64` plus cent rounding, while the v1 observations
describe JavaScript parsing/`toFixed` behavior.

Three corpus calculations are not implemented as derived behavior:

| Calculation ID | Current code behavior |
| --- | --- |
| `2550q-schedule1-row` | Allowable input tax and balance are imported, stored, and edited manually; the model notes that a needed months-in-use input is not available |
| `2550q-items44-46` | Item 44B, 45B, and 46B are editable source amounts |
| `2550q-item47` | `otherSpecify47B` / Item 47B is an editable source amount |

The calculation graph is not reference-closed against `fields.json`:

- 15 input tokens are not exact v1 field IDs;
- 14 output tokens are not exact v1 field IDs;
- examples of unresolved aliases include
  `frm2550qv2024:totalPenalties` versus source key
  `frm2550qv2024:penalties`,
  `frm2550qv2024:outputVat` versus
  `frm2550qv2024:outputVatSales`,
  `frm2550qv2024:totalInputTax` versus
  `frm2550qv2024:total43`,
  `frm2550qv2024:otherSpecify47B` versus unprefixed
  `otherSpecify47B`, and
  `frm2550qv2024:totalAllowableInputTax` versus
  `frm2550qv2024:totalAllowInputTax`; and
- other graph nodes are prose tokens such as `Schedule 3 withholding total`,
  `Item 50 input-tax total`, or `Items 52 through 56`.

Those aliases may look obvious, but the v1 corpus does not bind them. A future
v2 snapshot must use exact reviewed IDs and must not infer these edges from
names.

## Validation comparison

The validation inventory contains 38 records:

| Dimension | Count |
| --- | ---: |
| `validate` phase | 25 |
| `save` phase | 3 |
| `blur/change` phase | 4 |
| `page navigation` phase | 6 |
| `verified-correct` | 30 |
| `incorrect-official-behavior` | 7 |
| `official-bug-compatible` | 1 |

There are 73 field references and 48 unique reference strings. The sole
non-field reference is the placeholder `dynamic-other-row-families`.

The handwritten validator has no v1 rule IDs, phase dispatch, profile branch,
or official first-error behavior. It returns an aggregate vector under its own
semantic keys. There are recognizable overlaps for identity, period,
classification, treaty detail, Items 19/42/47/56, schedule completeness, and
non-negative amounts, but exact agreement is not established because
conditions, order, messages, raw-parse visibility, and field IDs differ.

Concrete gaps/conflicts include the future-period rule, the Schedule 1 date
cutoff, the observed life-range rule, the Schedule 3 year observation, all
dynamic additional-row rules, and the official save preflight. The current
view's Save blocks malformed numeric/date buffers and an unresolved quarter,
then permits a local save with other validation errors; it does not implement
the three-rule official save sequence. No exact rule-mapping count greater than
zero is claimed.

## Workflow comparison

`workflow.json` records five phases, five transitions, four quarterly deadline
entries, and one conditional attachment observation. The full application
state machine is not implemented. The source-pinned v2 candidate implements
the official Validate edge (`edit` to `validated`) and Edit edge (`validated`
to `edit`) in its test-only provider:

- local Save persists the draft without transitioning to a distinct
  `saved-draft` state;
- ordinary validation during input synchronization does not itself create a
  `validated` state or disable controls;
- the explicit candidate transition is request-bound and notification-bearing,
  but no production GPUI view consumes it to apply `AllControlDisabled(true)`;
- the candidate Edit transition is also request-bound and notification-bearing,
  but no production GPUI state map yet reproduces `enableAllControl()`'s
  asymmetric field/button updates. In particular it must not be implemented as
  generic enable-all;
- preview is explicitly not Final Copy;
- no encrypted Final Copy transition/materializer exists; and
- app queue/submission and automatic payment advancement are explicitly
  rejected.

These are rollout blockers, not permission to reinterpret the workflow or
change current status behavior.

## Machine-checkable queries

The following is the count query used for the mutually exclusive 188-record
partition. It treats the direct-binding category as the exact residual after
the reviewed code-observation lists and the family predicate are removed.

```powershell
$f = (Get-Content -Raw 'rules/forms/2550q-v2024/fields.json' |
    ConvertFrom-Json).fields

$computed = @(
    'frm2550qv2024:excessInputTax',
    'frm2550qv2024:creditableVat',
    'frm2550qv2024:advVatPayment',
    'frm2550qv2024:totalTaxCredits',
    'frm2550qv2024:excessCredits',
    'frm2550qv2024:penalties',
    'frm2550qv2024:totalPayable',
    'frm2550qv2024:outputVatSales',
    'frm2550qv2024:totalSales',
    'frm2550qv2024:outputTaxDue',
    'frm2550qv2024:totalAdjOutput',
    'frm2550qv2024:inputTaxDeferred',
    'frm2550qv2024:total43',
    'frm2550qv2024:totalCurPurchase',
    'frm2550qv2024:totalCurInputTax',
    'frm2550qv2024:totalAvailInputTax',
    'frm2550qv2024:importCapitalInputTax',
    'frm2550qv2024:inputTaxAttr',
    'frm2550qv2024:totalDeductions',
    'frm2550qv2024:adjDeductions',
    'frm2550qv2024:totalAllowInputTax',
    'frm2550qv2024:netVatPayable',
    'sched1TotalBalPrev',
    'sched1TotalBalNext',
    'frm2550qv2024:sched2TotalSales',
    'frm2550qv2024:sched2TotalRatable',
    'frm2550qv2024:sched2TotalAttr',
    'sched3TotalIncome',
    'sched3TotalTax',
    'sched4AmountPaid',
    'txtTotalAmountOfBalanceofInputTaxFromPrevious',
    'txtTotalAmountOfBalanceofInputTaxToBeCarried',
    'txtTotalAmountofIncomePayment',
    'txtTotalAmoungOfTaxWithHeld',
    'txtAmountPaidSched4'
)

$serializedContextDefault = @(
    'frm2550qv2024:txtCurrentPage',
    'frm2550qv2024:txtMaxPage',
    'resultOtherCreditsNo19',
    'resultOtherCreditsNo42',
    'resultOtherCreditsNo47',
    'resultOtherCreditsNo56',
    'txtFinalFlag',
    'txtEnroll',
    'ebirOnlineConfirmUsername',
    'ebirOnlineUsername',
    'ebirOnlineSecret',
    'txtEmail',
    'driveSelectTPExport',
    'dateFiled'
)

$uiRawMismatch = @(
    'frm2550qv2024:addOutputVat',
    'frm2550qv2024:lessOutputVat',
    'txtAllowedInputTax10',
    'txtAllowedInputTax11',
    'txtBalanceInputTax10',
    'txtBalanceInputTax11',
    'txtTotalTaxWithHeld30',
    'txtTotalTaxWithHeld31'
)

$family = @($f |
    Where-Object control_kind -eq 'runtime-indexed-family' |
    ForEach-Object field_key)

$direct = @($f | Where-Object {
    $_.field_key -notin $computed -and
    $_.field_key -notin $serializedContextDefault -and
    $_.field_key -notin $uiRawMismatch -and
    $_.field_key -notin $family
} | ForEach-Object field_key)

$listed = @($direct + $serializedContextDefault + $computed +
    $family + $uiRawMismatch)

[pscustomobject]@{
    direct = $direct.Count
    serialized_context_default = $serializedContextDefault.Count
    computed = $computed.Count
    family_descriptors = $family.Count
    ui_raw_mismatch = $uiRawMismatch.Count
    residual_unmapped = @($f.field_key | Where-Object { $_ -notin $listed }).Count
    duplicate_classification = $listed.Count -
        @($listed | Sort-Object -Unique).Count
    total = $listed.Count
}
```

Expected output:

```text
direct                    103
serialized_context_default 14
computed                   35
family_descriptors         28
ui_raw_mismatch             8
residual_unmapped           0
duplicate_classification    0
total                     188
```

Additional queries used:

```powershell
# Concrete-key and occurrence invariants.
$concrete = @($f | Where-Object { $null -ne $_.serialized_key })
$concrete.Count
@($concrete.serialized_key | Sort-Object -Unique).Count
@($concrete.serialized_occurrence | Sort-Object -Unique)

# The fields file is lexical inventory order, not artifact order.
$keys = @($concrete | ForEach-Object field_key)
(Compare-Object $keys @($keys | Sort-Object) -SyncWindow 0).Count

# Exact unresolved calculation references.
$fieldIds = $f.field_key
$calculations = (Get-Content -Raw
    'rules/forms/2550q-v2024/calculations.json' | ConvertFrom-Json).calculations
@($calculations | ForEach-Object inputs |
    Where-Object { $_ -notin $fieldIds }).Count
@($calculations | ForEach-Object outputs |
    Where-Object { $_ -notin $fieldIds }).Count

# Validation phase/assessment counts and unresolved field references.
$rules = (Get-Content -Raw
    'rules/forms/2550q-v2024/validations.json' | ConvertFrom-Json).rules
$rules | Group-Object phase
$rules | Group-Object assessment
$rules | ForEach-Object fields |
    Where-Object { $_ -notin $fieldIds } | Sort-Object -Unique

# Encrypted pseudo-div inventory; dateFiled is a separate metadata element.
$encrypted = (Get-Content -Raw
    'rules/forms/2550q-v2024/fixtures/encrypted-field-audit-v796.json' |
    ConvertFrom-Json).keys
$encrypted.Count
@($encrypted | Sort-Object -Unique).Count
'dateFiled' -in $encrypted
```

Repository inspection used `rtk read`, `rtk rg`, and `rtk json`; PowerShell JSON
queries and SHA-256 reads were invoked through `rtk proxy
C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe`.
