use bir_core::db::{Database, Job};
use gpui::*;
use gpui::prelude::FluentBuilder;

use gpui_component::*;
use gpui_component::input::{Input, InputState};
use crate::components::combobox::{Combobox, ComboboxState};
use std::sync::{Arc, Mutex};
use chrono::Utc;

pub enum CronTasksEvent {
    Reload,
}

impl EventEmitter<CronTasksEvent> for CronTasksView {}

#[derive(Clone, PartialEq)]
pub enum JobTab {
    All,
    Queued,
    Failed,
    Archived,
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
}

pub struct CronTasksView {
    db: Arc<Mutex<Database>>,
    background_cron_enabled: bool,
    test_notification_enabled: bool,
    has_profile: bool,
    jobs: Vec<JobViewModel>,
    new_job_name: Entity<InputState>,
    cron_amount: Entity<InputState>,
    cron_period: Entity<ComboboxState>,
    new_job_command: Entity<InputState>,
    cleanup_days: Entity<InputState>,
    active_tab: JobTab,
    test_output: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl CronTasksView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let new_job_name = cx.new(|cx| InputState::new(window, cx).placeholder("Job Name (e.g. Sync)"));
        let cron_amount = cx.new(|cx| InputState::new(window, cx).placeholder("Amt (e.g. 5)"));
        let cron_period = cx.new(|cx| ComboboxState::new(vec!["Seconds".into(), "Minutes".into(), "Hours".into(), "Days".into(), "Months".into(), "Raw Cron".into()], window, cx));
        
        // Default to Minutes
        cron_period.update(cx, |s, cx| s.set_selected_value("Minutes", window, cx));
        
        let new_job_command = cx.new(|cx| InputState::new(window, cx).placeholder("Cmd (e.g. osascript -e ...)"));
        
        let cleanup_days = cx.new(|cx| InputState::new(window, cx).placeholder("Days"));
        cleanup_days.update(cx, |s, cx| s.set_value("30".to_string(), window, cx));

        let mut view = Self {
            db,
            background_cron_enabled: false,
            test_notification_enabled: false,
            has_profile: false,
            jobs: Vec::new(),
            new_job_name,
            cron_amount,
            cron_period,
            new_job_command,
            cleanup_days,
            active_tab: JobTab::All,
            test_output: None,
            _subscriptions: Vec::new(),
        };
        
        view.load_settings(cx);
        view
    }

    pub fn load_settings(&mut self, cx: &mut Context<'_, Self>) {
        let mut view_jobs = Vec::new();
        if let Ok(db) = self.db.lock() {
            if let Ok(profiles) = db.list_profiles() {
                if let Some(profile) = profiles.first() {
                    self.has_profile = true;
                    self.background_cron_enabled = profile.background_cron_enabled;
                    self.test_notification_enabled = profile.test_notification_enabled;
                } else {
                    self.has_profile = false;
                }
            }
            if let Ok(jobs) = db.list_jobs() {
                for job in jobs {
                    view_jobs.push(JobViewModel {
                        is_system: false,
                        id: job.id,
                        name: job.name,
                        job_type: job.job_type,
                        cron_expr: job.cron_expr,
                        command: job.command,
                        status: job.status,
                        retries: job.retries,
                        last_run_at: job.last_run_at,
                        next_run_at: job.next_run_at,
                        created_at: job.created_at,
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

    fn toggle_test_notification(&mut self, value: bool, cx: &mut Context<'_, Self>) {
        self.test_notification_enabled = value;
        self.save_to_db(cx);
    }

    fn save_to_db(&mut self, cx: &mut Context<'_, Self>) {
        if let Ok(db) = self.db.lock() {
            if let Ok(mut profiles) = db.list_profiles() {
                if let Some(mut profile) = profiles.pop() {
                    profile.background_cron_enabled = self.background_cron_enabled;
                    profile.test_notification_enabled = self.test_notification_enabled;
                    let _ = db.save_profile(profile);
                }

                let should_run = self.background_cron_enabled || self.test_notification_enabled;
                if should_run {
                    bir_core::daemon_installer::install();
                } else {
                    bir_core::daemon_installer::uninstall();
                }
            }
        }
        cx.notify();
    }

    fn build_cron_string(&self, amount: &str, period: &str) -> Result<String, String> {
        if period == "Raw Cron" {
            return Ok(amount.trim().to_string());
        }
        let amount_num = amount.parse::<u32>().map_err(|_| "Amount must be a number".to_string())?;
        if amount_num < 1 { return Err("Amount must be >= 1".to_string()); }

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
        cx.spawn(async move |this, mut cx| {
            let output = match tokio::process::Command::new("sh").arg("-c").arg(&cmd).output().await {
                Ok(out) => {
                    let mut res = String::from_utf8_lossy(&out.stdout).to_string();
                    if !out.status.success() {
                        res.push_str(&format!("\nError: {}", String::from_utf8_lossy(&out.stderr)));
                    }
                    if res.trim().is_empty() {
                        "Success (no output)".to_string()
                    } else {
                        res
                    }
                }
                Err(e) => format!("Failed to execute: {}", e),
            };
            let _ = cx.update(|cx| {
                if let Some(this) = this.upgrade() {
                    this.update(cx, |this, cx| {
                        this.test_output = Some(output);
                        cx.notify();
                    });
                }
            });
        }).detach();
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

        let command = if cmd_str.trim().is_empty() { None } else { Some(cmd_str.trim().to_string()) };

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
        };

        if let Ok(db) = self.db.lock() {
            let _ = db.save_job(job);
        }

        self.new_job_name.update(cx, |s, cx| s.set_value("", window, cx));
        self.cron_amount.update(cx, |s, cx| s.set_value("", window, cx));
        self.new_job_command.update(cx, |s, cx| s.set_value("", window, cx));
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
        if let Ok(db) = self.db.lock() {
            if let Ok(jobs) = db.list_jobs() {
                if let Some(mut job) = jobs.into_iter().find(|j| j.id == Some(id)) {
                    job.status = "Archived".to_string();
                    let _ = db.save_job(job);
                }
            }
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
        if let Ok(db) = self.db.lock() {
            if let Ok(jobs) = db.list_jobs() {
                if let Some(mut job) = jobs.into_iter().find(|j| j.id == Some(db_id)) {
                    job.next_run_at = Some(Utc::now().to_rfc3339());
                    job.status = "Queued".to_string();
                    job.retries = 0;
                    let _ = db.save_job(job);
                }
            }
        }
        self.load_settings(cx);
    }
}

impl Render for CronTasksView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if !self.has_profile {
            return div()
                .flex()
                .justify_center()
                .items_center()
                .h_full()
                .child("Please create a Taxpayer Profile first.");
        }

        let bg = cx.theme().background;
        let border = cx.theme().border;

        let filtered_jobs: Vec<&JobViewModel> = self.jobs.iter().filter(|j| {
            match self.active_tab {
                JobTab::All => true,
                JobTab::Queued => j.status == "Queued",
                JobTab::Failed => j.status == "Failed",
                JobTab::Archived => j.status == "Archived",
            }
        }).collect();

        div()
            .flex()
            .flex_col()
            .p_8()
            .gap_6()
            .overflow_y_hidden()
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
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_xl()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .p_6()
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
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .p_6()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Test OS Notification Ping"))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Sends a native 'Hello' notification every minute while active, even if app is closed."),
                                    ),
                            )
                            .child(
                                gpui_component::switch::Switch::new("test_ping_toggle")
                                    .checked(self.test_notification_enabled)
                                    .on_click(cx.listener(|this, v, _, cx| {
                                        this.toggle_test_notification(*v, cx);
                                    })),
                            ),
                    )
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .mt_4()
                    .child(div().text_xl().font_weight(FontWeight::BOLD).child("Custom Job Builder"))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(div().w(px(200.)).child(Input::new(&self.new_job_name)))
                            .child(div().w(px(100.)).child(Input::new(&self.cron_amount)))
                            .child(div().w(px(150.)).child(Combobox::new(&self.cron_period)))
                            .child(div().w(px(250.)).child(Input::new(&self.new_job_command)))
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
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .mt_6()
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(
                                div()
                                    .id("tab_all")
                                    .cursor_pointer()
                                    .font_weight(if self.active_tab == JobTab::All { FontWeight::BOLD } else { FontWeight::NORMAL })
                                    .text_color(if self.active_tab == JobTab::All { cx.theme().foreground } else { cx.theme().muted_foreground })
                                    .child("All Jobs")
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.active_tab = JobTab::All;
                                        cx.notify();
                                    }))
                            )
                            .child(
                                div()
                                    .id("tab_queued")
                                    .cursor_pointer()
                                    .font_weight(if self.active_tab == JobTab::Queued { FontWeight::BOLD } else { FontWeight::NORMAL })
                                    .text_color(if self.active_tab == JobTab::Queued { cx.theme().foreground } else { cx.theme().muted_foreground })
                                    .child("Queued")
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.active_tab = JobTab::Queued;
                                        cx.notify();
                                    }))
                            )
                            .child(
                                div()
                                    .id("tab_failed")
                                    .cursor_pointer()
                                    .font_weight(if self.active_tab == JobTab::Failed { FontWeight::BOLD } else { FontWeight::NORMAL })
                                    .text_color(if self.active_tab == JobTab::Failed { cx.theme().foreground } else { cx.theme().muted_foreground })
                                    .child("Failed")
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.active_tab = JobTab::Failed;
                                        cx.notify();
                                    }))
                            )
                            .child(
                                div()
                                    .id("tab_archived")
                                    .cursor_pointer()
                                    .font_weight(if self.active_tab == JobTab::Archived { FontWeight::BOLD } else { FontWeight::NORMAL })
                                    .text_color(if self.active_tab == JobTab::Archived { cx.theme().foreground } else { cx.theme().muted_foreground })
                                    .child("Archived")
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.active_tab = JobTab::Archived;
                                        cx.notify();
                                    }))
                            )
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Archive limit (Days):"))
                            .child(div().w(px(50.)).child(Input::new(&self.cleanup_days)))
                            .child(
                                gpui_component::button::Button::new("purge_archived")
                                    .label("Purge Archived")
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
                    .children(filtered_jobs.into_iter().enumerate().map(|(i, job)| {
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
                            .justify_between()
                            .items_center()
                            .p_4()
                            .bg(cx.theme().muted)
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_lg()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .items_center()
                                            .child(div().font_weight(FontWeight::BOLD).text_color(cx.theme().foreground).child(job.name.clone()))
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
                                                format!("Cron: {} | Cmd: {}", job.cron_expr.clone().unwrap_or_else(|| "One-off".to_string()), job.command.clone().unwrap_or_else(|| "None".to_string()))
                                            })
                                    )
                                    .child(
                                        div()
                                            .flex()
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
                                    .gap_2()
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
                            )
                    }))
            )
    }
}
