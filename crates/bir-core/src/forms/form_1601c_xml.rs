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
        let filename = format!(
            "1601C_{}{}{}{}_{:02}_{}_{}",
            tin1, tin2, tin3, branch, self.month, self.taxable_year, timestamp
        );
        insert(&mut fields, "txtFileName", filename);

        // Header
        insert(
            &mut fields,
            "frm1601c:txtMonth",
            format!("{:02}", self.month),
        );
        insert(
            &mut fields,
            "frm1601c:txtYear",
            self.taxable_year.to_string(),
        );

        insert_bool_1_2(&mut fields, "frm1601c:AmendedRtn", self.is_amended);
        insert_bool_1_2(&mut fields, "frm1601c:TaxWithheld", self.any_taxes_withheld);

        insert(
            &mut fields,
            "frm1601c:txtSheets",
            self.number_of_sheets.to_string(),
        );
        insert(&mut fields, "frm1601c:txtATC", self.atc.clone());

        // Identity
        insert(&mut fields, "frm1601c:txtTIN1", tin1.clone());
        insert(&mut fields, "frm1601c:txtTIN2", tin2.clone());
        insert(&mut fields, "frm1601c:txtTIN3", tin3.clone());
        insert(&mut fields, "frm1601c:txtBranchCode", branch.clone());
        insert(&mut fields, "frm1601c:txtRDOCode", self.rdo_code.clone());

        // Use encoded spaces if requested by original XML format, but text should be fine
        insert(
            &mut fields,
            "frm1601c:txtTaxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(
            &mut fields,
            "frm1601c:txtAddress",
            self.registered_address.clone(),
        );
        insert(&mut fields, "frm1601c:txtZipCode", self.zip_code.clone());
        insert(
            &mut fields,
            "frm1601c:txtTelNum",
            self.contact_number.clone(),
        );

        insert_bool(
            &mut fields,
            "frm1601c:CatAgent_P",
            self.category_of_agent == "P",
        );
        insert_bool(
            &mut fields,
            "frm1601c:CatAgent_G",
            self.category_of_agent == "G",
        );

        insert(&mut fields, "txtEmail", self.email_address.clone());

        // Tax Relief / Treaty
        insert_bool_1_2(&mut fields, "frm1601c:SpecialTax", false);
        insert(&mut fields, "frm1601c:selTreaty", "0");

        // Part II - Computation
        insert_money(
            &mut fields,
            "frm1601c:txtTax14",
            self.tax_14_total_compensation,
        );
        insert_money(
            &mut fields,
            "frm1601c:txtTax15",
            self.tax_15_statutory_minimum_wage,
        );
        insert_money(&mut fields, "frm1601c:txtTax16", self.tax_16_holiday_pay);
        insert_money(&mut fields, "frm1601c:txtTax17", self.tax_17_13th_month_pay);
        insert_money(&mut fields, "frm1601c:txtTax18", self.tax_18_de_minimis);
        insert_money(&mut fields, "frm1601c:txtTax19", self.tax_19_sss_gsis);

        insert(
            &mut fields,
            "frm1601c:txt20Other",
            self.tax_20_other_name.clone(),
        );
        insert_money(&mut fields, "frm1601c:txtTax20", self.tax_20_other_amount);

        insert_money(
            &mut fields,
            "frm1601c:txtTax21",
            self.tax_21_total_non_taxable,
        );
        insert_money(&mut fields, "frm1601c:txtTax22", self.tax_22_total_taxable);
        insert_money(&mut fields, "frm1601c:txtTax23", self.tax_23_not_subject);
        insert_money(&mut fields, "frm1601c:txtTax24", self.tax_24_net_taxable);
        insert_money(
            &mut fields,
            "frm1601c:txtTax25",
            self.tax_25_total_taxes_withheld,
        );
        insert_money(&mut fields, "frm1601c:txtTax26", self.tax_26_adjustment);
        insert_money(
            &mut fields,
            "frm1601c:txtTax27",
            self.tax_27_taxes_withheld_for_remittance,
        );
        insert_money(
            &mut fields,
            "frm1601c:txtTax28",
            self.tax_28_tax_remitted_previously,
        );

        insert(
            &mut fields,
            "frm1601c:txt29Other",
            self.tax_29_other_remittances_name.clone(),
        );
        insert_money(
            &mut fields,
            "frm1601c:txtTax29",
            self.tax_29_other_remittances_amount,
        );

        insert_money(
            &mut fields,
            "frm1601c:txtTax30",
            self.tax_30_total_tax_remittances,
        );
        insert_money(&mut fields, "frm1601c:txtTax31", self.tax_31_tax_still_due);
        insert_money(&mut fields, "frm1601c:txtTax32", self.tax_32_surcharge);
        insert_money(&mut fields, "frm1601c:txtTax33", self.tax_33_interest);
        insert_money(&mut fields, "frm1601c:txtTax34", self.tax_34_compromise);
        insert_money(
            &mut fields,
            "frm1601c:txtTax35",
            self.tax_35_total_penalties,
        );
        insert_money(
            &mut fields,
            "frm1601c:txtTax36",
            self.tax_36_total_amount_payable,
        );

        // Part III - Details of Payment
        insert(&mut fields, "txtTaxAgentNo", "");
        insert(&mut fields, "txtDateIssue", "");
        insert(&mut fields, "txtDateExpiry", "");

        for i in 37..=40 {
            insert(&mut fields, &format!("frm1601c:txtAgency{}", i), "");
            insert(&mut fields, &format!("frm1601c:txtNumber{}", i), "");
            insert(&mut fields, &format!("frm1601c:txtDate{}", i), "");
            insert(&mut fields, &format!("frm1601c:txtAmount{}", i), "");
        }
        insert(&mut fields, "frm1601c:txtParticular40", "");

        // Page 2 Header
        insert(&mut fields, "frm1601c:txtPg2TIN1", tin1);
        insert(&mut fields, "frm1601c:txtPg2TIN2", tin2);
        insert(&mut fields, "frm1601c:txtPg2TIN3", tin3);
        insert(&mut fields, "frm1601c:txtPg2BranchCode", branch);
        insert(
            &mut fields,
            "frm1601c:txtPg2TaxpayerName",
            self.taxpayer_name.clone(),
        );

        // Schedule 1
        for i in 0..3 {
            insert(&mut fields, &format!("chkScheduleDelete{}", i), "false");
            insert(
                &mut fields,
                &format!("frm1601c:sched1:txtMonthYear{}", i),
                "",
            );
            insert(
                &mut fields,
                &format!("frm1601c:sched1:txtDatePaid{}", i),
                "",
            );
            insert(
                &mut fields,
                &format!("frm1601c:sched1:txtBankCode{}", i),
                "",
            );
            insert(&mut fields, &format!("frm1601c:sched1:txtNumber{}", i), "");
            insert(
                &mut fields,
                &format!("frm1601c:sched1:txtTaxPaid{}", i),
                "0.00",
            );
            insert(
                &mut fields,
                &format!("frm1601c:sched1:txtShouldTaxDue{}", i),
                "0.00",
            );
            insert(
                &mut fields,
                &format!("frm1601c:sched1:txtAdjustments{}", i),
                "0.00",
            );
        }
        insert(&mut fields, "frm1601c:sched1:txtTotal1", "0.00");

        // Pagination
        insert(&mut fields, "frm1601c:txtCurrentPage", "1");
        insert(&mut fields, "frm1601c:txtMaxPage", "2");
        insert(
            &mut fields,
            "frm1601c:txtLineBus",
            self.line_of_business.clone(),
        );

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
    insert(map, key, if value { "true" } else { "false" });
}

fn insert_bool_1_2(map: &mut BTreeMap<String, String>, key: &str, value: bool) {
    insert(
        map,
        &format!("{}_1", key),
        if value { "true" } else { "false" },
    );
    insert(
        map,
        &format!("{}_2", key),
        if !value { "true" } else { "false" },
    );
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
            birth_date: None,
            is_archived: false,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_versions: vec![],
            compliance_source_mode: Default::default(),
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: true,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
            profile_pin_hash: None,
            totp_secret: None,
        };

        let mut draft = Form1601CDraft::new_from_profile(&profile, 2026, 5);
        draft.tax_14_total_compensation = 100_000.0;
        draft.tax_17_13th_month_pay = 20_000.0;
        draft.tax_25_total_taxes_withheld = 15_000.0;
        draft.compute();

        let field_map = draft.to_bir_field_map();

        assert_eq!(field_map["frm1601c:txtTax14"], "100000.00");
        assert_eq!(field_map["frm1601c:txtTax17"], "20000.00");
        assert_eq!(field_map["frm1601c:txtTax21"], "20000.00"); // 15 to 20 = 20k
        assert_eq!(field_map["frm1601c:txtTax22"], "80000.00"); // 14 - 21 = 80k
        assert_eq!(field_map["frm1601c:txtTax27"], "15000.00"); // Taxes withheld
        assert_eq!(field_map["frm1601c:txtTax36"], "15000.00"); // Total payable

        let xml = draft.to_bir_xml_payload();
        assert!(xml.contains("<div>frm1601c:txtTax22=80000.00frm1601c:txtTax22=</div>"));
    }
}
