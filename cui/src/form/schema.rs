//! The form page's schema and its pure helpers (port of `nsh/screens/form/contract.ts` types and
//! `nsh/screens/form/schema.ts`). The schema comes from `screen_form_schema` (the rules corpus);
//! everything here is pure.

use serde::{Deserialize, Deserializer};

use crate::lib::dates::short_iso_date;
use crate::types::ScreenFormExisting;

/// A flag the schema writes as `1` or `true`: accepts either.
pub fn flex_flag<'de, D: Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.map(|x| match x {
        serde_json::Value::Bool(b) => b,
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        serde_json::Value::String(s) => s == "1" || s.eq_ignore_ascii_case("true"),
        _ => false,
    }))
}

fn default_text() -> String {
    "text".to_owned()
}

/// One box of an item. Flags are 1 when set (the schema writes 1, not true).
#[derive(Debug, Clone, Deserialize)]
pub struct SchemaField {
    /// The rules-corpus key the value is stored under.
    pub k: String,
    /// The box's own label.
    #[serde(default)]
    pub l: String,
    #[serde(default = "default_text")]
    pub t: String,
    /// Computed by the form: never typed.
    #[serde(default, deserialize_with = "flex_flag")]
    pub c: Option<bool>,
    /// Required.
    #[serde(default, deserialize_with = "flex_flag")]
    pub r: Option<bool>,
    /// Max length of the typed text.
    #[serde(default)]
    pub m: Option<f64>,
    /// Listed choices of a "sel" ("03 - March": code, then label).
    #[serde(default)]
    pub o: Option<Vec<String>>,
    /// Filled from the tax profile by the extractor's guess.
    #[serde(default, deserialize_with = "flex_flag")]
    pub a: Option<bool>,
    /// A peso box over a whole-number field: no centavos.
    #[serde(default, deserialize_with = "flex_flag")]
    pub w: Option<bool>,
    /// A radio's one-answer group (host).
    #[serde(default)]
    pub grp: Option<String>,
    /// How many boxes the radio's group has (host).
    #[serde(default)]
    pub rg: Option<f64>,
    /// A pooled address line: drawn once by the pool's first line.
    #[serde(default)]
    pub pool: Option<PoolKeys>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PoolKeys {
    pub keys: Vec<String>,
}

/// A column of a schedule table. "hidden" columns are not drawn.
#[derive(Debug, Clone, Deserialize)]
pub struct SchemaColumn {
    #[serde(default)]
    pub l: String,
    #[serde(default = "default_text")]
    pub t: String,
    #[serde(default, deserialize_with = "flex_flag")]
    pub c: Option<bool>,
    #[serde(default, deserialize_with = "flex_flag")]
    pub w: Option<bool>,
    #[serde(default)]
    pub o: Option<Vec<String>>,
}

/// A hint offering another item's amount for this item's box ("Use it").
#[derive(Debug, Clone, Deserialize)]
pub struct FromHint {
    pub k: String,
    pub to: String,
    pub l: String,
}

/// A schedule's preprinted row captions.
#[derive(Debug, Clone, Deserialize)]
pub struct RowLabels {
    pub caption: String,
    pub rows: Vec<String>,
}

/// One numbered item (row) of a part: its boxes (f), or a schedule table (rows, cols, keys[row][col]).
#[derive(Debug, Clone, Deserialize)]
pub struct SchemaItem {
    /// The printed item number ("13", "20A", "").
    #[serde(default)]
    pub no: String,
    #[serde(default)]
    pub l: String,
    /// The page of the paper form it prints on.
    #[serde(default)]
    pub p: f64,
    #[serde(default)]
    pub f: Vec<SchemaField>,
    #[serde(default)]
    pub rows: Option<f64>,
    #[serde(default)]
    pub cols: Vec<SchemaColumn>,
    #[serde(default)]
    pub keys: Vec<Vec<String>>,
    /// A printed item with no box: what the paper prints there.
    #[serde(default)]
    pub note: Option<String>,
    /// Month, day and year boxes shown as one date box.
    #[serde(default, deserialize_with = "flex_flag")]
    pub one_date: Option<bool>,
    /// A hint offering another item's amount.
    #[serde(default)]
    pub from: Option<FromHint>,
    /// A schedule's preprinted row captions.
    #[serde(default)]
    pub row_labels: Option<RowLabels>,
    #[serde(default, deserialize_with = "flex_flag")]
    pub numbered: Option<bool>,
    #[serde(default)]
    pub start: Option<f64>,
    #[serde(default, deserialize_with = "flex_flag")]
    pub calendar: Option<bool>,
    #[serde(default, deserialize_with = "flex_flag")]
    pub typed_tin: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchemaPart {
    /// "Part I · Background Information", or "Page 1" for the untitled first part.
    #[serde(default)]
    pub n: String,
    #[serde(default)]
    pub items: Vec<SchemaItem>,
}

/// A form as the page draws it.
#[derive(Debug, Clone, Deserialize)]
pub struct FormSchema {
    /// Form code ("2551Q").
    #[serde(default)]
    pub c: String,
    #[serde(default)]
    pub id: Option<String>,
    /// Title.
    #[serde(default)]
    pub t: String,
    #[serde(default)]
    pub rev: String,
    #[serde(default)]
    pub freq: String,
    #[serde(default)]
    pub pages: f64,
    /// Number of fields.
    #[serde(default)]
    pub n: f64,
    /// Layout quality 0..1 (under 0.8 the page says "Draft layout").
    #[serde(default)]
    pub q: f64,
    #[serde(default)]
    pub parts: Vec<SchemaPart>,
}

// ---------------------------------------------------------------- helpers

/// A listed dropdown option as (value, label). The schema writes many as "CODE - Label" strings
/// ("03 - March"): the host takes the code, so that is the value and the whole string the label.
/// Template text the extractor left ("\" + rdo.description + \"") is dropped.
pub fn option_of(o: &serde_json::Value) -> Option<(String, String)> {
    if o.is_object() {
        let v = o.get("value")?.as_str()?.to_owned();
        let l = o
            .get("label")
            .and_then(|x| x.as_str())
            .unwrap_or(&v)
            .to_owned();
        return Some((v, l));
    }
    let text = o.as_str()?;
    if text.starts_with('"') {
        return None;
    }
    // /^\s*(\S+) - (.+)$/
    let m = text.split_once(" - ");
    if let Some((v, _)) = m {
        if !v.trim().is_empty() && !v.contains(' ') {
            return Some((v.to_owned(), text.to_owned()));
        }
    }
    Some((text.to_owned(), text.to_owned()))
}

/// Items whose three to five text boxes are the TIN (3-3-3-5).
pub fn is_tin_item(it: &SchemaItem) -> bool {
    let fs = &it.f;
    let hay_f = |f: &SchemaField| format!("{} {}", f.l, f.k);
    let named = {
        let l = it.l.clone();
        l.contains("TIN") || l.contains("Identification")
    } || fs.iter().any(|x| {
        let h = hay_f(x);
        h.contains("TIN1")
            || h.contains("TIN2")
            || h.contains("TIN3")
            || h.contains("TIN4")
            || h.contains("TIN5")
            || h.contains("txtTIN")
            || (h.contains("TIN") && !h.contains("TINs"))
    });
    named && (3..=5).contains(&fs.len()) && fs.iter().all(|x| x.t == "text" || x.t == "int")
}

/// Drops hidden fields, tables whose every column is hidden, and items left with no field
/// (unless they carry a note).
pub fn without_hidden(schema: &FormSchema) -> FormSchema {
    let mut out = schema.clone();
    out.parts = schema
        .parts
        .iter()
        .map(|p| {
            let mut np = p.clone();
            np.items = p
                .items
                .iter()
                .filter(|it| it.rows.is_none() || it.cols.iter().any(|c| c.t != "hidden"))
                .map(|it| {
                    let mut nit = it.clone();
                    nit.f.retain(|f| f.t != "hidden");
                    nit
                })
                .filter(|it| it.rows.is_some() || !it.f.is_empty() || it.note.is_some())
                .collect();
            np
        })
        .filter(|p| !p.items.is_empty())
        .collect();
    out
}

/// The first part's name: "Return details" for an untitled "Page 1".
pub fn part_name(p: &SchemaPart, i: usize) -> String {
    if i == 0 && p.n.starts_with("Page ") && p.n[5..].chars().all(|c| c.is_ascii_digit()) {
        return "Return details".to_owned();
    }
    p.n.clone()
}

/// The navigator's one-line name: filing instructions in parentheses go, "Schedule" becomes "Sch.".
pub fn compact_part_name(name: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '(' => {
                depth += 1;
                // look ahead: drop the paren group only if it starts with attach/if graduated/except
                let rest: String = chars.clone().collect();
                let inner = rest.split(')').next().unwrap_or("");
                let inner_l = inner.to_lowercase();
                if inner_l.starts_with("attach")
                    || inner_l.starts_with("if graduated")
                    || inner_l.starts_with("except")
                {
                    // skip until ')'
                    let mut d = 1;
                    for c2 in chars.by_ref() {
                        if c2 == '(' {
                            d += 1;
                        }
                        if c2 == ')' {
                            d -= 1;
                            if d == 0 {
                                break;
                            }
                        }
                    }
                    continue;
                }
                depth -= 1;
                out.push(c);
            }
            ')' => {
                if depth > 0 {
                    depth -= 1;
                }
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    let short = out.trim().replacen("Schedule", "Sch.", 1).trim().to_owned();
    if short.is_empty() {
        name.to_owned()
    } else {
        short
    }
}

/// A part's place in the pill and its list: tag "Part II", short "Computation of Tax".
pub fn section_names(p: &SchemaPart, i: usize) -> (String, String) {
    let name = part_name(p, i);
    let strip = |text: &str| -> String {
        // remove " (attach|if graduated|except ...)" groups
        let mut out = String::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '(' {
                let rest: String = chars.clone().collect();
                let inner = rest.split(')').next().unwrap_or("").to_lowercase();
                if inner.starts_with("attach")
                    || inner.starts_with("if graduated")
                    || inner.starts_with("except")
                {
                    let mut d = 1;
                    for c2 in chars.by_ref() {
                        if c2 == '(' {
                            d += 1;
                        }
                        if c2 == ')' {
                            d -= 1;
                            if d == 0 {
                                break;
                            }
                        }
                    }
                    continue;
                }
                out.push(c);
            } else {
                out.push(c);
            }
        }
        let t = out.trim().to_owned();
        if t.is_empty() { text.to_owned() } else { t }
    };
    if let Some(at) = name.find(" · ") {
        return (name[..at].to_owned(), strip(&name[at + 3..]));
    }
    let tag = if name == p.n {
        format!("Section {}", i + 1)
    } else {
        p.n.clone()
    };
    (tag, strip(&name))
}

/// The tag on a box another answer turns off, cut if it ever runs long.
pub fn needs_text(tag: &str) -> String {
    let text = tag.trim();
    if text.chars().count() > 90 {
        let cut: String = text.chars().take(89).collect();
        format!("{}…", cut.trim_end())
    } else {
        text.to_owned()
    }
}

/// The month, day and year boxes of an item (a printed date split in boxes).
#[derive(Debug, Clone)]
pub struct DateParts {
    pub month: SchemaField,
    pub day: Option<SchemaField>,
    pub year: SchemaField,
}

fn date_find<'a>(
    fs: &'a [SchemaField],
    re: &regex::Regex,
    not: Option<&regex::Regex>,
) -> Option<&'a SchemaField> {
    fs.iter().find(|f| {
        if f.t == "radio" || f.t == "chk" {
            return false;
        }
        let tail = f.k.rsplit(':').next().unwrap_or(&f.k);
        let hay = format!("{} {}", f.l, tail);
        if !re.is_match(&hay) {
            return false;
        }
        if let Some(n) = not {
            let hay2 = format!("{} {}", f.l, f.k);
            if n.is_match(&hay2) {
                return false;
            }
        }
        true
    })
}

/// The month, day and year boxes of an item: at least a month and a year.
pub fn date_parts(fs: &[SchemaField]) -> Option<DateParts> {
    use std::sync::LazyLock;
    static MONTH: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)month|\bmm\b").unwrap());
    static MONTH_NOT: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)months|no\.? of").unwrap());
    static YEAR: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)year|yyyy|\byr\b").unwrap());
    static YEAR_NOT: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)month|years ended|no\.? of").unwrap());
    static DAY: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)\bday\b|\bdd\b|[a-z]day\b|day\d*$").unwrap());
    static DAY_NOT: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)days|month|year").unwrap());
    let month = date_find(fs, &MONTH, Some(&MONTH_NOT))?;
    let year = date_find(fs, &YEAR, Some(&YEAR_NOT))?;
    if std::ptr::eq(month, year) {
        return None;
    }
    let day = date_find(fs, &DAY, Some(&DAY_NOT));
    let day = day
        .filter(|d| !std::ptr::eq(*d, month) && !std::ptr::eq(*d, year))
        .cloned();
    Some(DateParts {
        month: month.clone(),
        day,
        year: year.clone(),
    })
}

/// An item the host marks one_date: its month, day and year boxes are one date box, keyed
/// "<month key>#date".
#[derive(Debug, Clone)]
pub struct OneDate {
    pub month: SchemaField,
    pub day: Option<SchemaField>,
    pub year: SchemaField,
    pub one_key: String,
}

pub fn one_date(it: &SchemaItem) -> Option<OneDate> {
    if it.one_date != Some(true) {
        return None;
    }
    let d = date_parts(&it.f)?;
    Some(OneDate {
        one_key: format!("{}#date", d.month.k),
        month: d.month,
        day: d.day,
        year: d.year,
    })
}

/// BIR's own words for the income-tax ATCs the forms list as bare codes.
fn atc_title(code: &str) -> Option<&'static str> {
    Some(match code {
        "II011" => "Compensation Income",
        "II012" => "Business Income – Graduated IT Rates",
        "II013" => "Mixed Income – Graduated IT Rates",
        "II014" => "Income from Profession – Graduated IT Rates",
        "II015" => "Business Income – 8% IT Rate",
        "II016" => "Mixed Income – 8% IT Rate",
        "II017" => "Income from Profession – 8% IT Rate",
        _ => return None,
    })
}

/// A radio's label as the page shows it ("Fiscal start month: 1" shows "1"; "ATC II012" its title).
pub fn choice_label(f: &SchemaField) -> String {
    // /^.*?:\s*/ strips everything through the last ": "
    let text = match f.l.rfind(": ") {
        Some(i) => &f.l[i + 2..],
        None => f.l.as_str(),
    };
    // /^A?TC\s+([A-Z]{2}\d{3})$/
    let code = {
        let t = text.trim();
        let t = t
            .strip_prefix("ATC ")
            .or_else(|| t.strip_prefix("TC "))
            .unwrap_or(t);
        if t.len() == 5
            && t.chars().take(2).all(|c| c.is_ascii_uppercase())
            && t[2..].chars().all(|c| c.is_ascii_digit())
        {
            Some(t.to_owned())
        } else {
            None
        }
    };
    if let Some(code) = code {
        if let Some(title) = atc_title(&code) {
            return format!("{code}  {title}");
        }
        return code;
    }
    text.to_owned()
}

/// The width a box gets (old controlWidth).
pub fn control_width(t: &str, m: Option<f64>, available: f64) -> f64 {
    match t {
        "money" => available.min(230.0),
        "pct" => 88.0,
        "date" => 140.0,
        "int" => {
            if m.is_some_and(|m| m <= 4.0) {
                88.0
            } else {
                140.0
            }
        }
        "sel" => available.min(260.0),
        "text" => {
            if m.is_some_and(|m| m <= 4.0) {
                88.0
            } else {
                available.min(320.0)
            }
        }
        _ => available.min(320.0),
    }
}

/// The strip on an amended return's filed version, opened from its history.
pub fn filed_original_strip(view: &ScreenFormExisting) -> String {
    let filed_on = view.submitted_at.as_deref();
    let when = filed_on
        .map(|d| format!(" on {}", short_iso_date(d)))
        .unwrap_or_default();
    format!(
        "This is the version filed{when}, kept in this return's history; the current return is its amendment. Nothing here can be changed."
    )
}

/// Where an amended return's filed version goes.
pub const FILED_KEPT: &str = "The filed version is kept in this return's history.";

/// A filed return whose tax profile changed after it.
pub const PROFILE_CHANGED_NOTE: &str =
    "The tax profile changed after this return was filed. This return keeps what was filed.";

pub fn profile_updates_note(updates: &[String]) -> String {
    format!("Updated from your tax profile: {}", updates.join("; "))
}

pub fn profile_differs_note(differs: &[String]) -> String {
    format!("Differs from your profile: {}", differs.join("; "))
}

/// The note at the top of a Form library sample.
pub const SAMPLE_NOTE: &str = "A sample from the Form library. Set its period and it becomes your return for that period, saved as you type; if that period already has a return, you choose what to do with it. Save as return asks for the period. New return (the + button or Cmd/Ctrl+N) starts a blank one.";

/// A sample's period parts in the period dialog, labelled.
pub fn period_part_label(key: &str) -> &'static str {
    match key {
        "year" => "Year (YYYY)",
        "month" => "Month (MM)",
        "year_end_month" => "Year-end month (MM)",
        "day" => "Day (DD)",
        "date" => "Date (MM/DD/YYYY)",
        "quarter" => "Quarter",
        _ => "Value",
    }
}

/// What the existing-return offer says and offers for a stored return's status.
#[derive(Debug, Clone)]
pub struct OfferChoice {
    pub id: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub variant: &'static str, // "primary" | "secondary" | "danger"
}

pub struct ExistingOffer {
    pub title: String,
    pub subtitle: String,
    pub choices: Vec<OfferChoice>,
    pub fast_lane: bool,
}

pub fn existing_offer_for(existing: &ScreenFormExisting) -> ExistingOffer {
    let label = &existing.label;
    let has = |action: &str| existing.actions.iter().any(|a| a == action);
    let keep = OfferChoice {
        id: "keep",
        label: "Keep this form",
        hint: "Stay in the sample; the period clears.",
        variant: "secondary",
    };
    if existing.status == "Draft" {
        return ExistingOffer {
            title: format!("You already have a {label} draft"),
            subtitle: existing
                .updated_at
                .as_deref()
                .map(|d| format!("edited {}.", short_iso_date(d)))
                .unwrap_or_default(),
            choices: vec![
                OfferChoice {
                    id: "load",
                    label: "Load saved draft",
                    hint: "Continue where you left off.",
                    variant: "primary",
                },
                keep,
            ],
            fast_lane: true,
        };
    }
    if existing.status == "Queued" {
        return ExistingOffer {
            title: format!("{label} is queued to send to BIR"),
            subtitle: String::new(),
            choices: vec![
                OfferChoice {
                    id: "view",
                    label: "View it",
                    hint: "Read-only while it waits to send.",
                    variant: "primary",
                },
                keep,
            ],
            fast_lane: false,
        };
    }
    let filed = if existing.status == "Submitted" {
        "submitted"
    } else {
        "filed"
    };
    let mut choices = vec![OfferChoice {
        id: "view",
        label: "View read-only",
        hint: Box::leak(format!("See exactly what was {filed}.").into_boxed_str()),
        variant: "primary",
    }];
    if has("amend") {
        choices.push(OfferChoice {
            id: "amend",
            label: "Amend this return",
            hint: Box::leak(format!("New amended return. {FILED_KEPT}").into_boxed_str()),
            variant: "danger",
        });
    }
    choices.push(keep);
    ExistingOffer {
        title: format!("{label} was {filed}"),
        subtitle: String::new(),
        choices,
        fast_lane: false,
    }
}

/// A host refusal in plain words: bir-core's "Other error: " prefix is not for the filer.
pub fn plain_refusal(text: &str) -> String {
    text.strip_prefix("Other error: ")
        .map(str::to_owned)
        .unwrap_or_else(|| text.to_owned())
}

/// Every key an item binds (a schedule's every cell, a pooled line's pool).
pub fn keys_of(it: &SchemaItem) -> Vec<String> {
    if it.rows.is_some() {
        return it.keys.iter().flat_map(|row| row.iter().cloned()).collect();
    }
    it.f.iter()
        .flat_map(|f| match &f.pool {
            Some(pool) => pool.keys.clone(),
            None => vec![f.k.clone()],
        })
        .collect()
}

/// An empty schedule cell, as the host reads a row (bir-core's row is_blank): blank text, a zero
/// amount, an unticked box.
pub fn empty_cell(value: &str, t: &str) -> bool {
    let v = value.trim();
    if v.is_empty() {
        return true;
    }
    if t == "money" || t == "pct" || t == "int" {
        let cleaned: String = v
            .chars()
            .filter(|c| !matches!(c, ',' | '₱' | '%' | ' '))
            .collect();
        return regex::Regex::new(r"^[-+]?0*\.?0*$")
            .unwrap()
            .is_match(&cleaned);
    }
    if t == "chk" || t == "radio" {
        return !(v == "true" || v == "1" || v == "Y");
    }
    false
}

#[cfg(test)]
mod audit_tests {
    use super::*;

    /// Every schema entry in app/forms/forms-schema.json must parse as FormSchema —
    /// this is the audit for "This form has no on-screen layout."
    #[test]
    fn all_43_schemas_parse() {
        let text = std::fs::read_to_string("../app/forms/forms-schema.json")
            .expect("app/forms/forms-schema.json");
        let forms: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
        assert_eq!(forms.len(), 43);
        let mut bad = Vec::new();
        for f in &forms {
            let code = f["c"].as_str().unwrap_or("?").to_owned();
            if let Err(e) = serde_json::from_value::<FormSchema>(f.clone()) {
                bad.push(format!("{code}: {e}"));
            }
        }
        assert!(bad.is_empty(), "unparseable schemas:\n{}", bad.join("\n"));
    }
}
