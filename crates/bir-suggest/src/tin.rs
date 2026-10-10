//! TIN format suggestions. Never emits the word "valid".

use core::fmt;
use core::fmt::Write as _;

use crate::ocr_digits::{OcrError, OcrRepair, repair_numeric_field};
use pua_core::Confidence;

/// Kind of suggestion (never a validity claim).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SuggestionKind {
    /// Separators stripped and/or OCR digit confusions repaired.
    TinFormat,
    /// Catalog longest-exact match.
    Catalog,
}

impl SuggestionKind {
    /// Stable name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::TinFormat => "tin_format",
            Self::Catalog => "catalog",
        }
    }
}

/// A suggestion with a reason. The reason string is forbidden from containing `"valid"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Suggestion {
    kind: SuggestionKind,
    suggested: Box<str>,
    reason: Box<str>,
    penalty: Confidence,
    repairs: Box<[OcrRepair]>,
}

impl Suggestion {
    /// Kind.
    pub const fn kind(&self) -> SuggestionKind {
        self.kind
    }
    /// Suggested value (digits only for TIN).
    pub fn suggested(&self) -> &str {
        &self.suggested
    }
    /// Human-readable reason. Never contains `"valid"`.
    pub fn reason(&self) -> &str {
        &self.reason
    }
    /// OCR / format penalty.
    pub const fn penalty(&self) -> Confidence {
        self.penalty
    }
    /// OCR replacements applied.
    pub fn repairs(&self) -> &[OcrRepair] {
        &self.repairs
    }
}

/// Why a TIN could not be suggested.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TinError {
    /// Empty after stripping separators.
    Empty,
    /// A char that is neither a digit, a separator, nor an OCR confusion.
    NotNumeric {
        /// Byte offset in the field after separators (`-`, space, `/`, `.`) were stripped,
        /// not in the original input.
        at: usize,
        /// The char.
        ch: char,
    },
}

impl fmt::Display for TinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("TIN field is empty"),
            Self::NotNumeric { at, ch } => {
                write!(f, "TIN has non-digit {ch:?} at byte {at}")
            }
        }
    }
}

impl std::error::Error for TinError {}

const SEPARATORS: &[char] = &['-', ' ', '.'];

/// Suggests a canonical digit-only TIN. Strips `-` / space / `.`, then applies the OCR digit
/// table. Separators are format, not content.
///
/// # Errors
/// [`TinError`].
pub fn suggest_tin(field: &str) -> Result<Suggestion, TinError> {
    let mut stripped = String::with_capacity(field.len());
    let mut sep_count = 0u32;
    for ch in field.chars() {
        if SEPARATORS.contains(&ch) {
            sep_count = sep_count.saturating_add(1);
        } else {
            stripped.push(ch);
        }
    }
    if stripped.is_empty() {
        return Err(TinError::Empty);
    }
    let repaired = repair_numeric_field(&stripped).map_err(|e| match e {
        OcrError::Empty => TinError::Empty,
        OcrError::NotNumeric { at, ch } => TinError::NotNumeric { at, ch },
    })?;
    let mut reason = String::from("format suggestion");
    if sep_count != 0 {
        reason.push_str(": stripped separators");
    }
    if !repaired.repairs().is_empty() {
        if sep_count != 0 {
            reason.push_str(" and");
        } else {
            reason.push(':');
        }
        let _ = write!(
            reason,
            " repaired {} OCR digit(s)",
            repaired.repairs().len()
        );
    }
    debug_assert!(!reason.to_ascii_lowercase().contains("valid"));
    Ok(Suggestion {
        kind: SuggestionKind::TinFormat,
        suggested: repaired.digits().into(),
        reason: reason.into(),
        penalty: repaired.penalty(Confidence::new(50).unwrap_or(Confidence::ZERO)),
        repairs: repaired.repairs().to_vec().into_boxed_slice(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_says_valid() {
        for sample in [
            "000-000-000-00000",
            "00000000000000",
            "000-000-000-0000O",
            "",
        ] {
            if let Ok(s) = suggest_tin(sample) {
                assert!(
                    !s.reason().to_ascii_lowercase().contains("valid"),
                    "{}",
                    s.reason()
                );
                assert!(!s.suggested().contains("valid"));
            }
        }
        // The type has no Valid variant.
        let _ = SuggestionKind::TinFormat;
    }

    #[test]
    fn strips_and_repairs() {
        let s = suggest_tin("123-456-789-0000O").unwrap();
        assert_eq!(s.suggested(), "12345678900000");
        assert_eq!(s.repairs().len(), 1);
        assert_eq!(s.kind(), SuggestionKind::TinFormat);
    }
}
