//! Small ports of the official eBIRForms input helpers (`js/string-util.js`
//! and per-form handlers) shared by the event-based capital gains, donor's and
//! estate tax returns (1706, 1707, 1707A, 1800, 1801).

use crate::official_xml::{official_amount, parse_official_amount};

/// `round(this, 2)` / `formatCurrency`: the value the official field holds.
pub fn cents(value: f64) -> f64 {
    parse_official_amount(&official_amount(value)).unwrap_or(0.0)
}

/// An optional amount after `round(this, 2)`; blank stays blank.
pub fn cents_opt(value: Option<f64>) -> Option<f64> {
    value.map(cents)
}

/// `round()` turns any amount with more than 12 integer digits into `0.00`.
pub const MAX_ROUNDED_AMOUNT: f64 = 1_000_000_000_000.0;

/// True when `round()` keeps the amount (at most 12 integer digits).
pub fn within_round_limit(value: f64) -> bool {
    value.is_finite() && value.abs() < MAX_ROUNDED_AMOUNT
}

/// JavaScript `Number.prototype.toFixed(2)`: the exact binary value rounded
/// half up to two decimals (`2.675.toFixed(2)` is `"2.67"`).
pub fn to_fixed_2(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    // Rust prints the exact decimal expansion for a large precision.
    let exact = format!("{:.40}", value.abs());
    let (whole, frac) = exact.split_once('.').unwrap_or((&exact, "0"));
    let mut digits: Vec<u8> = whole.bytes().chain(frac.bytes().take(2)).collect();
    let round_up = frac.as_bytes().get(2).is_some_and(|d| *d >= b'5');
    if round_up {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, b'1');
                break;
            }
            i -= 1;
            if digits[i] == b'9' {
                digits[i] = b'0';
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let text = String::from_utf8(digits).unwrap_or_default();
    let (int_part, dec_part) = text.split_at(text.len() - 2);
    let parsed: f64 = format!("{int_part}.{dec_part}").parse().unwrap_or(0.0);
    if value < 0.0 { -parsed } else { parsed }
}

/// `NumWithComma()` on a field that may be blank: blank is `NaN`.
pub fn js_num(value: Option<f64>) -> f64 {
    value.unwrap_or(f64::NAN)
}

/// `a >= b` with JavaScript `NaN` semantics (always false on `NaN`).
pub fn js_ge(a: f64, b: f64) -> bool {
    a >= b
}

pub fn has_cent_precision(value: f64) -> bool {
    value.is_finite() && ((value * 100.0) - (value * 100.0).round()).abs() < 1e-7
}

pub fn digits_only(value: &str) -> bool {
    value.bytes().all(|b| b.is_ascii_digit())
}

/// Digits and `.` only: what `numbersonly` lets a filer type.
pub fn numbers_only(value: &str) -> bool {
    value.bytes().all(|b| b.is_ascii_digit() || b == b'.')
}

/// Letters and digits only: what `letternumber` lets a filer type.
pub fn letters_and_digits(value: &str) -> bool {
    value.chars().all(|c| c.is_ascii_alphanumeric())
}

/// `(TIN1, TIN2, TIN3, branch)` from a stored TIN with or without dashes.
/// A missing branch is padded to `00000`.
pub fn split_tin(tin: &str) -> (String, String, String, String) {
    let digits: String = tin.chars().filter(char::is_ascii_digit).collect();
    let part = |range: std::ops::Range<usize>| digits.get(range).unwrap_or("").to_string();
    let branch = digits.get(9..).unwrap_or("");
    (part(0..3), part(3..6), part(6..9), format!("{branch:0>5}"))
}

/// A TIN written as 9 digits plus an optional 3–5 digit branch, with or
/// without dashes.
pub fn tin_is_well_formed(tin: &str) -> bool {
    let digits: String = tin.chars().filter(char::is_ascii_digit).collect();
    tin.chars().all(|c| c.is_ascii_digit() || c == '-') && digits.len() >= 9 && digits.len() <= 14
}

/// `capital()`: every text control except `txtEmail` is uppercased.
pub fn capital(value: &str) -> String {
    value.trim().to_uppercase()
}

/// An optional amount as the official control shows it: blank or `1,234.56`.
pub fn amount_text(value: Option<f64>) -> String {
    value.map(official_amount).unwrap_or_default()
}

/// A real calendar date with a 4-digit year.
pub fn is_calendar_date(year: u16, month: u8, day: u8) -> bool {
    (1000..=9999).contains(&year)
        && chrono::NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day))
            .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_fixed_matches_javascript() {
        assert_eq!(to_fixed_2(2.675), 2.67);
        assert_eq!(to_fixed_2(1.005), 1.0);
        assert_eq!(to_fixed_2(0.125), 0.13);
        assert_eq!(to_fixed_2(1_000_000.005), 1_000_000.01);
        assert_eq!(to_fixed_2(99.999), 100.0);
        assert_eq!(to_fixed_2(-1.5), -1.5);
    }

    #[test]
    fn helpers() {
        assert_eq!(cents(123_456.789), 123_456.79);
        assert_eq!(
            split_tin("123-456-788"),
            ("123".into(), "456".into(), "788".into(), "00000".into())
        );
        assert!(!js_ge(js_num(None), 0.0));
        assert!(is_calendar_date(2024, 2, 29));
        assert!(!is_calendar_date(2025, 2, 29));
        assert!(numbers_only("12.5"));
        assert!(!numbers_only("1,2"));
    }
}
