#![allow(clippy::collapsible_if)]
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

pub mod background_cron;
pub mod bir_xml;
pub mod crypto;
pub mod db;
pub mod email;
pub mod email_fetcher; // legacy — kept for backward compat, delegates to `email`
pub mod export;
pub mod form;
pub mod forms;
pub mod import;
pub mod integration;
pub mod ipc;
pub mod naming;
pub mod news_fetcher;
pub mod notification;
pub mod official_import;
pub mod penalties;
pub mod platform;
pub mod profile;
pub mod receipt;
pub mod reference;
pub mod schema;
pub mod temporal;
pub mod time_utils;
pub mod transport;
pub mod validation;

// Re-export core types
pub use bir_xml::{generate_bir_xml, parse_bir_xml};
pub use email::{
    ImapAuthenticator, fetch_and_process_emails, get_oauth_email, start_oauth_flow, test_connection,
};
pub use export::{export_database_zip, export_profile_data};
pub use forms::{
    ATC_TABLE_2551Q, FilingStatus, Form2551QDraft, FormDraftSummary, FormFilingProgress,
    QuarterState, Schedule1Row, find_atc, find_form, forms_for_taxpayer,
};
pub use import::{extract_database_zip, import_profile_data};
pub use integration::{
    FormDraftOutput, FormMapper, IncomeCategory, IncomeSource, Mapper2551Q, MapperError,
    PayloadValidationError, SyncError, SyncResponse, SyncResult, UniversalTaxPayload,
    applicable_forms_for_profile, import_payload_directory, import_payload_file, process_sync,
    process_sync_json, validate_form_applicability, validate_payload,
};
pub use naming::{Tin, iaf_filename, savefile_name};
pub use official_import::{OfficialSavefile, import_and_submit_savefile, parse_period_code};
pub use profile::{EmailAuthMethod, TaxClassification, TaxpayerProfile};
pub use receipt::{BirReceiptConfirmation, parse_bir_receipt_email, split_bir_filename};
pub use time_utils::format_next_run;
pub use validation::{ValidationError, validate_ph_phone, validate_profile, validate_zip};
