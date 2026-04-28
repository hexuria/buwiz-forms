//! BIR form data models, ATC tables, and form registry.

pub mod atc;
pub mod form_1701q;
pub mod form_2551q;
pub mod registry;

pub use atc::{ATC_TABLE_2551Q, AtcEntry, find_atc};
pub use form_1701q::Form1701QDraft;
pub use form_2551q::{
    FilingStatus, Form2551QDraft, FormDraftSummary, FormFilingProgress, QuarterState, Schedule1Row,
};
pub use registry::{FORM_REGISTRY, FilingFrequency, FormDefinition, find_form, forms_for_taxpayer};

pub mod form_2551q_xml;

pub trait FormValidator {
    /// Returns a list of (field_id, error_message)
    fn validate(&self) -> Vec<(String, String)>;
}
