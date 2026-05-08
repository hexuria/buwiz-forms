use super::form_1601c::Form1601CDraft;
use chrono::Local;
use std::collections::BTreeMap;

impl Form1601CDraft {
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let (tin1, tin2, tin3, branch) = split_tin(&self.tin);

        // Required hidden fields from eFPS
        insert(&mut fields, "newOfflineForm", "Y");
        
        let now = Local::now();
        let timestamp = now.format("%m%d%Y%H%M%S").to_string();
        // Standard BIR filename convention: FormCode_TIN_Period_Timestamp
        let filename = format!("1601C_{}{}{}{}_{}{}_{}", 
            tin1, tin2, tin3, branch, 
            format!("{:02}", self.month), 
            self.taxable_year, 
            timestamp
        );
        insert(&mut fields, "txtFileName", filename);

        // Header
        insert(&mut fields, "previousMonth", format!("{:02}", self.month));
        insert(&mut fields, "currentYear", self.taxable_year.to_string());
        insert_bool(&mut fields, "opt2AmendedYN", self.is_amended);
        insert(&mut fields, "sheets", self.number_of_sheets.to_string());
        insert_bool_yn(&mut fields, "taxWithheldFlag", self.any_taxes_withheld);
        
        // Identity
        insert(&mut fields, "txtTin1", tin1);
        insert(&mut fields, "txtTin2", tin2);
        insert(&mut fields, "txtTin3", tin3);
        insert(&mut fields, "txtBranchCode", branch);
        insert(&mut fields, "categoryFlag", self.category_of_agent.clone());

        // Part II - Computation
        insert_money(&mut fields, "field15Compensation", self.tax_15_total_compensation);
        insert_money(&mut fields, "field16NonTaxableCompensation", self.tax_16a_nontaxable);
        insert_money(&mut fields, "field16StatutoryMinimumWage", self.tax_16b_not_subject);
        insert_money(&mut fields, "field16MinimumWageEarner", self.tax_16c_exempt);
        
        insert_money(&mut fields, "field17TaxableCompensation", self.tax_17_regular);
        insert_money(&mut fields, "field18TaxRequired", self.tax_18_supplementary);
        insert_money(&mut fields, "field19Adjustment", self.tax_19_total_taxable);
        insert_money(&mut fields, "field20TaxRequiredRemittance", self.tax_20_required_withheld);
        
        // Adjustments
        insert_money(&mut fields, "field21TaxRemitted", self.tax_21a_previous_withheld);
        insert_money(&mut fields, "field21OtherPaymentsMade", self.tax_21b_other_payments);
        insert_money(&mut fields, "field22TaxStillDue", self.tax_22_still_due);
        
        // Penalties
        insert_money(&mut fields, "field23ASurcharge", self.tax_24a_surcharge);
        insert_money(&mut fields, "field23BInterest", self.tax_24b_interest);
        insert_money(&mut fields, "field23CCompromise", self.tax_24c_compromise);
        insert_money(&mut fields, "field23DPenalties", self.tax_24d_total_penalties);
        
        // Totals
        insert_money(&mut fields, "field24TaxAmount", self.tax_25_total_payable);
        insert_money(&mut fields, "field25Total", self.tax_25_total_payable);

        // Blank out the Section A schedule lines by default, per FDF keys
        insert(&mut fields, "monthlyYearlyTag", "");
        insert(&mut fields, "datePaidStr", "");
        insert(&mut fields, "monthPreviousStr", "");
        insert(&mut fields, "bankRorNo", "");
        insert(&mut fields, "bankCode", "");
        insert(&mut fields, "taxPaidMonth", "");
        insert(&mut fields, "taxDueMonth", "");
        insert(&mut fields, "monthlyadjustmentA", "");
        insert(&mut fields, "monthlyadjustmentB", "");
        
        insert(&mut fields, "monthlyYearlyTag1", "");
        insert(&mut fields, "datePaidStr1", "");
        insert(&mut fields, "monthPreviousStr1", "");
        insert(&mut fields, "bankRorNo1", "");
        insert(&mut fields, "bankCode1", "");
        insert(&mut fields, "taxPaidMonth1", "");
        insert(&mut fields, "taxDueMonth1", "");
        insert(&mut fields, "monthlyadjustmentA1", "");
        insert(&mut fields, "monthlyadjustmentB1", "");

        insert(&mut fields, "monthlyYearlyTag2", "");
        insert(&mut fields, "datePaidStr2", "");
        insert(&mut fields, "monthPreviousStr2", "");
        insert(&mut fields, "bankRorNo2", "");
        insert(&mut fields, "bankCode2", "");
        insert(&mut fields, "taxPaidMonth2", "");
        insert(&mut fields, "taxDueMonth2", "");
        insert(&mut fields, "monthlyadjustmentA2", "");
        insert(&mut fields, "monthlyadjustmentB2", "");

        fields
    }

    pub fn to_bir_xml_payload(&self) -> String {
        crate::bir_xml::generate_bir_xml(&self.to_bir_field_map())
    }
}

fn split_tin(tin: &str) -> (String, String, String, String) {
    if tin.contains('-') {
        let parts: Vec<&str> = tin.split('-').collect();
        let branch = parts.get(3).copied().unwrap_or("00000");
        return (
            parts.first().copied().unwrap_or("").to_string(),
            parts.get(1).copied().unwrap_or("").to_string(),
            parts.get(2).copied().unwrap_or("").to_string(),
            format!("{:0>5}", branch),
        );
    }

    let digits: String = tin.chars().filter(|ch| ch.is_ascii_digit()).collect();
    let segment = |start: usize, end: usize| -> String {
        digits
            .get(start..end.min(digits.len()))
            .unwrap_or("")
            .to_string()
    };

    let branch_raw = digits.get(9..).filter(|s| !s.is_empty()).unwrap_or("00000");

    (
        segment(0, 3),
        segment(3, 6),
        segment(6, 9),
        format!("{:0>5}", branch_raw),
    )
}

fn insert(map: &mut BTreeMap<String, String>, key: &str, value: impl Into<String>) {
    map.insert(key.to_string(), value.into());
}

fn insert_bool(map: &mut BTreeMap<String, String>, key: &str, value: bool) {
    insert(map, key, if value { "Y" } else { "N" });
}

fn insert_bool_yn(map: &mut BTreeMap<String, String>, key: &str, value: bool) {
    insert(map, key, if value { "Y" } else { "N" });
}

fn insert_money(map: &mut BTreeMap<String, String>, key: &str, value: f64) {
    insert(map, key, format!("{:.2}", value));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;
    use crate::profile::{TaxpayerProfile, TaxpayerType};

    #[test]
    fn test_1601c_xml_generation() {
        let profile = TaxpayerProfile {
            id: None,
            full_name: "Test Corp".into(),
            tin: Tin {
                segment1: "123".into(),
                segment2: "456".into(),
                segment3: "789".into(),
                branch: "00000".into(),
            },
            rdo_code: "043".into(),
            line_of_business: "Retail".into(),
            registered_address: "Manila".into(),
            zip_code: "1000".into(),
            phone: "09123456789".into(),
            email: "test@example.com".into(),
            default_form_type: "1601C".into(),
            taxpayer_type: TaxpayerType::Corporation,
            is_vat_registered: true,
            business_start_date: None,
            is_archived: false,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            _imap_enabled_compat: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            tax_classification: None,
            opted_for_8_percent_flat_rate: false,
            has_employees: true,
            profile_pin_hash: None,
            totp_secret: None,
        };

        let mut draft = Form1601CDraft::new_from_profile(&profile, 2026, 5);
        draft.tax_17_regular = 100_000.0;
        draft.tax_18_supplementary = 20_000.0;
        draft.tax_20_required_withheld = 15_000.0;
        draft.compute();

        let field_map = draft.to_bir_field_map();
        
        assert_eq!(field_map["field17TaxableCompensation"], "100000.00");
        assert_eq!(field_map["field18TaxRequired"], "20000.00");
        assert_eq!(field_map["field19Adjustment"], "120000.00"); // 19 = 17+18
        assert_eq!(field_map["field22TaxStillDue"], "15000.00"); // 20 - 21A - 21B
        assert_eq!(field_map["field24TaxAmount"], "15000.00"); // 22 + 24D

        let xml = draft.to_bir_xml_payload();
        assert!(xml.contains("<div>field19Adjustment=120000.00field19Adjustment=</div>"));
    }
}
