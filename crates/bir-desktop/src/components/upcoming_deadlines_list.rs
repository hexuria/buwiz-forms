use bir_core::calendar_rules::{DeadlineKind, DeadlineStatus, ResolvedTaxDeadline};
use bir_core::db::BirNotice;
use chrono::{Datelike, Local};
use gpui::prelude::*;
use gpui::*;
use gpui_component::*;

pub enum UpcomingDeadlinesListEvent {
    DeadlineClicked {
        form_code: String,
        year: u16,
        quarter: u8,
    },
}

impl EventEmitter<UpcomingDeadlinesListEvent> for UpcomingDeadlinesList {}

pub struct UpcomingDeadlinesList {
    deadlines: Vec<ResolvedTaxDeadline>,
    announcements: Vec<BirNotice>,
}

impl UpcomingDeadlinesList {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            deadlines: Vec::new(),
            announcements: Vec::new(),
        }
    }

    pub fn set_data(&mut self, deadlines: Vec<ResolvedTaxDeadline>, announcements: Vec<BirNotice>) {
        self.deadlines = deadlines;
        self.announcements = announcements;
    }

    fn render_list_view(&self, _window: &Window, cx: &Context<Self>) -> gpui::Div {
        let dated: Vec<&ResolvedTaxDeadline> = self
            .deadlines
            .iter()
            .filter(|d| d.final_deadline_date().is_some())
            .collect();
        let event_based: Vec<&ResolvedTaxDeadline> = self
            .deadlines
            .iter()
            .filter(|d| d.final_deadline_date().is_none())
            .collect();
        let mut schedule_list = div().flex().flex_col().gap_3();
        let today = chrono::Local::now().date_naive();

        if dated.is_empty() && event_based.is_empty() {
            schedule_list = schedule_list.child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .py_8()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().muted_foreground)
                            .child("No upcoming deadlines for selected filters."),
                    ),
            );
        } else {
            for d in &dated {
                let has_update = self
                    .announcements
                    .iter()
                    .any(|a| a.title.contains(&d.form_code) || a.body.contains(&d.form_code));

                let deadline_date = d.final_deadline_date();
                let days_until = deadline_date.map(|date| (date - today).num_days());

                let (bg_color, border_color, text_color): (gpui::Hsla, gpui::Hsla, gpui::Hsla) =
                    match days_until {
                        Some(days) if days < 0 => (
                            gpui::rgba(0xef444420).into(),
                            gpui::rgba(0xef444440).into(),
                            gpui::rgba(0xef4444ff).into(),
                        ),
                        Some(days) if days <= 7 => (
                            gpui::rgba(0xf9731620).into(),
                            gpui::rgba(0xf9731640).into(),
                            gpui::rgba(0xf97316ff).into(),
                        ),
                        Some(_) => (
                            cx.theme().primary.opacity(0.08),
                            cx.theme().primary.opacity(0.1),
                            cx.theme().primary,
                        ),
                        None => (
                            cx.theme().secondary,
                            cx.theme().border,
                            cx.theme().muted_foreground,
                        ),
                    };

                let form_code_clone = d.form_code.clone();
                let route = d.route_year_quarter();
                let date_id = d.final_deadline_string();
                let month_label = deadline_date
                    .map(|date| date.format("%b").to_string().to_uppercase())
                    .unwrap_or_else(|| "EVT".to_string());
                let day_label = deadline_date
                    .map(|date| date.format("%d").to_string())
                    .unwrap_or_else(|| "--".to_string());

                let mut date_card = div()
                    .id(format!("deadline-{}-{}", d.form_code, date_id))
                    .group("list-item")
                    .flex()
                    .items_center()
                    .gap_4()
                    .p_3()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_lg()
                    .hover(|s| {
                        s.bg(cx.theme().secondary)
                            .border_color(cx.theme().primary.opacity(0.5))
                    });

                if let Some((year_clone, quarter_clone)) = route {
                    date_card = date_card.cursor_pointer().on_click(cx.listener(
                        move |_this, _ev, _win, cx| {
                            cx.emit(UpcomingDeadlinesListEvent::DeadlineClicked {
                                form_code: form_code_clone.clone(),
                                year: year_clone,
                                quarter: quarter_clone,
                            });
                        },
                    ));
                } else {
                    date_card = date_card.cursor_default();
                }

                let date_card = date_card
                    .child(
                        div()
                            .w(px(64.))
                            .h(px(64.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .bg(bg_color)
                            .border_1()
                            .border_color(border_color)
                            .rounded_md()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(text_color)
                                    .child(month_label),
                            ) // month
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BLACK)
                                    .text_color(text_color)
                                    .child(day_label),
                            ), // day
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child(d.display_form_no.clone()),
                                    )
                                    .children(if has_update {
                                        Some(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .bg(cx.theme().warning.opacity(0.1))
                                                .border_1()
                                                .border_color(cx.theme().warning.opacity(0.2))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(cx.theme().warning)
                                                        .child("Updated"),
                                                ),
                                        )
                                    } else {
                                        None
                                    })
                                    .children(if d.status != DeadlineStatus::Normal {
                                        Some(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .bg(cx.theme().primary.opacity(0.1))
                                                .border_1()
                                                .border_color(cx.theme().primary.opacity(0.2))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(cx.theme().primary)
                                                        .child(d.status.label()),
                                                ),
                                        )
                                    } else {
                                        None
                                    }),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(d.form_name.clone()),
                            )
                            .children(match &d.deadline {
                                DeadlineKind::Dated {
                                    original_deadline,
                                    final_deadline,
                                } if original_deadline != final_deadline => Some(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!(
                                            "Originally due on: {}",
                                            original_deadline.format("%Y-%m-%d")
                                        )),
                                ),
                                DeadlineKind::EventBased {
                                    trigger,
                                    statutory_window,
                                } => Some(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{trigger} · {statutory_window}")),
                                ),
                                _ => None,
                            }),
                    );
                schedule_list = schedule_list.child(date_card);
            }

            // Ongoing obligations (event-based forms with no fixed date)
            if !event_based.is_empty() {
                schedule_list = schedule_list.child(
                    div()
                        .pt_4()
                        .pb_2()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().foreground)
                                .child("Ongoing Obligations"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("Event-based forms without fixed deadlines"),
                        ),
                );

                for d in &event_based {
                    let date_id = format!("evt-{}", d.form_code);

                    let date_card = div()
                        .id(format!("deadline-{}-{}", d.form_code, date_id))
                        .group("list-item")
                        .flex()
                        .items_center()
                        .gap_4()
                        .p_3()
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded_lg()
                        .cursor_default()
                        .child(
                            div()
                                .w(px(64.))
                                .h(px(64.))
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .bg(cx.theme().secondary)
                                .border_1()
                                .border_color(cx.theme().border)
                                .rounded_md()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(cx.theme().muted_foreground)
                                        .child("EVT"),
                                )
                                .child(
                                    div()
                                        .text_xl()
                                        .font_weight(FontWeight::BLACK)
                                        .text_color(cx.theme().muted_foreground)
                                        .child("--"),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_base()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().foreground)
                                                .child(d.display_form_no.clone()),
                                        )
                                        .child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .bg(cx.theme().secondary)
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child("Event Based"),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(d.form_name.clone()),
                                )
                                .children(match &d.deadline {
                                    DeadlineKind::EventBased {
                                        trigger,
                                        statutory_window,
                                    } => Some(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("{trigger} · {statutory_window}")),
                                    ),
                                    _ => None,
                                }),
                        );
                    schedule_list = schedule_list.child(date_card);
                }
            }
        }

        div()
            .w_full()
            .p_4()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_xl()
            .shadow_sm()
            .child(schedule_list)
    }
}

impl Render for UpcomingDeadlinesList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().flex_col().gap_4().child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().foreground)
                        .child("Upcoming Deadlines"),
                )
                .child(self.render_list_view(window, cx)),
        )
    }
}
