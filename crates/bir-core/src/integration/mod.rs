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

pub mod mapper;
pub mod models;
pub mod service;
pub mod validation;

pub use mapper::{FormDraftOutput, FormMapper, Mapper2551Q, MapperError};
pub use models::{IncomeCategory, IncomeSource, UniversalTaxPayload};
pub use service::{
    SyncError, SyncResponse, SyncResult, import_payload_directory, import_payload_file,
    process_sync, process_sync_json,
};
pub use validation::{
    PayloadValidationError, applicable_forms_for_profile, validate_form_applicability,
    validate_payload,
};
