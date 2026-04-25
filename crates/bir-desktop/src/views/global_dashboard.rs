use bir_core::db::{Announcement, Database, PenaltyCache, TaxDeadline};
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
    penalties: Vec<PenaltyCache>,
}

impl GlobalDashboardView {
    pub fn new(db: Arc<Mutex<Database>>, _window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let (profiles, deadlines, announcements, penalties) = if let Ok(db_lock) = db.lock() {
            let profiles = db_lock.list_profiles().unwrap_or_default();
            let mut deadlines = db_lock.list_tax_deadlines().unwrap_or_default();
            let mut announcements = db_lock.list_announcements().unwrap_or_default();
            
            // For now, load penalties for all profiles or just list all from cache if we had a list_all method.
            // Since we only have list by tin, we'll collect across profiles
            let mut penalties = Vec::new();
            for p in &profiles {
                if let Ok(p_cache) = db_lock.list_penalties_cache(&p.tin.full()) {
                    penalties.extend(p_cache);
                }
            }
            
            // Seed mock data if empty
            if deadlines.is_empty() {
                let mock_deadlines = vec![
                    TaxDeadline { id: None, form_type: "2551Q".into(), due_date: "2026-04-25".into(), description: "Q1 Percentage Tax".into() },
                    TaxDeadline { id: None, form_type: "1701Q".into(), due_date: "2026-05-15".into(), description: "Q1 Income Tax".into() },
                    TaxDeadline { id: None, form_type: "2550M".into(), due_date: "2026-05-20".into(), description: "April VAT".into() },
                ];
                for d in mock_deadlines.clone() { let _ = db_lock.save_tax_deadline(&d); }
                deadlines = mock_deadlines;
            }

            if announcements.is_empty() {
                let mock_announcements = vec![
                    Announcement { id: None, source: "BIR Official".into(), title: "Extension of Deadline for ITR Filing".into(), content: "The Bureau of Internal Revenue announces the extension of the deadline for the filing of Annual Income Tax Returns...".into(), published_at: "2 hrs ago".into(), read_status: false },
                    Announcement { id: None, source: "Facebook".into(), title: "RDO 43 System Maintenance".into(), content: "Please be advised that RDO 43 will undergo system maintenance this weekend. Online services will be temporarily unavailable.".into(), published_at: "5 hrs ago".into(), read_status: false },
                    Announcement { id: None, source: "RDO Announcement".into(), title: "New Guidelines for Form 2551Q".into(), content: "Updated procedures for the submission of Quarterly Percentage Tax Returns have been published.".into(), published_at: "1 day ago".into(), read_status: false },
                ];
                for a in mock_announcements.clone() { let _ = db_lock.save_announcement(&a); }
                announcements = mock_announcements;
            }
            
            if penalties.is_empty() && !profiles.is_empty() {
                let mock_penalties = vec![
                    PenaltyCache { id: None, tin: profiles[0].tin.full(), form_type: "1701Q - Q1".into(), period: "2026Q1".into(), penalty_amount: 10000.0, reason: "Late Filing".into(), is_high_risk: true, calculated_at: "now".into() },
                ];
                for p in mock_penalties.clone() { let _ = db_lock.save_penalty_cache(&p); }
                penalties = mock_penalties;
            }
            
            (profiles, deadlines, announcements, penalties)
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };

        let mut view = Self { db, profiles, deadlines, announcements, penalties };
        view.refresh_news(cx);
        view
    }

    pub fn refresh_news(&mut self, cx: &mut Context<Self>) {
        let fetch_db = self.db.clone();
        cx.spawn(async move |view, mut cx| {
            // Run all blocking reqwest operations on the background thread pool
            cx.background_executor().spawn(async move {
                let fetcher = bir_core::news_fetcher::NewsFetcher::new(fetch_db);
                let _ = fetcher.fetch_and_sync();
            }).await;
            
            // Back on the main thread, reload the announcements
            let _ = view.update(cx, |view, cx| {
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

        div()
            .id("global-dashboard-scroll")
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
                    .gap_4()
                    .child(Self::profile_stat_card(self.profiles.len(), cx))
                    .child(Self::stat_card("Unsubmitted", "2", "📝", cx, false))
                    .child(Self::stat_card("Overdue", "1", "⚠️", cx, true))
                    .child(Self::stat_card("Upcoming", "3", "📅", cx, false))
                    .child(Self::stat_card("Emails", "14", "✉️", cx, false)),
            )
            .child(
                div()
                    .flex()
                    .gap_6()
                    .mt_4()
                    .child(
                        // Left Column
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(Self::urgent_actions_section(cx))
                            .child(Self::calendar_section(&self.deadlines, &self.announcements, cx))
                            .child(Self::penalty_section(&self.penalties, cx)),
                    )
                    .child(
                        // Right Column
                        div()
                            .w(px(350.))
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(self.news_section(cx)),
                    ),
            )
    }
}

impl GlobalDashboardView {
    fn profile_stat_card(count: usize, cx: &Context<Self>) -> gpui::Div {
        let content = if count == 0 {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child("No profiles yet"))
                .child(
                    div()
                        .px_3()
                        .py_1()
                        .bg(cx.theme().primary)
                        .rounded_md()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().primary_foreground)
                        .cursor_pointer()
                        .child("+ Add Profile")
                )
        } else {
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .text_color(cx.theme().foreground)
                .child(count.to_string())
        };

        div()
            .flex_1()
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
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("Total Profiles"),
                    )
                    .child(div().text_xl().child("👤")),
            )
            .child(content)
    }

    fn stat_card(title: &str, value: &str, icon: &str, cx: &Context<Self>, is_warning: bool) -> gpui::Div {
        let warning_color: gpui::Hsla = gpui::rgb(0xef4444).into();
        div()
            .flex_1()
            .p_4()
            .bg(if is_warning { warning_color.opacity(0.05) } else { cx.theme().background })
            .border_1()
            .border_color(if is_warning { warning_color } else { cx.theme().border })
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
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if is_warning { warning_color } else { cx.theme().muted_foreground })
                            .child(title.to_string()),
                    )
                    .child(div().text_xl().child(icon.to_string())),
            )
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_warning { warning_color } else { cx.theme().foreground })
                    .child(value.to_string()),
            )
    }

    fn urgent_actions_section(cx: &Context<Self>) -> gpui::Div {
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
                    // Rows
                    .child(Self::action_row("Acme Corp", "2550Q", "Unsubmitted (Server Error)", true, cx))
                    .child(Self::action_row("Manila Logistics", "1601-C", "Overdue (Nov 10)", false, cx)),
            )
    }

    fn action_row(profile: &str, form: &str, status: &str, is_urgent: bool, cx: &Context<Self>) -> gpui::Div {
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
                            .bg(cx.theme().primary.opacity(0.1))
                            .text_color(cx.theme().primary)
                            .rounded_md()
                            .font_weight(FontWeight::BOLD)
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().primary).text_color(cx.theme().primary_foreground))
                            .child(if is_urgent { "Resume" } else { "Review" })
                    )
            )
    }

    fn calendar_section(deadlines: &[TaxDeadline], announcements: &[Announcement], cx: &Context<Self>) -> gpui::Div {
        let mut schedule_list = div().flex().flex_col().gap_3();
        
        if deadlines.is_empty() {
            schedule_list = schedule_list.child(
                div().text_sm().text_color(cx.theme().muted_foreground).child("No upcoming deadlines.")
            );
        } else {
            for d in deadlines {
                // Check if any announcement mentions this form type
                let has_update = announcements.iter().any(|a| a.title.contains(&d.form_type) || a.content.contains(&d.form_type));
                
                let date_card = div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .p_3()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_lg()
                    .child(
                        // Date Badge
                        div()
                            .w(px(60.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .p_2()
                            .bg(cx.theme().primary.opacity(0.1))
                            .rounded_md()
                            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(cx.theme().primary).child(d.due_date[5..7].to_string())) // month
                            .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(cx.theme().primary).child(d.due_date[8..10].to_string())) // day
                    )
                    .child(
                        // Details
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div().flex().items_center().gap_2()
                                    .child(div().text_sm().font_weight(FontWeight::BOLD).text_color(cx.theme().foreground).child(d.form_type.clone()))
                                    .children(if has_update {
                                        Some(div().px_2().py_0p5().bg(cx.theme().warning.opacity(0.1)).rounded_md().text_xs().text_color(cx.theme().warning).child("Updated"))
                                    } else {
                                        None
                                    })
                            )
                            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(d.description.clone()))
                    );
                schedule_list = schedule_list.child(date_card);
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
                            .child("Compliance Calendar"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(Self::filter_chip("All", true, cx))
                            .child(Self::filter_chip("2551Q", false, cx))
                            .child(Self::filter_chip("1701Q", false, cx)),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .p_4()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_xl()
                    .shadow_sm()
                    .child(schedule_list),
            )
    }

    fn penalty_section(penalties: &[PenaltyCache], cx: &Context<Self>) -> gpui::Div {
        let mut penalty_cards = div().flex().flex_col().gap_3();
        
        if penalties.is_empty() {
            penalty_cards = penalty_cards.child(
                div().text_sm().text_color(cx.theme().muted_foreground).child("No penalties incurred.")
            );
        } else {
            for p in penalties {
                // Formatting amount
                let amount_str = format!("₱ {:.2}", p.penalty_amount);
                penalty_cards = penalty_cards.child(
                    Self::penalty_card(&p.tin, &p.form_type, &p.reason, &amount_str, p.is_high_risk, cx)
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child("Incurred Penalties & Risks"),
            )
            .child(penalty_cards)
    }

    fn penalty_card(profile: &str, form: &str, reason: &str, amount: &str, is_high_risk: bool, cx: &Context<Self>) -> gpui::Div {
        let risk_color = if is_high_risk { gpui::rgb(0xef4444) } else { gpui::rgb(0xf59e0b) };

        div()
            .w_full()
            .p_4()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .border_l_4()
            .border_color(risk_color)
            .rounded_lg()
            .shadow_sm()
            .flex()
            .justify_between()
            .items_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(profile.to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(form.to_string())
                            .child("•")
                            .child(reason.to_string()),
                    ),
            )
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .text_color(risk_color)
                    .child(amount.to_string()),
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

