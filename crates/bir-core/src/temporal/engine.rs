//! Temporal Engine — the core evaluation loop (v3: snapshot + EligibilityFacts).
//!
//! Evaluates form eligibility exclusively from the compiled snapshot.
//! Rules receive `EligibilityFacts` (derived from the profile) and
//! `FormArtifact` (from the snapshot) — never raw profile or legacy registry.
//!
//! No `Local::now()` calls. Pure input → output.

use crate::profile::TaxpayerProfile;
use crate::temporal::context::TemporalContext;
use crate::temporal::eligibility::{ComplianceState, FormDecision, RuleApplication};
use crate::temporal::eligibility_facts::EligibilityFacts;
use crate::temporal::forms::{ArtifactLifecycle, FormArtifact};
use crate::temporal::rules::all_rules;
use crate::temporal::snapshot_loader::compiled_snapshot;
use crate::temporal::traits::TaxRule;

/// The temporal tax form engine.
///
/// Evaluates form eligibility for a given profile and temporal context,
/// applying era-scoped rules and producing auditable decisions.
pub struct TemporalEngine {
    rules: Vec<Box<dyn TaxRule>>,
}

impl Default for TemporalEngine {
    fn default() -> Self {
        Self { rules: all_rules() }
    }
}

impl TemporalEngine {
    /// Evaluate all forms against a profile for a specific temporal context.
    ///
    /// Iterates the compiled snapshot's `FormArtifact` entries (not the legacy
    /// registry). Derives `EligibilityFacts` once and passes them to all rules.
    pub fn evaluate_with_context(
        &self,
        profile: &TaxpayerProfile,
        context: &TemporalContext,
    ) -> Vec<FormDecision> {
        let snapshot = compiled_snapshot();
        let target_year = context.taxable_year;

        // Derive eligibility facts once — the single bridge from profile to engine
        let facts = EligibilityFacts::from_profile(profile);

        // Get active rules for the target year
        let active_rules: Vec<_> = self
            .rules
            .iter()
            .filter(|r| r.is_active_for_year(target_year))
            .collect();

        let mut decisions = Vec::new();

        for artifact in &snapshot.form_artifacts {
            let mut audit_log = Vec::new();

            // Resolve legal citations from the snapshot
            let mut citations: Vec<_> = artifact
                .legal_citations
                .iter()
                .filter_map(|cid| snapshot.find_citation(cid).cloned())
                .collect();

            // Step 1: Timeline check. Out-of-window artifacts still produce
            // decisions so inspector/admin views can explain why they are hidden.
            let mut state = Self::timeline_state_for_artifact(artifact, target_year);
            if state != ComplianceState::Applicable {
                audit_log.push(Self::timeline_rule_application(&state));
            }

            // Step 2: Entity type quick-filter. Timeline-hidden artifacts keep
            // their timeline reason instead of being overwritten by profile facts.
            if state.is_visible() && !artifact.taxpayer_types.contains(&facts.taxpayer_type) {
                state = ComplianceState::Suppressed("Entity type mismatch".into());
            }

            // Step 3: Apply active rules (only if form survived timeline/entity filters)
            if state.is_visible() {
                for rule in &active_rules {
                    let prev = state.clone();
                    state = rule.evaluate(&facts, artifact, state, target_year);
                    if state != prev {
                        audit_log.push(RuleApplication {
                            rule_name: rule.name().to_string(),
                            law: rule.law().to_string(),
                            before: prev,
                            after: state.clone(),
                            reason: state.reason().unwrap_or("").to_string(),
                        });
                        citations.push(rule.citation());
                    }
                }
            }

            // Resolve snapshot formula and rate table metadata
            let formula_id = artifact.formula_ref.clone();
            let rate_table_ids = formula_id
                .as_ref()
                .and_then(|fid| {
                    snapshot
                        .formulas
                        .iter()
                        .find(|f| &f.formula_id == fid)
                        .map(|f| f.rate_table_refs.clone())
                })
                .unwrap_or_default();

            decisions.push(FormDecision {
                form_code: artifact.form_code.clone(),
                title: artifact.title.clone(),
                category: artifact.category.clone(),
                frequency: artifact.frequency.clone(),
                eligibility: state,
                audit_log,
                legal_citations: citations,
                artifact_id: Some(artifact.artifact_id.clone()),
                formula_id: artifact.formula_ref.clone(),
                rate_table_ids,
            });
        }

        // Sort by compliance state rank for deterministic output
        decisions.sort_by(|a, b| {
            a.eligibility
                .sort_rank()
                .cmp(&b.eligibility.sort_rank())
                .then(a.form_code.cmp(&b.form_code))
        });

        decisions
    }

    fn timeline_state_for_artifact(artifact: &FormArtifact, target_year: u16) -> ComplianceState {
        let from_year = artifact.effective_from_year();

        if target_year < from_year {
            return ComplianceState::IllegalForPeriod(format!(
                "Form {} is not effective until {}",
                artifact.form_code, from_year
            ));
        }

        if let Some(until_year) = artifact.effective_until_year() {
            if target_year > until_year {
                return match &artifact.lifecycle {
                    ArtifactLifecycle::Abolished => ComplianceState::Deprecated(format!(
                        "Form {} was abolished after {}",
                        artifact.form_code, until_year
                    )),
                    ArtifactLifecycle::Deprecated => ComplianceState::Deprecated(format!(
                        "Form {} is deprecated after {}",
                        artifact.form_code, until_year
                    )),
                    _ => ComplianceState::Archived(format!(
                        "Form {} revision {} is not effective after {}",
                        artifact.form_code, artifact.revision, until_year
                    )),
                };
            }
        }

        ComplianceState::Applicable
    }

    fn timeline_rule_application(state: &ComplianceState) -> RuleApplication {
        RuleApplication {
            rule_name: "Timeline Window".to_string(),
            law: "CompiledRuleSnapshot effective dates".to_string(),
            before: ComplianceState::Applicable,
            after: state.clone(),
            reason: state.reason().unwrap_or("").to_string(),
        }
    }

    /// Returns only form codes that are visible (suggested) for the given context.
    pub fn visible_form_codes_for_context(
        &self,
        profile: &TaxpayerProfile,
        context: &TemporalContext,
    ) -> Vec<String> {
        self.evaluate_with_context(profile, context)
            .into_iter()
            .filter(|d| d.eligibility.is_visible())
            .map(|d| d.form_code)
            .collect()
    }

    /// Returns the compiled snapshot metadata for debugging.
    pub fn snapshot_info(&self) -> (&str, &str, usize) {
        let s = compiled_snapshot();
        (&s.snapshot_id, &s.content_hash, s.form_artifacts.len())
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Legacy API (preserved for backward compat)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Evaluate all forms against a profile for a specific tax year.
    ///
    /// This is the legacy API. New code should use `evaluate_with_context`.
    pub fn evaluate(&self, profile: &TaxpayerProfile, target_year: u16) -> Vec<FormDecision> {
        let context = TemporalContext::current_compliance(target_year);
        self.evaluate_with_context(profile, &context)
    }

    /// Returns only form codes that are visible (suggested) for the given profile + year.
    pub fn visible_form_codes(&self, profile: &TaxpayerProfile, target_year: u16) -> Vec<String> {
        self.evaluate(profile, target_year)
            .into_iter()
            .filter(|d| d.eligibility.is_visible())
            .map(|d| d.form_code)
            .collect()
    }
}

/// Resolve tax rates for a given context from the compiled snapshot.
///
/// Returns matching rate tables for the given tax type and year.
pub fn resolve_rates(
    context: &TemporalContext,
    tax_type: &str,
) -> Vec<&'static crate::temporal::rates::TaxRateTable> {
    let snapshot = compiled_snapshot();
    snapshot
        .rate_tables
        .iter()
        .filter(|t| t.tax_type == tax_type && t.is_effective_for_year(context.taxable_year))
        .collect()
}

/// Resolve a specific rate table by ID from the compiled snapshot.
pub fn resolve_rate_table(table_id: &str) -> Option<&'static crate::temporal::rates::TaxRateTable> {
    let snapshot = compiled_snapshot();
    snapshot.rate_tables.iter().find(|t| t.table_id == table_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;
    use crate::profile::{TaxClassification, TaxpayerType};

    fn test_profile(
        taxpayer_type: TaxpayerType,
        classification: Option<TaxClassification>,
    ) -> TaxpayerProfile {
        TaxpayerProfile {
            id: Some(1),
            full_name: "Test Taxpayer".into(),
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
            taxpayer_type,
            is_vat_registered: false,
            business_start_date: None,
            tax_classification: classification,
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
    fn test_evaluate_with_context_returns_decisions() {
        let engine = TemporalEngine::default();
        let profile = test_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::SelfEmployed),
        );
        let context = TemporalContext::current_compliance(2024);

        let decisions = engine.evaluate_with_context(&profile, &context);
        assert!(!decisions.is_empty(), "Should produce form decisions");

        // 2551Q should be visible for a non-VAT sole proprietor
        let q2551 = decisions.iter().find(|d| d.form_code == "2551Q");
        assert!(q2551.is_some(), "2551Q should be in decisions");
        assert!(
            q2551.unwrap().eligibility.is_visible(),
            "2551Q should be visible"
        );
    }

    #[test]
    fn test_context_driven_matches_legacy() {
        let engine = TemporalEngine::default();
        let profile = test_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::SelfEmployed),
        );

        let legacy_codes = engine.visible_form_codes(&profile, 2024);
        let context = TemporalContext::current_compliance(2024);
        let context_codes = engine.visible_form_codes_for_context(&profile, &context);

        assert_eq!(
            legacy_codes, context_codes,
            "Context-driven API should match legacy API"
        );
    }

    #[test]
    fn test_pre_train_era_shows_2551m() {
        let engine = TemporalEngine::default();
        let profile = test_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::SelfEmployed),
        );
        let context = TemporalContext::current_compliance(2017);

        let codes = engine.visible_form_codes_for_context(&profile, &context);
        // 2551M should be visible in pre-TRAIN era
        assert!(
            codes.contains(&"2551M".to_string()),
            "2551M should be visible in 2017. Got: {:?}",
            codes
        );
    }

    #[test]
    fn test_train_era_hides_2551m() {
        let engine = TemporalEngine::default();
        let profile = test_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::SelfEmployed),
        );
        let context = TemporalContext::current_compliance(2020);

        let codes = engine.visible_form_codes_for_context(&profile, &context);
        // 2551M should be hidden in TRAIN era
        assert!(
            !codes.contains(&"2551M".to_string()),
            "2551M should be hidden in 2020. Got: {:?}",
            codes
        );
    }

    #[test]
    fn test_deterministic_output() {
        let engine = TemporalEngine::default();
        let profile = test_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::SelfEmployed),
        );
        let context = TemporalContext::current_compliance(2024);

        let run1 = engine.evaluate_with_context(&profile, &context);
        let run2 = engine.evaluate_with_context(&profile, &context);

        let codes1: Vec<_> = run1.iter().map(|d| &d.form_code).collect();
        let codes2: Vec<_> = run2.iter().map(|d| &d.form_code).collect();
        assert_eq!(codes1, codes2, "Same input should produce same output");
    }

    #[test]
    fn test_snapshot_enrichment() {
        let engine = TemporalEngine::default();
        let profile = test_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::SelfEmployed),
        );
        let context = TemporalContext::current_compliance(2024);

        let decisions = engine.evaluate_with_context(&profile, &context);
        let q2551 = decisions.iter().find(|d| d.form_code == "2551Q").unwrap();

        // Should have snapshot artifact enrichment
        assert!(
            q2551.artifact_id.is_some(),
            "2551Q should have an artifact_id from snapshot"
        );
        assert!(
            q2551.formula_id.is_some(),
            "2551Q should have a formula_id from snapshot"
        );
        assert!(
            !q2551.rate_table_ids.is_empty(),
            "2551Q should have rate_table_ids from snapshot"
        );
    }

    #[test]
    fn test_resolve_percentage_tax_rates() {
        let context = TemporalContext::current_compliance(2024);
        let tables = resolve_rates(&context, "PercentageTax");
        assert!(
            !tables.is_empty(),
            "Should find percentage tax rate tables for 2024"
        );
        let rate = tables[0].find_rate("default").unwrap();
        assert_eq!(rate.rate_f64(), 0.03, "Percentage tax rate should be 3%");
    }

    #[test]
    fn test_resolve_cit_rates() {
        let context = TemporalContext::current_compliance(2024);
        let tables = resolve_rates(&context, "CIT");
        assert!(!tables.is_empty(), "Should find CIT rate tables for 2024");
    }

    #[test]
    fn test_snapshot_info() {
        let engine = TemporalEngine::default();
        let (id, hash, artifact_count) = engine.snapshot_info();
        assert!(!id.is_empty());
        assert!(!hash.is_empty());
        assert!(artifact_count > 0);
    }

    #[test]
    fn test_engine_uses_snapshot_not_legacy_registry() {
        // Verify the engine iterates snapshot artifacts, not load_registry()
        let engine = TemporalEngine::default();
        let profile = test_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::SelfEmployed),
        );
        let context = TemporalContext::current_compliance(2024);

        let decisions = engine.evaluate_with_context(&profile, &context);
        // Every decision should have an artifact_id (snapshot-sourced)
        for d in &decisions {
            assert!(
                d.artifact_id.is_some(),
                "Form {} should have snapshot artifact_id, not legacy source",
                d.form_code
            );
        }
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Annual Individual ITR Recommendation Tests
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Mixed income → only 1701 is visible (1701A/1701MS suppressed).
    #[test]
    fn test_itr_mixed_income_only_1701() {
        let engine = TemporalEngine::default();
        let profile = test_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::MixedIncome),
        );
        let decisions = engine.evaluate(&profile, 2024);

        let d1701 = decisions.iter().find(|d| d.form_code == "1701");
        let d1701a = decisions.iter().find(|d| d.form_code == "1701A");
        let d1701ms = decisions.iter().find(|d| d.form_code == "1701MS");

        assert!(
            d1701.is_some() && d1701.unwrap().eligibility.is_visible(),
            "1701 should be visible for mixed income"
        );
        assert!(
            d1701a.is_none() || !d1701a.unwrap().eligibility.is_visible(),
            "1701A should be suppressed for mixed income"
        );
        assert!(
            d1701ms.is_none() || !d1701ms.unwrap().eligibility.is_visible(),
            "1701MS should be suppressed for mixed income"
        );
    }

    /// Mixed income + micro/small tier → 1701MS should STILL be suppressed
    /// (EOPT rule must respect the exclusive group's suppression for mixed income).
    #[test]
    fn test_itr_mixed_income_micro_small_suppresses_1701ms() {
        use crate::profile::EoptTier;

        let engine = TemporalEngine::default();
        let mut profile = test_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::MixedIncome),
        );
        profile.eopt_tier = Some(EoptTier::Micro);

        let decisions = engine.evaluate(&profile, 2024);
        let d1701ms = decisions.iter().find(|d| d.form_code == "1701MS");

        assert!(
            d1701ms.is_none() || !d1701ms.unwrap().eligibility.is_visible(),
            "1701MS should be suppressed for mixed income even with Micro tier"
        );
    }

    /// Business/profession + 8% election → 1701A primary, 1701 Optional, 1701MS Recommended (micro).
    #[test]
    fn test_itr_business_8pct_micro_small_recommendation() {
        use crate::profile::{EoptTier, IncomeTaxElection, TaxElectionHistory};

        let engine = TemporalEngine::default();
        let mut profile = test_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::SelfEmployed),
        );
        profile.eopt_tier = Some(EoptTier::Micro);
        profile.tax_elections.push(TaxElectionHistory {
            taxable_year: 2024,
            election: IncomeTaxElection::EightPercent,
            elected_at: chrono::NaiveDateTime::default(),
            source_form: "1701Q".into(),
        });

        let decisions = engine.evaluate(&profile, 2024);

        let d1701 = decisions.iter().find(|d| d.form_code == "1701");
        let d1701a = decisions.iter().find(|d| d.form_code == "1701A");
        let d1701ms = decisions.iter().find(|d| d.form_code == "1701MS");

        // 1701A: primary (unchanged Applicable)
        assert!(
            d1701a.is_some() && d1701a.unwrap().eligibility.is_visible(),
            "1701A should be visible for business with 8%"
        );

        // 1701: Optional alternative
        assert!(
            d1701.is_some() && d1701.unwrap().eligibility.is_visible(),
            "1701 should be visible (Optional) for business with 8%"
        );
        assert!(
            matches!(&d1701.unwrap().eligibility, ComplianceState::Optional(_)),
            "1701 should be Optional for 8% filers, got: {:?}",
            d1701.unwrap().eligibility
        );

        // 1701MS: Recommended for Micro
        assert!(
            d1701ms.is_some() && d1701ms.unwrap().eligibility.is_visible(),
            "1701MS should be visible for micro taxpayer with 8%"
        );
        assert!(
            matches!(
                &d1701ms.unwrap().eligibility,
                ComplianceState::Recommended(_)
            ),
            "1701MS should be Recommended for Micro, got: {:?}",
            d1701ms.unwrap().eligibility
        );
    }

    /// Business/profession + no 8% election → 1701 primary, 1701A Optional.
    #[test]
    fn test_itr_business_no_8pct_1701_primary() {
        let engine = TemporalEngine::default();
        let profile = test_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::SelfEmployed),
        );

        let decisions = engine.evaluate(&profile, 2024);

        let d1701 = decisions.iter().find(|d| d.form_code == "1701");
        let d1701a = decisions.iter().find(|d| d.form_code == "1701A");

        assert!(
            d1701.is_some() && d1701.unwrap().eligibility.is_visible(),
            "1701 should be visible as primary"
        );

        assert!(
            d1701a.is_some() && d1701a.unwrap().eligibility.is_visible(),
            "1701A should be visible (Optional)"
        );
        assert!(
            matches!(&d1701a.unwrap().eligibility, ComplianceState::Optional(_)),
            "1701A should be Optional without 8% election, got: {:?}",
            d1701a.unwrap().eligibility
        );
    }

    /// Compensation only → 1701 is NOT in classifications (PurelyCompensation uses
    /// substituted filing via employer / 1700). 1701A and 1701MS are also suppressed.
    #[test]
    fn test_itr_compensation_only_suppressed() {
        let engine = TemporalEngine::default();
        let profile = test_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::PurelyCompensation),
        );

        let decisions = engine.evaluate(&profile, 2024);

        let d1701 = decisions.iter().find(|d| d.form_code == "1701");
        let d1701a = decisions.iter().find(|d| d.form_code == "1701A");
        let d1701ms = decisions.iter().find(|d| d.form_code == "1701MS");

        // 1701 is only for SelfEmployed/MixedIncome/EstateOrTrust per TOML snapshot
        assert!(
            d1701.is_none() || !d1701.unwrap().eligibility.is_visible(),
            "1701 should be suppressed for purely compensation (classification gate)"
        );
        assert!(
            d1701a.is_none() || !d1701a.unwrap().eligibility.is_visible(),
            "1701A should be suppressed for compensation-only"
        );
        assert!(
            d1701ms.is_none() || !d1701ms.unwrap().eligibility.is_visible(),
            "1701MS should be suppressed for compensation-only"
        );
    }
}
