//! BIR field mapping for Form 1701Q.
//!
//! This is a conservative scaffold mapped from `docs/efps_manifests/1701Q.json`.
//! It covers profile, period, option defaults, and current lightweight totals.
//! Full completion still requires expanding `Form1701QDraft` to the complete
//! 106-field manifest surface and calibrating `formtypes/1701Qv2018`.

use super::form_1701q::Form1701QDraft;
use chrono::Local;
use std::collections::BTreeMap;

impl Form1701QDraft {
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let (tin1, tin2, tin3, branch) = split_tin(&self.tin);
        let quarter = self.quarter_u8();
        let is_amended = false;

        // Common fields used by the eBIR pseudo-XML wrapper.
        insert(&mut fields, "driveSelectTPExport", "");
        insert(&mut fields, "ebirOnlineConfirmUsername", "");
        insert(&mut fields, "ebirOnlineSecret", "");
        insert(&mut fields, "ebirOnlineUsername", "");
        insert(&mut fields, "txtDateExpiry", "");
        insert(
            &mut fields,
            "txtDateIssue",
            Local::now().format("%m/%d/%Y %H:%M:%S").to_string(),
        );
        insert(&mut fields, "txtEmail", self.email.clone());
        insert(&mut fields, "txtEnroll", "Y");
        insert(&mut fields, "txtFinalFlag", "1");
        insert(&mut fields, "txtTaxAgentNo", "");

        // FDF/save-file aliases from the 1701Q manifest.
        insert(
            &mut fields,
            "txtFileName",
            self.default_submission_filename(),
        );
        insert(&mut fields, "newOfflineForm", "Y");
        insert(&mut fields, "currentVersion", "1701Qv2018");
        insert(&mut fields, "tinA", tin1.clone());
        insert(&mut fields, "tinB", tin2.clone());
        insert(&mut fields, "tinC", tin3.clone());
        insert(&mut fields, "branchCode", branch.clone());
        insert(&mut fields, "returnPeriodYear", self.taxable_year.clone());
        insert(&mut fields, "quarter", quarter.to_string());
        insert(&mut fields, "amendedYn", if is_amended { "Y" } else { "N" });
        insert(&mut fields, "atc", "");
        insert(&mut fields, "spsAtc", "");
        insert(&mut fields, "itemizedDeduction", "");
        insert(&mut fields, "spsItemizedDeduction", "");
        insert(&mut fields, "treaty", "N");
        insert(&mut fields, "selectedTreaty", "");
        insert(&mut fields, "computedTaxDue", money(self.total_tax_due));
        insert(
            &mut fields,
            "computedAmtPaybl",
            money(self.total_amount_payable),
        );
        insert(
            &mut fields,
            "tempAggrAmtPaybl",
            money(self.total_amount_payable),
        );

        // Official HTML field IDs from the 1701Q manifest.
        insert(&mut fields, "frm1701q:txtYear", self.taxable_year.clone());
        insert_bool(&mut fields, "frm1701q:DateQuarter_1", quarter == 1);
        insert_bool(&mut fields, "frm1701q:DateQuarter_2", quarter == 2);
        insert_bool(&mut fields, "frm1701q:DateQuarter_3", quarter == 3);
        insert_bool(&mut fields, "frm1701q:AmendedRtn_1", is_amended);
        insert_bool(&mut fields, "frm1701q:AmendedRtn_2", !is_amended);
        insert(&mut fields, "frm1701q:txtSheets", "0");
        insert(&mut fields, "frm1701q:txt5TIN1", tin1);
        insert(&mut fields, "frm1701q:txt5TIN2", tin2);
        insert(&mut fields, "frm1701q:txt5TIN3", tin3);
        insert(&mut fields, "frm1701q:txt5BranchCode", branch);
        insert(&mut fields, "frm1701q:txt5RDOCode", self.rdo_code.clone());
        insert(&mut fields, "frm1701q:txt7TIN1", "");
        insert(&mut fields, "frm1701q:txt7TIN2", "");
        insert(&mut fields, "frm1701q:txt7TIN3", "");
        insert(&mut fields, "frm1701q:txt7BranchCode", "");
        insert(&mut fields, "frm1701q:txt7RDOCode", "");
        insert(
            &mut fields,
            "frm1701q:txtTaxPayername",
            self.taxpayer_name.clone(),
        );
        insert(&mut fields, "frm1701q:txtSpousename", "");
        insert(
            &mut fields,
            "frm1701q:txt11Address",
            self.registered_address.clone(),
        );
        insert(&mut fields, "frm1701q:txt12Address", "");
        insert(&mut fields, "frm1701q:txt13BirthMonth", "");
        insert(&mut fields, "frm1701q:txt13BirthDay", "");
        insert(&mut fields, "frm1701q:txt13BirthYear", "");
        insert(&mut fields, "frm1701q:txt14zip", self.zip_code.clone());
        insert(
            &mut fields,
            "frm1701q:txt15Telno",
            self.contact_number.clone(),
        );
        insert(&mut fields, "frm1701q:txt16BirthMonth", "");
        insert(&mut fields, "frm1701q:txt16BirthDay", "");
        insert(&mut fields, "frm1701q:txt16BirthYear", "");
        insert(&mut fields, "frm1701q:txt17", "");
        insert(&mut fields, "frm1701q:txt18Telno", "");
        insert(&mut fields, "frm1701q:txt19", "");
        insert(&mut fields, "frm1701q:txt20A", "II011");
        insert_bool(&mut fields, "frm1701q:optATC20_1", false);
        insert(&mut fields, "frm1701q:txt20B", "II012");
        insert_bool(&mut fields, "frm1701q:optATC20_2", false);
        insert(&mut fields, "frm1701q:txt20C", "II013");
        insert_bool(&mut fields, "frm1701q:optATC20_3", false);
        insert(&mut fields, "frm1701q:txt21", "");
        insert(&mut fields, "frm1701q:txt22A", "II011");
        insert_bool(&mut fields, "frm1701q:optATC22_1", false);
        insert(&mut fields, "frm1701q:txt22B", "II012");
        insert_bool(&mut fields, "frm1701q:optATC22_2", false);
        insert(&mut fields, "frm1701q:txt22C", "II013");
        insert_bool(&mut fields, "frm1701q:optATC22_3", false);
        insert_bool(&mut fields, "frm1701:optMethodOfDeduction23:_1", false);
        insert_bool(&mut fields, "frm1701:optMethodOfDeduction23:_2", false);
        insert_bool(&mut fields, "frm1701:optMethodOfDeduction24:_1", false);
        insert_bool(&mut fields, "frm1701:optMethodOfDeduction24:_2", false);
        insert_bool(&mut fields, "frm1701q:SelTreaty_1", false);
        insert_bool(&mut fields, "frm1701q:SelTreaty_2", true);
        insert(&mut fields, "frm1701q:txtTaxRelief25", "");

        for key in [
            "txt26A", "txt26B", "txt27A", "txt27B", "txt28A", "txt28B", "txt29A", "txt29B",
            "txt30A", "txt30B", "txt31A", "txt31B", "txt32A", "txt32B", "txt33A", "txt33B",
            "txt34A", "txt34B", "txt35A", "txt35B", "txt36A", "txt36B", "txt37A", "txt37B",
            "txt38A", "txt38B", "txt38C", "txt38D", "txt38E", "txt38F", "txt38G", "txt38H",
            "txt38I", "txt38J", "txt38K", "txt38L", "txt38M", "txt38N", "txt39A", "txt39B",
            "txt40A", "txt40B", "txt40C", "txt40D", "txt40E", "txt40F", "txt40G", "txt40H",
            "txt41A", "txt41B",
        ] {
            insert(&mut fields, &format!("frm1701q:{key}"), "0.00");
        }
        insert(
            &mut fields,
            "frm1701q:txt41C",
            money(self.total_amount_payable),
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

fn money(value: f64) -> String {
    format!("{value:.2}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_map_contains_profile_period_and_total_keys() {
        let draft = Form1701QDraft {
            tin: "123456789000".into(),
            rdo_code: "039".into(),
            taxpayer_name: "JUAN DELA CRUZ".into(),
            registered_address: "OLONGAPO".into(),
            zip_code: "2200".into(),
            contact_number: "09170000000".into(),
            email: "test@example.com".into(),
            taxable_year: "2026".into(),
            quarter: "2".into(),
            total_tax_due: 123.45,
            total_amount_payable: 123.45,
            ..Default::default()
        };

        let fields = draft.to_bir_field_map();
        assert_eq!(fields["frm1701q:txtYear"], "2026");
        assert_eq!(fields["frm1701q:DateQuarter_2"], "true");
        assert_eq!(fields["frm1701q:txt5TIN1"], "123");
        assert_eq!(fields["frm1701q:txt5TIN2"], "456");
        assert_eq!(fields["frm1701q:txt5TIN3"], "789");
        assert_eq!(fields["frm1701q:txt5BranchCode"], "000");
        assert_eq!(fields["frm1701q:txtTaxPayername"], "JUAN DELA CRUZ");
        assert_eq!(fields["computedTaxDue"], "123.45");
        assert_eq!(fields["frm1701q:txt41C"], "123.45");
    }

    #[test]
    fn xml_payload_contains_1701q_keys() {
        let draft = Form1701QDraft {
            tin: "123456789000".into(),
            rdo_code: "039".into(),
            taxpayer_name: "JUAN DELA CRUZ".into(),
            taxable_year: "2026".into(),
            quarter: "1".into(),
            ..Default::default()
        };

        let xml = draft.to_bir_xml_payload();
        assert!(xml.contains("frm1701q:txtYear"));
        assert!(xml.contains("frm1701q:txt5TIN1"));
        assert!(xml.contains("computedTaxDue"));
    }
}
