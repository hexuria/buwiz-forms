//! Temporal Tax Form Engine — core module.
//!
//! This module replaces the static boolean-based form suggestion engine
//! with a temporal, era-scoped rule engine that evaluates form eligibility
//! for any target year.
//!
//! # Architecture
//!
//! ```text
//! TemporalFormDef[] (registry) + TaxRule[] (rule modules)
//!   ↓                              ↓
//!   TemporalEngine::evaluate(profile, target_year)
//!   ↓
//!   Vec<FormDecision> (every form tagged with eligibility + audit log)
//! ```

pub mod citations;
pub mod compat;
pub mod eligibility;
pub mod engine;
pub mod registry_loader;
pub mod rules;
pub mod traits;

pub use citations::{CitationKind, LegalCitation};
pub use eligibility::{FormDecision, FormEligibility, RuleApplication};
pub use engine::TemporalEngine;
pub use registry_loader::{RegulatoryStatus, TemporalFormDef, WithholdingTrigger};
pub use traits::TaxRule;
