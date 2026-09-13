//! Inventory-driven form editor spec.
//!
//! Field lists come from `rules/forms/*/fields.json`. UX section grouping comes
//! from the ELI5 sidecar (`*.sections.json`) so Rust does not re-implement
//! Python bucketing. Queue / XML / live BIR submit stay behind
//! [`super::can_queue_for_submission`].

use super::inventory_catalog::{INVENTORY_BUNDLES, InventoryBundle};
use super::{FilingFrequency, FilingPeriod, FilingStatus, find_form};
use crate::profile::TaxpayerProfile;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// JSON/map-backed draft used when no typed model exists, and as the editor
/// payload overlay for typed forms that still persist through dedicated paths.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenericFormDraft {
    pub id: Option<i64>,
    pub form_code: String,
    #[serde(default)]
    pub form_revision: String,
    pub tin: String,
    pub taxable_year: u16,
    pub period: FilingPeriod,
    #[serde(default)]
    pub status: FilingStatus,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    /// Distinguishes inventory JSON from legacy typed draft blobs in `data_json`.
    #[serde(default)]
    pub inventory_editor: bool,
}

impl GenericFormDraft {
    pub fn new(form_code: &str, tin: &str, year: u16, period: FilingPeriod) -> Self {
        let revision = load_spec(form_code)
            .map(|spec| spec.revision.clone())
            .unwrap_or_default();
        Self {
            id: None,
            form_code: form_code.to_string(),
            form_revision: revision,
            tin: tin.to_string(),
            taxable_year: year,
            period,
            status: FilingStatus::Draft,
            values: BTreeMap::new(),
            inventory_editor: true,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct FieldsFile {
    #[serde(default)]
    form_id: String,
    #[serde(default)]
    revision: String,
    field_count: usize,
    fields: Vec<InventoryField>,
}

/// One inventory field from `fields.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct InventoryField {
    pub field_key: String,
    #[serde(default)]
    pub serialized_key: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub page: Option<serde_json::Value>,
    #[serde(default)]
    pub item_number: Option<serde_json::Value>,
    #[serde(default)]
    pub control_kind: String,
    #[serde(default)]
    pub required: Option<String>,
    #[serde(default)]
    pub computed: bool,
    /// Choice tokens. `fields.json` stores strings, `{value,label}` objects,
    /// or other tagged maps; all collapse to the stored value.
    #[serde(default, deserialize_with = "deserialize_enum_values")]
    pub enum_values: Vec<String>,
    #[serde(default)]
    pub default_value: Option<serde_json::Value>,
}

impl InventoryField {
    /// Key written to the BIR field map / XML (occurrence suffixes stripped
    /// from `field_key` when `serialized_key` is absent).
    pub fn xml_key(&self) -> &str {
        self.serialized_key
            .as_deref()
            .filter(|key| !key.is_empty())
            .unwrap_or(self.field_key.as_str())
    }

    pub fn is_required(&self) -> bool {
        self.required.as_deref() == Some("required")
    }

    pub fn is_computed(&self) -> bool {
        self.computed || self.required.as_deref() == Some("computed")
    }

    pub fn display_label(&self) -> String {
        clean_label(self.label.as_deref(), &self.field_key)
    }

    pub fn is_select(&self) -> bool {
        matches!(
            self.control_kind.as_str(),
            "select" | "select-one" | "runtime-generated-select" | "runtime-indexed-select"
        )
    }

    pub fn is_choice_control(&self) -> bool {
        matches!(
            self.control_kind.as_str(),
            "radio"
                | "checkbox"
                | "checkbox/radio"
                | "runtime-checkbox"
                | "runtime-indexed-checkbox"
        ) || (self.is_select() && !self.enum_values.is_empty())
    }

    pub fn is_text_input(&self) -> bool {
        !self.is_computed()
            && !self.is_choice_control()
            && matches!(
                self.control_kind.as_str(),
                "text"
                    | "select"
                    | "select-one"
                    | "runtime-text"
                    | "runtime-indexed-text"
                    | "runtime-generated-select"
                    | "runtime-indexed-select"
                    | "serialized-runtime-control"
                    | "runtime-injected-control"
            )
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct SectionsFile {
    form_code: String,
    #[serde(default)]
    rules_id: String,
    #[serde(default)]
    frozen_bundle: String,
    field_count: usize,
    sections: Vec<InventorySection>,
}

/// One UX section from the ELI5 sidecar.
#[derive(Debug, Clone, Deserialize)]
pub struct InventorySection {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub editor_hidden: bool,
    pub fields: Vec<String>,
}

/// Parsed fields.json + sidecar for one form code.
#[derive(Debug, Clone)]
pub struct FormInventorySpec {
    pub form_code: String,
    pub rules_id: String,
    pub frozen_bundle: String,
    pub revision: String,
    pub field_count: usize,
    pub fields: Vec<InventoryField>,
    pub sections: Vec<InventorySection>,
    field_index: BTreeMap<String, usize>,
}

impl FormInventorySpec {
    pub fn field(&self, key: &str) -> Option<&InventoryField> {
        self.field_index
            .get(key)
            .and_then(|idx| self.fields.get(*idx))
    }

    pub fn editor_sections(&self) -> impl Iterator<Item = &InventorySection> {
        self.sections.iter().filter(|section| !section.editor_hidden)
    }

    pub fn print_xml_section(&self) -> Option<&InventorySection> {
        self.sections.iter().find(|section| section.id == "print_xml")
    }

    pub fn print_xml_keys(&self) -> Vec<&str> {
        self.print_xml_section()
            .map(|section| section.fields.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    pub fn editor_field_keys(&self) -> Vec<&str> {
        self.editor_sections()
            .flat_map(|section| section.fields.iter().map(String::as_str))
            .collect()
    }

    /// Every `fields.json` key appears in exactly one sidecar section.
    pub fn bound_keys_once(&self) -> Result<BTreeSet<String>, String> {
        let mut seen = BTreeSet::new();
        for section in &self.sections {
            for key in &section.fields {
                if !seen.insert(key.clone()) {
                    return Err(format!("duplicate sidecar key {key}"));
                }
            }
        }
        let inventory: BTreeSet<String> = self.fields.iter().map(|f| f.field_key.clone()).collect();
        if seen != inventory {
            let missing: Vec<_> = inventory.difference(&seen).cloned().collect();
            let extra: Vec<_> = seen.difference(&inventory).cloned().collect();
            return Err(format!(
                "sidecar/inventory mismatch missing={missing:?} extra={extra:?}"
            ));
        }
        if seen.len() != self.field_count {
            return Err(format!(
                "bound {} keys, field_count {}",
                seen.len(),
                self.field_count
            ));
        }
        Ok(seen)
    }
}

pub fn inventory_bundle(form_code: &str) -> Option<&'static InventoryBundle> {
    INVENTORY_BUNDLES
        .iter()
        .find(|bundle| bundle.code.eq_ignore_ascii_case(form_code))
}

pub fn has_inventory(form_code: &str) -> bool {
    inventory_bundle(form_code).is_some()
}

pub fn inventory_codes() -> impl Iterator<Item = &'static str> {
    INVENTORY_BUNDLES.iter().map(|bundle| bundle.code)
}

pub fn load_spec(form_code: &str) -> Result<FormInventorySpec, String> {
    let bundle = inventory_bundle(form_code).ok_or_else(|| {
        format!("no inventory sidecar for form `{form_code}`")
    })?;
    parse_spec(bundle)
}

fn parse_spec(bundle: &InventoryBundle) -> Result<FormInventorySpec, String> {
    let fields_file: FieldsFile = serde_json::from_str(bundle.fields_json)
        .map_err(|err| format!("{} fields.json: {err}", bundle.code))?;
    let sections_file: SectionsFile = serde_json::from_str(bundle.sections_json)
        .map_err(|err| format!("{} sections.json: {err}", bundle.code))?;
    if fields_file.field_count != fields_file.fields.len() {
        return Err(format!(
            "{} fields.json field_count {} != fields.len {}",
            bundle.code,
            fields_file.field_count,
            fields_file.fields.len()
        ));
    }
    let mut field_index = BTreeMap::new();
    for (idx, field) in fields_file.fields.iter().enumerate() {
        field_index.insert(field.field_key.clone(), idx);
        if let Some(serialized) = field.serialized_key.as_ref()
            && serialized != &field.field_key
        {
            field_index.entry(serialized.clone()).or_insert(idx);
        }
    }
    Ok(FormInventorySpec {
        form_code: bundle.code.to_string(),
        rules_id: if sections_file.rules_id.is_empty() {
            bundle.rules_id.to_string()
        } else {
            sections_file.rules_id
        },
        frozen_bundle: if sections_file.frozen_bundle.is_empty() {
            bundle.frozen_bundle.to_string()
        } else {
            sections_file.frozen_bundle
        },
        revision: fields_file.revision,
        field_count: fields_file.field_count,
        fields: fields_file.fields,
        sections: sections_file.sections,
        field_index,
    })
}

/// Period slot from the dashboard (`quarter` argument is month for monthly
/// forms). Frequency `?` forms that are still OpenEnded in the registry stay
/// open-ended; this helper does not invent monthly/quarterly/annual.
pub fn filing_period_for_form(form_code: &str, slot: u8) -> FilingPeriod {
    match find_form(form_code).map(|def| &def.frequency) {
        Some(FilingFrequency::Monthly) => FilingPeriod::Monthly(slot.clamp(1, 12)),
        Some(FilingFrequency::Quarterly) => FilingPeriod::Quarterly(slot.clamp(1, 4)),
        Some(FilingFrequency::Annual) => FilingPeriod::Annual,
        Some(FilingFrequency::OpenEnded) => FilingPeriod::OpenEnded(u32::from(slot.max(1))),
        None => FilingPeriod::OpenEnded(u32::from(slot.max(1))),
    }
}

/// Prefill identity / period keys from the taxpayer profile and the filing
/// slot. Unknown keys stay blank (`?` — do not invent form-specific mappings).
pub fn prefill_from_profile(
    spec: &FormInventorySpec,
    profile: &TaxpayerProfile,
    year: u16,
    period: &FilingPeriod,
) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let tin = &profile.tin;
    let (month, quarter) = match period {
        FilingPeriod::Monthly(m) => (Some(*m), None),
        FilingPeriod::Quarterly(q) => (None, Some(*q)),
        FilingPeriod::Annual | FilingPeriod::OpenEnded(_) => (None, None),
    };
    for field in &spec.fields {
        let probe = format!("{} {}", field.field_key, field.xml_key());
        let filled = if looks_like(probe.as_str(), "TIN1") {
            Some(tin.segment1.clone())
        } else if looks_like(probe.as_str(), "TIN2") {
            Some(tin.segment2.clone())
        } else if looks_like(probe.as_str(), "TIN3") {
            Some(tin.segment3.clone())
        } else if looks_like(probe.as_str(), "BranchCode") {
            Some(tin.branch.clone())
        } else if looks_like(probe.as_str(), "RDOCode") || looks_like(probe.as_str(), "RdoCode") {
            Some(profile.rdo_code.clone())
        } else if looks_like(probe.as_str(), "registeredName")
            || looks_like(probe.as_str(), "TaxpayerName")
            || looks_like(probe.as_str(), "txtTaxpayerName")
        {
            Some(profile.full_name.clone())
        } else if looks_like(probe.as_str(), "registeredAddress")
            || looks_like(probe.as_str(), "txtAddress")
        {
            Some(profile.registered_address.clone())
        } else if looks_like(probe.as_str(), "zipCode") || looks_like(probe.as_str(), "ZipCode") {
            Some(profile.zip_code.clone())
        } else if looks_like(probe.as_str(), "telNo")
            || looks_like(probe.as_str(), "txtTel")
            || looks_like(probe.as_str(), "txtTelNum")
        {
            Some(profile.phone.clone())
        } else if looks_like(probe.as_str(), "txtEmail") || looks_like(probe.as_str(), "Email") {
            Some(profile.email.clone())
        } else if looks_like(probe.as_str(), "txtYear") {
            Some(year.to_string())
        } else if let Some(month) = month
            && (looks_like(probe.as_str(), "txtMonth") || looks_like(probe.as_str(), "rtnMonth"))
        {
            Some(format!("{month:02}"))
        } else if let Some(quarter) = quarter {
            let q_key = format!("qtr_{quarter}");
            if probe.contains(&q_key) || probe.contains(&format!("DateQuarter_{quarter}")) {
                Some("true".to_string())
            } else if looks_like(probe.as_str(), "qtr_") || looks_like(probe.as_str(), "DateQuarter_")
            {
                Some("false".to_string())
            } else {
                None
            }
        } else {
            None
        };
        if let Some(value) = filled {
            values.insert(field.field_key.clone(), value);
        }
    }
    values
}

/// Map a BIR XML field map onto inventory `field_key`s for the editor.
pub fn values_from_bir_map(
    spec: &FormInventorySpec,
    typed: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for field in &spec.fields {
        if let Some(value) = typed
            .get(field.xml_key())
            .or_else(|| typed.get(&field.field_key))
        {
            values.insert(field.field_key.clone(), value.clone());
        }
    }
    values
}

/// Editor-visible required fields that are still blank. Used after touch/save.
pub fn required_blank_errors(
    spec: &FormInventorySpec,
    values: &BTreeMap<String, String>,
) -> Vec<(String, String)> {
    let mut errors = Vec::new();
    for section in spec.editor_sections() {
        for key in &section.fields {
            let Some(field) = spec.field(key) else {
                continue;
            };
            if field.is_computed() || !field.is_required() {
                continue;
            }
            let value = values
                .get(&field.field_key)
                .or_else(|| values.get(field.xml_key()))
                .map(String::as_str)
                .unwrap_or("");
            if value.trim().is_empty() || value == "false" && field.control_kind == "radio" {
                // Radio pairs are required as a group; a single `false` box is
                // not a blank. Only empty strings count here.
                if value.trim().is_empty() {
                    errors.push((
                        field.field_key.clone(),
                        format!("{} is required", field.display_label()),
                    ));
                }
            }
        }
    }
    errors
}

/// Merge typed XML keys with inventory values. Print/XML-only keys stay in
/// the map even when the editor hid them.
pub fn bir_field_map_with_inventory(
    spec: &FormInventorySpec,
    values: &BTreeMap<String, String>,
    typed: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut map = typed.clone();
    for field in &spec.fields {
        let value = values
            .get(&field.field_key)
            .or_else(|| values.get(field.xml_key()));
        if let Some(value) = value {
            map.entry(field.xml_key().to_string())
                .or_insert_with(|| value.clone());
        }
    }
    map
}

pub fn truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "y"
    )
}

pub fn parse_money(value: &str) -> Option<f64> {
    let trimmed = value.trim().replace(',', "");
    if trimmed.is_empty() {
        return Some(0.0);
    }
    trimmed.parse().ok()
}

fn deserialize_enum_values<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<Vec<serde_json::Value>>::deserialize(deserializer)?.unwrap_or_default();
    Ok(raw.into_iter().filter_map(enum_token).collect())
}

fn enum_token(value: serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Object(map) => {
            for key in [
                "value",
                "stored_value",
                "control_value",
                "html_value",
                "serialized_selected",
            ] {
                match map.get(key) {
                    Some(serde_json::Value::String(s)) if !s.trim().is_empty() => {
                        return Some(s.clone());
                    }
                    Some(serde_json::Value::Number(n)) => return Some(n.to_string()),
                    Some(serde_json::Value::Bool(b)) => return Some(b.to_string()),
                    _ => {}
                }
            }
            None
        }
        _ => None,
    }
}

fn looks_like(haystack: &str, needle: &str) -> bool {
    haystack.to_ascii_lowercase().contains(&needle.to_ascii_lowercase())
}

fn clean_label(raw: Option<&str>, fallback_key: &str) -> String {
    let short = fallback_key
        .rsplit(':')
        .next()
        .unwrap_or(fallback_key)
        .split('#')
        .next()
        .unwrap_or(fallback_key);
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return humanize(short);
    };
    let stripped = if raw.contains(':') && raw.split(':').next().is_some_and(|p| p.starts_with("frm"))
    {
        raw.split_once(':').map(|(_, rest)| rest).unwrap_or(raw)
    } else {
        raw
    };
    if stripped == short || looks_key_like(stripped) {
        return humanize(short);
    }
    stripped.to_string()
}

fn looks_key_like(label: &str) -> bool {
    !label.contains(' ') && label.chars().any(|c| c.is_ascii_digit()) && label.len() < 18
}

fn humanize(key: &str) -> String {
    let mut out = String::new();
    for (i, ch) in key.chars().enumerate() {
        if ch == '_' {
            out.push(' ');
            continue;
        }
        if i > 0 && ch.is_ascii_uppercase() {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::form_2551q::Form2551QDraft;
    use crate::naming::Tin;
    use crate::profile::TaxpayerType;

    fn sample_profile() -> TaxpayerProfile {
        TaxpayerProfile {
            id: None,
            full_name: "Sample Taxpayer".into(),
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
    fn catalog_covers_all_forty_three_rules_bundles() {
        assert_eq!(INVENTORY_BUNDLES.len(), 43);
        let mut codes = BTreeSet::new();
        for bundle in INVENTORY_BUNDLES {
            assert!(codes.insert(bundle.code), "duplicate {}", bundle.code);
            let spec = load_spec(bundle.code).expect(bundle.code);
            spec.bound_keys_once().expect(bundle.code);
        }
        assert!(has_inventory("2551Q"));
        assert!(has_inventory("2553"));
        assert!(!has_inventory("9999"));
        let month = load_spec("1601C")
            .expect("1601C")
            .field("frm1601c:txtMonth")
            .expect("month")
            .enum_values
            .clone();
        assert!(
            month.iter().any(|value| value == "01"),
            "1601C month enum_values must accept {{value,label}} objects, got {month:?}"
        );
    }

    #[test]
    fn every_2551q_fields_json_key_is_bound_exactly_once() {
        let spec = load_spec("2551Q").expect("2551Q");
        let keys = spec.bound_keys_once().expect("2551Q partition");
        assert_eq!(keys.len(), 99);
        assert_eq!(spec.field_count, 99);
    }

    #[test]
    fn inventory_hides_print_xml_keys_but_keeps_them_in_the_2551q_bir_map() {
        let spec = load_spec("2551Q").expect("2551Q");
        let editor: BTreeSet<&str> = spec.editor_field_keys().into_iter().collect();
        let print_xml = spec.print_xml_keys();
        assert!(!print_xml.is_empty());
        for key in &print_xml {
            assert!(
                !editor.contains(key),
                "print/XML key {key} must stay hidden in the editor"
            );
        }
        assert!(print_xml.iter().any(|k| k.contains("txtPg2TIN1")));
        assert!(print_xml.iter().any(|k| k.contains("txtPg2TaxpayerName")));

        let profile = sample_profile();
        let mut draft = Form2551QDraft::new_from_profile(&profile, 2026, 1);
        draft.tin = "26170801500000".into();
        let typed = draft.to_bir_field_map();
        let values = prefill_from_profile(
            &spec,
            &profile,
            2026,
            &FilingPeriod::Quarterly(1),
        );
        let merged = bir_field_map_with_inventory(&spec, &values, &typed);
        for key in print_xml {
            let xml_key = spec.field(key).map(|f| f.xml_key()).unwrap_or(key);
            assert!(
                merged.contains_key(xml_key) || typed.contains_key(xml_key),
                "BIR map missing print/XML key {xml_key}"
            );
        }
        assert_eq!(typed["frm2551Qv2018:txtPg2TIN1"], "261");
        assert_eq!(typed["frm2551Qv2018:txtPg2TaxpayerName"], "Sample Taxpayer");
    }

    #[test]
    fn apply_inventory_values_updates_2551q_typed_fields_and_xml_repeats() {
        let spec = load_spec("2551Q").expect("2551Q");
        let profile = sample_profile();
        let mut draft = Form2551QDraft::new_from_profile(&profile, 2026, 1);
        let mut values = values_from_bir_map(&spec, &draft.to_bir_field_map());
        values.insert("frm2551Qv2018:txt15".into(), "12.50".into());
        values.insert("frm2551Qv2018:registeredName".into(), "New Name".into());
        draft.apply_inventory_values(&values);
        assert!((draft.creditable_tax_withheld - 12.5).abs() < 0.001);
        assert_eq!(draft.taxpayer_name, "New Name");
        let map = draft.to_bir_field_map();
        assert_eq!(map["frm2551Qv2018:txtPg2TaxpayerName"], "New Name");
        assert!(map.contains_key("frm2551Qv2018:txtPg2TIN1"));
    }

    #[test]
    fn queue_gate_is_unchanged_for_inventory_only_codes() {
        assert!(crate::forms::can_queue_for_submission("2551Q"));
        assert!(crate::forms::can_queue_for_submission("1601C"));
        for code in inventory_codes().filter(|code| !matches!(*code, "2551Q" | "1601C")) {
            assert!(
                !crate::forms::can_queue_for_submission(code),
                "{code} must not gain queue authority from the inventory editor"
            );
        }
    }
}
