<!--
             Notes on the grouping rule: a group is one printed section's column,
             from the section's Part/Schedule band heading to the next heading on
             that page, at one column x (within 2 pt). A column also splits where
             a sub-table's column headers START inside the comb's width between
             two rows, and at a lettered sub-heading that names a different person
             or schedule of its own (such as 1701 Schedule 5's "5.A –
             Taxpayer/Filer" and "5.B – Spouse"): what sits above it is never
             the same column of figures as what sits below. Hidden template rows
             never decide geometry.
-->

## 0605-1999: 2 groups, 1 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c29..p1c29 | Part I                                                                           B a c k g r o u n d    I n f o r m a t i o n |  |
| 1 | p1c36..p1c36 | Part I                                                                           B a c k g r o u n d    I n f o r m a t i o n | frm0605:txtTelNum |

## 0619e-2018: 10 groups, 9 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c34..p1c34 | Part I – Background Information |  |
| 1 | p1c39..p1c39 | Part I – Background Information | frm0619E:txtTaxpayerName |
| 2 | p1c41..p1c42 | Part I – Background Information | frm0619E:txtAddress |
| 1 | p1c51..p1c51 | Part I – Background Information | txtEmail |
| 1 | p1c46..p1c46 | Part I – Background Information | frm0619E:txtTelNum |
| 8 | p1c54..p1c83 | Part II – Tax Remittance | frm0619E:txtTax14 |
| 3 | p1c104..p1c131 | Part III – Details of Payment | txtAgency19 |
| 4 | p1c105..p1c132 | Part III – Details of Payment | txtNumber19 |
| 4 | p1c109..p1c136 | Part III – Details of Payment | txtAmount19 |
| 1 | p1c130..p1c130 | Part III – Details of Payment | txtParticular22 |

## 0619f-2018: 10 groups, 9 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c34..p1c34 | Part I – Background Information |  |
| 1 | p1c39..p1c39 | Part I – Background Information | frm0619F:txtTaxpayerName |
| 2 | p1c41..p1c42 | Part I – Background Information | frm0619F:txtAddress |
| 1 | p1c51..p1c51 | Part I – Background Information | txtEmail |
| 1 | p1c46..p1c46 | Part I – Background Information | frm0619F:txtTelNum |
| 10 | p1c58..p1c96 | Part II – Tax Remittance | frm0619F:txtTax13 |
| 3 | p1c118..p1c145 | Part III – Details of Payment | txtAgency20 |
| 4 | p1c119..p1c146 | Part III – Details of Payment | txtNumber20 |
| 4 | p1c123..p1c150 | Part III – Details of Payment | txtAmount20 |
| 1 | p1c144..p1c144 | Part III – Details of Payment | txtParticular23 |

## 1600-pt-2018: 15 groups, 11 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c22..p1c22 | Part I – Background Information | frm1600PT:txtPg1TIN4 |
| 1 | p1c26..p1c26 | Part I – Background Information | frm1600PT:txtPg1Pt1I7WithholdingAgentsName |
| 2 | p1c28..p1c29 | Part I – Background Information | frm1600PT:txtPg1Pt1I8RegisteredAddress |
| 1 | p1c38..p1c38 | Part I – Background Information | frm1600PT:txtPg1Pt1I11Email |
| 1 | p1c33..p1c33 | Part I – Background Information | frm1600PT:txtPg1Pt1I9ContactNumber |
| 1 | p1c41..p1c41 | Part I – Background Information | frm1600PT:txtPg1Pt1I12TaxReliefSpec |
| 5 | p1c50..p1c90 | Part II – Computation of Tax | frm1600PT:txtAtcCd1 |
| 5 | p1c51..p1c91 | Part II – Computation of Tax | frm1600PT:txtPg1P2TaxBase1 |
| 15 | p1c56..p1c136 | Part II – Computation of Tax | frm1600PT:txtPg1P2TaxWithheld1 |
| 3 | p1c158..p1c186 | Part III – Details  of Payment |  |
| 4 | p1c159..p1c187 | Part III – Details  of Payment |  |
| 4 | p1c163..p1c191 | Part III – Details  of Payment |  |
| 1 | p1c185..p1c185 | Part III – Details  of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1600PT:txtPg2RegisteredName |

## 1600-vt-2018: 15 groups, 11 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c22..p1c22 | Part I – Background Information | frm1600VT:txtPg1TIN4 |
| 1 | p1c26..p1c26 | Part I – Background Information | frm1600VT:txtPg1Pt1I7WithholdingAgentsName |
| 2 | p1c28..p1c29 | Part I – Background Information | frm1600VT:txtPg1Pt1I8RegisteredAddress |
| 1 | p1c38..p1c38 | Part I – Background Information | frm1600VT:txtPg1Pt1I11Email |
| 1 | p1c33..p1c33 | Part I – Background Information | frm1600VT:txtPg1Pt1I9ContactNumber |
| 1 | p1c41..p1c41 | Part I – Background Information | frm1600VT:txtPg1Pt1I12TaxReliefSpec |
| 5 | p1c50..p1c90 | Part II – Computation of Tax | frm1600VT:txtAtcCd1 |
| 5 | p1c51..p1c91 | Part II – Computation of Tax | frm1600VT:txtPg1P2TaxBase1 |
| 15 | p1c56..p1c136 | Part II – Computation of Tax | frm1600VT:txtPg1P2TaxWithheld1 |
| 3 | p1c158..p1c186 | Part III – Details  of Payment |  |
| 4 | p1c159..p1c187 | Part III – Details  of Payment |  |
| 4 | p1c163..p1c191 | Part III – Details  of Payment |  |
| 1 | p1c185..p1c185 | Part III – Details  of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1600VT:txtPg2RegisteredName |

## 1600wp-2010: 3 groups, 3 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c1..p1c1 | (header) | derived:from_date |
| 1 | p1c16..p1c16 | Part I | frm1600WP:txtBranchCode |
| 1 | p1c74..p1c74 | Part II | derived:att_branch |

## 1601-fq-2020: 15 groups, 15 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c33..p1c33 | Part I – Background Information | frm1601FQ:txtBranchCode |
| 1 | p1c37..p1c37 | Part I – Background Information | frm1601FQ:txtTaxpayerName |
| 2 | p1c39..p1c40 | Part I – Background Information | derived:address_line1 |
| 1 | p1c44..p1c44 | Part I – Background Information | frm1601FQ:txtTelNum |
| 1 | p1c49..p1c49 | Part I – Background Information | txtEmail |
| 1 | p1c52..p1c52 | Part I – Background Information | derived:line:tax_relief_specify |
| 6 | p1c61..p1c121 | Part II – Computation of Tax | frm1601FQ:txtAtcCode1 |
| 6 | p1c62..p1c122 | Part II – Computation of Tax | frm1601FQ:txtTaxBase1 |
| 19 | p1c69..p1c181 | Part II – Computation of Tax | frm1601FQ:txtTaxbeWithHeld1 |
| 3 | p1c202..p1c229 | Part III – Details of Payment | frm1601FQ:txtAgency33 |
| 4 | p1c203..p1c230 | Part III – Details of Payment | frm1601FQ:txtNumber33 |
| 4 | p1c207..p1c234 | Part III – Details of Payment | frm1601FQ:txtAmount33 |
| 1 | p1c228..p1c228 | Part III – Details of Payment | frm1601FQ:txtParticular36 |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1601FQ:txtPg2TaxpayerName |

## 1601c-2018: 18 groups, 17 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c32..p1c32 | Part I – Background Information |  |
| 1 | p1c36..p1c36 | Part I – Background Information | frm1601c:txtTaxpayerName |
| 2 | p1c38..p1c39 | Part I – Background Information | derived:address_line1 |
| 1 | p1c43..p1c43 | Part I – Background Information | frm1601c:txtTelNum |
| 1 | p1c48..p1c48 | Part I – Background Information | txtEmail |
| 1 | p1c51..p1c51 | Part I – Background Information | derived:line:tax_relief_specify |
| 23 | p1c56..p1c145 | Part II – Computation of Tax | frm1601c:txtTax14 |
| 3 | p1c166..p1c193 | Part III – Details of Payment | frm1601c:txtAgency37 |
| 4 | p1c167..p1c194 | Part III – Details of Payment | frm1601c:txtNumber37 |
| 4 | p1c171..p1c198 | Part III – Details of Payment | frm1601c:txtAmount37 |
| 1 | p1c192..p1c192 | Part III – Details of Payment | frm1601c:txtParticular40 |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1601c:txtPg2TaxpayerName |
| 3 | p2c20..p2c42 | Schedule I | frm1601c:sched1:txtBankCode0 |
| 3 | p2c21..p2c43 | Schedule I | frm1601c:sched1:txtNumber0 |
| 3 | p2c22..p2c44 | Schedule I | frm1601c:sched1:txtTaxPaid0 |
| 3 | p2c53..p2c73 | Schedule I | frm1601c:sched1:txtShouldTaxDue0 |
| 4 | p2c57..p2c83 | Schedule I | frm1601c:sched1:txtAdjustments0 |

## 1601eq-2019: 14 groups, 14 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c26..p1c26 | Part I – Background Information | frm1601EQ:txtBranchCode |
| 1 | p1c30..p1c30 | Part I – Background Information | frm1601EQ:txtTaxpayerName |
| 2 | p1c32..p1c33 | Part I – Background Information | derived:address_line1 |
| 1 | p1c37..p1c37 | Part I – Background Information | frm1601EQ:txtTelNum |
| 1 | p1c42..p1c42 | Part I – Background Information | txtEmail |
| 6 | p1c49..p1c94 | Part II – Computation of Tax | frm1601EQ:txtAtcCd1 |
| 6 | p1c50..p1c95 | Part II – Computation of Tax | frm1601EQ:txtTaxBase1 |
| 19 | p1c54..p1c151 | Part II – Computation of Tax | frm1601EQ:txtTaxbeWithHeld1 |
| 3 | p1c176..p1c203 | Part III – Details of Payment | frm1601EQ:txtAgency34 |
| 4 | p1c177..p1c204 | Part III – Details of Payment | frm1601EQ:txtNumber34 |
| 4 | p1c181..p1c208 | Part III – Details of Payment | frm1601EQ:txtAmount34 |
| 1 | p1c202..p1c202 | Part III – Details of Payment | frm1601EQ:txtParticular36 |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1601EQ:txtTaxpayerName |

## 1602q-2019: 23 groups, 19 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c33..p1c33 | Part I – Background Information | frm1602Q:txtBranchCode |
| 1 | p1c37..p1c37 | Part I – Background Information | frm1602Q:txtTaxpayerName |
| 2 | p1c39..p1c40 | Part I – Background Information | derived:address_line1 |
| 1 | p1c44..p1c44 | Part I – Background Information | frm1602Q:txtTelNum |
| 1 | p1c49..p1c49 | Part I – Background Information | txtEmail |
| 1 | p1c52..p1c52 | Part I – Background Information | derived:line:tax_relief_specify |
| 15 | p1c57..p1c113 | Part II – Computation of Tax | frm1602Q:txt14 |
| 3 | p1c135..p1c162 | Part III – Details of Payment |  |
| 4 | p1c136..p1c163 | Part III – Details of Payment |  |
| 4 | p1c140..p1c167 | Part III – Details of Payment |  |
| 1 | p1c161..p1c161 | Part III – Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1602Q:txtPage2Agent |
| 14 | p2c16..p2c140 | Schedule 1 | frm1602Q:txtSched1Amount1 |
| 15 | p2c21..p2c151 | Schedule 1 | frm1602Q:txtSched1Tax1 |
| 3 | p2c162..p2c182 | Schedule 2 | frm1602Q:txtSched2ATC1 |
| 3 | p2c163..p2c183 | Schedule 2 | frm1602Q:txtSched2Interest1 |
| 4 | p2c167..p2c191 | Schedule 2 | frm1602Q:txtSched2TaxesWithheld1 |
| 3 | p2c201..p2c221 | Schedule 3 | frm1602Q:txtSched3IPA1 |
| 3 | p2c203..p2c223 | Schedule 3 | frm1602Q:txtSched3TotInterest1 |
| 4 | p2c207..p2c231 | Schedule 3 | frm1602Q:txtSched3TaxWithheld1 |
| 1 | p3c5..p3c5 | (header) | derived:tin_digits |
| 1 | p3c6..p3c6 | (header) | frm1602Q:txtPage3Agent |

## 1603q-2018: 16 groups, 16 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c33..p1c33 | Part I – Background Information | frm1603Q:txtBranchCode |
| 1 | p1c37..p1c37 | Part I – Background Information | frm1603Q:txtTaxpayerName |
| 2 | p1c39..p1c40 | Part I – Background Information | derived:address_line1 |
| 1 | p1c44..p1c44 | Part I – Background Information | frm1603Q:txtTelNum |
| 1 | p1c49..p1c49 | Part I – Background Information | txtEmail |
| 1 | p1c52..p1c52 | Part I – Background Information | derived:line:tax_relief_specify |
| 10 | p1c57..p1c93 | Part II – Computation of Tax | frm1603Q:txtTax14 |
| 3 | p1c114..p1c141 | Part III – Details of Payment | frm1603Q:txtAgency24 |
| 4 | p1c115..p1c142 | Part III – Details of Payment | frm1603Q:txtNumber24 |
| 4 | p1c119..p1c146 | Part III – Details of Payment | frm1603Q:txtAmount24 |
| 1 | p1c140..p1c140 | Part III – Details of Payment | frm1603Q:txtParticular27 |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1603Q:txtPg2TaxpayerName |
| 2 | p2c15..p2c21 | Schedule 1 | frm1603Q:Sched1:txtValue1 |
| 2 | p2c31..p2c40 | Schedule 1 | frm1603Q:Sched1:txtTaxBase1 |
| 3 | p2c35..p2c48 | Schedule 1 | frm1603Q:Sched1:txtTaxWithheld1 |

## 1604c-2018: 6 groups, 6 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c18..p1c18 | Part I – Background Information | frm1604c:branchCode |
| 1 | p1c22..p1c22 | Part I – Background Information | frm1604c:registeredName |
| 2 | p1c24..p1c25 | Part I – Background Information | derived:line:address1 |
| 1 | p1c36..p1c36 | Part I – Background Information | frm1604c:telephoneNumber |
| 1 | p1c37..p1c37 | Part I – Background Information | txtEmail |
| 1 | p1c46..p1c46 | Part I – Background Information | derived:overremittance |

## 1604e-2018: 7 groups, 7 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c18..p1c18 | Part I – Background Information | frm1604e:txtBranchCode |
| 1 | p1c22..p1c22 | Part I – Background Information | frm1604e:txtWthhldngAgntsNme |
| 2 | p1c24..p1c25 | Part I – Background Information | derived:address1 |
| 1 | p1c36..p1c36 | Part I – Background Information | frm1604e:txtTelNum |
| 1 | p1c37..p1c37 | Part I – Background Information | txtEmail |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1604e:txtPg2TaxpayerName |

## 1604f-2018: 8 groups, 8 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c18..p1c18 | Part I – Background Information | frm1604f:branchCode |
| 1 | p1c22..p1c22 | Part I – Background Information | frm1604f:registeredName |
| 2 | p1c24..p1c25 | Part I – Background Information | derived:address1 |
| 1 | p1c33..p1c33 | Part I – Background Information | frm1604f:telephoneNumber |
| 1 | p1c34..p1c34 | Part I – Background Information | frm1604f:email |
| 1 | p1c36..p1c36 | Part I – Background Information | frm1604f:availedTaxRelief |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1604f:txtPg2TaxpayerName |

## 1606-2018: 10 groups, 6 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c63..p1c63 | Part I – Background Information | derived:line:tax_relief_specify |
| 9 | p1c70..p1c108 | Part II – Computation of Tax | frm1606:txtTax |
| 3 | p1c130..p1c148 | Part III – Details  of Payment |  |
| 4 | p1c131..p1c158 | Part III – Details  of Payment |  |
| 4 | p1c135..p1c162 | Part III – Details  of Payment |  |
| 1 | p1c157..p1c157 | Part III – Details  of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:buyer_tin_digits |
| 1 | p2c6..p2c6 | (header) | derived:buyer_name |
| 5 | p2c81..p2c105 | Schedule 2 – Transaction Description | frm1606:txtSelling |
| 5 | p2c114..p2c131 | Schedule 3 –  Determination of Taxable Base | derived:sched3_1a |

## 1700-2018: 36 groups, 29 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c7..p1c7 | (header) |  |
| 1 | p1c20..p1c20 | Part I – Background Information on Taxpayer/Filer |  |
| 1 | p1c23..p1c23 | Part I – Background Information on Taxpayer/Filer | frm1700:txtPg1I7TaxpayerName |
| 2 | p1c25..p1c26 | Part I – Background Information on Taxpayer/Filer | derived:address_line1 |
| 1 | p1c38..p1c38 | Part I – Background Information on Taxpayer/Filer | frm1700:txtPg1I11Citizenship |
| 1 | p1c45..p1c45 | Part I – Background Information on Taxpayer/Filer | frm1700:txtPg1I14TelNum |
| 1 | p1c34..p1c34 | Part I – Background Information on Taxpayer/Filer | frm1700:txtPg1I10Email |
| 1 | p1c40..p1c40 | Part I – Background Information on Taxpayer/Filer | frm1700:txtPg1I13ForeignTaxNumber |
| 1 | p1c67..p1c67 | Part II – Background Information on Spouse |  |
| 1 | p1c70..p1c70 | Part II – Background Information on Spouse | frm1700:txtPg1I21SpouseName |
| 1 | p1c72..p1c72 | Part II – Background Information on Spouse | frm1700:txtPg1I22TSpouseTelNum |
| 1 | p1c74..p1c74 | Part II – Background Information on Spouse | frm1700:txtPg1I23SpouseCitizenship |
| 1 | p1c77..p1c77 | Part II – Background Information on Spouse | frm1700:txtPg1I25SpouseForeignTaxNumber |
| 10 | p1c83..p1c110 | Part III – Total Tax Payable | derived:amount_26A |
| 10 | p1c84..p1c111 | Part III – Total Tax Payable | derived:amount_26B |
| 1 | p1c113..p1c113 | Part III – Total Tax Payable | derived:amount_36 |
| 3 | p1c127..p1c148 | Part IV - Details of Payment |  |
| 4 | p1c128..p1c149 | Part IV - Details of Payment |  |
| 4 | p1c132..p1c153 | Part IV - Details of Payment |  |
| 1 | p1c147..p1c147 | Part IV - Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1700:txtPg2TaxpayerName |
| 6 | p2c13..p2c28 | Part V.A – Subject to Graduated Rates | derived:amount_42A |
| 6 | p2c14..p2c29 | Part V.A – Subject to Graduated Rates | derived:amount_42B |
| 6 | p2c32..p2c47 | Part V.B – Subject to 25% Flat Rate for NRANETB | derived:amount_48A |
| 6 | p2c33..p2c48 | Part V.B – Subject to 25% Flat Rate for NRANETB | derived:amount_48B |
| 6 | p2c51..p2c66 | Part V.C | derived:amount_54A |
| 6 | p2c52..p2c67 | Part V.C | derived:amount_54B |
| 2 | p2c73..p2c74 | Schedule 1 – Gross Compensation Income and Tax Withheld | frm1700:txtPg2I1PartVIEmployerName1 |
| 2 | p2c78..p2c79 | Schedule 1 – Gross Compensation Income and Tax Withheld | frm1700:txtPg2I2PartVIEmployerName1 |
| 2 | p2c83..p2c84 | Schedule 1 – Gross Compensation Income and Tax Withheld | frm1700:txtPg2I3PartVIEmployerName1 |
| 2 | p2c88..p2c89 | Schedule 1 – Gross Compensation Income and Tax Withheld | frm1700:txtPg2I4PartVIEmployerName1 |
| 4 | p2c76..p2c91 | Schedule 1 – Gross Compensation Income and Tax Withheld | derived:employer1_tin |
| 6 | p2c97..p2c117 | Schedule 1 – Gross Compensation Income and Tax Withheld | derived:amount_sched1_1c |
| 6 | p2c98..p2c118 | Schedule 1 – Gross Compensation Income and Tax Withheld | derived:amount_sched1_1d |
| 6 | p2c99..p2c119 | Schedule 1 – Gross Compensation Income and Tax Withheld | derived:amount_sched1_1e |

## 1701-2018: 53 groups, 51 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c19..p1c19 | PART I – Background Information of Taxpayer/Filer |  |
| 1 | p1c37..p1c37 | PART I – Background Information of Taxpayer/Filer | frm1701:txtPg1I8TaxpayerName |
| 2 | p1c39..p1c40 | PART I – Background Information of Taxpayer/Filer | frm1701:txtPg1I9Address |
| 1 | p1c52..p1c52 | PART I – Background Information of Taxpayer/Filer | frm1701:txtPg1I12Citizenship |
| 1 | p1c59..p1c59 | PART I – Background Information of Taxpayer/Filer | frm1701:txtPg1I15TelNum |
| 1 | p1c48..p1c48 | PART I – Background Information of Taxpayer/Filer | txtEmail |
| 1 | p1c54..p1c54 | PART I – Background Information of Taxpayer/Filer | frm1701:txtPg1I14ForeignTaxNumber |
| 10 | p1c87..p1c114 | PART II – Total Tax Payable | frm1701:txtPg1I22ATaxDue |
| 10 | p1c88..p1c115 | PART II – Total Tax Payable | frm1701:txtPg1I22BTaxDue |
| 1 | p1c117..p1c117 | PART II – Total Tax Payable | frm1701:txtPg1I32AggregateAmtPyble |
| 3 | p1c137..p1c158 | PART III - Details of Payment | frm1701:txtPg1I34Agency |
| 4 | p1c138..p1c159 | PART III - Details of Payment | frm1701:txtPg1I34Number |
| 4 | p1c142..p1c163 | PART III - Details of Payment | frm1701:txtPg1I34Amount |
| 1 | p1c157..p1c157 | PART III - Details of Payment | frm1701:txtPg1I37Particular |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1701:txtPg2TaxpayerName |
| 1 | p2c15..p2c15 | PART IV – Background Information of Spouse |  |
| 1 | p2c31..p2c31 | PART IV – Background Information of Spouse | frm1701:txtPg2I5SpouseName |
| 1 | p2c33..p2c33 | PART IV – Background Information of Spouse | frm1701:txtPg2I6TelNum |
| 1 | p2c35..p2c35 | PART IV – Background Information of Spouse | frm1701:txtPg2I7Citizenship |
| 1 | p2c38..p2c38 | PART IV – Background Information of Spouse | frm1701:txtPg2I9ForeignTaxNumber |
| 4 | p2c55..p2c61 | Schedule 1 – Gross Compensation Income and Tax Withheld | derived:sched1_row1_name_line1 |
| 2 | p2c58..p2c63 | Schedule 1 – Gross Compensation Income and Tax Withheld | derived:sched1_row1_tin |
| 4 | p2c68..p2c77 | Schedule 1 – Gross Compensation Income and Tax Withheld | frm1701:txtPg2IShed1c_1CI |
| 4 | p2c69..p2c78 | Schedule 1 – Gross Compensation Income and Tax Withheld | frm1701:txtPg2IShed1c_1TW |
| 4 | p2c84..p2c93 | Schedule 2 – Taxable Compensation Income | frm1701:txtPg2IShed2_4A |
| 4 | p2c85..p2c94 | Schedule 2 – Taxable Compensation Income | frm1701:txtPg2IShed2_4B |
| 18 | p2c98..p2c156 | Schedule 3 – Taxable Business Income | frm1701:txtPg2IShed3_8A |
| 18 | p2c99..p2c157 | Schedule 3 – Taxable Business Income | frm1701:txtPg2IShed3_8B |
| 1 | p3c5..p3c5 | (header) | derived:tin_digits |
| 1 | p3c6..p3c6 | (header) | frm1701:txtPg3TaxpayerName |
| 7 | p3c12..p3c33 | (header) | frm1701:txtPg3IShed3_26A |
| 7 | p3c13..p3c34 | (header) | frm1701:txtPg3IShed3_26B |
| 21 | p3c37..p3c103 | Schedule 4 - Ordinary Allowable Itemized Deductions | frm1701:txtPg3IShed4_1A |
| 21 | p3c38..p3c104 | Schedule 4 - Ordinary Allowable Itemized Deductions | frm1701:txtPg3IShed4_1B |
| 2 | p3c110..p3c115 | Schedule 5 – Special Allowable Itemized Deductions | frm1701:txtPg3IShed5_1Desc |
| 2 | p3c123..p3c128 | Schedule 5 – Special Allowable Itemized Deductions | frm1701:txtPg3IShed5_4Desc |
| 2 | p3c112..p3c117 | Schedule 5 – Special Allowable Itemized Deductions | frm1701:txtPg3IShed5_1Legal |
| 2 | p3c125..p3c130 | Schedule 5 – Special Allowable Itemized Deductions | frm1701:txtPg3IShed5_4Legal |
| 3 | p3c113..p3c120 | Schedule 5 – Special Allowable Itemized Deductions | frm1701:txtPg3IShed5_1Amt |
| 3 | p3c126..p3c133 | Schedule 5 – Special Allowable Itemized Deductions | frm1701:txtPg3IShed5_4Amt |
| 3 | p3c140..p3c146 | Schedule 6 - Computation of Net Operating Loss Carry Over (NOLCO) | frm1701:txtPg3IShed6_1A |
| 3 | p3c141..p3c147 | Schedule 6 - Computation of Net Operating Loss Carry Over (NOLCO) | frm1701:txtPg3IShed6_1B |
| 1 | p4c5..p4c5 | (header) | derived:tin_digits |
| 1 | p4c6..p4c6 | (header) | frm1701:txtPg4TaxpayerName |
| 5 | p4c50..p4c62 | PART VI – Summary of Income Tax Due | frm1701:txtPg4ISc6_1A |
| 5 | p4c51..p4c63 | PART VI – Summary of Income Tax Due | frm1701:txtPg4ISc6_1B |
| 10 | p4c66..p4c93 | PART VII - Tax Credits/Payments | frm1701:txtPg4IPart7_1A |
| 10 | p4c67..p4c94 | PART VII - Tax Credits/Payments | frm1701:txtPg4IPart7_1B |
| 10 | p4c98..p4c126 | PART VIII – Tax Relief Availment | frm1701:txtPg4IPart8_1A |
| 10 | p4c99..p4c127 | PART VIII – Tax Relief Availment | frm1701:txtPg4IPart8_1B |
| 11 | p4c133..p4c180 | PART IX – Reconciliation of Net Income per Books Against Taxable Income | frm1701:txtPg4IPart9_1A |
| 11 | p4c134..p4c181 | PART IX – Reconciliation of Net Income per Books Against Taxable Income | frm1701:txtPg4IPart9_1B |

## 1701a-2018: 22 groups, 16 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c22..p1c22 | Part I – Background Information on Taxpayer/Filer |  |
| 1 | p1c32..p1c32 | Part I – Background Information on Taxpayer/Filer | frm1701A:txtTaxpayerName |
| 2 | p1c34..p1c35 | Part I – Background Information on Taxpayer/Filer | derived:address_line1 |
| 1 | p1c47..p1c47 | Part I – Background Information on Taxpayer/Filer | frm1701A:txtCitizenship |
| 1 | p1c54..p1c54 | Part I – Background Information on Taxpayer/Filer | frm1701A:txtTelNum |
| 1 | p1c43..p1c43 | Part I – Background Information on Taxpayer/Filer | txtEmail |
| 1 | p1c49..p1c49 | Part I – Background Information on Taxpayer/Filer | frm1701A:txtForeignTaxNumber |
| 10 | p1c74..p1c101 | Part II – Total Tax Payable | derived:amount_20A |
| 10 | p1c75..p1c102 | Part II – Total Tax Payable | derived:amount_20B |
| 1 | p1c104..p1c104 | Part II – Total Tax Payable | derived:amount_30 |
| 3 | p1c124..p1c145 | Part III - Details of Payment |  |
| 4 | p1c125..p1c146 | Part III - Details of Payment |  |
| 4 | p1c129..p1c150 | Part III - Details of Payment |  |
| 1 | p1c144..p1c144 | Part III - Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1701A:txtPg2TaxpayerName |
| 30 | p2c13..p2c112 | Part IV – Computation of Income Tax | derived:amount_36A |
| 30 | p2c14..p2c113 | Part IV – Computation of Income Tax | derived:amount_36B |
| 1 | p2c124..p2c124 | Part V – Background Information on Spouse |  |
| 1 | p2c134..p2c134 | Part V – Background Information on Spouse | frm1701A:txtSpouseName |
| 1 | p2c136..p2c136 | Part V – Background Information on Spouse | frm1701A:txtSpouseTelNum |
| 2 | p2c138..p2c141 | Part V – Background Information on Spouse | frm1701A:txtSpouseCitizenship |

## 1701ms-2024: 18 groups, 14 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 14 | p1c99..p1c138 | PART II – Total Income Tax Payable | derived:tp_p2_19 |
| 14 | p1c100..p1c139 | PART II – Total Income Tax Payable | derived:sp_p2_19 |
| 1 | p1c141..p1c141 | PART II – Total Income Tax Payable | derived:aggregate |
| 1 | p1c147..p1c147 | PART II – Total Income Tax Payable | derived:refund_amount |
| 1 | p1c148..p1c148 | PART II – Total Income Tax Payable | derived:tcc_amount |
| 1 | p1c149..p1c149 | PART II – Total Income Tax Payable | derived:carry_over_amount |
| 3 | p1c164..p1c185 | PART III - Details of Payment |  |
| 4 | p1c165..p1c186 | PART III - Details of Payment |  |
| 4 | p1c169..p1c190 | PART III - Details of Payment |  |
| 1 | p1c184..p1c184 | PART III - Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | derived:page2_name |
| 28 | p2c13..p2c110 | PART IV – Computation of Tax | derived:tp_p4_1 |
| 28 | p2c14..p2c111 | PART IV – Computation of Tax | derived:sp_p4_1 |
| 10 | p2c114..p2c141 | PART V – Tax Credits/Payments | derived:tp_p5_1 |
| 10 | p2c115..p2c142 | PART V – Tax Credits/Payments | derived:sp_p5_1 |
| 1 | p2c145..p2c145 | PART VI – Tax Relief Availment | derived:tp_p6_1 |
| 1 | p2c146..p2c146 | PART VI – Tax Relief Availment | derived:sp_p6_1 |

## 1701q-2018: 27 groups, 25 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c22..p1c22 | PART I – BACKGROUND INFORMATION ON TAXPAYER/FILER |  |
| 1 | p1c39..p1c39 | PART I – BACKGROUND INFORMATION ON TAXPAYER/FILER | frm1701q:txtTaxpayerName |
| 2 | p1c41..p1c42 | PART I – BACKGROUND INFORMATION ON TAXPAYER/FILER | frm1701q:txtAddress |
| 1 | p1c54..p1c54 | PART I – BACKGROUND INFORMATION ON TAXPAYER/FILER | frm1701q:txtCitizenship |
| 1 | p1c50..p1c50 | PART I – BACKGROUND INFORMATION ON TAXPAYER/FILER | txtEmail |
| 1 | p1c55..p1c55 | PART I – BACKGROUND INFORMATION ON TAXPAYER/FILER | frm1701q:txtForeignTaxNumber |
| 1 | p1c72..p1c72 | PART II – BACKGROUND INFORMATION ON SPOUSE |  |
| 1 | p1c89..p1c89 | PART II – BACKGROUND INFORMATION ON SPOUSE | frm1701q:txtSpouseName |
| 1 | p1c93..p1c93 | PART II – BACKGROUND INFORMATION ON SPOUSE | frm1701q:txtSpouseCitizenship |
| 1 | p1c94..p1c94 | PART II – BACKGROUND INFORMATION ON SPOUSE | frm1701q:txtSpouseForeignTaxNum |
| 5 | p1c108..p1c120 | PART III – TOTAL TAX PAYABLE | frm1701q:txt26A |
| 5 | p1c109..p1c121 | PART III – TOTAL TAX PAYABLE | frm1701q:txt26B |
| 1 | p1c123..p1c123 | PART III – TOTAL TAX PAYABLE | frm1701q:txt31 |
| 3 | p1c134..p1c155 | PART IV – DETAILS OF PAYMENT | frm1701q:txtAgency32 |
| 4 | p1c135..p1c156 | PART IV – DETAILS OF PAYMENT | frm1701q:txtNumber32 |
| 4 | p1c139..p1c160 | PART IV – DETAILS OF PAYMENT | frm1701q:txtAmount32 |
| 1 | p1c154..p1c154 | PART IV – DETAILS OF PAYMENT | frm1701q:txtParticular35 |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | derived:page2_name |
| 11 | p2c14..p2c46 | Schedule I | frm1701q:txt36A |
| 11 | p2c15..p2c47 | Schedule I | frm1701q:txt36B |
| 8 | p2c50..p2c71 | Schedule II | frm1701q:txt47A |
| 8 | p2c51..p2c72 | Schedule II | frm1701q:txt47B |
| 9 | p2c75..p2c99 | Schedule III | frm1701q:txt55A |
| 9 | p2c76..p2c100 | Schedule III | frm1701q:txt55B |
| 5 | p2c103..p2c115 | Schedule IV | frm1701q:txt64A |
| 5 | p2c104..p2c116 | Schedule IV | frm1701q:txt64B |

## 1702ex-2018: 28 groups, 23 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c31..p1c31 | Part I – Background Information |  |
| 3 | p1c36..p1c38 | Part I – Background Information | derived:name_line1 |
| 3 | p1c40..p1c42 | Part I – Background Information | derived:address_line1 |
| 1 | p1c60..p1c60 | Part I – Background Information | frm1702EX:txtPg1Pt1I14LegalBasis |
| 1 | p1c64..p1c64 | Part I – Background Information | frm1702EX:txtPg1Pt1I16RegisteredActivity |
| 1 | p1c52..p1c52 | Part I – Background Information | frm1702EX:txtPg1Pt1I11ContactNumber |
| 1 | p1c54..p1c54 | Part I – Background Information | frm1702EX:txtPg1Pt1I12Email |
| 1 | p1c61..p1c61 | Part I – Background Information | frm1702EX:txtPg1Pt1I15Investment |
| 5 | p1c75..p1c83 | Part II – Total Tax Payable | derived:amount:txtPg1Pt2I19TotalTaxCrPmt |
| 3 | p1c111..p1c132 | Part III – Details of Payment |  |
| 4 | p1c112..p1c133 | Part III – Details of Payment |  |
| 4 | p1c116..p1c137 | Part III – Details of Payment |  |
| 1 | p1c131..p1c131 | Part III – Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1702EX:txtPg2RegisteredName |
| 23 | p2c9..p2c64 | Part IV – Computation of Tax | derived:amount:txtPg2Pt4I28SalesReceiptsRevFees |
| 2 | p2c54..p2c58 | Part IV – Computation of Tax | frm1702EX:txtPg2Pt4I48OtherTaxCrPmtDesc |
| 3 | p2c67..p2c71 | Part V – Tax Relief Availment | derived:amount:txtPg2Pt5I52RegularIncomeOtherwiseDue |
| 1 | p3c5..p3c5 | (header) | derived:tin_digits_p3 |
| 1 | p3c6..p3c6 | (header) | frm1702EX:txtPg3RegisteredName |
| 26 | p3c10..p3c76 | Schedule 1 – Ordinary Allowable Itemized Deductions | derived:amount:txtPg3Pt6S1I1Ammortization |
| 6 | p3c52..p3c72 | Schedule 1 – Ordinary Allowable Itemized Deductions | frm1702EX:txtPg3Pt6I17OthersDDesc |
| 4 | p3c81..p3c93 | Schedule 2 – Special Allowable Itemized Deductions | frm1702EX:txtPg3Pt6S2I1Description |
| 4 | p3c82..p3c94 | Schedule 2 – Special Allowable Itemized Deductions | frm1702EX:txtPg3Pt6S2I1LegalBasis |
| 5 | p3c83..p3c97 | Schedule 2 – Special Allowable Itemized Deductions | derived:amount:txtPg3Pt6S2I1Amount |
| 10 | p3c100..p3c133 | Schedule 3 – Reconciliation of Net Income per Books Against Taxable Income | derived:amount:txtPg3Pt6S3I1NetIncomePerBook |
| 2 | p3c103..p3c107 | Schedule 3 – Reconciliation of Net Income per Books Against Taxable Income | frm1702EX:txtPg3Pt6S3I2NonDeductExpenseOtherIncomeDesc |
| 4 | p3c114..p3c127 | Schedule 3 – Reconciliation of Net Income per Books Against Taxable Income | frm1702EX:txtPg3Pt6S3I5NonTaxIncomeAndIncomeSubjectToFinTaxDesc |

## 1702mx-2018c: 28 groups, 24 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c31..p1c31 | Part I – Background Information |  |
| 3 | p1c36..p1c38 | Part I – Background Information | frm1702MX:txtPg1Pt1I9RegisteredName |
| 3 | p1c40..p1c42 | Part I – Background Information | frm1702MX:txtPg1Pt1I10RegisteredAddress |
| 1 | p1c52..p1c52 | Part I – Background Information | frm1702MX:txtPg1Pt1I11ContactNumber |
| 1 | p1c54..p1c54 | Part I – Background Information | frm1702MX:txtPg1Pt1I12Email |
| 8 | p1c59..p1c74 | Part II – Total Tax Payable | frm1702MX:txtPg1Pt2I14TotalIncome |
| 3 | p1c102..p1c123 | Part III – Details of Payment | frm1702MX:txtPg1Pt1I26CashC1 |
| 4 | p1c103..p1c124 | Part III – Details of Payment | frm1702MX:txtPg1Pt1I26CashC2 |
| 4 | p1c107..p1c128 | Part III – Details of Payment | frm1702MX:txtPg1Pt1I26CashC4 |
| 1 | p1c122..p1c122 | Part III – Details of Payment | frm1702MX:txtPg1Pt1I29Others |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1702MX:txtPg2RegisteredName |
| 3 | p2c17..p2c25 | Schedule 1 – Basis of Tax Relief | frm1702MX:txtPg2Pt4I31CA |
| 3 | p2c18..p2c26 | Schedule 1 – Basis of Tax Relief | frm1702MX:txtPg2Pt4I31CB |
| 3 | p2c19..p2c27 | Schedule 1 – Basis of Tax Relief | frm1702MX:txtPg2Pt4I31CC |
| 1 | p3c5..p3c5 | (header) | derived:tin_digits |
| 1 | p3c6..p3c6 | (header) | frm1702MX:txtPg3RegisteredName |
| 3 | p3c218..p3c222 | Schedule 7 – Computation of Net Operating Loss Carry Over (NOLCO) for Regular Rate | frm1702MX:txtPg6Sc7I1 |
| 1 | p4c5..p4c5 | (header) | derived:tin_digits |
| 1 | p4c6..p4c6 | (header) | frm1702MX:txtPg4RegisteredName |
| 3 | p4c49..p4c53 | Schedule 8 – | frm1702MX:txtPg4Sc8It1 |
| 3 | p4c101..p4c111 | Schedule 9 – Computation of Minimum Corporate Income Tax (MCIT) | frm1702MX:txtPg7Sc9I1CA |
| 3 | p4c102..p4c112 | Schedule 9 – Computation of Minimum Corporate Income Tax (MCIT) | frm1702MX:txtPg7Sc9I1CB |
| 3 | p4c103..p4c113 | Schedule 9 – Computation of Minimum Corporate Income Tax (MCIT) | frm1702MX:txtPg7Sc9I1CC |
| 3 | p4c120..p4c130 | Schedule 9 – Computation of Minimum Corporate Income Tax (MCIT) |  |
| 3 | p4c121..p4c131 | Schedule 9 – Computation of Minimum Corporate Income Tax (MCIT) |  |
| 4 | p4c122..p4c135 | Schedule 9 – Computation of Minimum Corporate Income Tax (MCIT) | frm1702MX:txtPg7Sc9I4 |
| 3 | p4c123..p4c133 | Schedule 9 – Computation of Minimum Corporate Income Tax (MCIT) |  |

## 1702q-2018: 19 groups, 14 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c32..p1c32 | Part I – Background Information |  |
| 2 | p1c37..p1c38 | Part I – Background Information | derived:name_line1 |
| 2 | p1c40..p1c41 | Part I – Background Information | derived:address_line1 |
| 1 | p1c46..p1c46 | Part I – Background Information | frm1702q:txtTelNum |
| 1 | p1c47..p1c47 | Part I – Background Information | txtEmail |
| 1 | p1c52..p1c52 | Part I – Background Information | frm1702q:txtTxRlfSpcfy |
| 12 | p1c57..p1c79 | Part II – Total Tax Payable | derived:txtTax14 |
| 3 | p1c102..p1c123 | Part III – Details of Payment |  |
| 4 | p1c103..p1c124 | Part III – Details of Payment |  |
| 4 | p1c107..p1c128 | Part III – Details of Payment |  |
| 1 | p1c122..p1c122 | Part III – Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1702q:txtPg2TaxpayerName |
| 10 | p2c12..p2c47 | Schedule 1 – | derived:Sched1:txtTax1A |
| 12 | p2c13..p2c52 | Schedule 1 – | derived:Sched1:txtTax1B |
| 12 | p2c55..p2c80 | Schedule 2 – | derived:Sched2:txtTax1 |
| 5 | p2c83..p2c94 | Schedule 3 | derived:Sched3:txtTax1 |
| 9 | p2c97..p2c118 | Schedule 4 – Tax Credits/Payments | derived:Sched4:txtTax1 |
| 2 | p2c110..p2c114 | Schedule 4 – Tax Credits/Payments | frm1702q:Sched4:txtOthrTxCrdts0 |

## 1702rt-2018c: 40 groups, 39 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c31..p1c31 | Part I – Background Information |  |
| 3 | p1c36..p1c38 | Part I – Background Information | frm1702RT:txtPg1Pt1I8Name1 |
| 3 | p1c40..p1c42 | Part I – Background Information | frm1702RT:txtPg1Pt1I9Address1 |
| 1 | p1c52..p1c52 | Part I – Background Information | frm1702RT:txtPg1Pt1I11Contact |
| 1 | p1c54..p1c54 | Part I – Background Information | frm1702RT:txtPg1Pt1I12Email |
| 8 | p1c60..p1c75 | Part II – Total Tax Payable | frm1702RT:txtPg1Pt2I14IncomeTax |
| 3 | p1c103..p1c124 | Part III – Details of Payment | frm1702RT:txtPg1Pt3I23DebitMemoC1 |
| 4 | p1c104..p1c125 | Part III – Details of Payment | frm1702RT:txtPg1Pt3I23DebitMemoC2 |
| 4 | p1c108..p1c129 | Part III – Details of Payment | frm1702RT:txtPg1Pt3I23DebitMemoC4Amount |
| 1 | p1c123..p1c123 | Part III – Details of Payment | frm1702RT:txtPg1Pt3I26Others |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1702RT:txtPg2RegisteredName |
| 29 | p2c9..p2c76 | Part IV – Computation of Tax | frm1702RT:txtPg2Pt4I27Sales |
| 2 | p2c66..p2c70 | Part IV – Computation of Tax | frm1702RT:txtPg2Pt4I53C1 |
| 3 | p2c79..p2c83 | Part V – Tax Relief Availment | frm1702RT:txtPg2Pt5I57SpecialAllowable |
| 1 | p3c5..p3c5 | (header) | derived:tin_digits |
| 1 | p3c6..p3c6 | (header) | frm1702RT:txtPg3RegisteredName |
| 26 | p3c10..p3c76 | Schedule I – Ordinary Allowable Itemized Deductions | frm1702RT:txtPg3Sc1I1Amortization |
| 6 | p3c52..p3c72 | Schedule I – Ordinary Allowable Itemized Deductions | frm1702RT:txtPg3Sc1I17dC1 |
| 4 | p3c81..p3c93 | Schedule II – Special Allowable Itemized Deductions | frm1702RT:txtPg3Sc2I1C1 |
| 4 | p3c82..p3c94 | Schedule II – Special Allowable Itemized Deductions | frm1702RT:txtPg3Sc2I1C2 |
| 5 | p3c83..p3c97 | Schedule II – Special Allowable Itemized Deductions | frm1702RT:txtPg3Sc2I1C3 |
| 1 | p4c5..p4c5 | (header) | derived:tin_digits |
| 1 | p4c6..p4c6 | (header) | frm1702RT:txtPg4RegisteredName |
| 3 | p4c9..p4c13 | Schedule III – Computation of Net Operating Loss Carry Over (NOLCO) | frm1702RT:txtPg4Sc3I1GrossIncome |
| 4 | p4c25..p4c46 | Schedule III – Computation of Net Operating Loss Carry Over (NOLCO) | frm1702RT:txtPg4Sc3AI4C3 |
| 4 | p4c54..p4c66 | Schedule III – Computation of Net Operating Loss Carry Over (NOLCO) | frm1702RT:txtPg4Sc3AI4C6 |
| 4 | p4c23..p4c44 | Schedule III – Computation of Net Operating Loss Carry Over (NOLCO) | frm1702RT:txtPg4Sc3AI4C2 |
| 4 | p4c52..p4c64 | Schedule III – Computation of Net Operating Loss Carry Over (NOLCO) | frm1702RT:txtPg4Sc3AI4C4 |
| 5 | p4c53..p4c68 | Schedule III – Computation of Net Operating Loss Carry Over (NOLCO) | frm1702RT:txtPg4Sc3AI4C5 |
| 3 | p4c77..p4c87 | Schedule IV – Computation of Minimum Corporate Income Tax (MCIT) | frm1702RT:txtPg4Sc4I1C2 |
| 3 | p4c78..p4c88 | Schedule IV – Computation of Minimum Corporate Income Tax (MCIT) | frm1702RT:txtPg4Sc4I1C3 |
| 3 | p4c79..p4c89 | Schedule IV – Computation of Minimum Corporate Income Tax (MCIT) | frm1702RT:txtPg4Sc4I1C4 |
| 3 | p4c96..p4c106 | Schedule IV – Computation of Minimum Corporate Income Tax (MCIT) | frm1702RT:txtPg4Sc4I1C5 |
| 3 | p4c97..p4c107 | Schedule IV – Computation of Minimum Corporate Income Tax (MCIT) | frm1702RT:txtPg4Sc4I1C6 |
| 4 | p4c98..p4c111 | Schedule IV – Computation of Minimum Corporate Income Tax (MCIT) | frm1702RT:txtPg4Sc4I1C7 |
| 3 | p4c99..p4c109 | Schedule IV – Computation of Minimum Corporate Income Tax (MCIT) | frm1702RT:txtPg4Sc4I1C8 |
| 10 | p4c115..p4c148 | Schedule V – Reconciliation of Net Income per Books Against Taxable Income | frm1702RT:txtPg4Sc5I1NetIncome |
| 2 | p4c118..p4c122 | Schedule V – Reconciliation of Net Income per Books Against Taxable Income | frm1702RT:txtPg4Sc5I2C1 |
| 4 | p4c129..p4c142 | Schedule V – Reconciliation of Net Income per Books Against Taxable Income | frm1702RT:txtPg4Sc5I5C1 |

## 1706-2018: 11 groups, 7 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c77..p1c77 | Part I – Background Information | derived:line:tax_relief_specify |
| 9 | p1c84..p1c122 | Part II – Computation of Tax | frm1706:txtTax |
| 3 | p1c144..p1c162 | Part III – Details  of Payment |  |
| 4 | p1c145..p1c172 | Part III – Details  of Payment |  |
| 4 | p1c149..p1c176 | Part III – Details  of Payment |  |
| 1 | p1c171..p1c171 | Part III – Details  of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:buyer_tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1706:txtBuyerName |
| 5 | p2c81..p2c109 | Schedule 2 – Description of Transaction (Mark one box only) | frm1706:txtSelling |
| 1 | p2c107..p2c107 | Schedule 2 – Description of Transaction (Mark one box only) | frm1706:txtOthers21 |
| 6 | p2c131..p2c152 | Schedule 4 – Determination of Taxable Base | derived:sched4_a |

## 1707-2021: 15 groups, 11 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c217..p1c217 | Part I - Background Information | frm1707:txtPg1I8TaxReliefSpec |
| 12 | p1c62..p1c108 | Part II – Computation of Tax | frm1707:txtPg1P2I10TaxBase |
| 4 | p1c130..p1c158 | Part III – Details of Payment |  |
| 4 | p1c131..p1c159 | Part III – Details of Payment |  |
| 4 | p1c135..p1c163 | Part III – Details of Payment |  |
| 1 | p1c157..p1c157 | Part III – Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1707:txtPg2RegisteredName |
| 6 | p2c10..p2c38 | Schedule 1 – Installment Sale | frm1707:txtPg2P4Sch1I1SellPrice |
| 4 | p2c44..p2c50 | Schedule 2 – Description of Shares of Stocks | frm1707:txtPg2P4Sch2NameOfCorpStock1 |
| 4 | p2c56..p2c74 | Schedule 2 – Description of Shares of Stocks | frm1707:txtPg2P4Sch2NoOfShares1 |
| 4 | p2c57..p2c75 | Schedule 2 – Description of Shares of Stocks | frm1707:txtPg2P4Sch2StockCertNo1 |
| 5 | p2c58..p2c80 | Schedule 2 – Description of Shares of Stocks | frm1707:txtPg2P4Sch2TaxBaseSellPrice1 |
| 4 | p2c87..p2c102 | Schedule 3 – Schedule of Cost and Other Allowable Expenses | frm1707:txtPg2P4Sch3Particulars1 |
| 5 | p2c88..p2c107 | Schedule 3 – Schedule of Cost and Other Allowable Expenses | frm1707:txtPg2P4Sch3Amount1 |

## 1707a-2021: 12 groups, 7 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c27..p1c27 | Part I – Background Information |  |
| 1 | p1c32..p1c32 | Part I – Background Information | frm1707Av2021:txtI8RegisteredName |
| 2 | p1c34..p1c35 | Part I – Background Information | derived:address_line1 |
| 1 | p1c39..p1c39 | Part I – Background Information | derived:contact_email |
| 1 | p1c207..p1c207 | Part I – Background Information | frm1707Av2021:txtI11ASpecify |
| 13 | p1c46..p1c97 | Part II – Computation of Tax | frm1707Av2021:txtI12TotalCapitalGains |
| 4 | p1c119..p1c147 | Part III – Details of Payment |  |
| 4 | p1c120..p1c148 | Part III – Details of Payment |  |
| 4 | p1c124..p1c152 | Part III – Details of Payment |  |
| 1 | p1c146..p1c146 | Part III – Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1707Av2021:txtI8RegisteredNameP2 |

## 1800-2018: 16 groups, 16 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c18..p1c18 | Part I – Taxpayer Information | frm1800v2018:txtBranchCode |
| 1 | p1c22..p1c22 | Part I – Taxpayer Information | frm1800v2018:txtDonorName |
| 2 | p1c24..p1c25 | Part I – Taxpayer Information | frm1800v2018:txtRegAddress |
| 2 | p1c28..p1c29 | Part I – Taxpayer Information | frm1800v2018:txtResAddress |
| 1 | p1c36..p1c36 | Part I – Taxpayer Information | frm1800v2018:txtContact |
| 1 | p1c37..p1c37 | Part I – Taxpayer Information | frm1800v2018:txtEmail |
| 1 | p1c57..p1c57 | Part I – Taxpayer Information | derived:line:pr10:p1c57 |
| 12 | p1c62..p1c112 | Part II – Computation of Tax | frm1800v2018:TotalNetGifts |
| 3 | p1c134..p1c161 | Part III – Details of Payment | frm1800v2018:txtAgency20 |
| 4 | p1c135..p1c162 | Part III – Details of Payment | frm1800v2018:txtNumber20 |
| 4 | p1c139..p1c166 | Part III – Details of Payment | frm1800v2018:txtAmount20 |
| 1 | p1c160..p1c160 | Part III – Details of Payment | frm1800v2018:txtParticular36 |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1800v2018:txtPg2TaxpayerName |
| 14 | p2c9..p2c67 | Part IV – Computation of Tax | frm1800v2018:PersonalProperties |
| 5 | p2c22..p2c42 | Part IV – Computation of Tax | frm1800v2018:txtDeductionTitle28 |

## 1801-2018: 19 groups, 18 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c4..p1c4 | (header) |  |
| 1 | p1c17..p1c17 | Part I – Taxpayer Information | frm1801v2018:branchCode_1 |
| 1 | p1c33..p1c33 | Part I – Taxpayer Information | derived:line:pr10:p1c33 |
| 1 | p1c38..p1c38 | Part I – Taxpayer Information | derived:line:pr10:p1c38 |
| 1 | p1c22..p1c22 | Part I – Taxpayer Information | frm1801v2018:registeredName_1 |
| 2 | p1c24..p1c25 | Part I – Taxpayer Information | frm1801v2018:registeredAddress_1 |
| 2 | p1c30..p1c31 | Part I – Taxpayer Information | frm1801v2018:adminName |
| 1 | p1c32..p1c32 | Part I – Taxpayer Information | frm1801v2018:txtBranchCodeE |
| 1 | p1c35..p1c35 | Part I – Taxpayer Information | frm1801v2018:txtEmail |
| 8 | p1c56..p1c88 | Part II – Total Tax Payable | frm1801v2018:txtTaxableEstate |
| 4 | p1c93..p1c108 | Part II – Total Tax Payable | frm1801v2018:txtPen_Surcharge |
| 1 | p1c112..p1c112 | Part II – Total Tax Payable | frm1801v2018:txtTotPayable |
| 3 | p1c131..p1c158 | Part III – Details of Payment | frm1801v2018:cashBank |
| 4 | p1c132..p1c159 | Part III – Details of Payment | frm1801v2018:cashNum |
| 4 | p1c136..p1c163 | Part III – Details of Payment | frm1801v2018:cashAmt |
| 1 | p1c157..p1c157 | Part III – Details of Payment | frm1801v2018:othersDetails |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm1801v2018:registeredName_2 |
| 15 | p2c15..p2c86 | Part IV – Computation of Tax | frm1801v2018:txtRealProp_C |

## 2000-dst-2018: 25 groups, 23 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c19..p1c19 | Part I – Background Information | frm2000:txtBranchCode |
| 1 | p1c40..p1c40 | Part I – Background Information | frm2000:txtOtherTIN |
| 1 | p1c23..p1c23 | Part I – Background Information | frm2000:txtTaxpayerName |
| 2 | p1c25..p1c26 | Part I – Background Information | frm2000:txtAddress |
| 1 | p1c31..p1c31 | Part I – Background Information | derived:contact_email |
| 2 | p1c37..p1c38 | Part I – Background Information | frm2000:txtOtherName |
| 12 | p1c48..p1c71 | Part II – Computation of Payable | frm2000:txtTax14 |
| 4 | p1c90..p1c106 | Part III – Details of Payment | frm2000:txtAgency20 |
| 4 | p1c91..p1c107 | Part III – Details of Payment | frm2000:txtNumber20 |
| 4 | p1c92..p1c108 | Part III – Details of Payment |  |
| 4 | p1c93..p1c109 | Part III – Details of Payment |  |
| 1 | p1c105..p1c105 | Part III – Details of Payment | frm2000:txtParticular36 |
| 1 | p2c5..p2c5 | (header) | derived:page_header |
| 3 | p2c11..p2c27 | Schedule 1 – Summary of Computation of Taxes Due for the Month | drpATCCode0 |
| 3 | p2c12..p2c28 | Schedule 1 – Summary of Computation of Taxes Due for the Month | frm2000:sched1:txtTaxBase0 |
| 4 | p2c16..p2c36 | Schedule 1 – Summary of Computation of Taxes Due for the Month | frm2000:sched1:txtTaxDue0 |
| 3 | p2c48..p2c66 | Schedule 2 – Summary of Constructive Affixture for the Month | frm2000:sched2:txtReceipt0 |
| 4 | p2c49..p2c71 | Schedule 2 – Summary of Constructive Affixture for the Month | frm2000:sched2:txtAmountPaid0 |
| 3 | p2c83..p2c101 | Schedule 3 – Summary of DST Payments/Purchases for the Month | frm2000:sched3:txtReceipt0 |
| 4 | p2c84..p2c106 | Schedule 3 – Summary of DST Payments/Purchases for the Month | frm2000:sched3:txtAmountPaid0 |
| 3 | p2c117..p2c137 | Schedule 4 – Summary of Remittance from Collection on Sale of Loose Documentary Stamps | frm2000:sched4:txtRCOCode0 |
| 3 | p2c121..p2c141 | Schedule 4 – Summary of Remittance from Collection on Sale of Loose Documentary Stamps | frm2000:sched4:txtBank0 |
| 4 | p2c122..p2c148 | Schedule 4 – Summary of Remittance from Collection on Sale of Loose Documentary Stamps | frm2000:sched4:txtAmountRemitted0 |
| 3 | p2c125..p2c145 | Schedule 4 – Summary of Remittance from Collection on Sale of Loose Documentary Stamps | frm2000:sched4:txtNumberFrom0 |
| 3 | p2c126..p2c146 | Schedule 4 – Summary of Remittance from Collection on Sale of Loose Documentary Stamps | frm2000:sched4:txtNumberTo0 |

## 2000-ot-2018: 16 groups, 12 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c18..p1c18 | Part I – Background Information | derived:tin |
| 1 | p1c31..p1c31 | Part I – Background Information | txtEmail |
| 1 | p1c22..p1c22 | Part I – Background Information | frm2000OT:txtTaxpayerName |
| 2 | p1c24..p1c25 | Part I – Background Information | frm2000OT:txtAddress |
| 1 | p1c30..p1c30 | Part I – Background Information | frm2000OT:txtTelNum |
| 2 | p1c36..p1c37 | Part I – Background Information | frm2000OT:txtOtherName |
| 1 | p1c47..p1c47 | Part I – Background Information | frm2000OT:txtRealLocation |
| 1 | p1c39..p1c39 | Part I – Background Information | frm2000OT:txtOtherTIN |
| 10 | p1c50..p1c90 | Part II – Computation of Tax | frm2000OT:txtTax14 |
| 2 | p1c112..p1c123 | Part III – Details of Payment |  |
| 4 | p1c113..p1c145 | Part III – Details of Payment |  |
| 4 | p1c119..p1c151 | Part III – Details of Payment |  |
| 1 | p1c144..p1c144 | Part III – Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:pg2_tin |
| 1 | p2c6..p2c6 | (header) | frm2000OT:txtPg2TaxpayerName |
| 4 | p2c88..p2c101 | Schedule 2– Determination of Taxable Base for Real Property Sold/Transferred/Donated | frm2000OT:Sched2:txtGrossSellingPrice |

## 2200a-2020: 14 groups, 14 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c22..p1c22 | Part I – Background Information | frm2200Av2020:branchCode |
| 1 | p1c26..p1c26 | Part I – Background Information | frm2200Av2020:registeredName |
| 2 | p1c28..p1c29 | Part I – Background Information | derived:address_line1 |
| 1 | p1c34..p1c34 | Part I – Background Information | derived:contact_email |
| 1 | p1c39..p1c39 | Part I – Background Information | derived:place_of_production |
| 1 | p1c44..p1c44 | Part I – Background Information | derived:place_of_removal |
| 1 | p1c47..p1c47 | Part I – Background Information | frm2200Av2020:treatyY |
| 1 | p1c56..p1c56 | Part II – Manner of Payment | frm2200Av2020:paymentOther |
| 16 | p1c60..p1c90 | Part III – Payments and Application | frm2200Av2020:txtExciseDue |
| 3 | p1c108..p1c114 | Part IV – Details of Payment | derived:part4_cash |
| 1 | p1c111..p1c111 | Part IV – Details of Payment | derived:part4_tdm |
| 1 | p1c113..p1c113 | Part IV – Details of Payment | frm2200Av2020:othersDetails |
| 1 | p2c5..p2c5 | (header) | derived:page_header |
| 1 | p3c5..p3c5 | (header) | derived:page_header |

## 2200an-2018: 20 groups, 20 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c22..p1c22 | Part I – Background Information | frm2200ANv2018:txtBranchCode |
| 1 | p1c26..p1c26 | Part I – Background Information | frm2200ANv2018:txtTaxpayerName |
| 2 | p1c28..p1c29 | Part I – Background Information | frm2200ANv2018:txtAddress |
| 1 | p1c34..p1c34 | Part I – Background Information | frm2200ANv2018:txtTelNum |
| 1 | p1c40..p1c40 | Part I – Background Information | derived:line:item10_region |
| 1 | p1c47..p1c47 | Part I – Background Information | derived:line:item11_region |
| 1 | p1c35..p1c35 | Part I – Background Information | txtEmail |
| 1 | p1c41..p1c41 | Part I – Background Information | derived:line:item10_province |
| 1 | p1c48..p1c48 | Part I – Background Information | derived:line:item11_province |
| 1 | p1c42..p1c42 | Part I – Background Information | derived:line:item10_city |
| 1 | p1c49..p1c49 | Part I – Background Information | derived:line:item11_city |
| 1 | p1c52..p1c52 | Part I – Background Information | frm2200ANv2018:lstSpecialTax |
| 1 | p1c60..p1c60 | Part II – Manner of Payment | frm2200ANv2018:MannerOfPaymentOthers |
| 16 | p1c64..p1c124 | Part III – Payments and Application | frm2200ANv2018:txt16 |
| 4 | p1c145..p1c173 | Part IV – Details of Payment | frm2200ANv2018:txtAgency24 |
| 4 | p1c146..p1c174 | Part IV – Details of Payment | frm2200ANv2018:txtNumber24 |
| 4 | p1c150..p1c178 | Part IV – Details of Payment | frm2200ANv2018:txtAmount24 |
| 1 | p1c172..p1c172 | Part IV – Details of Payment | frm2200ANv2018:txtParticular27 |
| 1 | p2c4..p2c4 | (header) | derived:tin_digits |
| 1 | p2c5..p2c5 | (header) | frm2200ANv2018:txtPage2Taxpayer |

## 2200c-2018: 13 groups, 9 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c12..p1c12 | (header) |  |
| 1 | p1c24..p1c24 | Part I – Background Information | frm2200C:txtPg1TIN4 |
| 1 | p1c28..p1c28 | Part I – Background Information | frm2200C:txtPg1Pt1I7RegisteredName |
| 2 | p1c30..p1c31 | Part I – Background Information | frm2200C:txtPg1Pt1I8RegisteredAddress |
| 1 | p1c36..p1c36 | Part I – Background Information | derived:contact_email |
| 1 | p1c41..p1c41 | Part I – Background Information | derived:place |
| 1 | p1c44..p1c44 | Part I – Background Information | frm2200C:txtPg1I12TaxReliefSpecify |
| 1 | p1c52..p1c52 | Part II – Manner of Payment | frm2200C:txtPg1Pt2MOPOtherDesc |
| 16 | p1c56..p1c86 | Part III – Payments and Application | frm2200C:txtPg1P3I16ExciseTaxDue |
| 3 | p1c104..p1c110 | Part IV – Details of Payment |  |
| 1 | p1c107..p1c107 | Part IV – Details of Payment |  |
| 1 | p1c109..p1c109 | Part IV – Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:page_header |

## 2200m-2018: 20 groups, 16 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c22..p1c22 | Part I – Background Information | frm2200M:txtPg1TIN4 |
| 1 | p1c26..p1c26 | Part I – Background Information | frm2200M:txtPg1Pt1I6RegisteredName |
| 2 | p1c28..p1c29 | Part I – Background Information | frm2200M:txtPg1Pt1I7RegisteredAddress |
| 1 | p1c34..p1c34 | Part I – Background Information | frm2200M:txtPg1Pt1I8ContactNumber |
| 1 | p1c40..p1c40 | Part I – Background Information | derived:line:item10_region |
| 1 | p1c47..p1c47 | Part I – Background Information | derived:line:item11_region |
| 1 | p1c35..p1c35 | Part I – Background Information | frm2200M:txtPg1Pt1I9Email |
| 1 | p1c41..p1c41 | Part I – Background Information | derived:line:item10_province |
| 1 | p1c48..p1c48 | Part I – Background Information | derived:line:item11_province |
| 1 | p1c42..p1c42 | Part I – Background Information | derived:line:item10_city |
| 1 | p1c49..p1c49 | Part I – Background Information | derived:line:item11_city |
| 1 | p1c54..p1c54 | Part I – Background Information | frm2200M:txtPg1I12TaxReliefSpecify |
| 1 | p1c61..p1c61 | Part II – Manner of Payment | frm2200M:txtPg1Pt2MOPOtherDesc |
| 16 | p1c65..p1c125 | Part III – Payments and Application | frm2200M:txtPg1P3I16ExciseTaxDue |
| 4 | p1c146..p1c174 | Part IV – Details of Payment |  |
| 4 | p1c147..p1c175 | Part IV – Details of Payment |  |
| 4 | p1c151..p1c179 | Part IV – Details of Payment |  |
| 1 | p1c173..p1c173 | Part IV – Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm2200M:txtPg2RegisteredName |

## 2200p-2020: 13 groups, 10 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c22..p1c22 | Part I – Background Information | frm2200P:txtPg1TIN4 |
| 1 | p1c26..p1c26 | Part I – Background Information | frm2200P:txtPg1Pt1I6RegisteredName |
| 2 | p1c28..p1c29 | Part I – Background Information | frm2200P:txtPg1Pt1I7RegisteredAddress |
| 1 | p1c34..p1c34 | Part I – Background Information | derived:contact_email |
| 1 | p1c39..p1c39 | Part I – Background Information | derived:place_of_production |
| 1 | p1c44..p1c44 | Part I – Background Information | derived:place_of_removal |
| 1 | p1c47..p1c47 | Part I – Background Information | frm2200P:txtPg1I12TaxReliefSpecify |
| 1 | p1c55..p1c55 | Part II – Manner of Payment | frm2200P:txtPg1Pt2MOPOtherDesc |
| 16 | p1c59..p1c89 | Part III – Payments and Application | frm2200P:txtPg1P3I16ExciseTaxDue |
| 3 | p1c107..p1c113 | Part IV – Details of Payment |  |
| 1 | p1c110..p1c110 | Part IV – Details of Payment |  |
| 1 | p1c112..p1c112 | Part IV – Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:page_header |

## 2200s-2018: 16 groups, 16 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c21..p1c21 | Part I – Background Information | frm2200S:txtBranchCode |
| 1 | p1c25..p1c25 | Part I – Background Information | frm2200S:taxpayerName |
| 2 | p1c27..p1c28 | Part I – Background Information | derived:address_line1 |
| 1 | p1c33..p1c33 | Part I – Background Information | derived:contact_line |
| 1 | p1c35..p1c35 | Part I – Background Information | txtEmail |
| 1 | p1c40..p1c40 | Part I – Background Information | derived:place_of_production |
| 1 | p1c45..p1c45 | Part I – Background Information | derived:place_of_removal |
| 1 | p1c50..p1c50 | Part I – Background Information | derived:line:tax_relief_specify |
| 1 | p1c56..p1c56 | Part II – Manner Of Payment | frm2200S:txtOthMannerofPymnt |
| 16 | p1c60..p1c90 | Part III – Payments and Application | frm2200S:txtTax16 |
| 3 | p1c109..p1c142 | Part IV – Details of Payment | frm2200S:txtAgency33 |
| 4 | p1c110..p1c143 | Part IV – Details of Payment | frm2200S:txtNumber33 |
| 4 | p1c116..p1c149 | Part IV – Details of Payment | frm2200S:txtAmount33 |
| 1 | p1c141..p1c141 | Part IV – Details of Payment | frm2200S:txtParticular36 |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | derived:page2_name |

## 2200t-2020: 23 groups, 22 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c22..p1c22 | Part I – Background Information | frm2200Tv2020:branchCode |
| 1 | p1c26..p1c26 | Part I – Background Information | frm2200Tv2020:registeredName |
| 2 | p1c28..p1c29 | Part I – Background Information | derived:address_line1 |
| 2 | p1c34..p1c1021 | Part I – Background Information | frm2200Tv2020:phoneNumber |
| 2 | p1c39..p1c1023 | Part I – Background Information | derived:line:item10_region |
| 2 | p1c44..p1c1026 | Part I – Background Information | derived:line:item11_region |
| 2 | p1c47..p1c1037 | Part I – Background Information | frm2200Tv2020:treatyY |
| 2 | p1c55..p1c1029 | Part II – Manner of Payment | frm2200Tv2020:paymentOther |
| 32 | p1c59..p1c978 | Part III – Payments and Application | frm2200Tv2020:txtExciseDue |
| 6 | p1c107..p1c1010 | Part IV – Details of Payment | frm2200Tv2020:cashBank |
| 1 | p1c110..p1c110 | Part IV – Details of Payment |  |
| 2 | p1c112..p1c1020 | Part IV – Details of Payment | frm2200Tv2020:othersDetails |
| 4 | p1c981..p1c1011 | Part IV – Details of Payment | frm2200Tv2020:cashNum |
| 4 | p1c985..p1c1015 | Part IV – Details of Payment | frm2200Tv2020:cashAmt |
| 1 | p1c1022..p1c1022 | Part I – Background Information | frm2200Tv2020:txtEmail |
| 1 | p1c1024..p1c1024 | Part I – Background Information | derived:line:item10_province |
| 1 | p1c1027..p1c1027 | Part I – Background Information | derived:line:item11_province |
| 1 | p1c1025..p1c1025 | Part I – Background Information | derived:line:item10_city |
| 1 | p1c1028..p1c1028 | Part I – Background Information | derived:line:item11_city |
| 2 | p2c5..p2c950 | (header) | derived:tin_digits |
| 1 | p2c951..p2c951 | (header) | frm2200Tv2020:registeredName_2 |
| 2 | p3c5..p3c950 | (header) | derived:tin_digits |
| 1 | p3c951..p3c951 | (header) | frm2200Tv2020:registeredName_3 |

## 2550m-2007: 1 groups, 1 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c13..p1c13 | Part I | frm2550m:txtBranchCode |

## 2550q-2024: 19 groups, 14 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c35..p1c35 | Part I – Background Information |  |
| 1 | p1c39..p1c39 | Part I – Background Information | frm2550qv2024:taxpayerName |
| 2 | p1c41..p1c42 | Part I – Background Information | derived:address_line1 |
| 1 | p1c47..p1c47 | Part I – Background Information | frm2550qv2024:taxpayerContactNumber |
| 1 | p1c48..p1c48 | Part I – Background Information | frm2550qv2024:taxpayerEmailAddress |
| 1 | p1c56..p1c56 | Part I – Background Information | frm2550qv2024:specifyInternationalTreaty |
| 12 | p1c61..p1c106 | Part II – Total Tax Payable | frm2550qv2024:excessInputTax |
| 3 | p1c127..p1c154 | Part III – Details of Payment |  |
| 4 | p1c128..p1c155 | Part III – Details of Payment |  |
| 4 | p1c132..p1c159 | Part III – Details of Payment |  |
| 1 | p1c153..p1c153 | Part III – Details of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm2550qv2024:Pg2TaxPayer |
| 4 | p2c12..p2c28 | Part IV – Details of VAT Computation | frm2550qv2024:vatableSales |
| 7 | p2c76..p2c113 | Part IV – Details of VAT Computation | frm2550qv2024:domesticPurchase |
| 5 | p2c15..p2c43 | Part IV – Details of VAT Computation | frm2550qv2024:outputVatSales |
| 6 | p2c49..p2c69 | Part IV – Details of VAT Computation | frm2550qv2024:inputTaxCarried |
| 6 | p2c79..p2c120 | Part IV – Details of VAT Computation | frm2550qv2024:domesticInputTax |
| 10 | p2c126..p2c162 | Part IV – Details of VAT Computation | frm2550qv2024:importCapitalInputTax |

## 2551q-2018: 16 groups, 11 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c26..p1c26 | Part I – Background Information |  |
| 1 | p1c30..p1c30 | Part I – Background Information | frm2551Qv2018:registeredName |
| 2 | p1c32..p1c33 | Part I – Background Information | derived:address_line1 |
| 1 | p1c38..p1c38 | Part I – Background Information | frm2551Qv2018:telNo |
| 1 | p1c39..p1c39 | Part I – Background Information | txtEmail |
| 1 | p1c42..p1c42 | Part I – Background Information | derived:line:tax_relief_specify |
| 11 | p1c50..p1c92 | Part II – Total Tax Payable | frm2551Qv2018:txt14 |
| 3 | p1c116..p1c143 | Part III – Details  of Payment |  |
| 4 | p1c117..p1c144 | Part III – Details  of Payment |  |
| 4 | p1c121..p1c148 | Part III – Details  of Payment |  |
| 1 | p1c142..p1c142 | Part III – Details  of Payment |  |
| 1 | p2c5..p2c5 | (header) | derived:tin_digits |
| 1 | p2c6..p2c6 | (header) | frm2551Qv2018:txtPg2TaxpayerName |
| 6 | p2c14..p2c69 | Schedule 1 – Computation of Tax | derived:sched1_atc1 |
| 6 | p2c15..p2c70 | Schedule 1 – Computation of Tax | txtATCAmt1 |
| 7 | p2c20..p2c80 | Schedule 1 – Computation of Tax | txtATCDue1 |

## 2552-2018: 12 groups, 8 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c20..p1c20 | Part I – Background Information | frm2552:txtPg1TIN4 |
| 1 | p1c35..p1c35 | Part I – Background Information | frm2552:txtPg1Pt1I10TaxReliefSpec |
| 1 | p1c24..p1c24 | Part I – Background Information | frm2552:txtPg1Pt1I6Name |
| 2 | p1c26..p1c27 | Part I – Background Information | frm2552:txtPg1Pt1I7RegisteredAddress |
| 1 | p1c31..p1c31 | Part I – Background Information | frm2552:txtPg1Pt1I8ContactNumber |
| 1 | p1c32..p1c32 | Part I – Background Information | frm2552:txtPg1Pt1I9Email |
| 2 | p1c43..p1c44 | Part I – Background Information | frm2552:txtPg1Pt1I12NumOfShare |
| 13 | p1c51..p1c102 | Part II – Computation of Tax | frm2552:txtPg1P2I13TaxDue |
| 4 | p1c126..p1c154 | Part III – Details of Payment |  |
| 4 | p1c127..p1c155 | Part III – Details of Payment |  |
| 4 | p1c131..p1c159 | Part III – Details of Payment |  |
| 1 | p1c153..p1c153 | Part III – Details of Payment |  |

## 2553-1999: 2 groups, 2 with amount joins

| rows | cells | section | amount writer keys |
|---|---|---|---|
| 1 | p1c24..p1c24 | Part I | frm2553:txtBranchCode |
| 1 | p1c27..p1c27 | Part I | frm2553:txtTelNum |

