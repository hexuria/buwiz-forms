use bir_core::db::{BirNotice, TaxDeadline};
use chrono::{Datelike, Local, NaiveDate};
use gpui::prelude::*;
use gpui::*;
use gpui_component::*;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum CalendarMode {
    List,
    Grid,
}

pub struct ComplianceCalendar {
    pub mode: CalendarMode,
    pub filter_open: bool,
    pub filters: std::collections::HashSet<String>,
    pub available_filters: Vec<String>,
    pub current_month: NaiveDate,

    // Extracted props so it can hold the data temporarily during render
    deadlines: Vec<TaxDeadline>,
    announcements: Vec<BirNotice>,
}

impl ComplianceCalendar {
    pub fn new(_cx: &mut Context<Self>) -> Self {
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
            mode: CalendarMode::List,
            filter_open: false,
            filters,
            available_filters,
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
            .py_1()
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
        if self.filters.contains("All") {
            self.deadlines.iter().collect()
        } else {
            self.deadlines
                .iter()
                .filter(|d| self.filters.contains(&d.form_type))
                .collect()
        }
    }

    fn render_list_view(&self, cx: &Context<Self>) -> gpui::Div {
        let deadlines = self.filtered_deadlines();
        let mut schedule_list = div().flex().flex_col().gap_3();

        if deadlines.is_empty() {
            schedule_list = schedule_list.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("No upcoming deadlines for selected filters."),
            );
        } else {
            for d in deadlines {
                let has_update = self
                    .announcements
                    .iter()
                    .any(|a| a.title.contains(&d.form_type) || a.body.contains(&d.form_type));

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
                        div()
                            .w(px(60.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .p_2()
                            .bg(cx.theme().primary.opacity(0.1))
                            .rounded_md()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().primary)
                                    .child(d.due_date[5..7].to_string()),
                            ) // month
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
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
                                            .text_sm()
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
                                                .rounded_md()
                                                .text_xs()
                                                .text_color(cx.theme().warning)
                                                .child("Updated"),
                                        )
                                    } else {
                                        None
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
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

    fn render_grid_view(&self, cx: &Context<Self>) -> gpui::Div {
        let deadlines = self.filtered_deadlines();

        // Calculate the start day of the week for `self.current_month`
        let first_day =
            NaiveDate::from_ymd_opt(self.current_month.year(), self.current_month.month(), 1)
                .unwrap();
        let start_weekday = first_day.weekday().num_days_from_sunday(); // 0 = Sunday

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

        // Use explicit rows instead of flex-wrap to avoid fractional pixel rounding gaps
        let mut rows_container = div().flex().flex_col().w_full();

        // Header row
        let headers = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let mut header_row = div()
            .flex()
            .w_full()
            .border_t_1()
            .border_color(cx.theme().border);
        for h in headers {
            header_row = header_row.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .p_2()
                    .bg(cx.theme().muted)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().muted_foreground)
                    .text_center()
                    .border_r_1()
                    .border_b_1()
                    .border_l_1()
                    .border_color(cx.theme().border)
                    .child(h),
            );
        }
        rows_container = rows_container.child(header_row);

        // Calculate total cells needed: leading blanks + days + trailing blanks to complete last row
        let total_used = start_weekday + days_in_month as u32;
        let trailing = (7 - (total_used % 7)) % 7;
        let all_cells = total_used + trailing;

        // Render in chunks of 7 (one row per week)
        for week_start in (0..all_cells).step_by(7) {
            let mut week_row = div().flex().w_full();
            for slot in week_start..week_start + 7 {
                if slot < start_weekday || slot >= start_weekday + days_in_month as u32 {
                    // Blank cell
                    week_row = week_row.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h(px(80.))
                            .bg(cx.theme().muted.opacity(0.3))
                            .border_r_1()
                            .border_b_1()
                            .border_l_1()
                            .border_color(cx.theme().border),
                    );
                } else {
                    let day = (slot - start_weekday + 1) as i64;
                    let date_str = format!(
                        "{:04}-{:02}-{:02}",
                        self.current_month.year(),
                        self.current_month.month(),
                        day
                    );

                    // Find deadlines for this day
                    let day_deadlines: Vec<_> = deadlines
                        .iter()
                        .filter(|d| d.due_date == date_str)
                        .collect();

                    let mut day_cell = div()
                        .flex_1()
                        .min_w_0()
                        .h(px(80.))
                        .p_2()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .bg(cx.theme().background)
                        .border_r_1()
                        .border_b_1()
                        .border_l_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().foreground)
                                .child(day.to_string()),
                        );

                    if !day_deadlines.is_empty() {
                        // Render dots
                        let mut dots = div().flex().flex_wrap().gap_1();
                        for _ in &day_deadlines {
                            dots = dots.child(
                                div()
                                    .w(px(6.))
                                    .h(px(6.))
                                    .rounded_full()
                                    .bg(cx.theme().primary),
                            );
                        }

                        day_cell = day_cell
                            .child(dots)
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().secondary));
                    }

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
                            .gap_2()
                            .child(
                                div()
                                    .id("prev-month")
                                    .p_1()
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
                                    .child("◀"),
                            )
                            .child(
                                div()
                                    .id("next-month")
                                    .p_1()
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
                                    .child("▶"),
                            ),
                    ),
            )
            .child(rows_container)
    }
}

impl Render for ComplianceCalendar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .gap_4()
                            .items_center()
                            // View Toggle
                            .child(
                                div()
                                    .flex()
                                    .bg(cx.theme().muted)
                                    .p_1()
                                    .rounded_md()
                                    .child(
                                        div()
                                            .id("mode-list")
                                            .px_3()
                                            .py_1()
                                            .rounded_sm()
                                            .cursor_pointer()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if self.mode == CalendarMode::List {
                                                cx.theme().foreground
                                            } else {
                                                cx.theme().muted_foreground
                                            })
                                            .bg(if self.mode == CalendarMode::List {
                                                cx.theme().background
                                            } else {
                                                gpui::transparent_black()
                                            })
                                            .when(self.mode == CalendarMode::List, |s| {
                                                s.shadow_sm()
                                            })
                                            .child("List")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mode = CalendarMode::List;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id("mode-grid")
                                            .px_3()
                                            .py_1()
                                            .rounded_sm()
                                            .cursor_pointer()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if self.mode == CalendarMode::Grid {
                                                cx.theme().foreground
                                            } else {
                                                cx.theme().muted_foreground
                                            })
                                            .bg(if self.mode == CalendarMode::Grid {
                                                cx.theme().background
                                            } else {
                                                gpui::transparent_black()
                                            })
                                            .when(self.mode == CalendarMode::Grid, |s| {
                                                s.shadow_sm()
                                            })
                                            .child("Grid")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mode = CalendarMode::Grid;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            // Filter Combobox
                            .child(self.render_filter_combobox(cx)),
                    ),
            )
            .child(if self.mode == CalendarMode::List {
                self.render_list_view(cx)
            } else {
                self.render_grid_view(cx)
            })
    }
}
