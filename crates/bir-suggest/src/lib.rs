//! `bir-suggest`: format-repair and catalog suggestions for BIR form fields.
//!
//! Moved from `hexuria/pua` `packs/pua-bir` together with the OCR digit table
//! (`pua_lexicon::ocr`, now [`ocr_digits`]) in PUA's Phase 2 engine/consumer split: buwiz-forms
//! owns the BIR meaning (TIN layout, catalog, which fields are numeric); PUA supplies the Ñ-safe
//! normalizer (`pua-text`), WL fingerprints (`pua-graph`) and integer confidences (`pua-core`).
//!
//! Output is always a **suggestion** with a reason. The string `"valid"` is never emitted for a
//! TIN — bir-rules owns validation. The sample RDO/form catalog is labelled
//! **non-authoritative** (PUA q8). Names keep Ñ: `PEÑA ≠ PENA`.
//!
//! ```
//! use bir_suggest::{suggest_tin, SuggestionKind};
//!
//! let s = suggest_tin("000-000-000-0000O").expect("digits");
//! assert_eq!(s.suggested(), "00000000000000");
//! assert!(!s.reason().contains("valid"));
//! assert_eq!(s.kind(), SuggestionKind::TinFormat);
//! ```
#![forbid(unsafe_code)]

mod catalog;
mod name;
pub mod ocr_digits;
mod tin;

pub use catalog::{Catalog, CatalogError, CatalogHit, CodeKind};
pub use name::{fold_name, names_equal};
pub use tin::{Suggestion, SuggestionKind, TinError, suggest_tin};

use pua_core::DataVersion;
use pua_graph::{Edge, LabeledGraph, Rounds, wl_refine};

/// Algorithm tag.
pub const ALGORITHM_TAG: &str = "pua-bir/1";

/// A form record: unordered field map (order must not affect suggestions or the layout fingerprint).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormRecord<'a> {
    fields: Vec<(&'a str, &'a str)>,
}

impl<'a> FormRecord<'a> {
    /// Builds from `(key, value)` pairs. Duplicate keys keep the first.
    pub fn new(fields: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut out = Vec::new();
        for (k, v) in fields {
            if !out.iter().any(|(kk, _)| *kk == k) {
                out.push((k, v));
            }
        }
        out.sort_by(|a, b| a.0.cmp(b.0));
        Self { fields: out }
    }

    /// Fields in sorted key order.
    pub fn fields(&self) -> &[(&'a str, &'a str)] {
        &self.fields
    }
}

/// Layout signature: 1-WL fingerprint over field-key nodes (PUA spec §6.3). Stable under field
/// renumbering / submission order.
pub fn layout_fingerprint(record: &FormRecord<'_>) -> [u8; 32] {
    let mut g = LabeledGraph::new();
    let mut nodes = Vec::with_capacity(record.fields.len());
    for (k, _) in &record.fields {
        let mut h = blake3::Hasher::new();
        h.update(k.as_bytes());
        let label = u64::from_le_bytes(h.finalize().as_bytes()[..8].try_into().unwrap_or([0; 8]));
        if let Ok(id) = g.add_node(label) {
            nodes.push(id);
        }
    }
    // Complete graph on field-key nodes: edge set depends only on the key set (already sorted).
    for i in 0..nodes.len() {
        for j in i.saturating_add(1)..nodes.len() {
            let _ = g.add_edge(nodes[i], nodes[j], Edge::undirected());
        }
    }
    wl_refine(&g, Rounds::Fixed(3)).fingerprint()
}

/// Pack data version over the embedded catalog.
pub fn data_version(catalog: &Catalog) -> DataVersion {
    DataVersion::builder(ALGORITHM_TAG)
        .field("catalog", &catalog.fingerprint())
        .finish()
}

#[cfg(test)]
mod tests;
