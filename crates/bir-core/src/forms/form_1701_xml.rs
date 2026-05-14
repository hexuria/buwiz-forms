//! BIR field mapping for Form 1701v2018.
//!
//! Auto-generated from savefile: 00000000000000-1701v2018-122025.xml
//! Maps Rust struct fields to BIR pseudo-XML field IDs.

use super::form_1701::Form1701Draft;

use std::collections::BTreeMap;

impl Form1701Draft {
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let (tin1, tin2, tin3, branch) = split_tin(&self.tin);

        // === Common fields (all forms) ===
        insert(&mut fields, "attachmentCurrent", "1");
        insert(&mut fields, "attachmentTotal", "0");
        insert(&mut fields, "driveSelectTPExport", "0");
        insert(&mut fields, "ebirOnlineConfirmUsername", "");
        insert(&mut fields, "ebirOnlineSecret", "");
        insert(&mut fields, "ebirOnlineUsername", "");
        insert(&mut fields, "txtEmail", self.email.clone());
        insert(&mut fields, "txtEnroll", "Y");
        insert(&mut fields, "txtFinalFlag", "1");

        // === Form-specific fields ===
        insert_bool(
            &mut fields,
            "frm1701:chkPg2IShed1a_1Spouse",
            self.chk_pg2ished1a_1spouse,
        );
        insert_bool(
            &mut fields,
            "frm1701:chkPg2IShed1a_1Taxpayer",
            self.chk_pg2ished1a_1taxpayer,
        );
        insert_bool(
            &mut fields,
            "frm1701:chkPg2IShed2a_2Spouse",
            self.chk_pg2ished2a_2spouse,
        );
        insert_bool(
            &mut fields,
            "frm1701:chkPg2IShed2a_2Taxpayer",
            self.chk_pg2ished2a_2taxpayer,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoEXAttachmentS",
            self.rdo_exattachment_s,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoEXAttachmentTF",
            self.rdo_exattachment_tf,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I13ForeignTaxCreditsNo",
            self.rdo_pg1i13foreign_tax_credits_no,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I13ForeignTaxCreditsYes",
            self.rdo_pg1i13foreign_tax_credits_yes,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I16CivilStatusLS",
            self.rdo_pg1i16civil_status_ls,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I16CivilStatusM",
            self.rdo_pg1i16civil_status_m,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I16CivilStatusS",
            self.rdo_pg1i16civil_status_s,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I16CivilStatusW",
            self.rdo_pg1i16civil_status_w,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I17SpouseIncomeNo",
            self.rdo_pg1i17spouse_income_no,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I17SpouseIncomeYes",
            self.rdo_pg1i17spouse_income_yes,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I18FilingStatusJ",
            self.rdo_pg1i18filing_status_j,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I18FilingStatusS",
            self.rdo_pg1i18filing_status_s,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I19IncomeExemptNo",
            self.rdo_pg1i19income_exempt_no,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I19IncomeExemptYes",
            self.rdo_pg1i19income_exempt_yes,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I20IncomeSpecialNo",
            self.rdo_pg1i20income_special_no,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I20IncomeSpecialYes",
            self.rdo_pg1i20income_special_yes,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I21AMethodDeductionI",
            self.rdo_pg1i21amethod_deduction_i,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I21AMethodDeductionO",
            self.rdo_pg1i21amethod_deduction_o,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I21TaxRateG",
            self.rdo_pg1i21tax_rate_g,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I21TaxRateP",
            self.rdo_pg1i21tax_rate_p,
        );
        insert_bool(&mut fields, "frm1701:rdoPg1I2AmendedNo", self.is_amended);
        insert_bool(&mut fields, "frm1701:rdoPg1I2AmendedYes", self.is_amended);
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I3ShortPeriodNo",
            self.rdo_pg1i3short_period_no,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I3ShortPeriodYes",
            self.rdo_pg1i3short_period_yes,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I6TaxpayerTypeC",
            self.rdo_pg1i6taxpayer_type_c,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I6TaxpayerTypeE",
            self.rdo_pg1i6taxpayer_type_e,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I6TaxpayerTypeP",
            self.rdo_pg1i6taxpayer_type_p,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I6TaxpayerTypeS",
            self.rdo_pg1i6taxpayer_type_s,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I6TaxpayerTypeT",
            self.rdo_pg1i6taxpayer_type_t,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I7ATC_II011",
            self.rdo_pg1i7atc_ii011,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I7ATC_II012",
            self.rdo_pg1i7atc_ii012,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I7ATC_II013",
            self.rdo_pg1i7atc_ii013,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I7ATC_II014",
            self.rdo_pg1i7atc_ii014,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I7ATC_II015",
            self.rdo_pg1i7atc_ii015,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I7ATC_II016",
            self.rdo_pg1i7atc_ii016,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1I7ATC_II017",
            self.rdo_pg1i7atc_ii017,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1OverpaymentCarryOver",
            self.rdo_pg1overpayment_carry_over,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1OverpaymentRefund",
            self.rdo_pg1overpayment_refund,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg1OverpaymentTCC",
            self.rdo_pg1overpayment_tcc,
        );
        insert_bool(&mut fields, "frm1701:rdoPg1mOption1", self.rdo_pg1m_option1);
        insert_bool(&mut fields, "frm1701:rdoPg1mOption2", self.rdo_pg1m_option2);
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I10IncomeExemptNo",
            self.rdo_pg2i10income_exempt_no,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I10IncomeExemptYes",
            self.rdo_pg2i10income_exempt_yes,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I11IncomeSpecialNo",
            self.rdo_pg2i11income_special_no,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I11IncomeSpecialYes",
            self.rdo_pg2i11income_special_yes,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I12AMethodDeductionI",
            self.rdo_pg2i12amethod_deduction_i,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I12AMethodDeductionO",
            self.rdo_pg2i12amethod_deduction_o,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I12TaxRateG",
            self.rdo_pg2i12tax_rate_g,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I12TaxRateP",
            self.rdo_pg2i12tax_rate_p,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I3SpouseTypeC",
            self.rdo_pg2i3spouse_type_c,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I3SpouseTypeP",
            self.rdo_pg2i3spouse_type_p,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I3SpouseTypeS",
            self.rdo_pg2i3spouse_type_s,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I4ATC_II011",
            self.rdo_pg2i4atc_ii011,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I4ATC_II012",
            self.rdo_pg2i4atc_ii012,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I4ATC_II013",
            self.rdo_pg2i4atc_ii013,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I4ATC_II014",
            self.rdo_pg2i4atc_ii014,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I4ATC_II015",
            self.rdo_pg2i4atc_ii015,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I4ATC_II016",
            self.rdo_pg2i4atc_ii016,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I4ATC_II017",
            self.rdo_pg2i4atc_ii017,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I8ForeignTaxCreditsNo",
            self.rdo_pg2i8foreign_tax_credits_no,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg2I8ForeignTaxCreditsYes",
            self.rdo_pg2i8foreign_tax_credits_yes,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg3mExemptTYPE",
            self.rdo_pg3m_exempt_type,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoPg3mSpecialRateTYPE",
            self.rdo_pg3m_special_rate_type,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoSPAttachmentS",
            self.rdo_spattachment_s,
        );
        insert_bool(
            &mut fields,
            "frm1701:rdoSPAttachmentTF",
            self.rdo_spattachment_tf,
        );
        insert(
            &mut fields,
            "frm1701:txtAttachmentTypes",
            self.txt_attachment_types.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtCurrentPage",
            self.txt_current_page.to_string(),
        );
        insert(
            &mut fields,
            "frm1701:txtDisabledInputs",
            self.txt_disabled_inputs.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtDisabledOnSave",
            self.txt_disabled_on_save.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtEnabledInputsOnValidation",
            self.txt_enabled_inputs_on_validation.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtEnabledLinks",
            self.txt_enabled_links.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtEnabledOnSave",
            self.txt_enabled_on_save.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtIsSpouseDisabled",
            self.txt_is_spouse_disabled.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtIsTaxFilerDisabled",
            self.txt_is_tax_filer_disabled.clone(),
        );
        insert(&mut fields, "frm1701:txtLineBus", self.txt_line_bus.clone());
        insert(
            &mut fields,
            "frm1701:txtMaxPage",
            self.txt_max_page.to_string(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I10BirthDate",
            self.txt_pg1i10birth_date.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I12Citizenship",
            self.txt_pg1i12citizenship.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I14ForeignTaxNumber",
            self.txt_pg1i14foreign_tax_number.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I15TelNum",
            self.contact_number.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I1Month",
            format!("{:02}", self.month),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I1Year",
            self.taxable_year.to_string(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1I22ATaxDue",
            self.txt_pg1i22atax_due,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1I22BTaxDue",
            self.txt_pg1i22btax_due,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I235Number",
            self.txt_pg1i235number.clone(),
        );
        insert_money(&mut fields, "frm1701:txtPg1I23A", self.txt_pg1i23a);
        insert_money(&mut fields, "frm1701:txtPg1I23B", self.txt_pg1i23b);
        insert_money(
            &mut fields,
            "frm1701:txtPg1I24ATaxPayable",
            self.txt_pg1i24atax_payable,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1I24BTaxPayable",
            self.txt_pg1i24btax_payable,
        );
        insert_money(&mut fields, "frm1701:txtPg1I25A", self.txt_pg1i25a);
        insert_money(&mut fields, "frm1701:txtPg1I25B", self.txt_pg1i25b);
        insert_money(&mut fields, "frm1701:txtPg1I26A", self.txt_pg1i26a);
        insert_money(&mut fields, "frm1701:txtPg1I26B", self.txt_pg1i26b);
        insert_money(&mut fields, "frm1701:txtPg1I27A", self.txt_pg1i27a);
        insert_money(&mut fields, "frm1701:txtPg1I27B", self.txt_pg1i27b);
        insert_money(&mut fields, "frm1701:txtPg1I28A", self.txt_pg1i28a);
        insert_money(&mut fields, "frm1701:txtPg1I28B", self.txt_pg1i28b);
        insert_money(&mut fields, "frm1701:txtPg1I29A", self.txt_pg1i29a);
        insert_money(&mut fields, "frm1701:txtPg1I29B", self.txt_pg1i29b);
        insert_money(&mut fields, "frm1701:txtPg1I30A", self.txt_pg1i30a);
        insert_money(&mut fields, "frm1701:txtPg1I30B", self.txt_pg1i30b);
        insert_money(
            &mut fields,
            "frm1701:txtPg1I31ATotalAmtPyble",
            self.txt_pg1i31atotal_amt_pyble,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1I31BTotalAmtPyble",
            self.txt_pg1i31btotal_amt_pyble,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1I32AggregateAmtPyble",
            self.txt_pg1i32aggregate_amt_pyble,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I33NumberOfAttachments",
            self.txt_pg1i33number_of_attachments.to_string(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I34Agency",
            self.txt_pg1i34agency.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I34Amount",
            self.txt_pg1i34amount.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I34Date",
            self.txt_pg1i34date.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I34Number",
            self.txt_pg1i34number.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I35Agency",
            self.txt_pg1i35agency.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I35Amount",
            self.txt_pg1i35amount.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I35Date",
            self.txt_pg1i35date.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I36Amount",
            self.txt_pg1i36amount.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I36Date",
            self.txt_pg1i36date.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I36Number",
            self.txt_pg1i36number.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I37Agency",
            self.txt_pg1i37agency.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I37Amount",
            self.txt_pg1i37amount.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I37Date",
            self.txt_pg1i37date.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I37Number",
            self.txt_pg1i37number.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I37Particular",
            self.txt_pg1i37particular.clone(),
        );
        insert(&mut fields, "frm1701:txtPg1I4BranchCode", branch.clone());
        insert(&mut fields, "frm1701:txtPg1I4TIN1", tin1.clone());
        insert(&mut fields, "frm1701:txtPg1I4TIN2", tin2.clone());
        insert(&mut fields, "frm1701:txtPg1I4TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1701:txtPg1I5RDOCode",
            self.rdo_code.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I8TaxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I9AZipCode",
            self.zip_code.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg1I9Address",
            self.txt_pg1i9address.clone(),
        );
        insert(&mut fields, "frm1701:txtPg1mBranchCode", branch.clone());
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI10CSchdB",
            self.txt_pg1m_i10cschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI10DSchdB",
            self.txt_pg1m_i10dschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI10GSchdB",
            self.txt_pg1m_i10gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI10HSchdB",
            self.txt_pg1m_i10hschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI11ASchdB",
            self.txt_pg1m_i11aschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI11BSchdB",
            self.txt_pg1m_i11bschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI11CSchdB",
            self.txt_pg1m_i11cschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI11DSchdB",
            self.txt_pg1m_i11dschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI11ESchdB",
            self.txt_pg1m_i11eschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI11FSchdB",
            self.txt_pg1m_i11fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI11GSchdB",
            self.txt_pg1m_i11gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI11HSchdB",
            self.txt_pg1m_i11hschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI12ASchdB",
            self.txt_pg1m_i12aschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI12BSchdB",
            self.txt_pg1m_i12bschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI12CSchdB",
            self.txt_pg1m_i12cschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI12DSchdB",
            self.txt_pg1m_i12dschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI12DescSchdB",
            self.txt_pg1m_i12desc_schd_b.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI12ESchdB",
            self.txt_pg1m_i12eschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI12FSchdB",
            self.txt_pg1m_i12fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI12GSchdB",
            self.txt_pg1m_i12gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI12HSchdB",
            self.txt_pg1m_i12hschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI13ASchdB",
            self.txt_pg1m_i13aschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI13BSchdB",
            self.txt_pg1m_i13bschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI13CSchdB",
            self.txt_pg1m_i13cschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI13DSchdB",
            self.txt_pg1m_i13dschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI13DescSchdB",
            self.txt_pg1m_i13desc_schd_b.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI13ESchdB",
            self.txt_pg1m_i13eschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI13FSchdB",
            self.txt_pg1m_i13fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI13GSchdB",
            self.txt_pg1m_i13gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI13HSchdB",
            self.txt_pg1m_i13hschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI14CSchdB",
            self.txt_pg1m_i14cschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI14DSchdB",
            self.txt_pg1m_i14dschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI14GSchdB",
            self.txt_pg1m_i14gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI14HSchdB",
            self.txt_pg1m_i14hschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI15ASchdB",
            self.txt_pg1m_i15aschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI15BSchdB",
            self.txt_pg1m_i15bschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI15CSchdB",
            self.txt_pg1m_i15cschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI15DSchdB",
            self.txt_pg1m_i15dschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI15ESchdB",
            self.txt_pg1m_i15eschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI15FSchdB",
            self.txt_pg1m_i15fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI15GSchdB",
            self.txt_pg1m_i15gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI15HSchdB",
            self.txt_pg1m_i15hschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI16ASchdB",
            self.txt_pg1m_i16aschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI16BSchdB",
            self.txt_pg1m_i16bschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI16CSchdB",
            self.txt_pg1m_i16cschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI16DSchdB",
            self.txt_pg1m_i16dschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI16ESchdB",
            self.txt_pg1m_i16eschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI16FSchdB",
            self.txt_pg1m_i16fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI16GSchdB",
            self.txt_pg1m_i16gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI16HSchdB",
            self.txt_pg1m_i16hschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI17ASchdB",
            self.txt_pg1m_i17aschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI17BSchdB",
            self.txt_pg1m_i17bschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI17CSchdB",
            self.txt_pg1m_i17cschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI17DSchdB",
            self.txt_pg1m_i17dschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI17ESchdB",
            self.txt_pg1m_i17eschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI17FSchdB",
            self.txt_pg1m_i17fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI17GSchdB",
            self.txt_pg1m_i17gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI17HSchdB",
            self.txt_pg1m_i17hschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI1ASchdA",
            self.txt_pg1m_i1aschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI1ASchdB",
            self.txt_pg1m_i1aschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI1BSchdA",
            self.txt_pg1m_i1bschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI1BSchdB",
            self.txt_pg1m_i1bschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI1CSchdA",
            self.txt_pg1m_i1cschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI1CSchdB",
            self.txt_pg1m_i1cschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI1DSchdA",
            self.txt_pg1m_i1dschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI1DSchdB",
            self.txt_pg1m_i1dschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI1ESchdA",
            self.txt_pg1m_i1eschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI1ESchdB",
            self.txt_pg1m_i1eschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI1FSchdA",
            self.txt_pg1m_i1fschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI1FSchdB",
            self.txt_pg1m_i1fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI1GSchdB",
            self.txt_pg1m_i1gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI1HSchdB",
            self.txt_pg1m_i1hschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI2ASchdA",
            self.txt_pg1m_i2aschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI2ASchdB",
            self.txt_pg1m_i2aschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI2BSchdA",
            self.txt_pg1m_i2bschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI2BSchdB",
            self.txt_pg1m_i2bschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI2CSchdA",
            self.txt_pg1m_i2cschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI2CSchdB",
            self.txt_pg1m_i2cschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI2DSchdA",
            self.txt_pg1m_i2dschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI2DSchdB",
            self.txt_pg1m_i2dschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI2ESchdA",
            self.txt_pg1m_i2eschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI2ESchdB",
            self.txt_pg1m_i2eschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI2FSchdA",
            self.txt_pg1m_i2fschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI2FSchdB",
            self.txt_pg1m_i2fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI2GSchdB",
            self.txt_pg1m_i2gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI2HSchdB",
            self.txt_pg1m_i2hschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI3ASchdA",
            self.txt_pg1m_i3aschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI3ASchdB",
            self.txt_pg1m_i3aschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI3BSchdA",
            self.txt_pg1m_i3bschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI3BSchdB",
            self.txt_pg1m_i3bschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI3CSchdA",
            self.txt_pg1m_i3cschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI3CSchdB",
            self.txt_pg1m_i3cschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI3DSchdA",
            self.txt_pg1m_i3dschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI3DSchdB",
            self.txt_pg1m_i3dschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI3ESchdA",
            self.txt_pg1m_i3eschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI3ESchdB",
            self.txt_pg1m_i3eschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI3FSchdA",
            self.txt_pg1m_i3fschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI3FSchdB",
            self.txt_pg1m_i3fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI3GSchdB",
            self.txt_pg1m_i3gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI3HSchdB",
            self.txt_pg1m_i3hschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI4ASchdB",
            self.txt_pg1m_i4aschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI4BSchdA",
            self.txt_pg1m_i4bschd_a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI4BSchdB",
            self.txt_pg1m_i4bschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI4CSchdB",
            self.txt_pg1m_i4cschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI4DSchdB",
            self.txt_pg1m_i4dschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI4ESchdA",
            self.txt_pg1m_i4eschd_a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI4ESchdB",
            self.txt_pg1m_i4eschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI4FSchdB",
            self.txt_pg1m_i4fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI4GSchdB",
            self.txt_pg1m_i4gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI4HSchdB",
            self.txt_pg1m_i4hschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI5ASchdA",
            self.txt_pg1m_i5aschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI5ASchdB",
            self.txt_pg1m_i5aschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI5BSchdA",
            self.txt_pg1m_i5bschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI5BSchdB",
            self.txt_pg1m_i5bschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI5CSchdA",
            self.txt_pg1m_i5cschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI5CSchdB",
            self.txt_pg1m_i5cschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI5DSchdA",
            self.txt_pg1m_i5dschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI5DSchdB",
            self.txt_pg1m_i5dschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI5ESchdA",
            self.txt_pg1m_i5eschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI5ESchdB",
            self.txt_pg1m_i5eschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI5FSchdA",
            self.txt_pg1m_i5fschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI5FSchdB",
            self.txt_pg1m_i5fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI5GSchdB",
            self.txt_pg1m_i5gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI5HSchdB",
            self.txt_pg1m_i5hschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI6ASchdA",
            self.txt_pg1m_i6aschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI6ASchdB",
            self.txt_pg1m_i6aschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI6BSchdA",
            self.txt_pg1m_i6bschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI6BSchdB",
            self.txt_pg1m_i6bschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI6CSchdA",
            self.txt_pg1m_i6cschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI6CSchdB",
            self.txt_pg1m_i6cschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI6DSchdA",
            self.txt_pg1m_i6dschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI6DSchdB",
            self.txt_pg1m_i6dschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI6ESchdA",
            self.txt_pg1m_i6eschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI6ESchdB",
            self.txt_pg1m_i6eschd_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg1mI6FSchdA",
            self.txt_pg1m_i6fschd_a.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI6FSchdB",
            self.txt_pg1m_i6fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI6GSchdB",
            self.txt_pg1m_i6gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI6HSchdB",
            self.txt_pg1m_i6hschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI7ASchdB",
            self.txt_pg1m_i7aschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI7BSchdB",
            self.txt_pg1m_i7bschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI7CSchdB",
            self.txt_pg1m_i7cschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI7DSchdB",
            self.txt_pg1m_i7dschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI7ESchdB",
            self.txt_pg1m_i7eschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI7FSchdB",
            self.txt_pg1m_i7fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI7GSchdB",
            self.txt_pg1m_i7gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI7HSchdB",
            self.txt_pg1m_i7hschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI8CSchdB",
            self.txt_pg1m_i8cschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI8DSchdB",
            self.txt_pg1m_i8dschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI8GSchdB",
            self.txt_pg1m_i8gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI8HSchdB",
            self.txt_pg1m_i8hschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI9ASchdB",
            self.txt_pg1m_i9aschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI9BSchdB",
            self.txt_pg1m_i9bschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI9CSchdB",
            self.txt_pg1m_i9cschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI9DSchdB",
            self.txt_pg1m_i9dschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI9ESchdB",
            self.txt_pg1m_i9eschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI9FSchdB",
            self.txt_pg1m_i9fschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI9GSchdB",
            self.txt_pg1m_i9gschd_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg1mI9HSchdB",
            self.txt_pg1m_i9hschd_b,
        );
        insert(&mut fields, "frm1701:txtPg1mTIN1", tin1.clone());
        insert(&mut fields, "frm1701:txtPg1mTIN2", tin2.clone());
        insert(&mut fields, "frm1701:txtPg1mTIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1701:txtPg1mTaxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(&mut fields, "frm1701:txtPg2BranchCode", branch.clone());
        insert(&mut fields, "frm1701:txtPg2I1BranchCode", branch.clone());
        insert(&mut fields, "frm1701:txtPg2I1TIN1", tin1.clone());
        insert(&mut fields, "frm1701:txtPg2I1TIN2", tin2.clone());
        insert(&mut fields, "frm1701:txtPg2I1TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1701:txtPg2I2SpouseRDOCode",
            self.rdo_code.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2I5SpouseName",
            self.txt_pg2i5spouse_name.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2I6TelNum",
            self.contact_number.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2I7Citizenship",
            self.txt_pg2i7citizenship.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2I9ForeignTaxNumber",
            self.txt_pg2i9foreign_tax_number.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2IShed1a_1SName",
            self.txt_pg2ished1a_1sname.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2IShed1a_1TPName",
            self.txt_pg2ished1a_1tpname.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2IShed1a_BranchCode",
            branch.clone(),
        );
        insert(&mut fields, "frm1701:txtPg2IShed1a_TIN1", tin1.clone());
        insert(&mut fields, "frm1701:txtPg2IShed1a_TIN2", tin2.clone());
        insert(&mut fields, "frm1701:txtPg2IShed1a_TIN3", tin3.clone());
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed1c_1CI",
            self.txt_pg2ished1c_1ci,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed1c_1TW",
            self.txt_pg2ished1c_1tw,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed1c_2CI",
            self.txt_pg2ished1c_2ci,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed1c_2TW",
            self.txt_pg2ished1c_2tw,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed1c_3ACI",
            self.txt_pg2ished1c_3aci,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed1c_3ATW",
            self.txt_pg2ished1c_3atw,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed1c_3BCI",
            self.txt_pg2ished1c_3bci,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed1c_3BTW",
            self.txt_pg2ished1c_3btw,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed2_4A",
            self.txt_pg2ished2_4a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed2_4B",
            self.txt_pg2ished2_4b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed2_5A",
            self.txt_pg2ished2_5a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed2_5B",
            self.txt_pg2ished2_5b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed2_6A",
            self.txt_pg2ished2_6a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed2_6B",
            self.txt_pg2ished2_6b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed2_7A",
            self.txt_pg2ished2_7a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed2_7B",
            self.txt_pg2ished2_7b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg2IShed2a_2SName",
            self.txt_pg2ished2a_2sname.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2IShed2a_2TPName",
            self.txt_pg2ished2a_2tpname.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2IShed2a_BranchCode",
            branch.clone(),
        );
        insert(&mut fields, "frm1701:txtPg2IShed2a_TIN1", tin1.clone());
        insert(&mut fields, "frm1701:txtPg2IShed2a_TIN2", tin2.clone());
        insert(&mut fields, "frm1701:txtPg2IShed2a_TIN3", tin3.clone());
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_10A",
            self.txt_pg2ished3_10a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_10B",
            self.txt_pg2ished3_10b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_11A",
            self.txt_pg2ished3_11a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_11B",
            self.txt_pg2ished3_11b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_12A",
            self.txt_pg2ished3_12a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_12B",
            self.txt_pg2ished3_12b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_13A",
            self.txt_pg2ished3_13a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_13B",
            self.txt_pg2ished3_13b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_14A",
            self.txt_pg2ished3_14a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_14B",
            self.txt_pg2ished3_14b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_15A",
            self.txt_pg2ished3_15a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_15B",
            self.txt_pg2ished3_15b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_16A",
            self.txt_pg2ished3_16a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_16B",
            self.txt_pg2ished3_16b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_17A",
            self.txt_pg2ished3_17a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_17B",
            self.txt_pg2ished3_17b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_18A",
            self.txt_pg2ished3_18a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_18B",
            self.txt_pg2ished3_18b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_19A",
            self.txt_pg2ished3_19a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_19B",
            self.txt_pg2ished3_19b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg2IShed3_19Desc",
            self.txt_pg2ished3_19desc.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_20A",
            self.txt_pg2ished3_20a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_20B",
            self.txt_pg2ished3_20b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg2IShed3_20Desc",
            self.txt_pg2ished3_20desc.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_21A",
            self.txt_pg2ished3_21a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_21B",
            self.txt_pg2ished3_21b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_22A",
            self.txt_pg2ished3_22a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_22B",
            self.txt_pg2ished3_22b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_23A",
            self.txt_pg2ished3_23a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_23B",
            self.txt_pg2ished3_23b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_24A",
            self.txt_pg2ished3_24a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_24B",
            self.txt_pg2ished3_24b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_25A",
            self.txt_pg2ished3_25a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_25B",
            self.txt_pg2ished3_25b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_8A",
            self.txt_pg2ished3_8a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_8B",
            self.txt_pg2ished3_8b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_9A",
            self.txt_pg2ished3_9a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2IShed3_9B",
            self.txt_pg2ished3_9b,
        );
        insert(&mut fields, "frm1701:txtPg2TIN1", tin1.clone());
        insert(&mut fields, "frm1701:txtPg2TIN2", tin2.clone());
        insert(&mut fields, "frm1701:txtPg2TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1701:txtPg2TaxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(&mut fields, "frm1701:txtPg2mBranchCode", branch.clone());
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI10ASchdC",
            self.txt_pg2m_i10aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI10BSchdC",
            self.txt_pg2m_i10bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI10CSchdC",
            self.txt_pg2m_i10cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI10DSchdC",
            self.txt_pg2m_i10dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI11ASchdC",
            self.txt_pg2m_i11aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI11BSchdC",
            self.txt_pg2m_i11bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI11CSchdC",
            self.txt_pg2m_i11cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI11DSchdC",
            self.txt_pg2m_i11dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI12ASchdC",
            self.txt_pg2m_i12aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI12BSchdC",
            self.txt_pg2m_i12bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI12CSchdC",
            self.txt_pg2m_i12cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI12DSchdC",
            self.txt_pg2m_i12dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI13ASchdC",
            self.txt_pg2m_i13aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI13BSchdC",
            self.txt_pg2m_i13bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI13CSchdC",
            self.txt_pg2m_i13cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI13DSchdC",
            self.txt_pg2m_i13dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI14ASchdC",
            self.txt_pg2m_i14aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI14BSchdC",
            self.txt_pg2m_i14bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI14CSchdC",
            self.txt_pg2m_i14cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI14DSchdC",
            self.txt_pg2m_i14dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI15ASchdC",
            self.txt_pg2m_i15aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI15BSchdC",
            self.txt_pg2m_i15bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI15CSchdC",
            self.txt_pg2m_i15cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI15DSchdC",
            self.txt_pg2m_i15dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI16ASchdC",
            self.txt_pg2m_i16aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI16BSchdC",
            self.txt_pg2m_i16bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI16CSchdC",
            self.txt_pg2m_i16cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI16DSchdC",
            self.txt_pg2m_i16dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17aASchdC",
            self.txt_pg2m_i17a_aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17aBSchdC",
            self.txt_pg2m_i17a_bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17aCSchdC",
            self.txt_pg2m_i17a_cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17aDSchdC",
            self.txt_pg2m_i17a_dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17bASchdC",
            self.txt_pg2m_i17b_aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17bBSchdC",
            self.txt_pg2m_i17b_bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17bCSchdC",
            self.txt_pg2m_i17b_cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17bDSchdC",
            self.txt_pg2m_i17b_dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17cASchdC",
            self.txt_pg2m_i17c_aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17cBSchdC",
            self.txt_pg2m_i17c_bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17cCSchdC",
            self.txt_pg2m_i17c_cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17cDSchdC",
            self.txt_pg2m_i17c_dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17dASchdC",
            self.txt_pg2m_i17d_aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17dBSchdC",
            self.txt_pg2m_i17d_bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17dCSchdC",
            self.txt_pg2m_i17d_cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI17dDSchdC",
            self.txt_pg2m_i17d_dschd_c,
        );
        insert(
            &mut fields,
            "frm1701:txtPg2mI17dDescSchdC",
            self.txt_pg2m_i17d_desc_schd_c.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI18ASchdC",
            self.txt_pg2m_i18aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI18BSchdC",
            self.txt_pg2m_i18bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI18CSchdC",
            self.txt_pg2m_i18cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI18DSchdC",
            self.txt_pg2m_i18dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI1ASchdC",
            self.txt_pg2m_i1aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI1ASchdD",
            self.txt_pg2m_i1aschd_d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI1BSchdC",
            self.txt_pg2m_i1bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI1BSchdD",
            self.txt_pg2m_i1bschd_d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI1CSchdC",
            self.txt_pg2m_i1cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI1DSchdC",
            self.txt_pg2m_i1dschd_c,
        );
        insert(
            &mut fields,
            "frm1701:txtPg2mI1DescSchdD",
            self.txt_pg2m_i1desc_schd_d.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2mI1LBSchdD",
            self.txt_pg2m_i1lbschd_d.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI2ASchdC",
            self.txt_pg2m_i2aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI2ASchdD",
            self.txt_pg2m_i2aschd_d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI2BSchdC",
            self.txt_pg2m_i2bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI2BSchdD",
            self.txt_pg2m_i2bschd_d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI2CSchdC",
            self.txt_pg2m_i2cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI2DSchdC",
            self.txt_pg2m_i2dschd_c,
        );
        insert(
            &mut fields,
            "frm1701:txtPg2mI2DescSchdD",
            self.txt_pg2m_i2desc_schd_d.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2mI2LBSchdD",
            self.txt_pg2m_i2lbschd_d.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI3ASchdC",
            self.txt_pg2m_i3aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI3ASchdD",
            self.txt_pg2m_i3aschd_d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI3BSchdC",
            self.txt_pg2m_i3bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI3BSchdD",
            self.txt_pg2m_i3bschd_d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI3CSchdC",
            self.txt_pg2m_i3cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI3DSchdC",
            self.txt_pg2m_i3dschd_c,
        );
        insert(
            &mut fields,
            "frm1701:txtPg2mI3DescSchdD",
            self.txt_pg2m_i3desc_schd_d.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2mI3LBSchdD",
            self.txt_pg2m_i3lbschd_d.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI4ASchdC",
            self.txt_pg2m_i4aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI4ASchdD",
            self.txt_pg2m_i4aschd_d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI4BSchdC",
            self.txt_pg2m_i4bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI4BSchdD",
            self.txt_pg2m_i4bschd_d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI4CSchdC",
            self.txt_pg2m_i4cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI4DSchdC",
            self.txt_pg2m_i4dschd_c,
        );
        insert(
            &mut fields,
            "frm1701:txtPg2mI4DescSchdD",
            self.txt_pg2m_i4desc_schd_d.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg2mI4LBSchdD",
            self.txt_pg2m_i4lbschd_d.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI5ASchdC",
            self.txt_pg2m_i5aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI5ASchdD",
            self.txt_pg2m_i5aschd_d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI5BSchdC",
            self.txt_pg2m_i5bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI5BSchdD",
            self.txt_pg2m_i5bschd_d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI5CSchdC",
            self.txt_pg2m_i5cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI5DSchdC",
            self.txt_pg2m_i5dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI6ASchdC",
            self.txt_pg2m_i6aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI6BSchdC",
            self.txt_pg2m_i6bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI6CSchdC",
            self.txt_pg2m_i6cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI6DSchdC",
            self.txt_pg2m_i6dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI7ASchdC",
            self.txt_pg2m_i7aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI7BSchdC",
            self.txt_pg2m_i7bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI7CSchdC",
            self.txt_pg2m_i7cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI7DSchdC",
            self.txt_pg2m_i7dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI8ASchdC",
            self.txt_pg2m_i8aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI8BSchdC",
            self.txt_pg2m_i8bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI8CSchdC",
            self.txt_pg2m_i8cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI8DSchdC",
            self.txt_pg2m_i8dschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI9ASchdC",
            self.txt_pg2m_i9aschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI9BSchdC",
            self.txt_pg2m_i9bschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI9CSchdC",
            self.txt_pg2m_i9cschd_c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg2mI9DSchdC",
            self.txt_pg2m_i9dschd_c,
        );
        insert(&mut fields, "frm1701:txtPg2mTIN1", tin1.clone());
        insert(&mut fields, "frm1701:txtPg2mTIN2", tin2.clone());
        insert(&mut fields, "frm1701:txtPg2mTIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1701:txtPg2mTaxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(&mut fields, "frm1701:txtPg3BranchCode", branch.clone());
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_26A",
            self.txt_pg3ished3_26a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_26B",
            self.txt_pg3ished3_26b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_27A",
            self.txt_pg3ished3_27a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_27B",
            self.txt_pg3ished3_27b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed3_27Desc",
            self.txt_pg3ished3_27desc.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_28A",
            self.txt_pg3ished3_28a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_28B",
            self.txt_pg3ished3_28b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_29A",
            self.txt_pg3ished3_29a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_29B",
            self.txt_pg3ished3_29b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_30A",
            self.txt_pg3ished3_30a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_30B",
            self.txt_pg3ished3_30b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_31A",
            self.txt_pg3ished3_31a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_31B",
            self.txt_pg3ished3_31b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_32A",
            self.txt_pg3ished3_32a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed3_32B",
            self.txt_pg3ished3_32b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_10A",
            self.txt_pg3ished4_10a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_10B",
            self.txt_pg3ished4_10b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_11A",
            self.txt_pg3ished4_11a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_11B",
            self.txt_pg3ished4_11b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_12A",
            self.txt_pg3ished4_12a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_12B",
            self.txt_pg3ished4_12b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_13A",
            self.txt_pg3ished4_13a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_13B",
            self.txt_pg3ished4_13b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_14A",
            self.txt_pg3ished4_14a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_14B",
            self.txt_pg3ished4_14b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_15A",
            self.txt_pg3ished4_15a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_15B",
            self.txt_pg3ished4_15b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_16A",
            self.txt_pg3ished4_16a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_16B",
            self.txt_pg3ished4_16b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_17aA",
            self.txt_pg3ished4_17a_a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_17aB",
            self.txt_pg3ished4_17a_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_17bA",
            self.txt_pg3ished4_17b_a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_17bB",
            self.txt_pg3ished4_17b_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_17cA",
            self.txt_pg3ished4_17c_a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_17cB",
            self.txt_pg3ished4_17c_b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_17dA",
            self.txt_pg3ished4_17d_a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_17dB",
            self.txt_pg3ished4_17d_b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed4_17dDesc",
            self.txt_pg3ished4_17d_desc.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_18A",
            self.txt_pg3ished4_18a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_18B",
            self.txt_pg3ished4_18b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_1A",
            self.txt_pg3ished4_1a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_1B",
            self.txt_pg3ished4_1b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_2A",
            self.txt_pg3ished4_2a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_2B",
            self.txt_pg3ished4_2b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_3A",
            self.txt_pg3ished4_3a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_3B",
            self.txt_pg3ished4_3b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_4A",
            self.txt_pg3ished4_4a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_4B",
            self.txt_pg3ished4_4b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_5A",
            self.txt_pg3ished4_5a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_5B",
            self.txt_pg3ished4_5b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_6A",
            self.txt_pg3ished4_6a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_6B",
            self.txt_pg3ished4_6b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_7A",
            self.txt_pg3ished4_7a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_7B",
            self.txt_pg3ished4_7b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_8A",
            self.txt_pg3ished4_8a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_8B",
            self.txt_pg3ished4_8b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_9A",
            self.txt_pg3ished4_9a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed4_9B",
            self.txt_pg3ished4_9b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed5_1Amt",
            self.txt_pg3ished5_1amt,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed5_1Desc",
            self.txt_pg3ished5_1desc.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed5_1Legal",
            self.txt_pg3ished5_1legal.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed5_2Amt",
            self.txt_pg3ished5_2amt,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed5_2Desc",
            self.txt_pg3ished5_2desc.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed5_2Legal",
            self.txt_pg3ished5_2legal.clone(),
        );
        insert_money(&mut fields, "frm1701:txtPg3IShed5_3", self.txt_pg3ished5_3);
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed5_4Amt",
            self.txt_pg3ished5_4amt,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed5_4Desc",
            self.txt_pg3ished5_4desc.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed5_4Legal",
            self.txt_pg3ished5_4legal.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed5_5Amt",
            self.txt_pg3ished5_5amt,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed5_5Desc",
            self.txt_pg3ished5_5desc.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed5_5Legal",
            self.txt_pg3ished5_5legal.clone(),
        );
        insert_money(&mut fields, "frm1701:txtPg3IShed5_6", self.txt_pg3ished5_6);
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_1A",
            self.txt_pg3ished6_1a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_1B",
            self.txt_pg3ished6_1b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_2A",
            self.txt_pg3ished6_2a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_2B",
            self.txt_pg3ished6_2b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_3A",
            self.txt_pg3ished6_3a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_3B",
            self.txt_pg3ished6_3b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_4A",
            self.txt_pg3ished6_4a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_4B",
            self.txt_pg3ished6_4b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_4C",
            self.txt_pg3ished6_4c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_4D",
            self.txt_pg3ished6_4d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_4E",
            self.txt_pg3ished6_4e,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed6_4Year",
            self.taxable_year.to_string(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_5A",
            self.txt_pg3ished6_5a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_5B",
            self.txt_pg3ished6_5b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_5C",
            self.txt_pg3ished6_5c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_5D",
            self.txt_pg3ished6_5d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_5E",
            self.txt_pg3ished6_5e,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed6_5Year",
            self.taxable_year.to_string(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_6A",
            self.txt_pg3ished6_6a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_6B",
            self.txt_pg3ished6_6b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_6C",
            self.txt_pg3ished6_6c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_6D",
            self.txt_pg3ished6_6d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_6E",
            self.txt_pg3ished6_6e,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed6_6Year",
            self.taxable_year.to_string(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_7A",
            self.txt_pg3ished6_7a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_7B",
            self.txt_pg3ished6_7b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_7C",
            self.txt_pg3ished6_7c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_7D",
            self.txt_pg3ished6_7d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_7E",
            self.txt_pg3ished6_7e,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3IShed6_7Year",
            self.taxable_year.to_string(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3IShed6_8D",
            self.txt_pg3ished6_8d,
        );
        insert(&mut fields, "frm1701:txtPg3TIN1", tin1.clone());
        insert(&mut fields, "frm1701:txtPg3TIN2", tin2.clone());
        insert(&mut fields, "frm1701:txtPg3TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1701:txtPg3TaxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(&mut fields, "frm1701:txtPg3mBranchCode", branch.clone());
        insert(
            &mut fields,
            "frm1701:txtPg3mSchedA_1ATYPE",
            self.txt_pg3m_sched_a_1atype.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg3mSchedA_1BTYPE",
            self.txt_pg3m_sched_a_1btype.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg3mSchedA_2ATYPE",
            self.txt_pg3m_sched_a_2atype.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg3mSchedA_2BTYPE",
            self.txt_pg3m_sched_a_2btype.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg3mSchedA_3ATYPE",
            self.txt_pg3m_sched_a_3atype.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg3mSchedA_3BTYPE",
            self.txt_pg3m_sched_a_3btype.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedA_4ATYPE",
            self.txt_pg3m_sched_a_4atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedA_4BTYPE",
            self.txt_pg3m_sched_a_4btype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3mSchedA_5ATYPE",
            self.txt_pg3m_sched_a_5atype.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg3mSchedA_5BTYPE",
            self.txt_pg3m_sched_a_5btype.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg3mSchedA_6ATYPE",
            self.txt_pg3m_sched_a_6atype.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg3mSchedA_6BTYPE",
            self.txt_pg3m_sched_a_6btype.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_10ATYPE",
            self.txt_pg3m_sched_b_10atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_10BTYPE",
            self.txt_pg3m_sched_b_10btype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3mSchedB_10TYPE",
            self.txt_pg3m_sched_b_10type.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_11ATYPE",
            self.txt_pg3m_sched_b_11atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_11BTYPE",
            self.txt_pg3m_sched_b_11btype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg3mSchedB_11TYPE",
            self.txt_pg3m_sched_b_11type.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_12ATYPE",
            self.txt_pg3m_sched_b_12atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_12BTYPE",
            self.txt_pg3m_sched_b_12btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_13ATYPE",
            self.txt_pg3m_sched_b_13atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_13BTYPE",
            self.txt_pg3m_sched_b_13btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_14ATYPE",
            self.txt_pg3m_sched_b_14atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_14BTYPE",
            self.txt_pg3m_sched_b_14btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_15ATYPE",
            self.txt_pg3m_sched_b_15atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_15BTYPE",
            self.txt_pg3m_sched_b_15btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_1ATYPE",
            self.txt_pg3m_sched_b_1atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_1BTYPE",
            self.txt_pg3m_sched_b_1btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_2ATYPE",
            self.txt_pg3m_sched_b_2atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_2BTYPE",
            self.txt_pg3m_sched_b_2btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_3ATYPE",
            self.txt_pg3m_sched_b_3atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_3BTYPE",
            self.txt_pg3m_sched_b_3btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_4ATYPE",
            self.txt_pg3m_sched_b_4atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_4BTYPE",
            self.txt_pg3m_sched_b_4btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_5ATYPE",
            self.txt_pg3m_sched_b_5atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_5BTYPE",
            self.txt_pg3m_sched_b_5btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_6ATYPE",
            self.txt_pg3m_sched_b_6atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_6BTYPE",
            self.txt_pg3m_sched_b_6btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_7ATYPE",
            self.txt_pg3m_sched_b_7atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_7BTYPE",
            self.txt_pg3m_sched_b_7btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_8ATYPE",
            self.txt_pg3m_sched_b_8atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_8BTYPE",
            self.txt_pg3m_sched_b_8btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_9ATYPE",
            self.txt_pg3m_sched_b_9atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedB_9BTYPE",
            self.txt_pg3m_sched_b_9btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedC_1ATYPE",
            self.txt_pg3m_sched_c_1atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedC_1BTYPE",
            self.txt_pg3m_sched_c_1btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedC_2ATYPE",
            self.txt_pg3m_sched_c_2atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedC_2BTYPE",
            self.txt_pg3m_sched_c_2btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedC_3ATYPE",
            self.txt_pg3m_sched_c_3atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg3mSchedC_3BTYPE",
            self.txt_pg3m_sched_c_3btype,
        );
        insert(&mut fields, "frm1701:txtPg3mTIN1", tin1.clone());
        insert(&mut fields, "frm1701:txtPg3mTIN2", tin2.clone());
        insert(&mut fields, "frm1701:txtPg3mTIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1701:txtPg3mTaxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(&mut fields, "frm1701:txtPg4BranchCode", branch.clone());
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_10A",
            self.txt_pg4ipart7_10a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_10B",
            self.txt_pg4ipart7_10b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_1A",
            self.txt_pg4ipart7_1a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_1B",
            self.txt_pg4ipart7_1b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_2A",
            self.txt_pg4ipart7_2a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_2B",
            self.txt_pg4ipart7_2b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_3A",
            self.txt_pg4ipart7_3a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_3B",
            self.txt_pg4ipart7_3b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_4A",
            self.txt_pg4ipart7_4a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_4B",
            self.txt_pg4ipart7_4b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_5A",
            self.txt_pg4ipart7_5a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_5B",
            self.txt_pg4ipart7_5b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_6A",
            self.txt_pg4ipart7_6a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_6B",
            self.txt_pg4ipart7_6b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_7A",
            self.txt_pg4ipart7_7a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_7B",
            self.txt_pg4ipart7_7b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_8A",
            self.txt_pg4ipart7_8a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_8B",
            self.txt_pg4ipart7_8b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_9A",
            self.txt_pg4ipart7_9a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart7_9B",
            self.txt_pg4ipart7_9b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4IPart7_9Specify",
            self.txt_pg4ipart7_9specify.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_10A",
            self.txt_pg4ipart8_10a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_10B",
            self.txt_pg4ipart8_10b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_1A",
            self.txt_pg4ipart8_1a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_1B",
            self.txt_pg4ipart8_1b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_2A",
            self.txt_pg4ipart8_2a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_2B",
            self.txt_pg4ipart8_2b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_3A",
            self.txt_pg4ipart8_3a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_3B",
            self.txt_pg4ipart8_3b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_4A",
            self.txt_pg4ipart8_4a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_4B",
            self.txt_pg4ipart8_4b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_5A",
            self.txt_pg4ipart8_5a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_5B",
            self.txt_pg4ipart8_5b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_6A",
            self.txt_pg4ipart8_6a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_6B",
            self.txt_pg4ipart8_6b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_7A",
            self.txt_pg4ipart8_7a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_7B",
            self.txt_pg4ipart8_7b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_8A",
            self.txt_pg4ipart8_8a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_8B",
            self.txt_pg4ipart8_8b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_9A",
            self.txt_pg4ipart8_9a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart8_9B",
            self.txt_pg4ipart8_9b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_10A",
            self.txt_pg4ipart9_10a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_10B",
            self.txt_pg4ipart9_10b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_11A",
            self.txt_pg4ipart9_11a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_11B",
            self.txt_pg4ipart9_11b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_1A",
            self.txt_pg4ipart9_1a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_1B",
            self.txt_pg4ipart9_1b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_2A",
            self.txt_pg4ipart9_2a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_2B",
            self.txt_pg4ipart9_2b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4IPart9_2Particulars",
            self.txt_pg4ipart9_2particulars.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_3A",
            self.txt_pg4ipart9_3a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_3B",
            self.txt_pg4ipart9_3b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4IPart9_3Particulars",
            self.txt_pg4ipart9_3particulars.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_4A",
            self.txt_pg4ipart9_4a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_4B",
            self.txt_pg4ipart9_4b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4IPart9_4Particulars",
            self.txt_pg4ipart9_4particulars.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_5A",
            self.txt_pg4ipart9_5a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_5B",
            self.txt_pg4ipart9_5b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_6A",
            self.txt_pg4ipart9_6a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_6B",
            self.txt_pg4ipart9_6b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4IPart9_6Particulars",
            self.txt_pg4ipart9_6particulars.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_7A",
            self.txt_pg4ipart9_7a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_7B",
            self.txt_pg4ipart9_7b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4IPart9_7Particulars",
            self.txt_pg4ipart9_7particulars.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_8A",
            self.txt_pg4ipart9_8a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_8B",
            self.txt_pg4ipart9_8b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4IPart9_8Particulars",
            self.txt_pg4ipart9_8particulars.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_9A",
            self.txt_pg4ipart9_9a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IPart9_9B",
            self.txt_pg4ipart9_9b,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4IPart9_9Particulars",
            self.txt_pg4ipart9_9particulars.clone(),
        );
        insert_money(&mut fields, "frm1701:txtPg4ISc6_1A", self.txt_pg4isc6_1a);
        insert_money(&mut fields, "frm1701:txtPg4ISc6_1B", self.txt_pg4isc6_1b);
        insert_money(&mut fields, "frm1701:txtPg4ISc6_2A", self.txt_pg4isc6_2a);
        insert_money(&mut fields, "frm1701:txtPg4ISc6_2B", self.txt_pg4isc6_2b);
        insert_money(&mut fields, "frm1701:txtPg4ISc6_3A", self.txt_pg4isc6_3a);
        insert_money(&mut fields, "frm1701:txtPg4ISc6_3B", self.txt_pg4isc6_3b);
        insert_money(&mut fields, "frm1701:txtPg4ISc6_4A", self.txt_pg4isc6_4a);
        insert_money(&mut fields, "frm1701:txtPg4ISc6_4B", self.txt_pg4isc6_4b);
        insert_money(&mut fields, "frm1701:txtPg4ISc6_5A", self.txt_pg4isc6_5a);
        insert_money(&mut fields, "frm1701:txtPg4ISc6_5B", self.txt_pg4isc6_5b);
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_10A",
            self.txt_pg4ished6_10a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_10B",
            self.txt_pg4ished6_10b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_10C",
            self.txt_pg4ished6_10c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_10D",
            self.txt_pg4ished6_10d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_10E",
            self.txt_pg4ished6_10e,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4IShed6_10Year",
            self.taxable_year.to_string(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_11A",
            self.txt_pg4ished6_11a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_11B",
            self.txt_pg4ished6_11b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_11C",
            self.txt_pg4ished6_11c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_11D",
            self.txt_pg4ished6_11d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_11E",
            self.txt_pg4ished6_11e,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4IShed6_11Year",
            self.taxable_year.to_string(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_12A",
            self.txt_pg4ished6_12a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_12B",
            self.txt_pg4ished6_12b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_12C",
            self.txt_pg4ished6_12c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_12D",
            self.txt_pg4ished6_12d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_12E",
            self.txt_pg4ished6_12e,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4IShed6_12Year",
            self.taxable_year.to_string(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_13D",
            self.txt_pg4ished6_13d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_9A",
            self.txt_pg4ished6_9a,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_9B",
            self.txt_pg4ished6_9b,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_9C",
            self.txt_pg4ished6_9c,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_9D",
            self.txt_pg4ished6_9d,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4IShed6_9E",
            self.txt_pg4ished6_9e,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4IShed6_9Year",
            self.taxable_year.to_string(),
        );
        insert(&mut fields, "frm1701:txtPg4TIN1", tin1.clone());
        insert(&mut fields, "frm1701:txtPg4TIN2", tin2.clone());
        insert(&mut fields, "frm1701:txtPg4TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1701:txtPg4TaxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(&mut fields, "frm1701:txtPg4mBranchCode", branch.clone());
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_10ATYPE",
            self.txt_pg4m_sched_c_10atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_10BTYPE",
            self.txt_pg4m_sched_c_10btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_11ATYPE",
            self.txt_pg4m_sched_c_11atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_11BTYPE",
            self.txt_pg4m_sched_c_11btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_12ATYPE",
            self.txt_pg4m_sched_c_12atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_12BTYPE",
            self.txt_pg4m_sched_c_12btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_13ATYPE",
            self.txt_pg4m_sched_c_13atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_13BTYPE",
            self.txt_pg4m_sched_c_13btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_14ATYPE",
            self.txt_pg4m_sched_c_14atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_14BTYPE",
            self.txt_pg4m_sched_c_14btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_15ATYPE",
            self.txt_pg4m_sched_c_15atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_15BTYPE",
            self.txt_pg4m_sched_c_15btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_16ATYPE",
            self.txt_pg4m_sched_c_16atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_16BTYPE",
            self.txt_pg4m_sched_c_16btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_17aATYPE",
            self.txt_pg4m_sched_c_17a_atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_17aBTYPE",
            self.txt_pg4m_sched_c_17a_btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_17bATYPE",
            self.txt_pg4m_sched_c_17b_atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_17bBTYPE",
            self.txt_pg4m_sched_c_17b_btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_17cATYPE",
            self.txt_pg4m_sched_c_17c_atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_17cBTYPE",
            self.txt_pg4m_sched_c_17c_btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_17dATYPE",
            self.txt_pg4m_sched_c_17d_atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_17dBTYPE",
            self.txt_pg4m_sched_c_17d_btype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedC_17dTYPE",
            self.txt_pg4m_sched_c_17d_type.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_18ATYPE",
            self.txt_pg4m_sched_c_18atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_18BTYPE",
            self.txt_pg4m_sched_c_18btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_4ATYPE",
            self.txt_pg4m_sched_c_4atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_4BTYPE",
            self.txt_pg4m_sched_c_4btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_5ATYPE",
            self.txt_pg4m_sched_c_5atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_5BTYPE",
            self.txt_pg4m_sched_c_5btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_6ATYPE",
            self.txt_pg4m_sched_c_6atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_6BTYPE",
            self.txt_pg4m_sched_c_6btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_7ATYPE",
            self.txt_pg4m_sched_c_7atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_7BTYPE",
            self.txt_pg4m_sched_c_7btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_8ATYPE",
            self.txt_pg4m_sched_c_8atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_8BTYPE",
            self.txt_pg4m_sched_c_8btype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_9ATYPE",
            self.txt_pg4m_sched_c_9atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedC_9BTYPE",
            self.txt_pg4m_sched_c_9btype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD1_1ALBTYPE",
            self.txt_pg4m_sched_d1_1albtype.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedD1_1ATYPE",
            self.txt_pg4m_sched_d1_1atype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD1_1TYPE",
            self.txt_pg4m_sched_d1_1type.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD1_2ALBTYPE",
            self.txt_pg4m_sched_d1_2albtype.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedD1_2ATYPE",
            self.txt_pg4m_sched_d1_2atype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD1_2TYPE",
            self.txt_pg4m_sched_d1_2type.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD1_3ALBTYPE",
            self.txt_pg4m_sched_d1_3albtype.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedD1_3ATYPE",
            self.txt_pg4m_sched_d1_3atype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD1_3TYPE",
            self.txt_pg4m_sched_d1_3type.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD1_4ALBTYPE",
            self.txt_pg4m_sched_d1_4albtype.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedD1_4ATYPE",
            self.txt_pg4m_sched_d1_4atype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD1_4TYPE",
            self.txt_pg4m_sched_d1_4type.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedD1_5ATYPE",
            self.txt_pg4m_sched_d1_5atype,
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedD2_10BTYPE",
            self.txt_pg4m_sched_d2_10btype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD2_6BLBTYPE",
            self.txt_pg4m_sched_d2_6blbtype.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedD2_6BTYPE",
            self.txt_pg4m_sched_d2_6btype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD2_6TYPE",
            self.txt_pg4m_sched_d2_6type.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD2_7BLBTYPE",
            self.txt_pg4m_sched_d2_7blbtype.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedD2_7BTYPE",
            self.txt_pg4m_sched_d2_7btype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD2_7TYPE",
            self.txt_pg4m_sched_d2_7type.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedD2_8BTYPE",
            self.txt_pg4m_sched_d2_8btype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD2_8LBBTYPE",
            self.txt_pg4m_sched_d2_8lbbtype.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD2_8TYPE",
            self.txt_pg4m_sched_d2_8type.clone(),
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD2_9BLBTYPE",
            self.txt_pg4m_sched_d2_9blbtype.clone(),
        );
        insert_money(
            &mut fields,
            "frm1701:txtPg4mSchedD2_9BTYPE",
            self.txt_pg4m_sched_d2_9btype,
        );
        insert(
            &mut fields,
            "frm1701:txtPg4mSchedD2_9TYPE",
            self.txt_pg4m_sched_d2_9type.clone(),
        );
        insert(&mut fields, "frm1701:txtPg4mTIN1", tin1.clone());
        insert(&mut fields, "frm1701:txtPg4mTIN2", tin2.clone());
        insert(&mut fields, "frm1701:txtPg4mTIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1701:txtPg4mTaxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(&mut fields, "frm1701:txtTIN4", self.txt_tin4.clone());
        insert(
            &mut fields,
            "frm1701:txtVersion",
            self.txt_version.to_string(),
        );
        insert(&mut fields, "frm1701:txtZIP", self.txt_zip.clone());
        insert(
            &mut fields,
            "frm1701:txtdisabledID",
            self.txtdisabled_id.clone(),
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

fn insert_money(map: &mut BTreeMap<String, String>, key: &str, value: f64) {
    insert(map, key, format!("{:.2}", value));
}
