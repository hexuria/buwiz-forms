//! Per-form templates shared across tax years.
//!
//! Keyed by `(tin, form_code)` — not by tax year. Prefill order is
//! **typed → template → profile-year**: typed/generic defaults first, template
//! overlays non-identity keys, profile-year always wins for `src-profile`
//! keys (TIN, RDO, name, address, …). Period year/quarter/month come from
//! the new filing slot, not the template.

use super::inventory::{
    FormInventorySpec, InventoryField, prefill_from_profile,
};
use super::FilingPeriod;
use crate::profile::TaxpayerProfile;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Stored template row. The taxpayer profile is never copied onto this map.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormTemplate {
    pub tin: String,
    pub form_code: String,
    pub values: BTreeMap<String, String>,
}

fn profile_source_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)TIN|BranchCode|registeredName|registeredAddress|zipCode|ZipCode|telNo|txtTel|txtEmail|txtRDOCode|RDOCode|TaxpayerName|LineofBus|txtTaxpayerName|withholdingAgent|TradeName|txtAddress|CatAgent",
        )
        .expect("src-profile pattern")
    })
}

fn period_source_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)forThe_|rtnMonth|txtYear|qtr_|txtMonth|ReturnPeriod|DueMonth|DueDay|DueYear|YearEnded",
        )
        .expect("src-period pattern")
    })
}

/// ELI5 `src-profile` classification — TIN, RDO, name, address, and the
/// other identity needles from `form_page.py`.
pub fn is_src_profile_key(key: &str) -> bool {
    profile_source_re().is_match(key)
}

pub fn is_src_period_key(key: &str) -> bool {
    period_source_re().is_match(key)
}

pub fn is_template_eligible_field(spec: &FormInventorySpec, field: &InventoryField) -> bool {
    if field.is_computed() {
        return false;
    }
    if spec
        .print_xml_keys()
        .iter()
        .any(|print_key| *print_key == field.field_key)
    {
        return false;
    }
    if is_src_profile_key(&field.field_key) || is_src_profile_key(field.xml_key()) {
        return false;
    }
    if is_src_period_key(&field.field_key) || is_src_period_key(field.xml_key()) {
        return false;
    }
    true
}

/// Drop identity, period-slot, print/XML, computed, and blank values.
pub fn filter_template_values(
    spec: &FormInventorySpec,
    values: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut kept = BTreeMap::new();
    for (key, value) in values {
        if value.trim().is_empty() {
            continue;
        }
        let Some(field) = spec.field(key) else {
            continue;
        };
        if is_template_eligible_field(spec, field) {
            kept.insert(field.field_key.clone(), value.clone());
        }
    }
    kept
}

pub fn apply_template_values(
    spec: &FormInventorySpec,
    values: &mut BTreeMap<String, String>,
    template: &BTreeMap<String, String>,
) {
    for (key, value) in filter_template_values(spec, template) {
        values.insert(key, value);
    }
}

/// Typed/generic map → template overlay → profile-year identity.
///
/// Existing saved drafts skip the template overlay so a later open does not
/// rewrite the user's edits. Profile-year identity still overwrites
/// `src-profile` keys on a **new** draft.
pub fn compose_editor_values(
    spec: &FormInventorySpec,
    mut values: BTreeMap<String, String>,
    template: Option<&BTreeMap<String, String>>,
    profile: &TaxpayerProfile,
    year: u16,
    period: &FilingPeriod,
    is_existing_saved: bool,
) -> BTreeMap<String, String> {
    if !is_existing_saved {
        if let Some(template) = template {
            apply_template_values(spec, &mut values, template);
        }
    }
    let prefill = prefill_from_profile(spec, profile, year, period);
    for (key, value) in prefill {
        if is_src_profile_key(&key) {
            values.insert(key, value);
        } else {
            values.entry(key).or_insert(value);
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::inventory::load_spec;
    use crate::naming::Tin;
    use crate::profile::TaxpayerType;

    fn profile_named(name: &str) -> TaxpayerProfile {
        TaxpayerProfile {
            id: None,
            full_name: name.into(),
            tin: Tin {
                segment1: "261".into(),
                segment2: "708".into(),
                segment3: "015".into(),
                branch: "00000".into(),
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
            profile_years: Default::default(),
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
        }
    }

    #[test]
    fn template_skips_src_profile_and_period_keys() {
        let spec = load_spec("2551Q").expect("2551Q");
        let mut source = BTreeMap::new();
        source.insert("drpATC1".into(), "PT010".into());
        source.insert("frm2551Qv2018:txtTIN1".into(), "999".into());
        source.insert("frm2551Qv2018:txtYear".into(), "2025".into());
        source.insert("frm2551Qv2018:registeredName".into(), "Q1 Name".into());
        let filtered = filter_template_values(&spec, &source);
        assert_eq!(filtered.get("drpATC1").map(String::as_str), Some("PT010"));
        assert!(!filtered.keys().any(|k| k.contains("TIN")));
        assert!(!filtered.keys().any(|k| k.contains("Year")));
        assert!(!filtered.keys().any(|k| k.contains("registeredName")));
    }

    #[test]
    fn new_quarter_gets_template_atc_and_profile_year_identity() {
        let spec = load_spec("2551Q").expect("2551Q");
        let mut template = BTreeMap::new();
        template.insert("drpATC1".into(), "PT040".into());
        template.insert("frm2551Qv2018:txtTIN1".into(), "999".into());

        let profile = profile_named("2026 clone");
        let values = compose_editor_values(
            &spec,
            BTreeMap::new(),
            Some(&template),
            &profile,
            2026,
            &FilingPeriod::Quarterly(2),
            false,
        );
        assert_eq!(values.get("drpATC1").map(String::as_str), Some("PT040"));
        assert_eq!(
            values.get("frm2551Qv2018:txtTIN1").map(String::as_str),
            Some("261")
        );
        assert_eq!(
            values
                .get("frm2551Qv2018:registeredName")
                .map(String::as_str),
            Some("2026 clone")
        );
        assert_eq!(
            values.get("frm2551Qv2018:txtYear").map(String::as_str),
            Some("2026")
        );
    }

    #[test]
    fn renaming_the_profile_year_updates_part_i_with_a_template() {
        let spec = load_spec("2551Q").expect("2551Q");
        let mut template = BTreeMap::new();
        template.insert("drpATC1".into(), "PT010".into());

        let before = compose_editor_values(
            &spec,
            BTreeMap::new(),
            Some(&template),
            &profile_named("Alpha"),
            2026,
            &FilingPeriod::Quarterly(2),
            false,
        );
        let after = compose_editor_values(
            &spec,
            BTreeMap::new(),
            Some(&template),
            &profile_named("Beta"),
            2026,
            &FilingPeriod::Quarterly(2),
            false,
        );
        assert_eq!(before.get("drpATC1").map(String::as_str), Some("PT010"));
        assert_eq!(
            before
                .get("frm2551Qv2018:registeredName")
                .map(String::as_str),
            Some("Alpha")
        );
        assert_eq!(
            after
                .get("frm2551Qv2018:registeredName")
                .map(String::as_str),
            Some("Beta")
        );
        assert_eq!(after.get("drpATC1").map(String::as_str), Some("PT010"));
    }

    #[test]
    fn no_template_leaves_typed_blank_and_still_fills_identity() {
        let spec = load_spec("2551Q").expect("2551Q");
        let values = compose_editor_values(
            &spec,
            BTreeMap::new(),
            None,
            &profile_named("Identity Only"),
            2026,
            &FilingPeriod::Quarterly(1),
            false,
        );
        assert!(!values.contains_key("drpATC1") || values.get("drpATC1").unwrap().is_empty());
        assert_eq!(
            values
                .get("frm2551Qv2018:registeredName")
                .map(String::as_str),
            Some("Identity Only")
        );
    }

    #[test]
    fn existing_saved_draft_does_not_reapply_template() {
        let spec = load_spec("2551Q").expect("2551Q");
        let mut saved = BTreeMap::new();
        saved.insert("drpATC1".into(), "PT010".into());
        let mut template = BTreeMap::new();
        template.insert("drpATC1".into(), "PT040".into());
        let values = compose_editor_values(
            &spec,
            saved,
            Some(&template),
            &profile_named("Saved"),
            2026,
            &FilingPeriod::Quarterly(2),
            true,
        );
        assert_eq!(values.get("drpATC1").map(String::as_str), Some("PT010"));
    }
}
