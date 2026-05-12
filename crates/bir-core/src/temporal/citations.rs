//! Structured legal citations for tax rules.
//!
//! Citations are first-class data with unique IDs, not comments or
//! hardcoded explanation strings.

use serde::{Deserialize, Serialize};

/// The type of Philippine tax issuance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CitationKind {
    /// Republic Act (e.g., RA 10963 — TRAIN Law)
    RepublicAct,
    /// Revenue Regulation (e.g., RR 11-2018)
    RevenueRegulation,
    /// Revenue Memorandum Circular (e.g., RMC 52-2023)
    RevenueMemoCircular,
    /// Revenue Memorandum Order
    RevenueMemoOrder,
    /// BIR Form Instructions
    BirFormInstruction,
}

/// A structured legal citation attached to a rule or form definition.
///
/// Every citation has a unique `citation_id` for cross-referencing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct LegalCitation {
    /// Unique citation identifier for cross-referencing (e.g., "ra-10963-sec-24-a-2-b").
    #[serde(default)]
    pub citation_id: String,
    /// The type of issuance.
    pub kind: CitationKind,
    /// The identifier (e.g., "10963", "52-2023").
    pub number: String,
    /// Human-readable section or subject.
    pub section: String,
    /// The year the issuance was published.
    pub year: u16,
}

impl std::fmt::Display for LegalCitation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match self.kind {
            CitationKind::RepublicAct => "RA",
            CitationKind::RevenueRegulation => "RR",
            CitationKind::RevenueMemoCircular => "RMC",
            CitationKind::RevenueMemoOrder => "RMO",
            CitationKind::BirFormInstruction => "BIR Form",
        };
        write!(f, "{} {} ({})", prefix, self.number, self.year)
    }
}
