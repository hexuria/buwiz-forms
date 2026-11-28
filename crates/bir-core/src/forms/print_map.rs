//! Shaping shared by the forms' print maps (`to_print_field_map`): the
//! `derived:` values a frozen print sheet needs that the upload map does
//! not carry in the sheet's shape. Print only; the upload never reads them.

use std::collections::BTreeMap;

/// `text` over a comb of `width` slots: the first line broken at the last
/// space that fits (else at `width`), then the rest.
pub fn split_print_line(text: &str, width: usize) -> (String, String) {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        return (text.to_string(), String::new());
    }
    let cut = chars[..=width]
        .iter()
        .rposition(|ch| *ch == ' ')
        .filter(|at| *at > 0)
        .unwrap_or(width);
    let first: String = chars[..cut].iter().collect();
    let rest: String = chars[cut..].iter().collect();
    (first.trim_end().to_string(), rest.trim_start().to_string())
}

/// The digits of `fields[key]` for each key in order, joined: a later
/// page's one-comb TIN header from the page's TIN part keys.
pub fn digits_of(fields: &BTreeMap<String, String>, keys: &[&str]) -> String {
    keys.iter()
        .filter_map(|key| fields.get(*key))
        .flat_map(|part| part.chars().filter(char::is_ascii_digit))
        .collect()
}

/// A stored TIN (dashed or compact) as the 14 positions a header comb holds:
/// the nine TIN digits then the branch, zero-filled to five. A partial TIN
/// keeps its digits left to right and leaves the remaining positions blank.
pub fn tin_digits(tin: &str) -> String {
    let digits: String = tin.chars().filter(char::is_ascii_digit).collect();
    if digits.len() < 9 {
        return format!("{digits:<14}");
    }
    let (tin1, tin2, tin3, branch) = super::official_inputs::split_tin(tin);
    format!("{tin1}{tin2}{tin3}{branch}")
}

/// Formats a compact 12–14 digit TIN with standard BIR dashes
/// (`000-123-453-00000` / `000-123-453-000`) for plain text print cells
/// that have no comb ticks or preprinted dashes. Leaves blank, already-dashed,
/// or non-standard values unchanged.
pub fn format_tin_dashed(tin: &str) -> String {
    let trimmed = tin.trim();
    if (12..=14).contains(&trimmed.len()) && trimmed.bytes().all(|b| b.is_ascii_digit()) {
        format!(
            "{}-{}-{}-{}",
            &trimmed[0..3],
            &trimmed[3..6],
            &trimmed[6..9],
            &trimmed[9..]
        )
    } else {
        trimmed.to_string()
    }
}

/// Formats a count (installments, share count) for print without spurious
/// money decimals: `None` prints blank (`""`), whole numbers print without a
/// decimal point (`"12"`, `"1,000"`), and fractional values keep up to three
/// significant decimals (`"12,345.678"`, `"10.5"`). Upload XML is unaffected.
pub fn format_count(value: Option<f64>) -> String {
    let Some(v) = value else {
        return String::new();
    };
    if !v.is_finite() {
        return String::new();
    }
    let sign = if v < 0.0 { "-" } else { "" };
    let fixed = format!("{:.3}", v.abs());
    let (int_part, frac_part) = fixed.split_once('.').unwrap_or((fixed.as_str(), ""));
    let grouped = group_thousands_int(int_part);
    let frac = frac_part.trim_end_matches('0');
    if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{frac}")
    }
}

/// Blanks any listed date-part keys in a print map whose value is all zeros
/// (unset `0`, formatted as `"0"`, `"00"`, `"0000"`). Upload XML is unaffected.
pub fn blank_unset_date_parts(fields: &mut BTreeMap<String, String>, keys: &[&str]) {
    for &key in keys {
        if let Some(value) = fields.get_mut(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() && trimmed.bytes().all(|b| b == b'0') {
                value.clear();
            }
        }
    }
}

/// What a whole-peso amount comb prints for the upload amount `text` (as
/// `formatCurrency` writes it: `600,000.50`, `-1,000.00`, `(26,000)`): the
/// same figure without separators, its sign kept, and its centavos kept
/// whenever they are not `.00` (`600000.50`, `-1000`, `-26000`). The sheets
/// say "DO NOT enter Centavos", but the official page shows the centavos a
/// filer typed and the upload carries them, so the print never rounds them
/// away. `(negative, figure)`: `figure` is unsigned, `"0"` for zero (which
/// is never negative). `None` for text that is not an amount.
pub fn whole_peso_comb_figure(text: &str) -> Option<(bool, String)> {
    let trimmed = text.trim();
    let (negative, body) = match trimmed.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        Some(inner) => (true, inner.trim()),
        None => match trimmed.strip_prefix('-') {
            Some(rest) => (true, rest.trim()),
            None => (false, trimmed),
        },
    };
    let bare: String = body.chars().filter(|ch| *ch != ',').collect();
    let (whole, cents) = bare.split_once('.').unwrap_or((bare.as_str(), ""));
    if (whole.is_empty() && cents.is_empty())
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !cents.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let figure = if cents.bytes().all(|b| b == b'0') {
        whole.to_string()
    } else {
        format!("{whole}.{cents}")
    };
    let zero = figure.bytes().all(|b| matches!(b, b'0' | b'.'));
    Some((negative && !zero, figure))
}

/// [`whole_peso_comb_figure`] as one comb's print value: signed with a
/// minus (the official page's `formatCurrency` form), right-aligned in
/// `slots` boxes when it has no centavos and fits them; otherwise left
/// unpadded, and the print engine lays it over the comb as one plain
/// rectangle (a point never goes in a digit box).
pub fn whole_peso_comb_text(text: &str, slots: usize) -> Option<String> {
    let (negative, figure) = whole_peso_comb_figure(text)?;
    let printed = if negative {
        format!("-{figure}")
    } else {
        figure
    };
    Some(if printed.len() <= slots && !printed.contains('.') {
        format!("{printed:>slots$}")
    } else {
        printed
    })
}

fn group_thousands_int(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (idx, ch) in digits.chars().enumerate() {
        let remaining = digits.len() - idx;
        if idx > 0 && remaining.is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// A later page's header when the sheet draws its TIN and Taxpayer's Name
/// boxes as one 40-slot comb: the 14 TIN digits, one blank slot, then the
/// name from slot 15 (under the Taxpayer's Name caption).
pub fn tin_name_header(tin: &str, name: &str) -> String {
    let digits = tin_digits(tin);
    let name = name.trim().to_uppercase();
    if name.is_empty() {
        digits
    } else {
        format!("{digits}  {name}")
    }
}

/// The printed names of an excise place's region, province and city codes
/// (the upload carries the codes); a region prints its short name
/// (`REGION IV-A`, `NCR`). Blank for the `00` placeholder or an unknown code.
pub fn place_names(region: &str, province: &str, city: &str) -> [String; 3] {
    let name = |code: &str, short: bool| {
        let code = code.trim();
        let Some(name) = super::excise_places::place_name(code).filter(|_| !code.is_empty()) else {
            return String::new();
        };
        let name = if short {
            name.split(" - ").next().unwrap_or(name)
        } else {
            name
        };
        name.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    [name(region, true), name(province, false), name(city, false)]
}

/// Item "If yes, specify" text for the official tax-relief dropdown
/// position (`1` special rate, `2` treaty, `3` both); blank otherwise.
pub fn tax_relief_label(code: &str) -> &'static str {
    match code.trim() {
        "1" => "SPECIAL RATE",
        "2" => "INTERNATIONAL TAX TREATY",
        "3" => "SPECIAL RATE AND TAX TREATY",
        _ => "",
    }
}

/// Apply the same upper-casing the upload applies to each key on `form_id`:
/// every key marked `uppercase: true` in `official_xml::layout(form_id)`, plus
/// any key whose `official` upload map holds the same text in a different case.
/// Email keys and any key the upload does not upper-case stay as held.
pub fn match_upload_case(
    form_id: &str,
    fields: &mut BTreeMap<String, String>,
    official: Option<&BTreeMap<String, String>>,
) {
    crate::official_xml::apply_official_uppercase(form_id, fields);
    if let Some(official) = official {
        for (key, value) in fields.iter_mut() {
            if key.to_ascii_lowercase().contains("email") {
                continue;
            }
            if let Some(off) = official.get(key) {
                if off.eq_ignore_ascii_case(value) && off != value {
                    *value = off.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A whole-peso comb prints the upload's own figure: centavos kept
    /// (`600,000.50` printed `600001` before), sign kept, `.00` dropped.
    #[test]
    fn whole_peso_combs_print_the_uploads_figure() {
        assert_eq!(
            whole_peso_comb_text("600,000.50", 8).as_deref(),
            Some("600000.50")
        );
        assert_eq!(
            whole_peso_comb_text("-1,000.51", 8).as_deref(),
            Some("-1000.51")
        );
        assert_eq!(
            whole_peso_comb_text("10,000.00", 8).as_deref(),
            Some("   10000")
        );
        assert_eq!(
            whole_peso_comb_text("(26,000)", 8).as_deref(),
            Some("  -26000")
        );
        assert_eq!(
            whole_peso_comb_text("123,456,789.00", 8).as_deref(),
            Some("123456789")
        );
        assert_eq!(whole_peso_comb_text("-0.00", 3).as_deref(), Some("  0"));
        assert_eq!(whole_peso_comb_text("abc", 8), None);
        assert_eq!(
            whole_peso_comb_figure("0.40"),
            Some((false, "0.40".to_string()))
        );
    }

    #[test]
    fn splits_at_the_last_space_that_fits() {
        assert_eq!(
            split_print_line("123 SAMPLE STREET, BARANGAY EXAMPLE, QUEZON CITY", 40),
            (
                "123 SAMPLE STREET, BARANGAY EXAMPLE,".to_string(),
                "QUEZON CITY".to_string()
            )
        );
        assert_eq!(
            split_print_line("SHORT", 40),
            ("SHORT".to_string(), String::new())
        );
        assert_eq!(
            split_print_line("ABCDEFGHIJ", 4),
            ("ABCD".to_string(), "EFGHIJ".to_string())
        );
    }

    #[test]
    fn header_puts_the_name_after_fourteen_tin_digits() {
        let tin = "123-456-788-001";
        let (tin1, tin2, tin3, branch) = crate::forms::official_inputs::split_tin(tin);
        assert_eq!(tin_digits(tin), format!("{tin1}{tin2}{tin3}{branch}"));
        assert_eq!(
            tin_name_header("123456788", " Sample Co "),
            "12345678800000  SAMPLE CO"
        );
    }

    #[test]
    fn print_tin_matches_the_upload_split_for_complete_tins() {
        for tin in [
            "123456788",
            "1234567881",
            "123456788001",
            "123456788123",
            "1234567881234",
            "12345678812345",
        ] {
            let (tin1, tin2, tin3, branch) = crate::forms::official_inputs::split_tin(tin);
            assert_eq!(tin_digits(tin), format!("{tin1}{tin2}{tin3}{branch}"));
        }
    }

    #[test]
    fn partial_tin_prints_existing_digits_then_blank_cells() {
        assert_eq!(tin_digits("12345678"), format!("12345678{}", " ".repeat(6)));
    }

    #[test]
    fn empty_tin_prints_blank_cells_and_keeps_the_name_in_place() {
        assert_eq!(tin_digits(""), " ".repeat(14));
        assert_eq!(
            tin_name_header("", "Sample Co"),
            format!("{}  SAMPLE CO", " ".repeat(14))
        );
    }

    #[test]
    fn digits_follow_the_key_order() {
        let fields: BTreeMap<String, String> = [("a", "123"), ("b", "45-6"), ("c", "00000")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        assert_eq!(
            digits_of(&fields, &["a", "b", "c", "missing"]),
            "12345600000"
        );
    }

    #[test]
    fn blank_unset_date_parts_clears_zero_values_only() {
        let mut fields: BTreeMap<String, String> = [
            ("mm", "00"),
            ("dd", "0"),
            ("yyyy", "0000"),
            ("set_mm", "06"),
            ("set_yyyy", "2026"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        blank_unset_date_parts(&mut fields, &["mm", "dd", "yyyy", "set_mm", "set_yyyy"]);
        assert_eq!(fields["mm"], "");
        assert_eq!(fields["dd"], "");
        assert_eq!(fields["yyyy"], "");
        assert_eq!(fields["set_mm"], "06");
        assert_eq!(fields["set_yyyy"], "2026");
    }

    #[test]
    fn unset_date_parts_print_blank_across_all_15_affected_forms() {
        use crate::TaxpayerProfile;
        use crate::forms::form_1600wp::Form1600WpDraft;
        use crate::forms::form_1606::Form1606Draft;
        use crate::forms::form_1706::Form1706Draft;
        use crate::forms::form_1707::Form1707Draft;
        use crate::forms::form_1707a::Form1707ADraft;
        use crate::forms::form_1800::Form1800Draft;
        use crate::forms::form_1801::Form1801Draft;
        use crate::forms::form_2200a::Form2200ADraft;
        use crate::forms::form_2200an::Form2200ANDraft;
        use crate::forms::form_2200c::Form2200CDraft;
        use crate::forms::form_2200m::Form2200MDraft;
        use crate::forms::form_2200p::Form2200PDraft;
        use crate::forms::form_2200s::Form2200SDraft;
        use crate::forms::form_2200t::Form2200TDraft;
        use crate::forms::form_2552::Form2552Draft;

        let p: TaxpayerProfile = serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "JUANA SANTOS",
            "tin": crate::Tin::parse_digits("00012345300000").unwrap(),
            "rdo_code": "049",
            "line_of_business": "SERVICES",
            "registered_address": "123 RIZAL AVE MAKATI CITY",
            "zip_code": "1209",
            "phone": "09171234567",
            "email": "juana@example.com",
            "default_form_type": "1707"
        }))
        .expect("demo taxpayer profile");

        macro_rules! check_form {
            ($draft:expr, $zero:expr, [$($key:expr),+ $(,)?]) => {{
                let mut d = $draft;
                $zero(&mut d);
                let upload = d.to_bir_field_map();
                let print = d.to_print_field_map();
                $(
                    assert_eq!(
                        print[$key], "",
                        "{}: print map must blank unset date key {}",
                        stringify!($draft),
                        $key
                    );
                    if let Some(up) = upload.get($key) {
                        assert!(
                            !up.is_empty() && up.bytes().all(|b| b == b'0'),
                            "{}: upload map should remain unchanged for {}",
                            stringify!($draft),
                            $key
                        );
                    }
                )+
            }};
        }

        check_form!(
            Form1600WpDraft::new_from_profile(&p, 2026, 6),
            |d: &mut Form1600WpDraft| {
                d.month = 0;
                d.day = 0;
                d.year = 0;
            },
            [
                "frm1600WP:DateWithholdingMonth",
                "frm1600WP:DateWithholdingDay",
                "frm1600WP:DateWithholdingYear",
                "frm1600WP:DateWithholdingToMonth",
                "frm1600WP:DateWithholdingToDay",
                "frm1600WP:DateWithholdingToYear",
                "derived:att_from_mm",
                "derived:att_from_dd",
                "derived:att_from_yyyy",
                "derived:att_to_mm",
                "derived:att_to_dd",
                "derived:att_to_yyyy",
            ]
        );
        check_form!(
            Form1606Draft::new_from_profile(&p, 2026, 1),
            |d: &mut Form1606Draft| {
                d.transaction_month = 0;
                d.transaction_day = 0;
                d.transaction_year = 0;
            },
            [
                "frm1606:txtDateMonth",
                "frm1606:txtDateDay",
                "frm1606:txtDateYear"
            ]
        );
        check_form!(
            Form1706Draft::new_from_profile(&p, 2026, 1),
            |d: &mut Form1706Draft| {
                d.transaction_month = 0;
                d.transaction_day = 0;
                d.transaction_year = 0;
            },
            [
                "frm1706:txtDateMonth",
                "frm1706:txtDateDay",
                "frm1706:txtDateYear"
            ]
        );
        check_form!(
            Form1707Draft::new_from_profile(&p, 2026, 1),
            |d: &mut Form1707Draft| {
                d.transaction_month = 0;
                d.transaction_day = 0;
                d.transaction_year = 0;
            },
            [
                "frm1707:txtPg1I1Month",
                "frm1707:txtPg1I1Day",
                "frm1707:txtPg1I1Year"
            ]
        );
        check_form!(
            Form1707ADraft::new_from_profile(&p, 2026, 1),
            |d: &mut Form1707ADraft| {
                d.year_end_month = 0;
                d.year_end_day = 0;
                d.year_end_year = 0;
            },
            [
                "frm1707Av2021:txtI1YearEndMonth",
                "frm1707Av2021:txtI1YearEndDay",
                "frm1707Av2021:txtI1YearEndYear",
            ]
        );
        check_form!(
            Form1800Draft::new_from_profile(&p, 2026, 1),
            |d: &mut Form1800Draft| {
                d.donation_month = 0;
                d.donation_day = 0;
                d.donation_year = 0;
            },
            [
                "frm1800v2018:txtMonth",
                "frm1800v2018:txtDate",
                "frm1800v2018:txtYear"
            ]
        );
        check_form!(
            Form1801Draft::new_from_profile(&p, 2026, 1),
            |d: &mut Form1801Draft| {
                d.death_month = 0;
                d.death_day = 0;
                d.death_year = 0;
            },
            [
                "frm1801v2018:txtDateMonth",
                "frm1801v2018:txtDateDay",
                "frm1801v2018:txtDateYear",
            ]
        );
        check_form!(
            Form2200ADraft::new_from_profile(&p, 2026, 6),
            |d: &mut Form2200ADraft| {
                d.month = 0;
                d.day = 0;
                d.year = 0;
            },
            [
                "frm2200Av2020:txtDateMonth",
                "frm2200Av2020:txtDateDay",
                "frm2200Av2020:txtDateYear",
            ]
        );
        check_form!(
            Form2200ANDraft::new_from_profile(&p, 2026, 6),
            |d: &mut Form2200ANDraft| {
                d.month = 0;
                d.day = 0;
                d.year = 0;
            },
            [
                "frm2200ANv2018:txtMonth",
                "frm2200ANv2018:txtDate",
                "frm2200ANv2018:txtYear"
            ]
        );
        check_form!(
            Form2200CDraft::new_from_profile(&p, 2026),
            |d: &mut Form2200CDraft| {
                d.month = 0;
                d.day = 0;
                d.year = 0;
            },
            [
                "frm2200C:txtPg1I1Month",
                "frm2200C:txtPg1I1Day",
                "frm2200C:txtPg1I1Year"
            ]
        );
        check_form!(
            Form2200MDraft::new_from_profile(&p, 2026, 6),
            |d: &mut Form2200MDraft| {
                d.month = 0;
                d.day = 0;
                d.year = 0;
            },
            [
                "frm2200M:txtPg1I1Month",
                "frm2200M:txtPg1I1Day",
                "frm2200M:txtPg1I1Year"
            ]
        );
        check_form!(
            Form2200PDraft::new_from_profile(&p, 2026, 6),
            |d: &mut Form2200PDraft| {
                d.month = 0;
                d.day = 0;
                d.year = 0;
            },
            [
                "frm2200P:txtPg1I1Month",
                "frm2200P:txtPg1I1Day",
                "frm2200P:txtPg1I1Year"
            ]
        );
        check_form!(
            Form2200SDraft::new_from_profile(&p, 2026, 6),
            |d: &mut Form2200SDraft| {
                d.month = 0;
                d.day = 0;
                d.year = 0;
            },
            [
                "frm2200S:txtMonth1",
                "frm2200S:txtDate",
                "frm2200S:txtForYr"
            ]
        );
        check_form!(
            Form2200TDraft::new_from_profile(&p, 2026, 6),
            |d: &mut Form2200TDraft| {
                d.month = 0;
                d.day = 0;
                d.year = 0;
            },
            [
                "frm2200Tv2020:txtDateMonth",
                "frm2200Tv2020:txtDateDay",
                "frm2200Tv2020:txtDateYear",
            ]
        );
        check_form!(
            Form2552Draft::new_from_profile(&p, 2026, 6, 15),
            |d: &mut Form2552Draft| {
                d.transaction_month = 0;
                d.transaction_day = 0;
                d.transaction_year = 0;
            },
            [
                "frm2552:txtPg1I1Month",
                "frm2552:txtPg1I1Day",
                "frm2552:txtPg1I1Year"
            ]
        );
    }
}
