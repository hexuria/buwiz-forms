//! The form controls' pure rules — `nsh/screens/form/controls/model.ts`.
//!
//! Which options a list has, what a shown value is, how a TIN box is told, how a search matches,
//! what a read-only box shows, and how a schedule row is titled. No UI here, so it is tested on its
//! own; the rules are the old page's unless a comment says otherwise. `model.test.ts` is mirrored
//! in the tests below.

use crate::form::schema::RowLabels;
use crate::lib::dates;
use crate::lib::money;
use crate::types::ScreenFormOption;

/// The choice shown first in every list: nothing chosen.
pub const CHOOSE: &str = "Choose…";

/// A listed option's (value, label) from a schema `o` string or object — `schema::option_of`
/// already drops template text and splits "CODE - Label"; this wraps it as a `ScreenFormOption`.
pub fn option_of(o: &serde_json::Value) -> Option<ScreenFormOption> {
    crate::form::schema::option_of(o).map(|(value, label)| ScreenFormOption {
        value,
        label,
        selected: None,
    })
}

/// A sel box's listed choices: the host's win, else the schema's own (`listedOptions`).
pub fn listed_options(
    field_o: &Option<Vec<String>>,
    host: &[ScreenFormOption],
) -> Vec<ScreenFormOption> {
    if !host.is_empty() {
        return host.to_vec();
    }
    field_o
        .as_ref()
        .map(|os| {
            os.iter()
                .filter_map(|o| option_of(&serde_json::Value::String(o.clone())))
                .collect()
        })
        .unwrap_or_default()
}

/// The choices a list shows (with the empty one first) and the value it shows as chosen
/// (`selectModel`, the old selectBox): an unknown all-zero code or a lone "-" reads as empty; a
/// listed 0 or "-" stays selectable; an unknown value stays choosable by being appended.
pub fn select_model(listed: &[ScreenFormOption], value: &str) -> (Vec<ScreenFormOption>, String) {
    let mut options = vec![ScreenFormOption {
        value: String::new(),
        label: CHOOSE.to_owned(),
        selected: None,
    }];
    options.extend(listed.iter().cloned());
    let selected = listed.iter().find(|o| o.selected == Some(true));
    let known = options.iter().any(|o| o.value == value);
    // /^(0+|-)$/ — only zeros, or a lone dash.
    let zero_or_dash = (!value.is_empty() && value.chars().all(|c| c == '0')) || value == "-";
    let shown = if let Some(sel) = selected {
        sel.value.clone()
    } else if known {
        value.to_owned()
    } else if zero_or_dash {
        String::new()
    } else {
        value.to_owned()
    };
    if !shown.is_empty() && selected.is_none() && !known {
        options.push(ScreenFormOption {
            value: shown.clone(),
            label: shown.clone(),
            selected: None,
        });
    }
    (options, shown)
}

/// A tick box reads as ticked (`isOn`: the old page's ticked values).
pub fn is_on(value: &str) -> bool {
    matches!(value, "true" | "1" | "Y")
}

/// What a text box that holds a TIN is (`tinKind`): its whole 14 digits ("full"), one of the
/// 3-3-3-5 boxes ("part"), or no TIN at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TinKind {
    Full,
    Part,
}

/// `tinKind(field)`: `k` the key, `l` the label, `t` the type, `m` the mask length.
pub fn tin_kind(k: &str, l: &str, t: &str, m: Option<f64>) -> Option<TinKind> {
    if t != "text" && t != "int" {
        return None;
    }
    let key = k.rsplit(':').next().unwrap_or(k);
    // `/(^|[a-z0-9_])(TIN|Tin|tin)(?![a-z])/` — rust-regex has no lookahead, so the char after each
    // match is checked by hand.
    static KEY_RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(^|[a-z0-9_])(TIN|Tin|tin)").unwrap());
    static LABEL_TIN_RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(^|[^A-Za-z])TIN").unwrap());
    static LABEL_NOF_RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)\bTI nof\b").unwrap());
    static BRANCH_RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)branch ?code").unwrap());
    static BRANCH_KEY_RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"BranchCode").unwrap());
    let not_lower_after = |text: &str, m: regex::Match| {
        text[m.end()..]
            .chars()
            .next()
            .map(|c| !c.is_ascii_lowercase())
            .unwrap_or(true)
    };
    let key_named = KEY_RE.find_iter(key).any(|m| not_lower_after(key, m));
    let label_tin_named = LABEL_TIN_RE.find_iter(l).any(|m| {
        // LABEL_TIN_RE ends right after "TIN"; the lookahead applies to the char after it.
        not_lower_after(l, m)
    });
    let named = key_named
        || label_tin_named
        || LABEL_NOF_RE.is_match(l)
        || BRANCH_RE.is_match(l)
        || BRANCH_KEY_RE.is_match(key);
    if !named {
        return None;
    }
    let m = m?;
    if t == "text" && (14.0..=17.0).contains(&m) {
        return Some(TinKind::Full);
    }
    if m <= 5.0 {
        return Some(TinKind::Part);
    }
    None
}

/// A date text as the date box takes it: MM/DD/YYYY once it reads as a date, else as it is (`mdy`).
pub fn mdy(text: &str) -> String {
    dates::parse_typed_date(text)
        .map(dates::format_mdy)
        .unwrap_or_else(|| text.to_owned())
}

/// What a read-only box shows for the box's value (`readText`): an amount grouped (a blank computed
/// one 0.00), a date long, a choice by its label.
///
/// `t`/`w`/`o` are the field's type, whole flag and schema options; `value` the box's value;
/// `host` its host options; `zero_when_blank` the box's computed-blank flag.
pub fn read_text(
    t: &str,
    w: Option<bool>,
    o: &Option<Vec<String>>,
    value: &str,
    host: &[ScreenFormOption],
    zero_when_blank: bool,
) -> String {
    match t {
        "money" => {
            let v = if value.is_empty() && zero_when_blank {
                "0"
            } else {
                value
            };
            money::format_money(v, w == Some(true))
        }
        "date" => dates::shown_date(value),
        "sel" => {
            let listed = listed_options(o, host);
            let (_options, shown) = select_model(&listed, value);
            if shown.is_empty() {
                return String::new();
            }
            listed
                .iter()
                .find(|op| op.value == shown)
                .map(|op| op.label.clone())
                .unwrap_or(shown)
        }
        _ => value.to_owned(),
    }
}

/// An option's code and its words (`splitLabel`): "WI010 — Professional fees" ->
/// ["WI010", "Professional fees"]; one run of text -> [text, ""].
pub fn split_label(label: &str) -> (String, String) {
    for sep in [" — ", " – ", " - "] {
        if let Some((head, tail)) = label.split_once(sep) {
            if !head.is_empty() && !head.contains(char::is_whitespace) && !tail.is_empty() {
                return (head.to_owned(), tail.to_owned());
            }
        }
    }
    (label.to_owned(), String::new())
}

/// The options whose label or code holds every typed word, in any order (`matchWords`).
pub fn match_words(
    options: &[ScreenFormOption],
    text: &str,
    limit: usize,
) -> Vec<ScreenFormOption> {
    let words: Vec<String> = text
        .trim()
        .to_lowercase()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let mut out = Vec::new();
    for o in options {
        if out.len() >= limit {
            break;
        }
        let hay = format!("{} {}", o.value, o.label).to_lowercase();
        if words.iter().all(|w| hay.contains(w.as_str())) {
            out.push(o.clone());
        }
    }
    out
}

/// A schedule row's number as printed, and its preprinted caption ("" when it has none) —
/// `rowParts`.
pub fn row_parts(start: Option<f64>, row_labels: Option<&RowLabels>, row: usize) -> (i64, String) {
    let n = start.unwrap_or(1.0) as i64 + row as i64;
    let caption = row_labels
        .and_then(|l| l.rows.get(row))
        .cloned()
        .unwrap_or_default();
    (n, caption)
}

/// The drawer's title of a row (`rowTitleOf`): "Row 3", with its caption after a dot where the
/// sheet prints one.
pub fn row_title_of(start: Option<f64>, row_labels: Option<&RowLabels>, row: usize) -> String {
    let (n, caption) = row_parts(start, row_labels, row);
    if caption.is_empty() {
        format!("Row {n}")
    } else {
        format!("Row {n} · {caption}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn opt(value: &str, label: &str) -> ScreenFormOption {
        ScreenFormOption {
            value: value.to_owned(),
            label: label.to_owned(),
            selected: None,
        }
    }

    #[test]
    fn option_of_code_label_and_template_drop() {
        assert_eq!(
            option_of(&json!("03 - March")),
            Some(opt("03", "03 - March"))
        );
        assert_eq!(option_of(&json!("Single")), Some(opt("Single", "Single")));
        assert_eq!(option_of(&json!("\" + rdo.description + \"")), None);
        assert_eq!(
            option_of(&json!({"value": "WI010", "label": "WI010 — Fees"})),
            Some(opt("WI010", "WI010 — Fees"))
        );
    }

    #[test]
    fn listed_options_host_wins() {
        let host = vec![opt("x", "X")];
        assert_eq!(
            listed_options(&Some(vec!["01 - A".to_owned()]), &host),
            host
        );
        assert_eq!(
            listed_options(&Some(vec!["01 - A".to_owned(), "\"x\"".to_owned()]), &[]),
            vec![opt("01", "01 - A")]
        );
        assert!(listed_options(&None, &[]).is_empty());
    }

    #[test]
    fn select_model_empty_first_and_unknown_stays() {
        let listed = vec![opt("01", "01 - January"), opt("02", "02 - February")];
        let (options, shown) = select_model(&listed, "02");
        assert_eq!(shown, "02");
        assert_eq!(options[0].value, "");
        assert_eq!(options[0].label, CHOOSE);
        for v in ["", "00", "-"] {
            assert_eq!(select_model(&listed, v).1, "");
        }
        let (odd_options, odd_shown) = select_model(&listed, "13");
        assert_eq!(odd_shown, "13");
        assert_eq!(odd_options.last().unwrap().value, "13");
        // A listed 0 stays selectable.
        assert_eq!(select_model(&[opt("0", "0")], "0").1, "0");
        // Two options sharing a code: the host's marked one is the chosen one.
        let mut twin = vec![opt("2000", "2000 — A"), opt("2000", "2000 — B")];
        twin[1].selected = Some(true);
        assert_eq!(select_model(&twin, "2000").1, "2000");
    }

    #[test]
    fn is_on_ticked_values() {
        for v in ["true", "1", "Y"] {
            assert!(is_on(v));
        }
        for v in ["false", "", "0", "N"] {
            assert!(!is_on(v));
        }
    }

    #[test]
    fn tin_kind_full_and_part() {
        assert_eq!(
            tin_kind(
                "frm1707:txtPg1I7BuyerTIN0",
                "Buyer TIN0",
                "text",
                Some(14.0)
            ),
            Some(TinKind::Full)
        );
        assert_eq!(
            tin_kind("frm1700:txtPg2Pt4TIN3", "TIN3", "text", Some(3.0)),
            Some(TinKind::Part)
        );
        assert_eq!(
            tin_kind("frm1701:txtPg3BranchCode", "Branch code", "text", Some(5.0)),
            Some(TinKind::Part)
        );
        assert_eq!(
            tin_kind(
                "frm1601EQ:txtTIN1",
                "Taxpayer Identification Number",
                "int",
                Some(3.0)
            ),
            Some(TinKind::Part)
        );
        assert_eq!(
            tin_kind(
                "frm1702EX:txtPg1Pt2TINofSignatory",
                "TI nof signatory",
                "text",
                Some(14.0)
            ),
            Some(TinKind::Full)
        );
        // No length, no TIN: the box stays a text box.
        assert_eq!(tin_kind("frm1600PT:txtPg1TIN1", "TIN1", "text", None), None);
        assert_eq!(
            tin_kind("frm1:txtInterest", "Interest", "text", Some(14.0)),
            None
        );
        assert_eq!(
            tin_kind(
                "frm1:txtOperatingLoss",
                "Net operating loss",
                "text",
                Some(12.0)
            ),
            None
        );
        assert_eq!(tin_kind("frm1:txtTIN1", "TIN1", "money", Some(3.0)), None);
    }

    #[test]
    fn mdy_dates() {
        assert_eq!(mdy("12012020"), "12/01/2020");
        assert_eq!(mdy("Dec 1, 2020"), "12/01/2020");
        assert_eq!(mdy("2020-12-01"), "12/01/2020");
        assert_eq!(mdy("13/45"), "13/45");
        assert_eq!(mdy(""), "");
    }

    #[test]
    fn read_text_formats() {
        assert_eq!(
            read_text("money", None, &None, "1250000.5", &[], false),
            "1,250,000.50"
        );
        assert_eq!(read_text("money", None, &None, "", &[], true), "0.00");
        assert_eq!(read_text("money", None, &None, "", &[], false), "");
        assert_eq!(
            read_text("money", Some(true), &None, "48000", &[], false),
            "48,000"
        );
        assert_eq!(
            read_text("date", None, &None, "12/01/2020", &[], false),
            "Dec 1, 2020"
        );
        let o = Some(vec!["01 - January".to_owned()]);
        assert_eq!(read_text("sel", None, &o, "01", &[], false), "01 - January");
        assert_eq!(read_text("sel", None, &o, "", &[], false), "");
    }

    #[test]
    fn split_label_and_match_words() {
        assert_eq!(
            split_label("WI010 — Professional fees"),
            ("WI010".to_owned(), "Professional fees".to_owned())
        );
        assert_eq!(
            split_label("050 - South Makati"),
            ("050".to_owned(), "South Makati".to_owned())
        );
        assert_eq!(split_label("Single"), ("Single".to_owned(), String::new()));
        let rdo = vec![
            opt("050", "050 - South Makati"),
            opt("047", "047 - East Makati"),
            opt("039", "039 - South Quezon City"),
        ];
        assert_eq!(match_words(&rdo, "", usize::MAX).len(), 3);
        assert_eq!(
            match_words(&rdo, "makati south", usize::MAX)
                .iter()
                .map(|o| o.value.as_str())
                .collect::<Vec<_>>(),
            ["050"]
        );
        assert_eq!(
            match_words(&rdo, "050 makati", usize::MAX)
                .iter()
                .map(|o| o.value.as_str())
                .collect::<Vec<_>>(),
            ["050"]
        );
        assert_eq!(
            match_words(&rdo, "EAST", usize::MAX)
                .iter()
                .map(|o| o.value.as_str())
                .collect::<Vec<_>>(),
            ["047"]
        );
        assert!(match_words(&rdo, "zzz", usize::MAX).is_empty());
        assert_eq!(match_words(&rdo, "south", 1).len(), 1);
    }

    #[test]
    fn row_title_of_number_and_caption() {
        assert_eq!(row_title_of(None, None, 0), "Row 1");
        assert_eq!(row_title_of(Some(5.0), None, 2), "Row 7");
        let labels = RowLabels {
            caption: "Recipient".to_owned(),
            rows: vec!["Rent".to_owned(), "Fees".to_owned()],
        };
        assert_eq!(row_title_of(None, Some(&labels), 1), "Row 2 · Fees");
    }
}
