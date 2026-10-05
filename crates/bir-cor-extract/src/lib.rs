//! `bir-cor-extract`: COR (BIR Form 2303) label extraction from OCR text.
//!
//! Moved from `hexuria/pua` `packs/pua-ocr` in PUA's Phase 2 engine/consumer split. It does
//! **not** go through `pua-text`'s offset map: a small scanner of its own finds labels on the raw
//! lines, so every span is already an offset into the original text.
//!
//! Values are returned as spans into the **original** text — the fix for the A §5.3 bug where
//! offsets computed on normalized text were applied to the raw line. Label diacritics are **not**
//! folded (owner decision q14). Per-field confidence comes from the match type.
//!
//! ```
//! use bir_cor_extract::{extract, MatchType};
//!
//! let page = "  TIN: 123-456-789-000\nTRADE NAME: PEÑA STORE\n";
//! let fields = extract(page);
//! let tin = fields.iter().find(|f| f.label() == "TIN").expect("tin");
//! assert_eq!(tin.value_text(page), "123-456-789-000");
//! assert_eq!(tin.match_type(), MatchType::Exact);
//! ```
#![forbid(unsafe_code)]

use pua_core::{Confidence, Span};
use pua_graph::{Edge, LabeledGraph, Rounds, wl_refine};

/// Algorithm tag.
pub const ALGORITHM_TAG: &str = "pua-ocr/1";

/// COR label vocabulary (from `cor_ocr.rs`; sample, closed).
pub const LABELS: &[&str] = &[
    "TIN",
    "TAXPAYER IDENTIFICATION NUMBER",
    "TAXPAYER'S NAME",
    "TAXPAYER NAME",
    "NAME",
    "TRADE NAME",
    "REGISTRATION DATE",
    "REVENUE DISTRICT OFFICE",
    "RDO",
    "REGISTERED ADDRESS",
    "ADDRESS",
    "LINE OF BUSINESS",
    "REGISTERED ACTIVITIES",
    "REGISTERED ACTIVITY",
];

/// How the label was matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MatchType {
    /// Exact label match on the same line, value after `:`.
    Exact,
    /// Label matched after case fold only (diacritics preserved).
    CaseFolded,
    /// Value taken from the next line.
    NextLine,
}

impl MatchType {
    /// Confidence for this match type (replaces the hard-coded 0.6).
    pub const fn confidence(self) -> Confidence {
        match self {
            Self::Exact => Confidence::saturating(900),
            Self::CaseFolded => Confidence::saturating(800),
            Self::NextLine => Confidence::saturating(700),
        }
    }

    /// Stable name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::CaseFolded => "case_folded",
            Self::NextLine => "next_line",
        }
    }
}

/// One extracted field.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Field {
    label: &'static str,
    value: Span,
    match_type: MatchType,
}

impl Field {
    /// Vocabulary label that matched.
    pub const fn label(&self) -> &'static str {
        self.label
    }
    /// Value span in the **original** page text.
    pub const fn value_span(&self) -> Span {
        self.value
    }
    /// Match type (drives confidence).
    pub const fn match_type(&self) -> MatchType {
        self.match_type
    }
    /// Confidence.
    pub const fn confidence(&self) -> Confidence {
        self.match_type.confidence()
    }
    /// Slices the value from the original page.
    pub fn value_text<'a>(&self, page: &'a str) -> &'a str {
        self.value.slice(page).unwrap_or("")
    }
}

/// Extracts COR fields from a page (multi-line OCR text).
pub fn extract(page: &str) -> Vec<Field> {
    let lines: Vec<(usize, &str)> = {
        let mut out = Vec::new();
        let mut at = 0usize;
        for line in page.split_inclusive('\n') {
            let content = line.strip_suffix('\n').unwrap_or(line);
            out.push((at, content));
            at += line.len();
        }
        if page.ends_with('\n') {
            // trailing empty already represented
        }
        out
    };

    let mut fields = Vec::new();
    for (i, (line_at, line)) in lines.iter().enumerate() {
        let mut labels: Vec<&'static str> = LABELS.to_vec();
        labels.sort_by_key(|l| core::cmp::Reverse(l.len()));
        for &label in &labels {
            if let Some((mt, value_local)) = match_line(line, label) {
                let start = line_at.saturating_add(value_local.start);
                let end = line_at.saturating_add(value_local.end);
                if let Ok(span) = Span::new(u32_of(start), u32_of(end)) {
                    fields.push(Field {
                        label,
                        value: span,
                        match_type: mt,
                    });
                    break;
                }
            } else if line_has_label_only(line, label) {
                // Value on the next line.
                if let Some((next_at, next)) = lines.get(i.saturating_add(1)) {
                    let trimmed = next.trim();
                    if !trimmed.is_empty() {
                        let lead = next.len().saturating_sub(next.trim_start().len());
                        let start = next_at.saturating_add(lead);
                        let end = start.saturating_add(trimmed.len());
                        if let Ok(span) = Span::new(u32_of(start), u32_of(end)) {
                            fields.push(Field {
                                label,
                                value: span,
                                match_type: MatchType::NextLine,
                            });
                            break;
                        }
                    }
                }
            }
        }
    }
    fields
}

fn match_line(line: &str, label: &str) -> Option<(MatchType, core::ops::Range<usize>)> {
    // Find `label` then `:` then value, using original byte offsets.
    if let Some(r) = find_label_value(line, label, false) {
        return Some((MatchType::Exact, r));
    }
    if let Some(r) = find_label_value(line, label, true) {
        return Some((MatchType::CaseFolded, r));
    }
    None
}

pub(crate) fn line_has_label_only(line: &str, label: &str) -> bool {
    let t = line.trim();
    let t = t.strip_suffix(':').unwrap_or(t).trim_end();
    eq_label(t, label, true) && !line.contains(':')
}

fn find_label_value(line: &str, label: &str, fold: bool) -> Option<core::ops::Range<usize>> {
    // Scan for `LABEL : value` anywhere on the line (leading spaces / prose prefixes allowed).
    for (at, _) in line.char_indices() {
        let Some(lab_len) = label_len_in(&line[at..], label, fold) else {
            continue;
        };
        let after_label = at.saturating_add(lab_len);
        let Some(after) = line.get(after_label..) else {
            continue;
        };
        let Some(colon) = after.find(':') else {
            continue;
        };
        if !after[..colon].chars().all(char::is_whitespace) {
            continue;
        }
        let value_start = after_label.saturating_add(colon).saturating_add(1);
        let Some(value) = line.get(value_start..) else {
            continue;
        };
        let lead = value.len().saturating_sub(value.trim_start().len());
        let trim = value.trim();
        if trim.is_empty() {
            continue;
        }
        let start = value_start.saturating_add(lead);
        return Some(start..start.saturating_add(trim.len()));
    }
    None
}

fn label_len_in(rest: &str, label: &str, fold: bool) -> Option<usize> {
    // Match label char-by-char so multibyte originals keep correct byte length.
    let mut ri = rest.chars();
    let mut li = label.chars();
    let mut bytes = 0usize;
    loop {
        match (ri.next(), li.next()) {
            (Some(rc), Some(lc)) => {
                if !chars_eq(rc, lc, fold) {
                    return None;
                }
                bytes += rc.len_utf8();
            }
            (_, None) => return Some(bytes),
            (None, Some(_)) => return None,
        }
    }
}

pub(crate) fn eq_label(a: &str, b: &str, fold: bool) -> bool {
    if !fold {
        return a == b;
    }
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    ac.len() == bc.len() && ac.iter().zip(&bc).all(|(x, y)| chars_eq(*x, *y, true))
}

pub(crate) fn chars_eq(a: char, b: char, fold: bool) -> bool {
    if a == b {
        return true;
    }
    if !fold {
        return false;
    }
    // Case fold only — diacritics are NOT folded (Ñ ≠ N).
    a.to_lowercase().eq(b.to_lowercase())
}

fn u32_of(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// WL layout fingerprint over extracted label nodes. Stable under field renumbering.
pub fn layout_fingerprint(fields: &[Field]) -> [u8; 32] {
    let mut g = LabeledGraph::new();
    let mut nodes = Vec::new();
    let mut sorted: Vec<&Field> = fields.iter().collect();
    sorted.sort_by_key(|f| f.label());
    for f in sorted {
        let mut h = blake3::Hasher::new();
        h.update(f.label().as_bytes());
        let label = u64::from_le_bytes(h.finalize().as_bytes()[..8].try_into().unwrap_or([0; 8]));
        if let Ok(id) = g.add_node(label) {
            nodes.push(id);
        }
    }
    for i in 0..nodes.len() {
        for j in i.saturating_add(1)..nodes.len() {
            let _ = g.add_edge(nodes[i], nodes[j], Edge::undirected());
        }
    }
    wl_refine(&g, Rounds::Fixed(3)).fingerprint()
}

#[cfg(test)]
mod tests;
