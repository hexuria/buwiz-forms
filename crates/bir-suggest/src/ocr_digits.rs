//! OCR digit confusions for fields declared numeric (from `pua_lexicon::ocr`, PUA spec §4.3).
//!
//! `O→0`, `l→1`, `I→1`, `S→5`, `B→8`. These are **scored repairs**, never free symmetries, and
//! apply only where the caller has declared the field numeric (a TIN group, a branch code). Free
//! text never goes through this table. Lowercase `o`/`s` are not in the table: the spec lists the
//! confusions above and widening it is a data change.

use pua_core::Confidence;

/// One replaced character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OcrRepair {
    /// Byte offset of the replaced char in the input field.
    pub at: usize,
    /// The char OCR produced.
    pub from: char,
    /// The digit it was read as.
    pub to: char,
}

/// A numeric field with OCR confusions replaced.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NumericRepair {
    digits: String,
    repairs: Vec<OcrRepair>,
}

impl NumericRepair {
    /// ASCII digits only, one per input char.
    pub fn digits(&self) -> &str {
        &self.digits
    }
    /// The replacements, in input order. Empty when the field was already all digits.
    pub fn repairs(&self) -> &[OcrRepair] {
        &self.repairs
    }
    /// `repairs × per_repair`, saturating at [`Confidence::MAX`].
    pub fn penalty(&self, per_repair: Confidence) -> Confidence {
        let n = i32::try_from(self.repairs.len()).unwrap_or(i32::MAX);
        Confidence::saturating(n.saturating_mul(i32::from(per_repair.get())))
    }
}

/// Why a field could not be read as digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrError {
    /// The field is empty.
    Empty,
    /// A char that is neither a digit nor a known confusion (separators included: strip them
    /// first, the field layout is the caller's business).
    NotNumeric {
        /// Byte offset of the char.
        at: usize,
        /// The char.
        ch: char,
    },
}

impl core::fmt::Display for OcrError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Empty => f.write_str("numeric field is empty"),
            Self::NotNumeric { at, ch } => {
                write!(f, "numeric field has non-digit {ch:?} at byte {at}")
            }
        }
    }
}

impl std::error::Error for OcrError {}

const fn confusion(c: char) -> Option<char> {
    match c {
        'O' => Some('0'),
        'l' | 'I' => Some('1'),
        'S' => Some('5'),
        'B' => Some('8'),
        _ => None,
    }
}

/// Reads a field declared numeric, replacing OCR digit confusions.
///
/// ```
/// use bir_suggest::ocr_digits::repair_numeric_field;
/// let r = repair_numeric_field("1O2l")?;
/// assert_eq!(r.digits(), "1021");
/// assert_eq!(r.repairs().len(), 2);
/// # Ok::<(), bir_suggest::ocr_digits::OcrError>(())
/// ```
///
/// # Errors
/// [`OcrError::Empty`] for an empty field; [`OcrError::NotNumeric`] at the first char that is
/// neither an ASCII digit nor in the confusion table.
pub fn repair_numeric_field(field: &str) -> Result<NumericRepair, OcrError> {
    if field.is_empty() {
        return Err(OcrError::Empty);
    }
    let mut digits = String::with_capacity(field.len());
    let mut repairs = Vec::new();
    for (at, ch) in field.char_indices() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if let Some(to) = confusion(ch) {
            digits.push(to);
            repairs.push(OcrRepair { at, from: ch, to });
        } else {
            return Err(OcrError::NotNumeric { at, ch });
        }
    }
    Ok(NumericRepair { digits, repairs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_exactly_the_spec() {
        let r = repair_numeric_field("OlISB").unwrap();
        assert_eq!(r.digits(), "01158");
        assert_eq!(
            r.repairs()
                .iter()
                .map(|x| (x.at, x.from, x.to))
                .collect::<Vec<_>>(),
            [
                (0, 'O', '0'),
                (1, 'l', '1'),
                (2, 'I', '1'),
                (3, 'S', '5'),
                (4, 'B', '8')
            ]
        );
        for c in ['o', 's', 'b', 'i', 'L', 'Z', 'G'] {
            assert_eq!(
                repair_numeric_field(&c.to_string()),
                Err(OcrError::NotNumeric { at: 0, ch: c })
            );
        }
    }

    #[test]
    fn errors_are_exact() {
        assert_eq!(repair_numeric_field(""), Err(OcrError::Empty));
        assert_eq!(
            repair_numeric_field("12-3"),
            Err(OcrError::NotNumeric { at: 2, ch: '-' })
        );
        assert_eq!(
            repair_numeric_field("1٣"),
            Err(OcrError::NotNumeric { at: 1, ch: '٣' })
        );
        assert_eq!(OcrError::Empty.to_string(), "numeric field is empty");
        assert_eq!(
            OcrError::NotNumeric { at: 2, ch: '-' }.to_string(),
            "numeric field has non-digit '-' at byte 2"
        );
    }

    #[test]
    fn clean_digits_have_no_penalty() {
        let r = repair_numeric_field("0123456789").unwrap();
        assert_eq!(r.digits(), "0123456789");
        assert_eq!(r.repairs(), []);
        assert_eq!(r.penalty(Confidence::MAX), Confidence::ZERO);
        let r = repair_numeric_field("OO1").unwrap();
        assert_eq!(r.penalty(Confidence::new(100).unwrap()).get(), 200);
        assert_eq!(r.penalty(Confidence::new(600).unwrap()), Confidence::MAX);
    }
}
