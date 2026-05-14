//! Temporal Tax Compliance Engine — core module.
//!
//! This module replaces the static boolean-based form suggestion engine
//! with a temporal, era-scoped rule engine that evaluates form eligibility,
//! tax rates, and computation formulas for any target year.
//!
//! # Architecture
//!
//! ```text
//! TemporalContext + TaxpayerProfile
//!   ↓
//!   CompiledRuleSnapshot (embedded at build time)
//!   ↓
//!   TemporalEngine::evaluate(profile, context)
//!   ↓
//!   Vec<FormDecision> (every form tagged with ComplianceState + audit log)
//!   ↓
//!   DashboardFormDecision (UI view model for existing cards)
//! ```

// ── New canonical modules ──
pub mod context;
pub mod eligibility_facts;
pub mod forms;
pub mod formulas;
pub mod rates;
pub mod rule_model;
pub mod snapshot;
pub mod snapshot_loader;
pub mod support_level;
pub mod validator;

// ── Existing modules (retained for backward compat) ──
pub mod citations;
pub mod compat;
pub mod eligibility;
pub mod engine;
pub mod rules;
pub mod traits;

// ── Re-exports: New canonical types ──
pub use context::{FilingMode, FilingPeriod, Jurisdiction, SnapshotId, TemporalContext};
pub use eligibility::{
    ComplianceState, DashboardFormDecision, FormDecision, FormEligibility, RuleApplication,
};
pub use eligibility_facts::{
    CooperativeTaxTreatment, EligibilityFacts, IndividualIncomeKind, YearElection,
};
pub use forms::{
    ArtifactLifecycle, FormArtifact, FormAvailability, FormIdentity, OperationalRecommendation,
};
pub use snapshot::{CompiledRuleSnapshot, Era};
pub use support_level::{FormSupportLevel, form_support_level};

// ── Re-exports: Legacy types (backward compat) ──
pub use citations::{CitationKind, LegalCitation};
pub use engine::TemporalEngine;
pub use traits::TaxRule;
