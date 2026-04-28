#![allow(dead_code)]
use bir_core::db::{Database, Job};
use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::components::combobox::{Combobox, ComboboxEvent, ComboboxState};
use chrono::Utc;
use gpui_component::input::{Input, InputState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

/// Check if the bir-daemon process is currently running.
fn is_daemon_running() -> bool {
    std::process::Command::new("pgrep")
        .arg("-x")
        .arg("bir-daemon")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub enum CronTasksEvent {
    Reload,
}

impl EventEmitter<CronTasksEvent> for CronTasksView {}

fn humanize_cron(expr: &str) -> String {
    let expr = expr.trim();
    if expr == "0 * * * * *" || expr == "* * * * *" {
        return "Every minute".to_string();
    }
    if let Some(stripped) = expr.strip_prefix("*/")
        && stripped.ends_with(" * * * * *") {
            let num = stripped.trim_end_matches(" * * * * *");
            return format!("Every {} seconds", num);
        }
    if let Some(stripped) = expr.strip_prefix("0 */")
        && stripped.ends_with(" * * * *") {
            let num = stripped.trim_end_matches(" * * * *");
            if num == "1" {
                return "Every minute".to_string();
            }
            return format!("Every {} minutes", num);
        }
    if let Some(stripped) = expr.strip_prefix("0 0 */")
        && stripped.ends_with(" * * *") {
            let num = stripped.trim_end_matches(" * * *");
            if num == "1" {
                return "Every hour".to_string();
            }
            return format!("Every {} hours", num);
        }
    if let Some(stripped) = expr.strip_prefix("0 0 0 */")
        && stripped.ends_with(" * *") {
            let num = stripped.trim_end_matches(" * *");
            if num == "1" {
                return "Everyday".to_string();
            }
            return format!("Every {} days", num);
        }
    if let Some(stripped) = expr.strip_prefix("0 0 0 1 */")
        && stripped.ends_with(" *") {
            let num = stripped.trim_end_matches(" *");
            if num == "1" {
                return "Every month".to_string();
            }
            return format!("Every {} months", num);
        }
    // Fallback
    format!("Frequency: {}", expr)
}

#[derive(Clone)]
pub struct JobViewModel {
    pub is_system: bool,
    pub id: Option<i64>,
    pub name: String,
    pub job_type: String,
    pub cron_expr: Option<String>,
    pub command: Option<String>,
    pub status: String,
    pub retries: i64,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub created_at: String,
    pub output_log: Option<String>,
}

pub struct CronTasksView {
    db: Arc<Mutex<Database>>,
    background_cron_enabled: bool,
    error_telemetry_enabled: bool,
    daemon_running: bool,

    jobs: Vec<JobViewModel>,
    new_job_name: Entity<InputState>,
    cron_amount: Entity<InputState>,
    cron_period: Entity<ComboboxState>,
    new_job_command: Entity<InputState>,
    filter_combobox: Entity<ComboboxState>,
    search_input: Entity<InputState>,
    test_output: Option<String>,
    _subscriptions: Vec<Subscription>,
    current_page: usize,
    items_per_page: usize,
}

impl CronTasksView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let new_job_name =
            cx.new(|cx| InputState::new(window, cx).placeholder("Job Name (e.g. Sync)"));
        let cron_amount = cx.new(|cx| InputState::new(window, cx).placeholder("Amt (e.g. 5)"));
        let cron_period = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "Seconds".into(),
                    "Minutes".into(),
                    "Hours".into(),
                    "Days".into(),
                    "Months".into(),
                    "Raw Cron".into(),
                ],
                window,
                cx,
            )
        });

        // Default to Minutes
        cron_period.update(cx, |s, cx| s.set_selected_value("Minutes", window, cx));

        let new_job_command =
            cx.new(|cx| InputState::new(window, cx).placeholder("Cmd (e.g. osascript -e ...)"));

        let filter_combobox = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "All Jobs".into(),
                    "Queued".into(),
                    "Failed".into(),
                    "Archived".into(),
                ],
                window,
                cx,
            )
        });
        filter_combobox.update(cx, |s, cx| s.set_selected_value("All Jobs", window, cx));

        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search job name or type..."));

        let daemon_running = is_daemon_running();

        let mut view = Self {
            db,
            background_cron_enabled: false,
            error_telemetry_enabled: false,
            daemon_running,

            jobs: Vec::new(),
            new_job_name,
            cron_amount,
            cron_period,
            new_job_command,
            filter_combobox: filter_combobox.clone(),
            search_input: search_input.clone(),
            test_output: None,
            _subscriptions: Vec::new(),
            current_page: 1,
            items_per_page: 5,
        };

        let sub = cx.subscribe_in(
            &filter_combobox,
            window,
            |_this: &mut Self, _entity, _event: &ComboboxEvent, _window, cx| {
                _this.current_page = 1;
                cx.notify();
            },
        );
        view._subscriptions.push(sub);

        let sub2 = cx.subscribe_in(
            &search_input,
            window,
            |_this: &mut Self, _entity, event: &gpui_component::input::InputEvent, _window, cx| {
                if let gpui_component::input::InputEvent::Change = event {
                    _this.current_page = 1;
                    cx.notify();
                }
            },
        );
        view._subscriptions.push(sub2);

        view.load_settings(cx);
        view
    }

    pub fn load_settings(&mut self, cx: &mut Context<'_, Self>) {
        let mut view_jobs = Vec::new();
        if let Ok(db) = self.db.lock() {
            self.background_cron_enabled = db
                .get_setting("background_cron_enabled")
                .unwrap_or(Some("true".to_string()))
                .map(|s| s == "true")
                .unwrap_or(true);
            self.error_telemetry_enabled = db
                .get_setting("error_telemetry_enabled")
                .unwrap_or(Some("false".to_string()))
                .map(|s| s == "true")
                .unwrap_or(false);
            if let Ok(jobs) = db.list_jobs() {
                for job in jobs {
                    let mut display_name = job.name.clone();
                    if display_name.starts_with("Poll Receipts: ") {
                        let email = display_name.trim_start_matches("Poll Receipts: ");
                        display_name =
                            format!("Waiting for 2551Q confirmation email for {}", email);
                    }
                    view_jobs.push(JobViewModel {
                        is_system: false,
                        id: job.id,
                        name: display_name,
                        job_type: job.job_type,
                        cron_expr: job.cron_expr,
                        command: job.command,
                        status: job.status,
                        retries: job.retries,
                        last_run_at: job.last_run_at,
                        next_run_at: job.next_run_at,
                        created_at: job.created_at,
                        output_log: job.output_log,
                    });
                }
            }

            if let Ok(summaries) = db.list_all_queued_submissions() {
                for sum in summaries {
                    view_jobs.push(JobViewModel {
                        is_system: true,
                        id: Some(sum.id),
                        name: format!("Submit {} for {}", sum.form_code, sum.tin),
                        job_type: "System".to_string(),
                        cron_expr: None,
                        command: None,
                        status: format!("{:?}", sum.status),
                        retries: 0,
                        last_run_at: None,
                        next_run_at: None,
                        created_at: sum.updated_at,
                        output_log: None,
                    });
                }
            }
        }

        // Sort descending by created_at (simple string compare works for RFC3339)
        view_jobs.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        self.jobs = view_jobs;
        cx.notify();
    }

    fn toggle_cron(&mut self, value: bool, cx: &mut Context<'_, Self>) {
        self.background_cron_enabled = value;
        self.save_to_db(cx);
    }

    fn toggle_telemetry(&mut self, value: bool, cx: &mut Context<'_, Self>) {
        self.error_telemetry_enabled = value;
        self.save_to_db(cx);
    }

    fn toggle_daemon(&mut self, value: bool, cx: &mut Context<'_, Self>) {
        self.daemon_running = value;
        if value {
            // Start the daemon
            bir_core::daemon_installer::install();
        } else {
            // Stop the daemon: unload from launchctl and kill the process
            bir_core::daemon_installer::uninstall();
            let _ = std::process::Command::new("killall")
                .arg("bir-daemon")
                .output();
        }
        // Re-check actual state after a brief delay
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;
            cx.update(|cx| {
                if let Some(view) = view.upgrade() {
                    view.update(cx, |this, cx| {
                        this.daemon_running = is_daemon_running();
                        cx.notify();
                    });
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn save_to_db(&mut self, cx: &mut Context<'_, Self>) {
        if let Ok(db) = self.db.lock() {
            let _ = db.set_setting(
                "background_cron_enabled",
                if self.background_cron_enabled {
                    "true"
                } else {
                    "false"
                },
            );
            let _ = db.set_setting(
                "error_telemetry_enabled",
                if self.error_telemetry_enabled {
                    "true"
                } else {
                    "false"
                },
            );
        }
        cx.notify();
    }

    fn build_cron_string(&self, amount: &str, period: &str) -> Result<String, String> {
        if period == "Raw Cron" {
            return Ok(amount.trim().to_string());
        }
        let amount_num = amount
            .parse::<u32>()
            .map_err(|_| "Amount must be a number".to_string())?;
        if amount_num < 1 {
            return Err("Amount must be >= 1".to_string());
        }

        let cron = match period {
            "Seconds" => format!("*/{} * * * * *", amount_num),
            "Minutes" => format!("0 */{} * * * *", amount_num),
            "Hours" => format!("0 0 */{} * * *", amount_num),
            "Days" => format!("0 0 0 */{} * *", amount_num),
            "Months" => format!("0 0 0 1 */{} *", amount_num),
            _ => return Err("Invalid period".to_string()),
        };
        Ok(cron)
    }

    fn test_command(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        let cmd_str = self.new_job_command.read(cx).value().to_string();
        if cmd_str.trim().is_empty() {
            self.test_output = Some("Command is empty".to_string());
            cx.notify();
            return;
        }

        self.test_output = Some("Testing...".to_string());
        cx.notify();

        let cmd = cmd_str.clone();
        cx.spawn(async move |this, cx| {
            let output = match bir_core::platform::run_shell_command(&cmd).await {
                Ok(out) => {
                    let mut res = String::from_utf8_lossy(&out.stdout).to_string();
                    if !out.status.success() {
                        res.push_str(&format!(
                            "\nError: {}",
                            String::from_utf8_lossy(&out.stderr)
                        ));
                    }
                    if res.trim().is_empty() {
                        "Success (no output)".to_string()
                    } else {
                        res
                    }
                }
                Err(e) => format!("Failed to execute: {}", e),
            };
            cx.update(|cx| {
                if let Some(this) = this.upgrade() {
                    this.update(cx, |this, cx| {
                        this.test_output = Some(output);
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    fn add_job(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let name = self.new_job_name.read(cx).value().to_string();
        let amount = self.cron_amount.read(cx).value().to_string();
        let period = self.cron_period.read(cx).selected_value(cx);
        let cmd_str = self.new_job_command.read(cx).value().to_string();

        if name.trim().is_empty() {
            self.test_output = Some("Name is required".to_string());
            return;
        }

        let cron_expr = if amount.trim().is_empty() {
            None
        } else {
            match self.build_cron_string(&amount, &period) {
                Ok(expr) => Some(expr),
                Err(e) => {
                    self.test_output = Some(e);
                    return;
                }
            }
        };

        let command = if cmd_str.trim().is_empty() {
            None
        } else {
            Some(cmd_str.trim().to_string())
        };

        let job = Job {
            id: None,
            name,
            job_type: "Custom".to_string(),
            cron_expr,
            command,
            status: "Queued".to_string(),
            retries: 0,
            last_run_at: None,
            next_run_at: None,
            created_at: "".to_string(),
            output_log: None,
        };

        if let Ok(db) = self.db.lock() {
            let _ = db.save_job(job);
        }

        self.new_job_name
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.cron_amount
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.new_job_command
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.test_output = None;

        self.load_settings(cx);
    }

    fn delete_job(&mut self, id: i64, cx: &mut Context<'_, Self>) {
        if let Ok(db) = self.db.lock() {
            let _ = db.delete_job(id);
        }
        self.load_settings(cx);
    }

    fn archive_job(&mut self, id: i64, cx: &mut Context<'_, Self>) {
        if let Ok(db) = self.db.lock()
            && let Ok(jobs) = db.list_jobs()
                && let Some(mut job) = jobs.into_iter().find(|j| j.id == Some(id)) {
                    job.status = "Archived".to_string();
                    let _ = db.save_job(job);
                }
        self.load_settings(cx);
    }

    fn purge_archived(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Ok(db) = self.db.lock() {
            let _ = db.delete_archived_jobs();
        }
        self.load_settings(cx);
    }

    fn run_job_now(&mut self, db_id: i64, cx: &mut Context<'_, Self>) {
        // Find the job
        let job = if let Ok(db) = self.db.lock() {
            if let Ok(jobs) = db.list_jobs() {
                jobs.into_iter().find(|j| j.id == Some(db_id))
            } else {
                None
            }
        } else {
            None
        };

        let Some(job) = job else { return };

        // For email polling jobs, execute inline instead of waiting for daemon
        if let Some(ref cmd) = job.command
            && cmd.starts_with("bir_poll_email ") {
                let email = cmd.trim_start_matches("bir_poll_email ").trim().to_string();
                let db = self.db.clone();
                let job_id = db_id;

                // Mark as Running
                if let Ok(db_guard) = db.lock()
                    && let Ok(jobs) = db_guard.list_jobs()
                        && let Some(mut j) = jobs.into_iter().find(|j| j.id == Some(job_id)) {
                            j.status = "Running".to_string();
                            let _ = db_guard.save_job(j);
                        }
                self.load_settings(cx);

                cx.spawn(async move |this, cx| {
                    let _result = cx
                        .background_executor()
                        .spawn(async move {
                            let (poll_success, still_pending, err_msg) =
                                bir_core::email::fetch_and_process_emails_for_address(
                                    &email,
                                    db.clone(),
                                );

                            // Update the job in DB
                            if let Ok(db_guard) = db.lock()
                                && let Ok(jobs) = db_guard.list_jobs()
                                    && let Some(mut j) =
                                        jobs.into_iter().find(|j| j.id == Some(job_id))
                                    {
                                        j.last_run_at = Some(Utc::now().to_rfc3339());
                                        if poll_success {
                                            j.output_log = Some(
                                                "Email polling completed successfully.".to_string(),
                                            );
                                            j.retries = 0;
                                            if !still_pending {
                                                j.status = "Archived".to_string();
                                            } else {
                                                j.status = "Queued".to_string();
                                                // Set next run from cron
                                                if let Some(ref expr) = j.cron_expr
                                                    && let Ok(schedule) =
                                                        std::str::FromStr::from_str(expr)
                                                    {
                                                        let schedule: cron::Schedule = schedule;
                                                        if let Some(next) =
                                                            schedule.upcoming(chrono::Utc).next()
                                                        {
                                                            j.next_run_at = Some(next.to_rfc3339());
                                                        }
                                                    }
                                            }
                                        } else {
                                            j.output_log = Some(err_msg.unwrap_or_else(|| {
                                                "Email polling failed (unknown).".to_string()
                                            }));
                                            j.retries += 1;
                                            j.status = "Queued".to_string();
                                            if let Some(ref expr) = j.cron_expr
                                                && let Ok(schedule) =
                                                    std::str::FromStr::from_str(expr)
                                                {
                                                    let schedule: cron::Schedule = schedule;
                                                    if let Some(next) =
                                                        schedule.upcoming(chrono::Utc).next()
                                                    {
                                                        j.next_run_at = Some(next.to_rfc3339());
                                                    }
                                                }
                                        }
                                        let _ = db_guard.save_job(j);
                                    }
                            poll_success
                        })
                        .await;

                    cx.update(|cx| {
                        if let Some(view) = this.upgrade() {
                            view.update(cx, |this, cx| {
                                this.load_settings(cx);
                            });
                        }
                    });
                })
                .detach();
                return;
            }

        // Fallback: for non-email jobs, just mark for daemon pickup
        if let Ok(db) = self.db.lock()
            && let Ok(jobs) = db.list_jobs()
                && let Some(mut job) = jobs.into_iter().find(|j| j.id == Some(db_id)) {
                    job.next_run_at = Some(Utc::now().to_rfc3339());
                    job.status = "Queued".to_string();
                    job.retries = 0;
                    let _ = db.save_job(job);
                }
        self.load_settings(cx);
    }

    fn cancel_system_job(&mut self, db_id: i64, cx: &mut Context<'_, Self>) {
        if let Ok(db) = self.db.lock()
            && let Ok(summaries) = db.list_all_queued_submissions()
                && let Some(sum) = summaries.into_iter().find(|s| s.id == db_id)
                    && sum.form_code == "2551Q"
                        && let Ok(Some(mut draft)) =
                            db.get_2551q_draft(&sum.tin, sum.taxable_year, sum.quarter.unwrap_or(0))
                        {
                            draft.status = bir_core::forms::form_2551q::FilingStatus::Draft;
                            draft.submitted_at = None;
                            draft.confirmed_at = None;
                            draft.receipt_id = None;
                            draft.submission_filename = None;
                            let _ = db.save_2551q_draft(&draft);
                        }
        self.load_settings(cx);
    }
}

impl Render for CronTasksView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let bg = cx.theme().background;
        let border = cx.theme().border;

        let selected_filter = self.filter_combobox.read(cx).selected_value(cx);
        let search_query = self.search_input.read(cx).value().to_lowercase();

        let filtered_jobs: Vec<&JobViewModel> = self
            .jobs
            .iter()
            .filter(|j| match selected_filter.as_str() {
                "All Jobs" => true,
                "Queued" => j.status == "Queued",
                "Failed" => j.status == "Failed",
                "Archived" => j.status == "Archived",
                _ => true,
            })
            .filter(|j| {
                if search_query.is_empty() {
                    return true;
                }
                j.name.to_lowercase().contains(&search_query)
                    || j.job_type.to_lowercase().contains(&search_query)
            })
            .collect();

        let total_pages = std::cmp::max(
            1,
            filtered_jobs.len().div_ceil(self.items_per_page),
        );
        let start = (self.current_page - 1) * self.items_per_page;
        let mut end = start + self.items_per_page;
        if end > filtered_jobs.len() {
            end = filtered_jobs.len();
        }

        let paged_jobs = if start < filtered_jobs.len() {
            filtered_jobs[start..end].to_vec()
        } else {
            Vec::new()
        };

        div()
            .id("cron_tasks_scroll")
            .flex()
            .flex_col()
            .p_8()
            .gap_6()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_2xl().font_weight(FontWeight::BOLD).child("Background Tasks & Job Queue"))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("Manage system daemon jobs, retry queues, and custom OS commands."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_xl()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_start()
                            .p_6()
                            .gap_4()
                            .border_b_1()
                            .border_color(border)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Automated Form Submission & Email Tracking"))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Continuously retries queued forms and tracks BIR confirmation receipts."),
                                    ),
                            )
                            .child(
                                gpui_component::switch::Switch::new("cron_toggle")
                                    .checked(self.background_cron_enabled)
                                    .on_click(cx.listener(|this, v, _, cx| {
                                        this.toggle_cron(*v, cx);
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_start()
                                    .pt_4()
                                    .gap_4()
                                    .border_t_1()
                                    .border_color(border)
                                    .w_full()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Error Telemetry Reporting"))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Allows sending automated error logs to support for failed jobs."),
                                            ),
                                    )
                                    .child(
                                        gpui_component::switch::Switch::new("telemetry_toggle")
                                            .checked(self.error_telemetry_enabled)
                                            .on_click(cx.listener(|this, v, _, cx| {
                                                this.toggle_telemetry(*v, cx);
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_start()
                                    .pt_4()
                                    .gap_4()
                                    .border_t_1()
                                    .border_color(border)
                                    .w_full()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().font_weight(FontWeight::SEMIBOLD).child(
                                                if self.daemon_running { "Job Queue: On" } else { "Job Queue: Off" }
                                            ))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Controls the background daemon process. Turn off before installing app updates."),
                                            ),
                                    )
                                    .child(
                                        gpui_component::switch::Switch::new("daemon_toggle")
                                            .checked(self.daemon_running)
                                            .on_click(cx.listener(|this, v, _, cx| {
                                                this.toggle_daemon(*v, cx);
                                            })),
                                    ),
                            )

                    )
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .gap_4()
                    .mt_4()
                    .when(std::env::var("DEVELOPER_MODE").unwrap_or_default() == "true", |this| {
                        this.child(div().text_xl().font_weight(FontWeight::BOLD).child("Custom Job Builder"))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_4()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .flex_wrap()
                                        .gap_2()
                                        .items_center()
                                        .child(div().flex_1().min_w(px(200.)).child(Input::new(&self.new_job_name)))
                                        .child(div().flex_1().min_w(px(200.)).child(Input::new(&self.cron_amount)))
                                        .child(div().flex_1().min_w(px(200.)).child(Combobox::new(&self.cron_period)))
                                        .child(div().flex_1().min_w(px(200.)).child(Input::new(&self.new_job_command)))
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            gpui_component::button::Button::new("test_cmd_btn")
                                                .label("Test Cmd")
                                                .on_click(cx.listener(|this, _ev, window, cx| {
                                                    this.test_command(window, cx);
                                                }))
                                        )
                                        .child(
                                            gpui_component::button::Button::new("add_job_btn")
                                                .label("Add Job")
                                                .on_click(cx.listener(|this, _ev, window, cx| {
                                                    this.add_job(window, cx);
                                                }))
                                        )
                                )
                        )
                        .when_some(self.test_output.clone(), |this, out| {
                            this.child(
                                div()
                                    .p_2()
                                    .bg(cx.theme().muted)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_md()
                                    .text_sm()
                                    .child(out)
                            )
                        })
                    })
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .flex_shrink_0()
                    .justify_between()
                    .items_center()
                    .mt_6()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .gap_4()
                            .items_center()
                            .child(div().w_48().child(Combobox::new(&self.filter_combobox)))
                            .child(div().flex_1().min_w(px(200.)).child(Input::new(&self.search_input)))
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_4()
                            .items_center()
                            .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Archive Days: 30"))
                            .child(
                                gpui_component::button::Button::new("purge_archived")
                                    .label("Purge Archives")
                                    .small()
                                    .on_click(cx.listener(|this, _ev, window, cx| {
                                        this.purge_archived(window, cx);
                                    }))
                            )
                    )
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(paged_jobs.into_iter().enumerate().map(|(i, job)| {
                        let _id = job.id.unwrap_or(0);
                        let db_id = job.id.unwrap_or(0);
                        let is_system = job.is_system;
                        let status_color = match job.status.as_str() {
                            "Running" => cx.theme().info,
                            "Failed" => cx.theme().danger,
                            "Done" => cx.theme().success,
                            "Archived" => cx.theme().muted_foreground,
                            _ => cx.theme().primary,
                        };
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .justify_between()
                            .items_center()
                            .p_4()
                            .gap_4()
                            .bg(cx.theme().muted)
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_lg()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .flex_1()
                                    .min_w(px(250.))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .items_start()
                                            .child(div().w_full().font_weight(FontWeight::BOLD).text_color(cx.theme().foreground).child(job.name.clone()))
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(if is_system { cx.theme().info.opacity(0.2) } else { cx.theme().secondary })
                                                    .text_color(if is_system { cx.theme().info } else { cx.theme().foreground })
                                                    .text_xs()
                                                    .child(job.job_type.clone())
                                            )
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(if is_system {
                                                "Managed by System Daemon (Automatic Backoff)".to_string()
                                            } else {
                                                let cron_str = job.cron_expr.clone().unwrap_or_default();
                                                if cron_str.is_empty() {
                                                    "One-off".to_string()
                                                } else {
                                                    humanize_cron(&cron_str)
                                                }
                                            })
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap_2()
                                            .text_xs()
                                            .child(
                                                div().text_color(status_color).font_weight(FontWeight::BOLD).child(format!("Status: {}", job.status))
                                            )
                                            .child(
                                                div().text_color(cx.theme().muted_foreground).child(format!("| Retries: {}", job.retries))
                                            )
                                            .when_some(job.next_run_at.clone(), |this, time| {
                                                this.child(div().text_color(cx.theme().muted_foreground).child(format!("| Next Run: {}", time)))
                                            })
                                    )
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .flex_wrap()
                                    .gap_2()
                                    .when(is_system, |this| {
                                        this.child(
                                            gpui_component::button::Button::new(format!("cancel_system_job_{}", i))
                                                .label("Cancel Submission")
                                                .small()
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    this.cancel_system_job(db_id, cx);
                                                }))
                                        )
                                    })
                                    .when(!is_system && job.status != "Archived", |this| {
                                        this.child(
                                            gpui_component::button::Button::new(format!("run_job_{}", i))
                                                .label("Run Now")
                                                .small()
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    this.run_job_now(db_id, cx);
                                                }))
                                        )
                                        .child(
                                            gpui_component::button::Button::new(format!("archive_job_{}", i))
                                                .label("Archive")
                                                .small()
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    this.archive_job(db_id, cx);
                                                }))
                                        )
                                        .child(
                                            gpui_component::button::Button::new(format!("delete_job_{}", i))
                                                .label("Delete")
                                                .small()
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    this.delete_job(db_id, cx);
                                                }))
                                        )
                                    })
                                    .when(job.status == "Archived", |this| {
                                        this.child(
                                            gpui_component::button::Button::new(format!("delete_job_{}", i))
                                                .label("Delete")
                                                .small()
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    this.delete_job(db_id, cx);
                                                }))
                                        )
                                    })
                                    .when(job.status == "Failed" && self.error_telemetry_enabled, |this| {
                                        let log = job.output_log.clone().unwrap_or_default();
                                        let jname = job.name.clone();
                                        this.child(
                                            gpui_component::button::Button::new(format!("email_support_{}", i))
                                                .label("Email Support")
                                                .small()
                                                .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                                    let subject = format!("Job Error: {}", jname).replace(" ", "%20");
                                                    // Truncate body if it's too long for a mailto link
                                                    let mut body = log.replace("\n", "%0A").replace(" ", "%20");
                                                    if body.len() > 1000 {
                                                        body = body.chars().take(1000).collect::<String>();
                                                        body.push_str("...[truncated]");
                                                    }
                                                    let url = format!("mailto:codeitlikemiley@gmail.com?subject={}&body={}", subject, body);
                                                    cx.open_url(&url);
                                                }))
                                        )
                                    })
                                    .when(std::env::var("DEVELOPER_MODE").unwrap_or_default() == "true" && job.output_log.is_some(), |this| {
                                        let log = job.output_log.clone().unwrap_or_default();
                                        let jname = job.name.clone();
                                        this.child(
                                            gpui_component::button::Button::new(format!("view_log_{}", i))
                                                .label("👁")
                                                .small()
                                                .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                                    let log = log.clone();
                                                    let jname = jname.clone();
                                                    let options = WindowOptions {
                                                        titlebar: Some(TitlebarOptions {
                                                            title: None,
                                                            appears_transparent: true,
                                                            traffic_light_position: Some(point(px(9.0), px(9.0))),
                                                        }),
                                                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                                                            origin: point(px(100.), px(100.)),
                                                            size: size(px(800.), px(600.)),
                                                        })),
                                                        ..Default::default()
                                                    };
                                                    let _ = cx.open_window(options, move |_window, cx| {
                                                        cx.new(|_cx| crate::views::debug_log_viewer::DebugLogViewerView::new(jname.clone(), log.clone()))
                                                    });
                                                }))
                                        )
                                    })
                            )
                    }))
            )
            .child(
                div().flex().justify_center().pt_4().pb_8().when(total_pages > 1, |this| {
                    this.child(
                        gpui_component::pagination::Pagination::new("job-pagination")
                            .current_page(self.current_page)
                            .total_pages(total_pages)
                            .on_click(cx.listener(|this, page, _window, cx| {
                                this.current_page = *page;
                                cx.notify();
                            }))
                    )
                })
            )
            .into_any_element()
    }
}
