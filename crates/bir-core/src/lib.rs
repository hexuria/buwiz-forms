//! # bir-core
//!
//! Core library for the BIR eBIRForms replacement.
//!
//! This crate provides all business logic for Philippine tax return filing:
//! - BIR pseudo-XML format parsing and generation
//! - Taxpayer profile management with encrypted storage
//! - Form schema engine with validation
//! - ZLib + AES-128 encryption pipeline (BIR-compatible)
//! - FTP submission transport
//! - Reference data (RDO, ATC codes, regions, etc.)
//! - PDF generation for form printing
//!
//! This crate is UI-agnostic and can be consumed by any frontend:
//! CLI, GPUI desktop, web (via WASM), etc.

pub mod bir_xml;
pub mod crypto;
pub mod db;
pub mod form;
pub mod forms;
pub mod naming;
pub mod profile;
pub mod receipt;
pub mod reference;
pub mod schema;
pub mod transport;
pub mod validation;

// Re-export core types
pub use bir_xml::{generate_bir_xml, parse_bir_xml};
pub use forms::{
    ATC_TABLE_2551Q, FilingStatus, Form2551QDraft, FormDraftSummary, FormFilingProgress,
    QuarterState, Schedule1Row, find_atc, find_form, forms_for_taxpayer,
};
pub use naming::{Tin, iaf_filename, savefile_name};
pub use profile::TaxpayerProfile;
pub use receipt::{BirReceiptConfirmation, parse_bir_receipt_email, split_bir_filename};
pub use validation::{ValidationError, validate_ph_phone, validate_profile, validate_zip};
