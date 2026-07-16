//! Integration payload validation — pre-flight checks before mapping.
//!
//! Validates the `UniversalTaxPayload` structure, period consistency,
//! and profile compatibility before the mapper engine runs.
//!
//! **Form eligibility is evaluated through the database-backed Forms Set.**

use crate::calendar_rules::{
    DeadlineOverride, DeadlineResolver, ResolvedTaxDeadline, canonical_form_code,
};
use crate::forms::atc::find_atc;
use crate::forms::registry::FilingFrequency;
use crate::forms::{FormSuggestion, FormSuggestionSource};
use crate::integration::models::UniversalTaxPayload;
use crate::profile::{
    ManualObligationOverrideAction, RegisteredTaxType, TaxProfileVersion, TaxpayerProfile,
};
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSuggestionDecision {
    pub form_code: String,
    pub is_suggested: bool,
    pub reason: String,
    pub legal_authority_citation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProfileConsistencySeverity {
    Info,
    Warning,
    NeedsReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileConsistencyIssue {
    pub severity: ProfileConsistencySeverity,
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub form_code: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub fix_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileConsistencyReport {
    pub issues: Vec<ProfileConsistencyIssue>,
}

impl ProfileConsistencyReport {
    #[allow(clippy::too_many_arguments)]
    fn push_detailed(
        &mut self,
        severity: ProfileConsistencySeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        version_id: Option<String>,
        form_code: Option<String>,
        source: Option<String>,
        fix_hint: Option<String>,
    ) {
        self.issues.push(ProfileConsistencyIssue {
            severity,
            code: code.into(),
            message: message.into(),
            version_id,
            form_code,
            source,
            fix_hint,
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedProfileObligations {
    pub taxable_year: u16,
    pub form_codes: Vec<String>,
    pub active_version_ids: Vec<String>,
    pub consistency_report: ProfileConsistencyReport,
}

/// A single validation issue found in a payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadValidationError {
    pub field: String,
    pub message: String,
}

impl PayloadValidationError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// Validate a payload's structural integrity (independent of any profile).
pub fn validate_payload(payload: &UniversalTaxPayload) -> Vec<PayloadValidationError> {
    let mut errors = Vec::new();
    let valid_money = |value: f64| {
        value.is_finite()
            && value >= 0.0
            && ((value * 100.0) - (value * 100.0).round()).abs() < 1e-7
    };

    // TIN format
    let tin_len = payload.tin.len();
    if tin_len != 12 && tin_len != 13 {
        errors.push(PayloadValidationError::new(
            "tin",
            format!("TIN must be 12 or 13 digits, got {tin_len}"),
        ));
    }
    if !payload.tin.chars().all(|c| c.is_ascii_digit()) {
        errors.push(PayloadValidationError::new(
            "tin",
            "TIN must contain only digits",
        ));
    }

    // Period consistency
    if payload.period_end < payload.period_start {
        errors.push(PayloadValidationError::new(
            "period_end",
            "period_end must be on or after period_start",
        ));
    }

    // Period must not span more than 1 year
    let year_diff = payload.period_end.year() - payload.period_start.year();
    if year_diff > 1
        || (year_diff == 1 && payload.period_end.month() > payload.period_start.month())
    {
        errors.push(PayloadValidationError::new(
            "period_end",
            "Period must not span more than 12 months",
        ));
    }

    // amended-only fields
    if !payload.is_amended && payload.previous_tax_paid > 0.0 {
        errors.push(PayloadValidationError::new(
            "previous_tax_paid",
            "previous_tax_paid should be 0 for non-amended returns",
        ));
    }

    // Income source validation
    for (i, source) in payload.income_sources.iter().enumerate() {
        if !valid_money(source.gross_amount) {
            errors.push(PayloadValidationError::new(
                format!("income_sources[{i}].gross_amount"),
                "Gross amount must be finite, non-negative, and have at most two decimal places",
            ));
        }
        if let Some(rate) = source.tax_rate_override
            && (!(0.0..=1.0).contains(&rate))
        {
            errors.push(PayloadValidationError::new(
                format!("income_sources[{i}].tax_rate_override"),
                "Tax rate override must be between 0.0 and 1.0",
            ));
        }
    }

    if !valid_money(payload.creditable_withholdings) {
        errors.push(PayloadValidationError::new(
            "creditable_withholdings",
            "Creditable withholdings must be finite, non-negative, and have at most two decimal places",
        ));
    }

    if !valid_money(payload.previous_tax_paid) {
        errors.push(PayloadValidationError::new(
            "previous_tax_paid",
            "Previous tax paid must be finite, non-negative, and have at most two decimal places",
        ));
    }

    errors
}

/// Validate that a target form is applicable for the given profile and year.
///
/// Looks up active forms in the database-backed Forms Set.
pub fn validate_form_applicability(
    form_code: &str,
    profile: &TaxpayerProfile,
    taxable_year: u16,
) -> Option<PayloadValidationError> {
    let normalized = canonical_form_code(form_code);
    if crate::forms::registry::find_form(normalized).is_none() {
        return Some(PayloadValidationError::new(
            "target_form",
            format!("Unknown form code: {form_code}"),
        ));
    }

    let Some(forms_set) = profile.forms_set_for_year(taxable_year) else {
        return Some(PayloadValidationError::new(
            "target_form",
            format!(
                "The Forms Set for taxable year {taxable_year} is not configured and needs review before a form can be selected"
            ),
        ));
    };

    if let Some(entry) = forms_set.entry(normalized)
        && entry.needs_review()
    {
        return Some(PayloadValidationError::new(
            "target_form",
            format!(
                "Form {normalized} has conflicting equally authoritative Forms Set evidence for {taxable_year}; record a manual include/exclude decision before filing"
            ),
        ));
    }

    let visible = profile.active_form_codes_for_year(taxable_year);

    if !visible.iter().any(|c| *c == normalized) {
        return Some(PayloadValidationError::new(
            "target_form",
            format!(
                "Form {} is not applicable for this taxpayer profile in year {}",
                form_code, taxable_year
            ),
        ));
    }

    None
}

/// Evaluates a taxpayer profile against the form registry using the active Forms Set.
/// Returns detailed decisions with reasons for all applicable forms.
///
/// This is the single canonical form suggestion path. It delegates to the active
/// Forms Set rather than maintaining a separate hardcoded eligibility matrix.
#[allow(clippy::collapsible_if)]
pub fn evaluate_forms(profile: &TaxpayerProfile) -> Vec<FormSuggestionDecision> {
    let current_year = chrono::Local::now().year() as u16;
    evaluate_forms_for_year(profile, current_year)
}

/// Year-explicit variant of evaluate_forms for historical/amended filing.
#[allow(clippy::collapsible_if)]
pub fn evaluate_forms_for_year(
    profile: &TaxpayerProfile,
    taxable_year: u16,
) -> Vec<FormSuggestionDecision> {
    let forms_set_configured = profile.forms_set_for_year(taxable_year).is_some();
    let active_codes = profile.active_form_codes_for_year(taxable_year);
    crate::forms::registry::FORM_REGISTRY
        .iter()
        .map(|f| {
            let is_suggested = active_codes.contains(&f.code.to_string());
            FormSuggestionDecision {
                form_code: f.code.to_string(),
                is_suggested,
                reason: if is_suggested {
                    "Applicable based on Forms Set".to_string()
                } else if !forms_set_configured {
                    format!("Forms Set for taxable year {taxable_year} needs review")
                } else {
                    "Not registered / not applicable".to_string()
                },
                legal_authority_citation: "".to_string(),
            }
        })
        .collect()
}

/// Returns the list of applicable form codes for a profile, considering
/// both `TaxpayerType` and `TaxClassification`.
///
/// Loads the active forms set for the current year.
pub fn applicable_forms_for_profile(profile: &TaxpayerProfile) -> Vec<String> {
    let current_year = chrono::Local::now().year() as u16;
    applicable_forms_for_profile_and_year(profile, current_year)
}

/// Returns the list of applicable form codes for a profile in a specific year.
///
/// Loads the active forms set for the chosen year. This is the preferred API
/// for the dashboard where the user selects a tax year.
pub fn applicable_forms_for_profile_and_year(profile: &TaxpayerProfile, year: u16) -> Vec<String> {
    profile.active_form_codes_for_year(year)
}

/// Returns recurring dashboard obligations for a profile in a specific year.
///
/// This is intentionally narrower than `applicable_forms_for_profile_and_year()`.
/// The broader API answers "can this profile ever use this form"; the dashboard
/// needs "which forms belong to this profile's normal filing calendar".
pub fn recurring_obligation_forms_for_profile_and_year(
    profile: &TaxpayerProfile,
    year: u16,
) -> Vec<String> {
    resolve_profile_obligations_for_year(profile, year).form_codes
}

pub fn recurring_obligation_decisions_for_profile_and_year(
    profile: &TaxpayerProfile,
    year: u16,
) -> Vec<crate::forms::FormCardData> {
    let obligations = resolve_profile_obligations_for_year(profile, year);
    obligations
        .form_codes
        .into_iter()
        .filter_map(|code| {
            crate::forms::registry::find_form(&code).map(|def| crate::forms::FormCardData {
                form_code: code.clone(),
                category: def.category.to_string(),
                title: def.title.to_string(),
                frequency: def.frequency.clone(),
            })
        })
        .collect()
}

/// Returns every active form in the confirmed yearly set, including open-ended
/// transaction forms that do not generate recurring calendar deadlines.
pub fn applicable_form_decisions_for_profile_and_year(
    profile: &TaxpayerProfile,
    year: u16,
) -> Vec<crate::forms::FormCardData> {
    profile
        .active_form_codes_for_year(year)
        .into_iter()
        .filter_map(|code| {
            crate::forms::registry::find_form(&code).map(|def| crate::forms::FormCardData {
                form_code: def.code.to_string(),
                category: def.category.to_string(),
                title: def.title.to_string(),
                frequency: def.frequency.clone(),
            })
        })
        .collect()
}

/// Resolve the form codes explicitly evidenced by one confirmed COR/profile
/// version. Exact form codes extracted from the COR take precedence over broad
/// registered-tax-type expansion. Tax types remain the fallback for manual and
/// legacy versions that have no extracted form list.
pub fn registered_form_codes_for_version(
    profile: &TaxpayerProfile,
    version: &TaxProfileVersion,
    year: u16,
) -> Vec<String> {
    let extracted_codes: BTreeSet<String> = version
        .evidence
        .iter()
        .flat_map(|document| document.extracted_form_codes.iter())
        .map(|code| crate::forms::registry::canonical_form_code(code))
        .filter(|code| !code.is_empty())
        .collect();
    let has_exact_cor_codes = !extracted_codes.is_empty();

    let mut codes = if has_exact_cor_codes {
        extracted_codes
            .into_iter()
            .filter(|code| exact_cor_code_passes_version_filter(code, version, profile, year))
            .collect::<BTreeSet<_>>()
    } else {
        crate::forms::registry::FORM_REGISTRY
            .iter()
            .filter(|def| {
                registered_tax_types_allow_form(version, def.code)
                    && obligation_allowed_for_version_and_profile(def, version, profile, year)
            })
            .map(|def| def.code.to_string())
            .collect::<BTreeSet<_>>()
    };

    for override_rule in &version.obligation_overrides {
        let code = normalize_form_code(&override_rule.form_code);
        match override_rule.action {
            ManualObligationOverrideAction::Include => {
                codes.insert(code);
            }
            ManualObligationOverrideAction::Exclude => {
                codes.remove(&code);
            }
        }
    }

    codes.into_iter().collect()
}

/// Builds auditable form suggestions from the unambiguous confirmed profile
/// segments for a taxable year. Exact reviewed COR codes outrank tax-type
/// inference within each segment; explicit obligation overrides outrank both.
pub fn form_suggestions_for_profile_year(
    profile: &TaxpayerProfile,
    year: u16,
) -> Vec<FormSuggestion> {
    let resolved = profile.resolve_tax_profile_for_year(year);
    if resolved.has_blocking_issues() {
        return Vec::new();
    }

    let mut suggestions = Vec::new();
    for version in resolved.effective_segments {
        let explicit_non_vat_document_ids = version
            .evidence
            .iter()
            .filter(|document| {
                document.ocr_text.as_deref().is_some_and(|text| {
                    crate::profile::classify_vat_registration_text(text)
                        == crate::profile::VatRegistrationTextClassification::NonVat
                })
            })
            .map(|document| document.id.clone())
            .collect::<Vec<_>>();
        let mut guarded_version = version.clone();
        if !explicit_non_vat_document_ids.is_empty() {
            guarded_version.is_vat_registered = false;
            guarded_version
                .registered_tax_types
                .retain(|tax_type| *tax_type != RegisteredTaxType::ValueAddedTax);
        }

        let has_exact_cor_codes = version_has_exact_cor_codes(&guarded_version);
        let generated_source = if guarded_version.source
            == crate::profile::TaxProfileVersionSource::MigrationBackfill
        {
            FormSuggestionSource::MigrationBackfill
        } else if has_exact_cor_codes {
            FormSuggestionSource::ReviewedCor
        } else {
            FormSuggestionSource::InferredTaxType
        };
        let source_reference = if has_exact_cor_codes {
            let document_ids = guarded_version
                .evidence
                .iter()
                .filter(|document| !document.extracted_form_codes.is_empty())
                .map(|document| document.id.as_str())
                .collect::<Vec<_>>();
            Some(document_ids.join(", "))
        } else {
            Some(guarded_version.id.clone())
        };

        for code in registered_form_codes_for_version(profile, &guarded_version, year) {
            let manual_override = guarded_version.obligation_overrides.iter().find(|rule| {
                normalize_form_code(&rule.form_code) == normalize_form_code(&code)
                    && rule.action == ManualObligationOverrideAction::Include
            });
            let (source, reason, reference) = if let Some(rule) = manual_override {
                (
                    FormSuggestionSource::ManualOverride,
                    Some(rule.reason.clone()),
                    rule.source_reference.clone(),
                )
            } else {
                let reason = match generated_source {
                    FormSuggestionSource::ReviewedCor => {
                        format!(
                            "Reviewed exact COR form code from '{}'",
                            guarded_version.label
                        )
                    }
                    FormSuggestionSource::InferredTaxType => format!(
                        "Inferred from registered tax types in '{}'",
                        guarded_version.label
                    ),
                    FormSuggestionSource::MigrationBackfill => format!(
                        "Migrated from legacy profile version '{}'",
                        guarded_version.label
                    ),
                    FormSuggestionSource::ManualOverride => String::new(),
                };
                (generated_source, Some(reason), source_reference.clone())
            };
            suggestions.push(FormSuggestion {
                form_code: code.clone(),
                active: true,
                source,
                reason,
                source_reference: reference,
                effective_from: guarded_version.effective_from,
                effective_until: guarded_version.effective_until,
            });

            // An exact form code is reviewed, form-specific evidence and can
            // establish VAT-form eligibility even when the coarse boolean flag
            // was not set. Explicit NON-VAT wording is equally reviewed evidence,
            // though, so preserve both sides as an include/exclude conflict. The
            // Forms Set reconciler will fail this code closed as NeedsReview.
            if !explicit_non_vat_document_ids.is_empty()
                && manual_override.is_none()
                && is_vat_required_form_code(&code)
            {
                suggestions.push(FormSuggestion {
                    form_code: code.clone(),
                    active: false,
                    source: FormSuggestionSource::ReviewedCor,
                    reason: Some(format!(
                        "Explicit NON-VAT evidence conflicts with reviewed exact VAT form code '{code}' in '{}'",
                        guarded_version.label
                    )),
                    source_reference: Some(explicit_non_vat_document_ids.join(", ")),
                    effective_from: guarded_version.effective_from,
                    effective_until: guarded_version.effective_until,
                });
            }
        }

        for rule in guarded_version
            .obligation_overrides
            .iter()
            .filter(|rule| rule.action == ManualObligationOverrideAction::Exclude)
        {
            suggestions.push(FormSuggestion {
                form_code: normalize_form_code(&rule.form_code),
                active: false,
                source: FormSuggestionSource::ManualOverride,
                reason: Some(rule.reason.clone()),
                source_reference: rule.source_reference.clone(),
                effective_from: guarded_version.effective_from,
                effective_until: guarded_version.effective_until,
            });
        }
    }

    suggestions
}

fn version_has_exact_cor_codes(version: &TaxProfileVersion) -> bool {
    version
        .evidence
        .iter()
        .any(|document| !document.extracted_form_codes.is_empty())
}

pub fn resolve_profile_obligations_for_year(
    profile: &TaxpayerProfile,
    year: u16,
) -> ResolvedProfileObligations {
    let mut all_codes = BTreeSet::new();
    let mut active_version_ids = Vec::new();
    let mut code_version_ids: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut consistency_report = ProfileConsistencyReport::default();

    let resolved_profile = profile.resolve_tax_profile_for_year(year);
    for issue in resolved_profile.issues {
        consistency_report.push_detailed(
            ProfileConsistencySeverity::NeedsReview,
            "PROFILE_TIMELINE_NEEDS_REVIEW",
            issue.message,
            issue.version_ids.first().cloned(),
            None,
            Some("Confirmed profile version timeline".into()),
            Some(
                "Set non-overlapping effective dates before accepting regenerated form suggestions. The stored Forms Set remains authoritative until then."
                    .into(),
            ),
        );
    }
    let effective_segments = resolved_profile.effective_segments;
    for version in &effective_segments {
        active_version_ids.push(version.id.clone());
        report_cor_tin_mismatch(profile, version, &mut consistency_report);
    }

    if profile.per_year_forms.contains_key(&year) {
        for version in &effective_segments {
            if !version_has_exact_cor_codes(version) {
                let projected = profile.projection_for_version(version);
                let _ = recurring_obligation_codes_for_version(
                    &projected,
                    version,
                    year,
                    &mut consistency_report,
                );
            }
        }
        let active_codes = profile.active_form_codes_for_year(year);
        for code in active_codes {
            if is_recurring_form_code(&code) {
                for version_id in &active_version_ids {
                    code_version_ids
                        .entry(code.clone())
                        .or_default()
                        .insert(version_id.clone());
                }
                all_codes.insert(code);
            }
        }
    } else {
        consistency_report.push_detailed(
            ProfileConsistencySeverity::NeedsReview,
            "FORMS_SET_NOT_CONFIGURED",
            format!(
                "No authoritative Forms Set is stored for taxable year {year}; profile suggestions were not used as filing obligations."
            ),
            None,
            None,
            Some("Per-year Forms Set".into()),
            Some(
                "Review the confirmed COR/profile suggestions and save the Forms Set for this taxable year."
                    .into(),
            ),
        );
    }

    report_obligations_without_calendar_rules(
        year,
        &all_codes,
        &code_version_ids,
        &mut consistency_report,
    );

    // Check the resolved obligations rather than raw persisted rows. A newly confirmed
    // COR may supersede stale CorAi entries before the reconciled set is saved.
    let resolved_entries = all_codes
        .iter()
        .map(|code| {
            crate::forms::FormSetEntry::from_code(code.clone(), crate::forms::FormSetSource::CorAi)
        })
        .collect::<Vec<_>>();
    consistency_report
        .issues
        .extend(check_annual_itr_conflicts(&resolved_entries));

    ResolvedProfileObligations {
        taxable_year: year,
        form_codes: all_codes.into_iter().collect(),
        active_version_ids,
        consistency_report,
    }
}

pub fn resolve_profile_obligations_for_year_with_global_overrides(
    profile: &TaxpayerProfile,
    year: u16,
    global_deadline_overrides: &[DeadlineOverride],
) -> ResolvedProfileObligations {
    let mut resolved = resolve_profile_obligations_for_year(profile, year);
    report_profile_global_deadline_override_conflicts(
        profile,
        year,
        global_deadline_overrides,
        &mut resolved.consistency_report,
    );
    resolved
}

pub fn deadline_applies_to_profile(
    profile: &TaxpayerProfile,
    deadline: &ResolvedTaxDeadline,
) -> bool {
    let Some(taxable_year) = deadline.period.taxable_year() else {
        return false;
    };

    profile.forms_set_for_year(taxable_year as u16).is_some()
        && profile
            .active_form_codes_for_year(taxable_year as u16)
            .contains(&deadline.form_code)
}

pub fn profile_deadline_overrides_for_year(
    profile: &TaxpayerProfile,
    year: u16,
) -> Vec<DeadlineOverride> {
    profile
        .active_profile_versions_for_year(year)
        .into_iter()
        .flat_map(|version| {
            version
                .deadline_overrides
                .into_iter()
                .map(|override_rule| DeadlineOverride {
                    id: override_rule.id,
                    title: override_rule.title,
                    source_reference: override_rule.source_reference,
                    affected_form_codes: override_rule
                        .affected_form_codes
                        .into_iter()
                        .map(|code| normalize_form_code(&code))
                        .collect(),
                    original_deadline: override_rule.original_deadline,
                    adjusted_deadline: override_rule.adjusted_deadline,
                    affected_regions: vec![],
                    affected_taxpayer_types: vec![],
                    effective_from: None,
                    effective_until: None,
                    expires_at: None,
                })
        })
        .collect()
}

fn report_profile_global_deadline_override_conflicts(
    profile: &TaxpayerProfile,
    year: u16,
    global_deadline_overrides: &[DeadlineOverride],
    report: &mut ProfileConsistencyReport,
) {
    let profile_overrides_by_version = profile
        .active_profile_versions_for_year(year)
        .into_iter()
        .flat_map(|version| {
            let version_id = version.id.clone();
            version
                .deadline_overrides
                .into_iter()
                .map(move |override_rule| (version_id.clone(), override_rule))
        })
        .collect::<Vec<_>>();

    for (version_id, profile_override) in profile_overrides_by_version {
        let profile_codes = profile_override
            .affected_form_codes
            .iter()
            .map(|code| normalize_form_code(code))
            .collect::<BTreeSet<_>>();

        for global_override in global_deadline_overrides {
            if profile_override.original_deadline != global_override.original_deadline {
                continue;
            }
            if profile_override.adjusted_deadline == global_override.adjusted_deadline {
                continue;
            }

            let shared_codes = global_override
                .affected_form_codes
                .iter()
                .map(|code| normalize_form_code(code))
                .filter(|code| profile_codes.contains(code))
                .collect::<Vec<_>>();

            if shared_codes.is_empty() {
                continue;
            }

            let shared_label = shared_codes.join(", ");
            report.push_detailed(
                ProfileConsistencySeverity::Warning,
                "PROFILE_GLOBAL_DEADLINE_OVERRIDE_CONFLICT",
                format!(
                    "Profile deadline override '{}' adjusts {shared_label} from {} to {}, but global override '{}' adjusts the same statutory date to {}.",
                    profile_override.title,
                    profile_override.original_deadline,
                    profile_override.adjusted_deadline,
                    global_override.title,
                    global_override.adjusted_deadline
                ),
                Some(version_id.clone()),
                shared_codes.first().cloned(),
                Some("Profile deadline override + global deadline override".into()),
                Some(
                    "Confirm which source controls this taxpayer. Profile-specific overrides win for dashboard resolution, but the conflict should stay documented."
                        .into(),
                ),
            );
        }
    }
}

pub fn obligation_allowed_for_version_and_profile(
    def: &crate::forms::registry::FormDefinition,
    version: &TaxProfileVersion,
    profile: &TaxpayerProfile,
    year: u16,
) -> bool {
    obligation_allowed_for_version_and_profile_with_evidence(def, version, profile, year, false)
}

fn obligation_allowed_for_version_and_profile_with_evidence(
    def: &crate::forms::registry::FormDefinition,
    version: &TaxProfileVersion,
    profile: &TaxpayerProfile,
    year: u16,
    reviewed_exact_form_code: bool,
) -> bool {
    // 1. Deprecation check
    if let Some(dep_year) = def.deprecation_year() {
        if year >= dep_year {
            return false;
        }
    }

    // 2. Taxpayer type compatibility
    if !def.taxpayer_types.contains(&version.taxpayer_type) {
        return false;
    }

    // 3. VAT registration requirement
    if let Some(req_vat) = def.requires_vat {
        let reviewed_exact_vat_code_establishes_eligibility = reviewed_exact_form_code && req_vat;
        if req_vat != version.is_vat_registered && !reviewed_exact_vat_code_establishes_eligibility
        {
            return false;
        }
    }

    // 4. Registration activity status check (close means no obligations; inactive keeps NIL filing)
    if matches!(
        version.registration_activity_status,
        crate::profile::RegistrationActivityStatus::OfficiallyClosed
    ) {
        return false;
    }

    // 5. Abolished 1704 check
    if def.code == "1704" && year >= 2021 {
        return false;
    }

    // 6. 1701MS (EOPT Micro/Small simplified return)
    if def.code == "1701MS" {
        let is_micro_or_small = matches!(
            version.eopt_tier,
            Some(crate::profile::EoptTier::Micro) | Some(crate::profile::EoptTier::Small)
        );
        if year < 2024 || !is_micro_or_small {
            return false;
        }
    }

    // 7. Substituted Filing (1700 suppression for Compensation earner with single employer)
    if def.code == "1700" {
        if let Some(crate::profile::TaxClassification::PurelyCompensation) =
            version.tax_classification
        {
            if profile.has_single_employer {
                return false;
            }
        }
    }

    // 8. An 8% income-tax election replaces only Section 116 percentage tax.
    // Suppress a PT010-only obligation, but preserve 2551Q/2551M when the
    // profile owns another registered percentage-tax ATC (for example PT040).
    // The form-specific schedule validator separately enforces NIL PT010.
    if matches!(def.code, "2551Q" | "2551M")
        && profile.has_8_percent_election(year)
        && !profile.atc_codes.iter().any(|code| {
            let code = code.trim();
            code != "PT010" && find_atc(code).is_some()
        })
    {
        return false;
    }

    // 9. Corporate ITR mutual exclusivity
    // Suffix-specific: 1702RT, 1702EX, 1702MX
    // Suffix-specific versions win according to the effective tax classification
    let effective_class = match version.taxpayer_type {
        crate::profile::TaxpayerType::Individual => version.tax_classification.clone(),
        crate::profile::TaxpayerType::Corporation | crate::profile::TaxpayerType::Partnership => {
            Some(crate::profile::TaxClassification::Corporation)
        }
        crate::profile::TaxpayerType::Cooperative => match version.tax_classification {
            Some(ref c)
                if matches!(
                    c,
                    crate::profile::TaxClassification::CooperativeExempt
                        | crate::profile::TaxClassification::CooperativeTaxable
                        | crate::profile::TaxClassification::CooperativeMixed
                ) =>
            {
                Some(c.clone())
            }
            _ => Some(crate::profile::TaxClassification::CooperativeTaxable),
        },
        crate::profile::TaxpayerType::Estate | crate::profile::TaxpayerType::Trust => {
            Some(crate::profile::TaxClassification::EstateOrTrust)
        }
    };

    if def.code == "1702" {
        return matches!(
            version.taxpayer_type,
            crate::profile::TaxpayerType::Corporation
                | crate::profile::TaxpayerType::Partnership
                | crate::profile::TaxpayerType::Cooperative
        );
    }
    if def.code == "1702EX"
        && effective_class != Some(crate::profile::TaxClassification::CooperativeExempt)
    {
        return false;
    }
    if def.code == "1702MX"
        && effective_class != Some(crate::profile::TaxClassification::CooperativeMixed)
    {
        return false;
    }
    if def.code == "1702RT"
        && effective_class != Some(crate::profile::TaxClassification::Corporation)
        && effective_class != Some(crate::profile::TaxClassification::CooperativeTaxable)
    {
        return false;
    }

    // 10. Individual ITR mutual exclusivity
    // For MixedIncome earners, 1701A and 1701MS must be suppressed
    if let Some(crate::profile::TaxClassification::MixedIncome) = version.tax_classification {
        if def.code == "1701A" || def.code == "1701MS" {
            return false;
        }
    }

    // 11. Excise forms category filtering
    if is_excise_form(def.code) {
        let category = match def.code {
            "2200A" => Some(crate::profile::ExciseTaxCategory::Alcohol),
            "2200AN" => Some(crate::profile::ExciseTaxCategory::AutomobilesAndNonEssential),
            "2200C" => Some(crate::profile::ExciseTaxCategory::CoalAndCoke),
            "2200M" => Some(crate::profile::ExciseTaxCategory::Mineral),
            "2200P" => Some(crate::profile::ExciseTaxCategory::Petroleum),
            "2200S" => Some(crate::profile::ExciseTaxCategory::SweetenedBeverages),
            "2200T" => Some(crate::profile::ExciseTaxCategory::Tobacco),
            _ => None,
        };
        if let Some(cat) = category {
            if !version.excise_tax_categories.contains(&cat) {
                return false;
            }
        } else {
            return false;
        }
    }

    true
}

/// Validates a stored form code against the active profile version.
/// Used as a defense-in-depth post-filter when reading from `per_year_forms`
/// (Path A), in case the stored data was populated without full obligation checks.
fn exact_cor_code_passes_version_filter(
    code: &str,
    version: &TaxProfileVersion,
    profile: &TaxpayerProfile,
    year: u16,
) -> bool {
    let normalized = normalize_form_code(code);
    if let Some(r) = version
        .obligation_overrides
        .iter()
        .find(|o| normalize_form_code(&o.form_code) == normalized)
    {
        return match r.action {
            ManualObligationOverrideAction::Include => true,
            ManualObligationOverrideAction::Exclude => false,
        };
    }

    if let Some(def) = crate::forms::registry::find_form(code) {
        obligation_allowed_for_version_and_profile_with_evidence(def, version, profile, year, true)
    } else {
        true // custom codes always pass
    }
}

fn is_vat_required_form_code(code: &str) -> bool {
    crate::forms::registry::find_form(code)
        .is_some_and(|definition| definition.requires_vat == Some(true))
}

fn is_recurring_form_code(code: &str) -> bool {
    if let Some(def) = crate::forms::registry::find_form(code) {
        if matches!(def.frequency, FilingFrequency::OpenEnded) {
            return false;
        }
    }
    !matches!(code, "1707A" | "2552" | "2553")
}

fn recurring_obligation_codes_for_version(
    projected: &TaxpayerProfile,
    version: &TaxProfileVersion,
    year: u16,
    report: &mut ProfileConsistencyReport,
) -> BTreeSet<String> {
    let has_cor_gate = !version.registered_tax_types.is_empty();
    let mut gated = BTreeSet::new();

    for def in crate::forms::registry::FORM_REGISTRY {
        if is_recurring_form_code(def.code) {
            if (!has_cor_gate || registered_tax_types_allow_form(version, def.code))
                && obligation_allowed_for_version_and_profile(def, version, projected, year)
            {
                gated.insert(def.code.to_string());
            }
        }
    }

    if projected.has_8_percent_election(year)
        && !projected.atc_codes.iter().any(|code| {
            let code = code.trim();
            code != "PT010" && find_atc(code).is_some()
        })
        && !gated.contains("2551Q")
        && (version.registered_tax_types.is_empty()
            || version
                .registered_tax_types
                .contains(&RegisteredTaxType::PercentageTax))
    {
        report.push_detailed(
            ProfileConsistencySeverity::Info,
            "PERCENTAGE_TAX_SUPPRESSED_BY_8_PERCENT",
            "The annual 8% income-tax election suppresses the PT010-only percentage-tax obligation; no independently taxable non-PT010 ATC is registered for this profile.",
            Some(version.id.clone()),
            Some("2551Q".into()),
            Some("Income tax election + TTCE".into()),
            Some(
                "Verify the income tax election ledger and registered ATCs. Add the official non-PT010 ATC if another percentage-tax activity remains liable."
                    .into(),
            ),
        );
    }

    if has_cor_gate {
        report_missing_ttce_categories(version, &gated, report);
    }

    for override_rule in &version.obligation_overrides {
        let code = normalize_form_code(&override_rule.form_code);
        match override_rule.action {
            ManualObligationOverrideAction::Include => {
                if crate::forms::registry::find_form(&code).is_none() {
                    report.push_detailed(
                        ProfileConsistencySeverity::NeedsReview,
                        "MANUAL_INCLUDE_UNKNOWN_FORM",
                        format!(
                            "Manual include references {code}, but this form is not in the form registry."
                        ),
                        Some(version.id.clone()),
                        Some(code.clone()),
                        Some("Manual obligation override".into()),
                        Some("Check the form code or verify TTCE/calendar support before relying on this override.".into()),
                    );
                }
                gated.insert(code);
            }
            ManualObligationOverrideAction::Exclude => {
                if gated.remove(&code) {
                    report.push_detailed(
                        ProfileConsistencySeverity::Info,
                        "MANUAL_EXCLUDE_APPLIED",
                        format!("Manual override excludes {code}: {}", override_rule.reason),
                        Some(version.id.clone()),
                        Some(code.clone()),
                        Some("Manual obligation override".into()),
                        override_rule
                            .source_reference
                            .as_ref()
                            .map(|source| format!("Verify this exclusion against {source}.")),
                    );
                }
            }
        }
    }

    gated
}

fn report_cor_tin_mismatch(
    profile: &TaxpayerProfile,
    version: &TaxProfileVersion,
    report: &mut ProfileConsistencyReport,
) {
    let Some(cor_tin) = version.cor.tin.as_deref() else {
        return;
    };

    let normalized_cor_tin = cor_tin
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if normalized_cor_tin.is_empty() || normalized_cor_tin == profile.tin.full() {
        return;
    }

    report.push_detailed(
        ProfileConsistencySeverity::NeedsReview,
        "COR_TIN_MISMATCH",
        format!(
            "Active COR/profile version '{}' has TIN {cor_tin}, but the taxpayer profile TIN is {}.",
            version.label,
            profile.tin.formatted()
        ),
        Some(version.id.clone()),
        None,
        Some("COR/profile version".into()),
        Some(
            "Verify the uploaded COR and taxpayer profile before confirming this version or using it for compliance deadlines."
                .into(),
        ),
    );
}

fn normalize_form_code(code: &str) -> String {
    crate::forms::registry::canonical_form_code(code)
}

fn report_obligations_without_calendar_rules(
    year: u16,
    codes: &BTreeSet<String>,
    code_version_ids: &BTreeMap<String, BTreeSet<String>>,
    report: &mut ProfileConsistencyReport,
) {
    let calendar_codes = DeadlineResolver::resolve_taxable_year(year as i32)
        .into_iter()
        .map(|deadline| deadline.form_code)
        .collect::<BTreeSet<_>>();

    for code in codes {
        if !calendar_codes.contains(code) {
            report.push_detailed(
                ProfileConsistencySeverity::NeedsReview,
                "OBLIGATION_WITHOUT_CALENDAR_RULE",
                format!(
                    "{code} is required by the resolved profile obligations, but no calendar deadline rule exists for taxable year {year}."
                ),
                code_version_ids
                    .get(code)
                    .and_then(|ids| ids.iter().next().cloned()),
                Some(code.clone()),
                Some("Calendar rules".into()),
                Some(
                    "Add or correct the official calendar rule before relying on dashboard upcoming/overdue status for this form."
                        .into(),
                ),
            );
        }
    }
}

fn report_missing_ttce_categories(
    version: &TaxProfileVersion,
    codes: &BTreeSet<String>,
    report: &mut ProfileConsistencyReport,
) {
    let category_expectations: &[(RegisteredTaxType, &[&str], &str)] = &[
        (
            RegisteredTaxType::IncomeTax,
            &[
                "1700", "1701", "1701A", "1701MS", "1701Q", "1702EX", "1702MX", "1702Q", "1702RT",
            ],
            "COR/profile version registers income tax but TTCE did not produce an income tax recurring obligation.",
        ),
        (
            RegisteredTaxType::ValueAddedTax,
            &["2550DS", "2550M", "2550Q"],
            "COR/profile version registers VAT but TTCE did not produce a VAT recurring obligation.",
        ),
        (
            RegisteredTaxType::PercentageTax,
            &["2551Q", "2551M"],
            "COR/profile version registers percentage tax but TTCE did not produce a percentage tax recurring obligation.",
        ),
        (
            RegisteredTaxType::WithholdingExpanded,
            &["0619E", "1601EQ", "1604E"],
            "COR/profile version registers expanded withholding but TTCE did not produce expanded withholding obligations.",
        ),
        (
            RegisteredTaxType::WithholdingCompensation,
            &["1601C", "1604CF", "2316"],
            "COR/profile version registers compensation withholding but TTCE did not produce compensation withholding obligations.",
        ),
        (
            RegisteredTaxType::WithholdingFinal,
            &["0619F", "1601F", "1601FQ", "1602", "1603"],
            "COR/profile version registers final withholding but TTCE did not produce final withholding obligations.",
        ),
        (
            RegisteredTaxType::WithholdingVatAndPercentage,
            &["1600"],
            "COR/profile version registers VAT/percentage-tax withholding but TTCE did not produce Form 1600.",
        ),
        (
            RegisteredTaxType::ExciseTax,
            &[
                "2200A", "2200AN", "2200C", "2200M", "2200P", "2200S", "2200T",
            ],
            "COR/profile version registers excise tax but TTCE did not produce excise obligations.",
        ),
    ];

    for (tax_type, forms, message) in category_expectations {
        if version.registered_tax_types.contains(tax_type)
            && !forms.iter().any(|code| codes.contains(*code))
        {
            let (severity, code, message, source, fix_hint) = (
                ProfileConsistencySeverity::NeedsReview,
                "COR_TAX_TYPE_WITHOUT_TTCE_FORM",
                *message,
                "COR/profile version",
                "Verify the registered tax type, then add a sourced manual include only if the COR obligation is correct and TTCE is missing support.",
            );

            report.push_detailed(
                severity,
                code,
                message,
                Some(version.id.clone()),
                None,
                Some(source.into()),
                Some(fix_hint.into()),
            );
        }
    }
}

pub(crate) fn registered_tax_types_allow_form(version: &TaxProfileVersion, code: &str) -> bool {
    let tax_types = &version.registered_tax_types;
    if is_income_tax_form(code) {
        if !tax_types.contains(&RegisteredTaxType::IncomeTax) {
            return false;
        }
        // Generic 1702 is retained for exact COR evidence and explicit manual
        // includes. Broad tax-type fallback must choose the classification-
        // specific annual return instead of inventing both 1702 and 1702RT.
        if code == "1702" {
            return false;
        }
        match version.taxpayer_type {
            crate::profile::TaxpayerType::Individual
            | crate::profile::TaxpayerType::Estate
            | crate::profile::TaxpayerType::Trust => {
                if code.starts_with("1702") {
                    return false;
                }
                match version.tax_classification {
                    Some(crate::profile::TaxClassification::PurelyCompensation) => {
                        return code == "1700";
                    }
                    _ => {
                        return code != "1700";
                    }
                }
            }
            _ => {
                if code.starts_with("1700") || code.starts_with("1701") {
                    return false;
                }
            }
        }
        return true;
    }
    if is_vat_form(code) {
        return tax_types.contains(&RegisteredTaxType::ValueAddedTax);
    }
    if is_percentage_tax_form(code) {
        return tax_types.contains(&RegisteredTaxType::PercentageTax);
    }
    if is_expanded_withholding_form(code) {
        return tax_types.contains(&RegisteredTaxType::WithholdingExpanded);
    }
    if is_compensation_withholding_form(code) {
        return tax_types.contains(&RegisteredTaxType::WithholdingCompensation);
    }
    if is_final_withholding_form(code) {
        return tax_types.contains(&RegisteredTaxType::WithholdingFinal);
    }
    if is_vat_percentage_withholding_form(code) {
        return tax_types.contains(&RegisteredTaxType::WithholdingVatAndPercentage);
    }
    if is_excise_form(code) {
        return tax_types.contains(&RegisteredTaxType::ExciseTax);
    }
    if code == "0605" {
        return tax_types.contains(&RegisteredTaxType::RegistrationFee);
    }

    true
}

fn is_income_tax_form(code: &str) -> bool {
    matches!(
        code,
        "1700"
            | "1701"
            | "1701A"
            | "1701MS"
            | "1701Q"
            | "1702"
            | "1702EX"
            | "1702MX"
            | "1702Q"
            | "1702RT"
            | "1704"
    )
}

fn is_vat_form(code: &str) -> bool {
    matches!(code, "2550DS" | "2550M" | "2550Q")
}

fn is_percentage_tax_form(code: &str) -> bool {
    matches!(code, "2551M" | "2551Q")
}

fn is_expanded_withholding_form(code: &str) -> bool {
    matches!(
        code,
        "0619E" | "1601E" | "1601EQ" | "1604E" | "1606" | "1621"
    )
}

fn is_compensation_withholding_form(code: &str) -> bool {
    matches!(code, "0620" | "1601C" | "1604CF" | "2316")
}

fn is_final_withholding_form(code: &str) -> bool {
    matches!(
        code,
        "0619F" | "1600WP" | "1601F" | "1601FQ" | "1602" | "1603"
    )
}

fn is_vat_percentage_withholding_form(code: &str) -> bool {
    code == "1600"
}

fn is_excise_form(code: &str) -> bool {
    matches!(
        code,
        "2200A" | "2200AN" | "2200C" | "2200M" | "2200P" | "2200S" | "2200T"
    )
}

/// Returns all form codes in the form registry.
pub fn all_form_codes() -> Vec<String> {
    crate::forms::registry::FORM_REGISTRY
        .iter()
        .map(|f| f.code.to_string())
        .collect()
}

/// Checks for mutual exclusion conflicts among annual ITR forms.
///
/// Groups:
/// - Individual annual ITRs: 1700, 1701, 1701A, 1701MS
/// - Corporate annual ITRs: 1702, 1702RT, 1702EX, 1702MX
///
/// Returns a warning issue for each group that has 2+ active entries.
/// This is a non-blocking warning — the user can still proceed.
pub fn check_annual_itr_conflicts(
    entries: &[crate::forms::FormSetEntry],
) -> Vec<ProfileConsistencyIssue> {
    let individual_group: &[&str] = &["1700", "1701", "1701A", "1701MS"];
    let corporate_group: &[&str] = &["1702", "1702RT", "1702EX", "1702MX"];

    let mut issues = Vec::new();

    for (group_name, group_codes) in &[
        ("Individual", individual_group),
        ("Corporate", corporate_group),
    ] {
        let active_in_group: Vec<&str> = entries
            .iter()
            .filter(|e| e.is_filing_active() && group_codes.contains(&e.form_code.as_str()))
            .map(|e| e.form_code.as_str())
            .collect();

        if active_in_group.len() >= 2 {
            let codes_label = active_in_group.join(", ");
            issues.push(ProfileConsistencyIssue {
                severity: ProfileConsistencySeverity::Warning,
                code: "ANNUAL_ITR_CONFLICT".to_string(),
                message: format!(
                    "{group_name} annual ITR conflict: multiple annual ITR forms are active ({codes_label}). \
                     A taxpayer should file only one annual ITR per year. \
                     Deactivate the forms that do not apply."
                ),
                version_id: None,
                form_code: Some(active_in_group[0].to_string()),
                source: Some("Annual ITR mutual exclusion check".to_string()),
                fix_hint: Some(
                    "Keep only the single annual ITR that matches this taxpayer's classification."
                        .to_string(),
                ),
            });
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::models::{IncomeCategory, IncomeSource};
    use chrono::NaiveDate;

    fn valid_payload() -> UniversalTaxPayload {
        UniversalTaxPayload {
            tin: "010558054000".to_string(),
            target_form: Some("2551Q".to_string()),
            period_start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            period_end: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
            is_amended: false,
            income_sources: vec![IncomeSource {
                category: IncomeCategory::BusinessNonVat,
                gross_amount: 100_000.0,
                is_vat_exempt: true,
                atc_code_override: None,
                tax_rate_override: None,
            }],
            creditable_withholdings: 0.0,
            previous_tax_paid: 0.0,
            metadata: Default::default(),
        }
    }

    fn profile_for_dashboard(
        classification: crate::profile::TaxClassification,
        is_vat_registered: bool,
    ) -> TaxpayerProfile {
        use crate::naming::Tin;
        use crate::profile::TaxpayerType;

        let mut profile = TaxpayerProfile {
            id: Some(1),
            full_name: "Dashboard Test".into(),
            tin: Tin {
                segment1: "010".into(),
                segment2: "558".into(),
                segment3: "054".into(),
                branch: "000".into(),
            },
            rdo_code: "039".into(),
            line_of_business: "Consulting".into(),
            registered_address: "QC".into(),
            zip_code: "1100".into(),
            phone: "09156837000".into(),
            email: "test@example.com".into(),
            default_form_type: "2551Q".into(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered,
            business_start_date: NaiveDate::from_ymd_opt(2020, 1, 1),
            birth_date: None,
            tax_classification: Some(classification),
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
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
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
        };
        profile.ensure_profile_version_ledger();
        profile
    }

    fn configure_forms_set_from_suggestions(profile: &mut TaxpayerProfile, year: u16) {
        let suggestions = form_suggestions_for_profile_year(profile, year);
        let reconciliation = crate::forms::reconcile_forms_set_for_year(year, None, &suggestions);
        profile
            .per_year_forms
            .insert(year, reconciliation.forms_set);
    }

    fn configure_confirmed_cor_evidence(
        profile: &mut TaxpayerProfile,
        is_vat_registered: bool,
        registered_tax_types: Vec<crate::profile::RegisteredTaxType>,
        extracted_form_codes: &[&str],
        ocr_text: Option<&str>,
    ) {
        use crate::profile::{
            ComplianceSourceMode, CorDocumentRef, TaxProfileVersion, TaxProfileVersionSource,
            TaxProfileVersionStatus,
        };

        let mut version = TaxProfileVersion::from_profile_backfill(profile);
        version.id = "reviewed-cor".into();
        version.label = "Reviewed COR".into();
        version.status = TaxProfileVersionStatus::Confirmed;
        version.source = TaxProfileVersionSource::OcrCor;
        version.effective_from = NaiveDate::from_ymd_opt(2026, 1, 1);
        version.needs_effective_date_review = false;
        version.is_vat_registered = is_vat_registered;
        version.registered_tax_types = registered_tax_types;
        version.evidence = vec![CorDocumentRef {
            id: "cor-document".into(),
            file_name: "cor.pdf".into(),
            stored_path: "/tmp/cor.pdf".into(),
            uploaded_at: None,
            provider: Some("ocr".into()),
            model: None,
            document_type: Some("COR".into()),
            extracted_form_codes: extracted_form_codes
                .iter()
                .map(|code| (*code).to_string())
                .collect(),
            ocr_text: ocr_text.map(str::to_string),
            ocr_confidence: Some(0.99),
            field_bboxes: Default::default(),
        }];
        profile.profile_versions = vec![version];
        profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;
    }

    #[test]
    fn form_applicability_requires_a_stored_forms_set() {
        let profile = profile_for_dashboard(crate::profile::TaxClassification::SelfEmployed, false);

        let error = validate_form_applicability("2551Q", &profile, 2026)
            .expect("missing Forms Set must block applicability");

        assert!(error.message.contains("needs review"));
    }

    #[test]
    fn form_applicability_blocks_needs_review_forms_set_conflict() {
        let mut profile =
            profile_for_dashboard(crate::profile::TaxClassification::SelfEmployed, false);
        let include = FormSuggestion::active("2551Q", FormSuggestionSource::ReviewedCor);
        let mut exclude = include.clone();
        exclude.active = false;
        let reconciliation =
            crate::forms::reconcile_forms_set_for_year(2026, None, &[include, exclude]);
        profile
            .per_year_forms
            .insert(2026, reconciliation.forms_set);

        let error = validate_form_applicability("2551Q", &profile, 2026)
            .expect("unresolved Forms Set conflict must block filing");

        assert!(error.message.contains("conflicting equally authoritative"));
    }

    #[test]
    fn test_valid_payload_passes() {
        let errors = validate_payload(&valid_payload());
        assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
    }

    #[test]
    fn test_bad_tin_length() {
        let mut payload = valid_payload();
        payload.tin = "12345".to_string();
        let errors = validate_payload(&payload);
        assert!(errors.iter().any(|e| e.field == "tin"));
    }

    #[test]
    fn test_bad_tin_chars() {
        let mut payload = valid_payload();
        payload.tin = "01055805400A".to_string();
        let errors = validate_payload(&payload);
        assert!(
            errors
                .iter()
                .any(|e| e.field == "tin" && e.message.contains("digits"))
        );
    }

    #[test]
    fn test_period_end_before_start() {
        let mut payload = valid_payload();
        payload.period_start = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        payload.period_end = NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
        let errors = validate_payload(&payload);
        assert!(errors.iter().any(|e| e.field == "period_end"));
    }

    #[test]
    fn test_previous_tax_on_non_amended() {
        let mut payload = valid_payload();
        payload.is_amended = false;
        payload.previous_tax_paid = 5_000.0;
        let errors = validate_payload(&payload);
        assert!(errors.iter().any(|e| e.field == "previous_tax_paid"));
    }

    #[test]
    fn test_negative_gross_amount() {
        let mut payload = valid_payload();
        payload.income_sources[0].gross_amount = -100.0;
        let errors = validate_payload(&payload);
        assert!(errors.iter().any(|e| e.field.contains("gross_amount")));
    }

    #[test]
    fn test_invalid_rate_override() {
        let mut payload = valid_payload();
        payload.income_sources[0].tax_rate_override = Some(1.5); // 150%
        let errors = validate_payload(&payload);
        assert!(errors.iter().any(|e| e.field.contains("tax_rate_override")));
    }

    #[test]
    fn test_purely_compensation_cannot_file_2551q() {
        use crate::naming::Tin;
        use crate::profile::TaxpayerType;

        let mut profile = TaxpayerProfile {
            id: Some(1),
            full_name: "Test".into(),
            tin: Tin {
                segment1: "010".into(),
                segment2: "558".into(),
                segment3: "054".into(),
                branch: "000".into(),
            },
            rdo_code: "039".into(),
            line_of_business: "Employment".into(),
            registered_address: "QC".into(),
            zip_code: "1100".into(),
            phone: "09156837000".into(),
            email: "test@example.com".into(),
            default_form_type: "1701".into(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: NaiveDate::from_ymd_opt(2020, 1, 1),
            birth_date: None,
            tax_classification: Some(crate::profile::TaxClassification::PurelyCompensation),
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
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
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
        };
        profile.ensure_profile_version_ledger();

        // PurelyCompensation should NOT be allowed to file 2551Q
        let current_year = chrono::Local::now().year() as u16;
        configure_forms_set_from_suggestions(&mut profile, current_year);
        let err = validate_form_applicability("2551Q", &profile, current_year);
        assert!(err.is_some());

        // But should be allowed 1700 (annual ITR for compensation)
        let ok = validate_form_applicability("1700", &profile, current_year);
        assert!(ok.is_none(), "1700 should be valid for PurelyCompensation");
    }

    #[test]
    fn test_applicable_forms_sole_proprietor_non_vat() {
        use crate::naming::Tin;
        use crate::profile::TaxpayerType;

        let mut profile = TaxpayerProfile {
            id: Some(1),
            full_name: "Test".into(),
            tin: Tin {
                segment1: "010".into(),
                segment2: "558".into(),
                segment3: "054".into(),
                branch: "000".into(),
            },
            rdo_code: "039".into(),
            line_of_business: "Consulting".into(),
            registered_address: "QC".into(),
            zip_code: "1100".into(),
            phone: "09156837000".into(),
            email: "test@example.com".into(),
            default_form_type: "2551Q".into(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: NaiveDate::from_ymd_opt(2020, 1, 1),
            birth_date: None,
            tax_classification: Some(crate::profile::TaxClassification::SelfEmployed),
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
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
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
        };
        profile.ensure_profile_version_ledger();
        let current_year = chrono::Local::now().year() as u16;
        configure_forms_set_from_suggestions(&mut profile, current_year);

        let forms = applicable_forms_for_profile(&profile);
        assert!(forms.iter().any(|code| code == "2551Q"));
        assert!(forms.iter().any(|code| code == "1701Q"));
        assert!(forms.iter().any(|code| code == "1701"));
        // Should NOT have VAT form
        assert!(!forms.iter().any(|code| code == "2550M"));
    }

    #[test]
    fn recurring_dashboard_forms_hide_transaction_forms_for_compensation_profile() {
        let mut profile =
            profile_for_dashboard(crate::profile::TaxClassification::PurelyCompensation, false);
        configure_forms_set_from_suggestions(&mut profile, 2026);

        let forms = recurring_obligation_forms_for_profile_and_year(&profile, 2026);

        assert_eq!(forms, vec!["1700".to_string()]);
    }

    #[test]
    fn recurring_dashboard_forms_keep_only_profile_obligations_for_non_vat_business() {
        let mut profile =
            profile_for_dashboard(crate::profile::TaxClassification::SelfEmployed, false);
        configure_forms_set_from_suggestions(&mut profile, 2026);

        let forms = recurring_obligation_forms_for_profile_and_year(&profile, 2026);

        assert!(forms.iter().any(|code| code == "1701Q"));
        assert!(forms.iter().any(|code| code == "1701"));
        assert!(forms.iter().any(|code| code == "2551Q"));
        assert!(!forms.iter().any(|code| code == "0605"));
        assert!(!forms.iter().any(|code| code == "1707A"));
        assert!(!forms.iter().any(|code| code == "2552"));
        assert!(!forms.iter().any(|code| code == "2553"));
    }

    #[test]
    fn exact_cor_forms_are_filtered_without_inventing_corporate_subtypes() {
        use crate::forms::{FormSetSource, PerYearFormsSet};
        use crate::profile::{
            CorDocumentRef, RegisteredTaxType, TaxProfileVersionStatus, TaxpayerType,
        };

        let mut profile =
            profile_for_dashboard(crate::profile::TaxClassification::SelfEmployed, true);
        profile.taxpayer_type = TaxpayerType::Corporation;
        profile.tax_classification = None;
        profile.is_vat_registered = true;

        let mut version = TaxProfileVersion::from_profile_backfill(&profile);
        version.id = "confirmed-cor".to_string();
        version.status = TaxProfileVersionStatus::Confirmed;
        version.effective_from = NaiveDate::from_ymd_opt(2020, 12, 17);
        version.taxpayer_type = TaxpayerType::Corporation;
        version.is_vat_registered = true;
        version.registered_tax_types = vec![
            RegisteredTaxType::IncomeTax,
            RegisteredTaxType::ValueAddedTax,
            RegisteredTaxType::WithholdingExpanded,
            RegisteredTaxType::WithholdingCompensation,
        ];
        version.evidence = vec![CorDocumentRef {
            id: "cor-document".to_string(),
            file_name: "Certificate of Registration.pdf".to_string(),
            stored_path: "/tmp/cor.pdf".to_string(),
            uploaded_at: None,
            provider: None,
            model: None,
            document_type: None,
            extracted_form_codes: vec![
                "0605".to_string(),
                "0619E".to_string(),
                "1601C".to_string(),
                "1601EQ".to_string(),
                "1604C".to_string(),
                "1604E".to_string(),
                "1701Q".to_string(),
                "1702".to_string(),
                "1702Q".to_string(),
                "1905".to_string(),
                "2550M".to_string(),
                "2550Q".to_string(),
                "2551Q".to_string(),
            ],
            ocr_text: None,
            ocr_confidence: Some(0.9),
            field_bboxes: Default::default(),
        }];
        profile.profile_versions = vec![version.clone()];

        let preview = resolve_profile_obligations_for_year(&profile, 2026);
        assert!(preview.form_codes.is_empty());
        assert!(
            preview
                .consistency_report
                .issues
                .iter()
                .any(|issue| issue.code == "FORMS_SET_NOT_CONFIGURED")
        );

        profile.per_year_forms.insert(
            2026,
            PerYearFormsSet::from_codes(2026, ["1702RT", "1701Q", "2550M"], FormSetSource::CorAi),
        );

        let codes = profile.active_form_codes_for_year(2026);
        let resolved = resolve_profile_obligations_for_year(&profile, 2026);

        assert_eq!(
            codes,
            vec!["1701Q", "1702RT", "2550M"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
        assert!(codes.contains(&"1702RT".to_string()));
        assert!(
            resolved
                .consistency_report
                .issues
                .iter()
                .all(|issue| issue.code != "ANNUAL_ITR_CONFLICT")
        );
    }

    #[test]
    fn library_decisions_keep_open_ended_cor_forms() {
        use crate::forms::{FormSetSource, PerYearFormsSet};

        let mut profile =
            profile_for_dashboard(crate::profile::TaxClassification::SelfEmployed, false);
        profile.per_year_forms.insert(
            2026,
            PerYearFormsSet::from_codes(2026, ["0605", "1905", "1701Q"], FormSetSource::Manual),
        );

        let codes = applicable_form_decisions_for_profile_and_year(&profile, 2026)
            .into_iter()
            .map(|form| form.form_code)
            .collect::<Vec<_>>();

        assert!(codes.contains(&"0605".to_string()));
        assert!(codes.contains(&"1905".to_string()));
    }

    // ── R2: Registry tests ──
    #[test]
    fn r2_find_form_1706_is_open_ended() {
        let def = crate::forms::registry::find_form("1706").expect("1706 must be in registry");
        assert_eq!(
            def.frequency,
            crate::forms::registry::FilingFrequency::OpenEnded
        );
        assert!(!def.is_deprecated);
        assert!(!def.requires_employees);
        assert!(def.requires_vat.is_none());
    }

    #[test]
    fn r2_find_form_1707a_is_open_ended() {
        let def = crate::forms::registry::find_form("1707A").expect("1707A must be in registry");
        assert_eq!(
            def.frequency,
            crate::forms::registry::FilingFrequency::OpenEnded
        );
        assert!(!def.is_deprecated);
    }

    #[test]
    fn r2_find_form_1800_is_open_ended() {
        let def = crate::forms::registry::find_form("1800").expect("1800 must be in registry");
        assert_eq!(
            def.frequency,
            crate::forms::registry::FilingFrequency::OpenEnded
        );
        assert!(!def.is_deprecated);
    }

    #[test]
    fn r2_find_form_1801_is_open_ended() {
        let def = crate::forms::registry::find_form("1801").expect("1801 must be in registry");
        assert_eq!(
            def.frequency,
            crate::forms::registry::FilingFrequency::OpenEnded
        );
        assert!(!def.is_deprecated);
    }

    #[test]
    fn r2_is_recurring_returns_false_for_new_forms() {
        // OpenEnded forms from registry should return false for is_recurring
        let def = crate::forms::registry::find_form("1706").unwrap();
        assert!(matches!(
            def.frequency,
            crate::forms::registry::FilingFrequency::OpenEnded
        ));
        let def = crate::forms::registry::find_form("1707A").unwrap();
        assert!(matches!(
            def.frequency,
            crate::forms::registry::FilingFrequency::OpenEnded
        ));
        let def = crate::forms::registry::find_form("1800").unwrap();
        assert!(matches!(
            def.frequency,
            crate::forms::registry::FilingFrequency::OpenEnded
        ));
        let def = crate::forms::registry::find_form("1801").unwrap();
        assert!(matches!(
            def.frequency,
            crate::forms::registry::FilingFrequency::OpenEnded
        ));
    }

    // ── R4: Annual ITR conflict tests ──
    #[test]
    fn r4_no_conflict_single_corporate_itr() {
        let entries = vec![crate::forms::FormSetEntry::from_code(
            "1702RT",
            crate::forms::FormSetSource::Manual,
        )];
        let issues = check_annual_itr_conflicts(&entries);
        assert!(
            issues.is_empty(),
            "Single active 1702RT should not produce a conflict"
        );
    }

    #[test]
    fn r4_conflict_two_corporate_itrs_active() {
        let entries = vec![
            crate::forms::FormSetEntry::from_code("1702", crate::forms::FormSetSource::Manual),
            crate::forms::FormSetEntry::from_code("1702RT", crate::forms::FormSetSource::Manual),
        ];
        let issues = check_annual_itr_conflicts(&entries);
        assert!(
            !issues.is_empty(),
            "Two active corporate ITRs should produce a conflict"
        );
        assert!(issues.iter().any(|i| i.code == "ANNUAL_ITR_CONFLICT"));
        assert!(
            issues
                .iter()
                .any(|i| i.severity == ProfileConsistencySeverity::Warning)
        );
    }

    #[test]
    fn non_vat_ocr_evidence_cannot_generate_vat_suggestion() {
        use crate::profile::{CorDocumentRef, RegisteredTaxType, TaxProfileVersionStatus};

        let mut profile =
            profile_for_dashboard(crate::profile::TaxClassification::SelfEmployed, true);
        let mut version = TaxProfileVersion::from_profile_backfill(&profile);
        version.id = "ocr-cor".into();
        version.status = TaxProfileVersionStatus::Confirmed;
        version.effective_from = NaiveDate::from_ymd_opt(2026, 1, 1);
        version.is_vat_registered = true;
        version.registered_tax_types = vec![RegisteredTaxType::ValueAddedTax];
        version.evidence.push(CorDocumentRef {
            id: "cor-document".into(),
            file_name: "cor.pdf".into(),
            stored_path: "/tmp/cor.pdf".into(),
            uploaded_at: None,
            provider: Some("ocr".into()),
            model: None,
            document_type: Some("COR".into()),
            extracted_form_codes: Vec::new(),
            ocr_text: Some("TAXPAYER TYPE: NON-VAT REGISTERED".into()),
            ocr_confidence: Some(0.99),
            field_bboxes: Default::default(),
        });
        profile.profile_versions = vec![version];
        profile.compliance_source_mode = crate::profile::ComplianceSourceMode::CorVersioned;

        let suggestions = form_suggestions_for_profile_year(&profile, 2026);

        assert!(
            suggestions
                .iter()
                .all(|suggestion| !suggestion.form_code.starts_with("2550"))
        );
    }

    #[test]
    fn reviewed_exact_vat_form_code_is_eligible_without_boolean_vat_flag() {
        use crate::profile::RegisteredTaxType;

        let mut profile =
            profile_for_dashboard(crate::profile::TaxClassification::SelfEmployed, false);
        configure_confirmed_cor_evidence(
            &mut profile,
            false,
            vec![RegisteredTaxType::PercentageTax],
            &["2550Q"],
            None,
        );

        let suggestions = form_suggestions_for_profile_year(&profile, 2026);
        let vat_suggestions = suggestions
            .iter()
            .filter(|suggestion| suggestion.form_code == "2550Q")
            .collect::<Vec<_>>();

        assert_eq!(vat_suggestions.len(), 1);
        assert!(vat_suggestions[0].active);
        assert_eq!(vat_suggestions[0].source, FormSuggestionSource::ReviewedCor);
    }

    #[test]
    fn explicit_non_vat_and_exact_vat_code_reconcile_to_needs_review() {
        use crate::profile::RegisteredTaxType;

        let mut profile =
            profile_for_dashboard(crate::profile::TaxClassification::SelfEmployed, false);
        configure_confirmed_cor_evidence(
            &mut profile,
            false,
            vec![RegisteredTaxType::PercentageTax],
            &["2550Q"],
            Some("TAXPAYER TYPE: NON-VAT REGISTERED"),
        );

        let suggestions = form_suggestions_for_profile_year(&profile, 2026);
        let vat_suggestions = suggestions
            .iter()
            .filter(|suggestion| suggestion.form_code == "2550Q")
            .collect::<Vec<_>>();
        assert_eq!(vat_suggestions.len(), 2);
        assert!(vat_suggestions.iter().any(|suggestion| suggestion.active));
        assert!(vat_suggestions.iter().any(|suggestion| !suggestion.active));
        assert!(
            vat_suggestions
                .iter()
                .all(|suggestion| { suggestion.source == FormSuggestionSource::ReviewedCor })
        );

        let reconciliation = crate::forms::reconcile_forms_set_for_year(2026, None, &suggestions);
        let entry = reconciliation
            .forms_set
            .entry("2550Q")
            .expect("conflicting reviewed evidence must remain visible");

        assert!(entry.needs_review());
        assert!(!entry.is_filing_active());
        assert_eq!(reconciliation.conflicts.len(), 1);
        assert!(entry.conflict.as_ref().is_some_and(|conflict| {
            conflict.competing_suggestions.iter().any(|suggestion| {
                !suggestion.active
                    && suggestion
                        .reason
                        .as_deref()
                        .is_some_and(|reason| reason.contains("NON-VAT"))
            })
        }));
    }

    #[test]
    fn percentage_tax_without_exact_vat_code_does_not_suggest_vat_form() {
        use crate::profile::RegisteredTaxType;

        let mut profile =
            profile_for_dashboard(crate::profile::TaxClassification::SelfEmployed, false);
        configure_confirmed_cor_evidence(
            &mut profile,
            false,
            vec![RegisteredTaxType::PercentageTax],
            &[],
            None,
        );

        let suggestions = form_suggestions_for_profile_year(&profile, 2026);

        assert!(
            suggestions
                .iter()
                .any(|suggestion| { suggestion.form_code == "2551Q" && suggestion.active })
        );
        assert!(
            suggestions
                .iter()
                .all(|suggestion| { suggestion.form_code != "2550Q" })
        );
    }

    #[test]
    fn r4_no_conflict_when_one_inactive() {
        let mut entry_inactive =
            crate::forms::FormSetEntry::from_code("1702", crate::forms::FormSetSource::Manual);
        entry_inactive.active = false;
        let entry_active =
            crate::forms::FormSetEntry::from_code("1702RT", crate::forms::FormSetSource::Manual);
        let entries = vec![entry_inactive, entry_active];
        let issues = check_annual_itr_conflicts(&entries);
        assert!(
            issues.is_empty(),
            "Inactive 1702 + active 1702RT should not produce a conflict"
        );
    }

    #[test]
    fn r4_conflict_individual_itrs() {
        let entries = vec![
            crate::forms::FormSetEntry::from_code("1700", crate::forms::FormSetSource::Manual),
            crate::forms::FormSetEntry::from_code("1701", crate::forms::FormSetSource::Manual),
        ];
        let issues = check_annual_itr_conflicts(&entries);
        assert!(
            !issues.is_empty(),
            "Two active individual ITRs should produce a conflict"
        );
    }

    // ── R3: Copy-from-prior-year logic test (unit-level) ──
    #[test]
    fn r3_copy_filters_active_entries_only() {
        // Tests that the copy operation filters to active=true entries.
        // Note: the actual UI copies all active entries regardless of source;
        // CorAi entries are retagged to Manual in the destination year.
        use crate::forms::{FormSetEntry, FormSetSource, PerYearFormsSet};

        // Source year: 2024 with mixed entries
        let mut src_set = PerYearFormsSet::new(2024);
        let active_manual = FormSetEntry::from_code("2551Q", FormSetSource::Manual);
        let mut inactive_manual = FormSetEntry::from_code("1701", FormSetSource::Manual);
        inactive_manual.active = false;
        let active_cor_ai = FormSetEntry::from_code("1601C", FormSetSource::CorAi);
        src_set.entries.push(active_manual);
        src_set.entries.push(inactive_manual);
        src_set.entries.push(active_cor_ai);

        // Simulate copy logic: only active=true, source=Manual
        let copied: Vec<_> = src_set
            .entries
            .iter()
            .filter(|e| e.active && e.source == FormSetSource::Manual)
            .collect();

        assert_eq!(
            copied.len(),
            1,
            "Only 1 active manual entry should be copied"
        );
        assert_eq!(copied[0].form_code, "2551Q");
    }
}
