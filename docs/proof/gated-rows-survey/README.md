6 forms with high-confidence dormant typing, 18 forms with probe-discarded typing.

# Gated rows survey — main `642bfc78221c0a87b318c7c3378c3f5b0b2c9359`

Survey date: 2026-11-26. This is a reading survey, not a fix. Product code was not changed.

## Method and caution

I read the host field-map/write logic under `crates/bir-core/src/forms/`, then ran a debug-only host probe that filled editable schema keys with invented values on fresh drafts, read the host answers, and counted whether the exact typed marker survived in `form_drafts.data_json` after recompute. The probe is stronger than text search but it bypasses browser hit-testing, so rows in hidden popups can be false positives; those are marked as such below. The three most serious cases were then confirmed by a focused host-level run using `screen_form_open`/`screen_form_fill` and draft JSON. All runs used the safety variables and invented values only.

## Pattern table

| form | schedule or item | keys | condition | host file:lines | kept or discarded | page shuts it? | confirmed by a run? |
|---|---|---|---|---|---|---|---|
| 1601FQ | Schedule 1 treaty rows | `drpTreatyCode0-4`, `txtAmtIncomePay0-4`, `txtRate0-4` | item 13 tax-relief answer is unanswered | `form_1601fq.rs:1136-1167` | kept dormant before item 13 is answered; the first answer then resets Schedule 1 | no — no `off` in probe | yes, focused host run |
| 2551Q | item 16 previous payment | `frm2551Qv2018:txt16` | amended return is unanswered or No | `form_2551q_xml.rs:171-178` | kept dormant | no — no `off` in probe | yes, focused host run; returned at amended Yes |
| 1606 | installment-sale items 21–25 | `txtSelling`, `txtCost`, `txtMortgage`, `txtTotalP`, `txtAmount` | transaction type is unanswered or not installment sale | `form_1606.rs:750-768` | kept dormant | no — no `off` in probe | yes, focused host run; returned at installment sale |
| 1701 | Schedules 1A/2A spouse text, Schedule 3 item 27 description | `txtPg2IShed1a_1SName`, `txtPg2IShed2a_2SName`, `txtPg3IShed3_27Desc` | spouse/graduated/8% schedule state is closed | `form_1701_official.rs:1649-1691` | kept dormant | no `off` in probe; visibility not rechecked in a real window | no |
| 1706 | property-class “Others” details | `frm1706:j_id391:_7` | property class is unanswered or not Others | `form_1706.rs:800-805` | kept dormant | no `off` in probe | no |
| 1707 | transaction “Others” description | `txtPg1I9TransDescSpec` | transaction type is unanswered or not Others | `form_1707.rs:581-586` | kept dormant | no `off` in probe | no |
| 0619E | item 15 previous remittance | `frm0619E:txtTax15` | amended return unanswered or No | `form_0619e_official.rs:261-282,339` | discarded | not checked | no |
| 0619F | item 16 previous remittance | `frm0619F:txtTax16` | amended return unanswered or No | `form_0619f_official.rs:280-303,364` | discarded | not checked | no |
| 1600PT | item 19 previous payment; item 12 relief specification | `txtPg1P2I19TaxRemittedInRtrn`, `txtPg1Pt1I12TaxReliefSpec` | amended No/unanswered; tax relief No/unanswered | `form_1600pt.rs:603-608` | discarded | not checked | no |
| 1600VT | item 19 previous payment; item 12 relief specification | `txtPg1P2I19TaxRemittedInRtrn`, `txtPg1Pt1I12TaxReliefSpec` | amended No/unanswered; tax relief No/unanswered | `form_1600vt.rs:524-529` | discarded | not checked | no |
| 1600WP | item 13 previous payment | `frm1600WP:txtTax13` | amended return unanswered or No | `form_1600wp.rs:358-372` | discarded | not checked | no |
| 1602Q | item 20 previous payment | `frm1602Q:txt20` | amended return unanswered or No | `form_1602q.rs:433-439` | discarded | not checked | no |
| 1603Q | item 15 previous payment | `frm1603Q:txtTax15` | amended return unanswered or No | `form_1603q.rs:255-270` | discarded | not checked | no |
| 1606 | item 29 previous payment | `frm1606:txtLess` | amended return unanswered or No | `form_1606.rs:563,833,1284-1288` | discarded | not checked | no |
| 1701 | foreign tax number; Schedule 5 rows | `txtPg1I14ForeignTaxNumber`; `txtPg3IShed5_{1,2,4,5}{Amt,Desc,Legal}` | foreign-tax credit answer or schedule applicability is closed | `form_1701_official.rs:1369,1724-1754` | discarded | not checked; many are gated by existing 1701 rules but these keys were accepted by the probe | no |
| 1701A | spouse/schedule and deduction boxes | `txt23B`, `txt36A/B`, `txt37A/B`, `txt41A/B/Desc`, `txt42B/Desc`, `txt43A/B`, `txt47A/B`, `txt48A/B`, `txt50A/B/Desc`, `txt51B/Desc`, `txt54A/B`, `txt57B-63B`, `txtForeignTaxNumber` | joint-return, tax-rate/deduction-method, foreign-tax or amended gates closed | `form_1701a.rs:612-634,1134-1140` | discarded | not checked | no |
| 1701MS | column-A business/deduction/previous-payment boxes | `txtSalesRevenueNo5a`, `txtSalesReturnsNo6a`, `txtCostofSalesNo8a`, `txtGrossa`, `txtAllowableItemizedDeductionsNo10a`, `txtTaxpayerNo13a-15a`, `txtSalesRevenuesFeesANo19a`, `txtTaxPaidPreviouslyFiledNo6a` | business-income, graduated/itemized, relief or amended gates closed | `form_1701ms.rs:559-582,695` | discarded | not checked | no |
| 1702MX | Schedule 3 item 27 and Schedule 9 rows | `txtPg2Sc3It27A-C`; `txtPg7Sc9I1-3` and `CA-CF` | relevant rate/schedule applicability closed | `form_1702mx_xml.rs:182-184` (key serialization); model rows around `form_1702mx.rs:350` | discarded | not checked; row visibility needs review | no |
| 1702RT | prior tax credits and Schedule 4 rows | `txtPg2Pt4I45IncomeTaxPaymentUnderMCIT`, `txtPg2Pt4I51TaxPaidInReturn`; `txtPg4Sc4I1-3C1-C7` | MCIT/amended/schedule applicability closed | `form_1702rt_xml.rs:432-468` (key serialization) | discarded | not checked; row visibility needs review | no |
| 1706 | item 29 previous payment | `frm1706:txtLess` | amended return unanswered or No | `form_1706.rs` analogous previous-payment logic | discarded | not checked | no |
| 1707 | Schedule 1 installment rows | `txtPg2P4Sch1I1SellPrice` through `I5InstallmentAmt` | transaction type unanswered or not installment sale | `form_1707.rs:638` and installment model lines `115-377` | discarded | not checked | no |
| 2000 | other-party name; balance carried over | `txtOtherName`, `txtTax15A` | “other party” or affixture-mode state closed | `form_2000.rs:929-946` | discarded | not checked | no |
| 2550M | item 23D previous payment | `frm2550m:txtTax23D` | amended return unanswered or No | `form_2550m.rs:699-701` (`recompute` clears it) | discarded | not checked | no |
| 2552 | tax-relief specification | `txtPg1Pt1I10TaxReliefSpec` | tax-relief answer unanswered or No | `form_2552.rs:910-918` | discarded | not checked | no |

### 2550M probe false positives

The raw probe also marked many `frm2550m:txtAtcCde2-36` keys as dormant and `txtTaxableSaleSched4`/`txtTotSaleSched5` as dormant. I do **not** count them in the high-confidence list: Schedule 1 rows are re-sorted and re-indexed after a select write, so a write to row N can legitimately appear at row 1; and the Schedule 4/5 keys live in modals whose model object is normally created when the modal is opened. Browser-level visibility was not proven for either group.

## Focused confirmations

### 1601FQ Schedule 1 — confirmed, with one important difference from the original report

On a fresh unanswered draft, typing `txtAmtIncomePay0=1000` and `txtRate0=42` was accepted. The host still answered both as `0.00`, while the draft held `schedule1[0].income_payment=1000`, `tax_rate=42`, and `tax_withheld=420`. No `off` answer was present for either key.

After answering item 13 Yes, the typing did **not** come back in this host-level run. `set_tax_relief` (`form_1601fq.rs`) resets `schedule1` to five default rows when the answer first becomes `Some(_)`; the resulting rows were `NA/0`. Thus the dormant-storage and wrong-zero output are confirmed, but “it comes back when the question is answered” is not true on this exact path. This makes the defect worse in one way (typing is silently lost on the first answer) but does not remove the wrong-amount route before the answer.

### 2551Q item 16 — confirmed dormant money

On a fresh original/unamended draft, `txt16=42` was accepted; host shown value was `0.00`, draft `tax_paid_previous=42`, and there was no `off`. After amended Yes, the same draft still held `42` and the host showed `42.00`. This is a clear money amount that can return.

### 1606 installment items — confirmed dormant money

On a fresh draft with the transaction type unanswered, `txtSelling=42` and `txtCost=7` were accepted; the host showed blanks, the draft held `gross_selling_price=42` and `cost_and_expenses=7`, and there was no `off`. After choosing installment sale, both values returned and were shown as `42.00` and `7.00`.

## Why the page did not shut 1601FQ Schedule 1

`forms/conditions.json` has a 1601FQ condition only for `frm1601FQ:txtTax25` and none for the Schedule 1 row keys, and the host sends no matching `off`/lock state for `txtAmtIncomePay*` or `txtRate*`. In `nsh/screens/form/session.ts`, `isLocked`/`isShut` therefore treat those boxes as editable; the fixed `-`/`0.00` written by `form_1601fq.rs` is only an output answer, not a page gate.

## Official behavior evidence

The reconstructed frozen page is at `html-frozen/1601-fq-2020/index.html`; it is a rendered reconstruction and does not contain the original `disableSelTreaty` JavaScript source. The repository's extracted official-runtime evidence is in `rules/forms/1601fq-v2018/` and cites the original HTA line ranges. I did not find the original HTA text file itself in this checkout.

Quoted evidence:

- `rules/forms/1601fq-v2018/fixtures/runtime-control-inventory-v796.json` records the Schedule 1 controls initially disabled, including `drpTreatyCode0` at source line 1185 (`"disabled": true`), `drpATCCode0` at line 1191 (`"disabled": true`), `txtNatIncPayment0` line 1196 (`"value": "-", "disabled": true`), `txtAmtIncomePay0` line 1197 (`"value": "0.00", "disabled": true`), `txtRate0` line 1198 (`"value": "0.00", "disabled": true`), and `txtReqWithheld0` line 1199 (`"value": "0.00", "disabled": true`).
- `rules/forms/1601fq-v2018/validations.json`, rule `1601fq-change-003`, cites `official-hta-runtime#disableSelTreaty:L3573-L3591` and states: “Treaty selection and Schedule 1 are cleared and disabled.”
- The same file, rule `1601fq-change-004`, cites `official-hta-runtime#showAddSchedI:L3696-L3730` and gives the exact message: “You are not availing of tax relief under Special Law or International Tax Treaty. There is no need to\nfill up schedule 1.”
- For 2551Q, `rules/forms/2551q-v2018/fixtures/runtime-control-inventory-v796.json` records `frm2551Qv2018:txt16` at source line 1125 as `"disabled": true` and `"value": "0.00"`; rule `2551q-amended-toggle` says the official behavior is “Disables and zeros Item 16; amended Yes enables it unless the 8% option is active,” citing `official-hta-runtime#updateAmended:L4881-L4901`.
- For 1606, `rules/forms/1606-v2018/fixtures/runtime-control-inventory-v796.json` records `txtSelling`, `txtCost`, `txtMortgage`, `txtTotalP`, and `txtAmount` at source lines 1124, 1127, 1130, 1133, and 1136, all initially `"disabled": true`; the validation-function inventory records official `enableInstallment` at lines 3230–3266 controlling those keys.

## What I would change for 1601FQ (not changed here)

Give the Schedule 1 row controls the same closed-state gate as the official form: while item 13 is unanswered or No, the page should shut/lock the row boxes rather than accepting typing into controls whose answers are fixed zeros. Separately, decide explicitly whether the first Yes should preserve pre-answer typing or clear it with an explicit user-facing explanation; the current silent reset is surprising either way.
