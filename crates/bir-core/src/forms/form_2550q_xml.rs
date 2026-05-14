//! BIR field mapping for Form 2550Qv2024.
//!
//! Auto-generated from savefile: 00000000000000-2550Qv2024-122025Q1.xml
//! Maps Rust struct fields to BIR pseudo-XML field IDs.

use super::form_2550q::Form2550QDraft;

use std::collections::BTreeMap;

impl Form2550QDraft {
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let (tin1, tin2, tin3, branch) = split_tin(&self.tin);

        // === Common fields (all forms) ===
        insert(&mut fields, "driveSelectTPExport", "0");
        insert(&mut fields, "ebirOnlineConfirmUsername", "");
        insert(&mut fields, "ebirOnlineSecret", "");
        insert(&mut fields, "ebirOnlineUsername", "");
        insert(&mut fields, "txtEmail", self.email.clone());
        insert(&mut fields, "txtEnroll", "Y");
        insert(&mut fields, "txtFinalFlag", "1");

        // === Form-specific fields ===
        insert(&mut fields, "dateFiled", self.date_filed.clone());
        // Quarter radio buttons — derive from month
        let q = ((self.month.saturating_sub(1)) / 3) + 1;
        insert_bool(&mut fields, "frm2550qv2024:OptQuarter1", q == 1);
        insert_bool(&mut fields, "frm2550qv2024:OptQuarter2", q == 2);
        insert_bool(&mut fields, "frm2550qv2024:OptQuarter3", q == 3);
        insert_bool(&mut fields, "frm2550qv2024:OptQuarter4", q == 4);
        insert_bool(
            &mut fields,
            "frm2550qv2024:OptShortPrd1",
            self.opt_short_prd1,
        );
        insert_bool(
            &mut fields,
            "frm2550qv2024:OptShortPrd2",
            self.opt_short_prd2,
        );
        insert(
            &mut fields,
            "frm2550qv2024:Pg2TaxPayer",
            self.pg2tax_payer.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:RtnPeriodFromNo4",
            self.rtn_period_from_no4.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:RtnPeriodToNo4",
            self.rtn_period_to_no4.clone(),
        );
        insert_money(&mut fields, "frm2550qv2024:addInputVat", self.add_input_vat);
        insert_money(
            &mut fields,
            "frm2550qv2024:addOutputVat",
            self.add_output_vat,
        );
        insert(
            &mut fields,
            "frm2550qv2024:addSpecifyNo19",
            self.add_specify_no19.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:addSpecifyNo42",
            self.add_specify_no42.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:addSpecifyNo47",
            self.add_specify_no47.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:addSpecifyNo56",
            self.add_specify_no56.clone(),
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:adjDeductions",
            self.adj_deductions,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:advVatPayment",
            self.adv_vat_payment,
        );
        insert_bool(
            &mut fields,
            "frm2550qv2024:amendedReturnNo5",
            self.amended_return_no5,
        );
        insert_bool(
            &mut fields,
            "frm2550qv2024:amendedReturnYesNo5",
            self.amended_return_yes_no5,
        );
        insert(
            &mut fields,
            "frm2550qv2024:branchCode",
            self.branch_code.to_string(),
        );
        insert_bool(&mut fields, "frm2550qv2024:calendarNo1", self.calendar_no1);
        insert_money(&mut fields, "frm2550qv2024:compromise", self.compromise);
        insert_money(
            &mut fields,
            "frm2550qv2024:creditableVat",
            self.creditable_vat,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:domesticInputTax",
            self.domestic_input_tax,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:domesticPurchase",
            self.domestic_purchase,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:domesticPurchaseNoTax",
            self.domestic_purchase_no_tax,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:excessCredits",
            self.excess_credits,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:excessInputTax",
            self.excess_input_tax,
        );
        insert_money(&mut fields, "frm2550qv2024:exemptSales", self.exempt_sales);
        insert_bool(&mut fields, "frm2550qv2024:fiscalNo1", self.fiscal_no1);
        insert_money(
            &mut fields,
            "frm2550qv2024:importCapitalInputTax",
            self.import_capital_input_tax,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:importInputTax",
            self.import_input_tax,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:importPurchase",
            self.import_purchase,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:inputTaxAttr",
            self.input_tax_attr,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:inputTaxCarried",
            self.input_tax_carried,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:inputTaxDeferred",
            self.input_tax_deferred,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:inputVatUnpaid",
            self.input_vat_unpaid,
        );
        insert_money(&mut fields, "frm2550qv2024:interest", self.interest);
        insert_bool(
            &mut fields,
            "frm2550qv2024:internationalTreatyYn",
            self.international_treaty_yn,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:lessOutputVat",
            self.less_output_vat,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:netVatPayable",
            self.net_vat_payable,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:otherCreditsNo19",
            self.other_credits_no19,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:otherSpecify42",
            self.other_specify42,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:otherSpecify47",
            self.other_specify47,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:otherSpecify56",
            self.other_specify56,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:outputTaxDue",
            self.output_tax_due,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:outputVatSales",
            self.output_vat_sales,
        );
        insert_money(&mut fields, "frm2550qv2024:penalties", self.penalties);
        insert_money(
            &mut fields,
            "frm2550qv2024:presumptiveInputTax",
            self.presumptive_input_tax,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:sched2AmountInputTax",
            self.sched2amount_input_tax,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:sched2InputTaxDirect",
            self.sched2input_tax_direct,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:sched2TotalAttr",
            self.sched2total_attr,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:sched2TotalRatable",
            self.sched2total_ratable,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:sched2TotalSales",
            self.sched2total_sales,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:sched2VatExemptSale",
            self.sched2vat_exempt_sale,
        );
        insert(
            &mut fields,
            "frm2550qv2024:selectedMonthNo2",
            format!("{:02}", self.month),
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:serviceInputTax",
            self.service_input_tax,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:servicesPurchase",
            self.services_purchase,
        );
        insert_bool(
            &mut fields,
            "frm2550qv2024:specialRateYn",
            self.special_rate_yn,
        );
        insert(
            &mut fields,
            "frm2550qv2024:specifyInternationalTreaty",
            self.specify_international_treaty.clone(),
        );
        insert_money(&mut fields, "frm2550qv2024:surcharge", self.surcharge);
        insert_bool(
            &mut fields,
            "frm2550qv2024:taxPayerClassification1",
            self.tax_payer_classification1,
        );
        insert_bool(
            &mut fields,
            "frm2550qv2024:taxPayerClassification2",
            self.tax_payer_classification2,
        );
        insert_bool(
            &mut fields,
            "frm2550qv2024:taxPayerClassification3",
            self.tax_payer_classification3,
        );
        insert_bool(
            &mut fields,
            "frm2550qv2024:taxPayerClassification4",
            self.tax_payer_classification4,
        );
        insert(
            &mut fields,
            "frm2550qv2024:taxpayerAddress",
            self.taxpayer_address.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:taxpayerContactNumber",
            self.taxpayer_contact_number.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:taxpayerEmailAddress",
            self.taxpayer_email_address.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:taxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:taxpayerZip",
            self.taxpayer_zip.to_string(),
        );
        insert_money(&mut fields, "frm2550qv2024:total43", self.total43);
        insert_money(
            &mut fields,
            "frm2550qv2024:totalAdjOutput",
            self.total_adj_output,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:totalAllowInputTax",
            self.total_allow_input_tax,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:totalAvailInputTax",
            self.total_avail_input_tax,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:totalCurInputTax",
            self.total_cur_input_tax,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:totalCurPurchase",
            self.total_cur_purchase,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:totalDeductions",
            self.total_deductions,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:totalPayable",
            self.total_payable,
        );
        insert_money(&mut fields, "frm2550qv2024:totalSales", self.total_sales);
        insert_money(
            &mut fields,
            "frm2550qv2024:totalTaxCredits",
            self.total_tax_credits,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:transitionalInputTax",
            self.transitional_input_tax,
        );
        insert(
            &mut fields,
            "frm2550qv2024:txtCurrentPage",
            self.txt_current_page.to_string(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:txtMaxPage",
            self.txt_max_page.to_string(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:txtPg2BranchCode",
            branch.clone(),
        );
        insert(&mut fields, "frm2550qv2024:txtPg2TIN1", tin1.clone());
        insert(&mut fields, "frm2550qv2024:txtPg2TIN2", tin2.clone());
        insert(&mut fields, "frm2550qv2024:txtPg2TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm2550qv2024:txtRDOCode",
            self.rdo_code.clone(),
        );
        insert(&mut fields, "frm2550qv2024:txtTIN1", tin1.clone());
        insert(&mut fields, "frm2550qv2024:txtTIN2", tin2.clone());
        insert(&mut fields, "frm2550qv2024:txtTIN3", tin3.clone());
        insert(
            &mut fields,
            "frm2550qv2024:txtYearNo2",
            self.taxable_year.to_string(),
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:vatExemptImports",
            self.vat_exempt_imports,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:vatPaidReturn",
            self.vat_paid_return,
        );
        insert_money(&mut fields, "frm2550qv2024:vatRefund", self.vat_refund);
        insert_money(
            &mut fields,
            "frm2550qv2024:vatableSales",
            self.vatable_sales,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:zeroRatedSales",
            self.zero_rated_sales,
        );
        insert_money(&mut fields, "otherSpecify47B", self.other_specify47b);
        insert_money(
            &mut fields,
            "resultOtherCreditsNo19",
            self.result_other_credits_no19,
        );
        insert_money(
            &mut fields,
            "resultOtherCreditsNo42",
            self.result_other_credits_no42,
        );
        insert_money(
            &mut fields,
            "resultOtherCreditsNo47",
            self.result_other_credits_no47,
        );
        insert_money(
            &mut fields,
            "resultOtherCreditsNo56",
            self.result_other_credits_no56,
        );
        insert_money(&mut fields, "sched1TotalBalNext", self.sched1total_bal_next);
        insert_money(&mut fields, "sched1TotalBalPrev", self.sched1total_bal_prev);
        insert_money(&mut fields, "sched3TotalIncome", self.sched3total_income);
        insert_money(&mut fields, "sched3TotalTax", self.sched3total_tax);
        insert_money(&mut fields, "sched4AmountPaid", self.sched4amount_paid);
        insert_money(
            &mut fields,
            "txtAllowedInputTax10",
            self.txt_allowed_input_tax10,
        );
        insert_money(
            &mut fields,
            "txtAllowedInputTax11",
            self.txt_allowed_input_tax11,
        );
        insert_money(&mut fields, "txtAmountPaid40", self.txt_amount_paid40);
        insert_money(&mut fields, "txtAmountPaid41", self.txt_amount_paid41);
        insert_money(
            &mut fields,
            "txtAmountPaidSched4",
            self.txt_amount_paid_sched4,
        );
        insert_money(
            &mut fields,
            "txtAmountPurchase10",
            self.txt_amount_purchase10,
        );
        insert_money(
            &mut fields,
            "txtAmountPurchase11",
            self.txt_amount_purchase11,
        );
        insert_money(
            &mut fields,
            "txtBalanceInputTax10",
            self.txt_balance_input_tax10,
        );
        insert_money(
            &mut fields,
            "txtBalanceInputTax11",
            self.txt_balance_input_tax11,
        );
        insert(&mut fields, "txtDate40", self.txt_date40.clone());
        insert(&mut fields, "txtDate41", self.txt_date41.clone());
        insert(&mut fields, "txtDate4To0", self.txt_date4to0.clone());
        insert(&mut fields, "txtDate4To1", self.txt_date4to1.clone());
        insert(
            &mut fields,
            "txtDateCovered30",
            self.txt_date_covered30.clone(),
        );
        insert(
            &mut fields,
            "txtDateCovered31",
            self.txt_date_covered31.clone(),
        );
        insert(
            &mut fields,
            "txtDateCovered3To0",
            self.txt_date_covered3to0.clone(),
        );
        insert(
            &mut fields,
            "txtDateCovered3To1",
            self.txt_date_covered3to1.clone(),
        );
        insert(
            &mut fields,
            "txtDatePurchase10",
            self.txt_date_purchase10.clone(),
        );
        insert(
            &mut fields,
            "txtDatePurchase11",
            self.txt_date_purchase11.clone(),
        );
        insert(
            &mut fields,
            "txtDescription10",
            self.txt_description10.clone(),
        );
        insert(
            &mut fields,
            "txtDescription11",
            self.txt_description11.clone(),
        );
        insert_money(&mut fields, "txtEstimatedLife10", self.txt_estimated_life10);
        insert_money(&mut fields, "txtEstimatedLife11", self.txt_estimated_life11);
        insert_money(&mut fields, "txtIncomePayment30", self.txt_income_payment30);
        insert_money(&mut fields, "txtIncomePayment31", self.txt_income_payment31);
        insert_money(&mut fields, "txtInputTax10", self.txt_input_tax10);
        insert_money(&mut fields, "txtInputTax11", self.txt_input_tax11);
        insert(
            &mut fields,
            "txtNameOfMiller40",
            self.txt_name_of_miller40.clone(),
        );
        insert(
            &mut fields,
            "txtNameOfMiller41",
            self.txt_name_of_miller41.clone(),
        );
        insert(
            &mut fields,
            "txtNameOfTaxpayer40",
            self.txt_name_of_taxpayer40.clone(),
        );
        insert(
            &mut fields,
            "txtNameOfTaxpayer41",
            self.txt_name_of_taxpayer41.clone(),
        );
        insert(
            &mut fields,
            "txtNameWithHoldingAgent30",
            self.txt_name_with_holding_agent30.clone(),
        );
        insert(
            &mut fields,
            "txtNameWithHoldingAgent31",
            self.txt_name_with_holding_agent31.clone(),
        );
        insert_money(
            &mut fields,
            "txtOfficialReceiptNumber40",
            self.txt_official_receipt_number40,
        );
        insert_money(
            &mut fields,
            "txtOfficialReceiptNumber41",
            self.txt_official_receipt_number41,
        );
        insert_money(
            &mut fields,
            "txtRecognizedLife10",
            self.txt_recognized_life10,
        );
        insert_money(
            &mut fields,
            "txtRecognizedLife11",
            self.txt_recognized_life11,
        );
        insert(
            &mut fields,
            "txtSourceCode10",
            self.txt_source_code10.clone(),
        );
        insert(
            &mut fields,
            "txtSourceCode11",
            self.txt_source_code11.clone(),
        );
        insert_money(
            &mut fields,
            "txtTotalAmoungOfTaxWithHeld",
            self.txt_total_amoung_of_tax_with_held,
        );
        insert_money(
            &mut fields,
            "txtTotalAmountOfBalanceofInputTaxFromPrevious",
            self.txt_total_amount_of_balanceof_input_tax_from_previous,
        );
        insert_money(
            &mut fields,
            "txtTotalAmountOfBalanceofInputTaxToBeCarried",
            self.txt_total_amount_of_balanceof_input_tax_to_be_carried,
        );
        insert_money(
            &mut fields,
            "txtTotalAmountofIncomePayment",
            self.txt_total_amountof_income_payment,
        );
        insert_money(
            &mut fields,
            "txtTotalTaxWithHeld30",
            self.txt_total_tax_with_held30,
        );
        insert_money(
            &mut fields,
            "txtTotalTaxWithHeld31",
            self.txt_total_tax_with_held31,
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
