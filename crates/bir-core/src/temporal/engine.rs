//! Temporal Engine — the core evaluation loop.

use crate::profile::TaxpayerProfile;
use crate::temporal::eligibility::{FormDecision, FormEligibility, RuleApplication};
use crate::temporal::registry_loader::load_registry;
use crate::temporal::rules::all_rules;
use crate::temporal::traits::TaxRule;

/// The temporal tax form engine.
///
/// Evaluates form eligibility for a given profile and target year,
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
    /// Evaluate all forms against a profile for a specific tax year.
    pub fn evaluate(&self, profile: &TaxpayerProfile, target_year: u16) -> Vec<FormDecision> {
        let forms = load_registry();
        let active_rules: Vec<_> = self.rules.iter()
            .filter(|r| r.is_active_for_year(target_year))
            .collect();

        let mut decisions = Vec::new();

        for form in &forms {
            let mut audit_log = Vec::new();
            let mut citations = form.legal_basis.clone();

            // Step 1: Timeline check
            let mut state = if target_year < form.active_from_year {
                FormEligibility::Suppressed(format!("Form not yet active in {}", target_year))
            } else if form.active_until_year.map_or(false, |end| target_year > end) {
                FormEligibility::Deprecated(format!(
                    "Form abolished after {}", form.active_until_year.unwrap()
                ))
            } else {
                // Step 2: Fail-open baseline
                FormEligibility::Allowed
            };

            // Step 3: Apply active rules (only if form is alive in this era)
            if state.is_visible() {
                // Check entity type first (quick filter)
                if !form.taxpayer_types.contains(&profile.taxpayer_type) {
                    state = FormEligibility::Suppressed("Entity type mismatch".into());
                } else {
                    for rule in &active_rules {
                        let prev = state.clone();
                        state = rule.evaluate(profile, form, state, target_year);
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
            }

            decisions.push(FormDecision {
                form_code: form.code.clone(),
                title: form.title.clone(),
                category: form.category.clone(),
                frequency: form.frequency.clone(),
                eligibility: state,
                audit_log,
                legal_citations: citations,
            });
        }

        decisions
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
