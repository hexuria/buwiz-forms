//! Money text from form_page.js: formatMoney (what an amount box shows) and wholePesosOf (the whole-peso
//! amount a "Use it" hint offers). Pure.

/// An amount in whole pesos, and whether the centavos were dropped to reach it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WholePesos {
    pub amount: i64,
    pub rounded: bool,
}

fn parse_amount(raw: &str) -> Option<f64> {
    let cleaned: String = raw
        .chars()
        .filter(|c| !matches!(c, '₱' | ',' | ' ' | '\t' | '\n' | '\r'))
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok()
}

/// "1,234.50" from a number-ish string; empty stays empty. `whole`: a box over a whole-number field shows
/// whole pesos ("1,234"); a typed fraction stays as typed so the box's own message explains it.
pub fn format_money(text: &str, whole: bool) -> String {
    let n = match parse_amount(text) {
        Some(n) => n,
        None => return text.to_owned(),
    };
    if whole && n.fract() != 0.0 {
        return text.to_owned();
    }
    // Group the digits by hand (no Intl).
    let (sign, abs) = if n < 0.0 { ("-", -n) } else { ("", n) };
    let decimals = if whole { 0 } else { 2 };
    let scaled = (abs * 10f64.powi(decimals)).round() as u64;
    let pow = 10u64.pow(decimals as u32);
    let (int, frac) = (scaled / pow, scaled % pow);
    let digits = int.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    if whole {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{frac:02}")
    }
}

/// What an amount box shows, in whole pesos rounded half up (f64::round is half away from zero, as
/// Math.round), and whether that dropped centavos; `None` for a blank, zero or unreadable box.
pub fn whole_pesos_of(text: &str) -> Option<WholePesos> {
    let n = parse_amount(text)?;
    if !n.is_finite() {
        return None;
    }
    let amount = n.round() as i64;
    if amount == 0 {
        return None;
    }
    Some(WholePesos {
        amount,
        rounded: amount as f64 != n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_the_digits() {
        assert_eq!(format_money("1234.5", false), "1,234.50");
        assert_eq!(format_money("1234", true), "1,234");
        assert_eq!(format_money("₱1,234.50", false), "1,234.50");
        assert_eq!(format_money("-2500.25", false), "-2,500.25");
        assert_eq!(format_money("abc", false), "abc");
        assert_eq!(format_money("", false), "");
        // A typed fraction in a whole field stays as typed.
        assert_eq!(format_money("1234.5", true), "1234.5");
    }

    #[test]
    fn whole_pesos_rounds_and_reports() {
        assert_eq!(
            whole_pesos_of("1234.50"),
            Some(WholePesos {
                amount: 1235,
                rounded: true
            })
        );
        assert_eq!(
            whole_pesos_of("1234"),
            Some(WholePesos {
                amount: 1234,
                rounded: false
            })
        );
        assert_eq!(whole_pesos_of("0"), None);
        assert_eq!(whole_pesos_of(""), None);
        assert_eq!(whole_pesos_of("abc"), None);
    }
}
