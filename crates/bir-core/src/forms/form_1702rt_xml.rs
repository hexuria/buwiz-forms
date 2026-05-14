//! BIR field mapping for Form 1702RTv2018C.
//!
//! Auto-generated from savefile: 00000000000000-1702RTv2018C-122025.xml
//! Maps Rust struct fields to BIR pseudo-XML field IDs.

use super::form_1702rt::Form1702RTDraft;

use std::collections::BTreeMap;

impl Form1702RTDraft {
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let (tin1, tin2, tin3, _branch) = split_tin(&self.tin);

        // === Common fields (all forms) ===
        insert(&mut fields, "driveSelectTPExport", "0");
        insert(&mut fields, "ebirOnlineConfirmUsername", "");
        insert(&mut fields, "ebirOnlineSecret", "");
        insert(&mut fields, "ebirOnlineUsername", "");
        insert(&mut fields, "txtEnroll", "Y");
        insert(&mut fields, "txtFinalFlag", "1");

        // === Form-specific fields ===
        insert(&mut fields, "BranchMaskP1", self.branch_mask_p1.to_string());
        insert(
            &mut fields,
            "frm1702RT:Pg2Pt4I40IncomeTaxRate",
            self.pg2pt4i40income_tax_rate.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:ddlPg1I2Month",
            format!("{:02}", self.month),
        );
        insert(
            &mut fields,
            "frm1702RT:drpPg1I5AtcOther",
            self.drp_pg1i5atc_other.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:drpPg1Pt1I7RDOCode",
            self.rdo_code.clone(),
        );
        insert_bool(
            &mut fields,
            "frm1702RT:rdoPg1I1Calendar",
            self.rdo_pg1i1calendar,
        );
        insert_bool(
            &mut fields,
            "frm1702RT:rdoPg1I1Fiscal",
            self.rdo_pg1i1fiscal,
        );
        insert_bool(
            &mut fields,
            "frm1702RT:rdoPg1I3AmmendNo",
            self.rdo_pg1i3ammend_no,
        );
        insert_bool(
            &mut fields,
            "frm1702RT:rdoPg1I3AmmendYes",
            self.rdo_pg1i3ammend_yes,
        );
        insert_bool(
            &mut fields,
            "frm1702RT:rdoPg1I4ShortPeriodNo",
            self.rdo_pg1i4short_period_no,
        );
        insert_bool(
            &mut fields,
            "frm1702RT:rdoPg1I4ShortPeriodYes",
            self.rdo_pg1i4short_period_yes,
        );
        insert_bool(&mut fields, "frm1702RT:rdoPg1I5Atc", self.rdo_pg1i5atc);
        insert_bool(
            &mut fields,
            "frm1702RT:rdoPg1I5AtcOther",
            self.rdo_pg1i5atc_other,
        );
        insert_bool(
            &mut fields,
            "frm1702RT:rdoPg1Pt1I13ItemizedDeduction",
            self.rdo_pg1pt1i13itemized_deduction,
        );
        insert_bool(
            &mut fields,
            "frm1702RT:rdoPg1Pt1I13OptionalStandard",
            self.rdo_pg1pt1i13optional_standard,
        );
        insert_bool(
            &mut fields,
            "frm1702RT:rdoPg1Pt2I21OverpaymentCarried",
            self.rdo_pg1pt2i21overpayment_carried,
        );
        insert_bool(
            &mut fields,
            "frm1702RT:rdoPg1Pt2I21OverpaymentIssued",
            self.rdo_pg1pt2i21overpayment_issued,
        );
        insert_bool(
            &mut fields,
            "frm1702RT:rdoPg1Pt2I21OverpaymentRefunded",
            self.rdo_pg1pt2i21overpayment_refunded,
        );
        insert(
            &mut fields,
            "frm1702RT:txtCurrentPage",
            self.txt_current_page.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtMaxPage",
            self.txt_max_page.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1I2Year",
            self.taxable_year.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt1I10",
            self.txt_pg1pt1i10.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt1I11Contact",
            self.txt_pg1pt1i11contact.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt1I12Email",
            self.txt_pg1pt1i12email.clone(),
        );
        insert(&mut fields, "frm1702RT:txtPg1Pt1I6TIN1", tin1.clone());
        insert(&mut fields, "frm1702RT:txtPg1Pt1I6TIN2", tin2.clone());
        insert(&mut fields, "frm1702RT:txtPg1Pt1I6TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt1I6TIN4",
            self.txt_pg1pt1i6tin4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt1I8Name1",
            self.txt_pg1pt1i8name1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt1I8Name2",
            self.txt_pg1pt1i8name2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt1I8Name3",
            self.txt_pg1pt1i8name3.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt1I9Address1",
            self.txt_pg1pt1i9address1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt1I9Address2",
            self.txt_pg1pt1i9address2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt1I9Address3",
            self.txt_pg1pt1i9address3.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2I14IncomeTax",
            self.txt_pg1pt2i14income_tax.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2I15TotalTaxCredits",
            self.txt_pg1pt2i15total_tax_credits.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2I16NetTax",
            self.txt_pg1pt2i16net_tax.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2I17Surcharge",
            self.txt_pg1pt2i17surcharge.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2I18Interest",
            self.txt_pg1pt2i18interest.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2I19Compromise",
            self.txt_pg1pt2i19compromise.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2I20TotalPenalties",
            self.txt_pg1pt2i20total_penalties.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2I21TotalAmount",
            self.txt_pg1pt2i21total_amount.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2PagesFilled",
            self.txt_pg1pt2pages_filled.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2Signatory1",
            self.txt_pg1pt2signatory1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2Signatory2",
            self.txt_pg1pt2signatory2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2SignatoryTin1",
            tin1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt2SignatoryTin2",
            tin2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I23DebitMemoC1",
            self.txt_pg1pt3i23debit_memo_c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I23DebitMemoC2",
            self.txt_pg1pt3i23debit_memo_c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I23DebitMemoC3Date",
            self.txt_pg1pt3i23debit_memo_c3date.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I23DebitMemoC4Amount",
            self.txt_pg1pt3i23debit_memo_c4amount.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I24CheckC1",
            self.txt_pg1pt3i24check_c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I24CheckC2",
            self.txt_pg1pt3i24check_c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I24CheckC3Date",
            self.txt_pg1pt3i24check_c3date.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I24CheckC4Amount",
            self.txt_pg1pt3i24check_c4amount.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I25TaxDebitC2",
            self.txt_pg1pt3i25tax_debit_c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I25TaxDebitC4Amount",
            self.txt_pg1pt3i25tax_debit_c4amount.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I25TaxDebitDate",
            self.txt_pg1pt3i25tax_debit_date.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I26Others",
            self.txt_pg1pt3i26others.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I26OthersC1",
            self.txt_pg1pt3i26others_c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I26OthersC2",
            self.txt_pg1pt3i26others_c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I26OthersC3Date",
            self.txt_pg1pt3i26others_c3date.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg1Pt3I26OthersC4Amount",
            self.txt_pg1pt3i26others_c4amount.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt452SpecialTaxCredits",
            self.txt_pg2pt452special_tax_credits.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I27Sales",
            self.txt_pg2pt4i27sales.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I28LessSales",
            self.txt_pg2pt4i28less_sales.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I29NetSales",
            self.txt_pg2pt4i29net_sales.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I30LessCost",
            self.txt_pg2pt4i30less_cost.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I31GrossIncome",
            self.txt_pg2pt4i31gross_income.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I32AddOtherTaxable",
            self.txt_pg2pt4i32add_other_taxable.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I33TotalGross",
            self.txt_pg2pt4i33total_gross.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I34OrdinaryAllowable",
            self.txt_pg2pt4i34ordinary_allowable.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I35SpecialAllowable",
            self.txt_pg2pt4i35special_allowable.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I36Nolco",
            self.txt_pg2pt4i36nolco.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I37TotalItemized",
            self.txt_pg2pt4i37total_itemized.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I38OptionalStandard",
            self.txt_pg2pt4i38optional_standard.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I39NetTaxable",
            self.txt_pg2pt4i39net_taxable.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I41IncomeTaxDue",
            self.txt_pg2pt4i41income_tax_due.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I42MinimumCorporate",
            self.txt_pg2pt4i42minimum_corporate.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I43TotalIncomeTax",
            self.txt_pg2pt4i43total_income_tax.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I44ExcessCredits",
            self.txt_pg2pt4i44excess_credits.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I45IncomeTaxPaymentUnderMCIT",
            self.txt_pg2pt4i45income_tax_payment_under_mcit.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I46IncomeTaxUnderRegular",
            self.txt_pg2pt4i46income_tax_under_regular.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I47ExcessMCIT",
            self.txt_pg2pt4i47excess_mcit.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I48CreditableTaxWithheldFromPrevious",
            self.txt_pg2pt4i48creditable_tax_withheld_from_previous
                .clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I49CreditableTaxWithheldFor4thQuarter",
            self.txt_pg2pt4i49creditable_tax_withheld_for4th_quarter
                .clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I50ForeignTaxCredits",
            self.txt_pg2pt4i50foreign_tax_credits.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I51TaxPaidInReturn",
            self.txt_pg2pt4i51tax_paid_in_return.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I53C1",
            self.txt_pg2pt4i53c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I53C2",
            self.txt_pg2pt4i53c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I54C1",
            self.txt_pg2pt4i54c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I54C2",
            self.txt_pg2pt4i54c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I54CtrModal",
            self.txt_pg2pt4i54ctr_modal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I54Subtotal",
            self.txt_pg2pt4i54subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I55TotalTaxCredits",
            self.txt_pg2pt4i55total_tax_credits.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt4I56NetTax",
            self.txt_pg2pt4i56net_tax.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt5I57SpecialAllowable",
            self.txt_pg2pt5i57special_allowable.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt5I58AddSpecialTax",
            self.txt_pg2pt5i58add_special_tax.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2Pt5I59TotalTax",
            self.txt_pg2pt5i59total_tax.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg2RegisteredName",
            self.txt_pg2registered_name.clone(),
        );
        insert(&mut fields, "frm1702RT:txtPg2TIN1", tin1.clone());
        insert(&mut fields, "frm1702RT:txtPg2TIN2", tin2.clone());
        insert(&mut fields, "frm1702RT:txtPg2TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1702RT:txtPg2TIN4",
            self.txt_pg2tin4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3RegisteredName",
            self.txt_pg3registered_name.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I10PensionTrust",
            self.txt_pg3sc1i10pension_trust.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I11Rental",
            self.txt_pg3sc1i11rental.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I12Research",
            self.txt_pg3sc1i12research.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I13Salaries",
            self.txt_pg3sc1i13salaries.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I14Contributions",
            self.txt_pg3sc1i14contributions.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I15TaxesandLicenses",
            self.txt_pg3sc1i15taxesand_licenses.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I16TransportationandTravel",
            self.txt_pg3sc1i16transportationand_travel.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17aJanitorial",
            self.txt_pg3sc1i17a_janitorial.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17bProfessionalFees",
            self.txt_pg3sc1i17b_professional_fees.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17cSecurityServices",
            self.txt_pg3sc1i17c_security_services.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17dC1",
            self.txt_pg3sc1i17d_c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17dC2",
            self.txt_pg3sc1i17d_c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17eC1",
            self.txt_pg3sc1i17e_c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17eC2",
            self.txt_pg3sc1i17e_c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17fC1",
            self.txt_pg3sc1i17f_c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17fC2",
            self.txt_pg3sc1i17f_c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17gC1",
            self.txt_pg3sc1i17g_c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17gC2",
            self.txt_pg3sc1i17g_c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17hC1",
            self.txt_pg3sc1i17h_c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17hC2",
            self.txt_pg3sc1i17h_c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17iC1",
            self.txt_pg3sc1i17i_c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17iC2",
            self.txt_pg3sc1i17i_c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17iCtrModal",
            self.txt_pg3sc1i17i_ctr_modal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I17iSubtotal",
            self.txt_pg3sc1i17i_subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I18TotalOrdinaryAllowable",
            self.txt_pg3sc1i18total_ordinary_allowable.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I1Amortization",
            self.txt_pg3sc1i1amortization.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I2BadDebts",
            self.txt_pg3sc1i2bad_debts.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I3CharitableContributions",
            self.txt_pg3sc1i3charitable_contributions.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I4Depletion",
            self.txt_pg3sc1i4depletion.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I5Depreciation",
            self.txt_pg3sc1i5depreciation.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I6Entertainment",
            self.txt_pg3sc1i6entertainment.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I7FringeBenefits",
            self.txt_pg3sc1i7fringe_benefits.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I8Interest",
            self.txt_pg3sc1i8interest.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc1I9Losses",
            self.txt_pg3sc1i9losses.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I1C1",
            self.txt_pg3sc2i1c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I1C2",
            self.txt_pg3sc2i1c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I1C3",
            self.txt_pg3sc2i1c3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I2C1",
            self.txt_pg3sc2i2c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I2C2",
            self.txt_pg3sc2i2c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I2C3",
            self.txt_pg3sc2i2c3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I3C1",
            self.txt_pg3sc2i3c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I3C2",
            self.txt_pg3sc2i3c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I3C3",
            self.txt_pg3sc2i3c3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I4C1",
            self.txt_pg3sc2i4c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I4C2",
            self.txt_pg3sc2i4c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I4C3",
            self.txt_pg3sc2i4c3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I4CtrModal",
            self.txt_pg3sc2i4ctr_modal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I4Subtotal",
            self.txt_pg3sc2i4subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg3Sc2I5TotalSpecialAllowable",
            self.txt_pg3sc2i5total_special_allowable.to_string(),
        );
        insert(&mut fields, "frm1702RT:txtPg3TIN1", tin1.clone());
        insert(&mut fields, "frm1702RT:txtPg3TIN2", tin2.clone());
        insert(&mut fields, "frm1702RT:txtPg3TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1702RT:txtPg3TIN4",
            self.txt_pg3tin4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4RegisteredName",
            self.txt_pg4registered_name.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI4C1",
            self.txt_pg4sc3ai4c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI4C2",
            self.txt_pg4sc3ai4c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI4C3",
            self.txt_pg4sc3ai4c3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI4C4",
            self.txt_pg4sc3ai4c4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI4C5",
            self.txt_pg4sc3ai4c5.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI4C6",
            self.txt_pg4sc3ai4c6.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI5C1",
            self.txt_pg4sc3ai5c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI5C2",
            self.txt_pg4sc3ai5c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI5C3",
            self.txt_pg4sc3ai5c3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI5C4",
            self.txt_pg4sc3ai5c4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI5C5",
            self.txt_pg4sc3ai5c5.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI5C6",
            self.txt_pg4sc3ai5c6.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI6C1",
            self.txt_pg4sc3ai6c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI6C2",
            self.txt_pg4sc3ai6c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI6C3",
            self.txt_pg4sc3ai6c3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI6C4",
            self.txt_pg4sc3ai6c4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI6C5",
            self.txt_pg4sc3ai6c5.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI6C6",
            self.txt_pg4sc3ai6c6.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI7C1",
            self.txt_pg4sc3ai7c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI7C2",
            self.txt_pg4sc3ai7c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI7C2Subtotal",
            self.txt_pg4sc3ai7c2subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI7C3",
            self.txt_pg4sc3ai7c3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI7C3Subtotal",
            self.txt_pg4sc3ai7c3subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI7C4",
            self.txt_pg4sc3ai7c4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI7C4Subtotal",
            self.txt_pg4sc3ai7c4subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI7C5",
            self.txt_pg4sc3ai7c5.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI7C5Subtotal",
            self.txt_pg4sc3ai7c5subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI7C6",
            self.txt_pg4sc3ai7c6.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3AI7C6Subtotal",
            self.txt_pg4sc3ai7c6subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3I1GrossIncome",
            self.txt_pg4sc3i1gross_income.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3I2TotalDeductions",
            self.txt_pg4sc3i2total_deductions.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3I3NetOperatingLoss",
            self.txt_pg4sc3i3net_operating_loss.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc3I3Subtotal",
            self.txt_pg4sc3i3subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I1C1",
            self.txt_pg4sc4i1c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I1C2",
            self.txt_pg4sc4i1c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I1C3",
            self.txt_pg4sc4i1c3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I1C4",
            self.txt_pg4sc4i1c4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I1C5",
            self.txt_pg4sc4i1c5.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I1C6",
            self.txt_pg4sc4i1c6.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I1C7",
            self.txt_pg4sc4i1c7.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I1C8",
            self.txt_pg4sc4i1c8.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I2C1",
            self.txt_pg4sc4i2c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I2C2",
            self.txt_pg4sc4i2c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I2C3",
            self.txt_pg4sc4i2c3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I2C4",
            self.txt_pg4sc4i2c4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I2C5",
            self.txt_pg4sc4i2c5.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I2C6",
            self.txt_pg4sc4i2c6.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I2C7",
            self.txt_pg4sc4i2c7.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I2C8",
            self.txt_pg4sc4i2c8.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I3C1",
            self.txt_pg4sc4i3c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I3C2",
            self.txt_pg4sc4i3c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I3C3",
            self.txt_pg4sc4i3c3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I3C4",
            self.txt_pg4sc4i3c4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I3C5",
            self.txt_pg4sc4i3c5.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I3C6",
            self.txt_pg4sc4i3c6.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I3C7",
            self.txt_pg4sc4i3c7.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I3C8",
            self.txt_pg4sc4i3c8.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I4Subtotal",
            self.txt_pg4sc4i4subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I4TotalExcessMCIT",
            self.txt_pg4sc4i4total_excess_mcit.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc4I8TotalNOLCO",
            self.txt_pg4sc4i8total_nolco.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I10NetTaxableIncome",
            self.txt_pg4sc5i10net_taxable_income.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I1NetIncome",
            self.txt_pg4sc5i1net_income.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I2C1",
            self.txt_pg4sc5i2c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I2C2",
            self.txt_pg4sc5i2c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I3C1",
            self.txt_pg4sc5i3c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I3C2",
            self.txt_pg4sc5i3c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I3CtrModal",
            self.txt_pg4sc5i3ctr_modal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I3Subtotal",
            self.txt_pg4sc5i3subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I4Total",
            self.txt_pg4sc5i4total.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I5C1",
            self.txt_pg4sc5i5c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I5C2",
            self.txt_pg4sc5i5c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I6C1",
            self.txt_pg4sc5i6c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I6C2",
            self.txt_pg4sc5i6c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I6CtrModal",
            self.txt_pg4sc5i6ctr_modal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I6Subtotal",
            self.txt_pg4sc5i6subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I7C1",
            self.txt_pg4sc5i7c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I7C2",
            self.txt_pg4sc5i7c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I8C1",
            self.txt_pg4sc5i8c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I8C2",
            self.txt_pg4sc5i8c2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I8CtrModal",
            self.txt_pg4sc5i8ctr_modal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I8Subtotal",
            self.txt_pg4sc5i8subtotal.to_string(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtPg4Sc5I9Total",
            self.txt_pg4sc5i9total.to_string(),
        );
        insert(&mut fields, "frm1702RT:txtPg4TIN1", tin1.clone());
        insert(&mut fields, "frm1702RT:txtPg4TIN2", tin2.clone());
        insert(&mut fields, "frm1702RT:txtPg4TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1702RT:txtPg4TIN4",
            self.txt_pg4tin4.to_string(),
        );
        insert(&mut fields, "frm1702RT:txtRDO", self.txt_rdo.to_string());
        insert(
            &mut fields,
            "frm1702RT:txtSignaturePresident",
            self.txt_signature_president.clone(),
        );
        insert(
            &mut fields,
            "frm1702RT:txtSignatureTreasurer",
            self.txt_signature_treasurer.clone(),
        );
        insert(&mut fields, "frm1702RT:txtZIP", self.txt_zip.to_string());
        insert(
            &mut fields,
            "txtBranchMaskP2",
            self.txt_branch_mask_p2.to_string(),
        );
        insert(
            &mut fields,
            "txtBranchMaskP3",
            self.txt_branch_mask_p3.to_string(),
        );
        insert(
            &mut fields,
            "txtBranchMaskP4",
            self.txt_branch_mask_p4.to_string(),
        );

        fields
    }

    pub fn to_bir_xml_payload(&self) -> String {
        crate::bir_xml::generate_bir_xml(&self.to_bir_field_map())
    }
}

fn split_tin(tin: &str) -> (String, String, String, String) {
    let digits: String = tin.chars().filter(|ch| ch.is_ascii_digit()).collect();
    let segment = |start: usize, end: usize| -> String {
        digits
            .get(start..end.min(digits.len()))
            .unwrap_or("")
            .to_string()
    };
    (
        segment(0, 3),
        segment(3, 6),
        segment(6, 9),
        digits
            .get(9..)
            .filter(|s| !s.is_empty())
            .unwrap_or("000")
            .to_string(),
    )
}

fn insert(map: &mut BTreeMap<String, String>, key: &str, value: impl Into<String>) {
    map.insert(key.to_string(), value.into());
}

fn insert_bool(map: &mut BTreeMap<String, String>, key: &str, value: bool) {
    insert(map, key, if value { "true" } else { "false" });
}
