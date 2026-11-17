//! The taxpayer screens' pure rules (`nsh/screens/taxpayer/logic.ts`): what a typed value is worth,
//! what each sentence says. No UI and no host calls; the clock comes in as a parameter where a rule
//! needs today.
//!
//! `logic.test.ts` is mirrored in the tests below.

use std::collections::HashMap;

use crate::lib::facts::{self, FactName};
use crate::lib::format;
use crate::types::{FormTemplateSummary, SetupApplied, Taxpayer};

/// The taxpayer types the Taxpayer type dropdown offers (TAXPAYER_TYPES).
pub const TAXPAYER_TYPES: [&str; 6] = [
    "Individual",
    "Corporation",
    "Partnership",
    "Cooperative",
    "Estate",
    "Trust",
];

/// The Forms tab's frequency filter (FREQS).
pub const FREQS: [&str; 5] = ["All", "Monthly", "Quarterly", "Annual", "As needed"];

/// The address and contact fields: field, label, full width (ADDRESS_FIELDS).
pub const ADDRESS_FIELDS: [(&str, &str, bool); 4] = [
    ("registered_address", "Registered address", true),
    ("zip_code", "ZIP code", false),
    ("phone", "Phone", false),
    ("email", "Email", true),
];

/// Every branch field the New branch page sets, in order, with its label (BRANCH_FIELDS).
pub const BRANCH_FIELDS: [(&str, &str); 5] = [
    ("rdo_code", "RDO"),
    ("registered_address", "Registered address"),
    ("zip_code", "ZIP code"),
    ("phone", "Phone"),
    ("email", "Email"),
];

/// The profile's seven sections, in order (SECTIONS).
pub const SECTIONS: [&str; 7] = [
    "taxpayer", "branch", "taxtypes", "facts", "address", "inbox", "agent",
];

/// The sections with an Edit (EditableSection).
pub type EditableSection = &'static str;

pub const EMAIL_RE: &str = r"^[^\s@]+@[^\s@]+\.[^\s@]+$";
pub const TIN_ERROR: &str = "TIN is 9 digits, like 123-456-789.";
pub const BRANCH_CODE_ERROR: &str = "Branch code is 5 digits, like 00001.";

/// Today as "YYYY-MM-DD" in local time (todayText).
pub fn today_text(now: &chrono::NaiveDate) -> String {
    now.format("%Y-%m-%d").to_string()
}

/// "YYYY-MM-DD" that names a real day (realDate).
pub fn real_date(text: &str) -> bool {
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok()
}

/// What is wrong with a typed date that must not be after today, or None. Empty is fine
/// (pastDateError).
pub fn past_date_error(text: &str, now: &chrono::NaiveDate) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    if !real_date(text) {
        return Some("Type the date as YYYY-MM-DD, like 1990-05-17.".to_owned());
    }
    if text > today_text(now).as_str() {
        return Some("That date is in the future.".to_owned());
    }
    None
}

/// ATC codes as typed ("wc160, WI010"): the codes, upper case, or None when one is not a code
/// (parseAtcCodes).
pub fn parse_atc_codes(text: &str) -> Option<Vec<String>> {
    let codes: Vec<String> = text
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .map(|c| c.trim().to_uppercase())
        .filter(|c| !c.is_empty())
        .collect();
    let ok = codes.iter().all(|c| {
        let b = c.as_bytes();
        b.len() >= 5
            && b[0].is_ascii_alphabetic()
            && b[1].is_ascii_alphabetic()
            && b[2..5].iter().all(|x| x.is_ascii_digit())
    });
    if ok { Some(codes) } else { None }
}

/// "is" or "are" for a count of names (isAre).
pub fn is_are(n: usize) -> &'static str {
    if n == 1 { "is" } else { "are" }
}

/// "1 form", "3 forms" (plural).
pub fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

fn email_ok(v: &str) -> bool {
    // /^[^\s@]+@[^\s@]+\.[^\s@]+$/
    let Some((local, rest)) = v.split_once('@') else {
        return false;
    };
    if local.is_empty() || local.chars().any(char::is_whitespace) || local.contains('@') {
        return false;
    }
    let Some((host, tld)) = rest.rsplit_once('.') else {
        return false;
    };
    !host.is_empty()
        && !tld.is_empty()
        && !rest.chars().any(char::is_whitespace)
        && !rest.contains('@')
}

/// What is wrong with an RDO, ZIP code or email as typed, or None. Empty is fine: none is required.
/// `known_rdo` says whether an RDO is one of the reference list's (factError).
pub fn fact_error(
    field: &str,
    raw: &str,
    known_rdo: Option<&dyn Fn(&str) -> bool>,
) -> Option<String> {
    let v = raw.trim();
    if v.is_empty() {
        return None;
    }
    if field == "rdo_code"
        && let Some(known) = known_rdo
        && !known(v)
    {
        return Some("Pick an RDO from the list. Type its code or district to search.".to_owned());
    }
    if field == "zip_code" && !(v.len() == 4 && v.bytes().all(|b| b.is_ascii_digit())) {
        return Some(
            "ZIP code is 4 digits, like 1226. Type a code or a place to search.".to_owned(),
        );
    }
    if field == "email" && !email_ok(v) {
        return Some("Type an email like name@example.com.".to_owned());
    }
    None
}

/// The errors to show now: a field's once it was left (touched), every one after the first submit,
/// and the host's (shownErrors).
pub fn shown_errors(
    problems: &HashMap<String, String>,
    touched: &[String],
    submitted: bool,
    host: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut out = host.clone();
    for (k, v) in problems {
        if submitted || touched.iter().any(|t| t == k) {
            out.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    out
}

// ---------------------------------------------------------------- new taxpayer

/// What the New taxpayer page holds while it is filled in (NewTaxpayerDraft).
#[derive(Debug, Clone, Default)]
pub struct NewTaxpayerDraft {
    pub tin: String,
    pub name: String,
    pub taxpayer_type: String,
    pub registered_address: String,
    pub zip_code: String,
    pub rdo_code: String,
    pub phone: String,
    pub email: String,
    /// "none", "setup:<id>" or "copy" (with `from`, a branch TIN).
    pub start: String,
    pub from: Option<String>,
    pub touched: Vec<String>,
    pub submitted: bool,
    /// Errors the host answered on Create, by field.
    pub host_errors: HashMap<String, String>,
    /// A host error that names no field.
    pub error: Option<String>,
}

/// Every problem with the new taxpayer as typed, by field (newTaxpayerProblems).
pub fn new_taxpayer_problems(
    n: &NewTaxpayerDraft,
    taxpayers: &[Taxpayer],
    known_rdo: &dyn Fn(&str) -> bool,
) -> HashMap<String, String> {
    let mut e = HashMap::new();
    if n.tin.len() != 9 {
        e.insert("tin".to_owned(), TIN_ERROR.to_owned());
    } else if let Some(taken) = taxpayers.iter().find(|t| t.tin_root == n.tin) {
        e.insert(
            "tin".to_owned(),
            format!("TIN {} is already {}.", format_root(&n.tin), taken.name),
        );
    }
    if n.name.trim().is_empty() {
        e.insert("name".to_owned(), "Type the registered name.".to_owned());
    }
    if n.start == "copy" && n.from.is_none() {
        e.insert(
            "from".to_owned(),
            "Choose the taxpayer and branch to copy.".to_owned(),
        );
    }
    for f in ["zip_code", "rdo_code", "email"] {
        let raw = match f {
            "zip_code" => &n.zip_code,
            "rdo_code" => &n.rdo_code,
            _ => &n.email,
        };
        if let Some(error) = fact_error(f, raw, Some(known_rdo)) {
            e.insert(f.to_owned(), error);
        }
    }
    e
}

/// The errors the New taxpayer page shows now; a TIN already in use is said as soon as all 9 digits
/// are in (newTaxpayerErrors).
pub fn new_taxpayer_errors(
    n: &NewTaxpayerDraft,
    problems: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut touched = n.touched.clone();
    if n.tin.len() == 9 && !touched.iter().any(|t| t == "tin") {
        touched.push("tin".to_owned());
    }
    shown_errors(problems, &touched, n.submitted, &n.host_errors)
}

/// "000-123-453" (formatRoot).
pub fn format_root(digits: &str) -> String {
    let b = digits.as_bytes();
    if b.len() == 9 {
        format!("{}-{}-{}", &digits[0..3], &digits[3..6], &digits[6..9])
    } else {
        digits.to_owned()
    }
}

/// The tax year a new taxpayer starts in: this year, or the copied branch's latest when it lacks
/// this year (startYear).
pub fn start_year(start: &str, from_years: Option<&[i64]>, now_year: i64) -> i64 {
    if start != "copy" {
        return now_year;
    }
    let Some(years) = from_years else {
        return now_year;
    };
    if years.contains(&now_year) || years.is_empty() {
        now_year
    } else {
        *years.last().unwrap()
    }
}

/// The line under "Start from" (startHint).
pub fn start_hint(chosen: Option<&(bool, Vec<String>)>, start: &str, year: i64) -> String {
    if let Some((adds, forms)) = chosen {
        return format!(
            "{} {year} with {} and the facts they use.",
            if *adds { "Adds" } else { "Starts" },
            forms.join(", ")
        );
    }
    if start == "copy" {
        return format!(
            "Copies the forms, facts and templates of {year}. Never the name, TIN or address."
        );
    }
    "Add a tax year after, and choose its forms.".to_owned()
}

// ---------------------------------------------------------------- setups and copies

/// The question after a setup or copy left set things as they were (ReplaceAsk).
#[derive(Debug, Clone)]
pub struct ReplaceAsk {
    pub kind: String,
    pub tin: String,
    pub year: i64,
    pub id: Option<String>,
    pub from: Option<String>,
    pub label: String,
    pub kept: Vec<FactName>,
    pub templates: Vec<String>,
    pub forms: Vec<String>,
}

/// What a setup or copy did: the toast, and the question to keep (null when nothing set was left)
/// (afterApply).
pub fn after_apply(r: &SetupApplied, ask: &ReplaceAskSeed) -> (String, Option<ReplaceAsk>) {
    let kept: Vec<FactName> = r
        .kept
        .iter()
        .filter_map(|f| FactName::from_wire(f))
        .filter(|f| !(*f == FactName::IncomeTaxElection && r.election_locked))
        .collect();
    let templates = r.templates_kept.clone();
    let forms = r.forms_kept.clone();
    let n = r.forms.len();
    let mut text = format!("{}: {} for {}.", ask.label, plural(n, "form"), ask.year);
    if r.election_locked && r.kept.iter().any(|f| f == "income_tax_election") {
        text += &format!(
            " The 8% or graduated choice for {} stays: a return that carries it was filed.",
            ask.year
        );
    }
    let ask_out = if !kept.is_empty() || !templates.is_empty() || !forms.is_empty() {
        Some(ReplaceAsk {
            kind: ask.kind.clone(),
            tin: ask.tin.clone(),
            year: ask.year,
            id: ask.id.clone(),
            from: ask.from.clone(),
            label: ask.label.clone(),
            kept,
            templates,
            forms,
        })
    } else {
        None
    };
    (text, ask_out)
}

/// The fields of a ReplaceAsk the caller already knows (afterApply's `ask` argument).
#[derive(Debug, Clone)]
pub struct ReplaceAskSeed {
    pub kind: String,
    pub tin: String,
    pub year: i64,
    pub id: Option<String>,
    pub from: Option<String>,
    pub label: String,
}

/// The banner's sentence: what is already set, what stays on the list, then the question
/// (replaceSentence).
pub fn replace_sentence(a: &ReplaceAsk) -> String {
    let mut names: Vec<String> = a
        .kept
        .iter()
        .map(|f| facts::fact_label(*f).to_owned())
        .collect();
    if !a.templates.is_empty() {
        let refs: Vec<&str> = a.templates.iter().map(String::as_str).collect();
        names.push(format!(
            "the {} template{}",
            facts::join_names(&refs),
            if a.templates.len() == 1 { "" } else { "s" }
        ));
    }
    let mut lines: Vec<String> = Vec::new();
    if !names.is_empty() {
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        lines.push(format!(
            "{} {} already set. {} has something else.",
            facts::join_names(&refs),
            is_are(names.len()),
            a.label
        ));
    }
    if !a.forms.is_empty() {
        let refs: Vec<&str> = a.forms.iter().map(String::as_str).collect();
        lines.push(format!(
            "{} {} on your forms list and not in {}; {} unless you replace.",
            facts::join_names(&refs),
            is_are(a.forms.len()),
            a.label,
            if a.forms.len() == 1 {
                "it stays"
            } else {
                "they stay"
            }
        ));
    }
    format!("{} Replace what is set?", lines.join(" "))
}

/// Two or more annual income tax returns of one kind on a year's list: the warning
/// (annualConflictSentence).
pub fn annual_conflict_sentence(group: &[String], year: i64) -> String {
    let both = if group.len() == 2 { "both" } else { "all" };
    let other = if group.len() == 2 { "other" } else { "others" };
    let refs: Vec<&str> = group.iter().map(String::as_str).collect();
    format!(
        "{} are {} annual income tax returns. A taxpayer files only one of them for {}. Keep the one on your Certificate of Registration and remove the {}.",
        facts::join_names(&refs),
        both,
        year,
        other
    )
}

// ---------------------------------------------------------------- tax years and forms

/// The years that may be added, newest first (allowedYears).
pub fn allowed_years(bounds: (i64, i64), have: &[i64]) -> Vec<i64> {
    let mut out = Vec::new();
    for y in (bounds.0..=bounds.1).rev() {
        if !have.contains(&y) {
            out.push(y);
        }
    }
    out
}

/// The year the Add year band offers first: the one after the latest, else this year, else the
/// newest allowed (defaultAddYear).
pub fn default_add_year(allowed: &[i64], have: &[i64], now_year: i64) -> Option<i64> {
    if allowed.is_empty() {
        return None;
    }
    if let Some(&last) = have.last() {
        return if allowed.contains(&(last + 1)) {
            Some(last + 1)
        } else {
            Some(allowed[0])
        };
    }
    if allowed.contains(&now_year) {
        Some(now_year)
    } else {
        Some(allowed[0])
    }
}

/// A deadline as the Forms tab reads it (DeadlineLike).
#[derive(Debug, Clone)]
pub struct DeadlineLike {
    pub status: String,
    pub due_date: String,
    pub period_label: String,
}

/// The next open period of a form in the year and its due date, from its deadlines (schedule).
pub fn schedule<F, G>(
    rows: &[DeadlineLike],
    days_until: F,
    short_date: G,
) -> (Option<String>, String, bool)
where
    F: Fn(&str) -> i64,
    G: Fn(&str) -> String,
{
    if rows.is_empty() {
        return (None, String::new(), true);
    }
    let open: Vec<&DeadlineLike> = rows
        .iter()
        .filter(|d| d.status == "Not started" || d.status == "Draft")
        .collect();
    let upcoming: Option<&&DeadlineLike> = open
        .iter()
        .find(|d| days_until(&d.due_date) >= 0)
        .or_else(|| open.last());
    match upcoming {
        Some(d) => (Some(d.period_label.clone()), short_date(&d.due_date), false),
        None => (None, String::new(), false),
    }
}

/// The line under a form's name on the Forms tab (formSub).
pub fn form_sub(on: bool, editing: bool, next: Option<&str>, none: bool, year: i64) -> String {
    if editing && !on {
        return "Not filed this year".to_owned();
    }
    if let Some(n) = next {
        return format!("Next: {n}");
    }
    if none {
        return format!("Filed in {year}");
    }
    "Every period filed".to_owned()
}

/// The form library's search text of a form: code, code without dashes, title, and "vat" for a
/// value-added tax form (searchText).
pub fn search_text(c: &str, t: &str) -> String {
    let vat = if t.to_lowercase().contains("value-added") {
        " vat"
    } else {
        ""
    };
    format!("{} {} {}{}", c, c.replace('-', ""), t, vat).to_lowercase()
}

/// The toast after Save forms (formsSavedText).
pub fn forms_saved_text(year: i64, before: &[String], after: &[String]) -> String {
    let added = after.iter().filter(|c| !before.contains(c)).count();
    let removed = before.iter().filter(|c| !after.contains(c)).count();
    format!("{year} forms updated: {added} added, {removed} removed.")
}

/// The toast after Add year (yearAddedText).
pub fn year_added_text(year: i64, copied_from: Option<i64>, forms: &[String]) -> String {
    let n = forms.len();
    match copied_from {
        Some(from) => format!("{year} added with {} from {from}.", plural(n, "form")),
        None => format!("{year} added. Choose the forms it files."),
    }
}

// ---------------------------------------------------------------- templates

/// "3 answers · 2 stops": what a template fixes and how many "Stop here" switches are on
/// (templateCounts).
pub fn template_counts(tpl: &FormTemplateSummary) -> String {
    let answers = tpl.answers_kept.max(0) as usize;
    let stops = tpl.marked.max(0) as usize;
    format!(
        "{} · {}",
        if answers == 1 {
            "1 answer".to_owned()
        } else {
            format!("{answers} answers")
        },
        if stops == 1 {
            "1 stop".to_owned()
        } else {
            format!("{stops} stops")
        }
    )
}

/// A failed "template from the newest return", in words (templateErrorText).
pub fn template_error_text(code: &str, text: &str) -> String {
    let t = text.to_lowercase();
    let first = t.contains("no return")
        || t.contains("not found")
        || t.contains("no ") && t.contains("draft")
        || t.contains("first");
    if first {
        format!("Open and save a {code} return first. The template starts from its answers.")
    } else {
        text.to_owned()
    }
}

// ---------------------------------------------------------------- section metas

/// What the Confirmation inbox section's summary says (inboxMeta). The host's InboxState fields the
/// rule reads, as plain values.
pub struct InboxMetaInput {
    pub tracking_enabled: bool,
    pub inbox_email: String,
    pub oauth_connected: bool,
    pub has_app_password: bool,
    pub oauth_inbox_email: Option<String>,
}

/// The Confirmation inbox section's summary (inboxMeta).
pub fn inbox_meta(ib: Option<&InboxMetaInput>) -> String {
    let Some(ib) = ib else {
        return "Not connected".to_owned();
    };
    if ib.tracking_enabled {
        return format!("Tracking on · {}", ib.inbox_email);
    }
    if ib.oauth_connected || ib.has_app_password {
        let email = if !ib.inbox_email.is_empty() {
            ib.inbox_email.clone()
        } else {
            ib.oauth_inbox_email.clone().unwrap_or_default()
        };
        return format!("Connected · {email}");
    }
    "Not connected".to_owned()
}

/// The Agent filing section's summary (agentMeta).
pub fn agent_meta(forms: &[String]) -> String {
    if forms.is_empty() {
        "Assistant must ask before filing".to_owned()
    } else {
        forms.join(", ")
    }
}

/// The Address and contact section's summary (addressMeta).
pub fn address_meta(is_head: bool, inherited_count: usize) -> String {
    if is_head {
        "Used on every return".to_owned()
    } else if inherited_count > 0 {
        format!("{inherited_count} of 4 from head office")
    } else {
        "All set for this branch".to_owned()
    }
}

/// The Facts section's summary (factsMeta).
pub fn facts_meta_summary(has_year: bool, shown: usize, not_set: usize) -> String {
    if !has_year {
        return "Add a tax year first".to_owned();
    }
    if shown == 0 {
        return "No form on your list reads one".to_owned();
    }
    if not_set > 0 {
        format!("{not_set} not set")
    } else {
        "All set".to_owned()
    }
}

/// The line under the facts: how many facts no form on the list reads (hiddenFactsText).
pub fn hidden_facts_text(hidden: usize) -> Option<String> {
    if hidden == 0 {
        return None;
    }
    if hidden == 1 {
        Some("1 fact is not shown: no form on your list uses it.".to_owned())
    } else {
        Some(format!(
            "{hidden} facts are not shown: no form on your list uses them."
        ))
    }
}

/// The result of a "check now", in words (checkText).
pub fn check_text(ok: bool, still_pending: bool, error: Option<&str>) -> String {
    if !ok {
        return error.unwrap_or("The check failed.").to_owned();
    }
    if still_pending {
        "Checked. A submitted return is still waiting for its confirmation.".to_owned()
    } else {
        "Checked. Everything submitted is confirmed.".to_owned()
    }
}

/// The tracking line at the foot of the inbox section (trackingLine).
pub fn tracking_line(tracking_enabled: bool, still_pending: bool) -> String {
    if !tracking_enabled {
        return "○ Verify your connection to activate email tracking.".to_owned();
    }
    if still_pending {
        "● Automated BIR receipt tracking is active — a submitted return is still waiting for its confirmation.".to_owned()
    } else {
        "● Automated BIR receipt tracking is active.".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn now() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 15).unwrap()
    }

    #[test]
    fn t_dates() {
        assert!(real_date("2024-02-29"));
        assert!(!real_date("2025-02-29"));
        assert_eq!(past_date_error("", &now()), None);
        assert_eq!(
            past_date_error("1990-13-01", &now()),
            Some("Type the date as YYYY-MM-DD, like 1990-05-17.".to_owned())
        );
        assert_eq!(
            past_date_error("2026-11-16", &now()),
            Some("That date is in the future.".to_owned())
        );
        assert_eq!(past_date_error("2026-11-15", &now()), None);
    }

    #[test]
    fn t_atc_and_field_errors() {
        assert_eq!(
            parse_atc_codes("wc160, WI010;pt010"),
            Some(vec![
                "WC160".to_owned(),
                "WI010".to_owned(),
                "PT010".to_owned()
            ])
        );
        assert_eq!(parse_atc_codes("WC16"), None);
        assert_eq!(parse_atc_codes(""), Some(vec![]));
        assert!(
            fact_error("zip_code", "12a6", None)
                .unwrap()
                .starts_with("ZIP code is 4 digits")
        );
        assert_eq!(fact_error("zip_code", "1226", None), None);
        assert_eq!(
            fact_error("email", "a@b", None),
            Some("Type an email like name@example.com.".to_owned())
        );
        assert!(
            fact_error("rdo_code", "999", Some(&|c| c == "050"))
                .unwrap()
                .starts_with("Pick an RDO")
        );
        assert_eq!(fact_error("rdo_code", "", Some(&|_| false)), None);
    }

    #[test]
    fn t_shown_errors() {
        let p = HashMap::from([
            ("tin".to_owned(), "a".to_owned()),
            ("name".to_owned(), "b".to_owned()),
        ]);
        assert!(shown_errors(&p, &[], false, &HashMap::new()).is_empty());
        let touched = vec!["name".to_owned()];
        let out = shown_errors(&p, &touched, false, &HashMap::new());
        assert_eq!(out.len(), 1);
        assert_eq!(out["name"], "b");
        assert_eq!(shown_errors(&p, &[], true, &HashMap::new()).len(), 2);
        let host = HashMap::from([("tin".to_owned(), "host".to_owned())]);
        let out = shown_errors(&HashMap::new(), &[], false, &host);
        assert_eq!(out["tin"], "host");
    }

    #[test]
    fn t_new_taxpayer() {
        let taxpayers = vec![Taxpayer {
            tin_root: "000123453".to_owned(),
            name: "Juana Santos".to_owned(),
            ..Default::default()
        }];
        let n = NewTaxpayerDraft {
            tin: "000123453".to_owned(),
            ..Default::default()
        };
        let problems = new_taxpayer_problems(&n, &taxpayers, &|_| true);
        assert_eq!(problems["tin"], "TIN 000-123-453 is already Juana Santos.");
        assert_eq!(problems["name"], "Type the registered name.");
        let errs = new_taxpayer_errors(&n, &problems);
        assert_eq!(errs.len(), 1);
        assert!(errs.contains_key("tin"));
        let mut copy = n.clone();
        copy.start = "copy".to_owned();
        let problems = new_taxpayer_problems(&copy, &taxpayers, &|_| true);
        assert_eq!(problems["from"], "Choose the taxpayer and branch to copy.");
        assert_eq!(start_year("copy", Some(&[2023, 2024]), 2026), 2024);
        assert_eq!(start_year("copy", Some(&[2025, 2026]), 2026), 2026);
        assert_eq!(start_year("none", None, 2026), 2026);
        assert_eq!(
            start_hint(Some(&(true, vec!["1601C".to_owned()])), "setup:x", 2026),
            "Adds 2026 with 1601C and the facts they use."
        );
        assert_eq!(
            start_hint(None, "none", 2026),
            "Add a tax year after, and choose its forms."
        );
    }

    fn mk_setup(extra: serde_json::Value) -> SetupApplied {
        let mut base = serde_json::json!({
            "forms": ["1601C"], "set": [], "kept": [], "election_locked": false,
            "templates_copied": [], "templates_kept": [], "forms_kept": [],
        });
        if let (Some(b), Some(e)) = (base.as_object_mut(), extra.as_object()) {
            for (k, v) in e {
                b.insert(k.clone(), v.clone());
            }
        }
        serde_json::from_value(base).unwrap()
    }

    fn seed(kind: &str) -> ReplaceAskSeed {
        ReplaceAskSeed {
            kind: kind.to_owned(),
            tin: "t".to_owned(),
            year: 2026,
            id: Some("x".to_owned()),
            from: None,
            label: "Employer".to_owned(),
        }
    }

    #[test]
    fn t_setups() {
        let (toast, ask) = after_apply(&mk_setup(serde_json::json!({})), &seed("setup"));
        assert_eq!(toast, "Employer: 1 form for 2026.");
        assert!(ask.is_none());
        let kept = mk_setup(serde_json::json!({
            "kept": ["eopt_tier", "income_tax_election"],
            "election_locked": true,
            "forms_kept": ["2551Q"],
        }));
        let (toast, ask) = after_apply(&kept, &seed("setup"));
        assert_eq!(
            toast,
            "Employer: 1 form for 2026. The 8% or graduated choice for 2026 stays: a return that carries it was filed."
        );
        let ask = ask.unwrap();
        assert_eq!(ask.kept.len(), 1);
        assert_eq!(ask.kept[0], FactName::EoptTier);
        assert_eq!(
            replace_sentence(&ask),
            "EOPT tier is already set. Employer has something else. 2551Q is on your forms list and not in Employer; it stays unless you replace. Replace what is set?"
        );
    }

    #[test]
    fn t_years_schedules_counts() {
        assert_eq!(allowed_years((2024, 2027), &[2025, 2026]), vec![2027, 2024]);
        assert_eq!(
            default_add_year(&[2027, 2024], &[2025, 2026], 2026),
            Some(2027)
        );
        assert_eq!(default_add_year(&[2027, 2026, 2025], &[], 2026), Some(2026));
        assert_eq!(default_add_year(&[], &[2026], 2026), None);

        let days = |iso: &str| if iso < "2026-11-15" { -1 } else { 1 };
        let rows = vec![
            DeadlineLike {
                status: "Filed".to_owned(),
                due_date: "2026-04-25".to_owned(),
                period_label: "Q1 2026".to_owned(),
            },
            DeadlineLike {
                status: "Draft".to_owned(),
                due_date: "2026-10-25".to_owned(),
                period_label: "Q3 2026".to_owned(),
            },
            DeadlineLike {
                status: "Not started".to_owned(),
                due_date: "2027-01-25".to_owned(),
                period_label: "Q4 2026".to_owned(),
            },
        ];
        let (next, due, none) = schedule(&rows, days, |d| d.to_owned());
        assert_eq!(next.as_deref(), Some("Q4 2026"));
        assert_eq!(due, "2027-01-25");
        assert!(!none);
        let (next, due, none) = schedule(&[], days, |d| d.to_owned());
        assert_eq!(next, None);
        assert_eq!(due, "");
        assert!(none);

        assert_eq!(
            form_sub(false, true, None, false, 2026),
            "Not filed this year"
        );
        assert_eq!(form_sub(true, false, None, true, 2026), "Filed in 2026");
        assert_eq!(
            form_sub(true, false, None, false, 2026),
            "Every period filed"
        );
        assert_eq!(
            forms_saved_text(
                2026,
                &["1601C".to_owned(), "2551Q".to_owned()],
                &["1601C".to_owned(), "0619E".to_owned()]
            ),
            "2026 forms updated: 1 added, 1 removed."
        );
        assert_eq!(
            year_added_text(2027, Some(2026), &["1601C".to_owned()]),
            "2027 added with 1 form from 2026."
        );
        assert_eq!(
            search_text("2550-Q", "Quarterly Value-Added Tax Return"),
            "2550-q 2550q quarterly value-added tax return vat"
        );
        let tpl = FormTemplateSummary {
            answers_kept: 1,
            marked: 2,
            ..Default::default()
        };
        assert_eq!(template_counts(&tpl), "1 answer · 2 stops");
        assert_eq!(
            template_error_text("1601C", "No return found"),
            "Open and save a 1601C return first. The template starts from its answers."
        );
    }

    #[test]
    fn t_section_summaries() {
        assert_eq!(inbox_meta(None), "Not connected");
        let ib = InboxMetaInput {
            tracking_enabled: true,
            inbox_email: "a@b.c".to_owned(),
            oauth_connected: false,
            has_app_password: false,
            oauth_inbox_email: None,
        };
        assert_eq!(inbox_meta(Some(&ib)), "Tracking on · a@b.c");
        assert_eq!(agent_meta(&[]), "Assistant must ask before filing");
        assert_eq!(
            agent_meta(&["1601C".to_owned(), "2551Q".to_owned()]),
            "1601C, 2551Q"
        );
        assert_eq!(address_meta(true, 0), "Used on every return");
        assert_eq!(address_meta(false, 2), "2 of 4 from head office");
        assert_eq!(facts_meta_summary(false, 0, 0), "Add a tax year first");
        assert_eq!(facts_meta_summary(true, 3, 1), "1 not set");
        assert_eq!(
            hidden_facts_text(1),
            Some("1 fact is not shown: no form on your list uses it.".to_owned())
        );
        assert_eq!(
            tracking_line(false, false),
            "○ Verify your connection to activate email tracking."
        );
        assert_eq!(check_text(false, false, Some("boom")), "boom");
        assert_eq!(
            check_text(true, true, None),
            "Checked. A submitted return is still waiting for its confirmation."
        );
        assert_eq!(
            check_text(true, false, None),
            "Checked. Everything submitted is confirmed."
        );
        // silence unused-import warning for format if any
        let _ = format::display_code;
    }
}
