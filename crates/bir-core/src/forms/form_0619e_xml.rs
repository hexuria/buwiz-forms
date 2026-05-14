//! BIR field mapping for Form 0619E.
//!
//! Auto-generated from savefile: 00000000000000-0619E-042026.xml
//! Maps Rust struct fields to BIR pseudo-XML field IDs.

use super::form_0619e::Form0619EDraft;
use chrono::Local;
use std::collections::BTreeMap;

impl Form0619EDraft {
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let (tin1, tin2, tin3, branch) = split_tin(&self.tin);

        // === Common fields (all forms) ===
        insert(&mut fields, "driveSelectTPExport", "");
        insert(&mut fields, "ebirOnlineConfirmUsername", "");
        insert(&mut fields, "ebirOnlineSecret", "");
        insert(&mut fields, "ebirOnlineUsername", "");
        insert(&mut fields, "txtDateExpiry", "");
        let now = Local::now();
        insert(
            &mut fields,
            "txtDateIssue",
            now.format("%m/%d/%Y %H:%M:%S").to_string(),
        );
        insert(&mut fields, "txtEmail", self.email.clone());
        insert(&mut fields, "txtEnroll", "Y");
        insert(&mut fields, "txtFinalFlag", "1");
        insert(&mut fields, "txtTaxAgentNo", "");

        // === Form-specific fields ===
        insert_bool(&mut fields, "frm0619E:optAmend:N", self.opt_amend_n);
        insert_bool(&mut fields, "frm0619E:optAmend:Y", self.opt_amend_y);
        insert_bool(&mut fields, "frm0619E:optCategory:G", self.opt_category_g);
        insert_bool(&mut fields, "frm0619E:optCategory:P", self.opt_category_p);
        insert_bool(&mut fields, "frm0619E:optWithheld:N", self.opt_withheld_n);
        insert_bool(&mut fields, "frm0619E:optWithheld:Y", self.opt_withheld_y);
        insert(&mut fields, "frm0619E:txtAddress", self.txt_address.clone());
        insert(&mut fields, "frm0619E:txtAtc", self.txt_atc.clone());
        insert(&mut fields, "frm0619E:txtBranchCode", branch.clone());
        insert(
            &mut fields,
            "frm0619E:txtDueDay",
            self.txt_due_day.to_string(),
        );
        insert(
            &mut fields,
            "frm0619E:txtDueMonth",
            format!("{:02}", self.month + 1),
        );
        insert(
            &mut fields,
            "frm0619E:txtDueYear",
            self.taxable_year.to_string(),
        );
        insert(
            &mut fields,
            "frm0619E:txtLineBus",
            self.txt_line_bus.clone(),
        );
        insert(
            &mut fields,
            "frm0619E:txtMonth",
            format!("{:02}", self.month),
        );
        insert(&mut fields, "frm0619E:txtRDOCode", self.rdo_code.clone());
        insert(&mut fields, "frm0619E:txtTIN1", tin1.clone());
        insert(&mut fields, "frm0619E:txtTIN2", tin2.clone());
        insert(&mut fields, "frm0619E:txtTIN3", tin3.clone());
        insert_money(&mut fields, "frm0619E:txtTax14", self.txt_tax14);
        insert_money(&mut fields, "frm0619E:txtTax15", self.txt_tax15);
        insert_money(&mut fields, "frm0619E:txtTax16", self.txt_tax16);
        insert_money(&mut fields, "frm0619E:txtTax17A", self.txt_tax17a);
        insert_money(&mut fields, "frm0619E:txtTax17B", self.txt_tax17b);
        insert_money(&mut fields, "frm0619E:txtTax17C", self.txt_tax17c);
        insert_money(&mut fields, "frm0619E:txtTax17D", self.txt_tax17d);
        insert_money(&mut fields, "frm0619E:txtTax18", self.txt_tax18);
        insert(
            &mut fields,
            "frm0619E:txtTaxTypeCode",
            self.txt_tax_type_code.clone(),
        );
        insert(
            &mut fields,
            "frm0619E:txtTaxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(
            &mut fields,
            "frm0619E:txtTelNum",
            self.contact_number.clone(),
        );
        insert(
            &mut fields,
            "frm0619E:txtYear",
            self.taxable_year.to_string(),
        );
        insert(&mut fields, "frm0619E:txtZipCode", self.zip_code.clone());
        insert(&mut fields, "txtAgency19", self.txt_agency19.clone());
        insert(&mut fields, "txtAgency20", self.txt_agency20.clone());
        insert(&mut fields, "txtAgency21", self.txt_agency21.clone());
        insert(&mut fields, "txtAgency22", self.txt_agency22.clone());
        insert(&mut fields, "txtAmount19", self.txt_amount19.clone());
        insert(&mut fields, "txtAmount20", self.txt_amount20.clone());
        insert(&mut fields, "txtAmount21", self.txt_amount21.clone());
        insert(&mut fields, "txtAmount22", self.txt_amount22.clone());
        insert(&mut fields, "txtDate19", self.txt_date19.clone());
        insert(&mut fields, "txtDate20", self.txt_date20.clone());
        insert(&mut fields, "txtDate21", self.txt_date21.clone());
        insert(&mut fields, "txtDate22", self.txt_date22.clone());
        insert(&mut fields, "txtNumber19", self.txt_number19.clone());
        insert(&mut fields, "txtNumber20", self.txt_number20.clone());
        insert(&mut fields, "txtNumber21", self.txt_number21.clone());
        insert(&mut fields, "txtNumber22", self.txt_number22.clone());
        insert(
            &mut fields,
            "txtParticular22",
            self.txt_particular22.clone(),
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
