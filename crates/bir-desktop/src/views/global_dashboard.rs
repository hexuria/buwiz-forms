use bir_core::db::{BirNotice, Database, TaxDeadline};
use bir_core::forms::FormDraftSummary;
use bir_core::profile::TaxpayerProfile;
use chrono::{Datelike, Local};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::*;
use std::sync::{Arc, Mutex};

pub enum GlobalDashboardEvent {
    OpenForm {
        tin: String,
        form_code: String,
        year: u16,
        quarter: u8,
    },
    CheckStatus {
        tin: String,
    },
    /// Notification to be displayed to the user (level, title, message).
    PushNotification(String, String, String),
    /// Signal that a form status has changed and the dashboard should refresh.
    StatusChanged,
}

pub struct GlobalDashboardView {
    db: Arc<Mutex<Database>>,
    profiles: Vec<TaxpayerProfile>,
    deadlines: Vec<TaxDeadline>,
    announcements: Vec<BirNotice>,
    actionable_forms: Vec<(String, FormDraftSummary)>,
    is_fetching_news: bool,
    compliance_calendar: Entity<crate::components::compliance_calendar::ComplianceCalendar>,
}

impl EventEmitter<GlobalDashboardEvent> for GlobalDashboardView {}

impl GlobalDashboardView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let (profiles, deadlines, announcements, actionable_forms) = if let Ok(db_lock) = db.lock()
        {
            let profiles = db_lock.list_profiles().unwrap_or_default();
            let mut deadlines = db_lock.list_tax_deadlines().unwrap_or_default();
            let announcements = db_lock.list_bir_notices().unwrap_or_default();

            let mut actionable_forms = Vec::new();
            let current_year = chrono::Local::now().date_naive().year() as u16;
            for p in &profiles {
                if let Ok(summaries) = db_lock.list_draft_summaries(&p.tin.full(), current_year) {
                    for sum in summaries {
                        actionable_forms.push((p.full_name.clone(), sum));
                    }
                }
            }

            if deadlines.is_empty() {
                let mock_deadlines = vec![
                    TaxDeadline {
                        id: None,
                        form_type: "2551Q".into(),
                        due_date: "2026-04-25".into(),
                        description: "Q1 Percentage Tax".into(),
                    },
                    TaxDeadline {
                        id: None,
                        form_type: "1701Q".into(),
                        due_date: "2026-05-15".into(),
                        description: "Q1 Income Tax".into(),
                    },
                    TaxDeadline {
                        id: None,
                        form_type: "2550M".into(),
                        due_date: "2026-05-20".into(),
                        description: "April VAT".into(),
                    },
                ];
                for d in mock_deadlines.clone() {
                    let _ = db_lock.save_tax_deadline(&d);
                }
                deadlines = mock_deadlines;
            }

            (profiles, deadlines, announcements, actionable_forms)
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };

        let compliance_calendar = cx
            .new(|cx| crate::components::compliance_calendar::ComplianceCalendar::new(window, cx));

        let mut view = Self {
            db,
            profiles,
            deadlines,
            announcements,
            actionable_forms,
            is_fetching_news: false,
            compliance_calendar,
        };

        let bus = cx.global::<crate::events::GlobalEventBus>().0.clone();
        cx.subscribe(
            &bus,
            |this: &mut Self, _bus, event: &crate::events::AppEvent, cx| {
                match event {
                    crate::events::AppEvent::DatabaseChanged => {
                        this.reload_actionable_forms(cx);
                        // Also refresh profiles and other DB-backed info if needed
                    }
                }
            },
        )
        .detach();

        view.refresh_news(cx);
        view
    }

    pub fn refresh_news(&mut self, cx: &mut Context<Self>) {
        if self.is_fetching_news {
            return;
        }
        self.is_fetching_news = true;

        let fetch_db = self.db.clone();
        cx.spawn(async move |view, cx| {
            // Run all blocking reqwest operations on the background thread pool
            cx.background_executor()
                .spawn(async move {
                    let fetcher = bir_core::news_fetcher::NoticeFetcher::new(fetch_db);
                    let _ = fetcher.fetch_and_sync();
                })
                .await;

            // Back on the main thread, reload the announcements
            let _ = view.update(cx, |view, cx| {
                view.is_fetching_news = false;
                if let Ok(db_lock) = view.db.lock() {
                    view.announcements = db_lock.list_bir_notices().unwrap_or_default();
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub fn set_profiles(&mut self, profiles: Vec<TaxpayerProfile>, cx: &mut Context<Self>) {
        self.profiles = profiles;
        // Optionally reload penalties here
        cx.notify();
    }
}

impl Render for GlobalDashboardView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let now = Local::now().date_naive();
        let year = now.year();

        let is_narrow = window.viewport_size().width < px(768.);

        self.compliance_calendar.update(cx, |calendar, _| {
            calendar.set_data(self.deadlines.clone(), self.announcements.clone());
        });

        div()
            .id("global-dashboard")
            .size_full()
            .flex()
            .flex_col()
            .when(is_narrow, |this| this.p_4())
            .when(!is_narrow, |this| this.p_8())
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child("Global Dashboard"),
                    )
                    .child(
                        div()
                            .text_base()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Overview of all taxpayer profiles for {}", year)),
                    ),
            )
            .child(
                div()
                    .id("columns-container")
                    .flex()
                    .flex_wrap()
                    .content_start()
                    .items_start()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap_6()
                    .mt_4()
                    .child(
                        // Left Column
                        div()
                            .id("left-column")
                            .flex_1()
                            .min_w(px(320.))
                            .pr_2()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(self.urgent_actions_section(window, cx))
                            .child(self.compliance_calendar.clone()),
                    )
                    .child(
                        // Right Column
                        div()
                            .flex_1()
                            .min_w(px(320.))
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(self.news_section(cx)),
                    ),
            )
    }
}

impl GlobalDashboardView {
    /// Reload actionable forms from the database (called after email check updates status).
    pub fn reload_actionable_forms(&mut self, cx: &mut Context<Self>) {
        if let Ok(db_lock) = self.db.lock() {
            let current_year = chrono::Local::now().date_naive().year() as u16;
            let mut actionable = Vec::new();
            for p in &self.profiles {
                if let Ok(summaries) = db_lock.list_draft_summaries(&p.tin.full(), current_year) {
                    for sum in summaries {
                        actionable.push((p.full_name.clone(), sum));
                    }
                }
            }
            self.actionable_forms = actionable;
        }
        cx.notify();
    }

    fn urgent_actions_section(&self, window: &Window, cx: &mut Context<Self>) -> gpui::Div {
        let items: Vec<_> = self
            .actionable_forms
            .iter()
            .filter(|(_, sum)| sum.status != bir_core::forms::FilingStatus::Paid)
            .map(|(profile_name, sum)| {
                let (status_text, action_label, is_urgent) = match sum.status {
                    bir_core::forms::FilingStatus::Draft => ("Draft", "Resume", false),
                    bir_core::forms::FilingStatus::Queued => ("Queued", "Check Status", false),
                    bir_core::forms::FilingStatus::Submitted => {
                        ("Awaiting Confirmation", "Check Confirmation", true)
                    }
                    bir_core::forms::FilingStatus::Confirmed => {
                        ("Confirmed", "Upload Receipt", true)
                    }
                    bir_core::forms::FilingStatus::Paid => ("Paid", "View Paid Return", false),
                };
                (
                    profile_name.as_str(),
                    sum.tin.as_str(),
                    sum.form_code.as_str(),
                    sum.taxable_year,
                    sum.quarter,
                    status_text,
                    action_label,
                    is_urgent,
                )
            })
            .collect();

        let use_card_view = window.viewport_size().width < px(900.);

        let content = if items.is_empty() {
            div()
                .w_full()
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border)
                .rounded_xl()
                .shadow_sm()
                .child(
                    div()
                        .p_4()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("No actionable items found."),
                )
        } else if use_card_view {
            self.render_action_cards(&items, cx)
        } else {
            self.render_action_table(&items, cx)
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child("Action Required"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().primary)
                            .cursor_pointer()
                            .child("View All"),
                    ),
            )
            .child(content)
    }

    /// Table layout for wider viewports.
    fn render_action_table(
        &self,
        items: &[(&str, &str, &str, u16, Option<u8>, &str, &str, bool)],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let mut rows = div().flex().flex_col();
        for &(profile, tin, form, year, quarter, status, action_label, is_urgent) in items {
            rows = rows.child(Self::action_table_row(
                profile,
                tin,
                form,
                year,
                quarter,
                status,
                action_label,
                is_urgent,
                cx,
            ));
        }

        div()
            .w_full()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_xl()
            .shadow_sm()
            .flex()
            .flex_col()
            .child(
                // Table Header
                div()
                    .flex()
                    .p_3()
                    .bg(cx.theme().muted)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().muted_foreground)
                    .child(div().flex_1().child("Profile"))
                    .child(div().w(px(80.)).child("Form"))
                    .child(div().flex_1().child("Status / Issue"))
                    .child(div().w(px(50.)).text_center().child("Action")),
            )
            .child(rows)
    }

    /// Card layout for narrow viewports.
    fn render_action_cards(
        &self,
        items: &[(&str, &str, &str, u16, Option<u8>, &str, &str, bool)],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let mut cards = div().flex().flex_col().gap_3();
        for &(profile, tin, form, year, quarter, status, action_label, is_urgent) in items {
            cards = cards.child(Self::action_card(
                profile,
                tin,
                form,
                year,
                quarter,
                status,
                action_label,
                is_urgent,
                cx,
            ));
        }
        cards
    }

    /// Single table row for the action required table.
    fn action_table_row(
        profile: &str,
        tin: &str,
        form: &str,
        year: u16,
        quarter: Option<u8>,
        status: &str,
        action_label: &str,
        is_urgent: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let warning_color: gpui::Hsla = gpui::rgb(0xef4444).into();
        let tin_clone = tin.to_string();
        let form_clone = form.to_string();
        let q_num = quarter.unwrap_or(0);

        div()
            .flex()
            .p_3()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .text_sm()
            .child(
                div()
                    .flex_1()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .overflow_hidden()
                    .child(profile.to_string()),
            )
            .child(
                div()
                    .w(px(80.))
                    .text_color(cx.theme().muted_foreground)
                    .child(form.to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .overflow_hidden()
                    .children(if is_urgent {
                        Some(
                            div()
                                .px_2()
                                .py_0p5()
                                .bg(warning_color.opacity(0.1))
                                .text_color(warning_color)
                                .rounded_md()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .child("!"),
                        )
                    } else {
                        None
                    })
                    .child(
                        div()
                            .text_color(if is_urgent {
                                warning_color
                            } else {
                                cx.theme().foreground
                            })
                            .child(status.to_string()),
                    ),
            )
            .child(Self::action_icon_button(
                &tin_clone,
                &form_clone,
                year,
                q_num,
                action_label,
                is_urgent,
                cx,
            ))
    }

    /// Single card for narrow viewport.
    fn action_card(
        profile: &str,
        tin: &str,
        form: &str,
        year: u16,
        quarter: Option<u8>,
        status: &str,
        action_label: &str,
        is_urgent: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let warning_color: gpui::Hsla = gpui::rgb(0xef4444).into();
        let tin_clone = tin.to_string();
        let form_clone = form.to_string();
        let q_num = quarter.unwrap_or(0);

        div()
            .w_full()
            .p_4()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .shadow_sm()
            .flex()
            .flex_col()
            .gap_2()
            // Top row: profile name + action button
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(profile.to_string()),
                    )
                    .child(Self::action_icon_button(
                        &tin_clone,
                        &form_clone,
                        year,
                        q_num,
                        action_label,
                        is_urgent,
                        cx,
                    )),
            )
            // Form type
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Form: {}", form)),
            )
            // Status with urgency indicator
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(if is_urgent {
                        Some(
                            div()
                                .px_2()
                                .py_0p5()
                                .bg(warning_color.opacity(0.1))
                                .text_color(warning_color)
                                .rounded_md()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .child("!"),
                        )
                    } else {
                        None
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(if is_urgent {
                                warning_color
                            } else {
                                cx.theme().foreground
                            })
                            .child(status.to_string()),
                    ),
            )
    }

    /// Compact icon-only action button with tooltip.
    fn action_icon_button(
        tin: &str,
        form: &str,
        year: u16,
        q_num: u8,
        action_label: &str,
        _is_check_status: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let tin_clone = tin.to_string();
        let form_clone = form.to_string();
        let tooltip_text = action_label.to_string();

        let icon = if action_label == "Check Confirmation" {
            "✉"
        } else if action_label == "Upload Receipt" {
            "↑"
        } else {
            "▶"
        };

        div()
            .w(px(50.))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id(format!("action-btn-{}-{}-{}", tin, form, q_num))
                    .w(px(32.))
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(cx.theme().secondary)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().secondary_hover))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child(icon),
                    )
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(tooltip_text.clone())
                            .build(window, cx)
                    })
                    .on_click(cx.listener({
                        let is_check = action_label == "Check Confirmation";
                        move |this, _, window, cx| {
                            if is_check {
                                this.check_status_for_tin(&tin_clone, window, cx);
                            } else {
                                cx.emit(GlobalDashboardEvent::OpenForm {
                                    tin: tin_clone.clone(),
                                    form_code: form_clone.clone(),
                                    year,
                                    quarter: q_num,
                                });
                            }
                        }
                    })),
            )
    }

    /// Run the email check for a specific TIN — the core of the "Check Status" action.
    fn check_status_for_tin(&mut self, tin: &str, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_component::WindowExt;

        // Look up the profile
        let profile = self
            .db
            .lock()
            .ok()
            .and_then(|db| db.get_profile(tin).ok().flatten());

        let Some(profile) = profile else {
            window.push_notification(
                gpui_component::notification::Notification::error("Profile Not Found".to_string())
                    .message(format!("Could not find profile for TIN {}", tin)),
                cx,
            );
            return;
        };

        if !profile.is_email_tracking_active() {
            window.push_notification(
                gpui_component::notification::Notification::error(
                    "Email Tracking Not Enabled".to_string(),
                )
                .message(
                    "Go to Email Settings in your profile to set up App Password or Google OAuth2."
                        .to_string(),
                ),
                cx,
            );
            return;
        }

        // Show progress notification
        window.push_notification(
            gpui_component::notification::Notification::new()
                .message("Checking email for BIR confirmation...".to_string())
                .with_type(gpui_component::notification::NotificationType::Info)
                .autohide(true),
            cx,
        );

        let db_clone = self.db.clone();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move {
                bir_core::email::fetch_and_process_emails(&profile, db_clone)
            }).await;

            let _ = cx.update(|cx| {
                if let Some(this) = this.upgrade() {
                    this.update(cx, |this, cx| {
                        match result {
                            Ok(receipts) if !receipts.is_empty() => {
                                // Refresh actionable forms from DB
                                this.reload_actionable_forms(cx);
                                cx.emit(GlobalDashboardEvent::PushNotification(
                                    "success".to_string(),
                                    "Confirmation Received!".to_string(),
                                    format!("{} confirmation(s) processed successfully.", receipts.len()),
                                ));
                                cx.emit(GlobalDashboardEvent::StatusChanged);
                            }
                            Ok(_) => {
                                cx.emit(GlobalDashboardEvent::PushNotification(
                                    "info".to_string(),
                                    "No Confirmation Yet".to_string(),
                                    "No new confirmation email from BIR was found. Please try again later.".to_string(),
                                ));
                            }
                            Err(e) => {
                                cx.emit(GlobalDashboardEvent::PushNotification(
                                    "error".to_string(),
                                    "Email Check Failed".to_string(),
                                    e.to_string(),
                                ));
                            }
                        }
                        cx.notify();
                    });
                }
            });
        }).detach();
    }

    fn news_section(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut news_list = div().id("news-list").flex().flex_col().gap_4().pr_2(); // add some padding

        for ann in &self.announcements {
            news_list = news_list.child(Self::news_card(ann, cx));
        }

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child("Important News"),
                    )
                    .child(if self.is_fetching_news {
                        div()
                            .px_3()
                            .py_1()
                            .bg(cx.theme().muted)
                            .text_color(cx.theme().muted_foreground)
                            .rounded_md()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child("Fetching...")
                            .into_any_element()
                    } else {
                        div()
                            .id("refresh-news")
                            .px_3()
                            .py_1()
                            .bg(cx.theme().primary.opacity(0.1))
                            .text_color(cx.theme().primary)
                            .rounded_md()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .hover(|s| {
                                s.bg(cx.theme().primary)
                                    .text_color(cx.theme().primary_foreground)
                            })
                            .child("Refresh")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.refresh_news(cx);
                            }))
                            .into_any_element()
                    }),
            )
            .child(news_list)
    }

    fn news_card(notice: &BirNotice, cx: &Context<Self>) -> gpui::Div {
        let badge_bg: gpui::Hsla = match notice.source_kind {
            bir_core::db::NoticeSourceKind::BirCms => cx.theme().primary,
            bir_core::db::NoticeSourceKind::FacebookGraph => gpui::rgb(0x1877f2).into(), // Facebook Blue
            bir_core::db::NoticeSourceKind::Rss => cx.theme().accent,
            bir_core::db::NoticeSourceKind::Manual => cx.theme().muted_foreground,
        };

        div()
            .w_full()
            .p_4()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_xl()
            .shadow_sm()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .bg(badge_bg.opacity(0.1))
                            .rounded_md()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(badge_bg)
                            .child(notice.source.to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(notice.posted_at.clone().unwrap_or_default()),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child(notice.title.to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .line_height(relative(1.4))
                    .child(notice.body.to_string()),
            )
    }
}
