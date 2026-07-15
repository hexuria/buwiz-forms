use super::form_2551q::{Form2551QDraft, Item13Election, OverpaymentDisposition, TaxPeriodBasis};
use super::{AtcRateResolution, resolve_2551q_atc_rate};
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

        insert_bool(
            &mut fields,
            "frm2551Qv2018:forThe_1",
            matches!(self.tax_period_basis, TaxPeriodBasis::Calendar),
        );
        insert_bool(
            &mut fields,
            "frm2551Qv2018:forThe_2",
            matches!(self.tax_period_basis, TaxPeriodBasis::Fiscal),
        );
        insert(
            &mut fields,
            "frm2551Qv2018:rtnMonth",
            self.year_end_month.to_string(),
        );
        insert(
            &mut fields,
            "__year_ended",
            format!("{:02}{}", self.year_end_month, self.taxable_year),
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
        insert(
            &mut fields,
            "frm2551Qv2018:txtTaxReliefSpecify",
            if self.tax_relief {
                self.tax_relief_specification.clone()
            } else {
                String::new()
            },
        );
        insert_bool(
            &mut fields,
            "frm2551Qv2018:taxRate1",
            matches!(self.item_13_election, Item13Election::Graduated),
        );
        insert_bool(
            &mut fields,
            "frm2551Qv2018:taxRate2",
            matches!(self.item_13_election, Item13Election::EightPercent),
        );
        insert(
            &mut fields,
            "frm2551Qv2018:txtSheets",
            self.number_of_attached_sheets.to_string(),
        );

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
        insert_money(
            &mut fields,
            "frm2551Qv2018:txt16",
            if self.is_amended {
                self.tax_paid_previous
            } else {
                0.0
            },
        );
        insert(
            &mut fields,
            "frm2551Qv2018:txt17Specify",
            self.other_tax_credit_description.clone(),
        );
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
        insert_bool(
            &mut fields,
            "frm2551Qv2018:overPayment1",
            matches!(self.overpayment_disposition, OverpaymentDisposition::Refund),
        );
        insert_bool(
            &mut fields,
            "frm2551Qv2018:overPayment2",
            matches!(
                self.overpayment_disposition,
                OverpaymentDisposition::TaxCreditCertificate
            ),
        );

        for i in 0..6 {
            let row = self.schedule_1.get(i);
            let atc = row.map(|r| r.atc.as_str()).unwrap_or("0");
            let atc_amt = row
                .map(|r| format_money(r.taxable_amount))
                .unwrap_or_else(|| "0.00".to_string());
            let atc_rate = row
                .map(|r| {
                    // Validation permits only a negligible float tolerance,
                    // but XML must still emit the exact registry-owned rate.
                    let canonical_rate = match resolve_2551q_atc_rate(
                        r.atc.trim(),
                        self.taxable_year,
                        self.quarter,
                        self.year_end_month,
                    ) {
                        Some(AtcRateResolution::Single(rate)) => rate,
                        // Unknown ATCs and split periods are rejected by
                        // `to_bir_xml_payload` validation. Keep the persisted
                        // value here so this infallible field-map helper never
                        // invents a different rate for an invalid draft.
                        _ => r.tax_rate,
                    };
                    format_rate_percent(canonical_rate)
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

        // This field is legal tax-agent metadata, not the XML render time.
        // Keep it blank until the draft owns an explicit issue date.
        insert(&mut fields, "txtDateIssue", "");

        fields
    }

    pub fn to_bir_xml_payload(&self) -> Result<String, Vec<(String, String)>> {
        let errors = <Self as super::FormValidator>::validate(self);
        if errors.is_empty() {
            Ok(crate::bir_xml::generate_bir_xml(&self.to_bir_field_map()))
        } else {
            Err(errors)
        }
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

fn format_rate_percent(rate: f64) -> String {
    let mut value = format!("{:.8}", rate * 100.0);
    while value.contains('.') && value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::ATC_TABLE_2551Q;
    use crate::forms::form_2551q::Schedule1Row;
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
            business_start_date: chrono::NaiveDate::from_ymd_opt(2010, 1, 1),
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
        draft.item_13_election = Item13Election::Graduated;
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
        assert_eq!(fields["frm2551Qv2018:forThe_1"], "true");
        assert_eq!(fields["frm2551Qv2018:forThe_2"], "false");
        assert_eq!(fields["frm2551Qv2018:rtnMonth"], "12");
        assert_eq!(fields["__year_ended"], "122026");
        assert_eq!(fields["frm2551Qv2018:txtSheets"], "0");

        // This Individual Q1 PT010 return explicitly selects the graduated
        // election; no overpayment disposition is selected for a positive balance.
        assert_eq!(fields["frm2551Qv2018:taxRate1"], "true");
        assert_eq!(fields["frm2551Qv2018:taxRate2"], "false");
        assert_eq!(fields["frm2551Qv2018:overPayment1"], "false");
        assert_eq!(fields["frm2551Qv2018:overPayment2"], "false");
        assert_eq!(fields["txtDateIssue"], "");
    }

    #[test]
    fn field_map_serializes_explicit_filing_and_legal_values() {
        let mut draft = sample_draft();
        draft.tax_period_basis = TaxPeriodBasis::Fiscal;
        draft.year_end_month = 6;
        draft.number_of_attached_sheets = 3;
        draft.tax_relief = true;
        draft.tax_relief_specification = "Special Law 123".to_string();
        draft.item_13_election = Item13Election::EightPercent;
        draft.other_tax_credit_description = "Prior quarter adjustment".to_string();
        draft.overpayment_disposition = OverpaymentDisposition::Refund;

        let fields = draft.to_bir_field_map();

        assert_eq!(fields["frm2551Qv2018:forThe_1"], "false");
        assert_eq!(fields["frm2551Qv2018:forThe_2"], "true");
        assert_eq!(fields["frm2551Qv2018:rtnMonth"], "6");
        assert_eq!(fields["__year_ended"], "062026");
        assert_eq!(fields["frm2551Qv2018:txtSheets"], "3");
        assert_eq!(fields["frm2551Qv2018:taxTreaty_1"], "true");
        assert_eq!(fields["frm2551Qv2018:taxTreaty_2"], "false");
        assert_eq!(
            fields["frm2551Qv2018:txtTaxReliefSpecify"],
            "Special Law 123"
        );
        assert_eq!(fields["frm2551Qv2018:taxRate1"], "false");
        assert_eq!(fields["frm2551Qv2018:taxRate2"], "true");
        assert_eq!(
            fields["frm2551Qv2018:txt17Specify"],
            "Prior quarter adjustment"
        );
        assert_eq!(fields["frm2551Qv2018:overPayment1"], "true");
        assert_eq!(fields["frm2551Qv2018:overPayment2"], "false");
    }

    #[test]
    fn field_map_maps_each_single_choice_without_overlap() {
        let mut draft = sample_draft();
        draft.item_13_election = Item13Election::Graduated;
        draft.overpayment_disposition = OverpaymentDisposition::TaxCreditCertificate;

        let fields = draft.to_bir_field_map();

        assert_eq!(fields["frm2551Qv2018:taxRate1"], "true");
        assert_eq!(fields["frm2551Qv2018:taxRate2"], "false");
        assert_eq!(fields["frm2551Qv2018:overPayment1"], "false");
        assert_eq!(fields["frm2551Qv2018:overPayment2"], "true");
    }

    #[test]
    fn field_map_never_prints_item_16_on_a_non_amended_return() {
        let mut draft = sample_draft();
        draft.is_amended = false;
        draft.tax_paid_previous = 500.0;
        assert_eq!(draft.to_bir_field_map()["frm2551Qv2018:txt16"], "0.00");

        draft.is_amended = true;
        assert_eq!(draft.to_bir_field_map()["frm2551Qv2018:txt16"], "500.00");
    }

    #[test]
    fn xml_export_uses_canonical_field_map() {
        let xml = sample_draft()
            .to_bir_xml_payload()
            .expect("the valid sample draft should serialize");

        assert!(xml.contains("frm2551Qv2018:txt18="));
        assert!(xml.contains("frm2551Qv2018:txtPg2TaxpayerName="));
        assert!(xml.contains("txtTotalSched1="));
    }

    #[test]
    fn xml_export_rejects_stale_or_tampered_derived_amounts() {
        for field in [
            "total_tax_due",
            "total_tax_credits",
            "tax_payable",
            "total_penalties",
            "total_amount_payable",
        ] {
            let mut draft = sample_draft();
            match field {
                "total_tax_due" => draft.total_tax_due += 1.0,
                "total_tax_credits" => draft.total_tax_credits += 1.0,
                "tax_payable" => draft.tax_payable += 1.0,
                "total_penalties" => draft.total_penalties += 1.0,
                "total_amount_payable" => draft.total_amount_payable += 1.0,
                _ => unreachable!(),
            }

            let errors = draft
                .to_bir_xml_payload()
                .expect_err("inconsistent derived amounts must not produce XML");
            assert!(
                errors.iter().any(|(error_field, _)| error_field == field),
                "expected {field} invariant error; got {errors:?}"
            );
        }
    }

    #[test]
    fn field_map_emits_the_registry_owned_atc_rate() {
        let mut draft = sample_draft();
        draft.schedule_1[0].tax_rate = 0.030_000_000_000_5;

        assert_eq!(draft.to_bir_field_map()["txtATCRate1"], "3");
        draft
            .to_bir_xml_payload()
            .expect("a tolerated binary float delta must serialize canonically");
    }

    #[test]
    fn every_registry_rate_serializes_as_a_canonical_decimal() {
        for entry in ATC_TABLE_2551Q {
            let mut draft = sample_draft();
            let mut row = Schedule1Row::new(entry.code).expect("registry entry must resolve");
            row.taxable_amount = 10_000.0;
            draft.schedule_1 = vec![row];
            draft.item_13_election = if entry.code == "PT010" {
                Item13Election::Graduated
            } else {
                Item13Election::NotApplicable
            };
            draft.recompute(None);

            let fields = draft.to_bir_field_map();
            assert_eq!(
                fields["txtATCRate1"],
                format!("{:.0}", entry.rate * 100.0),
                "{} must not expose a binary floating-point artifact",
                entry.code
            );
            draft.to_bir_xml_payload().unwrap_or_else(|errors| {
                panic!("{} failed XML validation: {errors:?}", entry.code)
            });
        }
    }

    #[test]
    fn field_map_emits_the_period_owned_temporary_pt010_rate() {
        let mut draft = sample_draft();
        draft.taxable_year = 2021;
        draft.quarter = 3;
        draft.item_13_election = Item13Election::NotApplicable;
        draft.recompute(None);

        assert_eq!(draft.schedule_1[0].tax_rate, 0.01);
        assert_eq!(draft.to_bir_field_map()["txtATCRate1"], "1");
        draft
            .to_bir_xml_payload()
            .expect("the temporary statutory rate must serialize canonically");
    }

    #[test]
    fn xml_export_enforces_item_13_applicability_before_serializing_boxes() {
        let mut draft = sample_draft();
        draft.quarter = 2;

        let errors = draft
            .to_bir_xml_payload()
            .expect_err("a later-quarter graduated election must not serialize");
        assert!(errors.iter().any(|(field, _)| field == "item_13_election"));

        draft.item_13_election = Item13Election::NotApplicable;
        let fields = draft.to_bir_field_map();
        assert_eq!(fields["frm2551Qv2018:taxRate1"], "false");
        assert_eq!(fields["frm2551Qv2018:taxRate2"], "false");
        draft.recompute(None);
        draft
            .to_bir_xml_payload()
            .expect("a later-quarter NotApplicable election should serialize");
    }

    #[test]
    fn xml_export_rejects_negative_or_non_finite_manual_penalties() {
        let cases = [
            ("surcharge", -1.0),
            ("interest", -1.0),
            ("compromise", -1.0),
            ("surcharge", f64::NAN),
            ("interest", f64::INFINITY),
            ("compromise", f64::NEG_INFINITY),
        ];

        for (field, value) in cases {
            let mut draft = sample_draft();
            draft.auto_compute_penalties = false;
            match field {
                "surcharge" => draft.surcharge = value,
                "interest" => draft.interest = value,
                "compromise" => draft.compromise = value,
                _ => unreachable!("test case uses a known penalty field"),
            }

            let errors = draft
                .to_bir_xml_payload()
                .expect_err("invalid manual penalties must not produce XML");

            assert!(
                errors.iter().any(|(error_field, _)| error_field == field),
                "expected {field} to be rejected for {value:?}; got {errors:?}"
            );
        }
    }

    #[test]
    fn xml_export_rejects_a_coherently_tampered_automatic_penalty_chain() {
        let mut draft = sample_draft();
        draft.surcharge += 100.0;
        draft.total_penalties += 100.0;
        draft.total_amount_payable += 100.0;

        let errors = draft
            .to_bir_xml_payload()
            .expect_err("automatic penalties must be recalculated at the XML boundary");
        assert!(errors.iter().any(|(field, message)| {
            field == "surcharge" && message.contains("automatic surcharge")
        }));
    }
}
