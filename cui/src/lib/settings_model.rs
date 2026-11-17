//! What Settings can show, and which of it a search keeps (settings_model.ts). Pure data: the
//! screen draws a row only when its id is in the result.

/// A Settings section: its id, its label and icon, its group, whether only an admin sees it, and
/// its search words.
#[derive(Debug, Clone)]
pub struct SettingsSection {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub group: &'static str,
    pub admin_only: bool,
    pub search: &'static str,
}

/// A row of a section: its id, its search words, and whether only an admin sees it.
#[derive(Debug, Clone)]
pub struct SettingsRow {
    pub id: &'static str,
    pub search: &'static str,
    pub admin_only: bool,
}

/// A section as the search shows it: its rows (ids) that the search keeps.
#[derive(Debug, Clone)]
pub struct SettingsViewSection {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub group: &'static str,
    pub rows: Vec<&'static str>,
}

/// A choice in a pill: its stored value and the label shown.
#[derive(Debug, Clone)]
pub struct SettingsPreset {
    pub value: String,
    pub label: String,
}

pub const SETTINGS_SECTIONS: &[SettingsSection] = &[
    SettingsSection {
        id: "general",
        label: "General",
        icon: "settings",
        group: "Personal",
        admin_only: false,
        search: "general personal appearance",
    },
    SettingsSection {
        id: "account",
        label: "Account",
        icon: "user",
        group: "Personal",
        admin_only: false,
        search: "account personal profile",
    },
    SettingsSection {
        id: "transfer",
        label: "Import / Export",
        icon: "archive",
        group: "Data",
        admin_only: false,
        search: "import export data transfer",
    },
    SettingsSection {
        id: "jobs",
        label: "Jobs",
        icon: "list",
        group: "Data",
        admin_only: true,
        search: "jobs data background tasks",
    },
    SettingsSection {
        id: "computer",
        label: "Computer",
        icon: "laptop",
        group: "System",
        admin_only: true,
        search: "computer system this computer",
    },
];

fn rows_general() -> &'static [SettingsRow] {
    &[
        SettingsRow {
            id: "theme",
            search: "theme appearance light dark system",
            admin_only: false,
        },
        SettingsRow {
            id: "recent",
            search: "recent returns sidebar",
            admin_only: false,
        },
        SettingsRow {
            id: "compact",
            search: "compact form sections",
            admin_only: false,
        },
        SettingsRow {
            id: "updates",
            search: "download updates automatically release install",
            admin_only: false,
        },
    ]
}
fn rows_account() -> &'static [SettingsRow] {
    &[
        SettingsRow {
            id: "buwiz",
            search: "buwiz server cloud sign in sign out device",
            admin_only: false,
        },
        SettingsRow {
            id: "identity",
            search: "account name email tin profile sign out add account taxpayer",
            admin_only: false,
        },
        SettingsRow {
            id: "google",
            search: "google gmail connect disconnect oauth inbox tracking mail",
            admin_only: false,
        },
        SettingsRow {
            id: "security",
            search: "pin authenticator recovery key security",
            admin_only: false,
        },
    ]
}
fn rows_transfer() -> &'static [SettingsRow] {
    &[
        SettingsRow {
            id: "import",
            search: "import zip restore database",
            admin_only: true,
        },
        SettingsRow {
            id: "export",
            search: "export profile archive backup zip",
            admin_only: false,
        },
    ]
}
fn rows_jobs() -> &'static [SettingsRow] {
    &[SettingsRow {
        id: "jobs",
        search: "jobs background tasks queue logs confirmation watcher cron archive",
        admin_only: false,
    }]
}
fn rows_computer() -> &'static [SettingsRow] {
    &[
        SettingsRow {
            id: "data",
            search: "database data location path local files",
            admin_only: false,
        },
        SettingsRow {
            id: "logs",
            search: "logs folder retention log files",
            admin_only: false,
        },
        SettingsRow {
            id: "hotkey",
            search: "hotkey shortcut global show keyboard",
            admin_only: false,
        },
        SettingsRow {
            id: "tray",
            search: "tray menu bar close window hide quit",
            admin_only: false,
        },
        SettingsRow {
            id: "setup",
            search: "setup owner help desk who this computer is for",
            admin_only: false,
        },
        SettingsRow {
            id: "version",
            search: "version about ebirforms",
            admin_only: false,
        },
    ]
}

pub fn settings_rows(section: &str) -> &'static [SettingsRow] {
    match section {
        "general" => rows_general(),
        "account" => rows_account(),
        "transfer" => rows_transfer(),
        "jobs" => rows_jobs(),
        "computer" => rows_computer(),
        _ => &[],
    }
}

/// The lower-case words of a query, split on spaces.
pub fn query_words(query: &str) -> Vec<String> {
    query
        .trim()
        .to_lowercase()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// Every word of query occurs in blob. An empty query matches.
pub fn hits(query: &str, blob: &str) -> bool {
    let words = query_words(query);
    if words.is_empty() {
        return true;
    }
    let hay = blob.to_lowercase();
    words.iter().all(|w| hay.contains(w))
}

/// The rows a search keeps in a section.
fn visible_rows<'a>(
    query: &str,
    section_search: &str,
    rows: &'a [SettingsRow],
) -> Vec<&'a SettingsRow> {
    if query_words(query).is_empty() || hits(query, section_search) {
        rows.iter().collect()
    } else {
        rows.iter().filter(|r| hits(query, r.search)).collect()
    }
}

/// The sections and rows the search keeps for this role.
pub fn settings_view(query: &str, is_admin: bool) -> Vec<SettingsViewSection> {
    let mut out = Vec::new();
    for section in SETTINGS_SECTIONS {
        if section.admin_only && !is_admin {
            continue;
        }
        let rows = settings_rows(section.id);
        let shown: Vec<&SettingsRow> = rows.iter().filter(|r| is_admin || !r.admin_only).collect();
        let owned: Vec<SettingsRow> = shown.iter().map(|r| (**r).clone()).collect();
        let visible = visible_rows(query, section.search, &owned);
        let visible_ids: Vec<&'static str> = visible.iter().map(|r| r.id).collect();
        if !visible_ids.is_empty() {
            out.push(SettingsViewSection {
                id: section.id,
                label: section.label,
                icon: section.icon,
                group: section.group,
                rows: visible_ids,
            });
        }
    }
    out
}

/// The section to draw: the one selected, or else the first the search still shows.
pub fn shown_section(selected: Option<&str>, view: &[SettingsViewSection]) -> Option<&'static str> {
    view.iter()
        .find(|s| Some(s.id) == selected)
        .or_else(|| view.first())
        .map(|s| s.id)
}

/// The label for a stored chord. Empty is Off; an unknown chord is shown as stored.
pub fn hotkey_label(combo: Option<&str>, presets: &[SettingsPreset]) -> String {
    let value = combo.unwrap_or("").trim();
    if value.is_empty() {
        return "Off".to_owned();
    }
    presets
        .iter()
        .find(|p| p.value == value)
        .map(|p| p.label.clone())
        .unwrap_or_else(|| value.to_owned())
}

/// The options for the hotkey pill: Off first, the presets, and a custom chord kept at the end.
pub fn hotkey_options(combo: Option<&str>, presets: &[SettingsPreset]) -> Vec<SettingsPreset> {
    let mut options = vec![SettingsPreset {
        value: String::new(),
        label: "Off".to_owned(),
    }];
    options.extend(presets.iter().cloned());
    let value = combo.unwrap_or("").trim();
    if !value.is_empty() && !options.iter().any(|o| o.value == value) {
        options.push(SettingsPreset {
            value: value.to_owned(),
            label: value.to_owned(),
        });
    }
    options
}

/// How long the log is kept, in words. Zero, a negative or an unreadable count is the seven-day default.
pub fn retention_label(days: i64) -> String {
    let n = if days > 0 { days } else { 7 };
    if n == 1 {
        "1 day".to_owned()
    } else {
        format!("{n} days")
    }
}

/// The account a Settings card names: a taxpayer, or the computer's local account.
pub struct AccountSummary {
    pub name: String,
    pub detail: String,
    pub email: String,
    pub mailbox: Option<(String, String, String)>, // (tin, label, email)
    pub show_add: bool,
}

/// The account card (accountSummary).
pub fn account_summary(
    taxpayers: &[crate::types::Taxpayer],
    branch: Option<&crate::types::Branch>,
    is_admin: bool,
    is_helpdesk: bool,
    admin_root: Option<&str>,
    session_kind: Option<&str>,
    session_root: Option<&str>,
    can_add_taxpayer: bool,
) -> AccountSummary {
    let own = |root: Option<&str>| root.and_then(|r| taxpayers.iter().find(|t| t.tin_root == r));
    let email_of = |b: Option<&crate::types::Branch>| {
        b.and_then(|b| b.facts.email.clone()).unwrap_or_default()
    };
    let (mut name, mut detail, mut email) = (
        "Local account".to_owned(),
        "This computer".to_owned(),
        String::new(),
    );
    if is_admin && is_helpdesk {
        name = "Help desk admin".to_owned();
        detail = "TIN 000-000-000".to_owned();
    } else if is_admin {
        if let Some(owner) = own(admin_root) {
            name = owner.name.clone();
        }
        detail = "Admin · this computer".to_owned();
        email = email_of(branch);
    } else if session_kind == Some("taxpayer") {
        if let Some(tp) = own(session_root) {
            name = tp.name.clone();
            if !tp.tin_display.is_empty() {
                detail = format!("TIN {}", tp.tin_display);
            }
        }
        email = email_of(branch);
    }
    let mailbox = branch.map(|b| {
        (
            b.tin.clone(),
            if b.name.is_empty() {
                name.clone()
            } else {
                b.name.clone()
            },
            email_of(Some(b)),
        )
    });
    AccountSummary {
        name,
        detail,
        email,
        mailbox,
        show_add: can_add_taxpayer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_requires_every_word() {
        assert!(hits("theme dark", "theme appearance light dark system"));
        assert!(!hits("theme zebra", "theme appearance"));
        assert!(hits("", "anything"));
    }

    #[test]
    fn settings_view_hides_admin_sections() {
        let view = settings_view("", false);
        assert!(!view.iter().any(|s| s.id == "jobs" || s.id == "computer"));
        let admin = settings_view("", true);
        assert!(admin.iter().any(|s| s.id == "jobs"));
    }

    #[test]
    fn retention_label_defaults() {
        assert_eq!(retention_label(0), "7 days");
        assert_eq!(retention_label(1), "1 day");
        assert_eq!(retention_label(30), "30 days");
    }

    #[test]
    fn hotkey_options_keeps_custom() {
        let opts = hotkey_options(Some("ctrl+alt+k"), &[]);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[1].value, "ctrl+alt+k");
        assert_eq!(hotkey_label(None, &[]), "Off");
    }
}

// ---------------------------------------------------------------- jobs & logs (settings/logic.ts)

/// The status filters of the Jobs list, in the order they show (STATUS_FILTERS).
pub const STATUS_FILTERS: [&str; 5] = ["All Jobs", "Queued", "Failed", "Completed", "Archived"];

/// The log's level filters: the stored value and the label (LOG_FILTERS).
pub const LOG_FILTERS: [(&str, &str); 4] = [
    ("All Logs", "All"),
    ("Error", "Error"),
    ("Warn", "Warn"),
    ("Info", "Info"),
];

/// Lines drawn from the tail of the log; older ones stay in the file (LOG_LINES).
pub const LOG_LINES: usize = 200;

/// A log line's level, by the same markers bir-core classifies on (log_line_level).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Other,
}

/// The level of one log line (logLineLevel).
pub fn log_line_level(line: &str) -> LogLevel {
    if line.contains(" ERROR ") || line.contains(" FATAL ") {
        LogLevel::Error
    } else if line.contains(" WARN ") {
        LogLevel::Warn
    } else if line.contains(" INFO ") {
        LogLevel::Info
    } else {
        LogLevel::Other
    }
}

/// The text tone a level is drawn in (levelTone).
pub fn level_tone(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "danger",
        LogLevel::Warn => "warning",
        LogLevel::Info => "fg",
        LogLevel::Other => "fg2",
    }
}

/// What the Logs tab shows of one file's text (LogView).
#[derive(Debug, Default)]
pub struct LogView {
    /// Lines at the chosen level, every one of the file's tail.
    pub matching: usize,
    /// The newest LOG_LINES of them, oldest first.
    pub shown: Vec<(usize, String, LogLevel)>,
}

/// Splits the tail the host gave and keeps one level (logView).
pub fn log_view(content: Option<&str>, filter: &str) -> LogView {
    let mut lines: Vec<&str> = content.unwrap_or("").split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let matching: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| {
            filter == "All Logs"
                || match filter {
                    "Error" => log_line_level(line) == LogLevel::Error,
                    "Warn" => log_line_level(line) == LogLevel::Warn,
                    "Info" => log_line_level(line) == LogLevel::Info,
                    _ => false,
                }
        })
        .collect();
    let start = matching.len().saturating_sub(LOG_LINES);
    LogView {
        matching: matching.len(),
        shown: matching[start..]
            .iter()
            .enumerate()
            .map(|(i, line)| (start + i, line.to_string(), log_line_level(line)))
            .collect(),
    }
}

/// The Logs tab's header line (logHeadline).
pub fn log_headline(file: Option<&str>, lines: i64, matching: usize) -> String {
    format!(
        "{} · {} lines{}",
        file.unwrap_or("No log file yet"),
        lines,
        if matching > LOG_LINES {
            format!(" · showing the last {LOG_LINES}")
        } else {
            String::new()
        }
    )
}

/// The jobs a status filter keeps (filterJobs).
pub fn filter_jobs(
    jobs: &[crate::types::BackgroundJob],
    filter: &str,
) -> Vec<crate::types::BackgroundJob> {
    if filter == "All Jobs" {
        jobs.to_vec()
    } else {
        jobs.iter()
            .filter(|job| job.status == filter)
            .cloned()
            .collect()
    }
}

/// The chip tone of a job's status (jobTone).
pub fn job_tone(status: &str) -> &'static str {
    if status == "Queued" || status == "Running" {
        "accent"
    } else if status == "Failed" {
        "danger"
    } else if ["Completed", "Submitted", "Confirmed", "Paid"].contains(&status) {
        "success"
    } else {
        "neutral"
    }
}

/// What a job row offers (jobActions): a queued filing can be cancelled; a stored job can be
/// archived (unless it is) and deleted.
pub fn job_actions(job: &crate::types::BackgroundJob) -> (bool, bool, bool) {
    let stored = job.kind == "job" && job.id.is_some();
    (
        job.kind == "submission" && job.status == "Queued",
        stored && job.status != "Archived",
        stored,
    )
}

/// The key a job row is identified by (jobKey).
pub fn job_key(job: &crate::types::BackgroundJob) -> String {
    format!(
        "{}-{}",
        job.kind,
        job.id.map(|i| i.to_string()).unwrap_or_default()
    )
}

/// The words under a job's name (jobSub): its schedule, then its last and next run.
pub fn job_sub(job: &crate::types::BackgroundJob) -> String {
    let schedule = if let Some(cron) = job.cron_expr.as_deref().filter(|c| !c.is_empty()) {
        format!("Schedule: {cron}")
    } else if job.job_type == "System" {
        "Runs when the queue wakes".to_owned()
    } else {
        "Manual".to_owned()
    };
    let runs = [
        job.last_run_at.as_ref().map(|t| format!("last {t}")),
        job.next_run_at.as_ref().map(|t| format!("next {t}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    [schedule, runs]
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The words under the cloud account's server row (serverHint).
pub fn server_hint(source: Option<&str>) -> &'static str {
    match source {
        Some("env") => "From BUWIZ_SERVER_URL for this run.",
        Some("build") => "Set when this app was built.",
        Some("invalid") => "That address was rejected.",
        _ => "Chosen for this build.",
    }
}

/// The retention select's choices: the host's, and the current one when it is none of them
/// (retentionChoices).
pub fn retention_choices(info: Option<&crate::types::SystemInfo>) -> Vec<SettingsPreset> {
    let days: Vec<i64> = info
        .filter(|i| !i.log_retention_choices.is_empty())
        .map(|i| i.log_retention_choices.clone())
        .unwrap_or_else(|| vec![3, 7, 14, 30, 90]);
    let mut choices: Vec<SettingsPreset> = days
        .iter()
        .map(|d| SettingsPreset {
            value: d.to_string(),
            label: retention_label(*d),
        })
        .collect();
    let current = info.map(|i| i.log_retention_days).unwrap_or(7).max(0);
    let current_s = current.to_string();
    if !choices.iter().any(|o| o.value == current_s) {
        choices.push(SettingsPreset {
            value: current_s,
            label: retention_label(current),
        });
    }
    choices
}

/// The value a select that has an "Off" (empty) choice keeps for it (OFF).
pub const OFF: &str = "off";

/// The hotkey select's choices: Off first (as OFF), the presets, a stored chord that is none of
/// them (hotkeyChoices).
pub fn hotkey_choices(combo: Option<&str>, presets: &[SettingsPreset]) -> Vec<SettingsPreset> {
    hotkey_options(combo, presets)
        .into_iter()
        .map(|o| {
            if o.value.is_empty() {
                SettingsPreset {
                    value: OFF.to_owned(),
                    label: o.label,
                }
            } else {
                o
            }
        })
        .collect()
}

/// The sections of a settings view grouped by their group name, in order (groupSections).
pub fn group_sections(view: &[SettingsViewSection]) -> Vec<(String, Vec<SettingsViewSection>)> {
    let mut out: Vec<(String, Vec<SettingsViewSection>)> = Vec::new();
    for section in view {
        if let Some(last) = out.last_mut()
            && last.0 == section.group
        {
            last.1.push(section.clone());
            continue;
        }
        out.push((section.group.to_string(), vec![section.clone()]));
    }
    out
}

#[cfg(test)]
mod settings_logic_tests {
    use super::*;

    fn mk_job(extra: serde_json::Value) -> crate::types::BackgroundJob {
        let mut base = serde_json::json!({
            "kind": "job", "id": 1, "name": "n", "job_type": "Manual",
            "cron_expr": null, "status": "Completed", "retries": 0,
            "last_run_at": null, "next_run_at": null, "created_at": "",
            "output_log": null,
        });
        if let (Some(b), Some(e)) = (base.as_object_mut(), extra.as_object()) {
            for (k, v) in e {
                b.insert(k.clone(), v.clone());
            }
        }
        serde_json::from_value(base).unwrap()
    }

    #[test]
    fn t_log_level_and_view() {
        assert_eq!(
            log_line_level("2026-11-15T08:00:00Z ERROR bir::x boom"),
            LogLevel::Error
        );
        assert_eq!(
            log_line_level("2026-11-15T08:00:00Z FATAL bir::x boom"),
            LogLevel::Error
        );
        assert_eq!(
            log_line_level("2026-11-15T08:00:00Z  WARN bir::x careful"),
            LogLevel::Warn
        );
        assert_eq!(
            log_line_level("2026-11-15T08:00:00Z  INFO bir::x fine"),
            LogLevel::Info
        );
        assert_eq!(log_line_level("a continuation line"), LogLevel::Other);
        assert_eq!(log_line_level("INFORMATION about ERRORS"), LogLevel::Other);
        assert_eq!(level_tone(LogLevel::Error), "danger");
        assert_eq!(level_tone(LogLevel::Warn), "warning");
        assert_eq!(level_tone(LogLevel::Info), "fg");
        assert_eq!(level_tone(LogLevel::Other), "fg2");

        let v = log_view(Some("a\n INFO b\n ERROR c\n"), "All Logs");
        let lines: Vec<&str> = v.shown.iter().map(|l| l.1.as_str()).collect();
        assert_eq!(lines, vec!["a", " INFO b", " ERROR c"]);
        assert_eq!(v.matching, 3);
        let e = log_view(Some("a\n INFO b\n ERROR c\n"), "Error");
        assert_eq!(e.shown.len(), 1);
        assert_eq!(e.shown[0].1, " ERROR c");
        assert!(log_view(Some(""), "All Logs").shown.is_empty());
        assert_eq!(log_view(None, "Info").matching, 0);
        let many: Vec<String> = (0..450).map(|i| format!(" INFO line {i}")).collect();
        let big = log_view(Some(&many.join("\n")), "Info");
        assert_eq!(big.matching, 450);
        assert_eq!(big.shown.len(), LOG_LINES);
        assert_eq!(big.shown[0].1, " INFO line 250");
        assert_eq!(big.shown[LOG_LINES - 1].1, " INFO line 449");
        assert_eq!(
            log_headline(Some("f.log"), 450, 450),
            "f.log · 450 lines · showing the last 200"
        );
        assert_eq!(log_headline(Some("f.log"), 12, 3), "f.log · 12 lines");
        assert_eq!(log_headline(Some(""), 0, 0), " · 0 lines");
    }

    #[test]
    fn t_job_filters_and_actions() {
        let jobs = vec![
            mk_job(serde_json::json!({"id": 1, "status": "Queued"})),
            mk_job(serde_json::json!({"id": 2, "status": "Failed"})),
            mk_job(serde_json::json!({"id": 3, "status": "Archived"})),
            mk_job(serde_json::json!({"id": 4, "status": "Failed"})),
        ];
        assert_eq!(filter_jobs(&jobs, "All Jobs").len(), 4);
        let failed: Vec<i64> = filter_jobs(&jobs, "Failed")
            .iter()
            .map(|j| j.id.unwrap())
            .collect();
        assert_eq!(failed, vec![2, 4]);
        assert!(filter_jobs(&jobs, "Completed").is_empty());

        let sub = mk_job(serde_json::json!({"kind": "submission", "status": "Queued", "id": 5}));
        assert_eq!(job_actions(&sub), (true, false, false));
        let sub_done =
            mk_job(serde_json::json!({"kind": "submission", "status": "Submitted", "id": 5}));
        assert_eq!(job_actions(&sub_done), (false, false, false));
        let done = mk_job(serde_json::json!({"status": "Completed"}));
        assert_eq!(job_actions(&done), (false, true, true));
        let arch = mk_job(serde_json::json!({"status": "Archived"}));
        assert_eq!(job_actions(&arch), (false, false, true));
        let noid = mk_job(serde_json::json!({"id": null}));
        assert_eq!(job_actions(&noid), (false, false, false));
        assert_eq!(job_key(&done), "job-1");
        assert_eq!(job_key(&sub), "submission-5");

        assert_eq!(job_tone("Queued"), "accent");
        assert_eq!(job_tone("Running"), "accent");
        assert_eq!(job_tone("Failed"), "danger");
        assert_eq!(job_tone("Completed"), "success");
        assert_eq!(job_tone("Submitted"), "success");
        assert_eq!(job_tone("Confirmed"), "success");
        assert_eq!(job_tone("Paid"), "success");
        assert_eq!(job_tone("Archived"), "neutral");

        let cron = mk_job(
            serde_json::json!({"cron_expr": "0 8 * * 1", "last_run_at": "2026-11-10", "next_run_at": "2026-11-17"}),
        );
        assert_eq!(
            job_sub(&cron),
            "Schedule: 0 8 * * 1 · last 2026-11-10 · next 2026-11-17"
        );
        let sys = mk_job(serde_json::json!({"job_type": "System"}));
        assert_eq!(job_sub(&sys), "Runs when the queue wakes");
        let man = mk_job(serde_json::json!({"job_type": "Manual", "last_run_at": "2026-11-10"}));
        assert_eq!(job_sub(&man), "Manual · last 2026-11-10");
    }

    #[test]
    fn t_server_hint_retention_hotkey_group() {
        assert_eq!(
            server_hint(Some("env")),
            "From BUWIZ_SERVER_URL for this run."
        );
        assert_eq!(server_hint(Some("build")), "Set when this app was built.");
        assert_eq!(server_hint(Some("invalid")), "That address was rejected.");
        assert_eq!(server_hint(Some("default")), "Chosen for this build.");
        assert_eq!(server_hint(None), "Chosen for this build.");
    }

    #[test]
    fn t_group_sections() {
        let s = |id: &'static str, group: &'static str| SettingsViewSection {
            id,
            label: id,
            icon: "x",
            group,
            rows: vec![],
        };
        let groups = group_sections(&[s("a", "One"), s("b", "One"), s("c", "Two")]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "One");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, "Two");
        assert!(group_sections(&[]).is_empty());
    }
}
