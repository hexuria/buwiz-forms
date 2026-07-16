//! Checked 160-field editable-save contract for exact form `2550Qv2024`.
//!
//! The reviewed plain and decrypted encrypted payloads have the same field
//! union. Their only semantic transport difference is `txtFinalFlag` (`1` in
//! the editable save and `0` in the encrypted companion); the encrypted file
//! also writes `dateFiled` as a standalone element rather than a field div.

use std::collections::{BTreeMap, BTreeSet};

use super::FormValidator;
use super::form_2550q::{
    Form2550QAdvanceVatRow, Form2550QCapitalGoodRow, Form2550QCreditableVatRow, Form2550QDate,
    Form2550QDraft, Form2550QFilingBasis, Form2550QLocalPrintFields, Form2550QPartII,
    Form2550QPartIV, Form2550QQuarter, Form2550QSchedule2, Form2550QTaxpayerClassification,
    Form2550QXmlFinalFlag,
};

const EXACT_SOURCE_FIELD_COUNT: usize = 160;
const SCHEDULE_ROW_SUFFIXES: [u8; 2] = [10, 11];
const SCHEDULE_3_SUFFIXES: [u8; 2] = [0, 1];
const SCHEDULE_4_SUFFIXES: [u8; 2] = [0, 1];

impl Form2550QDraft {
    /// Serialize the exact reviewed editable-save key set. A clone is
    /// recomputed first so stale JSON-derived totals never enter the payload.
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut normalized = self.clone();
        normalized.recompute();
        normalized.to_reviewed_field_map()
    }

    fn to_reviewed_field_map(&self) -> BTreeMap<String, String> {
        debug_assert_eq!(expected_xml_keys().len(), EXACT_SOURCE_FIELD_COUNT);
        let mut fields = self.preserved_unmodeled_xml_fields.clone();
        let (tin1, tin2, tin3, branch) = split_tin(&self.tin);

        insert_bool_pair(
            &mut fields,
            "frm2550qv2024:calendarNo1",
            "frm2550qv2024:fiscalNo1",
            matches!(self.filing_basis, Form2550QFilingBasis::Calendar),
        );
        insert(
            &mut fields,
            "frm2550qv2024:selectedMonthNo2",
            format!("{:02}", self.year_end_month),
        );
        insert(
            &mut fields,
            "frm2550qv2024:txtYearNo2",
            self.taxable_year.to_string(),
        );
        insert_one_of_four(
            &mut fields,
            "frm2550qv2024:OptQuarter",
            self.quarter.number(),
        );
        insert_optional_date(
            &mut fields,
            "frm2550qv2024:RtnPeriodFromNo4",
            self.return_period_from,
        );
        insert_optional_date(
            &mut fields,
            "frm2550qv2024:RtnPeriodToNo4",
            self.return_period_to,
        );
        insert_bool_pair(
            &mut fields,
            "frm2550qv2024:amendedReturnYesNo5",
            "frm2550qv2024:amendedReturnNo5",
            self.is_amended,
        );
        insert_bool_pair(
            &mut fields,
            "frm2550qv2024:OptShortPrd1",
            "frm2550qv2024:OptShortPrd2",
            self.is_short_period_return,
        );

        insert(&mut fields, "frm2550qv2024:txtTIN1", tin1.clone());
        insert(&mut fields, "frm2550qv2024:txtTIN2", tin2.clone());
        insert(&mut fields, "frm2550qv2024:txtTIN3", tin3.clone());
        insert(&mut fields, "frm2550qv2024:branchCode", branch.clone());
        insert(
            &mut fields,
            "frm2550qv2024:txtRDOCode",
            self.rdo_code.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:taxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:taxpayerAddress",
            self.registered_address.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:taxpayerZip",
            self.zip_code.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:taxpayerContactNumber",
            self.contact_number.clone(),
        );
        insert(
            &mut fields,
            "frm2550qv2024:taxpayerEmailAddress",
            self.email.clone(),
        );
        insert_taxpayer_classification(&mut fields, self.taxpayer_classification);
        insert_bool_pair(
            &mut fields,
            "frm2550qv2024:internationalTreatyYn",
            "frm2550qv2024:specialRateYn",
            self.is_availing_tax_relief,
        );
        insert(
            &mut fields,
            "frm2550qv2024:specifyInternationalTreaty",
            self.tax_relief_details.clone(),
        );

        let p2 = &self.part_ii;
        insert_money(
            &mut fields,
            "frm2550qv2024:excessInputTax",
            p2.item_15_net_vat_payable_or_excess,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:creditableVat",
            p2.item_16_creditable_vat_withheld,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:advVatPayment",
            p2.item_17_advance_vat_payments,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:vatPaidReturn",
            p2.item_18_paid_on_previous_return,
        );
        insert(
            &mut fields,
            "frm2550qv2024:addSpecifyNo19",
            p2.item_19_description.clone(),
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:otherCreditsNo19",
            p2.item_19_other_credit_or_payment,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:totalTaxCredits",
            p2.item_20_total_credits_or_payments,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:excessCredits",
            p2.item_21_tax_payable_or_excess_credits,
        );
        insert_money(&mut fields, "frm2550qv2024:surcharge", p2.item_22_surcharge);
        insert_money(&mut fields, "frm2550qv2024:interest", p2.item_23_interest);
        insert_money(
            &mut fields,
            "frm2550qv2024:compromise",
            p2.item_24_compromise,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:penalties",
            p2.item_25_total_penalties,
        );
        insert_money(
            &mut fields,
            "frm2550qv2024:totalPayable",
            p2.item_26_total_amount_payable_or_excess,
        );

        insert(&mut fields, "frm2550qv2024:txtPg2TIN1", tin1);
        insert(&mut fields, "frm2550qv2024:txtPg2TIN2", tin2);
        insert(&mut fields, "frm2550qv2024:txtPg2TIN3", tin3);
        insert(&mut fields, "frm2550qv2024:txtPg2BranchCode", branch);
        insert(
            &mut fields,
            "frm2550qv2024:Pg2TaxPayer",
            self.taxpayer_name.clone(),
        );

        let p4 = &self.part_iv;
        for (key, value) in [
            ("frm2550qv2024:vatableSales", p4.item_31a_vatable_sales),
            ("frm2550qv2024:outputVatSales", p4.item_31b_output_tax),
            ("frm2550qv2024:zeroRatedSales", p4.item_32a_zero_rated_sales),
            ("frm2550qv2024:exemptSales", p4.item_33a_exempt_sales),
            ("frm2550qv2024:totalSales", p4.item_34a_total_sales),
            ("frm2550qv2024:outputTaxDue", p4.item_34b_output_tax_due),
            (
                "frm2550qv2024:lessOutputVat",
                p4.item_35b_less_output_vat_uncollected,
            ),
            (
                "frm2550qv2024:addOutputVat",
                p4.item_36b_add_output_vat_recovered,
            ),
            (
                "frm2550qv2024:totalAdjOutput",
                p4.item_37b_adjusted_output_tax_due,
            ),
            (
                "frm2550qv2024:inputTaxCarried",
                p4.item_38b_input_tax_carried,
            ),
            (
                "frm2550qv2024:inputTaxDeferred",
                p4.item_39b_input_tax_deferred,
            ),
            (
                "frm2550qv2024:transitionalInputTax",
                p4.item_40b_transitional_input_tax,
            ),
            (
                "frm2550qv2024:presumptiveInputTax",
                p4.item_41b_presumptive_input_tax,
            ),
            ("frm2550qv2024:otherSpecify42", p4.item_42b_other_input_tax),
            ("frm2550qv2024:total43", p4.item_43b_total_prior_input_tax),
            (
                "frm2550qv2024:domesticPurchase",
                p4.item_44a_domestic_purchases,
            ),
            (
                "frm2550qv2024:domesticInputTax",
                p4.item_44b_domestic_input_tax,
            ),
            (
                "frm2550qv2024:servicesPurchase",
                p4.item_45a_nonresident_services,
            ),
            (
                "frm2550qv2024:serviceInputTax",
                p4.item_45b_nonresident_service_input_tax,
            ),
            ("frm2550qv2024:importPurchase", p4.item_46a_importations),
            ("frm2550qv2024:importInputTax", p4.item_46b_import_input_tax),
            ("frm2550qv2024:otherSpecify47", p4.item_47a_other_purchases),
            ("otherSpecify47B", p4.item_47b_other_input_tax),
            (
                "frm2550qv2024:domesticPurchaseNoTax",
                p4.item_48a_domestic_purchases_no_input_tax,
            ),
            (
                "frm2550qv2024:vatExemptImports",
                p4.item_49a_vat_exempt_importations,
            ),
            (
                "frm2550qv2024:totalCurPurchase",
                p4.item_50a_total_current_purchases,
            ),
            (
                "frm2550qv2024:totalCurInputTax",
                p4.item_50b_total_current_input_tax,
            ),
            (
                "frm2550qv2024:totalAvailInputTax",
                p4.item_51b_total_available_input_tax,
            ),
            (
                "frm2550qv2024:importCapitalInputTax",
                p4.item_52b_deferred_capital_goods_input_tax,
            ),
            (
                "frm2550qv2024:inputTaxAttr",
                p4.item_53b_input_tax_attributable_to_exempt_sales,
            ),
            (
                "frm2550qv2024:vatRefund",
                p4.item_54b_vat_refund_or_tcc_claimed,
            ),
            (
                "frm2550qv2024:inputVatUnpaid",
                p4.item_55b_input_vat_on_unpaid_payables,
            ),
            ("frm2550qv2024:otherSpecify56", p4.item_56b_other_deduction),
            (
                "frm2550qv2024:totalDeductions",
                p4.item_57b_total_deductions,
            ),
            (
                "frm2550qv2024:addInputVat",
                p4.item_58b_input_vat_on_settled_payables,
            ),
            (
                "frm2550qv2024:adjDeductions",
                p4.item_59b_adjusted_deductions,
            ),
            (
                "frm2550qv2024:totalAllowInputTax",
                p4.item_60b_total_allowable_input_tax,
            ),
            (
                "frm2550qv2024:netVatPayable",
                p4.item_61b_net_vat_payable_or_excess,
            ),
        ] {
            insert_money(&mut fields, key, value);
        }
        for (key, value) in [
            (
                "frm2550qv2024:addSpecifyNo42",
                p4.item_42_description.as_str(),
            ),
            (
                "frm2550qv2024:addSpecifyNo47",
                p4.item_47_description.as_str(),
            ),
            (
                "frm2550qv2024:addSpecifyNo56",
                p4.item_56_description.as_str(),
            ),
        ] {
            insert(&mut fields, key, value);
        }

        for (offset, suffix) in SCHEDULE_ROW_SUFFIXES.into_iter().enumerate() {
            let row = self.schedule_1.get(offset).cloned().unwrap_or_default();
            insert_optional_date(
                &mut fields,
                &format!("txtDatePurchase{suffix}"),
                row.purchase_or_import_date,
            );
            insert(
                &mut fields,
                &format!("txtSourceCode{suffix}"),
                row.source_code,
            );
            insert(
                &mut fields,
                &format!("txtDescription{suffix}"),
                row.description,
            );
            insert_money(
                &mut fields,
                &format!("txtAmountPurchase{suffix}"),
                row.purchase_or_import_amount,
            );
            insert_money(&mut fields, &format!("txtInputTax{suffix}"), row.input_tax);
            insert_life(
                &mut fields,
                &format!("txtEstimatedLife{suffix}"),
                row.estimated_life_months,
            );
            insert_life(
                &mut fields,
                &format!("txtRecognizedLife{suffix}"),
                row.recognized_life_months,
            );
            insert_money(
                &mut fields,
                &format!("txtAllowedInputTax{suffix}"),
                row.allowable_input_tax_for_period,
            );
            insert_money(
                &mut fields,
                &format!("txtBalanceInputTax{suffix}"),
                row.balance_to_next_period,
            );
        }
        insert_money(
            &mut fields,
            "sched1TotalBalPrev",
            self.schedule_1_previous_total(),
        );
        insert_money(
            &mut fields,
            "sched1TotalBalNext",
            self.schedule_1_next_total(),
        );

        let s2 = &self.schedule_2;
        for (key, value) in [
            (
                "frm2550qv2024:sched2InputTaxDirect",
                s2.input_tax_directly_attributable_to_exempt_sales,
            ),
            ("frm2550qv2024:sched2VatExemptSale", s2.vat_exempt_sales),
            (
                "frm2550qv2024:sched2AmountInputTax",
                s2.input_tax_not_directly_attributable,
            ),
            ("frm2550qv2024:sched2TotalSales", s2.total_sales),
            ("frm2550qv2024:sched2TotalRatable", s2.ratable_input_tax),
            (
                "frm2550qv2024:sched2TotalAttr",
                s2.total_input_tax_attributable_to_exempt_sales,
            ),
        ] {
            insert_money(&mut fields, key, value);
        }

        for (offset, suffix) in SCHEDULE_3_SUFFIXES.into_iter().enumerate() {
            let row = self.schedule_3.get(offset).cloned().unwrap_or_default();
            insert_optional_date(
                &mut fields,
                &format!("txtDateCovered3{suffix}"),
                row.period_from,
            );
            insert_optional_date(
                &mut fields,
                &format!("txtDateCovered3To{suffix}"),
                row.period_to,
            );
            insert(
                &mut fields,
                &format!("txtNameWithHoldingAgent3{suffix}"),
                row.withholding_agent_name,
            );
            insert_money(
                &mut fields,
                &format!("txtIncomePayment3{suffix}"),
                row.income_payment,
            );
            insert_money(
                &mut fields,
                &format!("txtTotalTaxWithHeld3{suffix}"),
                row.tax_withheld,
            );
        }
        insert_money(
            &mut fields,
            "sched3TotalIncome",
            self.schedule_3_income_total(),
        );
        insert_money(&mut fields, "sched3TotalTax", self.schedule_3_tax_total());

        for (offset, suffix) in SCHEDULE_4_SUFFIXES.into_iter().enumerate() {
            let row = self.schedule_4.get(offset).cloned().unwrap_or_default();
            insert_optional_date(&mut fields, &format!("txtDate4{suffix}"), row.period_from);
            insert_optional_date(&mut fields, &format!("txtDate4To{suffix}"), row.period_to);
            insert(
                &mut fields,
                &format!("txtNameOfMiller4{suffix}"),
                row.miller_name,
            );
            insert(
                &mut fields,
                &format!("txtNameOfTaxpayer4{suffix}"),
                row.taxpayer_name,
            );
            insert(
                &mut fields,
                &format!("txtOfficialReceiptNumber4{suffix}"),
                if row.official_receipt_number.trim().is_empty() {
                    "0.00".to_string()
                } else {
                    row.official_receipt_number
                },
            );
            insert_money(
                &mut fields,
                &format!("txtAmountPaid4{suffix}"),
                row.amount_paid,
            );
        }
        insert_money(
            &mut fields,
            "sched4AmountPaid",
            self.schedule_4_amount_total(),
        );

        insert(&mut fields, "frm2550qv2024:txtCurrentPage", "2");
        insert(&mut fields, "frm2550qv2024:txtMaxPage", "2");
        for key in [
            "resultOtherCreditsNo19",
            "resultOtherCreditsNo42",
            "resultOtherCreditsNo47",
            "resultOtherCreditsNo56",
        ] {
            fields
                .entry(key.to_string())
                .or_insert_with(|| "0.00".to_string());
        }
        insert_money(
            &mut fields,
            "txtTotalAmountOfBalanceofInputTaxFromPrevious",
            self.schedule_1_previous_total(),
        );
        insert_money(
            &mut fields,
            "txtTotalAmountOfBalanceofInputTaxToBeCarried",
            self.schedule_1_next_total(),
        );
        insert_money(
            &mut fields,
            "txtTotalAmountofIncomePayment",
            self.schedule_3_income_total(),
        );
        insert_money(
            &mut fields,
            "txtTotalAmoungOfTaxWithHeld",
            self.schedule_3_tax_total(),
        );
        insert_money(
            &mut fields,
            "txtAmountPaidSched4",
            self.schedule_4_amount_total(),
        );

        insert(
            &mut fields,
            "txtFinalFlag",
            self.xml_final_flag.as_xml_value(),
        );
        insert(&mut fields, "txtEnroll", "Y");
        insert(&mut fields, "ebirOnlineConfirmUsername", "");
        insert(&mut fields, "ebirOnlineUsername", "");
        insert(&mut fields, "ebirOnlineSecret", "");
        insert(&mut fields, "txtEmail", self.xml_contact_email.clone());
        insert(&mut fields, "driveSelectTPExport", "0");
        insert(
            &mut fields,
            "dateFiled",
            self.date_filed
                .map(Form2550QDate::to_filed_date)
                .unwrap_or_default(),
        );
        fields
    }

    pub fn to_bir_xml_payload(&self) -> String {
        crate::bir_xml::generate_bir_xml(&self.to_bir_field_map())
    }

    /// Produce an editable-save payload only after semantic and evidence gates
    /// pass. This is not a queue/submission API.
    pub fn try_to_bir_xml_payload(&self) -> Result<String, Vec<(String, String)>> {
        let mut normalized = self.clone();
        normalized.recompute();
        let errors = normalized.validate();
        if errors.is_empty() {
            Ok(crate::bir_xml::generate_bir_xml(
                &normalized.to_reviewed_field_map(),
            ))
        } else {
            Err(errors)
        }
    }

    pub fn from_bir_xml_payload(xml: &str) -> Result<Self, Vec<(String, String)>> {
        let mut fields = crate::bir_xml::parse_bir_xml_checked(xml).map_err(|error| {
            vec![(
                "xml_payload".to_string(),
                format!("Invalid 2550Q pseudo-XML: {error}"),
            )]
        })?;
        if !fields.contains_key("dateFiled")
            && let Some(value) = standalone_element(xml, "dateFiled")
        {
            fields.insert("dateFiled".to_string(), value);
        }
        Self::from_bir_field_map(&fields)
    }

    pub fn from_bir_field_map(
        fields: &BTreeMap<String, String>,
    ) -> Result<Self, Vec<(String, String)>> {
        let mut errors = Vec::new();
        let expected = expected_xml_keys();
        for key in &expected {
            if !fields.contains_key(key) {
                errors.push((
                    key.clone(),
                    format!("Required 2550Q source field {key} is missing"),
                ));
            }
        }

        require_exact_value(fields, "txtEnroll", "Y", &mut errors);
        require_exact_value(fields, "driveSelectTPExport", "0", &mut errors);
        require_exact_value(fields, "frm2550qv2024:txtCurrentPage", "2", &mut errors);
        require_exact_value(fields, "frm2550qv2024:txtMaxPage", "2", &mut errors);
        for key in [
            "ebirOnlineConfirmUsername",
            "ebirOnlineUsername",
            "ebirOnlineSecret",
        ] {
            require_exact_value(fields, key, "", &mut errors);
        }

        let filing_basis = parse_bool_pair(
            fields,
            "frm2550qv2024:calendarNo1",
            "frm2550qv2024:fiscalNo1",
            "filing_basis",
            &mut errors,
        )
        .map(|calendar| {
            if calendar {
                Form2550QFilingBasis::Calendar
            } else {
                Form2550QFilingBasis::Fiscal
            }
        });
        let quarter_index = parse_one_of(
            fields,
            &[
                "frm2550qv2024:OptQuarter1",
                "frm2550qv2024:OptQuarter2",
                "frm2550qv2024:OptQuarter3",
                "frm2550qv2024:OptQuarter4",
            ],
            "quarter",
            &mut errors,
        );
        let is_amended = parse_bool_pair(
            fields,
            "frm2550qv2024:amendedReturnYesNo5",
            "frm2550qv2024:amendedReturnNo5",
            "is_amended",
            &mut errors,
        );
        let is_short_period_return = parse_bool_pair(
            fields,
            "frm2550qv2024:OptShortPrd1",
            "frm2550qv2024:OptShortPrd2",
            "is_short_period_return",
            &mut errors,
        );
        let classification_index = parse_one_of(
            fields,
            &[
                "frm2550qv2024:taxPayerClassification1",
                "frm2550qv2024:taxPayerClassification2",
                "frm2550qv2024:taxPayerClassification3",
                "frm2550qv2024:taxPayerClassification4",
            ],
            "taxpayer_classification",
            &mut errors,
        );
        let is_availing_tax_relief = parse_bool_pair(
            fields,
            "frm2550qv2024:internationalTreatyYn",
            "frm2550qv2024:specialRateYn",
            "is_availing_tax_relief",
            &mut errors,
        );

        let tin = format!(
            "{}{}{}{}",
            field(fields, "frm2550qv2024:txtTIN1"),
            field(fields, "frm2550qv2024:txtTIN2"),
            field(fields, "frm2550qv2024:txtTIN3"),
            field(fields, "frm2550qv2024:branchCode")
        );
        verify_duplicate(
            fields,
            "frm2550qv2024:txtTIN1",
            "frm2550qv2024:txtPg2TIN1",
            &mut errors,
        );
        verify_duplicate(
            fields,
            "frm2550qv2024:txtTIN2",
            "frm2550qv2024:txtPg2TIN2",
            &mut errors,
        );
        verify_duplicate(
            fields,
            "frm2550qv2024:txtTIN3",
            "frm2550qv2024:txtPg2TIN3",
            &mut errors,
        );
        verify_duplicate(
            fields,
            "frm2550qv2024:branchCode",
            "frm2550qv2024:txtPg2BranchCode",
            &mut errors,
        );
        verify_duplicate(
            fields,
            "frm2550qv2024:taxpayerName",
            "frm2550qv2024:Pg2TaxPayer",
            &mut errors,
        );

        let schedule_1 = SCHEDULE_ROW_SUFFIXES
            .into_iter()
            .map(|suffix| parse_capital_good_row(fields, suffix, &mut errors))
            .collect::<Vec<_>>();
        let schedule_3 = SCHEDULE_3_SUFFIXES
            .into_iter()
            .map(|suffix| parse_creditable_vat_row(fields, suffix, &mut errors))
            .collect::<Vec<_>>();
        let schedule_4 = SCHEDULE_4_SUFFIXES
            .into_iter()
            .map(|suffix| parse_advance_vat_row(fields, suffix, &mut errors))
            .collect::<Vec<_>>();

        let xml_final_flag = match field(fields, "txtFinalFlag") {
            "0" => Form2550QXmlFinalFlag::Zero,
            "1" => Form2550QXmlFinalFlag::One,
            "" => Form2550QXmlFinalFlag::Missing,
            value => Form2550QXmlFinalFlag::Unknown(value.to_string()),
        };

        if !errors.is_empty() {
            return Err(errors);
        }

        let now = chrono::Utc::now().to_rfc3339();
        let mut draft = Form2550QDraft {
            id: None,
            tin,
            taxable_year: parse_required(fields, "frm2550qv2024:txtYearNo2", &mut errors)
                .unwrap_or_default(),
            filing_basis: filing_basis.unwrap_or_default(),
            year_end_month: parse_required(fields, "frm2550qv2024:selectedMonthNo2", &mut errors)
                .unwrap_or_default(),
            quarter: Form2550QQuarter::from_number(
                u8::try_from(quarter_index.unwrap_or_default()).unwrap_or_default(),
            ),
            return_period_from: parse_optional_date(
                fields,
                "frm2550qv2024:RtnPeriodFromNo4",
                DateFormat::ReturnPeriod,
                &mut errors,
            ),
            return_period_to: parse_optional_date(
                fields,
                "frm2550qv2024:RtnPeriodToNo4",
                DateFormat::ReturnPeriod,
                &mut errors,
            ),
            is_amended: is_amended.unwrap_or(false),
            is_short_period_return: is_short_period_return.unwrap_or(false),
            rdo_code: semantic_text(fields, "frm2550qv2024:txtRDOCode"),
            taxpayer_name: semantic_text(fields, "frm2550qv2024:taxpayerName"),
            registered_address: semantic_text(fields, "frm2550qv2024:taxpayerAddress"),
            zip_code: semantic_text(fields, "frm2550qv2024:taxpayerZip"),
            contact_number: semantic_text(fields, "frm2550qv2024:taxpayerContactNumber"),
            email: semantic_text(fields, "frm2550qv2024:taxpayerEmailAddress"),
            taxpayer_classification: classification_index.and_then(classification_from_index),
            is_availing_tax_relief: is_availing_tax_relief.unwrap_or(false),
            tax_relief_details: semantic_text(fields, "frm2550qv2024:specifyInternationalTreaty"),
            part_ii: Form2550QPartII {
                item_18_paid_on_previous_return: parse_money(
                    fields,
                    "frm2550qv2024:vatPaidReturn",
                    &mut errors,
                ),
                item_19_description: semantic_text(fields, "frm2550qv2024:addSpecifyNo19"),
                item_19_other_credit_or_payment: parse_money(
                    fields,
                    "frm2550qv2024:otherCreditsNo19",
                    &mut errors,
                ),
                item_22_surcharge: parse_money(fields, "frm2550qv2024:surcharge", &mut errors),
                item_23_interest: parse_money(fields, "frm2550qv2024:interest", &mut errors),
                item_24_compromise: parse_money(fields, "frm2550qv2024:compromise", &mut errors),
                ..Form2550QPartII::default()
            },
            part_iv: Form2550QPartIV {
                item_31a_vatable_sales: parse_money(
                    fields,
                    "frm2550qv2024:vatableSales",
                    &mut errors,
                ),
                item_32a_zero_rated_sales: parse_money(
                    fields,
                    "frm2550qv2024:zeroRatedSales",
                    &mut errors,
                ),
                item_33a_exempt_sales: parse_money(
                    fields,
                    "frm2550qv2024:exemptSales",
                    &mut errors,
                ),
                item_35b_less_output_vat_uncollected: parse_money(
                    fields,
                    "frm2550qv2024:lessOutputVat",
                    &mut errors,
                ),
                item_36b_add_output_vat_recovered: parse_money(
                    fields,
                    "frm2550qv2024:addOutputVat",
                    &mut errors,
                ),
                item_38b_input_tax_carried: parse_money(
                    fields,
                    "frm2550qv2024:inputTaxCarried",
                    &mut errors,
                ),
                item_40b_transitional_input_tax: parse_money(
                    fields,
                    "frm2550qv2024:transitionalInputTax",
                    &mut errors,
                ),
                item_41b_presumptive_input_tax: parse_money(
                    fields,
                    "frm2550qv2024:presumptiveInputTax",
                    &mut errors,
                ),
                item_42_description: semantic_text(fields, "frm2550qv2024:addSpecifyNo42"),
                item_42b_other_input_tax: parse_money(
                    fields,
                    "frm2550qv2024:otherSpecify42",
                    &mut errors,
                ),
                item_44a_domestic_purchases: parse_money(
                    fields,
                    "frm2550qv2024:domesticPurchase",
                    &mut errors,
                ),
                item_44b_domestic_input_tax: parse_money(
                    fields,
                    "frm2550qv2024:domesticInputTax",
                    &mut errors,
                ),
                item_45a_nonresident_services: parse_money(
                    fields,
                    "frm2550qv2024:servicesPurchase",
                    &mut errors,
                ),
                item_45b_nonresident_service_input_tax: parse_money(
                    fields,
                    "frm2550qv2024:serviceInputTax",
                    &mut errors,
                ),
                item_46a_importations: parse_money(
                    fields,
                    "frm2550qv2024:importPurchase",
                    &mut errors,
                ),
                item_46b_import_input_tax: parse_money(
                    fields,
                    "frm2550qv2024:importInputTax",
                    &mut errors,
                ),
                item_47_description: semantic_text(fields, "frm2550qv2024:addSpecifyNo47"),
                item_47a_other_purchases: parse_money(
                    fields,
                    "frm2550qv2024:otherSpecify47",
                    &mut errors,
                ),
                item_47b_other_input_tax: parse_money(fields, "otherSpecify47B", &mut errors),
                item_48a_domestic_purchases_no_input_tax: parse_money(
                    fields,
                    "frm2550qv2024:domesticPurchaseNoTax",
                    &mut errors,
                ),
                item_49a_vat_exempt_importations: parse_money(
                    fields,
                    "frm2550qv2024:vatExemptImports",
                    &mut errors,
                ),
                item_54b_vat_refund_or_tcc_claimed: parse_money(
                    fields,
                    "frm2550qv2024:vatRefund",
                    &mut errors,
                ),
                item_55b_input_vat_on_unpaid_payables: parse_money(
                    fields,
                    "frm2550qv2024:inputVatUnpaid",
                    &mut errors,
                ),
                item_56_description: semantic_text(fields, "frm2550qv2024:addSpecifyNo56"),
                item_56b_other_deduction: parse_money(
                    fields,
                    "frm2550qv2024:otherSpecify56",
                    &mut errors,
                ),
                item_58b_input_vat_on_settled_payables: parse_money(
                    fields,
                    "frm2550qv2024:addInputVat",
                    &mut errors,
                ),
                ..Form2550QPartIV::default()
            },
            schedule_1,
            schedule_2: Form2550QSchedule2 {
                input_tax_directly_attributable_to_exempt_sales: parse_money(
                    fields,
                    "frm2550qv2024:sched2InputTaxDirect",
                    &mut errors,
                ),
                vat_exempt_sales: parse_money(
                    fields,
                    "frm2550qv2024:sched2VatExemptSale",
                    &mut errors,
                ),
                input_tax_not_directly_attributable: parse_money(
                    fields,
                    "frm2550qv2024:sched2AmountInputTax",
                    &mut errors,
                ),
                ..Form2550QSchedule2::default()
            },
            schedule_3,
            schedule_4,
            local_print_fields: Form2550QLocalPrintFields::default(),
            xml_final_flag,
            xml_contact_email: semantic_text(fields, "txtEmail"),
            date_filed: parse_optional_date(fields, "dateFiled", DateFormat::Filed, &mut errors),
            preserved_unmodeled_xml_fields: fields
                .iter()
                .filter(|(key, _)| is_preserved_unmodeled_key(key, &expected))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            migration_review_items: Vec::new(),
            legacy_flat_draft_fields: BTreeMap::new(),
            status: super::FilingStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
            submitted_at: None,
            confirmed_at: None,
            submission_filename: None,
            receipt_id: None,
            submission_attempts: 0,
            next_retry_at: None,
            last_error: None,
        };

        if !errors.is_empty() {
            return Err(errors);
        }

        let source_computed = computed_source_values(fields, &mut errors);
        draft.recompute();
        for (key, actual) in computed_draft_values(&draft) {
            verify_computed_source(
                key,
                source_computed.get(key).copied().flatten(),
                actual,
                &mut errors,
            );
        }
        verify_computed_source(
            "txtTotalAmountOfBalanceofInputTaxFromPrevious",
            parse_money(
                fields,
                "txtTotalAmountOfBalanceofInputTaxFromPrevious",
                &mut errors,
            ),
            draft.schedule_1_previous_total(),
            &mut errors,
        );
        verify_computed_source(
            "txtTotalAmountOfBalanceofInputTaxToBeCarried",
            parse_money(
                fields,
                "txtTotalAmountOfBalanceofInputTaxToBeCarried",
                &mut errors,
            ),
            draft.schedule_1_next_total(),
            &mut errors,
        );
        verify_computed_source(
            "txtTotalAmountofIncomePayment",
            parse_money(fields, "txtTotalAmountofIncomePayment", &mut errors),
            draft.schedule_3_income_total(),
            &mut errors,
        );
        verify_computed_source(
            "txtTotalAmoungOfTaxWithHeld",
            parse_money(fields, "txtTotalAmoungOfTaxWithHeld", &mut errors),
            draft.schedule_3_tax_total(),
            &mut errors,
        );
        verify_computed_source(
            "txtAmountPaidSched4",
            parse_money(fields, "txtAmountPaidSched4", &mut errors),
            draft.schedule_4_amount_total(),
            &mut errors,
        );

        errors.extend(draft.validate());
        if errors.is_empty() {
            Ok(draft)
        } else {
            Err(errors)
        }
    }
}

fn computed_source_values(
    fields: &BTreeMap<String, String>,
    errors: &mut Vec<(String, String)>,
) -> BTreeMap<&'static str, Option<f64>> {
    COMPUTED_XML_KEYS
        .iter()
        .map(|key| (*key, parse_money(fields, key, errors)))
        .collect()
}

fn computed_draft_values(draft: &Form2550QDraft) -> Vec<(&'static str, Option<f64>)> {
    let p2 = &draft.part_ii;
    let p4 = &draft.part_iv;
    let s2 = &draft.schedule_2;
    vec![
        (
            "frm2550qv2024:excessInputTax",
            p2.item_15_net_vat_payable_or_excess,
        ),
        (
            "frm2550qv2024:creditableVat",
            p2.item_16_creditable_vat_withheld,
        ),
        (
            "frm2550qv2024:advVatPayment",
            p2.item_17_advance_vat_payments,
        ),
        (
            "frm2550qv2024:totalTaxCredits",
            p2.item_20_total_credits_or_payments,
        ),
        (
            "frm2550qv2024:excessCredits",
            p2.item_21_tax_payable_or_excess_credits,
        ),
        ("frm2550qv2024:penalties", p2.item_25_total_penalties),
        (
            "frm2550qv2024:totalPayable",
            p2.item_26_total_amount_payable_or_excess,
        ),
        ("frm2550qv2024:outputVatSales", p4.item_31b_output_tax),
        ("frm2550qv2024:totalSales", p4.item_34a_total_sales),
        ("frm2550qv2024:outputTaxDue", p4.item_34b_output_tax_due),
        (
            "frm2550qv2024:totalAdjOutput",
            p4.item_37b_adjusted_output_tax_due,
        ),
        (
            "frm2550qv2024:inputTaxDeferred",
            p4.item_39b_input_tax_deferred,
        ),
        ("frm2550qv2024:total43", p4.item_43b_total_prior_input_tax),
        (
            "frm2550qv2024:totalCurPurchase",
            p4.item_50a_total_current_purchases,
        ),
        (
            "frm2550qv2024:totalCurInputTax",
            p4.item_50b_total_current_input_tax,
        ),
        (
            "frm2550qv2024:totalAvailInputTax",
            p4.item_51b_total_available_input_tax,
        ),
        (
            "frm2550qv2024:importCapitalInputTax",
            p4.item_52b_deferred_capital_goods_input_tax,
        ),
        (
            "frm2550qv2024:inputTaxAttr",
            p4.item_53b_input_tax_attributable_to_exempt_sales,
        ),
        (
            "frm2550qv2024:totalDeductions",
            p4.item_57b_total_deductions,
        ),
        (
            "frm2550qv2024:adjDeductions",
            p4.item_59b_adjusted_deductions,
        ),
        (
            "frm2550qv2024:totalAllowInputTax",
            p4.item_60b_total_allowable_input_tax,
        ),
        (
            "frm2550qv2024:netVatPayable",
            p4.item_61b_net_vat_payable_or_excess,
        ),
        ("sched1TotalBalPrev", draft.schedule_1_previous_total()),
        ("sched1TotalBalNext", draft.schedule_1_next_total()),
        ("frm2550qv2024:sched2TotalSales", s2.total_sales),
        ("frm2550qv2024:sched2TotalRatable", s2.ratable_input_tax),
        (
            "frm2550qv2024:sched2TotalAttr",
            s2.total_input_tax_attributable_to_exempt_sales,
        ),
        ("sched3TotalIncome", draft.schedule_3_income_total()),
        ("sched3TotalTax", draft.schedule_3_tax_total()),
        ("sched4AmountPaid", draft.schedule_4_amount_total()),
    ]
}

const COMPUTED_XML_KEYS: [&str; 30] = [
    "frm2550qv2024:excessInputTax",
    "frm2550qv2024:creditableVat",
    "frm2550qv2024:advVatPayment",
    "frm2550qv2024:totalTaxCredits",
    "frm2550qv2024:excessCredits",
    "frm2550qv2024:penalties",
    "frm2550qv2024:totalPayable",
    "frm2550qv2024:outputVatSales",
    "frm2550qv2024:totalSales",
    "frm2550qv2024:outputTaxDue",
    "frm2550qv2024:totalAdjOutput",
    "frm2550qv2024:inputTaxDeferred",
    "frm2550qv2024:total43",
    "frm2550qv2024:totalCurPurchase",
    "frm2550qv2024:totalCurInputTax",
    "frm2550qv2024:totalAvailInputTax",
    "frm2550qv2024:importCapitalInputTax",
    "frm2550qv2024:inputTaxAttr",
    "frm2550qv2024:totalDeductions",
    "frm2550qv2024:adjDeductions",
    "frm2550qv2024:totalAllowInputTax",
    "frm2550qv2024:netVatPayable",
    "sched1TotalBalPrev",
    "sched1TotalBalNext",
    "frm2550qv2024:sched2TotalSales",
    "frm2550qv2024:sched2TotalRatable",
    "frm2550qv2024:sched2TotalAttr",
    "sched3TotalIncome",
    "sched3TotalTax",
    "sched4AmountPaid",
];

fn parse_capital_good_row(
    fields: &BTreeMap<String, String>,
    suffix: u8,
    errors: &mut Vec<(String, String)>,
) -> Form2550QCapitalGoodRow {
    Form2550QCapitalGoodRow {
        purchase_or_import_date: parse_optional_date(
            fields,
            &format!("txtDatePurchase{suffix}"),
            DateFormat::ReturnPeriod,
            errors,
        ),
        source_code: semantic_text(fields, &format!("txtSourceCode{suffix}")),
        description: semantic_text(fields, &format!("txtDescription{suffix}")),
        purchase_or_import_amount: parse_money(
            fields,
            &format!("txtAmountPurchase{suffix}"),
            errors,
        ),
        input_tax: parse_money(fields, &format!("txtInputTax{suffix}"), errors),
        estimated_life_months: parse_life(fields, &format!("txtEstimatedLife{suffix}"), errors),
        recognized_life_months: parse_life(fields, &format!("txtRecognizedLife{suffix}"), errors),
        allowable_input_tax_for_period: parse_money(
            fields,
            &format!("txtAllowedInputTax{suffix}"),
            errors,
        ),
        balance_to_next_period: parse_money(fields, &format!("txtBalanceInputTax{suffix}"), errors),
    }
}

fn parse_creditable_vat_row(
    fields: &BTreeMap<String, String>,
    suffix: u8,
    errors: &mut Vec<(String, String)>,
) -> Form2550QCreditableVatRow {
    Form2550QCreditableVatRow {
        period_from: parse_optional_date(
            fields,
            &format!("txtDateCovered3{suffix}"),
            DateFormat::ReturnPeriod,
            errors,
        ),
        period_to: parse_optional_date(
            fields,
            &format!("txtDateCovered3To{suffix}"),
            DateFormat::ReturnPeriod,
            errors,
        ),
        withholding_agent_name: semantic_text(fields, &format!("txtNameWithHoldingAgent3{suffix}")),
        income_payment: parse_money(fields, &format!("txtIncomePayment3{suffix}"), errors),
        tax_withheld: parse_money(fields, &format!("txtTotalTaxWithHeld3{suffix}"), errors),
    }
}

fn parse_advance_vat_row(
    fields: &BTreeMap<String, String>,
    suffix: u8,
    errors: &mut Vec<(String, String)>,
) -> Form2550QAdvanceVatRow {
    Form2550QAdvanceVatRow {
        period_from: parse_optional_date(
            fields,
            &format!("txtDate4{suffix}"),
            DateFormat::ReturnPeriod,
            errors,
        ),
        period_to: parse_optional_date(
            fields,
            &format!("txtDate4To{suffix}"),
            DateFormat::ReturnPeriod,
            errors,
        ),
        miller_name: semantic_text(fields, &format!("txtNameOfMiller4{suffix}")),
        taxpayer_name: semantic_text(fields, &format!("txtNameOfTaxpayer4{suffix}")),
        official_receipt_number: semantic_text(
            fields,
            &format!("txtOfficialReceiptNumber4{suffix}"),
        ),
        amount_paid: parse_money(fields, &format!("txtAmountPaid4{suffix}"), errors),
    }
}

fn expected_xml_keys() -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for key in [
        "frm2550qv2024:calendarNo1",
        "frm2550qv2024:fiscalNo1",
        "frm2550qv2024:selectedMonthNo2",
        "frm2550qv2024:txtYearNo2",
        "frm2550qv2024:OptQuarter1",
        "frm2550qv2024:OptQuarter2",
        "frm2550qv2024:OptQuarter3",
        "frm2550qv2024:OptQuarter4",
        "frm2550qv2024:RtnPeriodFromNo4",
        "frm2550qv2024:RtnPeriodToNo4",
        "frm2550qv2024:amendedReturnYesNo5",
        "frm2550qv2024:amendedReturnNo5",
        "frm2550qv2024:OptShortPrd1",
        "frm2550qv2024:OptShortPrd2",
        "frm2550qv2024:txtTIN1",
        "frm2550qv2024:txtTIN2",
        "frm2550qv2024:txtTIN3",
        "frm2550qv2024:branchCode",
        "frm2550qv2024:txtRDOCode",
        "frm2550qv2024:taxpayerName",
        "frm2550qv2024:taxpayerAddress",
        "frm2550qv2024:taxpayerZip",
        "frm2550qv2024:taxpayerContactNumber",
        "frm2550qv2024:taxpayerEmailAddress",
        "frm2550qv2024:taxPayerClassification1",
        "frm2550qv2024:taxPayerClassification2",
        "frm2550qv2024:taxPayerClassification3",
        "frm2550qv2024:taxPayerClassification4",
        "frm2550qv2024:internationalTreatyYn",
        "frm2550qv2024:specialRateYn",
        "frm2550qv2024:specifyInternationalTreaty",
        "frm2550qv2024:excessInputTax",
        "frm2550qv2024:creditableVat",
        "frm2550qv2024:advVatPayment",
        "frm2550qv2024:vatPaidReturn",
        "frm2550qv2024:addSpecifyNo19",
        "frm2550qv2024:otherCreditsNo19",
        "frm2550qv2024:totalTaxCredits",
        "frm2550qv2024:excessCredits",
        "frm2550qv2024:surcharge",
        "frm2550qv2024:interest",
        "frm2550qv2024:compromise",
        "frm2550qv2024:penalties",
        "frm2550qv2024:totalPayable",
        "frm2550qv2024:txtPg2TIN1",
        "frm2550qv2024:txtPg2TIN2",
        "frm2550qv2024:txtPg2TIN3",
        "frm2550qv2024:txtPg2BranchCode",
        "frm2550qv2024:Pg2TaxPayer",
        "frm2550qv2024:vatableSales",
        "frm2550qv2024:outputVatSales",
        "frm2550qv2024:zeroRatedSales",
        "frm2550qv2024:exemptSales",
        "frm2550qv2024:totalSales",
        "frm2550qv2024:outputTaxDue",
        "frm2550qv2024:lessOutputVat",
        "frm2550qv2024:addOutputVat",
        "frm2550qv2024:totalAdjOutput",
        "frm2550qv2024:inputTaxCarried",
        "frm2550qv2024:inputTaxDeferred",
        "frm2550qv2024:transitionalInputTax",
        "frm2550qv2024:presumptiveInputTax",
        "frm2550qv2024:addSpecifyNo42",
        "frm2550qv2024:otherSpecify42",
        "frm2550qv2024:total43",
        "frm2550qv2024:domesticPurchase",
        "frm2550qv2024:domesticInputTax",
        "frm2550qv2024:servicesPurchase",
        "frm2550qv2024:serviceInputTax",
        "frm2550qv2024:importPurchase",
        "frm2550qv2024:importInputTax",
        "frm2550qv2024:addSpecifyNo47",
        "frm2550qv2024:otherSpecify47",
        "otherSpecify47B",
        "frm2550qv2024:domesticPurchaseNoTax",
        "frm2550qv2024:vatExemptImports",
        "frm2550qv2024:totalCurPurchase",
        "frm2550qv2024:totalCurInputTax",
        "frm2550qv2024:totalAvailInputTax",
        "frm2550qv2024:importCapitalInputTax",
        "frm2550qv2024:inputTaxAttr",
        "frm2550qv2024:vatRefund",
        "frm2550qv2024:inputVatUnpaid",
        "frm2550qv2024:addSpecifyNo56",
        "frm2550qv2024:otherSpecify56",
        "frm2550qv2024:totalDeductions",
        "frm2550qv2024:addInputVat",
        "frm2550qv2024:adjDeductions",
        "frm2550qv2024:totalAllowInputTax",
        "frm2550qv2024:netVatPayable",
        "sched1TotalBalPrev",
        "sched1TotalBalNext",
        "frm2550qv2024:sched2InputTaxDirect",
        "frm2550qv2024:sched2VatExemptSale",
        "frm2550qv2024:sched2AmountInputTax",
        "frm2550qv2024:sched2TotalSales",
        "frm2550qv2024:sched2TotalRatable",
        "frm2550qv2024:sched2TotalAttr",
        "sched3TotalIncome",
        "sched3TotalTax",
        "sched4AmountPaid",
        "frm2550qv2024:txtCurrentPage",
        "frm2550qv2024:txtMaxPage",
        "resultOtherCreditsNo19",
        "resultOtherCreditsNo42",
        "resultOtherCreditsNo47",
        "resultOtherCreditsNo56",
        "txtTotalAmountOfBalanceofInputTaxFromPrevious",
        "txtTotalAmountOfBalanceofInputTaxToBeCarried",
        "txtTotalAmountofIncomePayment",
        "txtTotalAmoungOfTaxWithHeld",
        "txtAmountPaidSched4",
        "txtFinalFlag",
        "txtEnroll",
        "ebirOnlineConfirmUsername",
        "ebirOnlineUsername",
        "ebirOnlineSecret",
        "txtEmail",
        "driveSelectTPExport",
        "dateFiled",
    ] {
        keys.insert(key.to_string());
    }
    for suffix in SCHEDULE_ROW_SUFFIXES {
        for prefix in [
            "txtDatePurchase",
            "txtSourceCode",
            "txtDescription",
            "txtAmountPurchase",
            "txtInputTax",
            "txtEstimatedLife",
            "txtRecognizedLife",
            "txtAllowedInputTax",
            "txtBalanceInputTax",
        ] {
            keys.insert(format!("{prefix}{suffix}"));
        }
    }
    for suffix in SCHEDULE_3_SUFFIXES {
        for prefix in [
            "txtDateCovered3",
            "txtDateCovered3To",
            "txtNameWithHoldingAgent3",
            "txtIncomePayment3",
            "txtTotalTaxWithHeld3",
        ] {
            keys.insert(format!("{prefix}{suffix}"));
        }
    }
    for suffix in SCHEDULE_4_SUFFIXES {
        for prefix in [
            "txtDate4",
            "txtDate4To",
            "txtNameOfMiller4",
            "txtNameOfTaxpayer4",
            "txtOfficialReceiptNumber4",
            "txtAmountPaid4",
        ] {
            keys.insert(format!("{prefix}{suffix}"));
        }
    }
    keys
}

fn is_preserved_unmodeled_key(key: &str, expected: &BTreeSet<String>) -> bool {
    !expected.contains(key)
        || matches!(
            key,
            "resultOtherCreditsNo19"
                | "resultOtherCreditsNo42"
                | "resultOtherCreditsNo47"
                | "resultOtherCreditsNo56"
        )
}

fn standalone_element(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)?.checked_add(open.len())?;
    let end = xml[start..].find(&close)?.checked_add(start)?;
    Some(xml[start..end].trim().to_string())
}

fn semantic_text(fields: &BTreeMap<String, String>, key: &str) -> String {
    let mut value = field(fields, key).to_string();
    for _ in 0..2 {
        let decoded = urlencoding::decode(&value)
            .unwrap_or(std::borrow::Cow::Borrowed(&value))
            .into_owned();
        if decoded == value {
            break;
        }
        value = decoded;
    }
    value
}

fn field<'a>(fields: &'a BTreeMap<String, String>, key: &str) -> &'a str {
    fields.get(key).map(String::as_str).unwrap_or("")
}

fn parse_required<T: std::str::FromStr>(
    fields: &BTreeMap<String, String>,
    key: &str,
    errors: &mut Vec<(String, String)>,
) -> Option<T> {
    let value = field(fields, key).trim();
    if value.is_empty() {
        errors.push((
            key.to_string(),
            format!("Required 2550Q field {key} is blank"),
        ));
        return None;
    }
    value
        .parse()
        .map_err(|_| {
            errors.push((
                key.to_string(),
                format!("2550Q field {key} has an invalid value"),
            ));
        })
        .ok()
}

fn parse_bool(
    fields: &BTreeMap<String, String>,
    key: &str,
    errors: &mut Vec<(String, String)>,
) -> Option<bool> {
    match field(fields, key).trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => {
            errors.push((
                key.to_string(),
                format!("2550Q field {key} must be true or false"),
            ));
            None
        }
    }
}

fn parse_bool_pair(
    fields: &BTreeMap<String, String>,
    first_key: &str,
    second_key: &str,
    label: &str,
    errors: &mut Vec<(String, String)>,
) -> Option<bool> {
    let first = parse_bool(fields, first_key, errors)?;
    let second = parse_bool(fields, second_key, errors)?;
    if first == second {
        errors.push((
            label.to_string(),
            format!("{first_key} and {second_key} must contain exactly one true value"),
        ));
        None
    } else {
        Some(first)
    }
}

fn parse_one_of(
    fields: &BTreeMap<String, String>,
    keys: &[&str],
    label: &str,
    errors: &mut Vec<(String, String)>,
) -> Option<usize> {
    let selected = keys
        .iter()
        .enumerate()
        .filter_map(|(index, key)| {
            parse_bool(fields, key, errors)
                .filter(|value| *value)
                .map(|_| index + 1)
        })
        .collect::<Vec<_>>();
    if selected.len() == 1 {
        selected.first().copied()
    } else {
        errors.push((
            label.to_string(),
            format!("{label} must contain exactly one selected XML option"),
        ));
        None
    }
}

fn parse_money(
    fields: &BTreeMap<String, String>,
    key: &str,
    errors: &mut Vec<(String, String)>,
) -> Option<f64> {
    let value = field(fields, key).trim();
    if value.is_empty() {
        return None;
    }
    let normalized = value.replace(',', "");
    match normalized.parse::<f64>() {
        Ok(amount) if amount.is_finite() => Some(amount),
        _ => {
            errors.push((
                key.to_string(),
                format!("2550Q field {key} is not a finite amount"),
            ));
            None
        }
    }
}

fn parse_life(
    fields: &BTreeMap<String, String>,
    key: &str,
    errors: &mut Vec<(String, String)>,
) -> Option<u16> {
    let value = parse_money(fields, key, errors)?;
    if value < 0.0 || value.fract() != 0.0 || value > f64::from(u16::MAX) {
        errors.push((
            key.to_string(),
            "Life in months must be a whole non-negative number".to_string(),
        ));
        None
    } else {
        Some(value as u16)
    }
}

#[derive(Clone, Copy)]
enum DateFormat {
    ReturnPeriod,
    Filed,
}

fn parse_optional_date(
    fields: &BTreeMap<String, String>,
    key: &str,
    format: DateFormat,
    errors: &mut Vec<(String, String)>,
) -> Option<Form2550QDate> {
    let value = field(fields, key).trim();
    if value.is_empty() {
        return None;
    }
    let parsed = match format {
        DateFormat::ReturnPeriod => Form2550QDate::parse_return_period(value),
        DateFormat::Filed => Form2550QDate::parse_filed_date(value),
    };
    parsed
        .map_err(|message| errors.push((key.to_string(), message)))
        .ok()
}

fn classification_from_index(index: usize) -> Option<Form2550QTaxpayerClassification> {
    match index {
        1 => Some(Form2550QTaxpayerClassification::Micro),
        2 => Some(Form2550QTaxpayerClassification::Small),
        3 => Some(Form2550QTaxpayerClassification::Medium),
        4 => Some(Form2550QTaxpayerClassification::Large),
        _ => None,
    }
}

fn require_exact_value(
    fields: &BTreeMap<String, String>,
    key: &str,
    expected: &str,
    errors: &mut Vec<(String, String)>,
) {
    if field(fields, key) != expected {
        errors.push((
            key.to_string(),
            format!("2550Q field {key} is outside reviewed value {expected:?}"),
        ));
    }
}

fn verify_duplicate(
    fields: &BTreeMap<String, String>,
    first: &str,
    second: &str,
    errors: &mut Vec<(String, String)>,
) {
    if semantic_text(fields, first) != semantic_text(fields, second) {
        errors.push((
            second.to_string(),
            format!("Duplicate fields {first} and {second} disagree"),
        ));
    }
}

fn verify_computed_source(
    key: &str,
    source: Option<f64>,
    computed: Option<f64>,
    errors: &mut Vec<(String, String)>,
) {
    match (source, computed) {
        (Some(source), Some(computed)) if (source - computed).abs() <= 0.005 => {}
        (None, None) => {}
        _ => errors.push((
            key.to_string(),
            format!("Source computed field {key} does not match the official formula"),
        )),
    }
}

fn split_tin(tin: &str) -> (String, String, String, String) {
    let digits: String = tin
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
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
        digits.get(9..).unwrap_or("").to_string(),
    )
}

fn insert(map: &mut BTreeMap<String, String>, key: &str, value: impl Into<String>) {
    map.insert(key.to_string(), value.into());
}

fn insert_bool(map: &mut BTreeMap<String, String>, key: &str, value: bool) {
    insert(map, key, if value { "true" } else { "false" });
}

fn insert_bool_pair(
    map: &mut BTreeMap<String, String>,
    true_key: &str,
    false_key: &str,
    value: bool,
) {
    insert_bool(map, true_key, value);
    insert_bool(map, false_key, !value);
}

fn insert_one_of_four(map: &mut BTreeMap<String, String>, prefix: &str, selected: Option<u8>) {
    for index in 1..=4 {
        insert_bool(map, &format!("{prefix}{index}"), selected == Some(index));
    }
}

fn insert_taxpayer_classification(
    map: &mut BTreeMap<String, String>,
    classification: Option<Form2550QTaxpayerClassification>,
) {
    for (index, candidate) in Form2550QTaxpayerClassification::ALL.into_iter().enumerate() {
        insert_bool(
            map,
            &format!("frm2550qv2024:taxPayerClassification{}", index + 1),
            classification == Some(candidate),
        );
    }
}

fn insert_optional_date(
    map: &mut BTreeMap<String, String>,
    key: &str,
    value: Option<Form2550QDate>,
) {
    insert(
        map,
        key,
        value.map(|date| date.to_string()).unwrap_or_default(),
    );
}

fn insert_money(map: &mut BTreeMap<String, String>, key: &str, value: Option<f64>) {
    insert(map, key, value.map(format_money).unwrap_or_default());
}

fn insert_life(map: &mut BTreeMap<String, String>, key: &str, value: Option<u16>) {
    insert(
        map,
        key,
        value
            .map(|months| format!("{:.2}", f64::from(months)))
            .unwrap_or_default(),
    );
}

fn format_money(value: f64) -> String {
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let raw = format!("{:.2}", value.abs());
    let (integer, fraction) = raw.split_once('.').unwrap_or((&raw, "00"));
    let mut grouped_reversed = String::with_capacity(integer.len() + integer.len() / 3);

    for (index, character) in integer.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped_reversed.push(',');
        }
        grouped_reversed.push(character);
    }

    let grouped = grouped_reversed.chars().rev().collect::<String>();
    format!("{sign}{grouped}.{fraction}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::TaxpayerProfile;
    use sha2::{Digest, Sha256};

    fn profile() -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "JUAN DELA CRUZ",
            "tin": {
                "segment1": "000",
                "segment2": "000",
                "segment3": "000",
                "branch": "00000"
            },
            "rdo_code": "018",
            "line_of_business": "SOFTWARE DEVELOPMENT",
            "registered_address": "OLONGAPO",
            "zip_code": "2200",
            "phone": "09123456789",
            "email": "CODEITLIKEMILEY@GMAIL.COM",
            "default_form_type": "2550Qv2024",
            "taxpayer_type": "Individual",
            "tax_classification": "SelfEmployed",
            "is_vat_registered": true,
            "eopt_tier": "Medium",
            "withholding_obligations": [],
            "excise_tax_liabilities": [],
            "atc_codes": [],
            "business_start_date": null,
            "birth_date": null,
            "registration_activity_status": "Active",
            "is_dormant_entity": false,
            "is_government_withholding_entity": false,
            "is_gpp_partner": false,
            "is_top_withholding_agent": false,
            "income_tax_elections": { "elections": {} },
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        }))
        .expect("valid profile")
    }

    fn reviewed_sample_draft() -> Form2550QDraft {
        let mut draft = Form2550QDraft::new_from_profile(&profile(), 2025, 1);
        draft.date_filed = Some(Form2550QDate::new(2026, 5, 8).expect("date"));
        draft.part_iv.item_31a_vatable_sales = Some(1_000.0);
        draft.part_iv.item_32a_zero_rated_sales = Some(1_000.0);
        draft.part_iv.item_33a_exempt_sales = Some(1_000.0);
        draft.part_iv.item_35b_less_output_vat_uncollected = Some(1_000.0);
        draft.part_iv.item_36b_add_output_vat_recovered = Some(1_000.0);
        draft.part_iv.item_38b_input_tax_carried = Some(1_000.0);
        draft.part_iv.item_40b_transitional_input_tax = Some(1_000.0);
        draft.part_iv.item_41b_presumptive_input_tax = Some(1_000.0);
        draft.part_iv.item_42_description = "EXAMPLE2".to_string();
        draft.part_iv.item_42b_other_input_tax = Some(10_000.0);
        draft.part_iv.item_44a_domestic_purchases = Some(1_000.0);
        draft.part_iv.item_44b_domestic_input_tax = Some(120.0);
        draft.part_iv.item_45a_nonresident_services = Some(1_000.0);
        draft.part_iv.item_45b_nonresident_service_input_tax = Some(120.0);
        draft.part_iv.item_46a_importations = Some(10_000.0);
        draft.part_iv.item_46b_import_input_tax = Some(1_200.0);
        draft.part_iv.item_47_description = "EXAMPLE 3".to_string();
        draft.part_iv.item_47a_other_purchases = Some(1_000.0);
        draft.part_iv.item_47b_other_input_tax = Some(120.0);
        draft.part_iv.item_48a_domestic_purchases_no_input_tax = Some(1_000.0);
        draft.part_iv.item_49a_vat_exempt_importations = Some(1_000.0);
        draft.part_iv.item_54b_vat_refund_or_tcc_claimed = Some(1_000.0);
        draft.part_iv.item_55b_input_vat_on_unpaid_payables = Some(1_000.0);
        draft.part_iv.item_56_description = "EXAMPLE 4".to_string();
        draft.part_iv.item_56b_other_deduction = Some(1_000.0);
        draft.part_iv.item_58b_input_vat_on_settled_payables = Some(10_000.0);
        draft.schedule_2.vat_exempt_sales = Some(1_000.0);
        draft.part_ii.item_19_description = "RXAMPLE".to_string();
        draft.part_ii.item_19_other_credit_or_payment = Some(1_000.0);
        draft.part_ii.item_22_surcharge = Some(1_000.0);
        draft.part_ii.item_23_interest = Some(100.0);
        draft.part_ii.item_24_compromise = Some(200.0);
        draft.recompute();
        draft
    }

    #[test]
    fn reviewed_contract_contains_exactly_160_keys() {
        let fields = reviewed_sample_draft().to_bir_field_map();
        assert_eq!(expected_xml_keys().len(), EXACT_SOURCE_FIELD_COUNT);
        assert_eq!(fields.len(), EXACT_SOURCE_FIELD_COUNT);
        assert_eq!(
            fields.keys().cloned().collect::<BTreeSet<_>>(),
            expected_xml_keys()
        );
    }

    #[test]
    fn reviewed_field_map_round_trips_without_losing_zero_or_negative_values() {
        let source = reviewed_sample_draft().to_bir_field_map();
        let imported = Form2550QDraft::from_bir_field_map(&source).expect("source imports");
        let output = imported.to_bir_field_map();
        assert_eq!(
            output.get("frm2550qv2024:netVatPayable"),
            Some(&"-1,440.00".to_string())
        );
        assert_eq!(output, source);
    }

    #[test]
    fn encrypted_final_flag_and_standalone_filed_date_are_accepted() {
        let mut fields = reviewed_sample_draft().to_bir_field_map();
        fields.insert("txtFinalFlag".to_string(), "0".to_string());
        let mut xml = crate::bir_xml::generate_bir_xml(&fields);
        let date_div = "<div id=\"dateFiled\">dateFiled=2026%2F05%2F08dateFiled=</div>\n";
        xml = xml.replace(date_div, "<dateFiled>2026/05/08</dateFiled>");
        let draft = Form2550QDraft::from_bir_xml_payload(&xml).expect("encrypted shape imports");
        assert_eq!(draft.xml_final_flag, Form2550QXmlFinalFlag::Zero);
        assert_eq!(
            draft.date_filed,
            Some(Form2550QDate::new(2026, 5, 8).expect("date"))
        );
    }

    #[test]
    fn contradictory_quarter_flags_fail_closed() {
        let mut fields = reviewed_sample_draft().to_bir_field_map();
        fields.insert("frm2550qv2024:OptQuarter2".to_string(), "true".to_string());
        let errors = Form2550QDraft::from_bir_field_map(&fields).expect_err("must reject");
        assert!(errors.iter().any(|(field, _)| field == "quarter"));
    }

    #[test]
    fn source_computed_mismatch_is_rejected() {
        let mut fields = reviewed_sample_draft().to_bir_field_map();
        fields.insert(
            "frm2550qv2024:totalCurPurchase".to_string(),
            "1.00".to_string(),
        );
        let errors = Form2550QDraft::from_bir_field_map(&fields).expect_err("must reject");
        assert!(
            errors
                .iter()
                .any(|(field, _)| field == "frm2550qv2024:totalCurPurchase")
        );
    }

    #[test]
    fn malformed_money_is_rejected_instead_of_becoming_zero() {
        let mut fields = reviewed_sample_draft().to_bir_field_map();
        fields.insert(
            "frm2550qv2024:vatableSales".to_string(),
            "not-money".to_string(),
        );
        let errors = Form2550QDraft::from_bir_field_map(&fields).expect_err("must reject");
        assert!(
            errors
                .iter()
                .any(|(field, _)| field == "frm2550qv2024:vatableSales")
        );
    }

    #[test]
    #[ignore = "requires EBIRFORMS_2550Q_SOURCE_DIR pointing to the reviewed external source pack"]
    fn locked_external_sources_match_hashes_and_roundtrip_all_160_fields() {
        let source_dir = std::env::var("EBIRFORMS_2550Q_SOURCE_DIR")
            .expect("set EBIRFORMS_2550Q_SOURCE_DIR to the exact reviewed 2550Qv2024 folder");
        let plain_path =
            std::path::Path::new(&source_dir).join("00000000000000-2550Qv2024-122025Q1.xml");
        let plain = std::fs::read(&plain_path).expect("plain source must be readable");
        assert_eq!(
            hex::encode(Sha256::digest(&plain)),
            super::super::form_2550q::REVIEWED_EDITABLE_XML_SHA256
        );
        let plain_xml = std::str::from_utf8(&plain).expect("plain source must be UTF-8");
        let plain_fields = crate::bir_xml::parse_bir_xml_checked(plain_xml)
            .expect("plain source must parse through the checked parser");
        assert_eq!(plain_fields.len(), EXACT_SOURCE_FIELD_COUNT);
        let plain_draft = Form2550QDraft::from_bir_field_map(&plain_fields)
            .expect("plain source must satisfy the semantic contract");
        assert_eq!(plain_draft.to_bir_field_map(), plain_fields);

        let encrypted_path = std::path::Path::new(&source_dir)
            .join("00000000000000-2550Qv2024-122025Q1#CODEITLIKEMILEY@GMAIL.COM#.xml");
        let encrypted = std::fs::read(&encrypted_path).expect("encrypted source must be readable");
        assert_eq!(
            hex::encode(Sha256::digest(&encrypted)),
            super::super::form_2550q::REVIEWED_ENCRYPTED_XML_SHA256
        );
        let decrypted =
            crate::crypto::decrypt_and_decompress(&encrypted, crate::crypto::BIR_IAF_PASSPHRASE)
                .expect("encrypted companion must decrypt with the reviewed reader");
        let decrypted_xml = std::str::from_utf8(&decrypted).expect("decrypted source is UTF-8");
        let encrypted_draft = Form2550QDraft::from_bir_xml_payload(decrypted_xml)
            .expect("decrypted source must satisfy the semantic contract");
        let encrypted_fields = encrypted_draft.to_bir_field_map();
        assert_eq!(encrypted_fields.len(), EXACT_SOURCE_FIELD_COUNT);
        assert_eq!(encrypted_fields["txtFinalFlag"], "0");

        for (filename, expected_hash) in [
            (
                "2550Q  April 2024 ENCS_Final.pdf",
                super::super::form_2550q::OFFICIAL_FORM_SHA256,
            ),
            (
                "2550Q guidelines April 2024_final.pdf",
                super::super::form_2550q::OFFICIAL_GUIDELINES_SHA256,
            ),
        ] {
            let bytes = std::fs::read(std::path::Path::new(&source_dir).join(filename))
                .expect("locked PDF must be readable");
            assert_eq!(hex::encode(Sha256::digest(&bytes)), expected_hash);
        }
    }
}
