use super::form_2551q::Form2551QDraft;
use chrono::Local;
use std::collections::BTreeMap;

impl Form2551QDraft {
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let (tin1, tin2, tin3, branch) = split_tin(&self.tin);

        // Legacy / UI Fields from original eBIRForms
        insert(&mut fields, "driveSelectTPExport", "0");
        insert(&mut fields, "ebirOnlineConfirmUsername", "");
        insert(&mut fields, "ebirOnlineSecret", "");
        insert(&mut fields, "ebirOnlineUsername", "");
        insert(&mut fields, "txtEnroll", "Y");
        insert(&mut fields, "txtFinalFlag", "1");
        insert(&mut fields, "txtDateExpiry", "");
        insert(&mut fields, "txtTaxAgentNo", "");

        insert(&mut fields, "frm2551Qv2018:forThe_1", "true");
        insert(&mut fields, "frm2551Qv2018:forThe_2", "false");
        insert(&mut fields, "frm2551Qv2018:rtnMonth", "12");
        insert(
            &mut fields,
            "__year_ended",
            format!("12{}", self.taxable_year),
        );
        insert(
            &mut fields,
            "frm2551Qv2018:txtYear",
            self.taxable_year.to_string(),
        );
        for i in 1..=4 {
            insert_bool(
                &mut fields,
                &format!("frm2551Qv2018:qtr_{}", i),
                self.quarter == i,
            );
        }

        insert_bool(&mut fields, "frm2551Qv2018:amendedRtn_1", self.is_amended);
        insert_bool(&mut fields, "frm2551Qv2018:amendedRtn_2", !self.is_amended);
        insert_bool(&mut fields, "frm2551Qv2018:taxTreaty_1", self.tax_relief);
        insert_bool(&mut fields, "frm2551Qv2018:taxTreaty_2", !self.tax_relief);
        insert(&mut fields, "frm2551Qv2018:txtTaxReliefSpecify", "");
        insert(&mut fields, "frm2551Qv2018:taxRate1", "true");
        insert(&mut fields, "frm2551Qv2018:taxRate2", "false");
        insert(&mut fields, "frm2551Qv2018:txtSheets", "0");

        insert(&mut fields, "frm2551Qv2018:txtTIN1", tin1.clone());
        insert(&mut fields, "frm2551Qv2018:txtTIN2", tin2.clone());
        insert(&mut fields, "frm2551Qv2018:txtTIN3", tin3.clone());
        insert(&mut fields, "frm2551Qv2018:txtBranchCode", branch.clone());
        insert(
            &mut fields,
            "frm2551Qv2018:txtRDOCode",
            self.rdo_code.clone(),
        );
        insert(
            &mut fields,
            "frm2551Qv2018:registeredName",
            self.taxpayer_name.clone(),
        );
        insert(
            &mut fields,
            "frm2551Qv2018:registeredAddress",
            self.registered_address.clone(),
        );
        insert(&mut fields, "frm2551Qv2018:zipCode", self.zip_code.clone());
        insert(
            &mut fields,
            "frm2551Qv2018:telNo",
            self.contact_number.clone(),
        );
        insert(&mut fields, "txtEmail", self.email.clone());

        insert_money(&mut fields, "frm2551Qv2018:txt14", self.total_tax_due);
        insert_money(
            &mut fields,
            "frm2551Qv2018:txt15",
            self.creditable_tax_withheld,
        );
        insert_money(&mut fields, "frm2551Qv2018:txt16", self.tax_paid_previous);
        insert(&mut fields, "frm2551Qv2018:txt17Specify", "");
        insert_money(&mut fields, "frm2551Qv2018:txt17", self.other_tax_credit);
        insert_money(&mut fields, "frm2551Qv2018:txt18", self.total_tax_credits);
        insert_money(&mut fields, "frm2551Qv2018:txt19", self.tax_payable);
        insert_money(&mut fields, "frm2551Qv2018:txt20", self.surcharge);
        insert_money(&mut fields, "frm2551Qv2018:txt21", self.interest);
        insert_money(&mut fields, "frm2551Qv2018:txt22", self.compromise);
        insert_money(&mut fields, "frm2551Qv2018:txt23", self.total_penalties);
        insert_money(
            &mut fields,
            "frm2551Qv2018:txt24",
            self.total_amount_payable,
        );
        insert_money(&mut fields, "txtTotalSched1", self.total_tax_due);
        insert(&mut fields, "frm2551Qv2018:overPayment1", "false");
        insert(&mut fields, "frm2551Qv2018:overPayment2", "false");

        for i in 0..6 {
            let row = self.schedule_1.get(i);
            let atc = row.map(|r| r.atc.as_str()).unwrap_or("0");
            let atc_amt = row
                .map(|r| format_money(r.taxable_amount))
                .unwrap_or_else(|| "0.00".to_string());
            let atc_rate = row
                .map(|r| {
                    let pct = r.tax_rate * 100.0;
                    if (pct - pct.round()).abs() < f64::EPSILON {
                        format!("{}", pct as u32)
                    } else {
                        format!("{}", pct)
                    }
                })
                .unwrap_or_else(|| "0".to_string());
            let atc_due = row
                .map(|r| format_money(r.tax_due))
                .unwrap_or_else(|| "0.00".to_string());

            insert(&mut fields, &format!("drpATC{}", i + 1), atc.to_string());
            insert(&mut fields, &format!("txtATCAmt{}", i + 1), atc_amt);
            insert(&mut fields, &format!("txtATCRate{}", i + 1), atc_rate);
            insert(&mut fields, &format!("txtATCDue{}", i + 1), atc_due);
        }

        insert(&mut fields, "frm2551Qv2018:txtPg2TIN1", tin1);
        insert(&mut fields, "frm2551Qv2018:txtPg2TIN2", tin2);
        insert(&mut fields, "frm2551Qv2018:txtPg2TIN3", tin3);
        insert(&mut fields, "frm2551Qv2018:txtPg2BranchCode", branch);
        insert(
            &mut fields,
            "frm2551Qv2018:txtPg2TaxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(&mut fields, "frm2551Qv2018:txtCurrentPage", "1");
        insert(&mut fields, "frm2551Qv2018:txtMaxPage", "2");

        for field in 25..=28 {
            insert(&mut fields, &format!("frm2551Qv2018:txt{}", field), "");
        }

        let now = Local::now();
        let dynamic_date = now.format("%m/%d/%Y %H:%M:%S").to_string();
        insert(&mut fields, "txtDateIssue", dynamic_date);

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

fn insert_money(map: &mut BTreeMap<String, String>, key: &str, value: f64) {
    insert(map, key, format_money(value));
}

fn format_money(value: f64) -> String {
    format!("{:.2}", value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;
    use crate::profile::{TaxpayerProfile, TaxpayerType};

    fn sample_draft() -> Form2551QDraft {
        let profile = TaxpayerProfile {
            id: None,
            full_name: "Uriah Galang".into(),
            tin: Tin {
                segment1: "261".into(),
                segment2: "708".into(),
                segment3: "015".into(),
                branch: "000".into(),
            },
            rdo_code: "018".into(),
            line_of_business: "Software".into(),
            registered_address: "New Cabalan".into(),
            zip_code: "2200".into(),
            phone: "09156837000".into(),
            email: "tax@example.com".into(),
            default_form_type: "2551Qv2018".into(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
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
            per_year_forms: Default::default(),
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
            profile_pin_hash: None,
            totp_secret: None,
        };
        let mut draft = Form2551QDraft::new_from_profile(&profile, 2026, 1);
        draft.tin = "261708015000".to_string();
        draft.schedule_1[0].taxable_amount = 10_000.0;
        draft.creditable_tax_withheld = 12.5;
        draft.recompute(None);
        draft
    }

    #[test]
    fn field_map_contains_required_print_and_xml_keys() {
        let fields = sample_draft().to_bir_field_map();

        for key in [
            "frm2551Qv2018:txtYear",
            "__year_ended",
            "frm2551Qv2018:txtTIN1",
            "frm2551Qv2018:txtTIN2",
            "frm2551Qv2018:txtTIN3",
            "frm2551Qv2018:txtBranchCode",
            "frm2551Qv2018:txt14",
            "frm2551Qv2018:txt18",
            "frm2551Qv2018:txt19",
            "frm2551Qv2018:txt20",
            "frm2551Qv2018:txt21",
            "frm2551Qv2018:txt22",
            "frm2551Qv2018:txt23",
            "frm2551Qv2018:txt24",
            "txtTotalSched1",
            "frm2551Qv2018:txtPg2TIN1",
            "frm2551Qv2018:txtPg2TaxpayerName",
        ] {
            assert!(fields.contains_key(key), "missing {key}");
        }

        assert_eq!(fields["frm2551Qv2018:txtTIN1"], "261");
        assert_eq!(fields["frm2551Qv2018:txtTIN2"], "708");
        assert_eq!(fields["frm2551Qv2018:txtTIN3"], "015");
        assert_eq!(fields["frm2551Qv2018:txtBranchCode"], "00000");
    }

    #[test]
    fn xml_export_uses_canonical_field_map() {
        let xml = sample_draft().to_bir_xml_payload();

        assert!(xml.contains("frm2551Qv2018:txt18="));
        assert!(xml.contains("frm2551Qv2018:txtPg2TaxpayerName="));
        assert!(xml.contains("txtTotalSched1="));
    }
}
