//! Filings' pure model (`nsh/screens/filings/model.ts` + `filters.ts` preset half): statuses and
//! board columns, how a return is named and dated, the period/month keys the filters use, and
//! which returns the search and the four filters keep. No host calls — `model.test.ts` is
//! mirrored in the tests below.

use std::collections::HashSet;

use serde_json::Value;

use chrono::Datelike;

use super::dates::{MONTHS, short_date};
use super::periods::{self, FormIndexEntry, Frequency};
use crate::types::{DeadlineRow, ReturnRow};

/// The real statuses, in board order (STATUSES).
pub const STATUSES: [&str; 5] = ["Draft", "Queued", "Submitted", "Confirmed", "Paid"];

/// A board column: its id, its title and the statuses it holds (Paid joins Confirmed).
#[derive(Debug, Clone, Copy)]
pub struct Column {
    pub id: &'static str,
    pub title: &'static str,
    pub statuses: &'static [&'static str],
}

/// The four board columns, in order (COLUMNS).
pub const COLUMNS: [Column; 4] = [
    Column {
        id: "draft",
        title: "Draft",
        statuses: &["Draft"],
    },
    Column {
        id: "queued",
        title: "Queued",
        statuses: &["Queued"],
    },
    Column {
        id: "submitted",
        title: "Submitted",
        statuses: &["Submitted"],
    },
    Column {
        id: "confirmed",
        title: "Confirmed",
        statuses: &["Confirmed", "Paid"],
    },
];

/// "1 return", "3 returns" (plural in model.ts / logic.ts).
pub fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// The frequency of a form code in the index; Annual when unlisted (frequencyOf).
fn freq_of(index: &[FormIndexEntry], code: &str) -> Frequency {
    periods::frequency_of(index, code)
}

/// Newest change first; a SQLite timestamp sorts as text (newestFirst).
pub fn newest_first(rows: &[ReturnRow]) -> Vec<ReturnRow> {
    let mut out = rows.to_vec();
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    out
}

/// "Q3 2026" for a monthly or quarterly return, "TY 2026" for the rest (periodKey).
pub fn period_key(index: &[FormIndexEntry], r: &ReturnRow) -> String {
    let freq = freq_of(index, &r.form_code);
    let q = match freq {
        Frequency::Monthly => (r.period + 2) / 3,
        Frequency::Quarterly => r.period,
        _ => 0,
    };
    if (1..=4).contains(&q) {
        format!("Q{q} {}", r.taxable_year)
    } else {
        format!("TY {}", r.taxable_year)
    }
}

/// Sort rank of a period key: newer years first, then later quarters, TY after Q4 (periodRank).
pub fn period_rank(key: &str) -> i64 {
    // /^(?:Q(\d)|TY) (\d{4})$/
    let b = key.as_bytes();
    if key.len() == 7 {
        if b[0] == b'Q' && b[1].is_ascii_digit() && b[2] == b' ' {
            if let Ok(y) = key[3..7].parse::<i64>() {
                return y * 10 + i64::from(b[1] - b'0');
            }
        }
        if &key[..2] == "TY" && b[2] == b' ' {
            if let Ok(y) = key[3..7].parse::<i64>() {
                return y * 10 + 5;
            }
        }
    }
    0
}

/// "2026-11" of a SQLite UTC timestamp, in local time (localMonth).
pub fn local_month(text: &str) -> String {
    // ^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})
    let b = text.as_bytes();
    let parsed = if b.len() >= 16
        && b[4] == b'-'
        && b[7] == b'-'
        && (b[10] == b' ' || b[10] == b'T')
        && b[13] == b':'
        && text[..4].chars().all(|c| c.is_ascii_digit())
        && text[5..7].chars().all(|c| c.is_ascii_digit())
        && text[8..10].chars().all(|c| c.is_ascii_digit())
        && text[11..13].chars().all(|c| c.is_ascii_digit())
        && text[14..16].chars().all(|c| c.is_ascii_digit())
    {
        let y: i32 = text[0..4].parse().unwrap_or(0);
        let mo: u32 = text[5..7].parse().unwrap_or(0);
        let d: u32 = text[8..10].parse().unwrap_or(0);
        let h: u32 = text[11..13].parse().unwrap_or(0);
        let mi: u32 = text[14..16].parse().unwrap_or(0);
        chrono::NaiveDate::from_ymd_opt(y, mo.max(1).min(12), d.max(1).min(28))
            .and_then(|date| {
                chrono::DateTime::from_timestamp(
                    date.and_hms_opt(h.min(23), mi.min(59), 0)?
                        .and_utc()
                        .timestamp(),
                    0,
                )
            })
            .map(|dt| {
                let local = dt.with_timezone(&chrono::Local);
                format!("{:04}-{:02}", local.year(), local.month())
            })
    } else {
        None
    };
    parsed.unwrap_or_else(|| text.chars().take(7).collect())
}

/// "Nov 2026" of "2026-11" (monthLabel).
pub fn month_label(ym: &str) -> String {
    let m: usize = ym.get(5..7).and_then(|s| s.parse().ok()).unwrap_or(0);
    if (1..=12).contains(&m) {
        format!("{} {}", MONTHS[m - 1], &ym[..4.min(ym.len())])
    } else {
        ym.to_owned()
    }
}

/// Head office is the taxpayer itself: only another branch is worth naming (namedBranch).
pub fn named_branch(r: &ReturnRow) -> String {
    if r.branch_code.chars().all(|c| c == '0') {
        String::new()
    } else {
        r.branch_name.clone()
    }
}

/// "Juana Santos" or "Juana Santos · Cebu branch" (whoLine). Kept for model parity; the card
/// draws the taxpayer and the branch as separate lines like ReturnCard.tsx.
#[allow(dead_code)]
pub fn who_line(r: &ReturnRow) -> String {
    let branch = named_branch(r);
    if branch.is_empty() {
        r.taxpayer_name.clone()
    } else {
        format!("{} · {}", r.taxpayer_name, branch)
    }
}

/// "2551Q Q3 2026", "1601C Jan 2026", "1702RT TY 2026": a return as Recent names it (returnTitle).
pub fn return_title(index: &[FormIndexEntry], r: &ReturnRow) -> String {
    let freq = freq_of(index, &r.form_code);
    let period = if freq == Frequency::AsNeeded {
        r.period_label.clone()
    } else {
        periods::period_label(freq, r.taxable_year as i32, r.period, true)
    };
    format!("{} {}", r.form_code, period)
}

/// A draft's date is its last save; any other return's is its last change (dateLine).
pub fn date_line(r: &ReturnRow) -> String {
    format!(
        "{} {}",
        if r.status == "Draft" {
            "Saved"
        } else {
            "Updated"
        },
        short_date(&r.updated_at)
    )
}

/// What a screen reader says of a card (cardLabel). CUI cards carry no accessibility label;
/// kept for model parity and the unit tests.
#[allow(dead_code)]
pub fn card_label(r: &ReturnRow) -> String {
    format!(
        "{} {}, {}, {}. Open",
        r.form_code,
        r.period_label,
        who_line(r),
        r.status
    )
}

/// The search and the four filters; an empty set means every value was unticked (keeps nothing),
/// `None` means the filter is not narrowed (FilterValues).
#[derive(Debug, Clone, Default)]
pub struct FilterValues {
    pub query: String,
    pub tps: Option<HashSet<String>>,
    pub forms: Option<HashSet<String>>,
    pub periods: Option<HashSet<String>>,
    pub statuses: Option<HashSet<String>>,
    /// "YYYY-MM": only returns last changed in that month (Home's "Filed this month").
    pub updated_month: Option<String>,
}

/// The returns the search and every filter keep (filterRows).
pub fn filter_rows(
    rows: &[ReturnRow],
    index: &[FormIndexEntry],
    f: &FilterValues,
) -> Vec<ReturnRow> {
    let words: Vec<String> = f
        .query
        .trim()
        .to_lowercase()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    rows.iter()
        .filter(|r| {
            if let Some(tps) = &f.tps
                && !tps.contains(&r.tin_root)
            {
                return false;
            }
            if let Some(forms) = &f.forms
                && !forms.contains(&r.form_code)
            {
                return false;
            }
            if let Some(statuses) = &f.statuses
                && !statuses.contains(&r.status)
            {
                return false;
            }
            if let Some(periods) = &f.periods
                && !periods.contains(&period_key(index, r))
            {
                return false;
            }
            if let Some(m) = &f.updated_month
                && local_month(&r.updated_at) != *m
            {
                return false;
            }
            if words.is_empty() {
                return true;
            }
            let hay = format!(
                "{} {} {} {} {} {}",
                r.form_code, r.period_label, r.taxpayer_name, r.branch_name, r.status, r.tin
            )
            .to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .cloned()
        .collect()
}

/// What a Home tile asks Filings to show (FilingsPreset).
#[derive(Debug, Clone, Default)]
pub struct FilingsPreset {
    pub statuses: Option<Vec<String>>,
    pub updated_month: Option<String>,
}

fn is_record(v: &Value) -> bool {
    v.is_object()
}

/// The preset in a navigation's params, or None (presetOf): the same two keys inside a `preset`
/// object are read too.
pub fn preset_of(params: &Value) -> Option<FilingsPreset> {
    let src = if is_record(params.get("preset").unwrap_or(&Value::Null)) {
        params.get("preset").unwrap()
    } else {
        params
    };
    let statuses = src.get("statuses").and_then(Value::as_array).map(|a| {
        a.iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    });
    let updated_month = src
        .get("updatedMonth")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if statuses.is_none() && updated_month.is_none() {
        None
    } else {
        Some(FilingsPreset {
            statuses,
            updated_month,
        })
    }
}

/// The filters a preset sets — it replaces every filter (presetFilters): the real statuses it
/// names (all five means "not narrowed"), and a well-formed month.
pub fn preset_filters(preset: &FilingsPreset) -> (Option<HashSet<String>>, Option<String>) {
    // `on.size === STATUSES.length ? null : on`: naming all five means "not narrowed"; an empty
    // selection (no valid statuses) stays an empty set, which keeps nothing.
    let statuses = preset.statuses.as_ref().map(|list| {
        let on: HashSet<String> = list
            .iter()
            .filter(|s| STATUSES.contains(&s.as_str()))
            .cloned()
            .collect();
        if on.len() == STATUSES.len() {
            None
        } else {
            Some(on)
        }
    });
    let statuses = statuses.flatten().map(Some).unwrap_or(None);
    let statuses = match &preset.statuses {
        Some(_) => statuses,
        None => None,
    };
    let month_ok = preset.updated_month.as_deref().is_some_and(|m| {
        m.len() == 7
            && m[..4].chars().all(|c| c.is_ascii_digit())
            && m.as_bytes()[4] == b'-'
            && m[5..].chars().all(|c| c.is_ascii_digit())
    });
    (
        statuses,
        if month_ok {
            preset.updated_month.clone()
        } else {
            None
        },
    )
}

/// home/rows.ts `statusKind`: the chip kind of a return's status.
pub fn status_kind(status: &str) -> &'static str {
    match status {
        "Draft" => "draft",
        "Queued" => "queued",
        "Submitted" | "Confirmed" | "Paid" => "filed",
        _ => "not_started",
    }
}

/// home/rows.ts `whoLine`: the taxpayer, plus the branch when it has several.
pub fn home_who_line(branch_count: i64, taxpayer_name: &str, branch_name: &str) -> String {
    if branch_count > 1 {
        format!("{taxpayer_name} · {branch_name}")
    } else {
        taxpayer_name.to_owned()
    }
}

/// home/rows.ts `deadlineKey`: an as-needed row is keyed by its saved return as well.
pub fn deadline_key(d: &DeadlineRow) -> String {
    // `due-${tin}-${form_code}-${taxable_year}-${period}` plus `-${draft_id}` for as-needed.
    let mut key = format!(
        "due-{}-{}-{}-{}",
        d.tin, d.form_code, d.taxable_year, d.period
    );
    if d.frequency == "As needed" {
        // `${d.draft_id}`: a null draft_id stringifies to "null" in JS.
        key.push_str(&format!(
            "-{}",
            d.draft_id
                .map(|i| i.to_string())
                .unwrap_or_else(|| "null".to_owned())
        ));
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn index() -> Vec<FormIndexEntry> {
        [
            ("1601C", Frequency::Monthly),
            ("2551Q", Frequency::Quarterly),
            ("1604C", Frequency::Annual),
            ("2000", Frequency::AsNeeded),
        ]
        .iter()
        .map(|(c, f)| FormIndexEntry {
            c: c.to_string(),
            freq: *f,
        })
        .collect()
    }

    fn row(over: impl FnOnce(&mut ReturnRow)) -> ReturnRow {
        let mut r = ReturnRow {
            id: 1,
            tin: "00012345300000".into(),
            tin_root: "000123453".into(),
            branch_code: "00000".into(),
            branch_name: "Head office".into(),
            taxpayer_name: "Juana Santos".into(),
            branch_count: 2,
            form_code: "2551Q".into(),
            taxable_year: 2026,
            period: 3,
            period_label: "Q3 2026".into(),
            status: "Draft".into(),
            updated_at: "2026-11-14 09:30:00".into(),
            return_key: Some(3),
        };
        over(&mut r);
        r
    }

    fn rows() -> Vec<ReturnRow> {
        vec![
            row(|_| {}),
            row(|r| {
                r.id = 2;
                r.form_code = "1601C".into();
                r.period = 9;
                r.period_label = "September 2026".into();
                r.status = "Queued".into();
            }),
            row(|r| {
                r.id = 3;
                r.form_code = "1604C".into();
                r.period = 0;
                r.period_label = "TY 2025".into();
                r.taxable_year = 2025;
                r.status = "Paid".into();
                r.updated_at = "2026-10-31 22:30:00".into();
            }),
            row(|r| {
                r.id = 4;
                r.tin = "00012345300001".into();
                r.branch_code = "00001".into();
                r.branch_name = "Cebu branch".into();
                r.period = 2;
                r.period_label = "Q2 2026".into();
                r.updated_at = "2026-11-15 01:00:00".into();
            }),
            row(|r| {
                r.id = 5;
                r.tin = "00011122900000".into();
                r.tin_root = "000111229".into();
                r.taxpayer_name = "Sample Trading Corp".into();
                r.form_code = "1601C".into();
                r.period = 8;
                r.period_label = "August 2026".into();
                r.status = "Confirmed".into();
            }),
        ]
    }

    fn ids(rs: &[ReturnRow]) -> Vec<i64> {
        rs.iter().map(|r| r.id).collect()
    }

    fn all() -> FilterValues {
        FilterValues::default()
    }

    #[test]
    fn return_is_named_as_recent_names_it() {
        let idx = index();
        let r = rows();
        assert_eq!(return_title(&idx, &r[0]), "2551Q Q3 2026");
        assert_eq!(return_title(&idx, &r[1]), "1601C Sep 2026");
        assert_eq!(return_title(&idx, &r[2]), "1604C TY 2025");
        let as_needed = row(|r| {
            r.form_code = "2000".into();
            r.period = 1;
            r.period_label = "2026 #1".into();
        });
        assert_eq!(return_title(&idx, &as_needed), "2000 2026 #1");
    }

    #[test]
    fn period_key_groups_months_into_quarters() {
        let idx = index();
        let r = rows();
        assert_eq!(period_key(&idx, &r[0]), "Q3 2026");
        assert_eq!(period_key(&idx, &r[1]), "Q3 2026");
        let m1 = row(|r| {
            r.form_code = "1601C".into();
            r.period = 1;
        });
        assert_eq!(period_key(&idx, &m1), "Q1 2026");
        let m12 = row(|r| {
            r.form_code = "1601C".into();
            r.period = 12;
        });
        assert_eq!(period_key(&idx, &m12), "Q4 2026");
        assert_eq!(period_key(&idx, &r[2]), "TY 2025");
        let an = row(|r| {
            r.form_code = "2000".into();
            r.period = 1;
        });
        assert_eq!(period_key(&idx, &an), "TY 2026");
        let unknown = row(|r| {
            r.form_code = "9999".into();
            r.period = 2;
        });
        assert_eq!(period_key(&idx, &unknown), "TY 2026");
    }

    #[test]
    fn periods_sort_newest_first_ty_after_q4() {
        let mut keys = vec!["Q1 2026", "TY 2025", "Q4 2025", "TY 2026", "Q3 2026"];
        keys.sort_by(|a, b| period_rank(b).cmp(&period_rank(a)));
        assert_eq!(
            keys,
            vec!["TY 2026", "Q3 2026", "Q1 2026", "TY 2025", "Q4 2025"]
        );
        assert_eq!(period_rank("nonsense"), 0);
    }

    #[test]
    fn branch_named_only_when_not_head_office() {
        let r = rows();
        assert_eq!(named_branch(&r[0]), "");
        let ho = row(|r| {
            r.branch_code = "0".into();
        });
        assert_eq!(named_branch(&ho), "");
        assert_eq!(named_branch(&r[3]), "Cebu branch");
        assert_eq!(who_line(&r[0]), "Juana Santos");
        assert_eq!(who_line(&r[3]), "Juana Santos · Cebu branch");
        assert_eq!(
            card_label(&r[3]),
            "2551Q Q2 2026, Juana Santos · Cebu branch, Draft. Open"
        );
    }

    #[test]
    fn draft_date_is_a_save_other_is_a_change() {
        let r = rows();
        assert!(date_line(&r[0]).starts_with("Saved "));
        assert!(date_line(&r[1]).starts_with("Updated "));
        let mut c = r[0].clone();
        c.status = "Confirmed".into();
        assert!(date_line(&c).starts_with("Updated "));
    }

    #[test]
    fn local_month_is_local_time() {
        // The test env's local zone is UTC on CI; either way both forms parse the same way.
        assert_eq!(
            local_month("2026-11-15 12:00:00"),
            local_month("2026-11-15T12:00:00")
        );
        assert_eq!(local_month("2026-11-15"), "2026-11");
        assert_eq!(local_month(""), "");
        assert_eq!(month_label("2026-11"), "Nov 2026");
        assert_eq!(month_label("2026-01"), "Jan 2026");
        let expected = {
            let d = chrono::DateTime::parse_from_rfc3339("2026-11-15T12:00:00Z").unwrap();
            let l = d.with_timezone(&chrono::Local);
            format!("{:04}-{:02}", l.year(), l.month())
        };
        assert_eq!(local_month("2026-11-15 12:00:00"), expected);
    }

    #[test]
    fn newest_first_without_reordering_input() {
        let r = rows();
        assert_eq!(ids(&newest_first(&r)), vec![4, 1, 2, 5, 3]);
        assert_eq!(ids(&r), vec![1, 2, 3, 4, 5]);
    }

    fn set(items: &[&str]) -> Option<HashSet<String>> {
        Some(items.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn filters_keep_what_they_name() {
        let idx = index();
        let r = rows();
        assert_eq!(ids(&filter_rows(&r, &idx, &all())), vec![1, 2, 3, 4, 5]);
        let mut f = all();
        f.tps = set(&["000111229"]);
        assert_eq!(ids(&filter_rows(&r, &idx, &f)), vec![5]);
        let mut f = all();
        f.forms = set(&["1601C"]);
        assert_eq!(ids(&filter_rows(&r, &idx, &f)), vec![2, 5]);
        let mut f = all();
        f.periods = set(&["Q3 2026"]);
        assert_eq!(ids(&filter_rows(&r, &idx, &f)), vec![1, 2, 5]);
        let mut f = all();
        f.periods = set(&["TY 2025"]);
        assert_eq!(ids(&filter_rows(&r, &idx, &f)), vec![3]);
        let mut f = all();
        f.statuses = set(&["Paid", "Confirmed"]);
        assert_eq!(ids(&filter_rows(&r, &idx, &f)), vec![3, 5]);
        // An empty selection keeps nothing ("No statuses").
        let mut f = all();
        f.statuses = Some(HashSet::new());
        assert!(filter_rows(&r, &idx, &f).is_empty());
        let mut f = all();
        f.forms = set(&["1601C"]);
        f.tps = set(&["000123453"]);
        assert_eq!(ids(&filter_rows(&r, &idx, &f)), vec![2]);
    }

    #[test]
    fn search_wants_every_word() {
        let idx = index();
        let r = rows();
        let q = |query: &str| {
            let mut f = all();
            f.query = query.to_owned();
            ids(&filter_rows(&r, &idx, &f))
        };
        assert_eq!(q("cebu"), vec![4]);
        assert_eq!(q("CEBU branch"), vec![4]);
        assert_eq!(q("2551q q3"), vec![1]);
        assert_eq!(q("september"), vec![2]);
        assert_eq!(q("sample trading"), vec![5]);
        assert_eq!(q("paid"), vec![3]);
        assert_eq!(q("00012345300001"), vec![4]);
        assert_eq!(q("  juana   queued "), vec![2]);
        assert!(q("juana nothing").is_empty());
        assert_eq!(q("   "), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn month_filter_keeps_that_local_month() {
        let idx = index();
        let r = rows();
        let mut f = all();
        f.updated_month = Some(local_month("2026-10-31 22:30:00"));
        assert_eq!(ids(&filter_rows(&r, &idx, &f)), vec![3]);
    }

    #[test]
    fn preset_of_and_filters() {
        let p = preset_of(&json!({"statuses": ["Draft", "Queued"]})).unwrap();
        assert_eq!(
            p.statuses.as_deref(),
            Some(&["Draft".to_owned(), "Queued".to_owned()][..])
        );
        let p2 = preset_of(&json!({"preset": {"updatedMonth": "2026-11"}})).unwrap();
        assert_eq!(p2.updated_month.as_deref(), Some("2026-11"));
        assert!(preset_of(&json!({})).is_none());
        let (statuses, month) = preset_filters(&FilingsPreset {
            statuses: Some(vec!["Draft".into(), "Bogus".into()]),
            updated_month: Some("2026-11".into()),
        });
        assert_eq!(statuses.unwrap(), HashSet::from(["Draft".to_owned()]));
        assert_eq!(month.as_deref(), Some("2026-11"));
        // All five real statuses means "not narrowed".
        let (statuses, _) = preset_filters(&FilingsPreset {
            statuses: Some(STATUSES.iter().map(|s| s.to_string()).collect()),
            updated_month: None,
        });
        assert!(statuses.is_none());
        // A malformed month is dropped.
        let (_, month) = preset_filters(&FilingsPreset {
            statuses: None,
            updated_month: Some("November 2026".into()),
        });
        assert!(month.is_none());
    }

    #[test]
    fn status_kind_and_home_helpers() {
        assert_eq!(status_kind("Draft"), "draft");
        assert_eq!(status_kind("Queued"), "queued");
        assert_eq!(status_kind("Submitted"), "filed");
        assert_eq!(status_kind("Confirmed"), "filed");
        assert_eq!(status_kind("Paid"), "filed");
        assert_eq!(status_kind(""), "not_started");
        assert_eq!(home_who_line(2, "Juana", "Cebu"), "Juana · Cebu");
        assert_eq!(home_who_line(1, "Juana", "Head"), "Juana");
        let d = DeadlineRow {
            tin: "00012345300000".into(),
            form_code: "2000".into(),
            taxable_year: 2026,
            period: 1,
            frequency: "As needed".into(),
            draft_id: Some(7),
            ..Default::default()
        };
        assert_eq!(deadline_key(&d), "due-00012345300000-2000-2026-1-7");
        let d2 = DeadlineRow {
            frequency: "Monthly".into(),
            draft_id: Some(7),
            ..d.clone()
        };
        assert_eq!(deadline_key(&d2), "due-00012345300000-2000-2026-1");
    }
}
