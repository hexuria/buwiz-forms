//! BIR field mapping for Form 0605.
//!
//! Auto-generated from savefile: 00000000000000-0605-12312025143024.xml
//! Maps Rust struct fields to BIR pseudo-XML field IDs.

use super::form_0605::Form0605Draft;

use std::collections::BTreeMap;

impl Form0605Draft {
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
        insert_bool(&mut fields, "AtcCode1", self.atc_code1);
        insert_bool(&mut fields, "AtcCode10", self.atc_code10);
        insert_bool(&mut fields, "AtcCode100", self.atc_code100);
        insert_bool(&mut fields, "AtcCode101", self.atc_code101);
        insert_bool(&mut fields, "AtcCode102", self.atc_code102);
        insert_bool(&mut fields, "AtcCode103", self.atc_code103);
        insert_bool(&mut fields, "AtcCode104", self.atc_code104);
        insert_bool(&mut fields, "AtcCode105", self.atc_code105);
        insert_bool(&mut fields, "AtcCode106", self.atc_code106);
        insert_bool(&mut fields, "AtcCode107", self.atc_code107);
        insert_bool(&mut fields, "AtcCode108", self.atc_code108);
        insert_bool(&mut fields, "AtcCode109", self.atc_code109);
        insert_bool(&mut fields, "AtcCode11", self.atc_code11);
        insert_bool(&mut fields, "AtcCode110", self.atc_code110);
        insert_bool(&mut fields, "AtcCode111", self.atc_code111);
        insert_bool(&mut fields, "AtcCode112", self.atc_code112);
        insert_bool(&mut fields, "AtcCode113", self.atc_code113);
        insert_bool(&mut fields, "AtcCode114", self.atc_code114);
        insert_bool(&mut fields, "AtcCode115", self.atc_code115);
        insert_bool(&mut fields, "AtcCode116", self.atc_code116);
        insert_bool(&mut fields, "AtcCode117", self.atc_code117);
        insert_bool(&mut fields, "AtcCode118", self.atc_code118);
        insert_bool(&mut fields, "AtcCode119", self.atc_code119);
        insert_bool(&mut fields, "AtcCode12", self.atc_code12);
        insert_bool(&mut fields, "AtcCode120", self.atc_code120);
        insert_bool(&mut fields, "AtcCode121", self.atc_code121);
        insert_bool(&mut fields, "AtcCode122", self.atc_code122);
        insert_bool(&mut fields, "AtcCode123", self.atc_code123);
        insert_bool(&mut fields, "AtcCode124", self.atc_code124);
        insert_bool(&mut fields, "AtcCode125", self.atc_code125);
        insert_bool(&mut fields, "AtcCode126", self.atc_code126);
        insert_bool(&mut fields, "AtcCode127", self.atc_code127);
        insert_bool(&mut fields, "AtcCode128", self.atc_code128);
        insert_bool(&mut fields, "AtcCode129", self.atc_code129);
        insert_bool(&mut fields, "AtcCode13", self.atc_code13);
        insert_bool(&mut fields, "AtcCode130", self.atc_code130);
        insert_bool(&mut fields, "AtcCode131", self.atc_code131);
        insert_bool(&mut fields, "AtcCode132", self.atc_code132);
        insert_bool(&mut fields, "AtcCode133", self.atc_code133);
        insert_bool(&mut fields, "AtcCode134", self.atc_code134);
        insert_bool(&mut fields, "AtcCode135", self.atc_code135);
        insert_bool(&mut fields, "AtcCode136", self.atc_code136);
        insert_bool(&mut fields, "AtcCode137", self.atc_code137);
        insert_bool(&mut fields, "AtcCode138", self.atc_code138);
        insert_bool(&mut fields, "AtcCode139", self.atc_code139);
        insert_bool(&mut fields, "AtcCode14", self.atc_code14);
        insert_bool(&mut fields, "AtcCode140", self.atc_code140);
        insert_bool(&mut fields, "AtcCode141", self.atc_code141);
        insert_bool(&mut fields, "AtcCode142", self.atc_code142);
        insert_bool(&mut fields, "AtcCode15", self.atc_code15);
        insert_bool(&mut fields, "AtcCode16", self.atc_code16);
        insert_bool(&mut fields, "AtcCode17", self.atc_code17);
        insert_bool(&mut fields, "AtcCode18", self.atc_code18);
        insert_bool(&mut fields, "AtcCode19", self.atc_code19);
        insert_bool(&mut fields, "AtcCode2", self.atc_code2);
        insert_bool(&mut fields, "AtcCode20", self.atc_code20);
        insert_bool(&mut fields, "AtcCode21", self.atc_code21);
        insert_bool(&mut fields, "AtcCode22", self.atc_code22);
        insert_bool(&mut fields, "AtcCode23", self.atc_code23);
        insert_bool(&mut fields, "AtcCode24", self.atc_code24);
        insert_bool(&mut fields, "AtcCode25", self.atc_code25);
        insert_bool(&mut fields, "AtcCode26", self.atc_code26);
        insert_bool(&mut fields, "AtcCode27", self.atc_code27);
        insert_bool(&mut fields, "AtcCode28", self.atc_code28);
        insert_bool(&mut fields, "AtcCode29", self.atc_code29);
        insert_bool(&mut fields, "AtcCode3", self.atc_code3);
        insert_bool(&mut fields, "AtcCode30", self.atc_code30);
        insert_bool(&mut fields, "AtcCode31", self.atc_code31);
        insert_bool(&mut fields, "AtcCode32", self.atc_code32);
        insert_bool(&mut fields, "AtcCode33", self.atc_code33);
        insert_bool(&mut fields, "AtcCode34", self.atc_code34);
        insert_bool(&mut fields, "AtcCode35", self.atc_code35);
        insert_bool(&mut fields, "AtcCode36", self.atc_code36);
        insert_bool(&mut fields, "AtcCode37", self.atc_code37);
        insert_bool(&mut fields, "AtcCode38", self.atc_code38);
        insert_bool(&mut fields, "AtcCode39", self.atc_code39);
        insert_bool(&mut fields, "AtcCode4", self.atc_code4);
        insert_bool(&mut fields, "AtcCode40", self.atc_code40);
        insert_bool(&mut fields, "AtcCode41", self.atc_code41);
        insert_bool(&mut fields, "AtcCode42", self.atc_code42);
        insert_bool(&mut fields, "AtcCode43", self.atc_code43);
        insert_bool(&mut fields, "AtcCode44", self.atc_code44);
        insert_bool(&mut fields, "AtcCode45", self.atc_code45);
        insert_bool(&mut fields, "AtcCode46", self.atc_code46);
        insert_bool(&mut fields, "AtcCode47", self.atc_code47);
        insert_bool(&mut fields, "AtcCode48", self.atc_code48);
        insert_bool(&mut fields, "AtcCode49", self.atc_code49);
        insert_bool(&mut fields, "AtcCode5", self.atc_code5);
        insert_bool(&mut fields, "AtcCode50", self.atc_code50);
        insert_bool(&mut fields, "AtcCode51", self.atc_code51);
        insert_bool(&mut fields, "AtcCode52", self.atc_code52);
        insert_bool(&mut fields, "AtcCode53", self.atc_code53);
        insert_bool(&mut fields, "AtcCode54", self.atc_code54);
        insert_bool(&mut fields, "AtcCode55", self.atc_code55);
        insert_bool(&mut fields, "AtcCode56", self.atc_code56);
        insert_bool(&mut fields, "AtcCode57", self.atc_code57);
        insert_bool(&mut fields, "AtcCode58", self.atc_code58);
        insert_bool(&mut fields, "AtcCode59", self.atc_code59);
        insert_bool(&mut fields, "AtcCode6", self.atc_code6);
        insert_bool(&mut fields, "AtcCode60", self.atc_code60);
        insert_bool(&mut fields, "AtcCode61", self.atc_code61);
        insert_bool(&mut fields, "AtcCode62", self.atc_code62);
        insert_bool(&mut fields, "AtcCode63", self.atc_code63);
        insert_bool(&mut fields, "AtcCode64", self.atc_code64);
        insert_bool(&mut fields, "AtcCode65", self.atc_code65);
        insert_bool(&mut fields, "AtcCode66", self.atc_code66);
        insert_bool(&mut fields, "AtcCode67", self.atc_code67);
        insert_bool(&mut fields, "AtcCode68", self.atc_code68);
        insert_bool(&mut fields, "AtcCode69", self.atc_code69);
        insert_bool(&mut fields, "AtcCode7", self.atc_code7);
        insert_bool(&mut fields, "AtcCode70", self.atc_code70);
        insert_bool(&mut fields, "AtcCode71", self.atc_code71);
        insert_bool(&mut fields, "AtcCode72", self.atc_code72);
        insert_bool(&mut fields, "AtcCode73", self.atc_code73);
        insert_bool(&mut fields, "AtcCode74", self.atc_code74);
        insert_bool(&mut fields, "AtcCode75", self.atc_code75);
        insert_bool(&mut fields, "AtcCode76", self.atc_code76);
        insert_bool(&mut fields, "AtcCode77", self.atc_code77);
        insert_bool(&mut fields, "AtcCode78", self.atc_code78);
        insert_bool(&mut fields, "AtcCode79", self.atc_code79);
        insert_bool(&mut fields, "AtcCode8", self.atc_code8);
        insert_bool(&mut fields, "AtcCode80", self.atc_code80);
        insert_bool(&mut fields, "AtcCode81", self.atc_code81);
        insert_bool(&mut fields, "AtcCode82", self.atc_code82);
        insert_bool(&mut fields, "AtcCode83", self.atc_code83);
        insert_bool(&mut fields, "AtcCode84", self.atc_code84);
        insert_bool(&mut fields, "AtcCode85", self.atc_code85);
        insert_bool(&mut fields, "AtcCode86", self.atc_code86);
        insert_bool(&mut fields, "AtcCode87", self.atc_code87);
        insert_bool(&mut fields, "AtcCode88", self.atc_code88);
        insert_bool(&mut fields, "AtcCode89", self.atc_code89);
        insert_bool(&mut fields, "AtcCode9", self.atc_code9);
        insert_bool(&mut fields, "AtcCode90", self.atc_code90);
        insert_bool(&mut fields, "AtcCode91", self.atc_code91);
        insert_bool(&mut fields, "AtcCode92", self.atc_code92);
        insert_bool(&mut fields, "AtcCode93", self.atc_code93);
        insert_bool(&mut fields, "AtcCode94", self.atc_code94);
        insert_bool(&mut fields, "AtcCode95", self.atc_code95);
        insert_bool(&mut fields, "AtcCode96", self.atc_code96);
        insert_bool(&mut fields, "AtcCode97", self.atc_code97);
        insert_bool(&mut fields, "AtcCode98", self.atc_code98);
        insert_bool(&mut fields, "AtcCode99", self.atc_code99);
        insert_bool(&mut fields, "TaxTypeCode1", self.tax_type_code1);
        insert_bool(&mut fields, "TaxTypeCode10", self.tax_type_code10);
        insert_bool(&mut fields, "TaxTypeCode11", self.tax_type_code11);
        insert_bool(&mut fields, "TaxTypeCode12", self.tax_type_code12);
        insert_bool(&mut fields, "TaxTypeCode13", self.tax_type_code13);
        insert_bool(&mut fields, "TaxTypeCode14", self.tax_type_code14);
        insert_bool(&mut fields, "TaxTypeCode15", self.tax_type_code15);
        insert_bool(&mut fields, "TaxTypeCode16", self.tax_type_code16);
        insert_bool(&mut fields, "TaxTypeCode17", self.tax_type_code17);
        insert_bool(&mut fields, "TaxTypeCode18", self.tax_type_code18);
        insert_bool(&mut fields, "TaxTypeCode19", self.tax_type_code19);
        insert_bool(&mut fields, "TaxTypeCode2", self.tax_type_code2);
        insert_bool(&mut fields, "TaxTypeCode20", self.tax_type_code20);
        insert_bool(&mut fields, "TaxTypeCode21", self.tax_type_code21);
        insert_bool(&mut fields, "TaxTypeCode22", self.tax_type_code22);
        insert_bool(&mut fields, "TaxTypeCode23", self.tax_type_code23);
        insert_bool(&mut fields, "TaxTypeCode24", self.tax_type_code24);
        insert_bool(&mut fields, "TaxTypeCode25", self.tax_type_code25);
        insert_bool(&mut fields, "TaxTypeCode26", self.tax_type_code26);
        insert_bool(&mut fields, "TaxTypeCode27", self.tax_type_code27);
        insert_bool(&mut fields, "TaxTypeCode28", self.tax_type_code28);
        insert_bool(&mut fields, "TaxTypeCode29", self.tax_type_code29);
        insert_bool(&mut fields, "TaxTypeCode3", self.tax_type_code3);
        insert_bool(&mut fields, "TaxTypeCode30", self.tax_type_code30);
        insert_bool(&mut fields, "TaxTypeCode31", self.tax_type_code31);
        insert_bool(&mut fields, "TaxTypeCode32", self.tax_type_code32);
        insert_bool(&mut fields, "TaxTypeCode33", self.tax_type_code33);
        insert_bool(&mut fields, "TaxTypeCode34", self.tax_type_code34);
        insert_bool(&mut fields, "TaxTypeCode35", self.tax_type_code35);
        insert_bool(&mut fields, "TaxTypeCode36", self.tax_type_code36);
        insert_bool(&mut fields, "TaxTypeCode37", self.tax_type_code37);
        insert_bool(&mut fields, "TaxTypeCode4", self.tax_type_code4);
        insert_bool(&mut fields, "TaxTypeCode5", self.tax_type_code5);
        insert_bool(&mut fields, "TaxTypeCode6", self.tax_type_code6);
        insert_bool(&mut fields, "TaxTypeCode7", self.tax_type_code7);
        insert_bool(&mut fields, "TaxTypeCode8", self.tax_type_code8);
        insert_bool(&mut fields, "TaxTypeCode9", self.tax_type_code9);
        insert_bool(
            &mut fields,
            "frm0605:itemApprovedYN:_1",
            self.item_approved_yn_1,
        );
        insert_bool(
            &mut fields,
            "frm0605:itemApprovedYN:_2",
            self.item_approved_yn_2,
        );
        insert(
            &mut fields,
            "frm0605:itemFiscalStartMonth:_1",
            format!("{:02}", self.month),
        );
        insert(
            &mut fields,
            "frm0605:itemFiscalStartMonth:_2",
            format!("{:02}", self.month),
        );
        insert_bool(
            &mut fields,
            "frm0605:itemMannerOfPayment:_1",
            self.item_manner_of_payment_1,
        );
        insert_bool(
            &mut fields,
            "frm0605:itemMannerOfPayment:_2",
            self.item_manner_of_payment_2,
        );
        insert_bool(
            &mut fields,
            "frm0605:itemMannerOfPayment:_3",
            self.item_manner_of_payment_3,
        );
        insert_bool(
            &mut fields,
            "frm0605:itemMannerOfPayment:_4",
            self.item_manner_of_payment_4,
        );
        insert_bool(
            &mut fields,
            "frm0605:itemMannerOfPayment:_5",
            self.item_manner_of_payment_5,
        );
        insert_bool(
            &mut fields,
            "frm0605:itemMannerOfPaymentB:_1",
            self.item_manner_of_payment_b_1,
        );
        insert_bool(
            &mut fields,
            "frm0605:itemMannerOfPaymentB:_2",
            self.item_manner_of_payment_b_2,
        );
        insert_bool(
            &mut fields,
            "frm0605:itemModeOfPayment:_1",
            self.item_mode_of_payment_1,
        );
        insert_bool(
            &mut fields,
            "frm0605:itemModeOfPayment:_2",
            self.item_mode_of_payment_2,
        );
        insert_bool(
            &mut fields,
            "frm0605:itemModeOfPayment:_3",
            self.item_mode_of_payment_3,
        );
        insert(
            &mut fields,
            "frm0605:itemYearEndMonth",
            format!("{:02}", self.month),
        );
        insert(&mut fields, "frm0605:txtAddress", self.txt_address.clone());
        insert(&mut fields, "frm0605:txtBranchCode", branch.clone());
        insert_bool(
            &mut fields,
            "frm0605:txtClassification:_1",
            self.txt_classification_1,
        );
        insert_bool(
            &mut fields,
            "frm0605:txtClassification:_2",
            self.txt_classification_2,
        );
        insert(
            &mut fields,
            "frm0605:txtDueDateDay",
            self.txt_due_date_day.to_string(),
        );
        insert(
            &mut fields,
            "frm0605:txtDueDateMonth",
            format!("{:02}", self.month),
        );
        insert(
            &mut fields,
            "frm0605:txtDueDateYear",
            self.taxable_year.to_string(),
        );
        insert(&mut fields, "frm0605:txtLineBus", self.txt_line_bus.clone());
        insert(
            &mut fields,
            "frm0605:txtNoOfSheets",
            self.txt_no_of_sheets.to_string(),
        );
        insert(
            &mut fields,
            "frm0605:txtNumOfInstallment",
            self.txt_num_of_installment.to_string(),
        );
        insert(
            &mut fields,
            "frm0605:txtOthersName",
            self.txt_others_name.clone(),
        );
        insert(&mut fields, "frm0605:txtRDOCode", self.rdo_code.clone());
        insert(
            &mut fields,
            "frm0605:txtReturnPeriodDay",
            self.txt_return_period_day.to_string(),
        );
        insert(
            &mut fields,
            "frm0605:txtReturnPeriodMonth",
            format!("{:02}", self.month),
        );
        insert(
            &mut fields,
            "frm0605:txtReturnPeriodYear",
            self.taxable_year.to_string(),
        );
        insert(&mut fields, "frm0605:txtTIN1", tin1.clone());
        insert(&mut fields, "frm0605:txtTIN2", tin2.clone());
        insert(&mut fields, "frm0605:txtTIN3", tin3.clone());
        insert_money(&mut fields, "frm0605:txtTax19", self.txt_tax19);
        insert_money(&mut fields, "frm0605:txtTax20A", self.txt_tax20a);
        insert_money(&mut fields, "frm0605:txtTax20B", self.txt_tax20b);
        insert_money(&mut fields, "frm0605:txtTax20C", self.txt_tax20c);
        insert_money(&mut fields, "frm0605:txtTax20D", self.txt_tax20d);
        insert_money(&mut fields, "frm0605:txtTax21", self.txt_tax21);
        insert(
            &mut fields,
            "frm0605:txtTaxPayerName",
            self.taxpayer_name.clone(),
        );
        insert(
            &mut fields,
            "frm0605:txtTelNum",
            self.contact_number.clone(),
        );
        insert(
            &mut fields,
            "frm0605:txtYearEnded",
            self.taxable_year.to_string(),
        );
        insert(&mut fields, "frm0605:txtZipCode", self.zip_code.clone());
        // Quarter radio buttons — derive from month
        let q = ((self.month.saturating_sub(1)) / 3) + 1;
        insert_bool(&mut fields, "itemQuarter_1", q == 1);
        insert_bool(&mut fields, "itemQuarter_2", q == 2);
        insert_bool(&mut fields, "itemQuarter_3", q == 3);
        insert_bool(&mut fields, "itemQuarter_4", q == 4);
        insert(&mut fields, "txtATCCode", self.txt_atccode.clone());
        insert(
            &mut fields,
            "txtTaxTypeCode",
            self.txt_tax_type_code.clone(),
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
