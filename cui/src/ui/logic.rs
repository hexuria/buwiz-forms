//! The kit's pure helpers (`nsh/ui/logic.ts`): avatar hues and initials, status chip wording,
//! the filters of the pick lists.
//!
//! `logic.test.ts` is mirrored in the tests below.

use std::collections::HashSet;

/// A stable hue (0-359) from a taxpayer id (TIN root) or a name (hueFor).
pub fn hue_for(key: &str) -> u32 {
    let mut h: u32 = 0;
    for c in key.chars() {
        h = (h * 31 + c as u32) % 360;
    }
    h
}

/// Initials: the first and last capitalised words ("Juana Dela Cruz Santos" -> "JS"); "?" without
/// one (initials).
pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name
        .split_whitespace()
        .filter(|w| w.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
        .collect();
    if words.is_empty() {
        return "?".to_owned();
    }
    let first = words[0].chars().next().unwrap();
    if words.len() > 1 {
        let last = words[words.len() - 1].chars().next().unwrap();
        format!("{first}{last}")
    } else {
        first.to_string()
    }
}

/// "#rrggbb" from HSL (degrees, percent, percent) (hslToHex).
pub fn hsl_to_hex(h: f64, s: f64, l: f64) -> String {
    let s = s / 100.0;
    let l = l / 100.0;
    let k = |n: f64| (n + h / 30.0).rem_euclid(12.0);
    let a = s * l.min(1.0 - l);
    let f = |n: f64| l - a * (k(n) - 3.0).min(9.0 - k(n)).min(1.0).max(-1.0);
    let hex = |x: f64| format!("{:02x}", (x * 255.0).round() as i64);
    format!("#{}{}{}", hex(f(0.0)), hex(f(8.0)), hex(f(4.0)))
}

/// Avatar tones per appearance (AVATAR_TONE).
const AVATAR_TONE_LIGHT: (f64, f64, f64) = (55.0, 90.0, 30.0);
const AVATAR_TONE_DARK: (f64, f64, f64) = (40.0, 26.0, 84.0);

/// The avatar's fill and initials colors for a key in an appearance (avatarColors).
pub fn avatar_colors(key: &str, dark: bool) -> (String, String) {
    let (s, bg, fg) = if dark {
        AVATAR_TONE_DARK
    } else {
        AVATAR_TONE_LIGHT
    };
    let hue = f64::from(hue_for(key));
    (hsl_to_hex(hue, s, bg), hsl_to_hex(hue, s, fg))
}

/// A status chip's words and tone (statusChip).
#[derive(Debug, Clone, PartialEq)]
pub struct StatusChip {
    pub label: String,
    pub tone: &'static str,
    pub check: bool,
    pub send: bool,
}

/// What a status chip says and its tone (statusChip).
pub fn status_chip(status: &str, days: Option<i64>) -> StatusChip {
    let n = days.unwrap_or(0).abs();
    let chip = |label: &str, tone: &'static str, check: bool| StatusChip {
        label: label.to_owned(),
        tone,
        check,
        send: false,
    };
    match status {
        "draft" => chip("Draft", "accent", false),
        "queued" => chip("Queued", "accent", false),
        "submitted" => StatusChip {
            label: "Submitted".to_owned(),
            tone: "neutral",
            check: false,
            send: true,
        },
        "confirmed" => chip("Confirmed", "success", true),
        "paid" => chip("Paid", "success", true),
        "filed" => chip("Filed", "success", true),
        "due_soon" => {
            let d = days.unwrap_or(0);
            chip(
                &if d == 0 {
                    "Due today".to_owned()
                } else {
                    format!("In {d} day{}", if d == 1 { "" } else { "s" })
                },
                "warning",
                false,
            )
        }
        "overdue" => chip(
            &format!("{n} day{} overdue", if n == 1 { "" } else { "s" }),
            "danger",
            false,
        ),
        _ => chip("Not started", "neutral", false),
    }
}

/// One choice of a pick list (PickOption).
#[derive(Debug, Clone)]
pub struct PickOption {
    pub value: String,
    pub label: String,
    /// A second line: a TIN, a form's title, "3 returns".
    pub sub: Option<String>,
}

/// Options whose label (and second line) hold every typed word, ignoring case; all of them for an
/// empty text (matchOptions).
pub fn match_options<T: std::borrow::Borrow<PickOption> + Clone>(
    options: &[T],
    text: &str,
    limit: usize,
) -> Vec<T> {
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
        let opt = o.borrow();
        let hay = format!("{} {}", opt.label, opt.sub.clone().unwrap_or_default()).to_lowercase();
        if words.iter().all(|w| hay.contains(w.as_str())) {
            out.push(o.clone());
        }
    }
    out
}

/// The words of a multi-select's summary (MultiComboWords).
pub struct MultiComboWords {
    /// "All forms": every value on.
    pub all: &'static str,
    /// "forms": "3 forms", "No forms".
    pub nouns: &'static str,
}

/// A multi-select's closed summary (multiComboLabel). `selected` None = all.
pub fn multi_combo_label(
    words: &MultiComboWords,
    options: &[PickOption],
    selected: Option<&HashSet<String>>,
) -> String {
    let on: Vec<&PickOption> = match selected {
        None => options.iter().collect(),
        Some(sel) => options.iter().filter(|o| sel.contains(&o.value)).collect(),
    };
    if on.len() == options.len() {
        return words.all.to_owned();
    }
    if on.is_empty() {
        return format!("No {}", words.nouns);
    }
    if on.len() == 1 {
        return on[0].label.clone();
    }
    format!("{} {}", on.len(), words.nouns)
}

/// A selection handed to the page: None once it holds every option (normalizeSelection).
pub fn normalize_selection(
    next: &HashSet<String>,
    options: &[PickOption],
) -> Option<HashSet<String>> {
    if options.iter().all(|o| next.contains(&o.value)) {
        None
    } else {
        Some(next.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_initials_and_hues() {
        assert_eq!(initials("Juana Santos"), "JS");
        assert_eq!(initials("Juana dela Cruz Santos"), "JS");
        assert_eq!(initials("Juana"), "J");
        assert_eq!(initials(""), "?");
        assert_eq!(hue_for("000123453"), hue_for("000123453"));
        assert!(hue_for("a") < 360);
        let (bg, _fg) = avatar_colors("Juana Santos", false);
        assert!(bg.starts_with('#') && bg.len() == 7);
        let (dark_bg, _) = avatar_colors("Juana Santos", true);
        assert_ne!(dark_bg, bg);
    }

    #[test]
    fn t_status_chips() {
        assert_eq!(status_chip("due_soon", Some(0)).label, "Due today");
        assert_eq!(status_chip("due_soon", Some(1)).label, "In 1 day");
        assert_eq!(
            status_chip("due_soon", Some(3)),
            StatusChip {
                label: "In 3 days".to_owned(),
                tone: "warning",
                check: false,
                send: false
            }
        );
        assert_eq!(
            status_chip("overdue", Some(-2)),
            StatusChip {
                label: "2 days overdue".to_owned(),
                tone: "danger",
                check: false,
                send: false
            }
        );
        assert!(status_chip("filed", None).check);
        assert_eq!(status_chip("other", None).label, "Not started");
        let sub = status_chip("submitted", None);
        assert_eq!(sub.label, "Submitted");
        assert!(sub.send);
    }

    fn rdo() -> Vec<PickOption> {
        [
            ("050", "050 - South Makati"),
            ("047", "047 - East Makati"),
            ("039", "039 - South Quezon City"),
        ]
        .iter()
        .map(|(v, l)| PickOption {
            value: v.to_string(),
            label: l.to_string(),
            sub: None,
        })
        .collect()
    }

    #[test]
    fn t_match_options() {
        let opts = rdo();
        assert_eq!(match_options(&opts, "", usize::MAX).len(), 3);
        let vals = |t: &str| -> Vec<String> {
            match_options(&opts, t, usize::MAX)
                .iter()
                .map(|o| o.value.clone())
                .collect()
        };
        assert_eq!(vals("makati"), vec!["050", "047"]);
        assert_eq!(vals("050 makati"), vec!["050"]);
        assert_eq!(vals("south"), vec!["050", "039"]);
        assert!(vals("nowhere").is_empty());
        assert_eq!(match_options(&opts, "", 2).len(), 2);
    }

    #[test]
    fn t_multi_combo() {
        let opts = rdo();
        let words = MultiComboWords {
            all: "All forms",
            nouns: "forms",
        };
        assert_eq!(multi_combo_label(&words, &opts, None), "All forms");
        assert_eq!(
            multi_combo_label(&words, &opts, Some(&HashSet::new())),
            "No forms"
        );
        let one: HashSet<String> = ["047".to_owned()].into_iter().collect();
        assert_eq!(
            multi_combo_label(&words, &opts, Some(&one)),
            "047 - East Makati"
        );
        let two: HashSet<String> = ["047".to_owned(), "050".to_owned()].into_iter().collect();
        assert_eq!(multi_combo_label(&words, &opts, Some(&two)), "2 forms");
        let all: HashSet<String> = ["047", "050", "039"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(normalize_selection(&all, &opts), None);
        let some: HashSet<String> = ["047".to_owned()].into_iter().collect();
        assert_eq!(normalize_selection(&some, &opts).unwrap(), some);
    }
}
