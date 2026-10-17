//! Home, Calendar and Filings feeds for the redesigned shell: deadlines for
//! the forms each branch files, and the saved and filed returns across every
//! taxpayer. Read-only; nothing here writes.
//!
//! Deadlines come from `calendar_rules::DeadlineResolver::deadlines_for_forms`
//! over each branch's yearly forms (the per-year Forms Set), with the user's
//! deadline overrides applied, exactly as the painted dashboard resolves them.

use bir_core::calendar_rules::{DeadlinePeriod, DeadlineResolver};
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
    frequency: "Monthly" | "Quarterly" | "Annual";
    taxable_year: number;
    /** Month 1-12, quarter 1-4, or 0 for an annual return. */
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
            if codes.is_empty() {
                continue;
            }
            let drafts = db
                .list_draft_summaries(&branch.tin, year16)
                .map_err(error)?;
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
