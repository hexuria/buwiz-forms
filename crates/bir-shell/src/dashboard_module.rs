//! Home, Calendar and Filings feeds for the redesigned shell: deadlines for
//! the forms each branch files, and the saved and filed returns across every
//! taxpayer. Read-only; nothing here writes.
//!
//! Deadlines come from `calendar_rules::DeadlineResolver::deadlines_for_forms`
//! over each branch's yearly forms (the per-year Forms Set), with the user's
//! deadline overrides applied, exactly as the painted dashboard resolves them.
//!
//! As-needed returns (decision D8c, 2026-10-18) have no period deadline. A
//! saved one that holds its trigger date (a removal, a withholding, a
//! transaction) is a dated row too, due per its form's rule
//! (`calendar_rules::EVENT_DUE_RULES`); the as-needed forms a branch files
//! in a year, with their saved returns that hold no date yet, are listed by
//! `as_needed_forms` for Home's "As needed" list.

use bir_core::calendar_rules::{
    BusinessDayCalendar, DeadlineOverride, DeadlinePeriod, DeadlineResolver, event_due_rule,
};
use bir_core::db::Database;
use bir_core::forms::{FilingStatus, FormDraftSummary};
use chrono::NaiveDate;
use gpui_shell::{HostError, HostModule, HostResult};
use serde_json::{Value, json};

use crate::bir_module::{serialize, with_database};

/// TypeScript appended to the `bir` module's declarations.
pub const DECLARATIONS: &str = r#"
export interface DeadlineRow {
    tin: string;
    tin_root: string;
    branch_code: string;
    branch_name: string;
    taxpayer_name: string;
    branch_count: number;
    form_code: string;
    form_name: string;
    frequency: "Monthly" | "Quarterly" | "Annual" | "As needed";
    taxable_year: number;
    /** Month 1-12, quarter 1-4, 0 for an annual return, or an as-needed return's stored period. */
    period: number;
    period_label: string;
    due_date: string;
    /** "Not started" | "Draft" | "Queued" | "Submitted" | "Confirmed" | "Paid" */
    status: string;
    draft_id: number | null;
    /** The saved return's stored key (`screen_form_open`'s `key`), null when none is saved. */
    return_key: number | null;
}
export function deadlines_between(from: string, to: string): DeadlineRow[];
export interface AsNeededReturn {
    draft_id: number;
    period: number;
    return_key: number | null;
    status: string;
    updated_at: string;
}
export interface AsNeededForm {
    tin: string;
    tin_root: string;
    branch_code: string;
    branch_name: string;
    taxpayer_name: string;
    branch_count: number;
    form_code: string;
    form_name: string;
    taxable_year: number;
    /** What dates a return ("Removal"), null when the form has no trigger-date rule. */
    trigger: string | null;
    /** When it is due, as the form's guide says. */
    due_rule: string;
    /** Saved returns of the form that hold no trigger date yet. */
    undated: AsNeededReturn[];
}
export function as_needed_forms(year: number): AsNeededForm[];
export interface ReturnRow {
    id: number;
    tin: string;
    tin_root: string;
    branch_code: string;
    branch_name: string;
    taxpayer_name: string;
    branch_count: number;
    form_code: string;
    taxable_year: number;
    period: number;
    period_label: string;
    status: string;
    updated_at: string;
    /** The return's stored key (`screen_form_open`'s `key`): opens exactly this return. */
    return_key: number | null;
}
export function list_returns(which: "filed" | "saved" | "all"): ReturnRow[];
"#;

pub fn register(module: HostModule) -> HostModule {
    module
        .function("deadlines_between", deadlines_between)
        .function("as_needed_forms", as_needed_forms)
        .function("list_returns", list_returns)
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn period_label(period: &DeadlinePeriod) -> (u8, String) {
    match period {
        DeadlinePeriod::Monthly {
            taxable_year,
            month,
        } => (
            *month,
            format!(
                "{} {taxable_year}",
                MONTHS[(*month as usize).clamp(1, 12) - 1]
            ),
        ),
        DeadlinePeriod::Quarterly {
            taxable_year,
            quarter,
        } => (*quarter, format!("Q{quarter} {taxable_year}")),
        DeadlinePeriod::Annual { taxable_year } => (0, format!("TY {taxable_year}")),
        DeadlinePeriod::EventBased => (0, "As needed".to_string()),
    }
}

fn summary_period(summary: &FormDraftSummary) -> u8 {
    summary.month.or(summary.quarter).unwrap_or(0)
}

fn summary_label(summary: &FormDraftSummary) -> String {
    match (summary.month, summary.quarter) {
        (Some(month), _) if (1..=12).contains(&month) => {
            format!("{} {}", MONTHS[month as usize - 1], summary.taxable_year)
        }
        (_, Some(quarter)) if (1..=4).contains(&quarter) => {
            format!("Q{quarter} {}", summary.taxable_year)
        }
        _ => format!("TY {}", summary.taxable_year),
    }
}

fn status_name(status: &FilingStatus) -> String {
    format!("{status:?}")
}

fn parse_date(text: &str) -> Result<NaiveDate, HostError> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map_err(|_| HostError::new(format!("dates are YYYY-MM-DD, got {text:?}")))
}

fn error(error: impl std::fmt::Display) -> HostError {
    HostError::new(error.to_string())
}

/// One branch of one taxpayer, flattened for the feeds.
struct BranchRef {
    tin: String,
    tin_root: String,
    branch_code: String,
    branch_name: String,
    taxpayer_name: String,
    branch_count: usize,
}

fn branches(db: &Database) -> Result<Vec<BranchRef>, HostError> {
    let mut out = Vec::new();
    for taxpayer in db.list_taxpayers().map_err(error)? {
        let count = taxpayer.branches.len();
        for branch in &taxpayer.branches {
            out.push(BranchRef {
                tin: branch.tin.clone(),
                tin_root: taxpayer.taxpayer.tin_root.clone(),
                branch_code: branch.branch_code.clone(),
                branch_name: branch.name.clone(),
                taxpayer_name: taxpayer.taxpayer.name.clone(),
                branch_count: count,
            });
        }
    }
    Ok(out)
}

fn branch_fields(branch: &BranchRef) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();
    map.insert("tin".into(), json!(branch.tin));
    map.insert("tin_root".into(), json!(branch.tin_root));
    map.insert("branch_code".into(), json!(branch.branch_code));
    map.insert("branch_name".into(), json!(branch.branch_name));
    map.insert("taxpayer_name".into(), json!(branch.taxpayer_name));
    map.insert("branch_count".into(), json!(branch.branch_count));
    map
}

/// Every dated deadline in `[from, to]` for the forms each branch files in
/// the taxable years that can fall in that window, with the return's status.
fn deadline_rows(db: &Database, from: NaiveDate, to: NaiveDate) -> Result<Vec<Value>, HostError> {
    use chrono::Datelike;
    let overrides = db.get_deadline_overrides();
    let working = bir_core::official_holidays::business_day_calendar();
    // An annual return for year Y is due in Y+1, so the window's first year
    // minus one is the earliest taxable year that can have a deadline in it.
    let first_year = from.year() - 1;
    let last_year = to.year();
    let mut rows = Vec::new();
    for branch in branches(db)? {
        for year in first_year..=last_year {
            let Ok(year16) = u16::try_from(year) else {
                continue;
            };
            let codes = db
                .active_form_codes_for_year(&branch.tin, year16)
                .map_err(error)?;
            let drafts = db
                .list_draft_summaries(&branch.tin, year16)
                .map_err(error)?;
            rows.extend(as_needed_rows(
                db, &branch, &drafts, &overrides, &working, from, to,
            )?);
            if codes.is_empty() {
                continue;
            }
            for deadline in DeadlineResolver::deadlines_for_forms(&codes, year, &overrides) {
                let Some(due) = deadline.final_deadline_date() else {
                    continue;
                };
                if due < from || due > to {
                    continue;
                }
                let (period, label) = period_label(&deadline.period);
                let draft = drafts.iter().find(|summary| {
                    summary.form_code == deadline.form_code && summary_period(summary) == period
                });
                let frequency = match deadline.period {
                    DeadlinePeriod::Monthly { .. } => "Monthly",
                    DeadlinePeriod::Quarterly { .. } => "Quarterly",
                    _ => "Annual",
                };
                let mut row = branch_fields(&branch);
                row.insert("form_code".into(), json!(deadline.form_code));
                row.insert("form_name".into(), json!(deadline.form_name));
                row.insert("frequency".into(), json!(frequency));
                row.insert("taxable_year".into(), json!(year));
                row.insert("period".into(), json!(period));
                row.insert("period_label".into(), json!(label));
                row.insert("due_date".into(), json!(due.format("%Y-%m-%d").to_string()));
                row.insert(
                    "status".into(),
                    json!(
                        draft.map_or_else(|| "Not started".to_string(), |d| status_name(&d.status))
                    ),
                );
                row.insert("draft_id".into(), json!(draft.map(|d| d.id)));
                let key = match draft {
                    Some(draft) => db.draft_period_column(draft.id).map_err(error)?,
                    None => None,
                };
                row.insert("return_key".into(), json!(key));
                rows.push(Value::Object(row));
            }
        }
    }
    rows.sort_by(|a, b| {
        a["due_date"]
            .as_str()
            .cmp(&b["due_date"].as_str())
            .then(
                a["taxpayer_name"]
                    .as_str()
                    .cmp(&b["taxpayer_name"].as_str()),
            )
            .then(a["form_code"].as_str().cmp(&b["form_code"].as_str()))
    });
    Ok(rows)
}

/// The dated rows of a branch's saved as-needed returns that hold their
/// trigger date, due in `[from, to]`. A saved return is its own evidence the
/// form is filed, so these do not depend on the year's forms.
fn as_needed_rows(
    db: &Database,
    branch: &BranchRef,
    drafts: &[FormDraftSummary],
    overrides: &[DeadlineOverride],
    working: &BusinessDayCalendar,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<Value>, HostError> {
    let mut rows = Vec::new();
    for summary in drafts {
        if !DeadlineResolver::is_as_needed(&summary.form_code) {
            continue;
        }
        let Some(trigger) = db.draft_event_trigger(summary.id).map_err(error)? else {
            continue;
        };
        let Some(deadline) =
            DeadlineResolver::event_deadline(&trigger.rule, trigger.date, overrides, working)
        else {
            continue;
        };
        let Some(due) = deadline.final_deadline_date() else {
            continue;
        };
        if due < from || due > to {
            continue;
        }
        let mut row = branch_fields(branch);
        row.insert("form_code".into(), json!(summary.form_code));
        row.insert("form_name".into(), json!(deadline.form_name));
        row.insert("frequency".into(), json!("As needed"));
        row.insert("taxable_year".into(), json!(summary.taxable_year));
        row.insert("period".into(), json!(summary_period(summary)));
        row.insert(
            "period_label".into(),
            json!(format!(
                "{} {}",
                trigger.rule.trigger,
                trigger.date.format("%b %-d, %Y")
            )),
        );
        row.insert("due_date".into(), json!(due.format("%Y-%m-%d").to_string()));
        row.insert("status".into(), json!(status_name(&summary.status)));
        row.insert("draft_id".into(), json!(summary.id));
        row.insert(
            "return_key".into(),
            json!(db.draft_period_column(summary.id).map_err(error)?),
        );
        rows.push(Value::Object(row));
    }
    Ok(rows)
}

/// The as-needed forms each branch files in `year` (its yearly forms the
/// calendar files as needed), each with its saved returns that hold no
/// trigger date yet. Branches in `list_taxpayers` order, forms by code.
fn as_needed_form_rows(db: &Database, year: u16) -> Result<Vec<Value>, HostError> {
    let mut rows = Vec::new();
    for branch in branches(db)? {
        let mut codes: Vec<String> = db
            .active_form_codes_for_year(&branch.tin, year)
            .map_err(error)?
            .into_iter()
            .filter(|code| DeadlineResolver::is_as_needed(code))
            .collect();
        if codes.is_empty() {
            continue;
        }
        codes.sort();
        let drafts = db.list_draft_summaries(&branch.tin, year).map_err(error)?;
        for code in codes {
            let rule = event_due_rule(&code);
            let mut undated = Vec::new();
            for summary in drafts.iter().filter(|summary| summary.form_code == code) {
                if db.draft_event_trigger(summary.id).map_err(error)?.is_some() {
                    continue;
                }
                undated.push(json!({
                    "draft_id": summary.id,
                    "period": summary_period(summary),
                    "return_key": db.draft_period_column(summary.id).map_err(error)?,
                    "status": status_name(&summary.status),
                    "updated_at": summary.updated_at,
                }));
            }
            let form_name = bir_core::forms::FORM_REGISTRY
                .iter()
                .find(|form| form.code == code)
                .map_or(code.as_str(), |form| form.title);
            let mut row = branch_fields(&branch);
            row.insert("form_code".into(), json!(code));
            row.insert("form_name".into(), json!(form_name));
            row.insert("taxable_year".into(), json!(year));
            row.insert("trigger".into(), json!(rule.map(|rule| rule.trigger)));
            row.insert(
                "due_rule".into(),
                json!(rule.map_or("Upon the transaction or event", |rule| rule.description)),
            );
            row.insert("undated".into(), Value::Array(undated));
            rows.push(Value::Object(row));
        }
    }
    Ok(rows)
}

fn as_needed_forms(arguments: &gpui_shell::HostArguments) -> HostResult {
    let year = arguments.integer(0)?;
    let year =
        u16::try_from(year).map_err(|_| HostError::new(format!("a tax year, got {year}")))?;
    with_database(|db| serialize(&as_needed_form_rows(db, year)?))
}

fn deadlines_between(arguments: &gpui_shell::HostArguments) -> HostResult {
    let from = parse_date(arguments.string(0)?)?;
    let to = parse_date(arguments.string(1)?)?;
    with_database(|db| serialize(&deadline_rows(db, from, to)?))
}

/// Saved (Draft) and/or filed (anything past Draft) returns across every
/// branch, newest first.
fn return_rows(db: &Database, which: &str) -> Result<Vec<Value>, HostError> {
    let want_filed = matches!(which, "filed" | "all");
    let want_saved = matches!(which, "saved" | "all");
    if !want_filed && !want_saved {
        return Err(HostError::new(format!(
            "list_returns takes \"filed\", \"saved\" or \"all\", got {which:?}"
        )));
    }
    let branches = branches(db)?;
    // Filed returns come from one query (the notifications poll reads them
    // every 30 s); saved drafts are listed per branch and year.
    let mut summaries: Vec<FormDraftSummary> = Vec::new();
    if want_filed {
        summaries.extend(db.list_all_filed_submissions().map_err(error)?);
    }
    if want_saved {
        for branch in &branches {
            for year in FIRST_TAX_YEAR..=current_year() + 1 {
                summaries.extend(
                    db.list_draft_summaries(&branch.tin, year)
                        .map_err(error)?
                        .into_iter()
                        .filter(|summary| summary.status == FilingStatus::Draft),
                );
            }
        }
    }
    let mut rows = Vec::new();
    for summary in summaries {
        // A return whose TIN no longer matches a branch (a removed profile)
        // has nobody to show it under.
        let Some(branch) = branches.iter().find(|branch| branch.tin == summary.tin) else {
            continue;
        };
        let mut row = branch_fields(branch);
        row.insert("id".into(), json!(summary.id));
        row.insert("form_code".into(), json!(summary.form_code));
        row.insert("taxable_year".into(), json!(summary.taxable_year));
        row.insert("period".into(), json!(summary_period(&summary)));
        row.insert("period_label".into(), json!(summary_label(&summary)));
        row.insert("status".into(), json!(status_name(&summary.status)));
        row.insert("updated_at".into(), json!(summary.updated_at));
        row.insert(
            "return_key".into(),
            json!(db.draft_period_column(summary.id).map_err(error)?),
        );
        rows.push(Value::Object(row));
    }
    rows.sort_by(|a, b| b["updated_at"].as_str().cmp(&a["updated_at"].as_str()));
    Ok(rows)
}

/// The design's earliest tax year; the bound itself is enforced in bir-core.
const FIRST_TAX_YEAR: u16 = 2010;

fn current_year() -> u16 {
    use chrono::Datelike;
    u16::try_from(chrono::Local::now().year()).unwrap_or(2026)
}

fn list_returns(arguments: &gpui_shell::HostArguments) -> HostResult {
    let which = arguments.string(0)?.to_owned();
    with_database(|db| serialize(&return_rows(db, &which)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bir_module::{to_json, use_ephemeral_database};
    use bir_core::profile::TaxpayerType;
    use bir_core::tax_profile::{BranchFacts, NewTaxpayer};
    use gpui_shell::HostArguments;

    const HEAD: &str = "12345678800000";

    fn args(values: &[&str]) -> HostArguments {
        HostArguments::new(
            values
                .iter()
                .map(|v| gpui_shell::HostValue::Str(v.to_string())),
        )
    }

    /// "Example Trading Corp" filing 1601C in 2026, with a saved January draft.
    fn setup() {
        use_ephemeral_database();
        with_database(|db| {
            db.create_taxpayer(NewTaxpayer {
                tin_root: "123456788".into(),
                name: "Example Trading Corp".into(),
                taxpayer_type: TaxpayerType::Corporation,
                birth_date: None,
                head_office: BranchFacts {
                    rdo_code: "040".into(),
                    registered_address: "1 Example Street, Pasig".into(),
                    zip_code: "1600".into(),
                    phone: "0280000000".into(),
                    email: "office@example.com".into(),
                },
            })
            .unwrap();
            db.set_yearly_forms(HEAD, 2026, &["1601C"]).unwrap();
            let profile = db.get_profile(HEAD).unwrap().unwrap();
            let draft = bir_core::forms::Form1601CDraft::new_from_profile(&profile, 2026, 1);
            db.save_1601c_draft(&draft).unwrap();
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn deadlines_between_lists_each_filed_form_with_its_return_status() {
        setup();
        let rows = to_json(&deadlines_between(&args(&["2026-01-01", "2026-12-31"])).unwrap());
        let rows = rows.as_array().unwrap();
        assert!(!rows.is_empty());
        assert!(
            rows.iter()
                .all(|r| r["form_code"] == "1601C" && r["tin"] == HEAD)
        );
        let january = rows
            .iter()
            .find(|r| r["taxable_year"] == 2026 && r["period"] == 1)
            .expect("January 2026 1601C deadline");
        assert_eq!(january["status"], "Draft");
        assert_eq!(january["return_key"], 1, "the saved return opens exactly");
        assert_eq!(january["period_label"], "January 2026");
        assert_eq!(january["taxpayer_name"], "Example Trading Corp");
        let february = rows.iter().find(|r| r["period"] == 2).unwrap();
        assert_eq!(february["status"], "Not started");
        assert!(february["return_key"].is_null());
        let dates: Vec<&str> = rows
            .iter()
            .map(|r| r["due_date"].as_str().unwrap())
            .collect();
        let mut sorted = dates.clone();
        sorted.sort_unstable();
        assert_eq!(dates, sorted, "rows come oldest deadline first");
    }

    #[test]
    fn deadlines_between_refuses_a_date_that_is_not_iso() {
        setup();
        let error = deadlines_between(&args(&["01/01/2026", "2026-12-31"])).unwrap_err();
        assert!(error.message().contains("YYYY-MM-DD"));
    }

    /// "Example Trading Corp" files 1601C and the as-needed 1600WP, 2200A
    /// and 2553 in 2026, with a 2200A return for a removal on Wednesday
    /// October 7, a 1600WP return for a withholding on Monday October 5
    /// (20 days later is Sunday October 25), and a 2000OT return with no
    /// transaction date yet.
    fn setup_as_needed() {
        setup();
        with_database(|db| {
            db.set_yearly_forms(HEAD, 2026, &["1601C", "1600WP", "2200A", "2553"])
                .unwrap();
            let profile = db.get_profile(HEAD).unwrap().unwrap();
            let mut removal =
                bir_core::forms::form_2200a::Form2200ADraft::new_from_profile(&profile, 2026, 10);
            removal.day = 7;
            db.save_queueable_draft(&removal).unwrap();
            let mut withholding =
                bir_core::forms::form_1600wp::Form1600WpDraft::new_from_profile(&profile, 2026, 10);
            withholding.day = 5;
            db.save_queueable_draft(&withholding).unwrap();
            let undated =
                bir_core::forms::form_2000ot::Form2000OTDraft::new_from_profile(&profile, 2026);
            db.save_queueable_draft(&undated).unwrap();
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_dated_as_needed_return_is_a_deadline_row_due_from_its_trigger() {
        setup_as_needed();
        let rows = to_json(&deadlines_between(&args(&["2026-10-01", "2026-10-31"])).unwrap());
        let rows = rows.as_array().unwrap();
        let removal = rows
            .iter()
            .find(|r| r["form_code"] == "2200A")
            .expect("the 2200A return");
        assert_eq!(removal["frequency"], "As needed");
        assert_eq!(removal["period_label"], "Removal Oct 7, 2026");
        assert_eq!(removal["due_date"], "2026-10-07", "due on removal");
        assert_eq!(removal["status"], "Draft");
        assert_eq!(removal["taxable_year"], 2026);
        assert!(
            removal["return_key"].is_number(),
            "opens exactly that return"
        );
        let withholding = rows.iter().find(|r| r["form_code"] == "1600WP").unwrap();
        assert_eq!(withholding["period_label"], "Withholding Oct 5, 2026");
        assert_eq!(
            withholding["due_date"], "2026-10-26",
            "20 days after withholding, moved off Sunday"
        );
        assert!(
            rows.iter()
                .all(|r| r["form_code"] != "2000OT" && r["form_code"] != "2553"),
            "a return with no trigger date and a form with no return have no row"
        );
        // The periodic rows are as before: 1601C's September return, due
        // October 12 (October 10 is a Saturday).
        let periodic: Vec<_> = rows
            .iter()
            .filter(|r| r["frequency"] != "As needed")
            .map(|r| {
                (
                    r["form_code"].as_str().unwrap(),
                    r["period_label"].as_str().unwrap(),
                    r["due_date"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(periodic, [("1601C", "September 2026", "2026-10-12")]);
        let outside = to_json(&deadlines_between(&args(&["2026-11-01", "2026-12-31"])).unwrap());
        assert!(
            outside
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["frequency"] != "As needed"),
            "a due date outside the window is not listed"
        );
    }

    #[test]
    fn as_needed_forms_lists_the_year_s_as_needed_forms_with_undated_returns() {
        setup_as_needed();
        let rows = to_json(
            &as_needed_forms(&HostArguments::new([gpui_shell::HostValue::Number(2026.0)])).unwrap(),
        );
        let rows = rows.as_array().unwrap();
        let listed: Vec<_> = rows
            .iter()
            .map(|r| {
                (
                    r["form_code"].as_str().unwrap(),
                    r["trigger"].as_str(),
                    r["undated"].as_array().unwrap().len(),
                )
            })
            .collect();
        assert_eq!(
            listed,
            [
                ("1600WP", Some("Withholding"), 0),
                ("2200A", Some("Removal"), 0),
                ("2553", None, 0),
            ],
            "only the year's as-needed forms; 1601C is periodic, 2000OT is not in the forms"
        );
        assert!(
            rows.iter()
                .all(|r| r["tin"] == HEAD && r["taxable_year"] == 2026)
        );
        assert_eq!(
            rows[0]["due_rule"],
            "Within 20 days from the date the tax was deducted and withheld"
        );
        assert!(!rows[0]["form_name"].as_str().unwrap().is_empty());
        let none = to_json(
            &as_needed_forms(&HostArguments::new([gpui_shell::HostValue::Number(2025.0)])).unwrap(),
        );
        assert!(none.as_array().unwrap().is_empty(), "no forms in 2025");
    }

    #[test]
    fn an_undated_return_stays_in_the_as_needed_list() {
        setup_as_needed();
        with_database(|db| {
            db.set_yearly_forms(HEAD, 2026, &["2000OT"]).unwrap();
            Ok(())
        })
        .unwrap();
        let rows = to_json(
            &as_needed_forms(&HostArguments::new([gpui_shell::HostValue::Number(2026.0)])).unwrap(),
        );
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["form_code"], "2000OT");
        assert_eq!(rows[0]["trigger"], "Transaction");
        let undated = rows[0]["undated"].as_array().unwrap();
        assert_eq!(undated.len(), 1);
        assert_eq!(undated[0]["status"], "Draft");
        assert!(undated[0]["draft_id"].is_number());
        let calendar = to_json(&deadlines_between(&args(&["2026-01-01", "2027-12-31"])).unwrap());
        assert!(
            calendar
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["form_code"] != "2000OT")
        );
    }

    #[test]
    fn list_returns_separates_saved_from_filed() {
        setup();
        let saved = to_json(&list_returns(&args(&["saved"])).unwrap());
        let saved = saved.as_array().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0]["form_code"], "1601C");
        assert_eq!(saved[0]["status"], "Draft");
        assert_eq!(saved[0]["period_label"], "January 2026");
        assert_eq!(
            saved[0]["return_key"], 1,
            "a 1601C return's stored key is its month"
        );
        let filed = to_json(&list_returns(&args(&["filed"])).unwrap());
        assert!(filed.as_array().unwrap().is_empty());
        let all = to_json(&list_returns(&args(&["all"])).unwrap());
        assert_eq!(all.as_array().unwrap().len(), 1);
        assert!(list_returns(&args(&["everything"])).is_err());
    }
}
