//! BIR field mapping for Form 0619F.
//!
//! Auto-generated from savefile: 00000000000000-0619F-042026WB.xml
//! Maps Rust struct fields to BIR pseudo-XML field IDs.

use super::form_0619f::Form0619FDraft;
use chrono::Local;
use std::collections::BTreeMap;

impl Form0619FDraft {
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
        insert_bool(&mut fields, "frm0619F:optAmend:N", self.opt_amend_n);
        insert_bool(&mut fields, "frm0619F:optAmend:Y", self.opt_amend_y);
        insert_bool(&mut fields, "frm0619F:optCategory:G", self.opt_category_g);
        insert_bool(&mut fields, "frm0619F:optCategory:P", self.opt_category_p);
        insert_bool(&mut fields, "frm0619F:optWithheld:N", self.opt_withheld_n);
        insert_bool(&mut fields, "frm0619F:optWithheld:Y", self.opt_withheld_y);
        insert(&mut fields, "frm0619F:txtAddress", self.txt_address.clone());
        insert(&mut fields, "frm0619F:txtBranchCode", branch.clone());
        insert(
            &mut fields,
            "frm0619F:txtDueDay",
            self.txt_due_day.to_string(),
        );
        insert(
            &mut fields,
            "frm0619F:txtDueMonth",
            format!("{:02}", self.month + 1),
        );
        insert(
            &mut fields,
            "frm0619F:txtDueYear",
            self.taxable_year.to_string(),
        );
        insert(
            &mut fields,
            "frm0619F:txtLineBus",
            self.txt_line_bus.clone(),
        );
        insert(
            &mut fields,
            "frm0619F:txtMonth",
            format!("{:02}", self.month),
        );
        insert(&mut fields, "frm0619F:txtRDOCode", self.rdo_code.clone());
        insert(&mut fields, "frm0619F:txtTIN1", tin1.clone());
        insert(&mut fields, "frm0619F:txtTIN2", tin2.clone());
        insert(&mut fields, "frm0619F:txtTIN3", tin3.clone());
        insert_money(&mut fields, "frm0619F:txtTax13", self.txt_tax13);
        insert_money(&mut fields, "frm0619F:txtTax14", self.txt_tax14);
        insert_money(&mut fields, "frm0619F:txtTax15", self.txt_tax15);
        insert_money(&mut fields, "frm0619F:txtTax16", self.txt_tax16);
        insert_money(&mut fields, "frm0619F:txtTax17", self.txt_tax17);
        insert_money(&mut fields, "frm0619F:txtTax18A", self.txt_tax18a);
        insert_money(&mut fields, "frm0619F:txtTax18B", self.txt_tax18b);
        insert_money(&mut fields, "frm0619F:txtTax18C", self.txt_tax18c);
        insert_money(&mut fields, "frm0619F:txtTax18D", self.txt_tax18d);
        insert_money(&mut fields, "frm0619F:txtTax19", self.txt_tax19);
        insert(
            &mut fields,
            "frm0619F:txtTaxTypeCode",
            self.txt_tax_type_code.clone(),
        );
        insert(
            &mut fields,
            "frm0619F:txtTaxpayerName",
            self.taxpayer_name.clone(),
        );
        insert(
            &mut fields,
            "frm0619F:txtTelNum",
            self.contact_number.clone(),
        );
        insert(
            &mut fields,
            "frm0619F:txtYear",
            self.taxable_year.to_string(),
        );
        insert(&mut fields, "frm0619F:txtZipCode", self.zip_code.clone());
        insert(&mut fields, "txtAgency20", self.txt_agency20.clone());
        insert(&mut fields, "txtAgency21", self.txt_agency21.clone());
        insert(&mut fields, "txtAgency22", self.txt_agency22.clone());
        insert(&mut fields, "txtAgency23", self.txt_agency23.clone());
        insert(&mut fields, "txtAmount20", self.txt_amount20.clone());
        insert(&mut fields, "txtAmount21", self.txt_amount21.clone());
        insert(&mut fields, "txtAmount22", self.txt_amount22.clone());
        insert(&mut fields, "txtAmount23", self.txt_amount23.clone());
        insert(&mut fields, "txtDate20", self.txt_date20.clone());
        insert(&mut fields, "txtDate21", self.txt_date21.clone());
        insert(&mut fields, "txtDate22", self.txt_date22.clone());
        insert(&mut fields, "txtDate23", self.txt_date23.clone());
        insert(&mut fields, "txtNumber20", self.txt_number20.clone());
        insert(&mut fields, "txtNumber21", self.txt_number21.clone());
        insert(&mut fields, "txtNumber22", self.txt_number22.clone());
        insert(&mut fields, "txtNumber23", self.txt_number23.clone());
        insert(
            &mut fields,
            "txtParticular23",
            self.txt_particular23.clone(),
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
