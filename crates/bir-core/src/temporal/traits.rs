//! The TaxRule trait — era-scoped, modular tax law implementations.

use crate::temporal::eligibility_facts::EligibilityFacts;
use crate::temporal::forms::FormArtifact;
use crate::temporal::{FormEligibility, LegalCitation};

/// A single, self-contained tax rule that can modify a form's eligibility.
///
/// Each rule represents a specific provision from a specific law.
/// Rules are era-scoped: they declare the year range they are active for,
/// and the engine only invokes them when `target_year` falls within that range.
///
/// Rules consume `EligibilityFacts` (derived from the profile) and
/// `FormArtifact` (from the compiled snapshot) — never raw profile or
/// legacy `TemporalFormDef` directly.
pub trait TaxRule: Send + Sync {
    /// Human-readable name for audit logs.
    fn name(&self) -> &'static str;

    /// The law or issuance this rule implements.
    fn law(&self) -> &'static str;

    /// Structured legal citation.
    fn citation(&self) -> LegalCitation;

    /// The first year this rule takes effect (inclusive).
    fn effective_from(&self) -> u16;

    /// The last year this rule applies (inclusive). None = still active.
    fn effective_until(&self) -> Option<u16>;

    /// Returns true if this rule is active for the given target year.
    fn is_active_for_year(&self, target_year: u16) -> bool {
        target_year >= self.effective_from()
            && self.effective_until().is_none_or(|end| target_year <= end)
    }

    /// Evaluate this rule against a form.
    ///
    /// Returns the (possibly mutated) eligibility state.
    /// If the rule doesn't care about this form, return `current_state` unchanged.
    fn evaluate(
        &self,
        facts: &EligibilityFacts,
        form: &FormArtifact,
        current_state: FormEligibility,
        target_year: u16,
    ) -> FormEligibility;
}
