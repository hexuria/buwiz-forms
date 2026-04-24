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
pub mod naming;
pub mod profile;
pub mod reference;
pub mod schema;
pub mod transport;

// Re-export core types
pub use bir_xml::{parse_bir_xml, generate_bir_xml};
pub use naming::{Tin, savefile_name, iaf_filename};
pub use profile::TaxpayerProfile;
