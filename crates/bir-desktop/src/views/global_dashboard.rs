#![allow(dead_code)]
use bir_core::calendar_rules::{DeadlineOverride, DeadlineResolver, ResolvedTaxDeadline};
use bir_core::db::{BirNotice, Database};
use bir_core::profile::TaxpayerProfile;
use chrono::{Datelike, Local};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::*;
use std::sync::{Arc, Mutex};

use crate::components::compliance_calendar::ComplianceCalendarEvent;

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
    deadlines: Vec<ResolvedTaxDeadline>,
    announcements: Vec<BirNotice>,
    is_fetching_news: bool,
    compliance_calendar: Entity<crate::components::compliance_calendar::ComplianceCalendar>,
    pub hide_tax_profiles: bool,
    pub active_session_tin: Option<String>,
    calendar_year: i32,
}

impl EventEmitter<GlobalDashboardEvent> for GlobalDashboardView {}

impl GlobalDashboardView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let year = chrono::Local::now().year();
        let (profiles, deadlines, announcements) = if let Ok(db_lock) = db.lock() {
            let profiles = db_lock.list_profiles().unwrap_or_default();
            let overrides = db_lock.get_deadline_overrides();
            let deadlines = Self::deadlines_for_profiles(&profiles, year, &overrides);
            let announcements = db_lock.list_bir_notices().unwrap_or_default();

            (profiles, deadlines, announcements)
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };

        let compliance_calendar = cx
            .new(|cx| crate::components::compliance_calendar::ComplianceCalendar::new(window, cx));

        cx.subscribe(
            &compliance_calendar,
            |this: &mut Self, _entity, event: &ComplianceCalendarEvent, cx| match event {
                ComplianceCalendarEvent::YearChanged { year } => {
                    this.calendar_year = *year;
                    let overrides = this
                        .db
                        .lock()
                        .map(|db| db.get_deadline_overrides())
                        .unwrap_or_default();
                    this.deadlines =
                        Self::deadlines_for_profiles(&this.profiles, *year, &overrides);
                    cx.notify();
                }
            },
        )
        .detach();

        let mut view = Self {
            db,
            profiles,
            deadlines,
            announcements,
            is_fetching_news: false,
            compliance_calendar,
            hide_tax_profiles: false,
            active_session_tin: None,
            calendar_year: year,
        };

        let bus = cx.global::<crate::events::GlobalEventBus>().0.clone();
        cx.subscribe(
            &bus,
            |this: &mut Self, _bus, event: &crate::events::AppEvent, cx| {
                match event {
                    crate::events::AppEvent::DatabaseChanged => {
                        // Refresh profiles and other DB-backed info if needed
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
        let year = self.calendar_year;
        let overrides = self
            .db
            .lock()
            .map(|db| db.get_deadline_overrides())
            .unwrap_or_default();
        self.deadlines = Self::deadlines_for_profiles(&self.profiles, year, &overrides);
        cx.notify();
    }

    pub fn reload_deadlines(&mut self, cx: &mut Context<Self>) {
        let year = self.calendar_year;
        let overrides = self
            .db
            .lock()
            .map(|db| db.get_deadline_overrides())
            .unwrap_or_default();
        self.deadlines = Self::deadlines_for_profiles(&self.profiles, year, &overrides);
        cx.notify();
    }

    fn deadlines_for_profiles(
        _profiles: &[TaxpayerProfile],
        calendar_year: i32,
        overrides: &[DeadlineOverride],
    ) -> Vec<ResolvedTaxDeadline> {
        // Show the FULL BIR tax calendar — all forms, all frequencies.
        // Profile-specific filtering belongs in the per-profile DashboardView,
        // not here. The Global Dashboard mirrors BIR's official tax calendar.
        let mut all = DeadlineResolver::resolve_deadline_calendar_year_with_overrides(
            calendar_year,
            overrides,
        );

        // Append event-based (as-needed) obligations that have no fixed date
        all.extend(DeadlineResolver::event_based_obligations());

        all
    }
}

impl Render for GlobalDashboardView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
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
                            .child(format!(
                                "Overview of all taxpayer profiles for {}",
                                self.calendar_year
                            )),
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
    fn news_section(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut news_list = div().id("news-list").flex().flex_col().gap_4().pr_2(); // add some padding

        for ann in self.announcements.iter().take(25) {
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
                    .child(if notice.title.len() > 150 {
                        format!("{}...", &notice.title[..147])
                    } else {
                        notice.title.to_string()
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .line_height(relative(1.4))
                    .child(Self::clean_html_summary(&notice.body, 250)),
            )
    }

    fn clean_html_summary(html: &str, max_len: usize) -> String {
        let mut cleaned = String::with_capacity(html.len());
        let mut in_tag = false;
        for c in html.chars() {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            } else if !in_tag {
                cleaned.push(c);
            }
        }

        cleaned = cleaned
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'");

        let words: Vec<&str> = cleaned.split_whitespace().collect();
        let condensed = words.join(" ");

        if condensed.chars().count() > max_len {
            let truncated: String = condensed.chars().take(max_len).collect();
            format!("{}...", truncated.trim_end())
        } else {
            condensed
        }
    }
}
