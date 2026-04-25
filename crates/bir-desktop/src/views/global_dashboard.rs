use bir_core::db::{Announcement, Database, TaxDeadline};
use bir_core::forms::FormDraftSummary;
use bir_core::profile::TaxpayerProfile;
use chrono::{Datelike, Local};
use gpui::*;
use gpui_component::*;
use std::sync::{Arc, Mutex};

pub struct GlobalDashboardView {
    db: Arc<Mutex<Database>>,
    profiles: Vec<TaxpayerProfile>,
    deadlines: Vec<TaxDeadline>,
    announcements: Vec<Announcement>,
    actionable_forms: Vec<(String, FormDraftSummary)>,
    is_fetching_news: bool,
    compliance_calendar: Entity<crate::components::compliance_calendar::ComplianceCalendar>,
}

impl GlobalDashboardView {
    pub fn new(db: Arc<Mutex<Database>>, _window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let (profiles, deadlines, announcements, actionable_forms) = if let Ok(db_lock) = db.lock() {
            let profiles = db_lock.list_profiles().unwrap_or_default();
            let mut deadlines = db_lock.list_tax_deadlines().unwrap_or_default();
            let announcements = db_lock.list_announcements().unwrap_or_default();
            
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
                    TaxDeadline { id: None, form_type: "2551Q".into(), due_date: "2026-04-25".into(), description: "Q1 Percentage Tax".into() },
                    TaxDeadline { id: None, form_type: "1701Q".into(), due_date: "2026-05-15".into(), description: "Q1 Income Tax".into() },
                    TaxDeadline { id: None, form_type: "2550M".into(), due_date: "2026-05-20".into(), description: "April VAT".into() },
                ];
                for d in mock_deadlines.clone() { let _ = db_lock.save_tax_deadline(&d); }
                deadlines = mock_deadlines;
            }
            
            (profiles, deadlines, announcements, actionable_forms)
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };

        let compliance_calendar = cx.new(|cx| crate::components::compliance_calendar::ComplianceCalendar::new(cx));

        let mut view = Self { db, profiles, deadlines, announcements, actionable_forms, is_fetching_news: false, compliance_calendar };
        view.refresh_news(cx);
        view
    }

    pub fn refresh_news(&mut self, cx: &mut Context<Self>) {
        if self.is_fetching_news { return; }
        self.is_fetching_news = true;
        
        let fetch_db = self.db.clone();
        cx.spawn(async move |view, mut cx| {
            // Run all blocking reqwest operations on the background thread pool
            cx.background_executor().spawn(async move {
                let fetcher = bir_core::news_fetcher::NoticeFetcher::new(fetch_db);
                let _ = fetcher.fetch_and_sync();
            }).await;
            
            // Back on the main thread, reload the announcements
            let _ = view.update(cx, |view, cx| {
                view.is_fetching_news = false;
                if let Ok(db_lock) = view.db.lock() {
                    view.announcements = db_lock.list_announcements().unwrap_or_default();
                    cx.notify();
                }
            });
        }).detach();
    }
    pub fn set_profiles(&mut self, profiles: Vec<TaxpayerProfile>, cx: &mut Context<Self>) {
        self.profiles = profiles;
        // Optionally reload penalties here
        cx.notify();
    }
}

impl Render for GlobalDashboardView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let now = Local::now().date_naive();
        let year = now.year();

        self.compliance_calendar.update(cx, |calendar, _| {
            calendar.set_data(self.deadlines.clone(), self.announcements.clone());
        });

        div()
            .id("global-dashboard")
            .size_full()
            .flex()
            .flex_col()
            .p_8()
            .gap_6()
            .overflow_y_scroll()
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
                    .flex()
                    .flex_wrap()
                    .gap_6()
                    .mt_4()
                    .child(
                        // Left Column
                        div()
                            .flex_1()
                            .min_w(px(500.))
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(self.urgent_actions_section(cx))
                            .child(self.compliance_calendar.clone()),
                    )
                    .child(
                        // Right Column
                        div()
                            .flex_1()
                            .min_w(px(350.))
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(self.news_section(cx)),
                    ),
            )
    }
}

impl GlobalDashboardView {


    fn urgent_actions_section(&self, cx: &Context<Self>) -> gpui::Div {
        let mut rows = div().flex().flex_col();
        
        let actionable = self.actionable_forms.iter().filter(|(_, sum)| sum.status != bir_core::forms::FilingStatus::Confirmed).collect::<Vec<_>>();
        
        if actionable.is_empty() {
            rows = rows.child(
                div().p_4().text_sm().text_color(cx.theme().muted_foreground).child("No actionable items found.")
            );
        } else {
            for (profile_name, sum) in actionable {
                let (status_text, action_label, is_urgent) = match sum.status {
                    bir_core::forms::FilingStatus::Draft => ("Draft", "Resume", false),
                    bir_core::forms::FilingStatus::Submitted => ("Awaiting Confirmation", "Check Status", true),
                    _ => ("Unknown", "View", false),
                };
                
                rows = rows.child(Self::action_row(
                    profile_name,
                    &sum.form_code,
                    status_text,
                    action_label,
                    is_urgent,
                    cx
                ));
            }
        }

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
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
            .child(
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
                            .child(div().w(px(200.)).child("Profile"))
                            .child(div().w(px(100.)).child("Form"))
                            .child(div().flex_1().child("Status / Issue"))
                            .child(div().w(px(100.)).child("Action")),
                    )
                    .child(rows),
            )
    }

    fn action_row(profile: &str, form: &str, status: &str, action_label: &str, is_urgent: bool, cx: &Context<Self>) -> gpui::Div {
        let warning_color: gpui::Hsla = gpui::rgb(0xef4444).into();
        div()
            .flex()
            .p_3()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .text_sm()
            .child(div().w(px(200.)).font_weight(FontWeight::SEMIBOLD).text_color(cx.theme().foreground).child(profile.to_string()))
            .child(div().w(px(100.)).text_color(cx.theme().muted_foreground).child(form.to_string()))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(if is_urgent {
                        Some(div().px_2().py_0p5().bg(warning_color.opacity(0.1)).text_color(warning_color).rounded_md().text_xs().font_weight(FontWeight::BOLD).child("!"))
                    } else { None })
                    .child(div().text_color(if is_urgent { warning_color } else { cx.theme().foreground }).child(status.to_string()))
            )
            .child(
                div()
                    .w(px(100.))
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .bg(cx.theme().secondary)
                            .text_color(cx.theme().foreground)
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_md()
                            .font_weight(FontWeight::BOLD)
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().secondary_hover))
                            .child(action_label.to_string())
                    )
            )
    }



    fn news_section(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut news_list = div()
            .id("news-list")
            .flex()
            .flex_col()
            .gap_4()
            .pr_2() // add some padding for scrollbar
            .max_h(px(600.))
            .overflow_y_scroll();
        
        for ann in &self.announcements {
            news_list = news_list.child(
                Self::news_card(&ann.source, &ann.title, &ann.content, &ann.published_at, cx)
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child("Important News"),
                    )
                    .child(
                        if self.is_fetching_news {
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
                                .hover(|s| s.bg(cx.theme().primary).text_color(cx.theme().primary_foreground))
                                .child("Refresh")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.refresh_news(cx);
                                }))
                                .into_any_element()
                        }
                    )
            )
            .child(news_list)
    }

    fn news_card(source: &str, title: &str, snippet: &str, time: &str, cx: &Context<Self>) -> gpui::Div {
        let badge_bg: gpui::Hsla = match source {
            "BIR Official" => cx.theme().primary,
            "Facebook" => gpui::rgb(0x1877f2).into(), // Facebook Blue
            _ => cx.theme().muted_foreground,
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
                            .child(source.to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(time.to_string()),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .line_height(relative(1.4))
                    .child(snippet.to_string()),
            )
    }

    fn filter_chip(label: &str, is_active: bool, cx: &Context<Self>) -> gpui::Div {
        let (bg, text_color, border_color) = if is_active {
            (cx.theme().primary.opacity(0.1), cx.theme().primary, cx.theme().primary)
        } else {
            (cx.theme().background, cx.theme().foreground, cx.theme().border)
        };

        div()
            .px_3()
            .py_1()
            .bg(bg)
            .border_1()
            .border_color(border_color)
            .rounded_full()
            .text_sm()
            .font_weight(if is_active { FontWeight::BOLD } else { FontWeight::NORMAL })
            .text_color(text_color)
            .child(label.to_string())
    }
}

