use super::form_2551q::Form2551QDraft;
use chrono::Local;
use std::collections::BTreeMap;

impl Form2551QDraft {
    pub fn to_bir_xml_payload(&self) -> String {
        let mut fields = BTreeMap::new();

        let tin_parts: Vec<&str> = self.tin.split('-').collect();
        let tin1 = tin_parts.first().copied().unwrap_or(self.tin.as_str());
        let tin2 = tin_parts.get(1).copied().unwrap_or("");
        let tin3 = tin_parts.get(2).copied().unwrap_or("");
        let branch = tin_parts.get(3).copied().unwrap_or("000");

        let insert = |map: &mut BTreeMap<String, String>, k: &str, v: String| {
            map.insert(k.to_string(), v);
        };
        let insert_bool = |map: &mut BTreeMap<String, String>, k: &str, v: bool| {
            map.insert(k.to_string(), if v { "true" } else { "false" }.to_string());
        };

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

        insert(&mut fields, "frm2551Qv2018:txtTIN1", tin1.to_string());
        insert(&mut fields, "frm2551Qv2018:txtTIN2", tin2.to_string());
        insert(&mut fields, "frm2551Qv2018:txtTIN3", tin3.to_string());
        insert(
            &mut fields,
            "frm2551Qv2018:txtBranchCode",
            branch.to_string(),
        );
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

        insert(
            &mut fields,
            "frm2551Qv2018:txt14",
            format!("{:.2}", self.total_tax_due),
        );
        insert(
            &mut fields,
            "frm2551Qv2018:txt15",
            format!("{:.2}", self.creditable_tax_withheld),
        );
        insert(
            &mut fields,
            "frm2551Qv2018:txt16",
            format!("{:.2}", self.tax_paid_previous),
        );
        insert(
            &mut fields,
            "frm2551Qv2018:txt24",
            format!("{:.2}", self.tax_payable),
        );
        insert(
            &mut fields,
            "txtTotalSched1",
            format!("{:.2}", self.total_tax_due),
        );

        for i in 0..6 {
            let row = self.schedule_1.get(i);
            let atc = row.map(|r| r.atc.as_str()).unwrap_or("0");
            let atc_amt = row
                .map(|r| format!("{:.2}", r.taxable_amount))
                .unwrap_or("0.00".to_string());
            let atc_rate = row
                .map(|r| format!("{:.1}", r.tax_rate * 100.0))
                .unwrap_or("0.00".to_string());
            let atc_due = row
                .map(|r| format!("{:.2}", r.tax_due))
                .unwrap_or("0.00".to_string());

            insert(&mut fields, &format!("drpATC{}", i + 1), atc.to_string());
            insert(&mut fields, &format!("txtATCAmt{}", i + 1), atc_amt);
            insert(&mut fields, &format!("txtATCRate{}", i + 1), atc_rate);
            insert(&mut fields, &format!("txtATCDue{}", i + 1), atc_due);
        }

        let now = Local::now();
        let dynamic_date = now.format("%m/%d/%Y %H:%M:%S").to_string();
        insert(&mut fields, "txtDateIssue", dynamic_date);

        crate::bir_xml::generate_bir_xml(&fields)
    }
}
