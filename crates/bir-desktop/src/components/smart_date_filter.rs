use chrono::{Datelike, Local};
use gpui::prelude::*;
use gpui::*;
use gpui_component::ActiveTheme;

/// Event emitted when the user changes the selected year.
pub struct SmartDateFilterEvent {
    pub year: i32,
}

pub struct SmartDateFilterState {
    pub open: bool,
    pub selected_year: i32,
    pub focus_handle: FocusHandle,
}

impl EventEmitter<SmartDateFilterEvent> for SmartDateFilterState {}

impl SmartDateFilterState {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let now = Local::now().date_naive();
        Self {
            open: false,
            selected_year: now.year(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn toggle_open(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }

    fn display_label(&self) -> String {
        format!("Tax Year {}", self.selected_year)
    }
}

impl Render for SmartDateFilterState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.open;
        let label = self.display_label();
        let current_real_year = Local::now().date_naive().year();
        let selected_year = self.selected_year;
        let can_go_forward = selected_year < current_real_year;

        div()
            .relative()
            .child(
                div()
                    .id("year_trigger")
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1p5()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .hover(|s| s.bg(cx.theme().secondary))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_open(cx);
                    }))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .child(label),
                    ),
            )
            .when(is_open, |this| {
                this.child(
                    deferred(
                        anchored().snap_to_window_with_margin(px(8.)).child(
                            div()
                                .occlude()
                                .on_mouse_down_out(cx.listener(|this, _ev, _window, cx| {
                                    this.open = false;
                                    cx.notify();
                                }))
                                .mt_1p5()
                                .w(px(240.))
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_lg()
                                .rounded_xl()
                                .bg(cx.theme().popover)
                                .text_color(cx.theme().popover_foreground)
                                .flex()
                                .flex_col()
                                .p_3()
                                // Year navigation row
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .id("year_prev")
                                                .px_2()
                                                .py_1()
                                                .text_sm()
                                                .cursor_pointer()
                                                .text_color(cx.theme().muted_foreground)
                                                .hover(|s| s.text_color(cx.theme().foreground))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.selected_year -= 1;
                                                    cx.emit(SmartDateFilterEvent {
                                                        year: this.selected_year,
                                                    });
                                                    cx.notify();
                                                }))
                                                .child("◀"),
                                        )
                                        .child(
                                            div()
                                                .text_lg()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().foreground)
                                                .child(format!("{}", selected_year)),
                                        )
                                        .child(
                                            div()
                                                .id("year_next")
                                                .px_2()
                                                .py_1()
                                                .text_sm()
                                                .cursor_pointer()
                                                .when(can_go_forward, |s| {
                                                    s.text_color(cx.theme().muted_foreground)
                                                        .hover(|s| {
                                                            s.text_color(cx.theme().foreground)
                                                        })
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.selected_year += 1;
                                                            cx.emit(SmartDateFilterEvent {
                                                                year: this.selected_year,
                                                            });
                                                            cx.notify();
                                                        }))
                                                })
                                                .when(!can_go_forward, |s| {
                                                    s.text_color(cx.theme().border)
                                                })
                                                .child("▶"),
                                        ),
                                )
                                // Nearby year pills for quick selection
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_around()
                                        .gap_1()
                                        .pt_3()
                                        .children((0..5).rev().map(|offset| {
                                            let yr = current_real_year - offset;
                                            let is_active = yr == selected_year;
                                            div()
                                                .id(format!("yr_{}", yr))
                                                .flex_1()
                                                .flex()
                                                .justify_center()
                                                .py_1p5()
                                                .text_xs()
                                                .font_weight(if is_active {
                                                    FontWeight::BOLD
                                                } else {
                                                    FontWeight::MEDIUM
                                                })
                                                .rounded_md()
                                                .cursor_pointer()
                                                .when(is_active, |s| {
                                                    s.bg(cx.theme().primary)
                                                        .text_color(cx.theme().primary_foreground)
                                                })
                                                .when(!is_active, |s| {
                                                    s.text_color(cx.theme().muted_foreground)
                                                        .hover(|s| s.bg(cx.theme().secondary))
                                                })
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.selected_year = yr;
                                                    cx.emit(SmartDateFilterEvent { year: yr });
                                                    this.open = false;
                                                    cx.notify();
                                                }))
                                                .child(format!("{}", yr))
                                        })),
                                ),
                        ),
                    )
                    .with_priority(2),
                )
            })
    }
}

#[derive(IntoElement)]
pub struct SmartDateFilter {
    state: Entity<SmartDateFilterState>,
}

impl SmartDateFilter {
    pub fn new(state: &Entity<SmartDateFilterState>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl RenderOnce for SmartDateFilter {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.state.clone().into_any_element()
    }
}
