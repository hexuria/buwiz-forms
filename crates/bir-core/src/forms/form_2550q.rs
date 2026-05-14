//! BIR Form 2550Qv2024 — Typed draft struct and computation logic.
//!
//! Generated from savefile: 00000000000000-2550Qv2024-122025Q1.xml
//! Total BIR fields: 160
//! Form-specific fields: 139
//!
//! ⚠️ ScaffoldOnly — formula evidence not yet verified

use crate::forms::{FilingStatus, FormValidator};
use crate::profile::TaxpayerProfile;
use serde::{Deserialize, Serialize};

/// Complete draft for Form 2550Qv2024.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form2550QDraft {
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

    // === other ===
    /// BIR: `dateFiled` (sample: `2026/05/08`)
    pub date_filed: String,
    /// BIR: `frm2550qv2024:OptShortPrd1` (sample: `false`)
    pub opt_short_prd1: bool,
    /// BIR: `frm2550qv2024:OptShortPrd2` (sample: `true`)
    pub opt_short_prd2: bool,
    /// BIR: `frm2550qv2024:Pg2TaxPayer` (sample: `JUAN DELA CRUZ`)
    pub pg2tax_payer: String,
    /// BIR: `frm2550qv2024:RtnPeriodFromNo4` (sample: `1/01/2025`)
    pub rtn_period_from_no4: String,
    /// BIR: `frm2550qv2024:RtnPeriodToNo4` (sample: `3/31/2025`)
    pub rtn_period_to_no4: String,
    /// BIR: `frm2550qv2024:addInputVat` (sample: `10,000.00`)
    pub add_input_vat: f64,
    /// BIR: `frm2550qv2024:addOutputVat` (sample: `1,000.00`)
    pub add_output_vat: f64,
    /// BIR: `frm2550qv2024:addSpecifyNo19` (sample: `RXAMPLE`)
    pub add_specify_no19: String,
    /// BIR: `frm2550qv2024:addSpecifyNo42` (sample: `EXAMPLE2`)
    pub add_specify_no42: String,
    /// BIR: `frm2550qv2024:addSpecifyNo47` (sample: `EXAMPLE 3`)
    pub add_specify_no47: String,
    /// BIR: `frm2550qv2024:addSpecifyNo56` (sample: `EXAMPLE 4`)
    pub add_specify_no56: String,
    /// BIR: `frm2550qv2024:adjDeductions` (sample: `13,000.00`)
    pub adj_deductions: f64,
    /// BIR: `frm2550qv2024:advVatPayment` (sample: `0.00`)
    pub adv_vat_payment: f64,
    /// BIR: `frm2550qv2024:amendedReturnNo5` (sample: `true`)
    pub amended_return_no5: bool,
    /// BIR: `frm2550qv2024:amendedReturnYesNo5` (sample: `false`)
    pub amended_return_yes_no5: bool,
    /// BIR: `frm2550qv2024:branchCode` (sample: `00000`)
    pub branch_code: u32,
    /// BIR: `frm2550qv2024:calendarNo1` (sample: `true`)
    pub calendar_no1: bool,
    /// BIR: `frm2550qv2024:compromise` (sample: `200.00`)
    pub compromise: f64,
    /// BIR: `frm2550qv2024:creditableVat` (sample: `0.00`)
    pub creditable_vat: f64,
    /// BIR: `frm2550qv2024:domesticInputTax` (sample: `120.00`)
    pub domestic_input_tax: f64,
    /// BIR: `frm2550qv2024:domesticPurchase` (sample: `1,000.00`)
    pub domestic_purchase: f64,
    /// BIR: `frm2550qv2024:domesticPurchaseNoTax` (sample: `1,000.00`)
    pub domestic_purchase_no_tax: f64,
    /// BIR: `frm2550qv2024:excessCredits` (sample: `-2,440.00`)
    pub excess_credits: f64,
    /// BIR: `frm2550qv2024:excessInputTax` (sample: `-1,440.00`)
    pub excess_input_tax: f64,
    /// BIR: `frm2550qv2024:exemptSales` (sample: `1,000.00`)
    pub exempt_sales: f64,
    /// BIR: `frm2550qv2024:fiscalNo1` (sample: `false`)
    pub fiscal_no1: bool,
    /// BIR: `frm2550qv2024:importCapitalInputTax` (sample: `0.00`)
    pub import_capital_input_tax: f64,
    /// BIR: `frm2550qv2024:importInputTax` (sample: `1,200.00`)
    pub import_input_tax: f64,
    /// BIR: `frm2550qv2024:importPurchase` (sample: `10,000.00`)
    pub import_purchase: f64,
    /// BIR: `frm2550qv2024:inputTaxAttr` (sample: `0.00`)
    pub input_tax_attr: f64,
    /// BIR: `frm2550qv2024:inputTaxCarried` (sample: `1,000.00`)
    pub input_tax_carried: f64,
    /// BIR: `frm2550qv2024:inputTaxDeferred` (sample: `0.00`)
    pub input_tax_deferred: f64,
    /// BIR: `frm2550qv2024:inputVatUnpaid` (sample: `1,000.00`)
    pub input_vat_unpaid: f64,
    /// BIR: `frm2550qv2024:interest` (sample: `100.00`)
    pub interest: f64,
    /// BIR: `frm2550qv2024:internationalTreatyYn` (sample: `false`)
    pub international_treaty_yn: bool,
    /// BIR: `frm2550qv2024:lessOutputVat` (sample: `1,000.00`)
    pub less_output_vat: f64,
    /// BIR: `frm2550qv2024:netVatPayable` (sample: `-1,440.00`)
    pub net_vat_payable: f64,
    /// BIR: `frm2550qv2024:otherCreditsNo19` (sample: `1,000.00`)
    pub other_credits_no19: f64,
    /// BIR: `frm2550qv2024:otherSpecify42` (sample: `10,000.00`)
    pub other_specify42: f64,
    /// BIR: `frm2550qv2024:otherSpecify47` (sample: `1,000.00`)
    pub other_specify47: f64,
    /// BIR: `frm2550qv2024:otherSpecify56` (sample: `1,000.00`)
    pub other_specify56: f64,
    /// BIR: `frm2550qv2024:outputTaxDue` (sample: `120.00`)
    pub output_tax_due: f64,
    /// BIR: `frm2550qv2024:outputVatSales` (sample: `120.00`)
    pub output_vat_sales: f64,
    /// BIR: `frm2550qv2024:penalties` (sample: `1,300.00`)
    pub penalties: f64,
    /// BIR: `frm2550qv2024:presumptiveInputTax` (sample: `1,000.00`)
    pub presumptive_input_tax: f64,
    /// BIR: `frm2550qv2024:sched2AmountInputTax` (sample: `0.00`)
    pub sched2amount_input_tax: f64,
    /// BIR: `frm2550qv2024:sched2InputTaxDirect` (sample: `0.00`)
    pub sched2input_tax_direct: f64,
    /// BIR: `frm2550qv2024:sched2TotalAttr` (sample: `0.00`)
    pub sched2total_attr: f64,
    /// BIR: `frm2550qv2024:sched2TotalRatable` (sample: `0.00`)
    pub sched2total_ratable: f64,
    /// BIR: `frm2550qv2024:sched2TotalSales` (sample: `3,000.00`)
    pub sched2total_sales: f64,
    /// BIR: `frm2550qv2024:sched2VatExemptSale` (sample: `1,000.00`)
    pub sched2vat_exempt_sale: f64,
    /// BIR: `frm2550qv2024:serviceInputTax` (sample: `120.00`)
    pub service_input_tax: f64,
    /// BIR: `frm2550qv2024:servicesPurchase` (sample: `1,000.00`)
    pub services_purchase: f64,
    /// BIR: `frm2550qv2024:specialRateYn` (sample: `true`)
    pub special_rate_yn: bool,
    /// BIR: `frm2550qv2024:specifyInternationalTreaty` (sample: ``)
    pub specify_international_treaty: String,
    /// BIR: `frm2550qv2024:surcharge` (sample: `1,000.00`)
    pub surcharge: f64,
    /// BIR: `frm2550qv2024:taxPayerClassification1` (sample: `false`)
    pub tax_payer_classification1: bool,
    /// BIR: `frm2550qv2024:taxPayerClassification2` (sample: `false`)
    pub tax_payer_classification2: bool,
    /// BIR: `frm2550qv2024:taxPayerClassification3` (sample: `true`)
    pub tax_payer_classification3: bool,
    /// BIR: `frm2550qv2024:taxPayerClassification4` (sample: `false`)
    pub tax_payer_classification4: bool,
    /// BIR: `frm2550qv2024:taxpayerAddress` (sample: `OLONGAPO`)
    pub taxpayer_address: String,
    /// BIR: `frm2550qv2024:taxpayerContactNumber` (sample: `09123456789`)
    pub taxpayer_contact_number: String,
    /// BIR: `frm2550qv2024:taxpayerEmailAddress` (sample: `CODEITLIKEMILEY@GMAIL.COM`)
    pub taxpayer_email_address: String,
    /// BIR: `frm2550qv2024:taxpayerZip` (sample: `2200`)
    pub taxpayer_zip: u32,
    /// BIR: `frm2550qv2024:total43` (sample: `13,000.00`)
    pub total43: f64,
    /// BIR: `frm2550qv2024:totalAdjOutput` (sample: `120.00`)
    pub total_adj_output: f64,
    /// BIR: `frm2550qv2024:totalAllowInputTax` (sample: `1,560.00`)
    pub total_allow_input_tax: f64,
    /// BIR: `frm2550qv2024:totalAvailInputTax` (sample: `14,560.00`)
    pub total_avail_input_tax: f64,
    /// BIR: `frm2550qv2024:totalCurInputTax` (sample: `1,560.00`)
    pub total_cur_input_tax: f64,
    /// BIR: `frm2550qv2024:totalCurPurchase` (sample: `15,000.00`)
    pub total_cur_purchase: f64,
    /// BIR: `frm2550qv2024:totalDeductions` (sample: `3,000.00`)
    pub total_deductions: f64,
    /// BIR: `frm2550qv2024:totalPayable` (sample: `1,300.00`)
    pub total_payable: f64,
    /// BIR: `frm2550qv2024:totalSales` (sample: `3,000.00`)
    pub total_sales: f64,
    /// BIR: `frm2550qv2024:totalTaxCredits` (sample: `1,000.00`)
    pub total_tax_credits: f64,
    /// BIR: `frm2550qv2024:transitionalInputTax` (sample: `1,000.00`)
    pub transitional_input_tax: f64,
    /// BIR: `frm2550qv2024:vatExemptImports` (sample: `1,000.00`)
    pub vat_exempt_imports: f64,
    /// BIR: `frm2550qv2024:vatPaidReturn` (sample: `0.00`)
    pub vat_paid_return: f64,
    /// BIR: `frm2550qv2024:vatRefund` (sample: `1,000.00`)
    pub vat_refund: f64,
    /// BIR: `frm2550qv2024:vatableSales` (sample: `1,000.00`)
    pub vatable_sales: f64,
    /// BIR: `frm2550qv2024:zeroRatedSales` (sample: `1,000.00`)
    pub zero_rated_sales: f64,
    /// BIR: `otherSpecify47B` (sample: `120.00`)
    pub other_specify47b: f64,
    /// BIR: `resultOtherCreditsNo19` (sample: `0.00`)
    pub result_other_credits_no19: f64,
    /// BIR: `resultOtherCreditsNo42` (sample: `0.00`)
    pub result_other_credits_no42: f64,
    /// BIR: `resultOtherCreditsNo47` (sample: `0.00`)
    pub result_other_credits_no47: f64,
    /// BIR: `resultOtherCreditsNo56` (sample: `0.00`)
    pub result_other_credits_no56: f64,
    /// BIR: `sched1TotalBalNext` (sample: `0.00`)
    pub sched1total_bal_next: f64,
    /// BIR: `sched1TotalBalPrev` (sample: `0.00`)
    pub sched1total_bal_prev: f64,
    /// BIR: `sched3TotalIncome` (sample: `0.00`)
    pub sched3total_income: f64,
    /// BIR: `sched3TotalTax` (sample: `0.00`)
    pub sched3total_tax: f64,
    /// BIR: `sched4AmountPaid` (sample: `0.00`)
    pub sched4amount_paid: f64,

    // === shared_text ===
    /// BIR: `txtAllowedInputTax10` (sample: `0.00`)
    pub txt_allowed_input_tax10: f64,
    /// BIR: `txtAllowedInputTax11` (sample: `0.00`)
    pub txt_allowed_input_tax11: f64,
    /// BIR: `txtAmountPaid40` (sample: `0.00`)
    pub txt_amount_paid40: f64,
    /// BIR: `txtAmountPaid41` (sample: `0.00`)
    pub txt_amount_paid41: f64,
    /// BIR: `txtAmountPaidSched4` (sample: `0.00`)
    pub txt_amount_paid_sched4: f64,
    /// BIR: `txtAmountPurchase10` (sample: `0.00`)
    pub txt_amount_purchase10: f64,
    /// BIR: `txtAmountPurchase11` (sample: `0.00`)
    pub txt_amount_purchase11: f64,
    /// BIR: `txtBalanceInputTax10` (sample: `0.00`)
    pub txt_balance_input_tax10: f64,
    /// BIR: `txtBalanceInputTax11` (sample: `0.00`)
    pub txt_balance_input_tax11: f64,
    /// BIR: `txtDate40` (sample: ``)
    pub txt_date40: String,
    /// BIR: `txtDate41` (sample: ``)
    pub txt_date41: String,
    /// BIR: `txtDate4To0` (sample: ``)
    pub txt_date4to0: String,
    /// BIR: `txtDate4To1` (sample: ``)
    pub txt_date4to1: String,
    /// BIR: `txtDateCovered30` (sample: ``)
    pub txt_date_covered30: String,
    /// BIR: `txtDateCovered31` (sample: ``)
    pub txt_date_covered31: String,
    /// BIR: `txtDateCovered3To0` (sample: ``)
    pub txt_date_covered3to0: String,
    /// BIR: `txtDateCovered3To1` (sample: ``)
    pub txt_date_covered3to1: String,
    /// BIR: `txtDatePurchase10` (sample: ``)
    pub txt_date_purchase10: String,
    /// BIR: `txtDatePurchase11` (sample: ``)
    pub txt_date_purchase11: String,
    /// BIR: `txtDescription10` (sample: ``)
    pub txt_description10: String,
    /// BIR: `txtDescription11` (sample: ``)
    pub txt_description11: String,
    /// BIR: `txtEstimatedLife10` (sample: `0.00`)
    pub txt_estimated_life10: f64,
    /// BIR: `txtEstimatedLife11` (sample: `0.00`)
    pub txt_estimated_life11: f64,
    /// BIR: `txtIncomePayment30` (sample: `0.00`)
    pub txt_income_payment30: f64,
    /// BIR: `txtIncomePayment31` (sample: `0.00`)
    pub txt_income_payment31: f64,
    /// BIR: `txtInputTax10` (sample: `0.00`)
    pub txt_input_tax10: f64,
    /// BIR: `txtInputTax11` (sample: `0.00`)
    pub txt_input_tax11: f64,
    /// BIR: `txtNameOfMiller40` (sample: ``)
    pub txt_name_of_miller40: String,
    /// BIR: `txtNameOfMiller41` (sample: ``)
    pub txt_name_of_miller41: String,
    /// BIR: `txtNameOfTaxpayer40` (sample: ``)
    pub txt_name_of_taxpayer40: String,
    /// BIR: `txtNameOfTaxpayer41` (sample: ``)
    pub txt_name_of_taxpayer41: String,
    /// BIR: `txtNameWithHoldingAgent30` (sample: ``)
    pub txt_name_with_holding_agent30: String,
    /// BIR: `txtNameWithHoldingAgent31` (sample: ``)
    pub txt_name_with_holding_agent31: String,
    /// BIR: `txtOfficialReceiptNumber40` (sample: `0.00`)
    pub txt_official_receipt_number40: f64,
    /// BIR: `txtOfficialReceiptNumber41` (sample: `0.00`)
    pub txt_official_receipt_number41: f64,
    /// BIR: `txtRecognizedLife10` (sample: `0.00`)
    pub txt_recognized_life10: f64,
    /// BIR: `txtRecognizedLife11` (sample: `0.00`)
    pub txt_recognized_life11: f64,
    /// BIR: `txtSourceCode10` (sample: ``)
    pub txt_source_code10: String,
    /// BIR: `txtSourceCode11` (sample: ``)
    pub txt_source_code11: String,
    /// BIR: `txtTotalAmoungOfTaxWithHeld` (sample: `0.00`)
    pub txt_total_amoung_of_tax_with_held: f64,
    /// BIR: `txtTotalAmountOfBalanceofInputTaxFromPrevious` (sample: `0.00`)
    pub txt_total_amount_of_balanceof_input_tax_from_previous: f64,
    /// BIR: `txtTotalAmountOfBalanceofInputTaxToBeCarried` (sample: `0.00`)
    pub txt_total_amount_of_balanceof_input_tax_to_be_carried: f64,
    /// BIR: `txtTotalAmountofIncomePayment` (sample: `0.00`)
    pub txt_total_amountof_income_payment: f64,
    /// BIR: `txtTotalTaxWithHeld30` (sample: `0.00`)
    pub txt_total_tax_with_held30: f64,
    /// BIR: `txtTotalTaxWithHeld31` (sample: `0.00`)
    pub txt_total_tax_with_held31: f64,

    // === text_fields ===
    /// BIR: `frm2550qv2024:txtCurrentPage` (sample: `2`)
    pub txt_current_page: u32,
    /// BIR: `frm2550qv2024:txtMaxPage` (sample: `2`)
    pub txt_max_page: u32,

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

impl FormValidator for Form2550QDraft {
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

impl Form2550QDraft {
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
            date_filed: String::new(),
            opt_short_prd1: false,
            opt_short_prd2: true,
            pg2tax_payer: String::new(),
            rtn_period_from_no4: String::new(),
            rtn_period_to_no4: String::new(),
            add_input_vat: 0.0,
            add_output_vat: 0.0,
            add_specify_no19: String::new(),
            add_specify_no42: String::new(),
            add_specify_no47: String::new(),
            add_specify_no56: String::new(),
            adj_deductions: 0.0,
            adv_vat_payment: 0.0,
            amended_return_no5: true,
            amended_return_yes_no5: false,
            branch_code: 0,
            calendar_no1: true,
            compromise: 0.0,
            creditable_vat: 0.0,
            domestic_input_tax: 0.0,
            domestic_purchase: 0.0,
            domestic_purchase_no_tax: 0.0,
            excess_credits: 0.0,
            excess_input_tax: 0.0,
            exempt_sales: 0.0,
            fiscal_no1: false,
            import_capital_input_tax: 0.0,
            import_input_tax: 0.0,
            import_purchase: 0.0,
            input_tax_attr: 0.0,
            input_tax_carried: 0.0,
            input_tax_deferred: 0.0,
            input_vat_unpaid: 0.0,
            interest: 0.0,
            international_treaty_yn: false,
            less_output_vat: 0.0,
            net_vat_payable: 0.0,
            other_credits_no19: 0.0,
            other_specify42: 0.0,
            other_specify47: 0.0,
            other_specify56: 0.0,
            output_tax_due: 0.0,
            output_vat_sales: 0.0,
            penalties: 0.0,
            presumptive_input_tax: 0.0,
            sched2amount_input_tax: 0.0,
            sched2input_tax_direct: 0.0,
            sched2total_attr: 0.0,
            sched2total_ratable: 0.0,
            sched2total_sales: 0.0,
            sched2vat_exempt_sale: 0.0,
            service_input_tax: 0.0,
            services_purchase: 0.0,
            special_rate_yn: true,
            specify_international_treaty: String::new(),
            surcharge: 0.0,
            tax_payer_classification1: false,
            tax_payer_classification2: false,
            tax_payer_classification3: true,
            tax_payer_classification4: false,
            taxpayer_address: String::new(),
            taxpayer_contact_number: String::new(),
            taxpayer_email_address: String::new(),
            taxpayer_zip: 0,
            total43: 0.0,
            total_adj_output: 0.0,
            total_allow_input_tax: 0.0,
            total_avail_input_tax: 0.0,
            total_cur_input_tax: 0.0,
            total_cur_purchase: 0.0,
            total_deductions: 0.0,
            total_payable: 0.0,
            total_sales: 0.0,
            total_tax_credits: 0.0,
            transitional_input_tax: 0.0,
            vat_exempt_imports: 0.0,
            vat_paid_return: 0.0,
            vat_refund: 0.0,
            vatable_sales: 0.0,
            zero_rated_sales: 0.0,
            other_specify47b: 0.0,
            result_other_credits_no19: 0.0,
            result_other_credits_no42: 0.0,
            result_other_credits_no47: 0.0,
            result_other_credits_no56: 0.0,
            sched1total_bal_next: 0.0,
            sched1total_bal_prev: 0.0,
            sched3total_income: 0.0,
            sched3total_tax: 0.0,
            sched4amount_paid: 0.0,
            txt_allowed_input_tax10: 0.0,
            txt_allowed_input_tax11: 0.0,
            txt_amount_paid40: 0.0,
            txt_amount_paid41: 0.0,
            txt_amount_paid_sched4: 0.0,
            txt_amount_purchase10: 0.0,
            txt_amount_purchase11: 0.0,
            txt_balance_input_tax10: 0.0,
            txt_balance_input_tax11: 0.0,
            txt_date40: String::new(),
            txt_date41: String::new(),
            txt_date4to0: String::new(),
            txt_date4to1: String::new(),
            txt_date_covered30: String::new(),
            txt_date_covered31: String::new(),
            txt_date_covered3to0: String::new(),
            txt_date_covered3to1: String::new(),
            txt_date_purchase10: String::new(),
            txt_date_purchase11: String::new(),
            txt_description10: String::new(),
            txt_description11: String::new(),
            txt_estimated_life10: 0.0,
            txt_estimated_life11: 0.0,
            txt_income_payment30: 0.0,
            txt_income_payment31: 0.0,
            txt_input_tax10: 0.0,
            txt_input_tax11: 0.0,
            txt_name_of_miller40: String::new(),
            txt_name_of_miller41: String::new(),
            txt_name_of_taxpayer40: String::new(),
            txt_name_of_taxpayer41: String::new(),
            txt_name_with_holding_agent30: String::new(),
            txt_name_with_holding_agent31: String::new(),
            txt_official_receipt_number40: 0.0,
            txt_official_receipt_number41: 0.0,
            txt_recognized_life10: 0.0,
            txt_recognized_life11: 0.0,
            txt_source_code10: String::new(),
            txt_source_code11: String::new(),
            txt_total_amoung_of_tax_with_held: 0.0,
            txt_total_amount_of_balanceof_input_tax_from_previous: 0.0,
            txt_total_amount_of_balanceof_input_tax_to_be_carried: 0.0,
            txt_total_amountof_income_payment: 0.0,
            txt_total_tax_with_held30: 0.0,
            txt_total_tax_with_held31: 0.0,
            txt_current_page: 0,
            txt_max_page: 0,
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

    /// Recompute all derived fields per BIR 2550Q (Quarterly VAT Return) computation rules.
    ///
    /// Computation flow (per official BIR form instructions):
    ///
    /// **Sales & Output VAT:**
    /// - Total Sales = Vatable + Zero-Rated + Exempt + VAT-Exempt Imports
    /// - Output VAT on Sales = 12% × Vatable Sales
    /// - Total Adjusted Output VAT = Output VAT on Sales + Add Output VAT − Less Output VAT
    /// - Output Tax Due = Total Adjusted Output VAT
    ///
    /// **Purchases & Input VAT:**
    /// - Total Current Purchases = Domestic + Import + Services + Domestic(No Tax) + Capital Goods Import
    /// - Total Current Input Tax = Domestic Input + Import Input + Service Input + Import Capital Input
    /// - Total Available Input Tax = Current + Carried Forward + Transitional + Presumptive + Deferred + Others(19)
    ///
    /// **Deductions:**
    /// - Total Deductions = Adj Deductions + Other(42) + Other(47) + Other(56)
    /// - Total Allowable Input Tax = Total Available Input Tax − Total Deductions − Input Tax Attributed
    ///
    /// **Net VAT:**
    /// - Net VAT Payable = Output Tax Due − Total Allowable Input Tax
    ///   - If positive → tax payable; if negative → excess input tax carried forward
    /// - Excess Input Tax = −Net VAT Payable (when negative)
    ///
    /// **Credits & Final:**
    /// - Total Tax Credits = Creditable VAT + Advance VAT + VAT Paid on Return + VAT Refund
    /// - Excess Credits = Net VAT Payable − Total Tax Credits (may be negative = overpayment)
    /// - Total Penalties = Surcharge + Interest + Compromise
    /// - Total Payable = max(0, Excess Credits) + Total Penalties
    pub fn recompute(&mut self) {
        // ── Sales ──
        self.total_sales = self.vatable_sales
            + self.zero_rated_sales
            + self.exempt_sales
            + self.vat_exempt_imports;

        // ── Output VAT ──
        self.output_vat_sales = self.vatable_sales * 0.12;
        self.total_adj_output = self.output_vat_sales + self.add_output_vat - self.less_output_vat;
        self.output_tax_due = f64::max(0.0, self.total_adj_output);

        // ── Current Purchases ──
        self.total_cur_purchase = self.domestic_purchase
            + self.import_purchase
            + self.services_purchase
            + self.domestic_purchase_no_tax;

        // ── Current Input Tax ──
        self.total_cur_input_tax = self.domestic_input_tax
            + self.import_input_tax
            + self.service_input_tax
            + self.import_capital_input_tax;

        // ── Total Available Input Tax ──
        self.total_avail_input_tax = self.total_cur_input_tax
            + self.input_tax_carried
            + self.transitional_input_tax
            + self.presumptive_input_tax
            + self.input_tax_deferred
            + self.other_credits_no19;

        // ── Deductions from Input Tax ──
        self.total_deductions = self.adj_deductions
            + self.other_specify42
            + self.other_specify47
            + self.other_specify56;

        // ── Total Allowable Input Tax ──
        self.total_allow_input_tax = f64::max(
            0.0,
            self.total_avail_input_tax - self.total_deductions - self.input_tax_attr,
        );

        // ── Net VAT Payable ──
        self.net_vat_payable = self.output_tax_due - self.total_allow_input_tax;

        // ── Excess Input Tax (negative net = overpayment / carry-forward) ──
        self.excess_input_tax = if self.net_vat_payable < 0.0 {
            -self.net_vat_payable
        } else {
            0.0
        };

        // ── Tax Credits ──
        self.total_tax_credits =
            self.creditable_vat + self.adv_vat_payment + self.vat_paid_return + self.vat_refund;

        // ── Excess Credits ──
        let vat_after_credits = f64::max(0.0, self.net_vat_payable) - self.total_tax_credits;
        self.excess_credits = vat_after_credits;

        // ── Penalties ──
        self.penalties = self.surcharge + self.interest + self.compromise;

        // ── Total Payable ──
        self.total_payable = f64::max(0.0, vat_after_credits) + self.penalties;

        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    // ── State Transition Methods ──

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        assert!(matches!(self.status, FilingStatus::Draft), "Must be Draft");
        let errors = <Self as FormValidator>::validate(self);
        if !errors.is_empty() {
            return Err(errors);
        }
        self.status = FilingStatus::Queued;
        self.submission_attempts = 0;
        self.next_retry_at = Some(chrono::Utc::now().to_rfc3339());
        self.last_error = None;
        self.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
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
