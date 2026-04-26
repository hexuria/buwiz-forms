use super::form_2551q::Form2551QDraft;
use chrono::Local;
use std::collections::BTreeMap;

impl Form2551QDraft {
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let (tin1, tin2, tin3, branch) = split_tin(&self.tin);
        let total_credits = self.creditable_tax_withheld
            + if self.is_amended {
                self.tax_paid_previous
            } else {
                0.0
            };

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
        insert_money(&mut fields, "frm2551Qv2018:txt17", 0.0);
        insert_money(&mut fields, "frm2551Qv2018:txt18", total_credits);
        insert_money(&mut fields, "frm2551Qv2018:txt19", self.tax_payable);
        insert_money(&mut fields, "frm2551Qv2018:txt20", self.surcharge);
        insert_money(&mut fields, "frm2551Qv2018:txt21", self.interest);
        insert_money(&mut fields, "frm2551Qv2018:txt22", self.compromise);
        insert_money(&mut fields, "frm2551Qv2018:txt23", self.total_penalties);
        insert_money(&mut fields, "frm2551Qv2018:txt24", self.total_amount_payable);
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
                .map(|r| format!("{:.1}", r.tax_rate * 100.0))
                .unwrap_or_else(|| "0.00".to_string());
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
        insert(&mut fields, "frm2551Qv2018:txtCurrentPage", "2");
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
        return (
            parts.first().copied().unwrap_or("").to_string(),
            parts.get(1).copied().unwrap_or("").to_string(),
            parts.get(2).copied().unwrap_or("").to_string(),
            parts.get(3).copied().unwrap_or("000").to_string(),
        );
    }

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
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            _imap_enabled_compat: None,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
        };
        let mut draft = Form2551QDraft::new_from_profile(&profile, 2026, 1);
        draft.tin = "261708015000".to_string();
        draft.schedule_1[0].taxable_amount = 10_000.0;
        draft.creditable_tax_withheld = 12.5;
        draft.recompute();
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
        assert_eq!(fields["frm2551Qv2018:txtBranchCode"], "000");
    }

    #[test]
    fn xml_export_uses_canonical_field_map() {
        let xml = sample_draft().to_bir_xml_payload();

        assert!(xml.contains("frm2551Qv2018:txt18="));
        assert!(xml.contains("frm2551Qv2018:txtPg2TaxpayerName="));
        assert!(xml.contains("txtTotalSched1="));
    }
}
