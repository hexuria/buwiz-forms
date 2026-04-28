use bir_core::db::{BirNotice, TaxDeadline};
use chrono::{Datelike, Local, NaiveDate};
use gpui::prelude::*;
use gpui::*;
use gpui_component::*;

pub struct ComplianceCalendar {
    pub filter_open: bool,
    pub filters: std::collections::HashSet<String>,
    pub available_filters: Vec<String>,
    pub selected_date: Option<NaiveDate>,
    pub current_month: NaiveDate,

    // Extracted props so it can hold the data temporarily during render
    deadlines: Vec<TaxDeadline>,
    announcements: Vec<BirNotice>,
}

impl ComplianceCalendar {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let current_date = Local::now().date_naive();
        let current_month =
            NaiveDate::from_ymd_opt(current_date.year(), current_date.month(), 1).unwrap();

        let mut filters = std::collections::HashSet::new();
        filters.insert("All".to_string());

        let available_filters = bir_core::forms::registry::FORM_REGISTRY
            .iter()
            .map(|f| f.code.to_string())
            .collect();

        Self {
            filter_open: false,
            filters,
            available_filters,
            selected_date: None,
            current_month,
            deadlines: Vec::new(),
            announcements: Vec::new(),
        }
    }

    pub fn set_data(&mut self, deadlines: Vec<TaxDeadline>, announcements: Vec<BirNotice>) {
        self.deadlines = deadlines;
        self.announcements = announcements;
    }

    fn toggle_filter(&mut self, filter: &str, cx: &mut Context<Self>) {
        if filter == "All" {
            self.filters.clear();
            self.filters.insert("All".to_string());
        } else {
            self.filters.remove("All");
            if self.filters.contains(filter) {
                self.filters.remove(filter);
                if self.filters.is_empty() {
                    self.filters.insert("All".to_string());
                }
            } else {
                self.filters.insert(filter.to_string());
            }
        }
        cx.notify();
    }

    fn render_filter_combobox(&self, cx: &mut Context<Self>) -> gpui::Div {
        let selected_text = if self.filters.contains("All") {
            "All Forms".to_string()
        } else {
            let count = self.filters.len();
            if count == 1 {
                self.filters.iter().next().unwrap().clone()
            } else {
                format!("{} selected", count)
            }
        };

        let filter_button = div()
            .id("filter-combobox")
            .px_3()
            .py_1p5()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .hover(|s| s.bg(cx.theme().secondary))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child(selected_text),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().muted_foreground)
                    .child("▼"),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.filter_open = !this.filter_open;
                cx.notify();
            }));

        let dropdown = if self.filter_open {
            let mut options_list = div().flex().flex_col().w_full();

            // "All" option
            let is_all = self.filters.contains("All");
            options_list = options_list.child(
                div()
                    .id("filter-all")
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap_2()
                    .hover(|s| s.bg(cx.theme().secondary))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_filter("All", cx);
                    }))
                    .child(
                        div()
                            .w(px(16.))
                            .h(px(16.))
                            .border_1()
                            .border_color(if is_all {
                                cx.theme().primary
                            } else {
                                cx.theme().border
                            })
                            .bg(if is_all {
                                cx.theme().primary
                            } else {
                                cx.theme().background
                            })
                            .rounded_sm()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(if is_all {
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().primary_foreground)
                                    .child("✓")
                            } else {
                                div()
                            }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child("All Forms"),
                    ),
            );

            // Other options
            for (i, opt) in self.available_filters.iter().enumerate() {
                let is_selected = self.filters.contains(opt);
                let opt_clone = opt.clone();
                options_list = options_list.child(
                    div()
                        .id(("filter-opt", i))
                        .px_3()
                        .py_2()
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_2()
                        .hover(|s| s.bg(cx.theme().secondary))
                        .on_click(cx.listener({
                            let opt_clone = opt_clone.clone();
                            move |this, _, _, cx| {
                                this.toggle_filter(&opt_clone, cx);
                            }
                        }))
                        .child(
                            div()
                                .w(px(16.))
                                .h(px(16.))
                                .border_1()
                                .border_color(if is_selected {
                                    cx.theme().primary
                                } else {
                                    cx.theme().border
                                })
                                .bg(if is_selected {
                                    cx.theme().primary
                                } else {
                                    cx.theme().background
                                })
                                .rounded_sm()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(if is_selected {
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().primary_foreground)
                                        .child("✓")
                                } else {
                                    div()
                                }),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().foreground)
                                .child(opt.clone()),
                        ),
                );
            }

            deferred(
                anchored().snap_to_window_with_margin(px(8.)).child(
                    div()
                        .occlude()
                        .mt_1p5()
                        .w(px(200.))
                        .border_1()
                        .border_color(cx.theme().border)
                        .shadow_lg()
                        .rounded_md()
                        .bg(cx.theme().popover)
                        .child(options_list),
                ),
            )
            .with_priority(2)
            .into_any_element()
        } else {
            div().into_any_element()
        };

        div().relative().child(filter_button).child(dropdown)
    }

    fn filtered_deadlines(&self) -> Vec<&TaxDeadline> {
        let mut filtered: Vec<_> = if self.filters.contains("All") {
            self.deadlines.iter().collect()
        } else {
            self.deadlines
                .iter()
                .filter(|d| self.filters.contains(&d.form_type))
                .collect()
        };

        if let Some(date) = self.selected_date {
            let date_str = date.format("%Y-%m-%d").to_string();
            filtered.retain(|d| d.due_date == date_str);
        }

        filtered
    }

    fn render_list_view(&self, _window: &Window, cx: &Context<Self>) -> gpui::Div {
        let deadlines = self.filtered_deadlines();
        let mut schedule_list = div().flex().flex_col().gap_3();

        if deadlines.is_empty() {
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
            for d in deadlines {
                let has_update = self
                    .announcements
                    .iter()
                    .any(|a| a.title.contains(&d.form_type) || a.body.contains(&d.form_type));

                let date_card = div()
                    .group("list-item")
                    .flex()
                    .items_center()
                    .gap_4()
                    .p_3()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_lg()
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(cx.theme().secondary)
                            .border_color(cx.theme().primary.opacity(0.5))
                    })
                    .child(
                        div()
                            .w(px(64.))
                            .h(px(64.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .bg(cx.theme().primary.opacity(0.08))
                            .border_1()
                            .border_color(cx.theme().primary.opacity(0.1))
                            .rounded_md()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().primary)
                                    .child(
                                        chrono::NaiveDate::parse_from_str(&d.due_date, "%Y-%m-%d")
                                            .map(|date| {
                                                date.format("%b").to_string().to_uppercase()
                                            })
                                            .unwrap_or_else(|_| {
                                                d.due_date[5..7].to_string().to_uppercase()
                                            }),
                                    ),
                            ) // month
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BLACK)
                                    .text_color(cx.theme().primary)
                                    .child(d.due_date[8..10].to_string()),
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
                                            .child(d.form_type.clone()),
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
                                    }),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(d.description.clone()),
                            ),
                    );
                schedule_list = schedule_list.child(date_card);
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
    fn render_grid_view(&self, _window: &Window, cx: &Context<Self>) -> gpui::Div {
        let deadlines = self.filtered_deadlines();

        let first_day =
            NaiveDate::from_ymd_opt(self.current_month.year(), self.current_month.month(), 1)
                .unwrap();
        let start_weekday = first_day.weekday().num_days_from_sunday();

        let days_in_month = if self.current_month.month() == 12 {
            31
        } else {
            let next_month = NaiveDate::from_ymd_opt(
                self.current_month.year(),
                self.current_month.month() + 1,
                1,
            )
            .unwrap();
            (next_month - first_day).num_days()
        };

        let today = chrono::Local::now().date_naive();

        let mut rows_container = div()
            .flex()
            .flex_col()
            .w_full()
            .border_t_1()
            .border_l_1()
            .border_color(cx.theme().border);

        // Header row
        let headers = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let mut header_row = div().flex().w_full();
        for h in headers {
            header_row = header_row.child(
                div()
                    .w(relative(1.0 / 7.0))
                    .py_2()
                    .bg(cx.theme().muted)
                    .border_r_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().muted_foreground)
                    .text_center()
                    .child(h),
            );
        }
        rows_container = rows_container.child(header_row);

        let total_used = start_weekday + days_in_month as u32;
        let trailing = (7 - (total_used % 7)) % 7;
        let all_cells = total_used + trailing;

        for week_start in (0..all_cells).step_by(7) {
            let mut week_row = div().flex().w_full();
            for slot in week_start..week_start + 7 {
                if slot < start_weekday || slot >= start_weekday + days_in_month as u32 {
                    week_row = week_row.child(
                        div()
                            .w(relative(1.0 / 7.0))
                            .aspect_ratio(1.0)
                            .border_r_1()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().muted.opacity(0.3)),
                    );
                } else {
                    let day = (slot - start_weekday + 1) as i64;
                    let date_str = format!(
                        "{:04}-{:02}-{:02}",
                        self.current_month.year(),
                        self.current_month.month(),
                        day
                    );

                    let current_date = NaiveDate::from_ymd_opt(
                        self.current_month.year(),
                        self.current_month.month(),
                        day as u32,
                    )
                    .unwrap();

                    let day_deadlines: Vec<_> = deadlines
                        .iter()
                        .filter(|d| d.due_date == date_str)
                        .collect();

                    let is_today = today == current_date;
                    let is_selected = self.selected_date == Some(current_date);

                    let day_label = if is_selected {
                        div()
                            .w(px(24.))
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(cx.theme().primary)
                            .text_color(cx.theme().primary_foreground)
                            .rounded_full()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .child(day.to_string())
                    } else if is_today {
                        div()
                            .w(px(24.))
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_1()
                            .border_color(cx.theme().primary)
                            .text_color(cx.theme().primary)
                            .rounded_full()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .child(day.to_string())
                    } else {
                        div()
                            .w(px(24.))
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(cx.theme().foreground)
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .child(day.to_string())
                    };

                    let mut day_cell = div()
                        .id(format!("day-{}-{}", self.current_month.month(), day))
                        .w(relative(1.0 / 7.0))
                        .aspect_ratio(1.0)
                        .border_r_1()
                        .border_b_1()
                        .border_color(if is_selected {
                            cx.theme().primary.opacity(0.5)
                        } else {
                            cx.theme().border
                        })
                        .p_1()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .bg(if is_selected {
                            cx.theme().primary.opacity(0.05)
                        } else if is_today {
                            cx.theme().primary.opacity(0.02)
                        } else {
                            cx.theme().background
                        })
                        .child(div().flex().w_full().justify_end().child(day_label));

                    if !day_deadlines.is_empty() {
                        let mut adjusted_deadline = current_date;
                        match current_date.weekday() {
                            chrono::Weekday::Sat => adjusted_deadline -= chrono::Duration::days(2),
                            chrono::Weekday::Sun => adjusted_deadline -= chrono::Duration::days(3),
                            _ => {}
                        }

                        let days_until = (adjusted_deadline - today).num_days();
                        let dot_color: gpui::Hsla = if days_until < 0 {
                            gpui::rgba(0xef4444ff).into() // Missed
                        } else if days_until <= 1 {
                            gpui::rgba(0xef4444ff).into() // Urgent
                        } else if days_until < 7 {
                            gpui::rgba(0xf97316ff).into() // Warning (Orange)
                        } else {
                            cx.theme().foreground // Safe
                        };

                        let mut dots = div()
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .justify_center()
                            .px_1()
                            .pb_1();
                        for _ in day_deadlines.iter().take(4) {
                            dots =
                                dots.child(div().w(px(6.)).h(px(6.)).rounded_full().bg(dot_color));
                        }
                        if day_deadlines.len() > 4 {
                            dots = dots.child(
                                div()
                                    .text_xs()
                                    .line_height(relative(1.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(dot_color)
                                    .child("+"),
                            );
                        }

                        day_cell = day_cell.child(dots);
                    } else {
                        day_cell = day_cell.child(div()); // Empty spacer for justify_between
                    }

                    day_cell = day_cell
                        .cursor_pointer()
                        .hover(|s| s.bg(cx.theme().secondary))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.selected_date == Some(current_date) {
                                this.selected_date = None;
                            } else {
                                this.selected_date = Some(current_date);
                            }
                            cx.notify();
                        }));

                    week_row = week_row.child(day_cell);
                }
            }
            rows_container = rows_container.child(week_row);
        }

        div()
            .w_full()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_xl()
            .shadow_sm()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .p_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(self.current_month.format("%B %Y").to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .bg(cx.theme().muted.opacity(0.5))
                            .rounded_lg()
                            .p_1()
                            .child(
                                div()
                                    .id("prev-month")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(cx.theme().secondary))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.current_month.month() == 1 {
                                            this.current_month = NaiveDate::from_ymd_opt(
                                                this.current_month.year() - 1,
                                                12,
                                                1,
                                            )
                                            .unwrap();
                                        } else {
                                            this.current_month = NaiveDate::from_ymd_opt(
                                                this.current_month.year(),
                                                this.current_month.month() - 1,
                                                1,
                                            )
                                            .unwrap();
                                        }
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child("◀"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("next-month")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(cx.theme().secondary))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.current_month.month() == 12 {
                                            this.current_month = NaiveDate::from_ymd_opt(
                                                this.current_month.year() + 1,
                                                1,
                                                1,
                                            )
                                            .unwrap();
                                        } else {
                                            this.current_month = NaiveDate::from_ymd_opt(
                                                this.current_month.year(),
                                                this.current_month.month() + 1,
                                                1,
                                            )
                                            .unwrap();
                                        }
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child("▶"),
                                    ),
                            ),
                    ),
            )
            .child(rows_container)
    }
}

impl Render for ComplianceCalendar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .child("Compliance Calendar"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            // Filter Combobox
                            .child(self.render_filter_combobox(cx)),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .child(self.render_grid_view(window, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(if let Some(date) = self.selected_date {
                                format!("Deadlines for {}", date.format("%b %d, %Y"))
                            } else {
                                "Upcoming Deadlines".to_string()
                            }),
                    )
                    .child(self.render_list_view(window, cx)),
            )
    }
}
