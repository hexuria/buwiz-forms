//! External integration API — models, mappers, validation, and service layer.
//!
//! This module provides a standardized interface for external systems (Odoo, QuickBooks,
//! Taxman, etc.) to push financial data into eBirForms without needing to understand
//! BIR-specific XML field names or form structures.
//!
//! # Architecture
//!
//! ```text
//! External System → JSON payload → service::process_sync_json()
//!                                      ↓
//!                               validation::validate_payload()
//!                                      ↓
//!                               db.get_profile(tin)
//!                                      ↓
//!                               validation::validate_form_applicability()
//!                                      ↓
//!                               mapper::resolve_mappers()
//!                                      ↓
//!                               mapper.map() → Form2551QDraft
//!                                      ↓
//!                               draft.recompute() + draft.validate()
//!                                      ↓
//!                               db.save_2551q_draft()
//!                                      ↓
//!                               SyncResponse { results, warnings }
//! ```

pub mod computation;
#[cfg(feature = "erp-integration")]
pub mod erp;
pub mod fraud;
pub mod mapper;
pub mod models;
pub mod providers;
pub mod service;
pub mod simulation;
pub mod transactions;
pub mod validation;
pub mod withholding;

pub use mapper::{FormDraftOutput, FormMapper, Mapper2551Q, MapperError};
pub use models::{IncomeCategory, IncomeSource, UniversalTaxPayload};
pub use providers::{DataProvider, ProviderConfig, ProviderError, generic::GenericProvider};
pub use service::{
    SyncError, SyncResponse, SyncResult, import_payload_directory, import_payload_file,
    process_sync, process_sync_json,
};
pub use validation::{
    PayloadValidationError, ProfileConsistencyIssue, ProfileConsistencyReport,
    ProfileConsistencySeverity, ResolvedProfileObligations, all_form_codes,
    applicable_forms_for_profile, applicable_forms_for_profile_and_year,
    deadline_applies_to_profile, evaluate_forms_for_year, profile_deadline_overrides_for_year,
    recurring_obligation_decisions_for_profile_and_year,
    recurring_obligation_forms_for_profile_and_year, resolve_profile_obligations_for_year,
    validate_form_applicability, validate_payload,
};
