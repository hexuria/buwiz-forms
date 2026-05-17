//! BIR Form 1701v2018 — Typed draft struct and computation logic.
//!
//! Generated from savefile: 00000000000000-1701v2018-122025.xml
//! Total BIR fields: 837
//! Form-specific fields: 759
//!
//! ⚠️ ScaffoldOnly — formula evidence not yet verified

use crate::forms::{FilingStatus, FormValidator};
use crate::profile::TaxpayerProfile;
use serde::{Deserialize, Serialize};

/// Complete draft for Form 1701v2018.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form1701Draft {
    /// Database row ID (None before first save)
    pub id: Option<i64>,

    // === Filing Period ===
    pub tin: String,
    pub taxable_year: u16,
    pub month: u8,

    // === Header / Options ===
    pub is_amended: bool,

    // === Profile Fields (pre-filled) ===
    pub rdo_code: String,
    pub taxpayer_name: String,
    pub registered_address: String,
    pub zip_code: String,
    pub contact_number: String,
    pub email: String,

    // === checkboxes ===
    /// BIR: `frm1701:chkPg2IShed1a_1Spouse` (sample: `false`)
    pub chk_pg2ished1a_1spouse: bool,
    /// BIR: `frm1701:chkPg2IShed1a_1Taxpayer` (sample: `false`)
    pub chk_pg2ished1a_1taxpayer: bool,
    /// BIR: `frm1701:chkPg2IShed2a_2Spouse` (sample: `false`)
    pub chk_pg2ished2a_2spouse: bool,
    /// BIR: `frm1701:chkPg2IShed2a_2Taxpayer` (sample: `false`)
    pub chk_pg2ished2a_2taxpayer: bool,

    // === radio_options ===
    /// BIR: `frm1701:rdoEXAttachmentS` (sample: `false`)
    pub rdo_exattachment_s: bool,
    /// BIR: `frm1701:rdoEXAttachmentTF` (sample: `false`)
    pub rdo_exattachment_tf: bool,
    /// BIR: `frm1701:rdoPg1I13ForeignTaxCreditsNo` (sample: `true`)
    pub rdo_pg1i13foreign_tax_credits_no: bool,
    /// BIR: `frm1701:rdoPg1I13ForeignTaxCreditsYes` (sample: `false`)
    pub rdo_pg1i13foreign_tax_credits_yes: bool,
    /// BIR: `frm1701:rdoPg1I16CivilStatusLS` (sample: `false`)
    pub rdo_pg1i16civil_status_ls: bool,
    /// BIR: `frm1701:rdoPg1I16CivilStatusM` (sample: `false`)
    pub rdo_pg1i16civil_status_m: bool,
    /// BIR: `frm1701:rdoPg1I16CivilStatusS` (sample: `true`)
    pub rdo_pg1i16civil_status_s: bool,
    /// BIR: `frm1701:rdoPg1I16CivilStatusW` (sample: `false`)
    pub rdo_pg1i16civil_status_w: bool,
    /// BIR: `frm1701:rdoPg1I17SpouseIncomeNo` (sample: `false`)
    pub rdo_pg1i17spouse_income_no: bool,
    /// BIR: `frm1701:rdoPg1I17SpouseIncomeYes` (sample: `false`)
    pub rdo_pg1i17spouse_income_yes: bool,
    /// BIR: `frm1701:rdoPg1I18FilingStatusJ` (sample: `false`)
    pub rdo_pg1i18filing_status_j: bool,
    /// BIR: `frm1701:rdoPg1I18FilingStatusS` (sample: `false`)
    pub rdo_pg1i18filing_status_s: bool,
    /// BIR: `frm1701:rdoPg1I19IncomeExemptNo` (sample: `true`)
    pub rdo_pg1i19income_exempt_no: bool,
    /// BIR: `frm1701:rdoPg1I19IncomeExemptYes` (sample: `false`)
    pub rdo_pg1i19income_exempt_yes: bool,
    /// BIR: `frm1701:rdoPg1I20IncomeSpecialNo` (sample: `true`)
    pub rdo_pg1i20income_special_no: bool,
    /// BIR: `frm1701:rdoPg1I20IncomeSpecialYes` (sample: `false`)
    pub rdo_pg1i20income_special_yes: bool,
    /// BIR: `frm1701:rdoPg1I21AMethodDeductionI` (sample: `false`)
    pub rdo_pg1i21amethod_deduction_i: bool,
    /// BIR: `frm1701:rdoPg1I21AMethodDeductionO` (sample: `true`)
    pub rdo_pg1i21amethod_deduction_o: bool,
    /// BIR: `frm1701:rdoPg1I21TaxRateG` (sample: `true`)
    pub rdo_pg1i21tax_rate_g: bool,
    /// BIR: `frm1701:rdoPg1I21TaxRateP` (sample: `false`)
    pub rdo_pg1i21tax_rate_p: bool,
    /// BIR: `frm1701:rdoPg1I3ShortPeriodNo` (sample: `true`)
    pub rdo_pg1i3short_period_no: bool,
    /// BIR: `frm1701:rdoPg1I3ShortPeriodYes` (sample: `false`)
    pub rdo_pg1i3short_period_yes: bool,
    /// BIR: `frm1701:rdoPg1I6TaxpayerTypeC` (sample: `false`)
    pub rdo_pg1i6taxpayer_type_c: bool,
    /// BIR: `frm1701:rdoPg1I6TaxpayerTypeE` (sample: `false`)
    pub rdo_pg1i6taxpayer_type_e: bool,
    /// BIR: `frm1701:rdoPg1I6TaxpayerTypeP` (sample: `false`)
    pub rdo_pg1i6taxpayer_type_p: bool,
    /// BIR: `frm1701:rdoPg1I6TaxpayerTypeS` (sample: `true`)
    pub rdo_pg1i6taxpayer_type_s: bool,
    /// BIR: `frm1701:rdoPg1I6TaxpayerTypeT` (sample: `false`)
    pub rdo_pg1i6taxpayer_type_t: bool,
    /// BIR: `frm1701:rdoPg1I7ATC_II011` (sample: `false`)
    pub rdo_pg1i7atc_ii011: bool,
    /// BIR: `frm1701:rdoPg1I7ATC_II012` (sample: `true`)
    pub rdo_pg1i7atc_ii012: bool,
    /// BIR: `frm1701:rdoPg1I7ATC_II013` (sample: `false`)
    pub rdo_pg1i7atc_ii013: bool,
    /// BIR: `frm1701:rdoPg1I7ATC_II014` (sample: `false`)
    pub rdo_pg1i7atc_ii014: bool,
    /// BIR: `frm1701:rdoPg1I7ATC_II015` (sample: `false`)
    pub rdo_pg1i7atc_ii015: bool,
    /// BIR: `frm1701:rdoPg1I7ATC_II016` (sample: `false`)
    pub rdo_pg1i7atc_ii016: bool,
    /// BIR: `frm1701:rdoPg1I7ATC_II017` (sample: `false`)
    pub rdo_pg1i7atc_ii017: bool,
    /// BIR: `frm1701:rdoPg1OverpaymentCarryOver` (sample: `false`)
    pub rdo_pg1overpayment_carry_over: bool,
    /// BIR: `frm1701:rdoPg1OverpaymentRefund` (sample: `false`)
    pub rdo_pg1overpayment_refund: bool,
    /// BIR: `frm1701:rdoPg1OverpaymentTCC` (sample: `false`)
    pub rdo_pg1overpayment_tcc: bool,
    /// BIR: `frm1701:rdoPg1mOption1` (sample: `true`)
    pub rdo_pg1m_option1: bool,
    /// BIR: `frm1701:rdoPg1mOption2` (sample: `false`)
    pub rdo_pg1m_option2: bool,
    /// BIR: `frm1701:rdoPg2I10IncomeExemptNo` (sample: `false`)
    pub rdo_pg2i10income_exempt_no: bool,
    /// BIR: `frm1701:rdoPg2I10IncomeExemptYes` (sample: `false`)
    pub rdo_pg2i10income_exempt_yes: bool,
    /// BIR: `frm1701:rdoPg2I11IncomeSpecialNo` (sample: `false`)
    pub rdo_pg2i11income_special_no: bool,
    /// BIR: `frm1701:rdoPg2I11IncomeSpecialYes` (sample: `false`)
    pub rdo_pg2i11income_special_yes: bool,
    /// BIR: `frm1701:rdoPg2I12AMethodDeductionI` (sample: `false`)
    pub rdo_pg2i12amethod_deduction_i: bool,
    /// BIR: `frm1701:rdoPg2I12AMethodDeductionO` (sample: `false`)
    pub rdo_pg2i12amethod_deduction_o: bool,
    /// BIR: `frm1701:rdoPg2I12TaxRateG` (sample: `false`)
    pub rdo_pg2i12tax_rate_g: bool,
    /// BIR: `frm1701:rdoPg2I12TaxRateP` (sample: `false`)
    pub rdo_pg2i12tax_rate_p: bool,
    /// BIR: `frm1701:rdoPg2I3SpouseTypeC` (sample: `false`)
    pub rdo_pg2i3spouse_type_c: bool,
    /// BIR: `frm1701:rdoPg2I3SpouseTypeP` (sample: `false`)
    pub rdo_pg2i3spouse_type_p: bool,
    /// BIR: `frm1701:rdoPg2I3SpouseTypeS` (sample: `false`)
    pub rdo_pg2i3spouse_type_s: bool,
    /// BIR: `frm1701:rdoPg2I4ATC_II011` (sample: `false`)
    pub rdo_pg2i4atc_ii011: bool,
    /// BIR: `frm1701:rdoPg2I4ATC_II012` (sample: `false`)
    pub rdo_pg2i4atc_ii012: bool,
    /// BIR: `frm1701:rdoPg2I4ATC_II013` (sample: `false`)
    pub rdo_pg2i4atc_ii013: bool,
    /// BIR: `frm1701:rdoPg2I4ATC_II014` (sample: `false`)
    pub rdo_pg2i4atc_ii014: bool,
    /// BIR: `frm1701:rdoPg2I4ATC_II015` (sample: `false`)
    pub rdo_pg2i4atc_ii015: bool,
    /// BIR: `frm1701:rdoPg2I4ATC_II016` (sample: `false`)
    pub rdo_pg2i4atc_ii016: bool,
    /// BIR: `frm1701:rdoPg2I4ATC_II017` (sample: `false`)
    pub rdo_pg2i4atc_ii017: bool,
    /// BIR: `frm1701:rdoPg2I8ForeignTaxCreditsNo` (sample: `false`)
    pub rdo_pg2i8foreign_tax_credits_no: bool,
    /// BIR: `frm1701:rdoPg2I8ForeignTaxCreditsYes` (sample: `false`)
    pub rdo_pg2i8foreign_tax_credits_yes: bool,
    /// BIR: `frm1701:rdoPg3mExemptTYPE` (sample: `false`)
    pub rdo_pg3m_exempt_type: bool,
    /// BIR: `frm1701:rdoPg3mSpecialRateTYPE` (sample: `false`)
    pub rdo_pg3m_special_rate_type: bool,
    /// BIR: `frm1701:rdoSPAttachmentS` (sample: `false`)
    pub rdo_spattachment_s: bool,
    /// BIR: `frm1701:rdoSPAttachmentTF` (sample: `false`)
    pub rdo_spattachment_tf: bool,

    // === text_fields ===
    /// BIR: `frm1701:txtAttachmentTypes` (sample: ``)
    pub txt_attachment_types: String,
    /// BIR: `frm1701:txtCurrentPage` (sample: `1`)
    pub txt_current_page: u32,
    /// BIR: `frm1701:txtDisabledInputs` (sample: ``)
    pub txt_disabled_inputs: String,
    /// BIR: `frm1701:txtDisabledOnSave` (sample: ``)
    pub txt_disabled_on_save: String,
    /// BIR: `frm1701:txtEnabledInputsOnValidation` (sample: ``)
    pub txt_enabled_inputs_on_validation: String,
    /// BIR: `frm1701:txtEnabledLinks` (sample: ``)
    pub txt_enabled_links: String,
    /// BIR: `frm1701:txtEnabledOnSave` (sample: ``)
    pub txt_enabled_on_save: String,
    /// BIR: `frm1701:txtIsSpouseDisabled` (sample: ``)
    pub txt_is_spouse_disabled: String,
    /// BIR: `frm1701:txtIsTaxFilerDisabled` (sample: `FALSE`)
    pub txt_is_tax_filer_disabled: String,
    /// BIR: `frm1701:txtLineBus` (sample: `SOFTWARE%2520DEVELOPMENT`)
    pub txt_line_bus: String,
    /// BIR: `frm1701:txtMaxPage` (sample: `4`)
    pub txt_max_page: u32,
    /// BIR: `frm1701:txtPg1I10BirthDate` (sample: `08/17/1988`)
    pub txt_pg1i10birth_date: String,
    /// BIR: `frm1701:txtPg1I12Citizenship` (sample: `FILIPINO`)
    pub txt_pg1i12citizenship: String,
    /// BIR: `frm1701:txtPg1I14ForeignTaxNumber` (sample: ``)
    pub txt_pg1i14foreign_tax_number: String,
    /// BIR: `frm1701:txtPg1I22ATaxDue` (sample: `0.00`)
    pub txt_pg1i22atax_due: f64,
    /// BIR: `frm1701:txtPg1I22BTaxDue` (sample: `0.00`)
    pub txt_pg1i22btax_due: f64,
    /// BIR: `frm1701:txtPg1I235Number` (sample: ``)
    pub txt_pg1i235number: String,
    /// BIR: `frm1701:txtPg1I23A` (sample: `0.00`)
    pub txt_pg1i23a: f64,
    /// BIR: `frm1701:txtPg1I23B` (sample: `0.00`)
    pub txt_pg1i23b: f64,
    /// BIR: `frm1701:txtPg1I24ATaxPayable` (sample: `0.00`)
    pub txt_pg1i24atax_payable: f64,
    /// BIR: `frm1701:txtPg1I24BTaxPayable` (sample: `0.00`)
    pub txt_pg1i24btax_payable: f64,
    /// BIR: `frm1701:txtPg1I25A` (sample: `0.00`)
    pub txt_pg1i25a: f64,
    /// BIR: `frm1701:txtPg1I25B` (sample: `0.00`)
    pub txt_pg1i25b: f64,
    /// BIR: `frm1701:txtPg1I26A` (sample: `0.00`)
    pub txt_pg1i26a: f64,
    /// BIR: `frm1701:txtPg1I26B` (sample: `0.00`)
    pub txt_pg1i26b: f64,
    /// BIR: `frm1701:txtPg1I27A` (sample: `100.00`)
    pub txt_pg1i27a: f64,
    /// BIR: `frm1701:txtPg1I27B` (sample: `0.00`)
    pub txt_pg1i27b: f64,
    /// BIR: `frm1701:txtPg1I28A` (sample: `100.00`)
    pub txt_pg1i28a: f64,
    /// BIR: `frm1701:txtPg1I28B` (sample: `0.00`)
    pub txt_pg1i28b: f64,
    /// BIR: `frm1701:txtPg1I29A` (sample: `100.00`)
    pub txt_pg1i29a: f64,
    /// BIR: `frm1701:txtPg1I29B` (sample: `0.00`)
    pub txt_pg1i29b: f64,
    /// BIR: `frm1701:txtPg1I30A` (sample: `300.00`)
    pub txt_pg1i30a: f64,
    /// BIR: `frm1701:txtPg1I30B` (sample: `0.00`)
    pub txt_pg1i30b: f64,
    /// BIR: `frm1701:txtPg1I31ATotalAmtPyble` (sample: `300.00`)
    pub txt_pg1i31atotal_amt_pyble: f64,
    /// BIR: `frm1701:txtPg1I31BTotalAmtPyble` (sample: `0.00`)
    pub txt_pg1i31btotal_amt_pyble: f64,
    /// BIR: `frm1701:txtPg1I32AggregateAmtPyble` (sample: `300.00`)
    pub txt_pg1i32aggregate_amt_pyble: f64,
    /// BIR: `frm1701:txtPg1I33NumberOfAttachments` (sample: `00`)
    pub txt_pg1i33number_of_attachments: u32,
    /// BIR: `frm1701:txtPg1I34Agency` (sample: ``)
    pub txt_pg1i34agency: String,
    /// BIR: `frm1701:txtPg1I34Amount` (sample: ``)
    pub txt_pg1i34amount: String,
    /// BIR: `frm1701:txtPg1I34Date` (sample: ``)
    pub txt_pg1i34date: String,
    /// BIR: `frm1701:txtPg1I34Number` (sample: ``)
    pub txt_pg1i34number: String,
    /// BIR: `frm1701:txtPg1I35Agency` (sample: ``)
    pub txt_pg1i35agency: String,
    /// BIR: `frm1701:txtPg1I35Amount` (sample: ``)
    pub txt_pg1i35amount: String,
    /// BIR: `frm1701:txtPg1I35Date` (sample: ``)
    pub txt_pg1i35date: String,
    /// BIR: `frm1701:txtPg1I36Amount` (sample: ``)
    pub txt_pg1i36amount: String,
    /// BIR: `frm1701:txtPg1I36Date` (sample: ``)
    pub txt_pg1i36date: String,
    /// BIR: `frm1701:txtPg1I36Number` (sample: ``)
    pub txt_pg1i36number: String,
    /// BIR: `frm1701:txtPg1I37Agency` (sample: ``)
    pub txt_pg1i37agency: String,
    /// BIR: `frm1701:txtPg1I37Amount` (sample: ``)
    pub txt_pg1i37amount: String,
    /// BIR: `frm1701:txtPg1I37Date` (sample: ``)
    pub txt_pg1i37date: String,
    /// BIR: `frm1701:txtPg1I37Number` (sample: ``)
    pub txt_pg1i37number: String,
    /// BIR: `frm1701:txtPg1I37Particular` (sample: ``)
    pub txt_pg1i37particular: String,
    /// BIR: `frm1701:txtPg1I9Address` (sample: `OLONGAPO`)
    pub txt_pg1i9address: String,
    /// BIR: `frm1701:txtPg1mI10CSchdB` (sample: `0.00`)
    pub txt_pg1m_i10cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI10DSchdB` (sample: `0.00`)
    pub txt_pg1m_i10dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI10GSchdB` (sample: `0.00`)
    pub txt_pg1m_i10gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI10HSchdB` (sample: `0.00`)
    pub txt_pg1m_i10hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI11ASchdB` (sample: `0.00`)
    pub txt_pg1m_i11aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI11BSchdB` (sample: `0.00`)
    pub txt_pg1m_i11bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI11CSchdB` (sample: `0.00`)
    pub txt_pg1m_i11cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI11DSchdB` (sample: `0.00`)
    pub txt_pg1m_i11dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI11ESchdB` (sample: `0.00`)
    pub txt_pg1m_i11eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI11FSchdB` (sample: `0.00`)
    pub txt_pg1m_i11fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI11GSchdB` (sample: `0.00`)
    pub txt_pg1m_i11gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI11HSchdB` (sample: `0.00`)
    pub txt_pg1m_i11hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI12ASchdB` (sample: `0.00`)
    pub txt_pg1m_i12aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI12BSchdB` (sample: `0.00`)
    pub txt_pg1m_i12bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI12CSchdB` (sample: `0.00`)
    pub txt_pg1m_i12cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI12DSchdB` (sample: `0.00`)
    pub txt_pg1m_i12dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI12DescSchdB` (sample: ``)
    pub txt_pg1m_i12desc_schd_b: String,
    /// BIR: `frm1701:txtPg1mI12ESchdB` (sample: `0.00`)
    pub txt_pg1m_i12eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI12FSchdB` (sample: `0.00`)
    pub txt_pg1m_i12fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI12GSchdB` (sample: `0.00`)
    pub txt_pg1m_i12gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI12HSchdB` (sample: `0.00`)
    pub txt_pg1m_i12hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI13ASchdB` (sample: `0.00`)
    pub txt_pg1m_i13aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI13BSchdB` (sample: `0.00`)
    pub txt_pg1m_i13bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI13CSchdB` (sample: `0.00`)
    pub txt_pg1m_i13cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI13DSchdB` (sample: `0.00`)
    pub txt_pg1m_i13dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI13DescSchdB` (sample: ``)
    pub txt_pg1m_i13desc_schd_b: String,
    /// BIR: `frm1701:txtPg1mI13ESchdB` (sample: `0.00`)
    pub txt_pg1m_i13eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI13FSchdB` (sample: `0.00`)
    pub txt_pg1m_i13fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI13GSchdB` (sample: `0.00`)
    pub txt_pg1m_i13gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI13HSchdB` (sample: `0.00`)
    pub txt_pg1m_i13hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI14CSchdB` (sample: `0.00`)
    pub txt_pg1m_i14cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI14DSchdB` (sample: `0.00`)
    pub txt_pg1m_i14dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI14GSchdB` (sample: `0.00`)
    pub txt_pg1m_i14gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI14HSchdB` (sample: `0.00`)
    pub txt_pg1m_i14hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI15ASchdB` (sample: `0.00`)
    pub txt_pg1m_i15aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI15BSchdB` (sample: `0.00`)
    pub txt_pg1m_i15bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI15CSchdB` (sample: `0.00`)
    pub txt_pg1m_i15cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI15DSchdB` (sample: `0.00`)
    pub txt_pg1m_i15dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI15ESchdB` (sample: `0.00`)
    pub txt_pg1m_i15eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI15FSchdB` (sample: `0.00`)
    pub txt_pg1m_i15fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI15GSchdB` (sample: `0.00`)
    pub txt_pg1m_i15gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI15HSchdB` (sample: `0.00`)
    pub txt_pg1m_i15hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI16ASchdB` (sample: `0.00`)
    pub txt_pg1m_i16aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI16BSchdB` (sample: `0.00`)
    pub txt_pg1m_i16bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI16CSchdB` (sample: `0.00`)
    pub txt_pg1m_i16cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI16DSchdB` (sample: `0.00`)
    pub txt_pg1m_i16dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI16ESchdB` (sample: `0.00`)
    pub txt_pg1m_i16eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI16FSchdB` (sample: `0.00`)
    pub txt_pg1m_i16fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI16GSchdB` (sample: `0.00`)
    pub txt_pg1m_i16gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI16HSchdB` (sample: `0.00`)
    pub txt_pg1m_i16hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI17ASchdB` (sample: `0.00`)
    pub txt_pg1m_i17aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI17BSchdB` (sample: `0.00`)
    pub txt_pg1m_i17bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI17CSchdB` (sample: `0.00`)
    pub txt_pg1m_i17cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI17DSchdB` (sample: `0.00`)
    pub txt_pg1m_i17dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI17ESchdB` (sample: `0.00`)
    pub txt_pg1m_i17eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI17FSchdB` (sample: `0.00`)
    pub txt_pg1m_i17fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI17GSchdB` (sample: `0.00`)
    pub txt_pg1m_i17gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI17HSchdB` (sample: `0.00`)
    pub txt_pg1m_i17hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI1ASchdA` (sample: ``)
    pub txt_pg1m_i1aschd_a: String,
    /// BIR: `frm1701:txtPg1mI1ASchdB` (sample: `0.00`)
    pub txt_pg1m_i1aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI1BSchdA` (sample: ``)
    pub txt_pg1m_i1bschd_a: String,
    /// BIR: `frm1701:txtPg1mI1BSchdB` (sample: `0.00`)
    pub txt_pg1m_i1bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI1CSchdA` (sample: ``)
    pub txt_pg1m_i1cschd_a: String,
    /// BIR: `frm1701:txtPg1mI1CSchdB` (sample: `0.00`)
    pub txt_pg1m_i1cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI1DSchdA` (sample: ``)
    pub txt_pg1m_i1dschd_a: String,
    /// BIR: `frm1701:txtPg1mI1DSchdB` (sample: `0.00`)
    pub txt_pg1m_i1dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI1ESchdA` (sample: ``)
    pub txt_pg1m_i1eschd_a: String,
    /// BIR: `frm1701:txtPg1mI1ESchdB` (sample: `0.00`)
    pub txt_pg1m_i1eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI1FSchdA` (sample: ``)
    pub txt_pg1m_i1fschd_a: String,
    /// BIR: `frm1701:txtPg1mI1FSchdB` (sample: `0.00`)
    pub txt_pg1m_i1fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI1GSchdB` (sample: `0.00`)
    pub txt_pg1m_i1gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI1HSchdB` (sample: `0.00`)
    pub txt_pg1m_i1hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI2ASchdA` (sample: ``)
    pub txt_pg1m_i2aschd_a: String,
    /// BIR: `frm1701:txtPg1mI2ASchdB` (sample: `0.00`)
    pub txt_pg1m_i2aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI2BSchdA` (sample: ``)
    pub txt_pg1m_i2bschd_a: String,
    /// BIR: `frm1701:txtPg1mI2BSchdB` (sample: `0.00`)
    pub txt_pg1m_i2bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI2CSchdA` (sample: ``)
    pub txt_pg1m_i2cschd_a: String,
    /// BIR: `frm1701:txtPg1mI2CSchdB` (sample: `0.00`)
    pub txt_pg1m_i2cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI2DSchdA` (sample: ``)
    pub txt_pg1m_i2dschd_a: String,
    /// BIR: `frm1701:txtPg1mI2DSchdB` (sample: `0.00`)
    pub txt_pg1m_i2dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI2ESchdA` (sample: ``)
    pub txt_pg1m_i2eschd_a: String,
    /// BIR: `frm1701:txtPg1mI2ESchdB` (sample: `0.00`)
    pub txt_pg1m_i2eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI2FSchdA` (sample: ``)
    pub txt_pg1m_i2fschd_a: String,
    /// BIR: `frm1701:txtPg1mI2FSchdB` (sample: `0.00`)
    pub txt_pg1m_i2fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI2GSchdB` (sample: `0.00`)
    pub txt_pg1m_i2gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI2HSchdB` (sample: `0.00`)
    pub txt_pg1m_i2hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI3ASchdA` (sample: ``)
    pub txt_pg1m_i3aschd_a: String,
    /// BIR: `frm1701:txtPg1mI3ASchdB` (sample: `0.00`)
    pub txt_pg1m_i3aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI3BSchdA` (sample: ``)
    pub txt_pg1m_i3bschd_a: String,
    /// BIR: `frm1701:txtPg1mI3BSchdB` (sample: `0.00`)
    pub txt_pg1m_i3bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI3CSchdA` (sample: ``)
    pub txt_pg1m_i3cschd_a: String,
    /// BIR: `frm1701:txtPg1mI3CSchdB` (sample: `0.00`)
    pub txt_pg1m_i3cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI3DSchdA` (sample: ``)
    pub txt_pg1m_i3dschd_a: String,
    /// BIR: `frm1701:txtPg1mI3DSchdB` (sample: `0.00`)
    pub txt_pg1m_i3dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI3ESchdA` (sample: ``)
    pub txt_pg1m_i3eschd_a: String,
    /// BIR: `frm1701:txtPg1mI3ESchdB` (sample: `0.00`)
    pub txt_pg1m_i3eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI3FSchdA` (sample: ``)
    pub txt_pg1m_i3fschd_a: String,
    /// BIR: `frm1701:txtPg1mI3FSchdB` (sample: `0.00`)
    pub txt_pg1m_i3fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI3GSchdB` (sample: `0.00`)
    pub txt_pg1m_i3gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI3HSchdB` (sample: `0.00`)
    pub txt_pg1m_i3hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI4ASchdB` (sample: `0.00`)
    pub txt_pg1m_i4aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI4BSchdA` (sample: `0.00`)
    pub txt_pg1m_i4bschd_a: f64,
    /// BIR: `frm1701:txtPg1mI4BSchdB` (sample: `0.00`)
    pub txt_pg1m_i4bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI4CSchdB` (sample: `0.00`)
    pub txt_pg1m_i4cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI4DSchdB` (sample: `0.00`)
    pub txt_pg1m_i4dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI4ESchdA` (sample: `0.00`)
    pub txt_pg1m_i4eschd_a: f64,
    /// BIR: `frm1701:txtPg1mI4ESchdB` (sample: `0.00`)
    pub txt_pg1m_i4eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI4FSchdB` (sample: `0.00`)
    pub txt_pg1m_i4fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI4GSchdB` (sample: `0.00`)
    pub txt_pg1m_i4gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI4HSchdB` (sample: `0.00`)
    pub txt_pg1m_i4hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI5ASchdA` (sample: ``)
    pub txt_pg1m_i5aschd_a: String,
    /// BIR: `frm1701:txtPg1mI5ASchdB` (sample: `0.00`)
    pub txt_pg1m_i5aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI5BSchdA` (sample: ``)
    pub txt_pg1m_i5bschd_a: String,
    /// BIR: `frm1701:txtPg1mI5BSchdB` (sample: `0.00`)
    pub txt_pg1m_i5bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI5CSchdA` (sample: ``)
    pub txt_pg1m_i5cschd_a: String,
    /// BIR: `frm1701:txtPg1mI5CSchdB` (sample: `0.00`)
    pub txt_pg1m_i5cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI5DSchdA` (sample: ``)
    pub txt_pg1m_i5dschd_a: String,
    /// BIR: `frm1701:txtPg1mI5DSchdB` (sample: `0.00`)
    pub txt_pg1m_i5dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI5ESchdA` (sample: ``)
    pub txt_pg1m_i5eschd_a: String,
    /// BIR: `frm1701:txtPg1mI5ESchdB` (sample: `0.00`)
    pub txt_pg1m_i5eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI5FSchdA` (sample: ``)
    pub txt_pg1m_i5fschd_a: String,
    /// BIR: `frm1701:txtPg1mI5FSchdB` (sample: `0.00`)
    pub txt_pg1m_i5fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI5GSchdB` (sample: `0.00`)
    pub txt_pg1m_i5gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI5HSchdB` (sample: `0.00`)
    pub txt_pg1m_i5hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI6ASchdA` (sample: ``)
    pub txt_pg1m_i6aschd_a: String,
    /// BIR: `frm1701:txtPg1mI6ASchdB` (sample: `0.00`)
    pub txt_pg1m_i6aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI6BSchdA` (sample: ``)
    pub txt_pg1m_i6bschd_a: String,
    /// BIR: `frm1701:txtPg1mI6BSchdB` (sample: `0.00`)
    pub txt_pg1m_i6bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI6CSchdA` (sample: ``)
    pub txt_pg1m_i6cschd_a: String,
    /// BIR: `frm1701:txtPg1mI6CSchdB` (sample: `0.00`)
    pub txt_pg1m_i6cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI6DSchdA` (sample: ``)
    pub txt_pg1m_i6dschd_a: String,
    /// BIR: `frm1701:txtPg1mI6DSchdB` (sample: `0.00`)
    pub txt_pg1m_i6dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI6ESchdA` (sample: ``)
    pub txt_pg1m_i6eschd_a: String,
    /// BIR: `frm1701:txtPg1mI6ESchdB` (sample: `0.00`)
    pub txt_pg1m_i6eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI6FSchdA` (sample: ``)
    pub txt_pg1m_i6fschd_a: String,
    /// BIR: `frm1701:txtPg1mI6FSchdB` (sample: `0.00`)
    pub txt_pg1m_i6fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI6GSchdB` (sample: `0.00`)
    pub txt_pg1m_i6gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI6HSchdB` (sample: `0.00`)
    pub txt_pg1m_i6hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI7ASchdB` (sample: `0.00`)
    pub txt_pg1m_i7aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI7BSchdB` (sample: `0.00`)
    pub txt_pg1m_i7bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI7CSchdB` (sample: `0.00`)
    pub txt_pg1m_i7cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI7DSchdB` (sample: `0.00`)
    pub txt_pg1m_i7dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI7ESchdB` (sample: `0.00`)
    pub txt_pg1m_i7eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI7FSchdB` (sample: `0.00`)
    pub txt_pg1m_i7fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI7GSchdB` (sample: `0.00`)
    pub txt_pg1m_i7gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI7HSchdB` (sample: `0.00`)
    pub txt_pg1m_i7hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI8CSchdB` (sample: `0.00`)
    pub txt_pg1m_i8cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI8DSchdB` (sample: `0.00`)
    pub txt_pg1m_i8dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI8GSchdB` (sample: `0.00`)
    pub txt_pg1m_i8gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI8HSchdB` (sample: `0.00`)
    pub txt_pg1m_i8hschd_b: f64,
    /// BIR: `frm1701:txtPg1mI9ASchdB` (sample: `0.00`)
    pub txt_pg1m_i9aschd_b: f64,
    /// BIR: `frm1701:txtPg1mI9BSchdB` (sample: `0.00`)
    pub txt_pg1m_i9bschd_b: f64,
    /// BIR: `frm1701:txtPg1mI9CSchdB` (sample: `0.00`)
    pub txt_pg1m_i9cschd_b: f64,
    /// BIR: `frm1701:txtPg1mI9DSchdB` (sample: `0.00`)
    pub txt_pg1m_i9dschd_b: f64,
    /// BIR: `frm1701:txtPg1mI9ESchdB` (sample: `0.00`)
    pub txt_pg1m_i9eschd_b: f64,
    /// BIR: `frm1701:txtPg1mI9FSchdB` (sample: `0.00`)
    pub txt_pg1m_i9fschd_b: f64,
    /// BIR: `frm1701:txtPg1mI9GSchdB` (sample: `0.00`)
    pub txt_pg1m_i9gschd_b: f64,
    /// BIR: `frm1701:txtPg1mI9HSchdB` (sample: `0.00`)
    pub txt_pg1m_i9hschd_b: f64,
    /// BIR: `frm1701:txtPg2I5SpouseName` (sample: ``)
    pub txt_pg2i5spouse_name: String,
    /// BIR: `frm1701:txtPg2I7Citizenship` (sample: ``)
    pub txt_pg2i7citizenship: String,
    /// BIR: `frm1701:txtPg2I9ForeignTaxNumber` (sample: ``)
    pub txt_pg2i9foreign_tax_number: String,
    /// BIR: `frm1701:txtPg2IShed1a_1SName` (sample: ``)
    pub txt_pg2ished1a_1sname: String,
    /// BIR: `frm1701:txtPg2IShed1a_1TPName` (sample: ``)
    pub txt_pg2ished1a_1tpname: String,
    /// BIR: `frm1701:txtPg2IShed1c_1CI` (sample: `0.00`)
    pub txt_pg2ished1c_1ci: f64,
    /// BIR: `frm1701:txtPg2IShed1c_1TW` (sample: `0.00`)
    pub txt_pg2ished1c_1tw: f64,
    /// BIR: `frm1701:txtPg2IShed1c_2CI` (sample: `0.00`)
    pub txt_pg2ished1c_2ci: f64,
    /// BIR: `frm1701:txtPg2IShed1c_2TW` (sample: `0.00`)
    pub txt_pg2ished1c_2tw: f64,
    /// BIR: `frm1701:txtPg2IShed1c_3ACI` (sample: `0.00`)
    pub txt_pg2ished1c_3aci: f64,
    /// BIR: `frm1701:txtPg2IShed1c_3ATW` (sample: `0.00`)
    pub txt_pg2ished1c_3atw: f64,
    /// BIR: `frm1701:txtPg2IShed1c_3BCI` (sample: `0.00`)
    pub txt_pg2ished1c_3bci: f64,
    /// BIR: `frm1701:txtPg2IShed1c_3BTW` (sample: `0.00`)
    pub txt_pg2ished1c_3btw: f64,
    /// BIR: `frm1701:txtPg2IShed2_4A` (sample: `0.00`)
    pub txt_pg2ished2_4a: f64,
    /// BIR: `frm1701:txtPg2IShed2_4B` (sample: `0.00`)
    pub txt_pg2ished2_4b: f64,
    /// BIR: `frm1701:txtPg2IShed2_5A` (sample: `0.00`)
    pub txt_pg2ished2_5a: f64,
    /// BIR: `frm1701:txtPg2IShed2_5B` (sample: `0.00`)
    pub txt_pg2ished2_5b: f64,
    /// BIR: `frm1701:txtPg2IShed2_6A` (sample: `0.00`)
    pub txt_pg2ished2_6a: f64,
    /// BIR: `frm1701:txtPg2IShed2_6B` (sample: `0.00`)
    pub txt_pg2ished2_6b: f64,
    /// BIR: `frm1701:txtPg2IShed2_7A` (sample: `0.00`)
    pub txt_pg2ished2_7a: f64,
    /// BIR: `frm1701:txtPg2IShed2_7B` (sample: `0.00`)
    pub txt_pg2ished2_7b: f64,
    /// BIR: `frm1701:txtPg2IShed2a_2SName` (sample: ``)
    pub txt_pg2ished2a_2sname: String,
    /// BIR: `frm1701:txtPg2IShed2a_2TPName` (sample: ``)
    pub txt_pg2ished2a_2tpname: String,
    /// BIR: `frm1701:txtPg2IShed3_10A` (sample: `0.00`)
    pub txt_pg2ished3_10a: f64,
    /// BIR: `frm1701:txtPg2IShed3_10B` (sample: `0.00`)
    pub txt_pg2ished3_10b: f64,
    /// BIR: `frm1701:txtPg2IShed3_11A` (sample: `0.00`)
    pub txt_pg2ished3_11a: f64,
    /// BIR: `frm1701:txtPg2IShed3_11B` (sample: `0.00`)
    pub txt_pg2ished3_11b: f64,
    /// BIR: `frm1701:txtPg2IShed3_12A` (sample: `0.00`)
    pub txt_pg2ished3_12a: f64,
    /// BIR: `frm1701:txtPg2IShed3_12B` (sample: `0.00`)
    pub txt_pg2ished3_12b: f64,
    /// BIR: `frm1701:txtPg2IShed3_13A` (sample: `0.00`)
    pub txt_pg2ished3_13a: f64,
    /// BIR: `frm1701:txtPg2IShed3_13B` (sample: `0.00`)
    pub txt_pg2ished3_13b: f64,
    /// BIR: `frm1701:txtPg2IShed3_14A` (sample: `0.00`)
    pub txt_pg2ished3_14a: f64,
    /// BIR: `frm1701:txtPg2IShed3_14B` (sample: `0.00`)
    pub txt_pg2ished3_14b: f64,
    /// BIR: `frm1701:txtPg2IShed3_15A` (sample: `0.00`)
    pub txt_pg2ished3_15a: f64,
    /// BIR: `frm1701:txtPg2IShed3_15B` (sample: `0.00`)
    pub txt_pg2ished3_15b: f64,
    /// BIR: `frm1701:txtPg2IShed3_16A` (sample: `0.00`)
    pub txt_pg2ished3_16a: f64,
    /// BIR: `frm1701:txtPg2IShed3_16B` (sample: `0.00`)
    pub txt_pg2ished3_16b: f64,
    /// BIR: `frm1701:txtPg2IShed3_17A` (sample: `0.00`)
    pub txt_pg2ished3_17a: f64,
    /// BIR: `frm1701:txtPg2IShed3_17B` (sample: `0.00`)
    pub txt_pg2ished3_17b: f64,
    /// BIR: `frm1701:txtPg2IShed3_18A` (sample: `0.00`)
    pub txt_pg2ished3_18a: f64,
    /// BIR: `frm1701:txtPg2IShed3_18B` (sample: `0.00`)
    pub txt_pg2ished3_18b: f64,
    /// BIR: `frm1701:txtPg2IShed3_19A` (sample: `0.00`)
    pub txt_pg2ished3_19a: f64,
    /// BIR: `frm1701:txtPg2IShed3_19B` (sample: `0.00`)
    pub txt_pg2ished3_19b: f64,
    /// BIR: `frm1701:txtPg2IShed3_19Desc` (sample: ``)
    pub txt_pg2ished3_19desc: String,
    /// BIR: `frm1701:txtPg2IShed3_20A` (sample: `0.00`)
    pub txt_pg2ished3_20a: f64,
    /// BIR: `frm1701:txtPg2IShed3_20B` (sample: `0.00`)
    pub txt_pg2ished3_20b: f64,
    /// BIR: `frm1701:txtPg2IShed3_20Desc` (sample: ``)
    pub txt_pg2ished3_20desc: String,
    /// BIR: `frm1701:txtPg2IShed3_21A` (sample: `0.00`)
    pub txt_pg2ished3_21a: f64,
    /// BIR: `frm1701:txtPg2IShed3_21B` (sample: `0.00`)
    pub txt_pg2ished3_21b: f64,
    /// BIR: `frm1701:txtPg2IShed3_22A` (sample: `0.00`)
    pub txt_pg2ished3_22a: f64,
    /// BIR: `frm1701:txtPg2IShed3_22B` (sample: `0.00`)
    pub txt_pg2ished3_22b: f64,
    /// BIR: `frm1701:txtPg2IShed3_23A` (sample: `0.00`)
    pub txt_pg2ished3_23a: f64,
    /// BIR: `frm1701:txtPg2IShed3_23B` (sample: `0.00`)
    pub txt_pg2ished3_23b: f64,
    /// BIR: `frm1701:txtPg2IShed3_24A` (sample: `0.00`)
    pub txt_pg2ished3_24a: f64,
    /// BIR: `frm1701:txtPg2IShed3_24B` (sample: `0.00`)
    pub txt_pg2ished3_24b: f64,
    /// BIR: `frm1701:txtPg2IShed3_25A` (sample: `0.00`)
    pub txt_pg2ished3_25a: f64,
    /// BIR: `frm1701:txtPg2IShed3_25B` (sample: `0.00`)
    pub txt_pg2ished3_25b: f64,
    /// BIR: `frm1701:txtPg2IShed3_8A` (sample: `0.00`)
    pub txt_pg2ished3_8a: f64,
    /// BIR: `frm1701:txtPg2IShed3_8B` (sample: `0.00`)
    pub txt_pg2ished3_8b: f64,
    /// BIR: `frm1701:txtPg2IShed3_9A` (sample: `0.00`)
    pub txt_pg2ished3_9a: f64,
    /// BIR: `frm1701:txtPg2IShed3_9B` (sample: `0.00`)
    pub txt_pg2ished3_9b: f64,
    /// BIR: `frm1701:txtPg2mI10ASchdC` (sample: `0.00`)
    pub txt_pg2m_i10aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI10BSchdC` (sample: `0.00`)
    pub txt_pg2m_i10bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI10CSchdC` (sample: `0.00`)
    pub txt_pg2m_i10cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI10DSchdC` (sample: `0.00`)
    pub txt_pg2m_i10dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI11ASchdC` (sample: `0.00`)
    pub txt_pg2m_i11aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI11BSchdC` (sample: `0.00`)
    pub txt_pg2m_i11bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI11CSchdC` (sample: `0.00`)
    pub txt_pg2m_i11cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI11DSchdC` (sample: `0.00`)
    pub txt_pg2m_i11dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI12ASchdC` (sample: `0.00`)
    pub txt_pg2m_i12aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI12BSchdC` (sample: `0.00`)
    pub txt_pg2m_i12bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI12CSchdC` (sample: `0.00`)
    pub txt_pg2m_i12cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI12DSchdC` (sample: `0.00`)
    pub txt_pg2m_i12dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI13ASchdC` (sample: `0.00`)
    pub txt_pg2m_i13aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI13BSchdC` (sample: `0.00`)
    pub txt_pg2m_i13bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI13CSchdC` (sample: `0.00`)
    pub txt_pg2m_i13cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI13DSchdC` (sample: `0.00`)
    pub txt_pg2m_i13dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI14ASchdC` (sample: `0.00`)
    pub txt_pg2m_i14aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI14BSchdC` (sample: `0.00`)
    pub txt_pg2m_i14bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI14CSchdC` (sample: `0.00`)
    pub txt_pg2m_i14cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI14DSchdC` (sample: `0.00`)
    pub txt_pg2m_i14dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI15ASchdC` (sample: `0.00`)
    pub txt_pg2m_i15aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI15BSchdC` (sample: `0.00`)
    pub txt_pg2m_i15bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI15CSchdC` (sample: `0.00`)
    pub txt_pg2m_i15cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI15DSchdC` (sample: `0.00`)
    pub txt_pg2m_i15dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI16ASchdC` (sample: `0.00`)
    pub txt_pg2m_i16aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI16BSchdC` (sample: `0.00`)
    pub txt_pg2m_i16bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI16CSchdC` (sample: `0.00`)
    pub txt_pg2m_i16cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI16DSchdC` (sample: `0.00`)
    pub txt_pg2m_i16dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17aASchdC` (sample: `0.00`)
    pub txt_pg2m_i17a_aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17aBSchdC` (sample: `0.00`)
    pub txt_pg2m_i17a_bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17aCSchdC` (sample: `0.00`)
    pub txt_pg2m_i17a_cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17aDSchdC` (sample: `0.00`)
    pub txt_pg2m_i17a_dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17bASchdC` (sample: `0.00`)
    pub txt_pg2m_i17b_aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17bBSchdC` (sample: `0.00`)
    pub txt_pg2m_i17b_bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17bCSchdC` (sample: `0.00`)
    pub txt_pg2m_i17b_cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17bDSchdC` (sample: `0.00`)
    pub txt_pg2m_i17b_dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17cASchdC` (sample: `0.00`)
    pub txt_pg2m_i17c_aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17cBSchdC` (sample: `0.00`)
    pub txt_pg2m_i17c_bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17cCSchdC` (sample: `0.00`)
    pub txt_pg2m_i17c_cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17cDSchdC` (sample: `0.00`)
    pub txt_pg2m_i17c_dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17dASchdC` (sample: `0.00`)
    pub txt_pg2m_i17d_aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17dBSchdC` (sample: `0.00`)
    pub txt_pg2m_i17d_bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17dCSchdC` (sample: `0.00`)
    pub txt_pg2m_i17d_cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17dDSchdC` (sample: `0.00`)
    pub txt_pg2m_i17d_dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI17dDescSchdC` (sample: ``)
    pub txt_pg2m_i17d_desc_schd_c: String,
    /// BIR: `frm1701:txtPg2mI18ASchdC` (sample: `0.00`)
    pub txt_pg2m_i18aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI18BSchdC` (sample: `0.00`)
    pub txt_pg2m_i18bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI18CSchdC` (sample: `0.00`)
    pub txt_pg2m_i18cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI18DSchdC` (sample: `0.00`)
    pub txt_pg2m_i18dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI1ASchdC` (sample: `0.00`)
    pub txt_pg2m_i1aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI1ASchdD` (sample: `0.00`)
    pub txt_pg2m_i1aschd_d: f64,
    /// BIR: `frm1701:txtPg2mI1BSchdC` (sample: `0.00`)
    pub txt_pg2m_i1bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI1BSchdD` (sample: `0.00`)
    pub txt_pg2m_i1bschd_d: f64,
    /// BIR: `frm1701:txtPg2mI1CSchdC` (sample: `0.00`)
    pub txt_pg2m_i1cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI1DSchdC` (sample: `0.00`)
    pub txt_pg2m_i1dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI1DescSchdD` (sample: ``)
    pub txt_pg2m_i1desc_schd_d: String,
    /// BIR: `frm1701:txtPg2mI1LBSchdD` (sample: ``)
    pub txt_pg2m_i1lbschd_d: String,
    /// BIR: `frm1701:txtPg2mI2ASchdC` (sample: `0.00`)
    pub txt_pg2m_i2aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI2ASchdD` (sample: `0.00`)
    pub txt_pg2m_i2aschd_d: f64,
    /// BIR: `frm1701:txtPg2mI2BSchdC` (sample: `0.00`)
    pub txt_pg2m_i2bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI2BSchdD` (sample: `0.00`)
    pub txt_pg2m_i2bschd_d: f64,
    /// BIR: `frm1701:txtPg2mI2CSchdC` (sample: `0.00`)
    pub txt_pg2m_i2cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI2DSchdC` (sample: `0.00`)
    pub txt_pg2m_i2dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI2DescSchdD` (sample: ``)
    pub txt_pg2m_i2desc_schd_d: String,
    /// BIR: `frm1701:txtPg2mI2LBSchdD` (sample: ``)
    pub txt_pg2m_i2lbschd_d: String,
    /// BIR: `frm1701:txtPg2mI3ASchdC` (sample: `0.00`)
    pub txt_pg2m_i3aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI3ASchdD` (sample: `0.00`)
    pub txt_pg2m_i3aschd_d: f64,
    /// BIR: `frm1701:txtPg2mI3BSchdC` (sample: `0.00`)
    pub txt_pg2m_i3bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI3BSchdD` (sample: `0.00`)
    pub txt_pg2m_i3bschd_d: f64,
    /// BIR: `frm1701:txtPg2mI3CSchdC` (sample: `0.00`)
    pub txt_pg2m_i3cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI3DSchdC` (sample: `0.00`)
    pub txt_pg2m_i3dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI3DescSchdD` (sample: ``)
    pub txt_pg2m_i3desc_schd_d: String,
    /// BIR: `frm1701:txtPg2mI3LBSchdD` (sample: ``)
    pub txt_pg2m_i3lbschd_d: String,
    /// BIR: `frm1701:txtPg2mI4ASchdC` (sample: `0.00`)
    pub txt_pg2m_i4aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI4ASchdD` (sample: `0.00`)
    pub txt_pg2m_i4aschd_d: f64,
    /// BIR: `frm1701:txtPg2mI4BSchdC` (sample: `0.00`)
    pub txt_pg2m_i4bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI4BSchdD` (sample: `0.00`)
    pub txt_pg2m_i4bschd_d: f64,
    /// BIR: `frm1701:txtPg2mI4CSchdC` (sample: `0.00`)
    pub txt_pg2m_i4cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI4DSchdC` (sample: `0.00`)
    pub txt_pg2m_i4dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI4DescSchdD` (sample: ``)
    pub txt_pg2m_i4desc_schd_d: String,
    /// BIR: `frm1701:txtPg2mI4LBSchdD` (sample: ``)
    pub txt_pg2m_i4lbschd_d: String,
    /// BIR: `frm1701:txtPg2mI5ASchdC` (sample: `0.00`)
    pub txt_pg2m_i5aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI5ASchdD` (sample: `0.00`)
    pub txt_pg2m_i5aschd_d: f64,
    /// BIR: `frm1701:txtPg2mI5BSchdC` (sample: `0.00`)
    pub txt_pg2m_i5bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI5BSchdD` (sample: `0.00`)
    pub txt_pg2m_i5bschd_d: f64,
    /// BIR: `frm1701:txtPg2mI5CSchdC` (sample: `0.00`)
    pub txt_pg2m_i5cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI5DSchdC` (sample: `0.00`)
    pub txt_pg2m_i5dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI6ASchdC` (sample: `0.00`)
    pub txt_pg2m_i6aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI6BSchdC` (sample: `0.00`)
    pub txt_pg2m_i6bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI6CSchdC` (sample: `0.00`)
    pub txt_pg2m_i6cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI6DSchdC` (sample: `0.00`)
    pub txt_pg2m_i6dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI7ASchdC` (sample: `0.00`)
    pub txt_pg2m_i7aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI7BSchdC` (sample: `0.00`)
    pub txt_pg2m_i7bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI7CSchdC` (sample: `0.00`)
    pub txt_pg2m_i7cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI7DSchdC` (sample: `0.00`)
    pub txt_pg2m_i7dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI8ASchdC` (sample: `0.00`)
    pub txt_pg2m_i8aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI8BSchdC` (sample: `0.00`)
    pub txt_pg2m_i8bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI8CSchdC` (sample: `0.00`)
    pub txt_pg2m_i8cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI8DSchdC` (sample: `0.00`)
    pub txt_pg2m_i8dschd_c: f64,
    /// BIR: `frm1701:txtPg2mI9ASchdC` (sample: `0.00`)
    pub txt_pg2m_i9aschd_c: f64,
    /// BIR: `frm1701:txtPg2mI9BSchdC` (sample: `0.00`)
    pub txt_pg2m_i9bschd_c: f64,
    /// BIR: `frm1701:txtPg2mI9CSchdC` (sample: `0.00`)
    pub txt_pg2m_i9cschd_c: f64,
    /// BIR: `frm1701:txtPg2mI9DSchdC` (sample: `0.00`)
    pub txt_pg2m_i9dschd_c: f64,
    /// BIR: `frm1701:txtPg3IShed3_26A` (sample: `0.00`)
    pub txt_pg3ished3_26a: f64,
    /// BIR: `frm1701:txtPg3IShed3_26B` (sample: `0.00`)
    pub txt_pg3ished3_26b: f64,
    /// BIR: `frm1701:txtPg3IShed3_27A` (sample: `0.00`)
    pub txt_pg3ished3_27a: f64,
    /// BIR: `frm1701:txtPg3IShed3_27B` (sample: `0.00`)
    pub txt_pg3ished3_27b: f64,
    /// BIR: `frm1701:txtPg3IShed3_27Desc` (sample: ``)
    pub txt_pg3ished3_27desc: String,
    /// BIR: `frm1701:txtPg3IShed3_28A` (sample: `0.00`)
    pub txt_pg3ished3_28a: f64,
    /// BIR: `frm1701:txtPg3IShed3_28B` (sample: `0.00`)
    pub txt_pg3ished3_28b: f64,
    /// BIR: `frm1701:txtPg3IShed3_29A` (sample: `0.00`)
    pub txt_pg3ished3_29a: f64,
    /// BIR: `frm1701:txtPg3IShed3_29B` (sample: `0.00`)
    pub txt_pg3ished3_29b: f64,
    /// BIR: `frm1701:txtPg3IShed3_30A` (sample: `0.00`)
    pub txt_pg3ished3_30a: f64,
    /// BIR: `frm1701:txtPg3IShed3_30B` (sample: `0.00`)
    pub txt_pg3ished3_30b: f64,
    /// BIR: `frm1701:txtPg3IShed3_31A` (sample: `0.00`)
    pub txt_pg3ished3_31a: f64,
    /// BIR: `frm1701:txtPg3IShed3_31B` (sample: `0.00`)
    pub txt_pg3ished3_31b: f64,
    /// BIR: `frm1701:txtPg3IShed3_32A` (sample: `0.00`)
    pub txt_pg3ished3_32a: f64,
    /// BIR: `frm1701:txtPg3IShed3_32B` (sample: `0.00`)
    pub txt_pg3ished3_32b: f64,
    /// BIR: `frm1701:txtPg3IShed4_10A` (sample: `0.00`)
    pub txt_pg3ished4_10a: f64,
    /// BIR: `frm1701:txtPg3IShed4_10B` (sample: `0.00`)
    pub txt_pg3ished4_10b: f64,
    /// BIR: `frm1701:txtPg3IShed4_11A` (sample: `0.00`)
    pub txt_pg3ished4_11a: f64,
    /// BIR: `frm1701:txtPg3IShed4_11B` (sample: `0.00`)
    pub txt_pg3ished4_11b: f64,
    /// BIR: `frm1701:txtPg3IShed4_12A` (sample: `0.00`)
    pub txt_pg3ished4_12a: f64,
    /// BIR: `frm1701:txtPg3IShed4_12B` (sample: `0.00`)
    pub txt_pg3ished4_12b: f64,
    /// BIR: `frm1701:txtPg3IShed4_13A` (sample: `0.00`)
    pub txt_pg3ished4_13a: f64,
    /// BIR: `frm1701:txtPg3IShed4_13B` (sample: `0.00`)
    pub txt_pg3ished4_13b: f64,
    /// BIR: `frm1701:txtPg3IShed4_14A` (sample: `0.00`)
    pub txt_pg3ished4_14a: f64,
    /// BIR: `frm1701:txtPg3IShed4_14B` (sample: `0.00`)
    pub txt_pg3ished4_14b: f64,
    /// BIR: `frm1701:txtPg3IShed4_15A` (sample: `0.00`)
    pub txt_pg3ished4_15a: f64,
    /// BIR: `frm1701:txtPg3IShed4_15B` (sample: `0.00`)
    pub txt_pg3ished4_15b: f64,
    /// BIR: `frm1701:txtPg3IShed4_16A` (sample: `0.00`)
    pub txt_pg3ished4_16a: f64,
    /// BIR: `frm1701:txtPg3IShed4_16B` (sample: `0.00`)
    pub txt_pg3ished4_16b: f64,
    /// BIR: `frm1701:txtPg3IShed4_17aA` (sample: `0.00`)
    pub txt_pg3ished4_17a_a: f64,
    /// BIR: `frm1701:txtPg3IShed4_17aB` (sample: `0.00`)
    pub txt_pg3ished4_17a_b: f64,
    /// BIR: `frm1701:txtPg3IShed4_17bA` (sample: `0.00`)
    pub txt_pg3ished4_17b_a: f64,
    /// BIR: `frm1701:txtPg3IShed4_17bB` (sample: `0.00`)
    pub txt_pg3ished4_17b_b: f64,
    /// BIR: `frm1701:txtPg3IShed4_17cA` (sample: `0.00`)
    pub txt_pg3ished4_17c_a: f64,
    /// BIR: `frm1701:txtPg3IShed4_17cB` (sample: `0.00`)
    pub txt_pg3ished4_17c_b: f64,
    /// BIR: `frm1701:txtPg3IShed4_17dA` (sample: `0.00`)
    pub txt_pg3ished4_17d_a: f64,
    /// BIR: `frm1701:txtPg3IShed4_17dB` (sample: `0.00`)
    pub txt_pg3ished4_17d_b: f64,
    /// BIR: `frm1701:txtPg3IShed4_17dDesc` (sample: ``)
    pub txt_pg3ished4_17d_desc: String,
    /// BIR: `frm1701:txtPg3IShed4_18A` (sample: `0.00`)
    pub txt_pg3ished4_18a: f64,
    /// BIR: `frm1701:txtPg3IShed4_18B` (sample: `0.00`)
    pub txt_pg3ished4_18b: f64,
    /// BIR: `frm1701:txtPg3IShed4_1A` (sample: `0.00`)
    pub txt_pg3ished4_1a: f64,
    /// BIR: `frm1701:txtPg3IShed4_1B` (sample: `0.00`)
    pub txt_pg3ished4_1b: f64,
    /// BIR: `frm1701:txtPg3IShed4_2A` (sample: `0.00`)
    pub txt_pg3ished4_2a: f64,
    /// BIR: `frm1701:txtPg3IShed4_2B` (sample: `0.00`)
    pub txt_pg3ished4_2b: f64,
    /// BIR: `frm1701:txtPg3IShed4_3A` (sample: `0.00`)
    pub txt_pg3ished4_3a: f64,
    /// BIR: `frm1701:txtPg3IShed4_3B` (sample: `0.00`)
    pub txt_pg3ished4_3b: f64,
    /// BIR: `frm1701:txtPg3IShed4_4A` (sample: `0.00`)
    pub txt_pg3ished4_4a: f64,
    /// BIR: `frm1701:txtPg3IShed4_4B` (sample: `0.00`)
    pub txt_pg3ished4_4b: f64,
    /// BIR: `frm1701:txtPg3IShed4_5A` (sample: `0.00`)
    pub txt_pg3ished4_5a: f64,
    /// BIR: `frm1701:txtPg3IShed4_5B` (sample: `0.00`)
    pub txt_pg3ished4_5b: f64,
    /// BIR: `frm1701:txtPg3IShed4_6A` (sample: `0.00`)
    pub txt_pg3ished4_6a: f64,
    /// BIR: `frm1701:txtPg3IShed4_6B` (sample: `0.00`)
    pub txt_pg3ished4_6b: f64,
    /// BIR: `frm1701:txtPg3IShed4_7A` (sample: `0.00`)
    pub txt_pg3ished4_7a: f64,
    /// BIR: `frm1701:txtPg3IShed4_7B` (sample: `0.00`)
    pub txt_pg3ished4_7b: f64,
    /// BIR: `frm1701:txtPg3IShed4_8A` (sample: `0.00`)
    pub txt_pg3ished4_8a: f64,
    /// BIR: `frm1701:txtPg3IShed4_8B` (sample: `0.00`)
    pub txt_pg3ished4_8b: f64,
    /// BIR: `frm1701:txtPg3IShed4_9A` (sample: `0.00`)
    pub txt_pg3ished4_9a: f64,
    /// BIR: `frm1701:txtPg3IShed4_9B` (sample: `0.00`)
    pub txt_pg3ished4_9b: f64,
    /// BIR: `frm1701:txtPg3IShed5_1Amt` (sample: `0.00`)
    pub txt_pg3ished5_1amt: f64,
    /// BIR: `frm1701:txtPg3IShed5_1Desc` (sample: ``)
    pub txt_pg3ished5_1desc: String,
    /// BIR: `frm1701:txtPg3IShed5_1Legal` (sample: ``)
    pub txt_pg3ished5_1legal: String,
    /// BIR: `frm1701:txtPg3IShed5_2Amt` (sample: `0.00`)
    pub txt_pg3ished5_2amt: f64,
    /// BIR: `frm1701:txtPg3IShed5_2Desc` (sample: ``)
    pub txt_pg3ished5_2desc: String,
    /// BIR: `frm1701:txtPg3IShed5_2Legal` (sample: ``)
    pub txt_pg3ished5_2legal: String,
    /// BIR: `frm1701:txtPg3IShed5_3` (sample: `0.00`)
    pub txt_pg3ished5_3: f64,
    /// BIR: `frm1701:txtPg3IShed5_4Amt` (sample: `0.00`)
    pub txt_pg3ished5_4amt: f64,
    /// BIR: `frm1701:txtPg3IShed5_4Desc` (sample: ``)
    pub txt_pg3ished5_4desc: String,
    /// BIR: `frm1701:txtPg3IShed5_4Legal` (sample: ``)
    pub txt_pg3ished5_4legal: String,
    /// BIR: `frm1701:txtPg3IShed5_5Amt` (sample: `0.00`)
    pub txt_pg3ished5_5amt: f64,
    /// BIR: `frm1701:txtPg3IShed5_5Desc` (sample: ``)
    pub txt_pg3ished5_5desc: String,
    /// BIR: `frm1701:txtPg3IShed5_5Legal` (sample: ``)
    pub txt_pg3ished5_5legal: String,
    /// BIR: `frm1701:txtPg3IShed5_6` (sample: `0.00`)
    pub txt_pg3ished5_6: f64,
    /// BIR: `frm1701:txtPg3IShed6_1A` (sample: `0.00`)
    pub txt_pg3ished6_1a: f64,
    /// BIR: `frm1701:txtPg3IShed6_1B` (sample: `0.00`)
    pub txt_pg3ished6_1b: f64,
    /// BIR: `frm1701:txtPg3IShed6_2A` (sample: `0.00`)
    pub txt_pg3ished6_2a: f64,
    /// BIR: `frm1701:txtPg3IShed6_2B` (sample: `0.00`)
    pub txt_pg3ished6_2b: f64,
    /// BIR: `frm1701:txtPg3IShed6_3A` (sample: `0.00`)
    pub txt_pg3ished6_3a: f64,
    /// BIR: `frm1701:txtPg3IShed6_3B` (sample: `0.00`)
    pub txt_pg3ished6_3b: f64,
    /// BIR: `frm1701:txtPg3IShed6_4A` (sample: `0.00`)
    pub txt_pg3ished6_4a: f64,
    /// BIR: `frm1701:txtPg3IShed6_4B` (sample: `0.00`)
    pub txt_pg3ished6_4b: f64,
    /// BIR: `frm1701:txtPg3IShed6_4C` (sample: `0.00`)
    pub txt_pg3ished6_4c: f64,
    /// BIR: `frm1701:txtPg3IShed6_4D` (sample: `0.00`)
    pub txt_pg3ished6_4d: f64,
    /// BIR: `frm1701:txtPg3IShed6_4E` (sample: `0.00`)
    pub txt_pg3ished6_4e: f64,
    /// BIR: `frm1701:txtPg3IShed6_5A` (sample: `0.00`)
    pub txt_pg3ished6_5a: f64,
    /// BIR: `frm1701:txtPg3IShed6_5B` (sample: `0.00`)
    pub txt_pg3ished6_5b: f64,
    /// BIR: `frm1701:txtPg3IShed6_5C` (sample: `0.00`)
    pub txt_pg3ished6_5c: f64,
    /// BIR: `frm1701:txtPg3IShed6_5D` (sample: `0.00`)
    pub txt_pg3ished6_5d: f64,
    /// BIR: `frm1701:txtPg3IShed6_5E` (sample: `0.00`)
    pub txt_pg3ished6_5e: f64,
    /// BIR: `frm1701:txtPg3IShed6_6A` (sample: `0.00`)
    pub txt_pg3ished6_6a: f64,
    /// BIR: `frm1701:txtPg3IShed6_6B` (sample: `0.00`)
    pub txt_pg3ished6_6b: f64,
    /// BIR: `frm1701:txtPg3IShed6_6C` (sample: `0.00`)
    pub txt_pg3ished6_6c: f64,
    /// BIR: `frm1701:txtPg3IShed6_6D` (sample: `0.00`)
    pub txt_pg3ished6_6d: f64,
    /// BIR: `frm1701:txtPg3IShed6_6E` (sample: `0.00`)
    pub txt_pg3ished6_6e: f64,
    /// BIR: `frm1701:txtPg3IShed6_7A` (sample: `0.00`)
    pub txt_pg3ished6_7a: f64,
    /// BIR: `frm1701:txtPg3IShed6_7B` (sample: `0.00`)
    pub txt_pg3ished6_7b: f64,
    /// BIR: `frm1701:txtPg3IShed6_7C` (sample: `0.00`)
    pub txt_pg3ished6_7c: f64,
    /// BIR: `frm1701:txtPg3IShed6_7D` (sample: `0.00`)
    pub txt_pg3ished6_7d: f64,
    /// BIR: `frm1701:txtPg3IShed6_7E` (sample: `0.00`)
    pub txt_pg3ished6_7e: f64,
    /// BIR: `frm1701:txtPg3IShed6_8D` (sample: `0.00`)
    pub txt_pg3ished6_8d: f64,
    /// BIR: `frm1701:txtPg3mSchedA_1ATYPE` (sample: ``)
    pub txt_pg3m_sched_a_1atype: String,
    /// BIR: `frm1701:txtPg3mSchedA_1BTYPE` (sample: ``)
    pub txt_pg3m_sched_a_1btype: String,
    /// BIR: `frm1701:txtPg3mSchedA_2ATYPE` (sample: ``)
    pub txt_pg3m_sched_a_2atype: String,
    /// BIR: `frm1701:txtPg3mSchedA_2BTYPE` (sample: ``)
    pub txt_pg3m_sched_a_2btype: String,
    /// BIR: `frm1701:txtPg3mSchedA_3ATYPE` (sample: ``)
    pub txt_pg3m_sched_a_3atype: String,
    /// BIR: `frm1701:txtPg3mSchedA_3BTYPE` (sample: ``)
    pub txt_pg3m_sched_a_3btype: String,
    /// BIR: `frm1701:txtPg3mSchedA_4ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_a_4atype: f64,
    /// BIR: `frm1701:txtPg3mSchedA_4BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_a_4btype: f64,
    /// BIR: `frm1701:txtPg3mSchedA_5ATYPE` (sample: ``)
    pub txt_pg3m_sched_a_5atype: String,
    /// BIR: `frm1701:txtPg3mSchedA_5BTYPE` (sample: ``)
    pub txt_pg3m_sched_a_5btype: String,
    /// BIR: `frm1701:txtPg3mSchedA_6ATYPE` (sample: ``)
    pub txt_pg3m_sched_a_6atype: String,
    /// BIR: `frm1701:txtPg3mSchedA_6BTYPE` (sample: ``)
    pub txt_pg3m_sched_a_6btype: String,
    /// BIR: `frm1701:txtPg3mSchedB_10ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_10atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_10BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_10btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_10TYPE` (sample: ``)
    pub txt_pg3m_sched_b_10type: String,
    /// BIR: `frm1701:txtPg3mSchedB_11ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_11atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_11BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_11btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_11TYPE` (sample: ``)
    pub txt_pg3m_sched_b_11type: String,
    /// BIR: `frm1701:txtPg3mSchedB_12ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_12atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_12BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_12btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_13ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_13atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_13BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_13btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_14ATYPE` (sample: `0.0`)
    pub txt_pg3m_sched_b_14atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_14BTYPE` (sample: `0.0`)
    pub txt_pg3m_sched_b_14btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_15ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_15atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_15BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_15btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_1ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_1atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_1BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_1btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_2ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_2atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_2BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_2btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_3ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_3atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_3BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_3btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_4ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_4atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_4BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_4btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_5ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_5atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_5BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_5btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_6ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_6atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_6BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_6btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_7ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_7atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_7BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_7btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_8ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_8atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_8BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_8btype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_9ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_9atype: f64,
    /// BIR: `frm1701:txtPg3mSchedB_9BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_b_9btype: f64,
    /// BIR: `frm1701:txtPg3mSchedC_1ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_c_1atype: f64,
    /// BIR: `frm1701:txtPg3mSchedC_1BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_c_1btype: f64,
    /// BIR: `frm1701:txtPg3mSchedC_2ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_c_2atype: f64,
    /// BIR: `frm1701:txtPg3mSchedC_2BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_c_2btype: f64,
    /// BIR: `frm1701:txtPg3mSchedC_3ATYPE` (sample: `0.00`)
    pub txt_pg3m_sched_c_3atype: f64,
    /// BIR: `frm1701:txtPg3mSchedC_3BTYPE` (sample: `0.00`)
    pub txt_pg3m_sched_c_3btype: f64,
    /// BIR: `frm1701:txtPg4IPart7_10A` (sample: `0.00`)
    pub txt_pg4ipart7_10a: f64,
    /// BIR: `frm1701:txtPg4IPart7_10B` (sample: `0.00`)
    pub txt_pg4ipart7_10b: f64,
    /// BIR: `frm1701:txtPg4IPart7_1A` (sample: `0.00`)
    pub txt_pg4ipart7_1a: f64,
    /// BIR: `frm1701:txtPg4IPart7_1B` (sample: `0.00`)
    pub txt_pg4ipart7_1b: f64,
    /// BIR: `frm1701:txtPg4IPart7_2A` (sample: `0.00`)
    pub txt_pg4ipart7_2a: f64,
    /// BIR: `frm1701:txtPg4IPart7_2B` (sample: `0.00`)
    pub txt_pg4ipart7_2b: f64,
    /// BIR: `frm1701:txtPg4IPart7_3A` (sample: `0.00`)
    pub txt_pg4ipart7_3a: f64,
    /// BIR: `frm1701:txtPg4IPart7_3B` (sample: `0.00`)
    pub txt_pg4ipart7_3b: f64,
    /// BIR: `frm1701:txtPg4IPart7_4A` (sample: `0.00`)
    pub txt_pg4ipart7_4a: f64,
    /// BIR: `frm1701:txtPg4IPart7_4B` (sample: `0.00`)
    pub txt_pg4ipart7_4b: f64,
    /// BIR: `frm1701:txtPg4IPart7_5A` (sample: `0.00`)
    pub txt_pg4ipart7_5a: f64,
    /// BIR: `frm1701:txtPg4IPart7_5B` (sample: `0.00`)
    pub txt_pg4ipart7_5b: f64,
    /// BIR: `frm1701:txtPg4IPart7_6A` (sample: `0.00`)
    pub txt_pg4ipart7_6a: f64,
    /// BIR: `frm1701:txtPg4IPart7_6B` (sample: `0.00`)
    pub txt_pg4ipart7_6b: f64,
    /// BIR: `frm1701:txtPg4IPart7_7A` (sample: `0.00`)
    pub txt_pg4ipart7_7a: f64,
    /// BIR: `frm1701:txtPg4IPart7_7B` (sample: `0.00`)
    pub txt_pg4ipart7_7b: f64,
    /// BIR: `frm1701:txtPg4IPart7_8A` (sample: `0.00`)
    pub txt_pg4ipart7_8a: f64,
    /// BIR: `frm1701:txtPg4IPart7_8B` (sample: `0.00`)
    pub txt_pg4ipart7_8b: f64,
    /// BIR: `frm1701:txtPg4IPart7_9A` (sample: `0.00`)
    pub txt_pg4ipart7_9a: f64,
    /// BIR: `frm1701:txtPg4IPart7_9B` (sample: `0.00`)
    pub txt_pg4ipart7_9b: f64,
    /// BIR: `frm1701:txtPg4IPart7_9Specify` (sample: ``)
    pub txt_pg4ipart7_9specify: String,
    /// BIR: `frm1701:txtPg4IPart8_10A` (sample: `0.00`)
    pub txt_pg4ipart8_10a: f64,
    /// BIR: `frm1701:txtPg4IPart8_10B` (sample: `0.00`)
    pub txt_pg4ipart8_10b: f64,
    /// BIR: `frm1701:txtPg4IPart8_1A` (sample: `0.00`)
    pub txt_pg4ipart8_1a: f64,
    /// BIR: `frm1701:txtPg4IPart8_1B` (sample: `0.00`)
    pub txt_pg4ipart8_1b: f64,
    /// BIR: `frm1701:txtPg4IPart8_2A` (sample: `0.00`)
    pub txt_pg4ipart8_2a: f64,
    /// BIR: `frm1701:txtPg4IPart8_2B` (sample: `0.00`)
    pub txt_pg4ipart8_2b: f64,
    /// BIR: `frm1701:txtPg4IPart8_3A` (sample: `0.00`)
    pub txt_pg4ipart8_3a: f64,
    /// BIR: `frm1701:txtPg4IPart8_3B` (sample: `0.00`)
    pub txt_pg4ipart8_3b: f64,
    /// BIR: `frm1701:txtPg4IPart8_4A` (sample: `0.00`)
    pub txt_pg4ipart8_4a: f64,
    /// BIR: `frm1701:txtPg4IPart8_4B` (sample: `0.00`)
    pub txt_pg4ipart8_4b: f64,
    /// BIR: `frm1701:txtPg4IPart8_5A` (sample: `0.00`)
    pub txt_pg4ipart8_5a: f64,
    /// BIR: `frm1701:txtPg4IPart8_5B` (sample: `0.00`)
    pub txt_pg4ipart8_5b: f64,
    /// BIR: `frm1701:txtPg4IPart8_6A` (sample: `0.00`)
    pub txt_pg4ipart8_6a: f64,
    /// BIR: `frm1701:txtPg4IPart8_6B` (sample: `0.00`)
    pub txt_pg4ipart8_6b: f64,
    /// BIR: `frm1701:txtPg4IPart8_7A` (sample: `0.00`)
    pub txt_pg4ipart8_7a: f64,
    /// BIR: `frm1701:txtPg4IPart8_7B` (sample: `0.00`)
    pub txt_pg4ipart8_7b: f64,
    /// BIR: `frm1701:txtPg4IPart8_8A` (sample: `0.00`)
    pub txt_pg4ipart8_8a: f64,
    /// BIR: `frm1701:txtPg4IPart8_8B` (sample: `0.00`)
    pub txt_pg4ipart8_8b: f64,
    /// BIR: `frm1701:txtPg4IPart8_9A` (sample: `0.00`)
    pub txt_pg4ipart8_9a: f64,
    /// BIR: `frm1701:txtPg4IPart8_9B` (sample: `0.00`)
    pub txt_pg4ipart8_9b: f64,
    /// BIR: `frm1701:txtPg4IPart9_10A` (sample: `0.00`)
    pub txt_pg4ipart9_10a: f64,
    /// BIR: `frm1701:txtPg4IPart9_10B` (sample: `0.00`)
    pub txt_pg4ipart9_10b: f64,
    /// BIR: `frm1701:txtPg4IPart9_11A` (sample: `0.00`)
    pub txt_pg4ipart9_11a: f64,
    /// BIR: `frm1701:txtPg4IPart9_11B` (sample: `0.00`)
    pub txt_pg4ipart9_11b: f64,
    /// BIR: `frm1701:txtPg4IPart9_1A` (sample: `0.00`)
    pub txt_pg4ipart9_1a: f64,
    /// BIR: `frm1701:txtPg4IPart9_1B` (sample: `0.00`)
    pub txt_pg4ipart9_1b: f64,
    /// BIR: `frm1701:txtPg4IPart9_2A` (sample: `0.00`)
    pub txt_pg4ipart9_2a: f64,
    /// BIR: `frm1701:txtPg4IPart9_2B` (sample: `0.00`)
    pub txt_pg4ipart9_2b: f64,
    /// BIR: `frm1701:txtPg4IPart9_2Particulars` (sample: ``)
    pub txt_pg4ipart9_2particulars: String,
    /// BIR: `frm1701:txtPg4IPart9_3A` (sample: `0.00`)
    pub txt_pg4ipart9_3a: f64,
    /// BIR: `frm1701:txtPg4IPart9_3B` (sample: `0.00`)
    pub txt_pg4ipart9_3b: f64,
    /// BIR: `frm1701:txtPg4IPart9_3Particulars` (sample: ``)
    pub txt_pg4ipart9_3particulars: String,
    /// BIR: `frm1701:txtPg4IPart9_4A` (sample: `0.00`)
    pub txt_pg4ipart9_4a: f64,
    /// BIR: `frm1701:txtPg4IPart9_4B` (sample: `0.00`)
    pub txt_pg4ipart9_4b: f64,
    /// BIR: `frm1701:txtPg4IPart9_4Particulars` (sample: ``)
    pub txt_pg4ipart9_4particulars: String,
    /// BIR: `frm1701:txtPg4IPart9_5A` (sample: `0.00`)
    pub txt_pg4ipart9_5a: f64,
    /// BIR: `frm1701:txtPg4IPart9_5B` (sample: `0.00`)
    pub txt_pg4ipart9_5b: f64,
    /// BIR: `frm1701:txtPg4IPart9_6A` (sample: `0.00`)
    pub txt_pg4ipart9_6a: f64,
    /// BIR: `frm1701:txtPg4IPart9_6B` (sample: `0.00`)
    pub txt_pg4ipart9_6b: f64,
    /// BIR: `frm1701:txtPg4IPart9_6Particulars` (sample: ``)
    pub txt_pg4ipart9_6particulars: String,
    /// BIR: `frm1701:txtPg4IPart9_7A` (sample: `0.00`)
    pub txt_pg4ipart9_7a: f64,
    /// BIR: `frm1701:txtPg4IPart9_7B` (sample: `0.00`)
    pub txt_pg4ipart9_7b: f64,
    /// BIR: `frm1701:txtPg4IPart9_7Particulars` (sample: ``)
    pub txt_pg4ipart9_7particulars: String,
    /// BIR: `frm1701:txtPg4IPart9_8A` (sample: `0.00`)
    pub txt_pg4ipart9_8a: f64,
    /// BIR: `frm1701:txtPg4IPart9_8B` (sample: `0.00`)
    pub txt_pg4ipart9_8b: f64,
    /// BIR: `frm1701:txtPg4IPart9_8Particulars` (sample: ``)
    pub txt_pg4ipart9_8particulars: String,
    /// BIR: `frm1701:txtPg4IPart9_9A` (sample: `0.00`)
    pub txt_pg4ipart9_9a: f64,
    /// BIR: `frm1701:txtPg4IPart9_9B` (sample: `0.00`)
    pub txt_pg4ipart9_9b: f64,
    /// BIR: `frm1701:txtPg4IPart9_9Particulars` (sample: ``)
    pub txt_pg4ipart9_9particulars: String,
    /// BIR: `frm1701:txtPg4ISc6_1A` (sample: `0.00`)
    pub txt_pg4isc6_1a: f64,
    /// BIR: `frm1701:txtPg4ISc6_1B` (sample: `0.00`)
    pub txt_pg4isc6_1b: f64,
    /// BIR: `frm1701:txtPg4ISc6_2A` (sample: `0.00`)
    pub txt_pg4isc6_2a: f64,
    /// BIR: `frm1701:txtPg4ISc6_2B` (sample: `0.00`)
    pub txt_pg4isc6_2b: f64,
    /// BIR: `frm1701:txtPg4ISc6_3A` (sample: `0.00`)
    pub txt_pg4isc6_3a: f64,
    /// BIR: `frm1701:txtPg4ISc6_3B` (sample: `0.00`)
    pub txt_pg4isc6_3b: f64,
    /// BIR: `frm1701:txtPg4ISc6_4A` (sample: `0.00`)
    pub txt_pg4isc6_4a: f64,
    /// BIR: `frm1701:txtPg4ISc6_4B` (sample: `0.00`)
    pub txt_pg4isc6_4b: f64,
    /// BIR: `frm1701:txtPg4ISc6_5A` (sample: `0.00`)
    pub txt_pg4isc6_5a: f64,
    /// BIR: `frm1701:txtPg4ISc6_5B` (sample: `0.00`)
    pub txt_pg4isc6_5b: f64,
    /// BIR: `frm1701:txtPg4IShed6_10A` (sample: `0.00`)
    pub txt_pg4ished6_10a: f64,
    /// BIR: `frm1701:txtPg4IShed6_10B` (sample: `0.00`)
    pub txt_pg4ished6_10b: f64,
    /// BIR: `frm1701:txtPg4IShed6_10C` (sample: `0.00`)
    pub txt_pg4ished6_10c: f64,
    /// BIR: `frm1701:txtPg4IShed6_10D` (sample: `0.00`)
    pub txt_pg4ished6_10d: f64,
    /// BIR: `frm1701:txtPg4IShed6_10E` (sample: `0.00`)
    pub txt_pg4ished6_10e: f64,
    /// BIR: `frm1701:txtPg4IShed6_11A` (sample: `0.00`)
    pub txt_pg4ished6_11a: f64,
    /// BIR: `frm1701:txtPg4IShed6_11B` (sample: `0.00`)
    pub txt_pg4ished6_11b: f64,
    /// BIR: `frm1701:txtPg4IShed6_11C` (sample: `0.00`)
    pub txt_pg4ished6_11c: f64,
    /// BIR: `frm1701:txtPg4IShed6_11D` (sample: `0.00`)
    pub txt_pg4ished6_11d: f64,
    /// BIR: `frm1701:txtPg4IShed6_11E` (sample: `0.00`)
    pub txt_pg4ished6_11e: f64,
    /// BIR: `frm1701:txtPg4IShed6_12A` (sample: `0.00`)
    pub txt_pg4ished6_12a: f64,
    /// BIR: `frm1701:txtPg4IShed6_12B` (sample: `0.00`)
    pub txt_pg4ished6_12b: f64,
    /// BIR: `frm1701:txtPg4IShed6_12C` (sample: `0.00`)
    pub txt_pg4ished6_12c: f64,
    /// BIR: `frm1701:txtPg4IShed6_12D` (sample: `0.00`)
    pub txt_pg4ished6_12d: f64,
    /// BIR: `frm1701:txtPg4IShed6_12E` (sample: `0.00`)
    pub txt_pg4ished6_12e: f64,
    /// BIR: `frm1701:txtPg4IShed6_13D` (sample: `0.00`)
    pub txt_pg4ished6_13d: f64,
    /// BIR: `frm1701:txtPg4IShed6_9A` (sample: `0.00`)
    pub txt_pg4ished6_9a: f64,
    /// BIR: `frm1701:txtPg4IShed6_9B` (sample: `0.00`)
    pub txt_pg4ished6_9b: f64,
    /// BIR: `frm1701:txtPg4IShed6_9C` (sample: `0.00`)
    pub txt_pg4ished6_9c: f64,
    /// BIR: `frm1701:txtPg4IShed6_9D` (sample: `0.00`)
    pub txt_pg4ished6_9d: f64,
    /// BIR: `frm1701:txtPg4IShed6_9E` (sample: `0.00`)
    pub txt_pg4ished6_9e: f64,
    /// BIR: `frm1701:txtPg4mSchedC_10ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_10atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_10BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_10btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_11ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_11atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_11BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_11btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_12ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_12atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_12BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_12btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_13ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_13atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_13BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_13btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_14ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_14atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_14BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_14btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_15ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_15atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_15BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_15btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_16ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_16atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_16BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_16btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_17aATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_17a_atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_17aBTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_17a_btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_17bATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_17b_atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_17bBTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_17b_btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_17cATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_17c_atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_17cBTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_17c_btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_17dATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_17d_atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_17dBTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_17d_btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_17dTYPE` (sample: ``)
    pub txt_pg4m_sched_c_17d_type: String,
    /// BIR: `frm1701:txtPg4mSchedC_18ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_18atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_18BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_18btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_4ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_4atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_4BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_4btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_5ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_5atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_5BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_5btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_6ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_6atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_6BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_6btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_7ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_7atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_7BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_7btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_8ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_8atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_8BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_8btype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_9ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_9atype: f64,
    /// BIR: `frm1701:txtPg4mSchedC_9BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_c_9btype: f64,
    /// BIR: `frm1701:txtPg4mSchedD1_1ALBTYPE` (sample: ``)
    pub txt_pg4m_sched_d1_1albtype: String,
    /// BIR: `frm1701:txtPg4mSchedD1_1ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_d1_1atype: f64,
    /// BIR: `frm1701:txtPg4mSchedD1_1TYPE` (sample: ``)
    pub txt_pg4m_sched_d1_1type: String,
    /// BIR: `frm1701:txtPg4mSchedD1_2ALBTYPE` (sample: ``)
    pub txt_pg4m_sched_d1_2albtype: String,
    /// BIR: `frm1701:txtPg4mSchedD1_2ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_d1_2atype: f64,
    /// BIR: `frm1701:txtPg4mSchedD1_2TYPE` (sample: ``)
    pub txt_pg4m_sched_d1_2type: String,
    /// BIR: `frm1701:txtPg4mSchedD1_3ALBTYPE` (sample: ``)
    pub txt_pg4m_sched_d1_3albtype: String,
    /// BIR: `frm1701:txtPg4mSchedD1_3ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_d1_3atype: f64,
    /// BIR: `frm1701:txtPg4mSchedD1_3TYPE` (sample: ``)
    pub txt_pg4m_sched_d1_3type: String,
    /// BIR: `frm1701:txtPg4mSchedD1_4ALBTYPE` (sample: ``)
    pub txt_pg4m_sched_d1_4albtype: String,
    /// BIR: `frm1701:txtPg4mSchedD1_4ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_d1_4atype: f64,
    /// BIR: `frm1701:txtPg4mSchedD1_4TYPE` (sample: ``)
    pub txt_pg4m_sched_d1_4type: String,
    /// BIR: `frm1701:txtPg4mSchedD1_5ATYPE` (sample: `0.00`)
    pub txt_pg4m_sched_d1_5atype: f64,
    /// BIR: `frm1701:txtPg4mSchedD2_10BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_d2_10btype: f64,
    /// BIR: `frm1701:txtPg4mSchedD2_6BLBTYPE` (sample: ``)
    pub txt_pg4m_sched_d2_6blbtype: String,
    /// BIR: `frm1701:txtPg4mSchedD2_6BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_d2_6btype: f64,
    /// BIR: `frm1701:txtPg4mSchedD2_6TYPE` (sample: ``)
    pub txt_pg4m_sched_d2_6type: String,
    /// BIR: `frm1701:txtPg4mSchedD2_7BLBTYPE` (sample: ``)
    pub txt_pg4m_sched_d2_7blbtype: String,
    /// BIR: `frm1701:txtPg4mSchedD2_7BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_d2_7btype: f64,
    /// BIR: `frm1701:txtPg4mSchedD2_7TYPE` (sample: ``)
    pub txt_pg4m_sched_d2_7type: String,
    /// BIR: `frm1701:txtPg4mSchedD2_8BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_d2_8btype: f64,
    /// BIR: `frm1701:txtPg4mSchedD2_8LBBTYPE` (sample: ``)
    pub txt_pg4m_sched_d2_8lbbtype: String,
    /// BIR: `frm1701:txtPg4mSchedD2_8TYPE` (sample: ``)
    pub txt_pg4m_sched_d2_8type: String,
    /// BIR: `frm1701:txtPg4mSchedD2_9BLBTYPE` (sample: ``)
    pub txt_pg4m_sched_d2_9blbtype: String,
    /// BIR: `frm1701:txtPg4mSchedD2_9BTYPE` (sample: `0.00`)
    pub txt_pg4m_sched_d2_9btype: f64,
    /// BIR: `frm1701:txtPg4mSchedD2_9TYPE` (sample: ``)
    pub txt_pg4m_sched_d2_9type: String,
    /// BIR: `frm1701:txtTIN4` (sample: ``)
    pub txt_tin4: String,
    /// BIR: `frm1701:txtVersion` (sample: `051414`)
    pub txt_version: u32,
    /// BIR: `frm1701:txtZIP` (sample: ``)
    pub txt_zip: String,
    /// BIR: `frm1701:txtdisabledID` (sample: ``)
    pub txtdisabled_id: String,

    // === Lifecycle ===
    pub status: FilingStatus,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub submitted_at: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<String>,
    #[serde(default)]
    pub submission_filename: Option<String>,
    #[serde(default)]
    pub receipt_id: Option<i64>,
    #[serde(default)]
    pub submission_attempts: u32,
    #[serde(default)]
    pub next_retry_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl FormValidator for Form1701Draft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        if self.tin.is_empty() {
            errors.push(("tin".into(), "TIN is required".into()));
        }
        if self.taxpayer_name.is_empty() {
            errors.push(("taxpayer_name".into(), "Taxpayer name is required".into()));
        }
        // TODO: Add form-specific validation rules
        errors
    }
}

impl Form1701Draft {
    /// Create a new draft from a taxpayer profile.
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, month: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            month,
            is_amended: false,
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            chk_pg2ished1a_1spouse: false,
            chk_pg2ished1a_1taxpayer: false,
            chk_pg2ished2a_2spouse: false,
            chk_pg2ished2a_2taxpayer: false,
            rdo_exattachment_s: false,
            rdo_exattachment_tf: false,
            rdo_pg1i13foreign_tax_credits_no: true,
            rdo_pg1i13foreign_tax_credits_yes: false,
            rdo_pg1i16civil_status_ls: false,
            rdo_pg1i16civil_status_m: false,
            rdo_pg1i16civil_status_s: true,
            rdo_pg1i16civil_status_w: false,
            rdo_pg1i17spouse_income_no: false,
            rdo_pg1i17spouse_income_yes: false,
            rdo_pg1i18filing_status_j: false,
            rdo_pg1i18filing_status_s: false,
            rdo_pg1i19income_exempt_no: true,
            rdo_pg1i19income_exempt_yes: false,
            rdo_pg1i20income_special_no: true,
            rdo_pg1i20income_special_yes: false,
            rdo_pg1i21amethod_deduction_i: false,
            rdo_pg1i21amethod_deduction_o: true,
            rdo_pg1i21tax_rate_g: true,
            rdo_pg1i21tax_rate_p: false,
            rdo_pg1i3short_period_no: true,
            rdo_pg1i3short_period_yes: false,
            rdo_pg1i6taxpayer_type_c: false,
            rdo_pg1i6taxpayer_type_e: false,
            rdo_pg1i6taxpayer_type_p: false,
            rdo_pg1i6taxpayer_type_s: true,
            rdo_pg1i6taxpayer_type_t: false,
            rdo_pg1i7atc_ii011: false,
            rdo_pg1i7atc_ii012: true,
            rdo_pg1i7atc_ii013: false,
            rdo_pg1i7atc_ii014: false,
            rdo_pg1i7atc_ii015: false,
            rdo_pg1i7atc_ii016: false,
            rdo_pg1i7atc_ii017: false,
            rdo_pg1overpayment_carry_over: false,
            rdo_pg1overpayment_refund: false,
            rdo_pg1overpayment_tcc: false,
            rdo_pg1m_option1: true,
            rdo_pg1m_option2: false,
            rdo_pg2i10income_exempt_no: false,
            rdo_pg2i10income_exempt_yes: false,
            rdo_pg2i11income_special_no: false,
            rdo_pg2i11income_special_yes: false,
            rdo_pg2i12amethod_deduction_i: false,
            rdo_pg2i12amethod_deduction_o: false,
            rdo_pg2i12tax_rate_g: false,
            rdo_pg2i12tax_rate_p: false,
            rdo_pg2i3spouse_type_c: false,
            rdo_pg2i3spouse_type_p: false,
            rdo_pg2i3spouse_type_s: false,
            rdo_pg2i4atc_ii011: false,
            rdo_pg2i4atc_ii012: false,
            rdo_pg2i4atc_ii013: false,
            rdo_pg2i4atc_ii014: false,
            rdo_pg2i4atc_ii015: false,
            rdo_pg2i4atc_ii016: false,
            rdo_pg2i4atc_ii017: false,
            rdo_pg2i8foreign_tax_credits_no: false,
            rdo_pg2i8foreign_tax_credits_yes: false,
            rdo_pg3m_exempt_type: false,
            rdo_pg3m_special_rate_type: false,
            rdo_spattachment_s: false,
            rdo_spattachment_tf: false,
            txt_attachment_types: String::new(),
            txt_current_page: 0,
            txt_disabled_inputs: String::new(),
            txt_disabled_on_save: String::new(),
            txt_enabled_inputs_on_validation: String::new(),
            txt_enabled_links: String::new(),
            txt_enabled_on_save: String::new(),
            txt_is_spouse_disabled: String::new(),
            txt_is_tax_filer_disabled: String::new(),
            txt_line_bus: String::new(),
            txt_max_page: 0,
            txt_pg1i10birth_date: String::new(),
            txt_pg1i12citizenship: String::new(),
            txt_pg1i14foreign_tax_number: String::new(),
            txt_pg1i22atax_due: 0.0,
            txt_pg1i22btax_due: 0.0,
            txt_pg1i235number: String::new(),
            txt_pg1i23a: 0.0,
            txt_pg1i23b: 0.0,
            txt_pg1i24atax_payable: 0.0,
            txt_pg1i24btax_payable: 0.0,
            txt_pg1i25a: 0.0,
            txt_pg1i25b: 0.0,
            txt_pg1i26a: 0.0,
            txt_pg1i26b: 0.0,
            txt_pg1i27a: 0.0,
            txt_pg1i27b: 0.0,
            txt_pg1i28a: 0.0,
            txt_pg1i28b: 0.0,
            txt_pg1i29a: 0.0,
            txt_pg1i29b: 0.0,
            txt_pg1i30a: 0.0,
            txt_pg1i30b: 0.0,
            txt_pg1i31atotal_amt_pyble: 0.0,
            txt_pg1i31btotal_amt_pyble: 0.0,
            txt_pg1i32aggregate_amt_pyble: 0.0,
            txt_pg1i33number_of_attachments: 0,
            txt_pg1i34agency: String::new(),
            txt_pg1i34amount: String::new(),
            txt_pg1i34date: String::new(),
            txt_pg1i34number: String::new(),
            txt_pg1i35agency: String::new(),
            txt_pg1i35amount: String::new(),
            txt_pg1i35date: String::new(),
            txt_pg1i36amount: String::new(),
            txt_pg1i36date: String::new(),
            txt_pg1i36number: String::new(),
            txt_pg1i37agency: String::new(),
            txt_pg1i37amount: String::new(),
            txt_pg1i37date: String::new(),
            txt_pg1i37number: String::new(),
            txt_pg1i37particular: String::new(),
            txt_pg1i9address: String::new(),
            txt_pg1m_i10cschd_b: 0.0,
            txt_pg1m_i10dschd_b: 0.0,
            txt_pg1m_i10gschd_b: 0.0,
            txt_pg1m_i10hschd_b: 0.0,
            txt_pg1m_i11aschd_b: 0.0,
            txt_pg1m_i11bschd_b: 0.0,
            txt_pg1m_i11cschd_b: 0.0,
            txt_pg1m_i11dschd_b: 0.0,
            txt_pg1m_i11eschd_b: 0.0,
            txt_pg1m_i11fschd_b: 0.0,
            txt_pg1m_i11gschd_b: 0.0,
            txt_pg1m_i11hschd_b: 0.0,
            txt_pg1m_i12aschd_b: 0.0,
            txt_pg1m_i12bschd_b: 0.0,
            txt_pg1m_i12cschd_b: 0.0,
            txt_pg1m_i12dschd_b: 0.0,
            txt_pg1m_i12desc_schd_b: String::new(),
            txt_pg1m_i12eschd_b: 0.0,
            txt_pg1m_i12fschd_b: 0.0,
            txt_pg1m_i12gschd_b: 0.0,
            txt_pg1m_i12hschd_b: 0.0,
            txt_pg1m_i13aschd_b: 0.0,
            txt_pg1m_i13bschd_b: 0.0,
            txt_pg1m_i13cschd_b: 0.0,
            txt_pg1m_i13dschd_b: 0.0,
            txt_pg1m_i13desc_schd_b: String::new(),
            txt_pg1m_i13eschd_b: 0.0,
            txt_pg1m_i13fschd_b: 0.0,
            txt_pg1m_i13gschd_b: 0.0,
            txt_pg1m_i13hschd_b: 0.0,
            txt_pg1m_i14cschd_b: 0.0,
            txt_pg1m_i14dschd_b: 0.0,
            txt_pg1m_i14gschd_b: 0.0,
            txt_pg1m_i14hschd_b: 0.0,
            txt_pg1m_i15aschd_b: 0.0,
            txt_pg1m_i15bschd_b: 0.0,
            txt_pg1m_i15cschd_b: 0.0,
            txt_pg1m_i15dschd_b: 0.0,
            txt_pg1m_i15eschd_b: 0.0,
            txt_pg1m_i15fschd_b: 0.0,
            txt_pg1m_i15gschd_b: 0.0,
            txt_pg1m_i15hschd_b: 0.0,
            txt_pg1m_i16aschd_b: 0.0,
            txt_pg1m_i16bschd_b: 0.0,
            txt_pg1m_i16cschd_b: 0.0,
            txt_pg1m_i16dschd_b: 0.0,
            txt_pg1m_i16eschd_b: 0.0,
            txt_pg1m_i16fschd_b: 0.0,
            txt_pg1m_i16gschd_b: 0.0,
            txt_pg1m_i16hschd_b: 0.0,
            txt_pg1m_i17aschd_b: 0.0,
            txt_pg1m_i17bschd_b: 0.0,
            txt_pg1m_i17cschd_b: 0.0,
            txt_pg1m_i17dschd_b: 0.0,
            txt_pg1m_i17eschd_b: 0.0,
            txt_pg1m_i17fschd_b: 0.0,
            txt_pg1m_i17gschd_b: 0.0,
            txt_pg1m_i17hschd_b: 0.0,
            txt_pg1m_i1aschd_a: String::new(),
            txt_pg1m_i1aschd_b: 0.0,
            txt_pg1m_i1bschd_a: String::new(),
            txt_pg1m_i1bschd_b: 0.0,
            txt_pg1m_i1cschd_a: String::new(),
            txt_pg1m_i1cschd_b: 0.0,
            txt_pg1m_i1dschd_a: String::new(),
            txt_pg1m_i1dschd_b: 0.0,
            txt_pg1m_i1eschd_a: String::new(),
            txt_pg1m_i1eschd_b: 0.0,
            txt_pg1m_i1fschd_a: String::new(),
            txt_pg1m_i1fschd_b: 0.0,
            txt_pg1m_i1gschd_b: 0.0,
            txt_pg1m_i1hschd_b: 0.0,
            txt_pg1m_i2aschd_a: String::new(),
            txt_pg1m_i2aschd_b: 0.0,
            txt_pg1m_i2bschd_a: String::new(),
            txt_pg1m_i2bschd_b: 0.0,
            txt_pg1m_i2cschd_a: String::new(),
            txt_pg1m_i2cschd_b: 0.0,
            txt_pg1m_i2dschd_a: String::new(),
            txt_pg1m_i2dschd_b: 0.0,
            txt_pg1m_i2eschd_a: String::new(),
            txt_pg1m_i2eschd_b: 0.0,
            txt_pg1m_i2fschd_a: String::new(),
            txt_pg1m_i2fschd_b: 0.0,
            txt_pg1m_i2gschd_b: 0.0,
            txt_pg1m_i2hschd_b: 0.0,
            txt_pg1m_i3aschd_a: String::new(),
            txt_pg1m_i3aschd_b: 0.0,
            txt_pg1m_i3bschd_a: String::new(),
            txt_pg1m_i3bschd_b: 0.0,
            txt_pg1m_i3cschd_a: String::new(),
            txt_pg1m_i3cschd_b: 0.0,
            txt_pg1m_i3dschd_a: String::new(),
            txt_pg1m_i3dschd_b: 0.0,
            txt_pg1m_i3eschd_a: String::new(),
            txt_pg1m_i3eschd_b: 0.0,
            txt_pg1m_i3fschd_a: String::new(),
            txt_pg1m_i3fschd_b: 0.0,
            txt_pg1m_i3gschd_b: 0.0,
            txt_pg1m_i3hschd_b: 0.0,
            txt_pg1m_i4aschd_b: 0.0,
            txt_pg1m_i4bschd_a: 0.0,
            txt_pg1m_i4bschd_b: 0.0,
            txt_pg1m_i4cschd_b: 0.0,
            txt_pg1m_i4dschd_b: 0.0,
            txt_pg1m_i4eschd_a: 0.0,
            txt_pg1m_i4eschd_b: 0.0,
            txt_pg1m_i4fschd_b: 0.0,
            txt_pg1m_i4gschd_b: 0.0,
            txt_pg1m_i4hschd_b: 0.0,
            txt_pg1m_i5aschd_a: String::new(),
            txt_pg1m_i5aschd_b: 0.0,
            txt_pg1m_i5bschd_a: String::new(),
            txt_pg1m_i5bschd_b: 0.0,
            txt_pg1m_i5cschd_a: String::new(),
            txt_pg1m_i5cschd_b: 0.0,
            txt_pg1m_i5dschd_a: String::new(),
            txt_pg1m_i5dschd_b: 0.0,
            txt_pg1m_i5eschd_a: String::new(),
            txt_pg1m_i5eschd_b: 0.0,
            txt_pg1m_i5fschd_a: String::new(),
            txt_pg1m_i5fschd_b: 0.0,
            txt_pg1m_i5gschd_b: 0.0,
            txt_pg1m_i5hschd_b: 0.0,
            txt_pg1m_i6aschd_a: String::new(),
            txt_pg1m_i6aschd_b: 0.0,
            txt_pg1m_i6bschd_a: String::new(),
            txt_pg1m_i6bschd_b: 0.0,
            txt_pg1m_i6cschd_a: String::new(),
            txt_pg1m_i6cschd_b: 0.0,
            txt_pg1m_i6dschd_a: String::new(),
            txt_pg1m_i6dschd_b: 0.0,
            txt_pg1m_i6eschd_a: String::new(),
            txt_pg1m_i6eschd_b: 0.0,
            txt_pg1m_i6fschd_a: String::new(),
            txt_pg1m_i6fschd_b: 0.0,
            txt_pg1m_i6gschd_b: 0.0,
            txt_pg1m_i6hschd_b: 0.0,
            txt_pg1m_i7aschd_b: 0.0,
            txt_pg1m_i7bschd_b: 0.0,
            txt_pg1m_i7cschd_b: 0.0,
            txt_pg1m_i7dschd_b: 0.0,
            txt_pg1m_i7eschd_b: 0.0,
            txt_pg1m_i7fschd_b: 0.0,
            txt_pg1m_i7gschd_b: 0.0,
            txt_pg1m_i7hschd_b: 0.0,
            txt_pg1m_i8cschd_b: 0.0,
            txt_pg1m_i8dschd_b: 0.0,
            txt_pg1m_i8gschd_b: 0.0,
            txt_pg1m_i8hschd_b: 0.0,
            txt_pg1m_i9aschd_b: 0.0,
            txt_pg1m_i9bschd_b: 0.0,
            txt_pg1m_i9cschd_b: 0.0,
            txt_pg1m_i9dschd_b: 0.0,
            txt_pg1m_i9eschd_b: 0.0,
            txt_pg1m_i9fschd_b: 0.0,
            txt_pg1m_i9gschd_b: 0.0,
            txt_pg1m_i9hschd_b: 0.0,
            txt_pg2i5spouse_name: String::new(),
            txt_pg2i7citizenship: String::new(),
            txt_pg2i9foreign_tax_number: String::new(),
            txt_pg2ished1a_1sname: String::new(),
            txt_pg2ished1a_1tpname: String::new(),
            txt_pg2ished1c_1ci: 0.0,
            txt_pg2ished1c_1tw: 0.0,
            txt_pg2ished1c_2ci: 0.0,
            txt_pg2ished1c_2tw: 0.0,
            txt_pg2ished1c_3aci: 0.0,
            txt_pg2ished1c_3atw: 0.0,
            txt_pg2ished1c_3bci: 0.0,
            txt_pg2ished1c_3btw: 0.0,
            txt_pg2ished2_4a: 0.0,
            txt_pg2ished2_4b: 0.0,
            txt_pg2ished2_5a: 0.0,
            txt_pg2ished2_5b: 0.0,
            txt_pg2ished2_6a: 0.0,
            txt_pg2ished2_6b: 0.0,
            txt_pg2ished2_7a: 0.0,
            txt_pg2ished2_7b: 0.0,
            txt_pg2ished2a_2sname: String::new(),
            txt_pg2ished2a_2tpname: String::new(),
            txt_pg2ished3_10a: 0.0,
            txt_pg2ished3_10b: 0.0,
            txt_pg2ished3_11a: 0.0,
            txt_pg2ished3_11b: 0.0,
            txt_pg2ished3_12a: 0.0,
            txt_pg2ished3_12b: 0.0,
            txt_pg2ished3_13a: 0.0,
            txt_pg2ished3_13b: 0.0,
            txt_pg2ished3_14a: 0.0,
            txt_pg2ished3_14b: 0.0,
            txt_pg2ished3_15a: 0.0,
            txt_pg2ished3_15b: 0.0,
            txt_pg2ished3_16a: 0.0,
            txt_pg2ished3_16b: 0.0,
            txt_pg2ished3_17a: 0.0,
            txt_pg2ished3_17b: 0.0,
            txt_pg2ished3_18a: 0.0,
            txt_pg2ished3_18b: 0.0,
            txt_pg2ished3_19a: 0.0,
            txt_pg2ished3_19b: 0.0,
            txt_pg2ished3_19desc: String::new(),
            txt_pg2ished3_20a: 0.0,
            txt_pg2ished3_20b: 0.0,
            txt_pg2ished3_20desc: String::new(),
            txt_pg2ished3_21a: 0.0,
            txt_pg2ished3_21b: 0.0,
            txt_pg2ished3_22a: 0.0,
            txt_pg2ished3_22b: 0.0,
            txt_pg2ished3_23a: 0.0,
            txt_pg2ished3_23b: 0.0,
            txt_pg2ished3_24a: 0.0,
            txt_pg2ished3_24b: 0.0,
            txt_pg2ished3_25a: 0.0,
            txt_pg2ished3_25b: 0.0,
            txt_pg2ished3_8a: 0.0,
            txt_pg2ished3_8b: 0.0,
            txt_pg2ished3_9a: 0.0,
            txt_pg2ished3_9b: 0.0,
            txt_pg2m_i10aschd_c: 0.0,
            txt_pg2m_i10bschd_c: 0.0,
            txt_pg2m_i10cschd_c: 0.0,
            txt_pg2m_i10dschd_c: 0.0,
            txt_pg2m_i11aschd_c: 0.0,
            txt_pg2m_i11bschd_c: 0.0,
            txt_pg2m_i11cschd_c: 0.0,
            txt_pg2m_i11dschd_c: 0.0,
            txt_pg2m_i12aschd_c: 0.0,
            txt_pg2m_i12bschd_c: 0.0,
            txt_pg2m_i12cschd_c: 0.0,
            txt_pg2m_i12dschd_c: 0.0,
            txt_pg2m_i13aschd_c: 0.0,
            txt_pg2m_i13bschd_c: 0.0,
            txt_pg2m_i13cschd_c: 0.0,
            txt_pg2m_i13dschd_c: 0.0,
            txt_pg2m_i14aschd_c: 0.0,
            txt_pg2m_i14bschd_c: 0.0,
            txt_pg2m_i14cschd_c: 0.0,
            txt_pg2m_i14dschd_c: 0.0,
            txt_pg2m_i15aschd_c: 0.0,
            txt_pg2m_i15bschd_c: 0.0,
            txt_pg2m_i15cschd_c: 0.0,
            txt_pg2m_i15dschd_c: 0.0,
            txt_pg2m_i16aschd_c: 0.0,
            txt_pg2m_i16bschd_c: 0.0,
            txt_pg2m_i16cschd_c: 0.0,
            txt_pg2m_i16dschd_c: 0.0,
            txt_pg2m_i17a_aschd_c: 0.0,
            txt_pg2m_i17a_bschd_c: 0.0,
            txt_pg2m_i17a_cschd_c: 0.0,
            txt_pg2m_i17a_dschd_c: 0.0,
            txt_pg2m_i17b_aschd_c: 0.0,
            txt_pg2m_i17b_bschd_c: 0.0,
            txt_pg2m_i17b_cschd_c: 0.0,
            txt_pg2m_i17b_dschd_c: 0.0,
            txt_pg2m_i17c_aschd_c: 0.0,
            txt_pg2m_i17c_bschd_c: 0.0,
            txt_pg2m_i17c_cschd_c: 0.0,
            txt_pg2m_i17c_dschd_c: 0.0,
            txt_pg2m_i17d_aschd_c: 0.0,
            txt_pg2m_i17d_bschd_c: 0.0,
            txt_pg2m_i17d_cschd_c: 0.0,
            txt_pg2m_i17d_dschd_c: 0.0,
            txt_pg2m_i17d_desc_schd_c: String::new(),
            txt_pg2m_i18aschd_c: 0.0,
            txt_pg2m_i18bschd_c: 0.0,
            txt_pg2m_i18cschd_c: 0.0,
            txt_pg2m_i18dschd_c: 0.0,
            txt_pg2m_i1aschd_c: 0.0,
            txt_pg2m_i1aschd_d: 0.0,
            txt_pg2m_i1bschd_c: 0.0,
            txt_pg2m_i1bschd_d: 0.0,
            txt_pg2m_i1cschd_c: 0.0,
            txt_pg2m_i1dschd_c: 0.0,
            txt_pg2m_i1desc_schd_d: String::new(),
            txt_pg2m_i1lbschd_d: String::new(),
            txt_pg2m_i2aschd_c: 0.0,
            txt_pg2m_i2aschd_d: 0.0,
            txt_pg2m_i2bschd_c: 0.0,
            txt_pg2m_i2bschd_d: 0.0,
            txt_pg2m_i2cschd_c: 0.0,
            txt_pg2m_i2dschd_c: 0.0,
            txt_pg2m_i2desc_schd_d: String::new(),
            txt_pg2m_i2lbschd_d: String::new(),
            txt_pg2m_i3aschd_c: 0.0,
            txt_pg2m_i3aschd_d: 0.0,
            txt_pg2m_i3bschd_c: 0.0,
            txt_pg2m_i3bschd_d: 0.0,
            txt_pg2m_i3cschd_c: 0.0,
            txt_pg2m_i3dschd_c: 0.0,
            txt_pg2m_i3desc_schd_d: String::new(),
            txt_pg2m_i3lbschd_d: String::new(),
            txt_pg2m_i4aschd_c: 0.0,
            txt_pg2m_i4aschd_d: 0.0,
            txt_pg2m_i4bschd_c: 0.0,
            txt_pg2m_i4bschd_d: 0.0,
            txt_pg2m_i4cschd_c: 0.0,
            txt_pg2m_i4dschd_c: 0.0,
            txt_pg2m_i4desc_schd_d: String::new(),
            txt_pg2m_i4lbschd_d: String::new(),
            txt_pg2m_i5aschd_c: 0.0,
            txt_pg2m_i5aschd_d: 0.0,
            txt_pg2m_i5bschd_c: 0.0,
            txt_pg2m_i5bschd_d: 0.0,
            txt_pg2m_i5cschd_c: 0.0,
            txt_pg2m_i5dschd_c: 0.0,
            txt_pg2m_i6aschd_c: 0.0,
            txt_pg2m_i6bschd_c: 0.0,
            txt_pg2m_i6cschd_c: 0.0,
            txt_pg2m_i6dschd_c: 0.0,
            txt_pg2m_i7aschd_c: 0.0,
            txt_pg2m_i7bschd_c: 0.0,
            txt_pg2m_i7cschd_c: 0.0,
            txt_pg2m_i7dschd_c: 0.0,
            txt_pg2m_i8aschd_c: 0.0,
            txt_pg2m_i8bschd_c: 0.0,
            txt_pg2m_i8cschd_c: 0.0,
            txt_pg2m_i8dschd_c: 0.0,
            txt_pg2m_i9aschd_c: 0.0,
            txt_pg2m_i9bschd_c: 0.0,
            txt_pg2m_i9cschd_c: 0.0,
            txt_pg2m_i9dschd_c: 0.0,
            txt_pg3ished3_26a: 0.0,
            txt_pg3ished3_26b: 0.0,
            txt_pg3ished3_27a: 0.0,
            txt_pg3ished3_27b: 0.0,
            txt_pg3ished3_27desc: String::new(),
            txt_pg3ished3_28a: 0.0,
            txt_pg3ished3_28b: 0.0,
            txt_pg3ished3_29a: 0.0,
            txt_pg3ished3_29b: 0.0,
            txt_pg3ished3_30a: 0.0,
            txt_pg3ished3_30b: 0.0,
            txt_pg3ished3_31a: 0.0,
            txt_pg3ished3_31b: 0.0,
            txt_pg3ished3_32a: 0.0,
            txt_pg3ished3_32b: 0.0,
            txt_pg3ished4_10a: 0.0,
            txt_pg3ished4_10b: 0.0,
            txt_pg3ished4_11a: 0.0,
            txt_pg3ished4_11b: 0.0,
            txt_pg3ished4_12a: 0.0,
            txt_pg3ished4_12b: 0.0,
            txt_pg3ished4_13a: 0.0,
            txt_pg3ished4_13b: 0.0,
            txt_pg3ished4_14a: 0.0,
            txt_pg3ished4_14b: 0.0,
            txt_pg3ished4_15a: 0.0,
            txt_pg3ished4_15b: 0.0,
            txt_pg3ished4_16a: 0.0,
            txt_pg3ished4_16b: 0.0,
            txt_pg3ished4_17a_a: 0.0,
            txt_pg3ished4_17a_b: 0.0,
            txt_pg3ished4_17b_a: 0.0,
            txt_pg3ished4_17b_b: 0.0,
            txt_pg3ished4_17c_a: 0.0,
            txt_pg3ished4_17c_b: 0.0,
            txt_pg3ished4_17d_a: 0.0,
            txt_pg3ished4_17d_b: 0.0,
            txt_pg3ished4_17d_desc: String::new(),
            txt_pg3ished4_18a: 0.0,
            txt_pg3ished4_18b: 0.0,
            txt_pg3ished4_1a: 0.0,
            txt_pg3ished4_1b: 0.0,
            txt_pg3ished4_2a: 0.0,
            txt_pg3ished4_2b: 0.0,
            txt_pg3ished4_3a: 0.0,
            txt_pg3ished4_3b: 0.0,
            txt_pg3ished4_4a: 0.0,
            txt_pg3ished4_4b: 0.0,
            txt_pg3ished4_5a: 0.0,
            txt_pg3ished4_5b: 0.0,
            txt_pg3ished4_6a: 0.0,
            txt_pg3ished4_6b: 0.0,
            txt_pg3ished4_7a: 0.0,
            txt_pg3ished4_7b: 0.0,
            txt_pg3ished4_8a: 0.0,
            txt_pg3ished4_8b: 0.0,
            txt_pg3ished4_9a: 0.0,
            txt_pg3ished4_9b: 0.0,
            txt_pg3ished5_1amt: 0.0,
            txt_pg3ished5_1desc: String::new(),
            txt_pg3ished5_1legal: String::new(),
            txt_pg3ished5_2amt: 0.0,
            txt_pg3ished5_2desc: String::new(),
            txt_pg3ished5_2legal: String::new(),
            txt_pg3ished5_3: 0.0,
            txt_pg3ished5_4amt: 0.0,
            txt_pg3ished5_4desc: String::new(),
            txt_pg3ished5_4legal: String::new(),
            txt_pg3ished5_5amt: 0.0,
            txt_pg3ished5_5desc: String::new(),
            txt_pg3ished5_5legal: String::new(),
            txt_pg3ished5_6: 0.0,
            txt_pg3ished6_1a: 0.0,
            txt_pg3ished6_1b: 0.0,
            txt_pg3ished6_2a: 0.0,
            txt_pg3ished6_2b: 0.0,
            txt_pg3ished6_3a: 0.0,
            txt_pg3ished6_3b: 0.0,
            txt_pg3ished6_4a: 0.0,
            txt_pg3ished6_4b: 0.0,
            txt_pg3ished6_4c: 0.0,
            txt_pg3ished6_4d: 0.0,
            txt_pg3ished6_4e: 0.0,
            txt_pg3ished6_5a: 0.0,
            txt_pg3ished6_5b: 0.0,
            txt_pg3ished6_5c: 0.0,
            txt_pg3ished6_5d: 0.0,
            txt_pg3ished6_5e: 0.0,
            txt_pg3ished6_6a: 0.0,
            txt_pg3ished6_6b: 0.0,
            txt_pg3ished6_6c: 0.0,
            txt_pg3ished6_6d: 0.0,
            txt_pg3ished6_6e: 0.0,
            txt_pg3ished6_7a: 0.0,
            txt_pg3ished6_7b: 0.0,
            txt_pg3ished6_7c: 0.0,
            txt_pg3ished6_7d: 0.0,
            txt_pg3ished6_7e: 0.0,
            txt_pg3ished6_8d: 0.0,
            txt_pg3m_sched_a_1atype: String::new(),
            txt_pg3m_sched_a_1btype: String::new(),
            txt_pg3m_sched_a_2atype: String::new(),
            txt_pg3m_sched_a_2btype: String::new(),
            txt_pg3m_sched_a_3atype: String::new(),
            txt_pg3m_sched_a_3btype: String::new(),
            txt_pg3m_sched_a_4atype: 0.0,
            txt_pg3m_sched_a_4btype: 0.0,
            txt_pg3m_sched_a_5atype: String::new(),
            txt_pg3m_sched_a_5btype: String::new(),
            txt_pg3m_sched_a_6atype: String::new(),
            txt_pg3m_sched_a_6btype: String::new(),
            txt_pg3m_sched_b_10atype: 0.0,
            txt_pg3m_sched_b_10btype: 0.0,
            txt_pg3m_sched_b_10type: String::new(),
            txt_pg3m_sched_b_11atype: 0.0,
            txt_pg3m_sched_b_11btype: 0.0,
            txt_pg3m_sched_b_11type: String::new(),
            txt_pg3m_sched_b_12atype: 0.0,
            txt_pg3m_sched_b_12btype: 0.0,
            txt_pg3m_sched_b_13atype: 0.0,
            txt_pg3m_sched_b_13btype: 0.0,
            txt_pg3m_sched_b_14atype: 0.0,
            txt_pg3m_sched_b_14btype: 0.0,
            txt_pg3m_sched_b_15atype: 0.0,
            txt_pg3m_sched_b_15btype: 0.0,
            txt_pg3m_sched_b_1atype: 0.0,
            txt_pg3m_sched_b_1btype: 0.0,
            txt_pg3m_sched_b_2atype: 0.0,
            txt_pg3m_sched_b_2btype: 0.0,
            txt_pg3m_sched_b_3atype: 0.0,
            txt_pg3m_sched_b_3btype: 0.0,
            txt_pg3m_sched_b_4atype: 0.0,
            txt_pg3m_sched_b_4btype: 0.0,
            txt_pg3m_sched_b_5atype: 0.0,
            txt_pg3m_sched_b_5btype: 0.0,
            txt_pg3m_sched_b_6atype: 0.0,
            txt_pg3m_sched_b_6btype: 0.0,
            txt_pg3m_sched_b_7atype: 0.0,
            txt_pg3m_sched_b_7btype: 0.0,
            txt_pg3m_sched_b_8atype: 0.0,
            txt_pg3m_sched_b_8btype: 0.0,
            txt_pg3m_sched_b_9atype: 0.0,
            txt_pg3m_sched_b_9btype: 0.0,
            txt_pg3m_sched_c_1atype: 0.0,
            txt_pg3m_sched_c_1btype: 0.0,
            txt_pg3m_sched_c_2atype: 0.0,
            txt_pg3m_sched_c_2btype: 0.0,
            txt_pg3m_sched_c_3atype: 0.0,
            txt_pg3m_sched_c_3btype: 0.0,
            txt_pg4ipart7_10a: 0.0,
            txt_pg4ipart7_10b: 0.0,
            txt_pg4ipart7_1a: 0.0,
            txt_pg4ipart7_1b: 0.0,
            txt_pg4ipart7_2a: 0.0,
            txt_pg4ipart7_2b: 0.0,
            txt_pg4ipart7_3a: 0.0,
            txt_pg4ipart7_3b: 0.0,
            txt_pg4ipart7_4a: 0.0,
            txt_pg4ipart7_4b: 0.0,
            txt_pg4ipart7_5a: 0.0,
            txt_pg4ipart7_5b: 0.0,
            txt_pg4ipart7_6a: 0.0,
            txt_pg4ipart7_6b: 0.0,
            txt_pg4ipart7_7a: 0.0,
            txt_pg4ipart7_7b: 0.0,
            txt_pg4ipart7_8a: 0.0,
            txt_pg4ipart7_8b: 0.0,
            txt_pg4ipart7_9a: 0.0,
            txt_pg4ipart7_9b: 0.0,
            txt_pg4ipart7_9specify: String::new(),
            txt_pg4ipart8_10a: 0.0,
            txt_pg4ipart8_10b: 0.0,
            txt_pg4ipart8_1a: 0.0,
            txt_pg4ipart8_1b: 0.0,
            txt_pg4ipart8_2a: 0.0,
            txt_pg4ipart8_2b: 0.0,
            txt_pg4ipart8_3a: 0.0,
            txt_pg4ipart8_3b: 0.0,
            txt_pg4ipart8_4a: 0.0,
            txt_pg4ipart8_4b: 0.0,
            txt_pg4ipart8_5a: 0.0,
            txt_pg4ipart8_5b: 0.0,
            txt_pg4ipart8_6a: 0.0,
            txt_pg4ipart8_6b: 0.0,
            txt_pg4ipart8_7a: 0.0,
            txt_pg4ipart8_7b: 0.0,
            txt_pg4ipart8_8a: 0.0,
            txt_pg4ipart8_8b: 0.0,
            txt_pg4ipart8_9a: 0.0,
            txt_pg4ipart8_9b: 0.0,
            txt_pg4ipart9_10a: 0.0,
            txt_pg4ipart9_10b: 0.0,
            txt_pg4ipart9_11a: 0.0,
            txt_pg4ipart9_11b: 0.0,
            txt_pg4ipart9_1a: 0.0,
            txt_pg4ipart9_1b: 0.0,
            txt_pg4ipart9_2a: 0.0,
            txt_pg4ipart9_2b: 0.0,
            txt_pg4ipart9_2particulars: String::new(),
            txt_pg4ipart9_3a: 0.0,
            txt_pg4ipart9_3b: 0.0,
            txt_pg4ipart9_3particulars: String::new(),
            txt_pg4ipart9_4a: 0.0,
            txt_pg4ipart9_4b: 0.0,
            txt_pg4ipart9_4particulars: String::new(),
            txt_pg4ipart9_5a: 0.0,
            txt_pg4ipart9_5b: 0.0,
            txt_pg4ipart9_6a: 0.0,
            txt_pg4ipart9_6b: 0.0,
            txt_pg4ipart9_6particulars: String::new(),
            txt_pg4ipart9_7a: 0.0,
            txt_pg4ipart9_7b: 0.0,
            txt_pg4ipart9_7particulars: String::new(),
            txt_pg4ipart9_8a: 0.0,
            txt_pg4ipart9_8b: 0.0,
            txt_pg4ipart9_8particulars: String::new(),
            txt_pg4ipart9_9a: 0.0,
            txt_pg4ipart9_9b: 0.0,
            txt_pg4ipart9_9particulars: String::new(),
            txt_pg4isc6_1a: 0.0,
            txt_pg4isc6_1b: 0.0,
            txt_pg4isc6_2a: 0.0,
            txt_pg4isc6_2b: 0.0,
            txt_pg4isc6_3a: 0.0,
            txt_pg4isc6_3b: 0.0,
            txt_pg4isc6_4a: 0.0,
            txt_pg4isc6_4b: 0.0,
            txt_pg4isc6_5a: 0.0,
            txt_pg4isc6_5b: 0.0,
            txt_pg4ished6_10a: 0.0,
            txt_pg4ished6_10b: 0.0,
            txt_pg4ished6_10c: 0.0,
            txt_pg4ished6_10d: 0.0,
            txt_pg4ished6_10e: 0.0,
            txt_pg4ished6_11a: 0.0,
            txt_pg4ished6_11b: 0.0,
            txt_pg4ished6_11c: 0.0,
            txt_pg4ished6_11d: 0.0,
            txt_pg4ished6_11e: 0.0,
            txt_pg4ished6_12a: 0.0,
            txt_pg4ished6_12b: 0.0,
            txt_pg4ished6_12c: 0.0,
            txt_pg4ished6_12d: 0.0,
            txt_pg4ished6_12e: 0.0,
            txt_pg4ished6_13d: 0.0,
            txt_pg4ished6_9a: 0.0,
            txt_pg4ished6_9b: 0.0,
            txt_pg4ished6_9c: 0.0,
            txt_pg4ished6_9d: 0.0,
            txt_pg4ished6_9e: 0.0,
            txt_pg4m_sched_c_10atype: 0.0,
            txt_pg4m_sched_c_10btype: 0.0,
            txt_pg4m_sched_c_11atype: 0.0,
            txt_pg4m_sched_c_11btype: 0.0,
            txt_pg4m_sched_c_12atype: 0.0,
            txt_pg4m_sched_c_12btype: 0.0,
            txt_pg4m_sched_c_13atype: 0.0,
            txt_pg4m_sched_c_13btype: 0.0,
            txt_pg4m_sched_c_14atype: 0.0,
            txt_pg4m_sched_c_14btype: 0.0,
            txt_pg4m_sched_c_15atype: 0.0,
            txt_pg4m_sched_c_15btype: 0.0,
            txt_pg4m_sched_c_16atype: 0.0,
            txt_pg4m_sched_c_16btype: 0.0,
            txt_pg4m_sched_c_17a_atype: 0.0,
            txt_pg4m_sched_c_17a_btype: 0.0,
            txt_pg4m_sched_c_17b_atype: 0.0,
            txt_pg4m_sched_c_17b_btype: 0.0,
            txt_pg4m_sched_c_17c_atype: 0.0,
            txt_pg4m_sched_c_17c_btype: 0.0,
            txt_pg4m_sched_c_17d_atype: 0.0,
            txt_pg4m_sched_c_17d_btype: 0.0,
            txt_pg4m_sched_c_17d_type: String::new(),
            txt_pg4m_sched_c_18atype: 0.0,
            txt_pg4m_sched_c_18btype: 0.0,
            txt_pg4m_sched_c_4atype: 0.0,
            txt_pg4m_sched_c_4btype: 0.0,
            txt_pg4m_sched_c_5atype: 0.0,
            txt_pg4m_sched_c_5btype: 0.0,
            txt_pg4m_sched_c_6atype: 0.0,
            txt_pg4m_sched_c_6btype: 0.0,
            txt_pg4m_sched_c_7atype: 0.0,
            txt_pg4m_sched_c_7btype: 0.0,
            txt_pg4m_sched_c_8atype: 0.0,
            txt_pg4m_sched_c_8btype: 0.0,
            txt_pg4m_sched_c_9atype: 0.0,
            txt_pg4m_sched_c_9btype: 0.0,
            txt_pg4m_sched_d1_1albtype: String::new(),
            txt_pg4m_sched_d1_1atype: 0.0,
            txt_pg4m_sched_d1_1type: String::new(),
            txt_pg4m_sched_d1_2albtype: String::new(),
            txt_pg4m_sched_d1_2atype: 0.0,
            txt_pg4m_sched_d1_2type: String::new(),
            txt_pg4m_sched_d1_3albtype: String::new(),
            txt_pg4m_sched_d1_3atype: 0.0,
            txt_pg4m_sched_d1_3type: String::new(),
            txt_pg4m_sched_d1_4albtype: String::new(),
            txt_pg4m_sched_d1_4atype: 0.0,
            txt_pg4m_sched_d1_4type: String::new(),
            txt_pg4m_sched_d1_5atype: 0.0,
            txt_pg4m_sched_d2_10btype: 0.0,
            txt_pg4m_sched_d2_6blbtype: String::new(),
            txt_pg4m_sched_d2_6btype: 0.0,
            txt_pg4m_sched_d2_6type: String::new(),
            txt_pg4m_sched_d2_7blbtype: String::new(),
            txt_pg4m_sched_d2_7btype: 0.0,
            txt_pg4m_sched_d2_7type: String::new(),
            txt_pg4m_sched_d2_8btype: 0.0,
            txt_pg4m_sched_d2_8lbbtype: String::new(),
            txt_pg4m_sched_d2_8type: String::new(),
            txt_pg4m_sched_d2_9blbtype: String::new(),
            txt_pg4m_sched_d2_9btype: 0.0,
            txt_pg4m_sched_d2_9type: String::new(),
            txt_tin4: String::new(),
            txt_version: 0,
            txt_zip: String::new(),
            txtdisabled_id: String::new(),
            status: FilingStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
            submitted_at: None,
            confirmed_at: None,
            submission_filename: None,
            receipt_id: None,
            submission_attempts: 0,
            next_retry_at: None,
            last_error: None,
        }
    }

    /// Recompute all derived fields per BIR 1701 (Annual ITR for Individuals).
    ///
    /// Key computation areas (per official BIR form):
    /// - Schedule B: Income, cost of sales, deductions → net taxable income
    /// - Graduated tax table (TRAIN law) OR 8% flat rate
    /// - Page 1 summary: Tax due, credits, penalties, total payable
    /// - Dual columns: A (taxpayer) and B (spouse, if joint filing)
    pub fn recompute(&mut self) {
        // ── Schedule B — Column C (Taxpayer) / D (Spouse) ──

        // Item 4C/4D: Total Gross Sales/Revenue (sum of income sources 1-3)
        self.txt_pg1m_i4cschd_b =
            self.txt_pg1m_i1gschd_b + self.txt_pg1m_i2gschd_b + self.txt_pg1m_i3gschd_b;
        self.txt_pg1m_i4dschd_b =
            self.txt_pg1m_i1hschd_b + self.txt_pg1m_i2hschd_b + self.txt_pg1m_i3hschd_b;

        // Item 7C/7D: Sum of cost-of-sales sub-items (5+6)
        self.txt_pg1m_i7cschd_b = self.txt_pg1m_i5gschd_b + self.txt_pg1m_i6gschd_b;
        self.txt_pg1m_i7dschd_b = self.txt_pg1m_i5hschd_b + self.txt_pg1m_i6hschd_b;

        // Item 8C/8D: Gross Income = Revenue − Cost of Sales
        self.txt_pg1m_i8cschd_b = self.txt_pg1m_i4cschd_b - self.txt_pg1m_i7cschd_b;
        self.txt_pg1m_i8dschd_b = self.txt_pg1m_i4dschd_b - self.txt_pg1m_i7dschd_b;

        // Item 9: OSD (Optional Standard Deduction) = 40% of gross income
        // Only used when rdoPg1I21AMethodDeductionO is selected
        if self.rdo_pg1i21amethod_deduction_o {
            self.txt_pg1m_i9cschd_b = self.txt_pg1m_i8cschd_b * 0.40;
            self.txt_pg1m_i9dschd_b = self.txt_pg1m_i8dschd_b * 0.40;
        }

        // Item 15C/15D: Total Deductions
        if self.rdo_pg1i21amethod_deduction_o {
            // OSD: just the 40% value
            self.txt_pg1m_i15cschd_b = self.txt_pg1m_i9cschd_b;
            self.txt_pg1m_i15dschd_b = self.txt_pg1m_i9dschd_b;
        } else {
            // Itemized: sum items 10-14
            self.txt_pg1m_i15cschd_b = self.txt_pg1m_i10cschd_b
                + self.txt_pg1m_i11cschd_b
                + self.txt_pg1m_i12cschd_b
                + self.txt_pg1m_i13cschd_b
                + self.txt_pg1m_i14cschd_b;
            self.txt_pg1m_i15dschd_b = self.txt_pg1m_i10dschd_b
                + self.txt_pg1m_i11dschd_b
                + self.txt_pg1m_i12dschd_b
                + self.txt_pg1m_i13dschd_b
                + self.txt_pg1m_i14dschd_b;
        }

        // Item 16C/16D: Net Taxable Income = Gross Income − Total Deductions
        self.txt_pg1m_i16cschd_b =
            f64::max(0.0, self.txt_pg1m_i8cschd_b - self.txt_pg1m_i15cschd_b);
        self.txt_pg1m_i16dschd_b =
            f64::max(0.0, self.txt_pg1m_i8dschd_b - self.txt_pg1m_i15dschd_b);

        // Item 17: Tax Due — either graduated table or 8% flat rate
        let tax_a = if self.rdo_pg1i21tax_rate_p {
            // 8% flat rate on gross sales/receipts exceeding 250,000
            f64::max(0.0, (self.txt_pg1m_i4cschd_b - 250_000.0) * 0.08)
        } else {
            // Graduated rate (TRAIN law — effective 2018)
            Self::graduated_tax(self.txt_pg1m_i16cschd_b)
        };

        let tax_b = if self.rdo_pg2i12tax_rate_p {
            f64::max(0.0, (self.txt_pg1m_i4dschd_b - 250_000.0) * 0.08)
        } else {
            Self::graduated_tax(self.txt_pg1m_i16dschd_b)
        };

        // Store in schedule columns 17a (Page 2, Schedule C)
        self.txt_pg2m_i17a_cschd_c = tax_a;
        self.txt_pg2m_i17a_dschd_c = tax_b;

        // ── Page 1 Summary ──

        // 22A/B: Tax Due
        self.txt_pg1i22atax_due = tax_a;
        self.txt_pg1i22btax_due = tax_b;

        // 23A/B: Tax Credits (user-entered, kept as-is)
        // These are pre-populated from schedules

        // 24A/B: Tax Payable = max(0, Tax Due − Credits)
        self.txt_pg1i24atax_payable = f64::max(0.0, self.txt_pg1i22atax_due - self.txt_pg1i23a);
        self.txt_pg1i24btax_payable = f64::max(0.0, self.txt_pg1i22btax_due - self.txt_pg1i23b);

        // 25-27: Penalties (user-entered)
        // 28A/B: Total Penalties
        self.txt_pg1i28a = self.txt_pg1i25a + self.txt_pg1i26a + self.txt_pg1i27a;
        self.txt_pg1i28b = self.txt_pg1i25b + self.txt_pg1i26b + self.txt_pg1i27b;

        // 29A/B: Net amount due
        self.txt_pg1i29a = self.txt_pg1i24atax_payable + self.txt_pg1i28a;
        self.txt_pg1i29b = self.txt_pg1i24btax_payable + self.txt_pg1i28b;

        // 30A/B: Total amount due (same as 29 for non-installment)
        self.txt_pg1i30a = self.txt_pg1i29a;
        self.txt_pg1i30b = self.txt_pg1i29b;

        // 31A/B: Total Amount Payable
        self.txt_pg1i31atotal_amt_pyble = self.txt_pg1i30a;
        self.txt_pg1i31btotal_amt_pyble = self.txt_pg1i30b;

        // 32: Aggregate Amount Payable = A + B
        self.txt_pg1i32aggregate_amt_pyble =
            self.txt_pg1i31atotal_amt_pyble + self.txt_pg1i31btotal_amt_pyble;

        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Philippine graduated income tax table (TRAIN law, effective 2018).
    ///
    /// | Over        | But not over | Tax     | + % of excess |
    /// |-------------|-------------|---------|---------------|
    /// | 0           | 250,000     | 0       | 0%            |
    /// | 250,000     | 400,000     | 0       | 15%           |
    /// | 400,000     | 800,000     | 22,500  | 20%           |
    /// | 800,000     | 2,000,000   | 102,500 | 25%           |
    /// | 2,000,000   | 8,000,000   | 402,500 | 30%           |
    /// | 8,000,000   | —           | 2,202,500| 35%          |
    fn graduated_tax(net_taxable: f64) -> f64 {
        if net_taxable <= 250_000.0 {
            0.0
        } else if net_taxable <= 400_000.0 {
            (net_taxable - 250_000.0) * 0.15
        } else if net_taxable <= 800_000.0 {
            22_500.0 + (net_taxable - 400_000.0) * 0.20
        } else if net_taxable <= 2_000_000.0 {
            102_500.0 + (net_taxable - 800_000.0) * 0.25
        } else if net_taxable <= 8_000_000.0 {
            402_500.0 + (net_taxable - 2_000_000.0) * 0.30
        } else {
            2_202_500.0 + (net_taxable - 8_000_000.0) * 0.35
        }
    }

    // ── State Transition Methods ──

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        assert!(matches!(self.status, FilingStatus::Draft), "Must be Draft");
        Err(vec![(
            "support_level".to_string(),
            "Form 1701 is scaffold-only and cannot be queued for submission yet.".to_string(),
        )])
    }

    pub fn transition_to_submitted(&mut self, filename: String) {
        assert!(
            matches!(self.status, FilingStatus::Queued),
            "Must be Queued"
        );
        let now = chrono::Utc::now();
        self.status = FilingStatus::Submitted;
        self.submitted_at = Some(now.to_rfc3339());
        self.submission_filename = Some(filename);
        self.submission_attempts = 0;
        self.next_retry_at = None;
        self.last_error = None;
        self.updated_at = now.to_rfc3339();
    }

    pub fn transition_to_confirmed(
        &mut self,
        confirmed_at: String,
        receipt_id: Option<i64>,
        filename: Option<String>,
    ) {
        assert!(
            matches!(self.status, FilingStatus::Submitted),
            "Must be Submitted"
        );
        self.status = FilingStatus::Confirmed;
        self.confirmed_at = Some(confirmed_at);
        self.receipt_id = receipt_id;
        if let Some(f) = filename {
            self.submission_filename = Some(f);
        }
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn transition_to_paid(&mut self) {
        assert!(
            matches!(self.status, FilingStatus::Confirmed),
            "Must be Confirmed"
        );
        self.status = FilingStatus::Paid;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn revert_to_draft(&mut self) {
        assert!(
            !matches!(self.status, FilingStatus::Paid),
            "Cannot revert Paid"
        );
        self.status = FilingStatus::Draft;
        self.submitted_at = None;
        self.confirmed_at = None;
        self.receipt_id = None;
        self.submission_filename = None;
        self.submission_attempts = 0;
        self.next_retry_at = None;
        self.last_error = None;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn record_submission_failure(&mut self, error_msg: String) {
        assert!(
            matches!(self.status, FilingStatus::Queued),
            "Must be Queued"
        );
        self.submission_attempts += 1;
        self.last_error = Some(error_msg);
        if self.submission_attempts >= 5 {
            self.status = FilingStatus::Draft;
            self.next_retry_at = None;
        } else {
            let delay = 2i64.pow(self.submission_attempts - 1);
            let next = chrono::Utc::now() + chrono::Duration::minutes(delay);
            self.next_retry_at = Some(next.to_rfc3339());
        }
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;

    fn make_draft() -> Form1701Draft {
        let profile = TaxpayerProfile {
            id: Some(1),
            full_name: "Test".into(),
            tin: Tin {
                segment1: "000".into(),
                segment2: "000".into(),
                segment3: "000".into(),
                branch: "000".into(),
            },
            rdo_code: "039".into(),
            line_of_business: String::new(),
            registered_address: "Test".into(),
            zip_code: "1100".into(),
            phone: "09170000000".into(),
            email: "t@t.com".into(),
            default_form_type: "1701".into(),
            taxpayer_type: Default::default(),
            is_vat_registered: false,
            business_start_date: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_versions: vec![],
        };
        Form1701Draft::new_from_profile(&profile, 2025, 12)
    }

    #[test]
    fn test_graduated_tax_brackets() {
        assert_eq!(Form1701Draft::graduated_tax(200_000.0), 0.0);
        assert_eq!(Form1701Draft::graduated_tax(250_000.0), 0.0);
        assert!((Form1701Draft::graduated_tax(300_000.0) - 7_500.0).abs() < 0.01);
        assert!((Form1701Draft::graduated_tax(500_000.0) - 42_500.0).abs() < 0.01);
        assert!((Form1701Draft::graduated_tax(1_000_000.0) - 152_500.0).abs() < 0.01);
        assert!((Form1701Draft::graduated_tax(5_000_000.0) - 1_302_500.0).abs() < 0.01);
        assert!((Form1701Draft::graduated_tax(10_000_000.0) - 2_902_500.0).abs() < 0.01);
    }

    #[test]
    fn test_recompute_graduated_with_osd() {
        let mut d = make_draft();
        d.rdo_pg1i21tax_rate_g = true;
        d.rdo_pg1i21tax_rate_p = false;
        d.rdo_pg1i21amethod_deduction_o = true;
        d.txt_pg1m_i1gschd_b = 1_000_000.0;
        d.recompute();
        // Gross=1M, OSD=400k, Net=600k, Tax=22500+(200k*20%)=62500
        assert!((d.txt_pg1m_i16cschd_b - 600_000.0).abs() < 0.01);
        assert!((d.txt_pg1i22atax_due - 62_500.0).abs() < 0.01);
    }

    #[test]
    fn test_recompute_8pct_flat() {
        let mut d = make_draft();
        d.rdo_pg1i21tax_rate_p = true;
        d.txt_pg1m_i1gschd_b = 500_000.0;
        d.recompute();
        // 8% of (500k-250k) = 20,000
        assert!((d.txt_pg1i22atax_due - 20_000.0).abs() < 0.01);
    }

    #[test]
    fn test_recompute_penalties_aggregate() {
        let mut d = make_draft();
        d.rdo_pg1i21tax_rate_g = true;
        d.rdo_pg1i21amethod_deduction_o = true;
        d.txt_pg1m_i1gschd_b = 500_000.0;
        d.txt_pg1i25a = 100.0;
        d.txt_pg1i26a = 50.0;
        d.txt_pg1i27a = 25.0;
        d.recompute();
        assert!((d.txt_pg1i28a - 175.0).abs() < 0.01);
        // Net=300k, tax=(300k-250k)*15%=7500, total=7500+175=7675
        assert!((d.txt_pg1i32aggregate_amt_pyble - 7_675.0).abs() < 0.01);
    }
}
