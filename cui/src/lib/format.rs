//! Text for a TIN and a form code, from store.js (formatTin, displayCode). Pure.

/// "009-123-456" from a 9-digit root; "009-123-456-00000" with a branch code. Anything but digits in the
/// root is dropped, a root of 9 digits or more shows its first nine, a shorter root stays as its digits,
/// and a missing branch leaves no trailing dash.
pub fn format_tin(root: &str, branch: Option<&str>) -> String {
    let digits: String = root.chars().filter(|c| c.is_ascii_digit()).collect();
    let base = if digits.len() >= 9 {
        format!("{}-{}-{}", &digits[0..3], &digits[3..6], &digits[6..9])
    } else {
        digits
    };
    match branch {
        Some(b) if !b.is_empty() => format!("{base}-{b}"),
        _ => base,
    }
}

/// The code people see for a form code ("2551Q", "1601C"): the code as it is.
pub fn display_code(code: &str) -> &str {
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tins() {
        assert_eq!(format_tin("009123456", None), "009-123-456");
        assert_eq!(format_tin("009123456", Some("00000")), "009-123-456-00000");
        assert_eq!(format_tin("009-123-456", None), "009-123-456");
        assert_eq!(format_tin("009 123 456", Some("")), "009-123-456");
        assert_eq!(format_tin("123", None), "123");
        assert_eq!(
            format_tin("00912345678901", Some("00000")),
            "009-123-456-00000"
        );
    }
}
