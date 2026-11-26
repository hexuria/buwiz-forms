# Money-box third-decimal audit

Report only; no behavior is changed. Probe values: `1000.005`, `1.005`, `2.675`.

## Official blur and computation inventory

| Form | Money boxes (input/total) | Blur rule | Official blur behavior quoted by the rules | Blur rounding | Computation rounding |
|---|---:|---|---|---|---|
| 0605-v2003 | 6/8 | `0605-input-001-amount-format` | The source silently replaces the value with 0.00; otherwise it applies parseFloat(...).toFixed(2). | parseFloat/toFixed(2) | formatCurrency/round |
| 0619e-v2018 | 9/12 | `none recorded` | normalization: NumWithComma; formatCurrency | formatCurrency/round | formatCurrency/round |
| 0619f-v2018 | 10/14 | `none recorded` | normalization: NumWithComma; round(...,2); formatCurrency | formatCurrency/round | formatCurrency/round |
| 1600pt-v2018 | 77/119 | `none recorded` | normalization: legacy currency formatting | unresolved | mixed: parseFloat/toFixed(2) + formatCurrency/round |
| 1600vt-v2018 | 41/65 | `none recorded` | normalization: legacy currency formatting | unresolved | mixed: parseFloat/toFixed(2) + formatCurrency/round |
| 1600wp-v2010 | 25/42 | `none recorded` | normalization: NumWithComma; formatCurrency; round(...,2) | formatCurrency/round | formatCurrency/round |
| 1601c-v2018 | 23/36 | `none recorded` | normalization: NumWithComma; round(...,2); formatCurrency; toFixed(2) | mixed: formatCurrency/round + parseFloat/toFixed(2) | parseFloat/toFixed(2) |
| 1601eq-v2018 | 122/239 | `1601eq-blur-002` | round() truncates integer input to 12 digits, rounds half-up to two decimals, comma-groups, and coerces NaN or excess precision to 0.00. | formatCurrency/round | mixed: unresolved + formatCurrency/round |
| 1601fq-v2018 | 96/157 | `1601fq-input-003` | Nonnumeric input is silently cleared; numeric input is rounded with parseFloat().toFixed(2). | parseFloat/toFixed(2) | formatCurrency/round |
| 1602q-v2018 | 34/64 | `none recorded` | normalization: NumWithComma; formatCurrency; round(...,2) | formatCurrency/round | formatCurrency/round |
| 1603q-v2018 | 13/23 | `none recorded` | normalization: legacy currency formatting | unresolved | formatCurrency/round |
| 1604c-v2018 | 37/53 | `none recorded` | normalization: NumWithComma; round(this,2); formatCurrency | formatCurrency/round | formatCurrency/round |
| 1604e-v2018 | 50/54 | `none recorded` | normalization: NumWithComma; round(this,2); formatCurrency | formatCurrency/round | formatCurrency/round |
| 1604f-v2018 | 24/45 | `none recorded` | normalization: parseFloat; toFixed(2); NumWithComma; formatCurrency | mixed: parseFloat/toFixed(2) + formatCurrency/round | parseFloat/toFixed(2) |
| 1606-v2018 | 15/22 | `none recorded` | normalization: NumWithComma; formatCurrency; round(...,2) | formatCurrency/round | formatCurrency/round |
| 1700-v2013 | 96/147 | `none recorded` | normalization: NumWithComma; parseInt; formatCurrencyWOC; NegativeValue | whole-peso | whole-peso |
| 1701-v2018 | 23/30 | `none recorded` | normalization: NumWithComma parsing; formatCurrency formatting where invoked | formatCurrency/round | formatCurrency/round |
| 1701a-v2018 | 59/98 | `none recorded` | normalization: NumWithComma; toFixed(0); formatCurrency; whole-number key filter | mixed: whole-peso + formatCurrency/round | whole-peso |
| 1701ms-v2024 | 73/124 | `none recorded` | normalization: NumWithComma; Math.round where invoked; formatCurrencyWithComma | formatCurrency/round | whole-peso |
| 1701q-v2018 | 44/80 | `none recorded` | normalization: round/format to two decimals on blur | formatCurrency/round | mixed: whole-peso + parseFloat/toFixed(2) |
| 1702ex-v2018c | 50/99 | `none recorded` | normalization: NumWithComma; formatCurrency; toFixed(0) | mixed: formatCurrency/round + whole-peso | whole-peso |
| 1702mx-v2018c | 253/525 | `none recorded` | normalization: NumWithComma; NumWithParenthesis; NegativeValue; formatCurrencyWOC | whole-peso | whole-peso |
| 1702q-v2018c | 32/62 | `1702q-input-001` | Nonnumeric and negative values are silently replaced with 0.00; numeric values are rounded with binary parseFloat().toFixed(2). | parseFloat/toFixed(2) | mixed: whole-peso + parseFloat/toFixed(2) + unresolved |
| 1702rt-v2018c | 110/119 | `none recorded` | normalization: NumWithComma/NumWithParenthesis; whole-peso display; whole-number key filter; currency formatting | whole-peso | unresolved |
| 1706-v2018 | 16/22 | `none recorded` | normalization: NumWithComma; formatCurrency; round(...,2) | formatCurrency/round | formatCurrency/round |
| 1707-v2021 | 19/33 | `none recorded` | normalization: NumWithComma; formatCurrency; toFixed(2) | mixed: formatCurrency/round + parseFloat/toFixed(2) | parseFloat/toFixed(2) |
| 1707a-v2021 | 38/62 | `none recorded` | normalization: valForCompute; preciseCompute; roundAmount | preciseCompute/roundAmount | preciseCompute/roundAmount |
| 1800-v2018 | 42/55 | `none recorded` | normalization: NumWithComma; formatCurrency; round(...,2) | formatCurrency/round | formatCurrency/round |
| 1801-v2018 | 45/95 | `none recorded` | normalization: NumWithComma; formatCurrency; round(...,2); blockNegativeNumber | formatCurrency/round | parseFloat/toFixed(2) |
| 2000-v2018 | 27/40 | `none recorded` | normalization: NumWithComma; formatCurrency; round(...,2) | formatCurrency/round | parseFloat/toFixed(2) |
| 2000ot-v2018 | 28/44 | `none recorded` | normalization: NumWithComma; formatCurrency; round(...,2) | formatCurrency/round | parseFloat/toFixed(2) |
| 2200a-v2020 | 141/214 | `none recorded` | normalization: NumWithComma; amtFormat; round(2) | unresolved | parseFloat/toFixed(2) |
| 2200an-v2018 | 146/157 | `none recorded` | normalization: NumWithComma; formatCurrency; round(2) | formatCurrency/round | formatCurrency/round |
| 2200c-v2018 | 88/125 | `2200c-defect-028-whole-peso-rounding` | Every formula calls toFixed(0), discarding centavos before currency formatting. | whole-peso | whole-peso |
| 2200m-v2018 | 155/163 | `none recorded` | normalization: NumWithComma; formatCurrency; round(2) | formatCurrency/round | parseFloat/toFixed(2) |
| 2200p-v2020 | 322/330 | `none recorded` | normalization: NumWithComma; formatCurrency; round(2) | formatCurrency/round | parseFloat/toFixed(2) |
| 2200s-v2018 | 43/65 | `none recorded` | normalization: NumWithComma; roundDownComputation; formatCurrency | mixed: round-down + formatCurrency/round | round-down |
| 2200t-v2020 | 208/218 | `none recorded` | normalization: round(2); blockNegativeNumber; amtFormat | unresolved | parseFloat/toFixed(2) |
| 2550m-v2007 | 76/162 | `2550m-input-001` | parseFloat/toFixed(2) normalization. | parseFloat/toFixed(2) | formatCurrency/round |
| 2550q-v2024 | 64/89 | `none recorded` | normalization: NumWithComma; formatCurrency | formatCurrency/round | parseFloat/toFixed(2) |
| 2551q-v2018 | 16/34 | `2551q-input-money` | parseFloat/toFixed(2); NaN becomes 0.00. | parseFloat/toFixed(2) | formatCurrency/round |
| 2552-v2018 | 57/98 | `none recorded` | normalization: NumWithComma; formatCurrency; toFixed(0) | mixed: formatCurrency/round + whole-peso | whole-peso |
| 2553-v1999 | 6/16 | `none recorded` | normalization: NumWithComma; formatCurrency; round(2) | formatCurrency/round | formatCurrency/round |

## Probe table

| Form | Box | Kind | Official rule | Probe | Ours | Stored | Upload | Match |
|---|---|---|---|---:|---|---:|---:|---|
| 1601C | `frm1601c:txtTax14` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | unclear-official |
| 1601C | `frm1601c:txtTax14` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | unclear-official |
| 1601C | `frm1601c:txtTax14` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | unclear-official |
| 1601C | `frm1601c:txtTax32` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | unclear-official |
| 1601C | `frm1601c:txtTax32` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.00 | unclear-official |
| 1601C | `frm1601c:txtTax32` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | unclear-official |
| 1601C | `frm1601c:sched1:txtShouldTaxDue0` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | unclear-official |
| 1601C | `frm1601c:sched1:txtShouldTaxDue0` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | unclear-official |
| 1601C | `frm1601c:sched1:txtShouldTaxDue0` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | unclear-official |
| 2551Q | `frm2551Qv2018:txt15` | ordinary-item | `2551q-input-money` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 2551Q | `frm2551Qv2018:txt15` | ordinary-item | `2551q-input-money` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 2551Q | `frm2551Qv2018:txt15` | ordinary-item | `2551q-input-money` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 2551Q | `frm2551Qv2018:txt20` | penalty-item | `2551q-input-money` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 2551Q | `frm2551Qv2018:txt20` | penalty-item | `2551q-input-money` | 1.005 | kept-3dp | 1.005 | 1.00 | mismatch-kept-3dp |
| 2551Q | `frm2551Qv2018:txt20` | penalty-item | `2551q-input-money` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 2551Q | `txtATCAmt1` | schedule-row | `2551q-input-money` | 1000.005 | refused: Exceeds the official 12-character input limit (at most 999,999,999,999 without centavos, or 999,999,999.99 with centavos); enter a smaller amount. | 0 | 0.00 | mismatch-wrong-refusal |
| 2551Q | `txtATCAmt1` | schedule-row | `2551q-input-money` | 1.005 | refused: Exceeds the official 12-character input limit (at most 999,999,999,999 without centavos, or 999,999,999.99 with centavos); enter a smaller amount. | 0 | 0.00 | mismatch-wrong-refusal |
| 2551Q | `txtATCAmt1` | schedule-row | `2551q-input-money` | 2.675 | refused: Exceeds the official 12-character input limit (at most 999,999,999,999 without centavos, or 999,999,999.99 with centavos); enter a smaller amount. | 0 | 0.00 | mismatch-wrong-refusal |
| 2553 | `frm2553:txt20B` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 2553 | `frm2553:txt20B` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 2553 | `frm2553:txt20B` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 2553 | `frm2553:txt22A` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 2553 | `frm2553:txt22A` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 2553 | `frm2553:txt22A` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 2553 | `frm2553:txt14C` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 2553 | `frm2553:txt14C` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 2553 | `frm2553:txt14C` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 1604F | `frm1604f:txtSched1Pen1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 1604F | `frm1604f:txtSched1Pen1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 1604F | `frm1604f:txtSched1Pen1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 1604C | `frm1604c:txt12` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | refused: Needs item 11 = Yes | 0 | 0.00 | inconclusive-gated |
| 1604C | `frm1604c:txt12` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | refused: Needs item 11 = Yes | 0 | 0.00 | inconclusive-gated |
| 1604C | `frm1604c:txt12` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | refused: Needs item 11 = Yes | 0 | 0.00 | inconclusive-gated |
| 1604C | `frm1604c:txtSched1Adj1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 1604C | `frm1604c:txtSched1Adj1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 1604C | `frm1604c:txtSched1Adj1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 1604E | `frm1604e:txtSched1Penalties1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 1604E | `frm1604e:txtSched1Penalties1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 1604E | `frm1604e:txtSched1Penalties1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 1606 | `frm1606:txtAmount` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 |  | match |
| 1606 | `frm1606:txtAmount` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 |  | match |
| 1606 | `frm1606:txtAmount` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 |  | match |
| 1606 | `frm1606:txtCompromise` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 1606 | `frm1606:txtCompromise` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 1606 | `frm1606:txtCompromise` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 1600VT | `frm1600VT:txtPg1P2I19TaxRemittedInRtrn` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1600VT | `frm1600VT:txtPg1P2I19TaxRemittedInRtrn` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1600VT | `frm1600VT:txtPg1P2I19TaxRemittedInRtrn` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1600VT | `frm1600VT:txtPg1P2I23Surcharge` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | unclear-official |
| 1600VT | `frm1600VT:txtPg1P2I23Surcharge` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | unclear-official |
| 1600VT | `frm1600VT:txtPg1P2I23Surcharge` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | unclear-official |
| 1600VT | `frm1600VT:Pg2Sc1AmtOfIncPmt1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 1600VT | `frm1600VT:Pg2Sc1AmtOfIncPmt1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 1600VT | `frm1600VT:Pg2Sc1AmtOfIncPmt1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 1600PT | `frm1600PT:txtPg1P2I19TaxRemittedInRtrn` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1600PT | `frm1600PT:txtPg1P2I19TaxRemittedInRtrn` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1600PT | `frm1600PT:txtPg1P2I19TaxRemittedInRtrn` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1600PT | `frm1600PT:txtPg1P2I23Surcharge` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | unclear-official |
| 1600PT | `frm1600PT:txtPg1P2I23Surcharge` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | unclear-official |
| 1600PT | `frm1600PT:txtPg1P2I23Surcharge` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | unclear-official |
| 1600PT | `frm1600PT:Pg2Sc1AmtOfIncPmt1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 1600PT | `frm1600PT:Pg2Sc1AmtOfIncPmt1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 1600PT | `frm1600PT:Pg2Sc1AmtOfIncPmt1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 1600WP | `frm1600WP:txtTax13` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1600WP | `frm1600WP:txtTax13` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1600WP | `frm1600WP:txtTax13` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1600WP | `frm1600WP:txtTax15A` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 1600WP | `frm1600WP:txtTax15A` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 1600WP | `frm1600WP:txtTax15A` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 1600WP | `frm1600WP:txtTaxBase1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 1600WP | `frm1600WP:txtTaxBase1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 1600WP | `frm1600WP:txtTaxBase1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 1601EQ | `frm1601EQ:txtTax20` | ordinary-item | `1601eq-blur-002` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 1601EQ | `frm1601EQ:txtTax20` | ordinary-item | `1601eq-blur-002` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 1601EQ | `frm1601EQ:txtTax20` | ordinary-item | `1601eq-blur-002` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 1601EQ | `frm1601EQ:txtTax26` | penalty-item | `1601eq-blur-002` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 1601EQ | `frm1601EQ:txtTax26` | penalty-item | `1601eq-blur-002` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 1601EQ | `frm1601EQ:txtTax26` | penalty-item | `1601eq-blur-002` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 1601EQ | `frm1601EQ:txtTaxBase9` | schedule-row | `1601eq-blur-002` | 1000.005 | refused: Choose this row's ATC first. | — | — | inconclusive-gated |
| 1601EQ | `frm1601EQ:txtTaxBase9` | schedule-row | `1601eq-blur-002` | 1.005 | refused: Choose this row's ATC first. | — | — | inconclusive-gated |
| 1601EQ | `frm1601EQ:txtTaxBase9` | schedule-row | `1601eq-blur-002` | 2.675 | refused: Choose this row's ATC first. | — | — | inconclusive-gated |
| 1601FQ | `frm1601FQ:txtTax23` | ordinary-item | `1601fq-input-003` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | mismatch-different-rounding |
| 1601FQ | `frm1601FQ:txtTax23` | ordinary-item | `1601fq-input-003` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | mismatch-different-rounding |
| 1601FQ | `frm1601FQ:txtTax23` | ordinary-item | `1601fq-input-003` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | mismatch-different-rounding |
| 1601FQ | `frm1601FQ:txtTax28` | penalty-item | `1601fq-input-003` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 1601FQ | `frm1601FQ:txtTax28` | penalty-item | `1601fq-input-003` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 1601FQ | `frm1601FQ:txtTax28` | penalty-item | `1601fq-input-003` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 1601FQ | `txtAmtIncomePay0` | schedule-row | `1601fq-input-003` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 0.00 | mismatch-different-rounding |
| 1601FQ | `txtAmtIncomePay0` | schedule-row | `1601fq-input-003` | 1.005 | rounded-formatCurrency/round | 1.01 | 0.00 | mismatch-different-rounding |
| 1601FQ | `txtAmtIncomePay0` | schedule-row | `1601fq-input-003` | 2.675 | rounded-formatCurrency/round | 2.68 | 0.00 | mismatch-different-rounding |
| 1603Q | `frm1603Q:txtTax15` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1603Q | `frm1603Q:txtTax15` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1603Q | `frm1603Q:txtTax15` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1603Q | `frm1603Q:txtTax19` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | unclear-official |
| 1603Q | `frm1603Q:txtTax19` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | unclear-official |
| 1603Q | `frm1603Q:txtTax19` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | unclear-official |
| 1603Q | `frm1603Q:Sched1:txtValue1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 1603Q | `frm1603Q:Sched1:txtValue1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 1603Q | `frm1603Q:Sched1:txtValue1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 1602Q | `frm1602Q:txt18` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 1602Q | `frm1602Q:txt18` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 1602Q | `frm1602Q:txt18` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 1602Q | `frm1602Q:txt24` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 1602Q | `frm1602Q:txt24` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 1602Q | `frm1602Q:txt24` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 1602Q | `frm1602Q:txtSched1Amount1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 1602Q | `frm1602Q:txtSched1Amount1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 1602Q | `frm1602Q:txtSched1Amount1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 2552 | `frm2552:txtPg1P2I18CredTaxWthhld` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 2552 | `frm2552:txtPg1P2I18CredTaxWthhld` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 2552 | `frm2552:txtPg1P2I18CredTaxWthhld` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 2552 | `frm2552:txtPg1P2I21Surcharge` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | unclear-official |
| 2552 | `frm2552:txtPg1P2I21Surcharge` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | unclear-official |
| 2552 | `frm2552:txtPg1P2I21Surcharge` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | unclear-official |
| 2552 | `frm2552:txtPg2P4I1Amount` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 2552 | `frm2552:txtPg2P4I1Amount` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 2552 | `frm2552:txtPg2P4I1Amount` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 2550M | `frm2550m:txtTax13A` | ordinary-item | `2550m-input-001` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | mismatch-different-rounding |
| 2550M | `frm2550m:txtTax13A` | ordinary-item | `2550m-input-001` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | mismatch-different-rounding |
| 2550M | `frm2550m:txtTax13A` | ordinary-item | `2550m-input-001` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | mismatch-different-rounding |
| 2550M | `frm2550m:txtTax25A` | penalty-item | `2550m-input-001` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 2550M | `frm2550m:txtTax25A` | penalty-item | `2550m-input-001` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 2550M | `frm2550m:txtTax25A` | penalty-item | `2550m-input-001` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 2550M | `txtAmountPaidSch70` | schedule-row | `2550m-input-001` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | mismatch-different-rounding |
| 2550M | `txtAmountPaidSch70` | schedule-row | `2550m-input-001` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | mismatch-different-rounding |
| 2550M | `txtAmountPaidSch70` | schedule-row | `2550m-input-001` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | mismatch-different-rounding |
| 1706 | `frm1706:txtAmount` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 1706 | `frm1706:txtAmount` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 1706 | `frm1706:txtAmount` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 1706 | `frm1706:txtCompromise` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 1706 | `frm1706:txtCompromise` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 1706 | `frm1706:txtCompromise` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 1707 | `frm1707:txtPg2P4Sch1I1SellPrice` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1707 | `frm1707:txtPg2P4Sch1I1SellPrice` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1707 | `frm1707:txtPg2P4Sch1I1SellPrice` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1707 | `frm1707:txtPg1P2I17Compromise` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | unclear-official |
| 1707 | `frm1707:txtPg1P2I17Compromise` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | unclear-official |
| 1707 | `frm1707:txtPg1P2I17Compromise` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | unclear-official |
| 1707 | `frm1707:txtPg2P4Sch3Amount1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 1707 | `frm1707:txtPg2P4Sch3Amount1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 1707 | `frm1707:txtPg2P4Sch3Amount1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 1707A | `frm1707Av2021:txtI18ASurcharge` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.00 | unclear-official |
| 1707A | `frm1707Av2021:txtI18ASurcharge` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.00 | unclear-official |
| 1707A | `frm1707Av2021:txtI18ASurcharge` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.67 | unclear-official |
| 1707A | `frm1707Av2021:txtS1C3SellingPriceI1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-parseFloat/toFixed(2) | 1000 | 1,000.00 | unclear-official |
| 1707A | `frm1707Av2021:txtS1C3SellingPriceI1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-parseFloat/toFixed(2) | 1 | 1.00 | unclear-official |
| 1707A | `frm1707Av2021:txtS1C3SellingPriceI1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-parseFloat/toFixed(2) | 2.67 | 2.67 | unclear-official |
| 1800 | `frm1800v2018:TaxCredit17A` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 1800 | `frm1800v2018:TaxCredit17A` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 1800 | `frm1800v2018:TaxCredit17A` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 1800 | `frm1800v2018:Compromise` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 1800 | `frm1800v2018:Compromise` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 1800 | `frm1800v2018:Compromise` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 1800 | `frm1800v2018:schedA:txtFairMarketValue0` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 1800 | `frm1800v2018:schedA:txtFairMarketValue0` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 1800 | `frm1800v2018:schedA:txtFairMarketValue0` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 1801 | `frm1801v2018:sched5EstateConj` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 1801 | `frm1801v2018:sched5EstateConj` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 1801 | `frm1801v2018:sched5EstateConj` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 1801 | `frm1801v2018:txtPen_Compromise` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 1801 | `frm1801v2018:txtPen_Compromise` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 1801 | `frm1801v2018:txtPen_Compromise` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 1801 | `frm1801v2018:sched1AConj_1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 1801 | `frm1801v2018:sched1AConj_1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 1801 | `frm1801v2018:sched1AConj_1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 2000 | `frm2000:txtTax15A` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 2000 | `frm2000:txtTax15A` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 2000 | `frm2000:txtTax15A` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 2000 | `frm2000:txtTax17A` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 2000 | `frm2000:txtTax17A` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 2000 | `frm2000:txtTax17A` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 2000 | `frm2000:sched1:txtTaxBase0` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 2000 | `frm2000:sched1:txtTaxBase0` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 2000 | `frm2000:sched1:txtTaxBase0` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 2000OT | `frm2000OT:Sched2:txtGrossSellingPrice` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 2000OT | `frm2000OT:Sched2:txtGrossSellingPrice` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 2000OT | `frm2000OT:Sched2:txtGrossSellingPrice` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 2000OT | `frm2000OT:txtTax20A` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 2000OT | `frm2000OT:txtTax20A` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 2000OT | `frm2000OT:txtTax20A` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 2000OT | `frm2000OT:sched1A:txtFMVCol10` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 2000OT | `frm2000OT:sched1A:txtFMVCol10` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 2000OT | `frm2000OT:sched1A:txtFMVCol10` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 2200S | `frm2200S:txtTax17A` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-parseFloat/toFixed(2) | 1000 | 1,000.00 | unclear-official |
| 2200S | `frm2200S:txtTax17A` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-parseFloat/toFixed(2) | 1 | 1.00 | unclear-official |
| 2200S | `frm2200S:txtTax17A` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-parseFloat/toFixed(2) | 2.67 | 2.67 | unclear-official |
| 2200S | `frm2200S:txtTax21A` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | refused: `frm2200S:txtTax21A` is computed; not part of this return as it stands | — | 0.00 | inconclusive-gated |
| 2200S | `frm2200S:txtTax21A` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | refused: `frm2200S:txtTax21A` is computed; not part of this return as it stands | — | 0.00 | inconclusive-gated |
| 2200S | `frm2200S:txtTax21A` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | refused: `frm2200S:txtTax21A` is computed; not part of this return as it stands | — | 0.00 | inconclusive-gated |
| 2200S | `frm2200S:txtsched1SalesValue12` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | accepted-no-numeric-store | — | — | inconclusive-no-numeric-store |
| 2200S | `frm2200S:txtsched1SalesValue12` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | accepted-no-numeric-store | — | — | inconclusive-no-numeric-store |
| 2200S | `frm2200S:txtsched1SalesValue12` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | accepted-no-numeric-store | — | — | inconclusive-no-numeric-store |
| 2200C | `frm2200C:txtPg1P3I17ABalCarriedOver` | ordinary-item | `2200c-defect-028-whole-peso-rounding` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | mismatch-different-rounding |
| 2200C | `frm2200C:txtPg1P3I17ABalCarriedOver` | ordinary-item | `2200c-defect-028-whole-peso-rounding` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | mismatch-different-rounding |
| 2200C | `frm2200C:txtPg1P3I17ABalCarriedOver` | ordinary-item | `2200c-defect-028-whole-peso-rounding` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | mismatch-different-rounding |
| 2200C | `frm2200C:txtPg1P3I21ASurcharge` | penalty-item | `2200c-defect-028-whole-peso-rounding` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 2200C | `frm2200C:txtPg1P3I21ASurcharge` | penalty-item | `2200c-defect-028-whole-peso-rounding` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 2200C | `frm2200C:txtPg1P3I21ASurcharge` | penalty-item | `2200c-defect-028-whole-peso-rounding` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 2200C | `frm2200C:CPPexcise1` | schedule-row | `2200c-defect-028-whole-peso-rounding` | 1000.005 | refused: Needs item 13 or item 15 | — | 0.00 | inconclusive-gated |
| 2200C | `frm2200C:CPPexcise1` | schedule-row | `2200c-defect-028-whole-peso-rounding` | 1.005 | refused: Needs item 13 or item 15 | — | 0.00 | inconclusive-gated |
| 2200C | `frm2200C:CPPexcise1` | schedule-row | `2200c-defect-028-whole-peso-rounding` | 2.675 | refused: Needs item 13 or item 15 | — | 0.00 | inconclusive-gated |
| 2200AN | `frm2200ANv2018:txt17A` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 2200AN | `frm2200ANv2018:txt17A` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 2200AN | `frm2200ANv2018:txt17A` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 2200AN | `frm2200ANv2018:txt21A` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 2200AN | `frm2200ANv2018:txt21A` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 2200AN | `frm2200ANv2018:txt21A` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 2200AN | `frm2200ANv2018:txtXG021Exempt2` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 2200AN | `frm2200ANv2018:txtXG021Exempt2` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 2200AN | `frm2200ANv2018:txtXG021Exempt2` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 2200M | `frm2200M:txtPg1P3I17ABalCarriedOver` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 2200M | `frm2200M:txtPg1P3I17ABalCarriedOver` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 2200M | `frm2200M:txtPg1P3I17ABalCarriedOver` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 2200M | `frm2200M:txtPg1P3I21ASurcharge` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 2200M | `frm2200M:txtPg1P3I21ASurcharge` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 2200M | `frm2200M:txtPg1P3I21ASurcharge` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 2200M | `frm2200M:txtSched1_ImportedExemptH1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 2200M | `frm2200M:txtSched1_ImportedExemptH1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 2200M | `frm2200M:txtSched1_ImportedExemptH1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 2200T | `frm2200Tv2020:txtLess_Balance` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 2200T | `frm2200Tv2020:txtLess_Balance` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 2200T | `frm2200Tv2020:txtLess_Balance` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 2200T | `frm2200Tv2020:txtPen_Compromise` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | unclear-official |
| 2200T | `frm2200Tv2020:txtPen_Compromise` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | unclear-official |
| 2200T | `frm2200Tv2020:txtPen_Compromise` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | unclear-official |
| 2200T | `frm2200Tv2020:txtSched1_1A_2020_Due` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 2200T | `frm2200Tv2020:txtSched1_1A_2020_Due` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 2200T | `frm2200Tv2020:txtSched1_1A_2020_Due` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 2200A | `frm2200Av2020:txtLess_Balance` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 2200A | `frm2200Av2020:txtLess_Balance` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 2200A | `frm2200Av2020:txtLess_Balance` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 2200A | `frm2200Av2020:txtPen_Compromise` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | unclear-official |
| 2200A | `frm2200Av2020:txtPen_Compromise` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | unclear-official |
| 2200A | `frm2200Av2020:txtPen_Compromise` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | unclear-official |
| 2200A | `frm2200Av2020:txtSched1_1A_2020_Export` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 2200A | `frm2200Av2020:txtSched1_1A_2020_Export` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 2200A | `frm2200Av2020:txtSched1_1A_2020_Export` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 2200P | `frm2200P:txtPg1P3I17ABalCarriedOver` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 2200P | `frm2200P:txtPg1P3I17ABalCarriedOver` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 2200P | `frm2200P:txtPg1P3I17ABalCarriedOver` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 2200P | `frm2200P:txtPg1P3I21ASurcharge` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 2200P | `frm2200P:txtPg1P3I21ASurcharge` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 2200P | `frm2200P:txtPg1P3I21ASurcharge` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 2200P | `frm2200P:txtSched1BasicExciseTaxDue1` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 2200P | `frm2200P:txtSched1BasicExciseTaxDue1` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 2200P | `frm2200P:txtSched1BasicExciseTaxDue1` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 1701MS | `frm1701MS:txtAllowableItemizedDeductionsNo10a` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-other:0 | 0 | 0 | inconclusive-dropped-in-incomplete-draft |
| 1701MS | `frm1701MS:txtAllowableItemizedDeductionsNo10a` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-other:0 | 0 | 0 | inconclusive-dropped-in-incomplete-draft |
| 1701MS | `frm1701MS:txtAllowableItemizedDeductionsNo10a` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-other:0 | 0 | 0 | inconclusive-dropped-in-incomplete-draft |
| 1701MS | `frm1701MS:txtCompromise28a` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000 | mismatch-kept-3dp |
| 1701MS | `frm1701MS:txtCompromise28a` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1 | mismatch-kept-3dp |
| 1701MS | `frm1701MS:txtCompromise28a` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 3 | mismatch-kept-3dp |
| 1702Q | `frm1702q:Sched1:txtTax1A` | ordinary-item | `1702q-input-001` | 1000.005 | refused: Needs item 5 = an ATC | 0 | 0.00 | inconclusive-gated |
| 1702Q | `frm1702q:Sched1:txtTax1A` | ordinary-item | `1702q-input-001` | 1.005 | refused: Needs item 5 = an ATC | 0 | 0.00 | inconclusive-gated |
| 1702Q | `frm1702q:Sched1:txtTax1A` | ordinary-item | `1702q-input-001` | 2.675 | refused: Needs item 5 = an ATC | 0 | 0.00 | inconclusive-gated |
| 1702Q | `frm1702q:txtTax21` | penalty-item | `1702q-input-001` | 1000.005 | kept-3dp | 1000.005 | 1,000.00 | mismatch-kept-3dp |
| 1702Q | `frm1702q:txtTax21` | penalty-item | `1702q-input-001` | 1.005 | kept-3dp | 1.005 | 1.00 | mismatch-kept-3dp |
| 1702Q | `frm1702q:txtTax21` | penalty-item | `1702q-input-001` | 2.675 | kept-3dp | 2.675 | 3.00 | mismatch-kept-3dp |
| 1702Q | `frm1702q:Sched4:txtOthrTxCrdtAmnt0` | schedule-row | `1702q-input-001` | 1000.005 | refused: Needs item 5 = an ATC | — | 0.00 | inconclusive-gated |
| 1702Q | `frm1702q:Sched4:txtOthrTxCrdtAmnt0` | schedule-row | `1702q-input-001` | 1.005 | refused: Needs item 5 = an ATC | — | 0.00 | inconclusive-gated |
| 1702Q | `frm1702q:Sched4:txtOthrTxCrdtAmnt0` | schedule-row | `1702q-input-001` | 2.675 | refused: Needs item 5 = an ATC | — | 0.00 | inconclusive-gated |
| 1701A | `frm1701A:txt23A` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 1701A | `frm1701A:txt23A` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 1701A | `frm1701A:txt23A` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 1701A | `frm1701A:txt25A` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | unclear-official |
| 1701A | `frm1701A:txt25A` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | unclear-official |
| 1701A | `frm1701A:txt25A` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | unclear-official |
| 1701A | `frm1701A:txtmodalPartIVA_1C2` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | accepted-no-numeric-store | — | — | inconclusive-no-numeric-store |
| 1701A | `frm1701A:txtmodalPartIVA_1C2` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | accepted-no-numeric-store | — | — | inconclusive-no-numeric-store |
| 1701A | `frm1701A:txtmodalPartIVA_1C2` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | accepted-no-numeric-store | — | — | inconclusive-no-numeric-store |
| 1700 | `frm1700:txtPg1I29A` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1700 | `frm1700:txtPg1I29A` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1700 | `frm1700:txtPg1I29A` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 1700 | `frm1700:txtPg1I31A` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 1700 | `frm1700:txtPg1I31A` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 1700 | `frm1700:txtPg1I31A` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 1700 | `frm1700:txtPg2I48A` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | accepted-no-numeric-store | — | 0.00 | inconclusive-no-numeric-store |
| 1700 | `frm1700:txtPg2I48A` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | accepted-no-numeric-store | — | 0.00 | inconclusive-no-numeric-store |
| 1700 | `frm1700:txtPg2I48A` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | accepted-no-numeric-store | — | 0.00 | inconclusive-no-numeric-store |
| 1702EX | `frm1702EX:txtPg1Pt2I21PenaltyCompromise` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 1702EX | `frm1702EX:txtPg1Pt2I21PenaltyCompromise` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 1702EX | `frm1702EX:txtPg1Pt2I21PenaltyCompromise` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 1702EX | `frm1702EX:txtPg2Pt4I48OtherTaxCrPmtAmt` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | unclear-official |
| 1702EX | `frm1702EX:txtPg2Pt4I48OtherTaxCrPmtAmt` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | unclear-official |
| 1702EX | `frm1702EX:txtPg2Pt4I48OtherTaxCrPmtAmt` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | unclear-official |
| 0605 | `frm0605:txtTax19` | ordinary-item | `0605-input-001-amount-format` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | mismatch-different-rounding |
| 0605 | `frm0605:txtTax19` | ordinary-item | `0605-input-001-amount-format` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | mismatch-different-rounding |
| 0605 | `frm0605:txtTax19` | ordinary-item | `0605-input-001-amount-format` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | mismatch-different-rounding |
| 0605 | `frm0605:txtTax20A` | penalty-item | `0605-input-001-amount-format` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 0605 | `frm0605:txtTax20A` | penalty-item | `0605-input-001-amount-format` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 0605 | `frm0605:txtTax20A` | penalty-item | `0605-input-001-amount-format` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 0619E | `frm0619E:txtTax15` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 0619E | `frm0619E:txtTax15` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 0619E | `frm0619E:txtTax15` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 0619E | `frm0619E:txtTax17A` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 0619E | `frm0619E:txtTax17A` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 0619E | `frm0619E:txtTax17A` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 0619F | `frm0619F:txtTax16` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 0619F | `frm0619F:txtTax16` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 0619F | `frm0619F:txtTax16` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-other:0 | 0 | 0.00 | inconclusive-dropped-in-incomplete-draft |
| 0619F | `frm0619F:txtTax18A` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 0619F | `frm0619F:txtTax18A` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 0619F | `frm0619F:txtTax18A` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 1701Q | `frm1701q:txt55A` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 1,000.01 | match |
| 1701Q | `frm1701q:txt55A` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 1.01 | match |
| 1701Q | `frm1701q:txt55A` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 2.68 | match |
| 1701Q | `frm1701q:txt64A` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 1701Q | `frm1701q:txt64A` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 1701Q | `frm1701q:txt64A` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 1702RT | `frm1702RT:txtPg1Pt2I17Surcharge` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000 | mismatch-kept-3dp |
| 1702RT | `frm1702RT:txtPg1Pt2I17Surcharge` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1 | mismatch-kept-3dp |
| 1702RT | `frm1702RT:txtPg1Pt2I17Surcharge` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 3 | mismatch-kept-3dp |
| 1701 | `frm1701:txtPg1I27A` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.01 | mismatch-kept-3dp |
| 1701 | `frm1701:txtPg1I27A` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.01 | mismatch-kept-3dp |
| 1701 | `frm1701:txtPg1I27A` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.68 | mismatch-kept-3dp |
| 1701 | `frm1701:txtPg2IShed1c_1CI` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | rounded-formatCurrency/round | 1000.01 | 0.00 | match |
| 1701 | `frm1701:txtPg2IShed1c_1CI` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | rounded-formatCurrency/round | 1.01 | 0.00 | match |
| 1701 | `frm1701:txtPg2IShed1c_1CI` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | rounded-formatCurrency/round | 2.68 | 0.00 | match |
| 1702MX | `frm1702MX:txtPg1Pt2I17` | penalty-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1000 | mismatch-kept-3dp |
| 1702MX | `frm1702MX:txtPg1Pt2I17` | penalty-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1 | mismatch-kept-3dp |
| 1702MX | `frm1702MX:txtPg1Pt2I17` | penalty-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 3 | mismatch-kept-3dp |
| 2550Q | `frm2550qv2024:addInputVat` | ordinary-item | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.00 | mismatch-kept-3dp |
| 2550Q | `frm2550qv2024:addInputVat` | ordinary-item | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.00 | mismatch-kept-3dp |
| 2550Q | `frm2550qv2024:addInputVat` | ordinary-item | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.67 | mismatch-kept-3dp |
| 2550Q | `frm2550qv2024:totalTaxPayableNo19Amount0` | schedule-row | `no validation rule; fields.json normalization` | 1000.005 | kept-3dp | 1000.005 | 1,000.00 | mismatch-kept-3dp |
| 2550Q | `frm2550qv2024:totalTaxPayableNo19Amount0` | schedule-row | `no validation rule; fields.json normalization` | 1.005 | kept-3dp | 1.005 | 1.00 | mismatch-kept-3dp |
| 2550Q | `frm2550qv2024:totalTaxPayableNo19Amount0` | schedule-row | `no validation rule; fields.json normalization` | 2.675 | kept-3dp | 2.675 | 2.67 | mismatch-kept-3dp |

## Forms that differ, worst first

| Rank | Form | Box | Kind | Ours | Why (1 stored three decimals, 2 wrong refusal, 3 different rounding) |
|---:|---|---|---|---|---|
| 1 | 0605 | `frm0605:txtTax20A` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 0619E | `frm0619E:txtTax17A` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 0619F | `frm0619F:txtTax18A` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1600WP | `frm1600WP:txtTax15A` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1601EQ | `frm1601EQ:txtTax26` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1601FQ | `frm1601FQ:txtTax28` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1602Q | `frm1602Q:txt24` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1606 | `frm1606:txtCompromise` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1700 | `frm1700:txtPg1I31A` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1701 | `frm1701:txtPg1I27A` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1701MS | `frm1701MS:txtCompromise28a` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1701Q | `frm1701q:txt64A` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1702MX | `frm1702MX:txtPg1Pt2I17` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1702Q | `frm1702q:txtTax21` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1702RT | `frm1702RT:txtPg1Pt2I17Surcharge` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1706 | `frm1706:txtCompromise` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1800 | `frm1800v2018:Compromise` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 1801 | `frm1801v2018:txtPen_Compromise` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 2000 | `frm2000:txtTax17A` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 2000OT | `frm2000OT:txtTax20A` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 2200AN | `frm2200ANv2018:txt21A` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 2200C | `frm2200C:txtPg1P3I21ASurcharge` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 2200M | `frm2200M:txtPg1P3I21ASurcharge` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 2200P | `frm2200P:txtPg1P3I21ASurcharge` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 2550M | `frm2550m:txtTax25A` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 2550Q | `frm2550qv2024:addInputVat` | ordinary-item | kept-3dp | mismatch-kept-3dp |
| 1 | 2550Q | `frm2550qv2024:totalTaxPayableNo19Amount0` | schedule-row | kept-3dp | mismatch-kept-3dp |
| 1 | 2551Q | `frm2551Qv2018:txt15` | ordinary-item | kept-3dp | mismatch-kept-3dp |
| 1 | 2551Q | `frm2551Qv2018:txt20` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 1 | 2553 | `frm2553:txt22A` | penalty-item | kept-3dp | mismatch-kept-3dp |
| 2 | 2551Q | `txtATCAmt1` | schedule-row | refused | mismatch-wrong-refusal |
| 3 | 0605 | `frm0605:txtTax19` | ordinary-item | rounded-formatCurrency/round | mismatch-different-rounding |
| 3 | 1601FQ | `frm1601FQ:txtTax23` | ordinary-item | rounded-formatCurrency/round | mismatch-different-rounding |
| 3 | 1601FQ | `txtAmtIncomePay0` | schedule-row | rounded-formatCurrency/round | mismatch-different-rounding |
| 3 | 2200C | `frm2200C:txtPg1P3I17ABalCarriedOver` | ordinary-item | rounded-formatCurrency/round | mismatch-different-rounding |
| 3 | 2550M | `frm2550m:txtTax13A` | ordinary-item | rounded-formatCurrency/round | mismatch-different-rounding |
| 3 | 2550M | `txtAmountPaidSch70` | schedule-row | rounded-formatCurrency/round | mismatch-different-rounding |

## Official-rule and probe caveats

These rows are not counted as mismatches: the rules corpus does not settle the exact blur rounding, the first representative was gated, or an incomplete draft dropped/held the value before the probe could measure entry rounding.

| Rank | Form | Box | Kind | Ours | Why (4 official rule unclear, 5 probe gated, 6 value dropped, 7 no numeric stored value) |
|---:|---|---|---|---|---|
| 4 | 1600PT | `frm1600PT:txtPg1P2I23Surcharge` | penalty-item | kept-3dp | unclear-official |
| 4 | 1600PT | `frm1600PT:Pg2Sc1AmtOfIncPmt1` | schedule-row | rounded-formatCurrency/round | unclear-official |
| 4 | 1600VT | `frm1600VT:txtPg1P2I23Surcharge` | penalty-item | kept-3dp | unclear-official |
| 4 | 1600VT | `frm1600VT:Pg2Sc1AmtOfIncPmt1` | schedule-row | rounded-formatCurrency/round | unclear-official |
| 4 | 1601C | `frm1601c:txtTax14` | ordinary-item | kept-3dp | unclear-official |
| 4 | 1601C | `frm1601c:txtTax32` | penalty-item | kept-3dp | unclear-official |
| 4 | 1601C | `frm1601c:sched1:txtShouldTaxDue0` | schedule-row | kept-3dp | unclear-official |
| 4 | 1603Q | `frm1603Q:txtTax19` | penalty-item | kept-3dp | unclear-official |
| 4 | 1603Q | `frm1603Q:Sched1:txtValue1` | schedule-row | rounded-formatCurrency/round | unclear-official |
| 4 | 1604F | `frm1604f:txtSched1Pen1` | schedule-row | rounded-formatCurrency/round | unclear-official |
| 4 | 1701A | `frm1701A:txt23A` | ordinary-item | rounded-formatCurrency/round | unclear-official |
| 4 | 1701A | `frm1701A:txt25A` | penalty-item | kept-3dp | unclear-official |
| 4 | 1702EX | `frm1702EX:txtPg1Pt2I21PenaltyCompromise` | ordinary-item | rounded-formatCurrency/round | unclear-official |
| 4 | 1702EX | `frm1702EX:txtPg2Pt4I48OtherTaxCrPmtAmt` | schedule-row | rounded-formatCurrency/round | unclear-official |
| 4 | 1707 | `frm1707:txtPg1P2I17Compromise` | penalty-item | kept-3dp | unclear-official |
| 4 | 1707 | `frm1707:txtPg2P4Sch3Amount1` | schedule-row | rounded-formatCurrency/round | unclear-official |
| 4 | 1707A | `frm1707Av2021:txtI18ASurcharge` | penalty-item | kept-3dp | unclear-official |
| 4 | 1707A | `frm1707Av2021:txtS1C3SellingPriceI1` | schedule-row | rounded-parseFloat/toFixed(2) | unclear-official |
| 4 | 2200A | `frm2200Av2020:txtLess_Balance` | ordinary-item | rounded-formatCurrency/round | unclear-official |
| 4 | 2200A | `frm2200Av2020:txtPen_Compromise` | penalty-item | kept-3dp | unclear-official |
| 4 | 2200A | `frm2200Av2020:txtSched1_1A_2020_Export` | schedule-row | rounded-formatCurrency/round | unclear-official |
| 4 | 2200S | `frm2200S:txtTax17A` | ordinary-item | rounded-parseFloat/toFixed(2) | unclear-official |
| 4 | 2200T | `frm2200Tv2020:txtLess_Balance` | ordinary-item | rounded-formatCurrency/round | unclear-official |
| 4 | 2200T | `frm2200Tv2020:txtPen_Compromise` | penalty-item | kept-3dp | unclear-official |
| 4 | 2200T | `frm2200Tv2020:txtSched1_1A_2020_Due` | schedule-row | rounded-formatCurrency/round | unclear-official |
| 4 | 2552 | `frm2552:txtPg1P2I18CredTaxWthhld` | ordinary-item | rounded-formatCurrency/round | unclear-official |
| 4 | 2552 | `frm2552:txtPg1P2I21Surcharge` | penalty-item | kept-3dp | unclear-official |
| 4 | 2552 | `frm2552:txtPg2P4I1Amount` | schedule-row | rounded-formatCurrency/round | unclear-official |
| 5 | 1601EQ | `frm1601EQ:txtTaxBase9` | schedule-row | refused | inconclusive-gated |
| 5 | 1604C | `frm1604c:txt12` | ordinary-item | refused | inconclusive-gated |
| 5 | 1702Q | `frm1702q:Sched1:txtTax1A` | ordinary-item | refused | inconclusive-gated |
| 5 | 1702Q | `frm1702q:Sched4:txtOthrTxCrdtAmnt0` | schedule-row | refused | inconclusive-gated |
| 5 | 2200C | `frm2200C:CPPexcise1` | schedule-row | refused | inconclusive-gated |
| 5 | 2200S | `frm2200S:txtTax21A` | penalty-item | refused | inconclusive-gated |
| 6 | 0619E | `frm0619E:txtTax15` | ordinary-item | rounded-other:0 | inconclusive-dropped-in-incomplete-draft |
| 6 | 0619F | `frm0619F:txtTax16` | ordinary-item | rounded-other:0 | inconclusive-dropped-in-incomplete-draft |
| 6 | 1600PT | `frm1600PT:txtPg1P2I19TaxRemittedInRtrn` | ordinary-item | rounded-other:0 | inconclusive-dropped-in-incomplete-draft |
| 6 | 1600VT | `frm1600VT:txtPg1P2I19TaxRemittedInRtrn` | ordinary-item | rounded-other:0 | inconclusive-dropped-in-incomplete-draft |
| 6 | 1600WP | `frm1600WP:txtTax13` | ordinary-item | rounded-other:0 | inconclusive-dropped-in-incomplete-draft |
| 6 | 1603Q | `frm1603Q:txtTax15` | ordinary-item | rounded-other:0 | inconclusive-dropped-in-incomplete-draft |
| 6 | 1700 | `frm1700:txtPg1I29A` | ordinary-item | rounded-other:0 | inconclusive-dropped-in-incomplete-draft |
| 6 | 1701MS | `frm1701MS:txtAllowableItemizedDeductionsNo10a` | ordinary-item | rounded-other:0 | inconclusive-dropped-in-incomplete-draft |
| 6 | 1707 | `frm1707:txtPg2P4Sch1I1SellPrice` | ordinary-item | rounded-other:0 | inconclusive-dropped-in-incomplete-draft |
| 6 | 2000 | `frm2000:txtTax15A` | ordinary-item | rounded-other:0 | inconclusive-dropped-in-incomplete-draft |
| 7 | 1700 | `frm1700:txtPg2I48A` | schedule-row | accepted-no-numeric-store | inconclusive-no-numeric-store |
| 7 | 1701A | `frm1701A:txtmodalPartIVA_1C2` | schedule-row | accepted-no-numeric-store | inconclusive-no-numeric-store |
| 7 | 2200S | `frm2200S:txtsched1SalesValue12` | schedule-row | accepted-no-numeric-store | inconclusive-no-numeric-store |
