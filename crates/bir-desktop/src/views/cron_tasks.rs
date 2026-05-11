#![allow(dead_code)]
use bir_core::db::{Database, Job};
use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::components::combobox::{Combobox, ComboboxEvent, ComboboxState};
use chrono::Utc;
use gpui_component::input::{Input, InputState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

// Daemon checking moved out of UI.

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
        && stripped.ends_with(" * * * * *")
    {
        let num = stripped.trim_end_matches(" * * * * *");
        return format!("Every {} seconds", num);
    }
    if let Some(stripped) = expr.strip_prefix("0 */")
        && stripped.ends_with(" * * * *")
    {
        let num = stripped.trim_end_matches(" * * * *");
        if num == "1" {
            return "Every minute".to_string();
        }
        return format!("Every {} minutes", num);
    }
    if let Some(stripped) = expr.strip_prefix("0 0 */")
        && stripped.ends_with(" * * *")
    {
        let num = stripped.trim_end_matches(" * * *");
        if num == "1" {
            return "Every hour".to_string();
        }
        return format!("Every {} hours", num);
    }
    if let Some(stripped) = expr.strip_prefix("0 0 0 */")
        && stripped.ends_with(" * *")
    {
        let num = stripped.trim_end_matches(" * *");
        if num == "1" {
            return "Everyday".to_string();
        }
        return format!("Every {} days", num);
    }
    if let Some(stripped) = expr.strip_prefix("0 0 0 1 */")
        && stripped.ends_with(" *")
    {
        let num = stripped.trim_end_matches(" *");
        if num == "1" {
            return "Every month".to_string();
        }
        return format!("Every {} months", num);
    }
    // Fallback
    format!("Frequency: {}", expr)
}

#[derive(Clone, Copy, PartialEq)]
pub enum JobKind {
    Submission,
    EmailPoll,
}

#[derive(Clone)]
pub struct JobViewModel {
    pub job_kind: JobKind,
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

#[derive(Clone, Copy, PartialEq)]
pub enum BackgroundTaskTab {
    Jobs,
    Logs,
}

pub struct CronTasksView {
    db: Arc<Mutex<Database>>,
    jobs: Vec<JobViewModel>,
    filter_combobox: Entity<ComboboxState>,
    search_input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
    current_page: usize,
    items_per_page: usize,
    active_tab: BackgroundTaskTab,
    log_content: String,
    logs_scroll_handle: ScrollHandle,
    log_filter_combobox: Entity<ComboboxState>,
}

impl CronTasksView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let filter_combobox = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "All Jobs".into(),
                    "Queued".into(),
                    "Done".into(),
                    "Failed".into(),
                    "Archived".into(),
                ],
                window,
                cx,
            )
        });
        filter_combobox.update(cx, |s, cx| s.set_selected_value("Queued", window, cx));

        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search job name or type..."));

        let log_filter_combobox = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "All Logs".into(),
                    "Error".into(),
                    "Warn".into(),
                    "Info".into(),
                ],
                window,
                cx,
            )
        });
        log_filter_combobox.update(cx, |s, cx| s.set_selected_value("All Logs", window, cx));

        let mut view = Self {
            db,
            jobs: Vec::new(),
            filter_combobox: filter_combobox.clone(),
            search_input: search_input.clone(),
            _subscriptions: Vec::new(),
            current_page: 1,
            items_per_page: 5,
            active_tab: BackgroundTaskTab::Jobs,
            log_content: String::new(),
            logs_scroll_handle: ScrollHandle::new(),
            log_filter_combobox: log_filter_combobox.clone(),
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

        let sub3 = cx.subscribe_in(
            &log_filter_combobox,
            window,
            |_this: &mut Self, _entity, _event: &ComboboxEvent, _window, cx| {
                cx.notify();
            },
        );
        view._subscriptions.push(sub3);

        let bus = cx.global::<crate::events::GlobalEventBus>().0.clone();
        cx.subscribe(
            &bus,
            |_this: &mut Self, _bus, event: &crate::events::AppEvent, cx| match event {
                crate::events::AppEvent::DatabaseChanged => {
                    _this.load_settings(cx);
                }
            },
        )
        .detach();

        let weak_view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(60))
                    .await;
                let result = cx.update(|cx| {
                    if let Some(view) = weak_view.upgrade() {
                        view.update(cx, |this, cx| {
                            this.load_settings(cx);
                        });
                        true
                    } else {
                        false
                    }
                });

                if result {
                    // updated successfully
                } else {
                    break;
                }
            }
        })
        .detach();

        view.load_settings(cx);
        view
    }

    pub fn load_settings(&mut self, cx: &mut Context<'_, Self>) {
        let mut view_jobs = Vec::new();
        if let Ok(db) = self.db.lock() {
            if let Ok(jobs) = db.list_jobs() {
                for job in jobs {
                    let mut display_name = job.name.clone();
                    if display_name.starts_with("Poll Receipts: ") {
                        let email = display_name.trim_start_matches("Poll Receipts: ");
                        display_name =
                            format!("Waiting for 2551Q confirmation email for {}", email);
                    }
                    view_jobs.push(JobViewModel {
                        job_kind: JobKind::EmailPoll,
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
                        job_kind: JobKind::Submission,
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

    fn delete_job(&mut self, id: i64, cx: &mut Context<'_, Self>) {
        if let Ok(db) = self.db.lock() {
            let _ = db.delete_job(id);
        }
        self.load_settings(cx);
    }

    fn archive_job(&mut self, id: i64, cx: &mut Context<'_, Self>) {
        if let Ok(db) = self.db.lock()
            && let Ok(jobs) = db.list_jobs()
            && let Some(mut job) = jobs.into_iter().find(|j| j.id == Some(id))
        {
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

        let Some(job) = job else {
            return;
        };

        // For email polling jobs, execute inline instead of waiting for daemon
        if let Some(ref cmd) = job.command
            && cmd.starts_with("bir_poll_email ")
        {
            let email = cmd.trim_start_matches("bir_poll_email ").trim().to_string();
            let db = self.db.clone();
            let job_id = db_id;

            // Mark as Running
            if let Ok(db_guard) = db.lock()
                && let Ok(jobs) = db_guard.list_jobs()
                && let Some(mut j) = jobs.into_iter().find(|j| j.id == Some(job_id))
            {
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
                            && let Some(mut j) = jobs.into_iter().find(|j| j.id == Some(job_id))
                        {
                            j.last_run_at = Some(Utc::now().to_rfc3339());
                            if poll_success {
                                j.output_log =
                                    Some("Email polling completed successfully.".to_string());
                                j.retries = 0;
                                if !still_pending {
                                    j.status = "Archived".to_string();
                                } else {
                                    j.status = "Queued".to_string();
                                    // Set next run from cron
                                    if let Some(ref expr) = j.cron_expr
                                        && let Ok(schedule) = std::str::FromStr::from_str(expr)
                                    {
                                        let schedule: cron::Schedule = schedule;
                                        if let Some(next) = schedule.upcoming(chrono::Utc).next() {
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
                                    && let Ok(schedule) = std::str::FromStr::from_str(expr)
                                {
                                    let schedule: cron::Schedule = schedule;
                                    if let Some(next) = schedule.upcoming(chrono::Utc).next() {
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
            && let Some(mut job) = jobs.into_iter().find(|j| j.id == Some(db_id))
        {
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
        {
            if sum.form_code == "2551Q" {
                if let Ok(Some(mut draft)) =
                    db.get_2551q_draft(&sum.tin, sum.taxable_year, sum.quarter.unwrap_or(0))
                    && !matches!(draft.status, bir_core::forms::FilingStatus::Paid)
                {
                    draft.revert_to_draft();
                    let _ = db.save_2551q_draft(&draft);
                }
            } else if sum.form_code == "1601C"
                && let Ok(Some(mut draft)) =
                    db.get_1601c_draft(&sum.tin, sum.taxable_year, sum.month.unwrap_or(0))
                && !matches!(draft.status, bir_core::forms::FilingStatus::Paid)
            {
                draft.revert_to_draft();
                let _ = db.save_form_draft(
                    &draft.tin,
                    "1601C",
                    draft.taxable_year,
                    Some(draft.month),
                    &draft.status,
                    &draft,
                );
            }
        }
        self.load_settings(cx);
    }

    fn refresh_logs(&mut self, cx: &mut Context<'_, Self>) {
        let logs_path = bir_core::platform::data_dir().join("logs/ebirforms.log");
        if let Ok(content) = std::fs::read_to_string(&logs_path) {
            self.log_content = content;
        } else {
            self.log_content = "Failed to load logs or logs are empty.".to_string();
        }
        cx.notify();
    }

    fn clear_logs(&mut self, cx: &mut Context<'_, Self>) {
        let logs_path = bir_core::platform::data_dir().join("logs/ebirforms.log");
        let _ = std::fs::write(&logs_path, "");
        self.log_content.clear();
        cx.notify();
    }

    fn email_support(&mut self, cx: &mut Context<'_, Self>) {
        let _ = open::that(
            "mailto:support@goldcoders.dev?subject=eBIRForms%20Bug%20Report&body=Please%20attach%20the%20log%20file.",
        );
        let logs_dir = bir_core::platform::data_dir().join("logs");
        let _ = open::that(&logs_dir);
        cx.notify();
    }

    fn export_error_logs(&mut self, cx: &mut Context<'_, Self>) {
        let logs_path = bir_core::platform::data_dir().join("logs/ebirforms.log");
        let mut error_logs = String::new();
        if let Ok(content) = std::fs::read_to_string(&logs_path) {
            for line in content.lines() {
                if line.contains(" ERROR ") || line.contains(" WARN ") || line.contains(" FATAL ") {
                    error_logs.push_str(line);
                    error_logs.push('\n');
                }
            }
        }

        if error_logs.is_empty() {
            error_logs = "No error logs found.".to_string();
        }

        cx.spawn(async move |_, cx| {
            if let Some(target_handle) = rfd::AsyncFileDialog::new()
                .set_title("Save Error Logs")
                .set_file_name("ebirforms_error_logs.txt")
                .add_filter("Text File", &["txt"])
                .save_file()
                .await
            {
                let path = target_handle.path().to_path_buf();
                let _ = std::fs::write(&path, error_logs);
                let _ = rfd::AsyncMessageDialog::new()
                    .set_title("Export Complete")
                    .set_description(format!("Error logs exported to {}", path.display()))
                    .show()
                    .await;
            }
        })
        .detach();
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
                "Done" => {
                    j.status == "Submitted"
                        || j.status == "Confirmed"
                        || j.status == "Paid"
                        || j.status == "Done"
                }
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

        let total_pages = std::cmp::max(1, filtered_jobs.len().div_ceil(self.items_per_page));
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

        let is_jobs = self.active_tab == BackgroundTaskTab::Jobs;
        let is_logs = self.active_tab == BackgroundTaskTab::Logs;

        let active_tab_bg = cx.theme().secondary;
        let inactive_tab_bg = gpui::rgba(0x00000000).into();

        let jobs_view = div()
            .flex()
            .flex_col()
            .gap_6()
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
                        let is_submission = job.job_kind == JobKind::Submission;
                        let is_queued_submission = is_submission && job.status == "Queued";
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
                                                    .bg(if is_submission { cx.theme().info.opacity(0.2) } else { cx.theme().secondary })
                                                    .text_color(if is_submission { cx.theme().info } else { cx.theme().foreground })
                                                    .text_xs()
                                                    .child(job.job_type.clone())
                                            )
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(if is_submission {
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
                                                this.child(div().text_color(cx.theme().muted_foreground).child(format!("| Next run: {}", bir_core::time_utils::format_next_run(&time))))
                                            })
                                    )
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .flex_wrap()
                                    .gap_2()
                                    .when(is_queued_submission, |this| {
                                        this.child(
                                            gpui_component::button::Button::new(format!("cancel_system_job_{}", i))
                                                .label("Cancel Submission")
                                                .small()
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    this.cancel_system_job(db_id, cx);
                                                }))
                                        )
                                    })
                                    .when(job.job_kind == JobKind::EmailPoll && job.status != "Archived", |this| {
                                        this.child(
                                            gpui_component::button::Button::new(format!("run_job_{}", i))
                                                .label("Run Now")
                                                .small()
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    this.run_job_now(db_id, cx);
                                                }))
                                        )
                                    })
                                    .when(job.job_kind == JobKind::EmailPoll && job.status != "Archived", |this| {
                                        this.child(
                                            gpui_component::button::Button::new(format!("archive_job_{}", i))
                                                .label("Archive")
                                                .small()
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    this.archive_job(db_id, cx);
                                                }))
                                        )
                                    })
                                    .when(job.job_kind == JobKind::EmailPoll, |this| {
                                        this.child(
                                            gpui_component::button::Button::new(format!("delete_job_{}", i))
                                                .label("Delete")
                                                .small()
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    this.delete_job(db_id, cx);
                                                }))
                                        )
                                    })
                                    .when(job.status == "Failed", |this| {
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
                                    .when(job.output_log.is_some(), |this| {
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
            );

        let selected_log_filter = self.log_filter_combobox.read(cx).selected_value(cx);

        let parsed_logs = self
            .log_content
            .lines()
            .filter(|line| {
                if selected_log_filter == "All Logs" {
                    true
                } else if selected_log_filter == "Error" {
                    line.contains(" ERROR ") || line.contains(" FATAL ")
                } else if selected_log_filter == "Warn" {
                    line.contains(" WARN ")
                } else if selected_log_filter == "Info" {
                    line.contains(" INFO ")
                } else {
                    true
                }
            })
            .map(|line| {
                let color = if line.contains(" ERROR ") || line.contains(" FATAL ") {
                    cx.theme().danger
                } else if line.contains(" WARN ") {
                    cx.theme().primary // Fallback for warning color
                } else if line.contains(" INFO ") {
                    cx.theme().info
                } else {
                    cx.theme().muted_foreground
                };
                div()
                    .font_family(crate::platform::MONOSPACE_FONT)
                    .text_sm()
                    .text_color(color)
                    .child(line.to_string())
            });

        let logs_view = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .w_full()
                    .mt_6()
                    .pb_4()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .items_center()
                            .child(
                                gpui_component::button::Button::new("refresh_logs")
                                    .label("Refresh Logs")
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.refresh_logs(cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("clear_logs")
                                    .label("Clear Logs")
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.clear_logs(cx);
                                    })),
                            )
                            .child(div().w_48().child(Combobox::new(&self.log_filter_combobox))),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(
                                gpui_component::button::Button::new("export_error_logs")
                                    .label("Export Error Logs")
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.export_error_logs(cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("email_support_global")
                                    .label("Email Support")
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.email_support(cx);
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .id("logs_view_content")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .p_4()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(border)
                    .rounded_md()
                    .overflow_y_scroll()
                    .track_scroll(&self.logs_scroll_handle)
                    .children(parsed_logs),
            );

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p_8()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child("Background Tasks & Job Queue"),
                            )
                            .child(div().text_color(cx.theme().muted_foreground).child(
                                "Manage system daemon jobs, retry queues, and application logs.",
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .w_full()
                            .gap_2()
                            .border_b_1()
                            .border_color(border)
                            .child(
                                div()
                                    .id("jobs_tab")
                                    .px_4()
                                    .py_2()
                                    .cursor_pointer()
                                    .bg(if is_jobs {
                                        active_tab_bg
                                    } else {
                                        inactive_tab_bg
                                    })
                                    .border_b_2()
                                    .border_color(if is_jobs {
                                        cx.theme().primary
                                    } else {
                                        gpui::rgba(0x00000000).into()
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.active_tab = BackgroundTaskTab::Jobs;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if is_jobs {
                                                cx.theme().foreground
                                            } else {
                                                cx.theme().muted_foreground
                                            })
                                            .child("Jobs"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("logs_tab")
                                    .px_4()
                                    .py_2()
                                    .cursor_pointer()
                                    .bg(if is_logs {
                                        active_tab_bg
                                    } else {
                                        inactive_tab_bg
                                    })
                                    .border_b_2()
                                    .border_color(if is_logs {
                                        cx.theme().primary
                                    } else {
                                        gpui::rgba(0x00000000).into()
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.active_tab = BackgroundTaskTab::Logs;
                                        this.refresh_logs(cx);
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if is_logs {
                                                cx.theme().foreground
                                            } else {
                                                cx.theme().muted_foreground
                                            })
                                            .child("Logs"),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .id("cron_tasks_content")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .px_8()
                    .pb_8()
                    .overflow_y_scroll()
                    .child(if is_jobs {
                        jobs_view.into_any_element()
                    } else {
                        logs_view.into_any_element()
                    }),
            )
    }
}
