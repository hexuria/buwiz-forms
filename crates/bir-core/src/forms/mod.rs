//! BIR form data models, ATC tables, and form registry.

pub mod atc;
pub mod form_1601c;
pub mod form_1701q;
pub mod form_2307;
pub mod form_2551q;
pub mod registry;

pub use atc::{ATC_TABLE_2551Q, AtcEntry, find_atc};
pub use form_1601c::Form1601CDraft;
pub use form_1701q::Form1701QDraft;
pub use form_2551q::{Form2551QDraft, Schedule1Row};
pub use registry::{
    FORM_REGISTRY, FilingFrequency, FormDefinition, find_form, forms_for_profile,
    forms_for_taxpayer,
};

pub mod form_1601c_xml;
pub mod form_2551q_xml;

pub trait FormValidator {
    /// Returns a list of (field_id, error_message)
    fn validate(&self) -> Vec<(String, String)>;
}

/// Status of a form draft or filed return.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum FilingStatus {
    #[default]
    Draft,
    Queued,
    #[serde(alias = "Filed")]
    Submitted,
    Confirmed,
    Paid,
}

/// Summary record returned by database list queries (no full JSON needed).
#[derive(Debug, Clone)]
pub struct FormDraftSummary {
    pub id: i64,
    pub tin: String,
    pub form_code: String,
    pub taxable_year: u16,
    pub quarter: Option<u8>,
    pub month: Option<u8>,
    pub status: FilingStatus,
    pub updated_at: String,
}

impl FormDraftSummary {
    pub fn quarter_state(&self) -> QuarterState {
        match self.status {
            FilingStatus::Draft => QuarterState::Draft,
            FilingStatus::Queued => QuarterState::Queued,
            FilingStatus::Submitted => QuarterState::Submitted,
            FilingStatus::Confirmed => QuarterState::Confirmed,
            FilingStatus::Paid => QuarterState::Paid,
        }
    }
}

/// Per-quarter filing state for a form card on the dashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarterState {
    NotStarted,
    Draft,
    Queued,
    Submitted,
    Confirmed,
    Paid,
}

/// Aggregated filing progress for a single form in a given year.
#[derive(Debug, Clone)]
pub struct FormFilingProgress {
    pub form_code: String,
    pub taxable_year: u16,
    /// For quarterly forms: index 0 = Q1, 1 = Q2, 2 = Q3, 3 = Q4
    pub quarters: [QuarterState; 4],
    /// For monthly forms: index 0 = Jan, 11 = Dec
    pub months: [QuarterState; 12],
    /// For annual forms
    pub annual_status: QuarterState,
    /// For open-ended forms: total filed this year
    pub open_ended_count: u32,
}

impl FormFilingProgress {
    pub fn new_empty(form_code: &str, year: u16) -> Self {
        Self {
            form_code: form_code.to_string(),
            taxable_year: year,
            quarters: [
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
            ],
            months: [
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
                QuarterState::NotStarted,
            ],
            annual_status: QuarterState::NotStarted,
            open_ended_count: 0,
        }
    }

    /// How many quarters have been filed (for quarterly forms).
    pub fn filed_count(&self) -> u32 {
        self.quarters
            .iter()
            .filter(|q| {
                **q == QuarterState::Queued
                    || **q == QuarterState::Submitted
                    || **q == QuarterState::Confirmed
                    || **q == QuarterState::Paid
            })
            .count() as u32
    }

    /// Next quarter that hasn't been started yet (1-based). Returns None if all 4 submitted/confirmed.
    pub fn next_unfiled_quarter(&self) -> Option<u8> {
        self.quarters
            .iter()
            .enumerate()
            .find(|(_, s)| **s == QuarterState::NotStarted)
            .map(|(i, _)| (i + 1) as u8)
    }

    /// Next quarter that is a draft in progress (1-based).
    pub fn draft_quarter(&self) -> Option<u8> {
        self.quarters
            .iter()
            .enumerate()
            .find(|(_, s)| **s == QuarterState::Draft || **s == QuarterState::Queued)
            .map(|(i, _)| (i + 1) as u8)
    }
}

pub enum FormDraft {
    Form2551Q(Form2551QDraft),
    Form1601C(Form1601CDraft),
    // Future forms will be added here
}

impl FormDraft {
    pub fn form_code(&self) -> &'static str {
        match self {
            Self::Form2551Q(_) => "2551Q",
            Self::Form1601C(_) => "1601C",
        }
    }

    pub fn status(&self) -> &FilingStatus {
        match self {
            Self::Form2551Q(f) => &f.status,
            Self::Form1601C(f) => &f.status,
        }
    }

    pub fn validate(&self) -> Vec<(String, String)> {
        match self {
            Self::Form2551Q(f) => f.validate(),
            Self::Form1601C(f) => f.validate(),
        }
    }
}
