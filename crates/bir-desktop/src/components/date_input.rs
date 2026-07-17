#![allow(dead_code)]
use chrono::Datelike;
use chrono::NaiveDate;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    calendar::{Calendar, CalendarEvent, CalendarState},
    input::{Input, InputEvent, InputState},
    v_flex,
};

pub struct DateInputEvent {
    pub date: Option<NaiveDate>,
}

pub struct DateInputState {
    pub input: Entity<InputState>,
    pub calendar: Entity<CalendarState>,
    pub focus_handle: FocusHandle,
    pub date: Option<NaiveDate>,
    pub open: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DateInputEvent> for DateInputState {}

impl DateInputState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("MM/DD/YYYY"));
        let calendar = cx.new(|cx| {
            let today = chrono::Local::now().naive_local().date();
            CalendarState::new(window, cx).year_range((1900, today.year() + 1))
        });
        let focus_handle = cx.focus_handle();

        let _subscriptions = vec![
            cx.subscribe_in(
                &input,
                window,
                |this, _input, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => {
                        let value = _input.read(cx).value().trim().to_string();
                        // Keep the parsed value synchronized with the text on
                        // every edit. Otherwise an invalid replacement can
                        // leave the previously valid date in memory and be
                        // saved silently by callers that read `date`.
                        this.date = Self::parse_date(&value);
                        // Keep the calendar in step here as well: the Blur
                        // set_date call may hit the no-op early-return when
                        // the typed text already matches the display format.
                        // Skip implausible years — chrono's %Y accepts 1-4
                        // digits, so mid-typing prefixes parse to years like
                        // 0002 and would strand the picker far outside its
                        // navigable range.
                        if let Some(d) = this.date
                            && chrono::Datelike::year(&d) >= 1900
                        {
                            this.calendar.update(cx, |cal, cx| {
                                cal.set_date(
                                    gpui_component::calendar::Date::Single(Some(d)),
                                    window,
                                    cx,
                                );
                            });
                        }
                        cx.emit(DateInputEvent { date: this.date });
                        cx.notify();
                    }
                    InputEvent::Blur | InputEvent::PressEnter { .. } => {
                        let value = _input.read(cx).value().trim().to_string();
                        if let Some(parsed) = Self::parse_date(&value) {
                            this.set_date(Some(parsed), window, cx);
                        } else if value.is_empty() {
                            this.set_date(None, window, cx);
                        } else {
                            this.date = None;
                            cx.emit(DateInputEvent { date: None });
                            cx.notify();
                        }
                    }
                    InputEvent::Focus => {
                        if let Some(date) = this.date {
                            let raw_value = date.format("%m/%d/%Y").to_string();
                            this.input.update(cx, |input, cx| {
                                input.set_value(raw_value, window, cx);
                            });
                        }
                    }
                    _ => {}
                },
            ),
            cx.subscribe_in(&calendar, window, |this, _cal, event, window, cx| {
                let CalendarEvent::Selected(date) = event;
                if let gpui_component::calendar::Date::Single(Some(d)) = date {
                    this.set_date(Some(*d), window, cx);
                    this.open = false;
                }
            }),
        ];

        Self {
            input,
            calendar,
            focus_handle,
            date: None,
            open: false,
            _subscriptions,
        }
    }

    fn parse_date(value: &str) -> Option<NaiveDate> {
        if value.is_empty() {
            return None;
        }
        // Try MMDDYYYY (e.g., 12102019)
        if value.len() == 8 && value.chars().all(|c| c.is_ascii_digit()) {
            return NaiveDate::parse_from_str(value, "%m%d%Y").ok();
        }
        // Try standard YYYY-MM-DD
        if let Ok(d) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
            return Some(d);
        }
        // Try MM/DD/YYYY
        if let Ok(d) = NaiveDate::parse_from_str(value, "%m/%d/%Y") {
            return Some(d);
        }
        // Try MM-DD-YYYY
        if let Ok(d) = NaiveDate::parse_from_str(value, "%m-%d-%Y") {
            return Some(d);
        }
        // The component itself displays selected values in this format.
        if let Ok(d) = NaiveDate::parse_from_str(value, "%b %d %Y") {
            return Some(d);
        }
        None
    }

    pub fn set_date(
        &mut self,
        date: Option<NaiveDate>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let display_value = date
            .map(|d| d.format("%b %d %Y").to_string())
            .unwrap_or_default();
        // A no-op must not emit DateInputEvent: programmatic re-loads (e.g.
        // re-opening the just-applied COR version) would otherwise mark the
        // profile dirty after its save was dispatched, permanently flagging
        // saved state as unsaved.
        if self.date == date {
            let current_text = self.input.read(cx).value().to_string();
            if current_text == display_value {
                return;
            }
            // Same date but different text — e.g. the focused "%m/%d/%Y"
            // editing format. Normalize the display without emitting;
            // subscribers treat an emission as a data change.
            if date.is_some() && Self::parse_date(current_text.trim()) == date {
                self.input.update(cx, |input, cx| {
                    input.set_value(display_value, window, cx);
                });
                if let Some(d) = date {
                    self.calendar.update(cx, |cal, cx| {
                        cal.set_date(
                            gpui_component::calendar::Date::Single(Some(d)),
                            window,
                            cx,
                        );
                    });
                }
                cx.notify();
                return;
            }
        }
        self.date = date;
        self.input.update(cx, |input, cx| {
            input.set_value(display_value, window, cx);
        });

        if let Some(d) = date {
            self.calendar.update(cx, |cal, cx| {
                cal.set_date(gpui_component::calendar::Date::Single(Some(d)), window, cx);
            });
        }

        cx.emit(DateInputEvent { date });
        cx.notify();
    }

    pub fn value(&self, cx: &gpui::App) -> String {
        self.input.read(cx).value().to_string()
    }

    pub fn has_invalid_value(&self, cx: &gpui::App) -> bool {
        !self.value(cx).trim().is_empty() && self.date.is_none()
    }

    pub fn set_value(&mut self, value: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(parsed) = Self::parse_date(&value) {
            self.set_date(Some(parsed), window, cx);
        } else {
            self.input.update(cx, |input, cx| {
                input.set_value(value, window, cx);
            });
            self.date = None;
            cx.notify();
        }
    }
}

impl Focusable for DateInputState {
    fn focus_handle(&self, cx: &gpui::App) -> gpui::FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for DateInputState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let input_view = self.input.clone();

        v_flex()
            .w_full()
            .child(
                div()
                    .relative()
                    .flex()
                    .w_full()
                    .items_center()
                    .child(div().flex_1().min_w_0().child(Input::new(&input_view)))
                    .child(
                        div()
                            .id("calendar_icon")
                            .absolute()
                            .right_2()
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open = !this.open;
                                cx.notify();
                            }))
                            .child(
                                Icon::new(IconName::Calendar)
                                    .small()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    ),
            )
            .when(self.open, |this| {
                this.child(
                    deferred(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .h_full()
                            .flex()
                            .justify_center()
                            .items_center()
                            .child(
                                div()
                                    .id("calendar_backdrop")
                                    .absolute()
                                    .inset_0()
                                    .bg(gpui::rgba(0x000000AA))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("calendar_container")
                                    .occlude()
                                    .on_click(|_, _, _| {}) // absorb click
                                    .p_4()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .shadow_lg()
                                    .rounded_lg()
                                    .bg(cx.theme().popover)
                                    .text_color(cx.theme().popover_foreground)
                                    .child(Calendar::new(&self.calendar)),
                            ),
                    )
                    .with_priority(2),
                )
            })
    }
}

#[derive(IntoElement)]
pub struct DateInput {
    state: Entity<DateInputState>,
}

impl DateInput {
    pub fn new(state: &Entity<DateInputState>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl RenderOnce for DateInput {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.state.clone().into_any_element()
    }
}
