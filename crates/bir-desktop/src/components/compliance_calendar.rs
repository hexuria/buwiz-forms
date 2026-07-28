use bir_core::calendar_rules::{DeadlineKind, DeadlineStatus, ResolvedTaxDeadline};
use bir_core::db::BirNotice;
use chrono::{Datelike, Local, NaiveDate};
use gpui::prelude::*;
use gpui::*;
use gpui_component::popover::Popover;
use gpui_component::*;
use gpui_rsx::rsx;

use super::multi_select::{MultiSelect, MultiSelectEvent, MultiSelectOption, MultiSelectState};

pub enum ComplianceCalendarEvent {
    YearChanged { year: i32 },
}

impl EventEmitter<ComplianceCalendarEvent> for ComplianceCalendar {}

pub struct ComplianceCalendar {
    pub filters: std::collections::HashSet<String>,
    pub form_filter: Entity<MultiSelectState>,
    pub selected_date: Option<NaiveDate>,
    pub current_month: NaiveDate,

    // Extracted props so it can hold the data temporarily during render
    deadlines: Vec<ResolvedTaxDeadline>,
    announcements: Vec<BirNotice>,
    _subscriptions: Vec<Subscription>,
}

impl ComplianceCalendar {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let current_date = Local::now().date_naive();
        let current_month =
            NaiveDate::from_ymd_opt(current_date.year(), current_date.month(), 1).unwrap();

        let available_filters = bir_core::integration::all_form_codes();

        let mut filters = std::collections::HashSet::new();
        for filter in &available_filters {
            filters.insert(filter.clone());
        }

        let options: Vec<MultiSelectOption> = available_filters
            .iter()
            .map(|code| MultiSelectOption::new(code, code))
            .collect();

        let form_filter = cx.new(|cx| {
            MultiSelectState::new(options, window, cx)
                .placeholder("All Forms")
                .max_visible_chips(1)
                .drop_down(true)
                .max_visible_items(10)
        });

        form_filter.update(cx, |state, cx| {
            state.set_selected_ids(available_filters, cx);
        });

        let subscriptions = vec![cx.subscribe(
            &form_filter,
            |this: &mut Self, _entity, event: &MultiSelectEvent, cx| {
                this.filters.clear();
                for id in &event.selected {
                    this.filters.insert(id.clone());
                }
                cx.notify();
            },
        )];

        Self {
            filters,
            form_filter,
            selected_date: None,
            current_month,
            deadlines: Vec::new(),
            announcements: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn set_data(&mut self, deadlines: Vec<ResolvedTaxDeadline>, announcements: Vec<BirNotice>) {
        self.deadlines = deadlines;
        self.announcements = announcements;
    }

    fn filtered_deadlines(&self) -> Vec<&ResolvedTaxDeadline> {
        let mut filtered: Vec<_> = self
            .deadlines
            .iter()
            .filter(|d| self.filters.contains(&d.form_code))
            .collect();

        // Event-based deadlines have no calendar date; exclude from grid/schedule views
        filtered.retain(|d| d.final_deadline_date().is_some());

        if let Some(date) = self.selected_date {
            filtered.retain(|d| d.final_deadline_date() == Some(date));
        } else {
            // Only show deadlines for the currently viewed month if no specific date is selected
            filtered.retain(|d| {
                if let Some(date) = d.final_deadline_date() {
                    date.year() == self.current_month.year()
                        && date.month() == self.current_month.month()
                } else {
                    false
                }
            });
        }

        filtered
    }

    fn render_list_view(&self, _window: &Window, cx: &Context<Self>) -> gpui::Div {
        let deadlines = self.filtered_deadlines();
        let mut schedule_list = rsx! { <div flex flex_col gap_3 /> };

        if deadlines.is_empty() {
            schedule_list = schedule_list.child(rsx! {
                <div w_full flex flex_col items_center justify_center py_8 gap_2>
                    <div
                        text_sm
                        font_weight={FontWeight::MEDIUM}
                        text_color={cx.theme().muted_foreground}
                    >
                        {"No upcoming deadlines for selected filters."}
                    </div>
                </div>
            });
        } else {
            for d in deadlines {
                let has_update = self
                    .announcements
                    .iter()
                    .any(|a| a.title.contains(&d.form_code) || a.body.contains(&d.form_code));

                let deadline_date = d.final_deadline_date();
                let date_label = deadline_date
                    .map(|date| date.format("%b").to_string().to_uppercase())
                    .unwrap_or_else(|| "EVT".to_string());
                let day_label = deadline_date
                    .map(|date| date.format("%d").to_string())
                    .unwrap_or_else(|| "--".to_string());

                let date_card = rsx! {
                    <div
                        group={"list-item"}
                        flex
                        items_center
                        gap_4
                        p_3
                        bg={cx.theme().background}
                        border_1
                        border_color={cx.theme().border}
                        rounded_lg
                        cursor_pointer
                        hover={|s| {
                            s.bg(cx.theme().secondary)
                                .border_color(cx.theme().primary.opacity(0.5))
                        }}
                    >
                        <div
                            w={px(64.)}
                            h={px(64.)}
                            flex
                            flex_col
                            items_center
                            justify_center
                            bg={cx.theme().primary.opacity(0.08)}
                            border_1
                            border_color={cx.theme().primary.opacity(0.1)}
                            rounded_md
                        >
                            <div
                                text_xs
                                font_weight={FontWeight::BOLD}
                                text_color={cx.theme().primary}
                            >
                                {date_label}
                            </div> // month
                            <div
                                text_xl
                                font_weight={FontWeight::BLACK}
                                text_color={cx.theme().primary}
                            >
                                {day_label}
                            </div> // day
                        </div>
                        <div flex_1 flex flex_col gap_1>
                            <div flex items_center gap_2>
                                <div
                                    text_base
                                    font_weight={FontWeight::BOLD}
                                    text_color={cx.theme().foreground}
                                >
                                    {d.display_form_no.clone()}
                                </div>
                                {...if has_update {
                                    Some(rsx! {
                                        <div
                                            px_2
                                            py_0p5
                                            bg={cx.theme().warning.opacity(0.1)}
                                            border_1
                                            border_color={cx.theme().warning.opacity(0.2)}
                                            rounded_full
                                            flex
                                            items_center
                                            gap_1
                                        >
                                            <div
                                                text_xs
                                                font_weight={FontWeight::SEMIBOLD}
                                                text_color={cx.theme().warning}
                                            >
                                                {"Updated"}
                                            </div>
                                        </div>
                                    })
                                } else {
                                    None
                                }}
                                {...if d.status != DeadlineStatus::Normal {
                                    Some(rsx! {
                                        <div
                                            px_2
                                            py_0p5
                                            bg={cx.theme().primary.opacity(0.1)}
                                            border_1
                                            border_color={cx.theme().primary.opacity(0.2)}
                                            rounded_full
                                            flex
                                            items_center
                                            gap_1
                                        >
                                            <div
                                                text_xs
                                                font_weight={FontWeight::SEMIBOLD}
                                                text_color={cx.theme().primary}
                                            >
                                                {d.status.label()}
                                            </div>
                                        </div>
                                    })
                                } else {
                                    None
                                }}
                            </div>
                            <div text_sm text_color={cx.theme().muted_foreground}>
                                {d.form_name.clone()}
                            </div>
                            {...match &d.deadline {
                                DeadlineKind::Dated {
                                    original_deadline,
                                    final_deadline,
                                } if original_deadline != final_deadline => Some(rsx! {
                                    <div text_xs text_color={cx.theme().muted_foreground}>
                                        {format!(
                                            "Originally due on: {}",
                                            original_deadline.format("%Y-%m-%d")
                                        )}
                                    </div>
                                }),
                                DeadlineKind::EventBased {
                                    trigger,
                                    statutory_window,
                                } => Some(rsx! {
                                    <div text_xs text_color={cx.theme().muted_foreground}>
                                        {format!("{trigger} · {statutory_window}")}
                                    </div>
                                }),
                                _ => None,
                            }}
                        </div>
                    </div>
                };
                schedule_list = schedule_list.child(date_card);
            }
        }

        rsx! {
            <div
                w_full
                p_4
                bg={cx.theme().background}
                border_1
                border_color={cx.theme().border}
                rounded_xl
                shadow_sm
            >
                {schedule_list}
            </div>
        }
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

        let mut rows_container = rsx! {
            <div flex flex_col w_full border_t_1 border_l_1 border_color={cx.theme().border} />
        };

        // Header row
        let headers = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let mut header_row = rsx! { <div flex w_full /> };
        for h in headers {
            header_row = header_row.child(rsx! {
                <div
                    w={relative(1.0 / 7.0)}
                    py_2
                    bg={cx.theme().muted}
                    border_r_1
                    border_b_1
                    border_color={cx.theme().border}
                    text_xs
                    font_weight={FontWeight::BOLD}
                    text_color={cx.theme().muted_foreground}
                    text_center
                >
                    {h}
                </div>
            });
        }
        rows_container = rows_container.child(header_row);

        let total_used = start_weekday + days_in_month as u32;
        let trailing = (7 - (total_used % 7)) % 7;
        let all_cells = total_used + trailing;

        for week_start in (0..all_cells).step_by(7) {
            let mut week_row = rsx! { <div flex w_full /> };
            for slot in week_start..week_start + 7 {
                if slot < start_weekday || slot >= start_weekday + days_in_month as u32 {
                    week_row = week_row.child(rsx! {
                        <div
                            w={relative(1.0 / 7.0)}
                            aspect_ratio={1.0}
                            border_r_1
                            border_b_1
                            border_color={cx.theme().border}
                            bg={cx.theme().muted.opacity(0.3)}
                        />
                    });
                } else {
                    let day = (slot - start_weekday + 1) as i64;
                    let current_date = NaiveDate::from_ymd_opt(
                        self.current_month.year(),
                        self.current_month.month(),
                        day as u32,
                    )
                    .unwrap();

                    let day_deadlines: Vec<_> = deadlines
                        .iter()
                        .filter(|d| d.final_deadline_date() == Some(current_date))
                        .collect();

                    let is_today = today == current_date;
                    let is_selected = self.selected_date == Some(current_date);

                    let day_label = if is_selected {
                        rsx! {
                            <div
                                w={px(24.)}
                                h={px(24.)}
                                flex
                                items_center
                                justify_center
                                bg={cx.theme().primary}
                                text_color={cx.theme().primary_foreground}
                                rounded_full
                                text_xs
                                font_weight={FontWeight::BOLD}
                            >
                                {day.to_string()}
                            </div>
                        }
                    } else if is_today {
                        rsx! {
                            <div
                                w={px(24.)}
                                h={px(24.)}
                                flex
                                items_center
                                justify_center
                                border_1
                                border_color={cx.theme().primary}
                                text_color={cx.theme().primary}
                                rounded_full
                                text_xs
                                font_weight={FontWeight::BOLD}
                            >
                                {day.to_string()}
                            </div>
                        }
                    } else {
                        rsx! {
                            <div
                                w={px(24.)}
                                h={px(24.)}
                                flex
                                items_center
                                justify_center
                                text_color={cx.theme().foreground}
                                text_xs
                                font_weight={FontWeight::MEDIUM}
                            >
                                {day.to_string()}
                            </div>
                        }
                    };

                    let mut day_cell = rsx! {
                        <div
                            id={format!("day-{}-{}", self.current_month.month(), day)}
                            w={relative(1.0 / 7.0)}
                            aspect_ratio={1.0}
                            border_r_1
                            border_b_1
                            border_color={if is_selected {
                                cx.theme().primary.opacity(0.5)
                            } else {
                                cx.theme().border
                            }}
                            p_1
                            flex
                            flex_col
                            justify_between
                            bg={if is_selected {
                                cx.theme().primary.opacity(0.05)
                            } else if is_today {
                                cx.theme().primary.opacity(0.02)
                            } else {
                                cx.theme().background
                            }}
                        >
                            <div flex w_full justify_end>{day_label}</div>
                        </div>
                    };

                    if !day_deadlines.is_empty() {
                        let mut adjusted_deadline = current_date;
                        match current_date.weekday() {
                            chrono::Weekday::Sat => adjusted_deadline -= chrono::Duration::days(2),
                            chrono::Weekday::Sun => adjusted_deadline -= chrono::Duration::days(3),
                            _ => {}
                        }

                        let days_until = (adjusted_deadline - today).num_days();
                        let dot_color: gpui::Hsla = if days_until <= 1 {
                            gpui::rgba(0xef4444ff).into() // Missed or Urgent
                        } else if days_until < 7 {
                            gpui::rgba(0xf97316ff).into() // Warning (Orange)
                        } else {
                            cx.theme().foreground // Safe
                        };

                        let mut dots = rsx! {
                            <div flex flex_wrap gap_1 justify_center px_1 pb_1 />
                        };
                        for _ in day_deadlines.iter().take(4) {
                            dots = dots.child(
                                rsx! { <div w={px(6.)} h={px(6.)} rounded_full bg={dot_color} /> },
                            );
                        }
                        if day_deadlines.len() > 4 {
                            dots = dots.child(rsx! {
                                <div
                                    text_xs
                                    line_height={relative(1.0)}
                                    font_weight={FontWeight::BOLD}
                                    text_color={dot_color}
                                >
                                    {"+"}
                                </div>
                            });
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

        rsx! {
            <div
                w_full
                bg={cx.theme().background}
                border_1
                border_color={cx.theme().border}
                rounded_xl
                shadow_sm
                overflow_hidden
            >
                <div
                    flex
                    justify_between
                    items_center
                    p_4
                    border_b_1
                    border_color={cx.theme().border}
                    bg={cx.theme().background}
                >
                    <div
                        text_lg
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().foreground}
                    >
                        {self.current_month.format("%B %Y").to_string()}
                    </div>
                    <div flex gap_1 bg={cx.theme().muted.opacity(0.5)} rounded_lg p_1>
                        <div
                            id="prev-month"
                            px_3
                            py_1p5
                            rounded_md
                            cursor_pointer
                            hover={|s| s.bg(cx.theme().secondary)}
                            on_click={cx.listener(|this, _, _, cx| {
                                let old_year = this.current_month.year();
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
                                this.selected_date = None;
                                let new_year = this.current_month.year();
                                if new_year != old_year {
                                    cx.emit(ComplianceCalendarEvent::YearChanged {
                                        year: new_year,
                                    });
                                }
                                cx.notify();
                            })}
                        >
                            <div
                                text_sm
                                font_weight={FontWeight::BOLD}
                                text_color={cx.theme().foreground}
                            >
                                {"◀"}
                            </div>
                        </div>
                        <div
                            id="next-month"
                            px_3
                            py_1p5
                            rounded_md
                            cursor_pointer
                            hover={|s| s.bg(cx.theme().secondary)}
                            on_click={cx.listener(|this, _, _, cx| {
                                let old_year = this.current_month.year();
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
                                this.selected_date = None;
                                let new_year = this.current_month.year();
                                if new_year != old_year {
                                    cx.emit(ComplianceCalendarEvent::YearChanged {
                                        year: new_year,
                                    });
                                }
                                cx.notify();
                            })}
                        >
                            <div
                                text_sm
                                font_weight={FontWeight::BOLD}
                                text_color={cx.theme().foreground}
                            >
                                {"▶"}
                            </div>
                        </div>
                    </div>
                </div>
                {rows_container}
            </div>
        }
    }
}

impl Render for ComplianceCalendar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        rsx! {
            <div flex flex_col gap_4>
                <div flex flex_wrap gap_2 justify_between items_center>
                    <div
                        text_xl
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().foreground}
                    >
                        {"Compliance Calendar"}
                    </div>
                    <div flex gap_2 items_center>
                        // Filter Combobox
                        <div w={px(240.)}>{MultiSelect::new(&self.form_filter)}</div>
                    </div>
                </div>
                <div w_full flex justify_center>
                    {self.render_grid_view(window, cx)}
                </div>
                <div flex flex_col gap_2>
                    <div
                        text_lg
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().foreground}
                    >
                        {if let Some(date) = self.selected_date {
                            format!("Deadlines for {}", date.format("%b %d, %Y"))
                        } else {
                            format!("Deadlines for {}", self.current_month.format("%B %Y"))
                        }}
                    </div>
                    {self.render_list_view(window, cx)}
                </div>
            </div>
        }
    }
}
