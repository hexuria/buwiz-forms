//! Integration payload validation — pre-flight checks before mapping.
//!
//! Validates the `UniversalTaxPayload` structure, period consistency,
//! and profile compatibility before the mapper engine runs.
//!
//! **Form eligibility is evaluated exclusively through the temporal engine.**
//! There is no separate hardcoded eligibility matrix in this module.

use crate::calendar_rules::{
    DeadlineOverride, DeadlinePeriod, DeadlineResolver, ResolvedTaxDeadline, canonical_form_code,
};
use crate::forms::registry::FilingFrequency;
use crate::integration::models::UniversalTaxPayload;
use crate::profile::{
    ManualObligationOverrideAction, RegisteredTaxType, TaxProfileVersion, TaxpayerProfile,
};
use crate::temporal::FormDecision;
use crate::temporal::snapshot_loader::compiled_snapshot;
use chrono::{Datelike, NaiveDate};
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
        if source.gross_amount < 0.0 {
            errors.push(PayloadValidationError::new(
                format!("income_sources[{i}].gross_amount"),
                "Gross amount must be non-negative",
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

    if payload.creditable_withholdings < 0.0 {
        errors.push(PayloadValidationError::new(
            "creditable_withholdings",
            "Creditable withholdings must be non-negative",
        ));
    }

    errors
}

/// Validate that a target form is applicable for the given profile and year.
///
/// Delegates to the temporal engine so there is a single source of truth
/// for form eligibility. No separate hardcoded matrix.
pub fn validate_form_applicability(
    form_code: &str,
    profile: &TaxpayerProfile,
    taxable_year: u16,
) -> Option<PayloadValidationError> {
    // Check if the form exists in the compiled temporal snapshot.
    if !compiled_snapshot().has_form_code(form_code) {
        return Some(PayloadValidationError::new(
            "target_form",
            format!("Unknown form code: {form_code}"),
        ));
    }

    // Delegate to the temporal engine for the explicit year
    let engine = crate::temporal::TemporalEngine::default();
    let visible = engine.visible_form_codes(profile, taxable_year);

    if !visible.iter().any(|c| c == form_code) {
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

/// Evaluates a taxpayer profile against the form registry using the temporal engine.
/// Returns detailed decisions with reasons and legal authorities for all applicable forms.
///
/// This is the single canonical form suggestion path. It delegates to the temporal
/// engine rather than maintaining a separate hardcoded eligibility matrix.
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
    let engine = crate::temporal::TemporalEngine::default();
    let context = crate::temporal::TemporalContext::current_compliance(taxable_year);
    let decisions = engine.evaluate_with_context(profile, &context);

    decisions
        .into_iter()
        .map(|d| {
            let legal_authority_citation = if d.legal_citations.is_empty() {
                "Standard BIR Filing Rules".to_string()
            } else {
                d.legal_citations
                    .iter()
                    .map(|c| format!("{} {}", c.number, c.section))
                    .collect::<Vec<_>>()
                    .join("; ")
            };

            let reason = if let Some(r) = d.eligibility.reason() {
                r.to_string()
            } else if d.eligibility.is_visible() {
                "Applicable based on taxpayer classification".to_string()
            } else {
                "Not applicable".to_string()
            };

            FormSuggestionDecision {
                form_code: d.form_code,
                is_suggested: d.eligibility.is_visible(),
                reason,
                legal_authority_citation,
            }
        })
        .collect()
}

/// Returns the list of applicable form codes for a profile, considering
/// both `TaxpayerType` and `TaxClassification`.
///
/// Delegates to the temporal engine for the current year.
pub fn applicable_forms_for_profile(profile: &TaxpayerProfile) -> Vec<String> {
    let current_year = chrono::Local::now().year() as u16;
    applicable_forms_for_profile_and_year(profile, current_year)
}

/// Returns the list of applicable form codes for a profile in a specific year.
///
/// Uses the temporal engine for era-aware evaluation. This is the preferred API
/// for the dashboard where the user selects a tax year.
pub fn applicable_forms_for_profile_and_year(profile: &TaxpayerProfile, year: u16) -> Vec<String> {
    let engine = crate::temporal::TemporalEngine::default();
    engine.visible_form_codes(profile, year)
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
) -> Vec<FormDecision> {
    let obligations = resolve_profile_obligations_for_year(profile, year);
    let wanted: BTreeSet<_> = obligations.form_codes.into_iter().collect();
    let mut by_code = BTreeMap::new();

    for version in profile.active_profile_versions_for_year(year) {
        let projected = profile.projection_for_version(&version);
        for decision in evaluated_recurring_decisions_for_projection(&projected, year) {
            if wanted.contains(&decision.form_code) {
                by_code
                    .entry(decision.form_code.clone())
                    .or_insert(decision);
            }
        }
    }

    by_code.into_values().collect()
}

pub fn resolve_profile_obligations_for_year(
    profile: &TaxpayerProfile,
    year: u16,
) -> ResolvedProfileObligations {
    let mut all_codes = BTreeSet::new();
    let mut active_version_ids = Vec::new();
    let mut code_version_ids: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut consistency_report = ProfileConsistencyReport::default();

    for version in profile.active_profile_versions_for_year(year) {
        active_version_ids.push(version.id.clone());
        let projected = profile.projection_for_version(&version);
        let version_codes = recurring_obligation_codes_for_version(
            &projected,
            &version,
            year,
            &mut consistency_report,
        );
        for code in &version_codes {
            code_version_ids
                .entry(code.clone())
                .or_default()
                .insert(version.id.clone());
        }
        all_codes.extend(version_codes);
    }

    report_obligations_without_calendar_rules(
        year,
        &all_codes,
        &code_version_ids,
        &mut consistency_report,
    );

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
    let Some((period_start, period_end)) = deadline_period_bounds(deadline) else {
        return false;
    };

    profile
        .active_profile_versions_for_period(period_start, period_end)
        .into_iter()
        .any(|version| {
            let projected = profile.projection_for_version(&version);
            let mut report = ProfileConsistencyReport::default();
            recurring_obligation_codes_for_version(
                &projected,
                &version,
                taxable_year as u16,
                &mut report,
            )
            .contains(&deadline.form_code)
        })
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

fn evaluated_recurring_decisions_for_projection(
    profile: &TaxpayerProfile,
    year: u16,
) -> Vec<FormDecision> {
    let engine = crate::temporal::TemporalEngine::default();
    let context = crate::temporal::TemporalContext::current_compliance(year);

    engine
        .evaluate_with_context(profile, &context)
        .into_iter()
        .filter(is_recurring_profile_obligation)
        .collect()
}

fn recurring_obligation_codes_for_version(
    projected: &TaxpayerProfile,
    version: &TaxProfileVersion,
    year: u16,
    report: &mut ProfileConsistencyReport,
) -> BTreeSet<String> {
    let engine = crate::temporal::TemporalEngine::default();
    let context = crate::temporal::TemporalContext::current_compliance(year);
    let all_decisions = engine.evaluate_with_context(projected, &context);
    let recurring_visible: BTreeSet<String> = all_decisions
        .iter()
        .filter(|decision| {
            decision.eligibility.is_visible() && is_recurring_profile_obligation(decision)
        })
        .map(|decision| decision.form_code.clone())
        .collect();

    let has_cor_gate = !version.registered_tax_types.is_empty();
    let mut gated = BTreeSet::new();

    for code in &recurring_visible {
        if !has_cor_gate || registered_tax_types_allow_form(&version.registered_tax_types, code) {
            gated.insert(code.clone());
        } else {
            report.push_detailed(
                ProfileConsistencySeverity::Warning,
                "TTCE_COR_TAX_TYPE_MISMATCH",
                format!(
                    "TTCE suggests {code}, but active COR/profile version '{}' does not register the matching tax type.",
                    version.label
                ),
                Some(version.id.clone()),
                Some(code.clone()),
                Some("TTCE".into()),
                Some(
                    "If the COR is correct, add a sourced manual exclude. If TTCE is correct, update the registered tax types on this COR version."
                        .into(),
                ),
            );
        }
    }

    if has_cor_gate {
        report_missing_ttce_categories(projected, version, year, &gated, report);
    }

    for override_rule in &version.obligation_overrides {
        let code = normalize_form_code(&override_rule.form_code);
        match override_rule.action {
            ManualObligationOverrideAction::Include => {
                if !compiled_snapshot().has_form_code(&code) {
                    report.push_detailed(
                        ProfileConsistencySeverity::NeedsReview,
                        "MANUAL_INCLUDE_UNKNOWN_FORM",
                        format!(
                            "Manual include references {code}, but this form is not in the temporal snapshot."
                        ),
                        Some(version.id.clone()),
                        Some(code.clone()),
                        Some("Manual obligation override".into()),
                        Some("Check the form code or add TTCE/calendar support before relying on this override.".into()),
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

fn deadline_period_bounds(deadline: &ResolvedTaxDeadline) -> Option<(NaiveDate, NaiveDate)> {
    match deadline.period {
        DeadlinePeriod::Monthly { .. }
        | DeadlinePeriod::Quarterly { .. }
        | DeadlinePeriod::Annual { .. } => Some((deadline.period_start?, deadline.period_end?)),
        DeadlinePeriod::EventBased => None,
    }
}

fn normalize_form_code(code: &str) -> String {
    let canonical = canonical_form_code(code);
    if canonical == "UNKNOWN" {
        code.to_string()
    } else {
        canonical.to_string()
    }
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
    projected: &TaxpayerProfile,
    version: &TaxProfileVersion,
    year: u16,
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
            let (severity, code, message, source, fix_hint) = if tax_type
                == &RegisteredTaxType::PercentageTax
                && projected.has_8_percent_election(year)
            {
                (
                    ProfileConsistencySeverity::Info,
                    "PERCENTAGE_TAX_SUPPRESSED_BY_8_PERCENT",
                    "COR/profile version registers percentage tax, but this taxable year has an 8% income tax election so TTCE suppresses percentage tax forms.",
                    "Income tax election + TTCE",
                    "If the 8% election is wrong for this year, update the income tax election ledger; otherwise this suppression is expected.",
                )
            } else {
                (
                    ProfileConsistencySeverity::NeedsReview,
                    "COR_TAX_TYPE_WITHOUT_TTCE_FORM",
                    *message,
                    "COR/profile version",
                    "Verify the registered tax type, then add a sourced manual include only if the COR obligation is correct and TTCE is missing support.",
                )
            };

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

fn registered_tax_types_allow_form(tax_types: &[RegisteredTaxType], code: &str) -> bool {
    if is_income_tax_form(code) {
        return tax_types.contains(&RegisteredTaxType::IncomeTax);
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
    matches!(code, "0619E" | "1601EQ" | "1604E" | "1606" | "1621")
}

fn is_compensation_withholding_form(code: &str) -> bool {
    matches!(code, "0620" | "1600" | "1601C" | "1604CF" | "2316")
}

fn is_final_withholding_form(code: &str) -> bool {
    matches!(
        code,
        "0619F" | "1600WP" | "1601F" | "1601FQ" | "1602" | "1603"
    )
}

fn is_excise_form(code: &str) -> bool {
    matches!(
        code,
        "2200A" | "2200AN" | "2200C" | "2200M" | "2200P" | "2200S" | "2200T"
    )
}

fn is_recurring_profile_obligation(decision: &FormDecision) -> bool {
    if matches!(decision.frequency, FilingFrequency::OpenEnded) {
        return false;
    }

    !matches!(
        decision.form_code.as_str(),
        // Transaction/special-law forms require a separate triggering event that
        // is not represented by the taxpayer profile configuration.
        "1707A" | "2552" | "2553"
    )
}

/// Returns all form codes known by the compiled temporal snapshot.
pub fn all_form_codes() -> Vec<String> {
    compiled_snapshot().form_codes()
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

        TaxpayerProfile {
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
            business_start_date: None,
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
        }
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

        let profile = TaxpayerProfile {
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
            business_start_date: None,
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
        };

        // PurelyCompensation should NOT be allowed to file 2551Q
        let current_year = chrono::Local::now().year() as u16;
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

        let profile = TaxpayerProfile {
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
            business_start_date: None,
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
        };

        let forms = applicable_forms_for_profile(&profile);
        assert!(forms.iter().any(|code| code == "2551Q"));
        assert!(forms.iter().any(|code| code == "1701Q"));
        assert!(forms.iter().any(|code| code == "1701"));
        // Should NOT have VAT form
        assert!(!forms.iter().any(|code| code == "2550M"));
    }

    #[test]
    fn recurring_dashboard_forms_hide_transaction_forms_for_compensation_profile() {
        let profile =
            profile_for_dashboard(crate::profile::TaxClassification::PurelyCompensation, false);

        let forms = recurring_obligation_forms_for_profile_and_year(&profile, 2026);

        assert_eq!(forms, vec!["1700".to_string()]);
    }

    #[test]
    fn recurring_dashboard_forms_keep_only_profile_obligations_for_non_vat_business() {
        let profile = profile_for_dashboard(crate::profile::TaxClassification::SelfEmployed, false);

        let forms = recurring_obligation_forms_for_profile_and_year(&profile, 2026);

        assert!(forms.iter().any(|code| code == "1701Q"));
        assert!(forms.iter().any(|code| code == "1701"));
        assert!(forms.iter().any(|code| code == "2551Q"));
        assert!(!forms.iter().any(|code| code == "0605"));
        assert!(!forms.iter().any(|code| code == "1707A"));
        assert!(!forms.iter().any(|code| code == "2552"));
        assert!(!forms.iter().any(|code| code == "2553"));
    }
}
