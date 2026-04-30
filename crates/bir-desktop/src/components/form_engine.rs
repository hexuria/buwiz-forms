use bir_core::forms::FilingStatus;
use gpui::*;
use gpui_component::*;

#[allow(dead_code)]
pub trait FormViewTrait: 'static + Sized {
    /// Provide the form title, e.g., "BIR Form No. 2551Q"
    fn form_title(&self) -> &'static str;
    /// Provide the form subtitle, e.g., "Quarterly Percentage Tax Return"
    fn form_subtitle(&self) -> &'static str;
    /// Provide the version/date, e.g., "January 2018 (ENCS)"
    fn form_version(&self) -> &'static str;

    /// Get the current status
    fn current_status(&self) -> FilingStatus;
    /// Get submission date if any
    fn submitted_at(&self) -> Option<&str>;
    /// Get confirmation date if any
    fn confirmed_at(&self) -> Option<&str>;

    // Core actions that forms must implement
    fn save_draft(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn mark_paid(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn preview_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn print_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>);

    /// Render the standardized status pipeline (Draft -> Queued -> Submitted -> Confirmed -> Paid)
    fn render_status_pipeline(&self, cx: &Context<Self>) -> gpui::Div {
        let steps: [(&str, FilingStatus, u8); 5] = [
            ("Draft", FilingStatus::Draft, 1),
            ("Queued", FilingStatus::Queued, 2),
            ("Submitted", FilingStatus::Submitted, 3),
            ("Confirmed", FilingStatus::Confirmed, 4),
            ("Paid", FilingStatus::Paid, 5),
        ];

        let step_order = |s: &FilingStatus| -> u8 {
            match s {
                FilingStatus::Draft => 0,
                FilingStatus::Queued => 1,
                FilingStatus::Submitted => 2,
                FilingStatus::Confirmed => 3,
                FilingStatus::Paid => 4,
            }
        };
        let current_idx = step_order(&self.current_status());

        let mut row = div().flex().items_center().w_full().justify_center();

        for (i, (label, step_status, step_num)) in steps.iter().enumerate() {
            let idx = step_order(step_status);
            let is_current = idx == current_idx;
            let is_completed = idx < current_idx;

            // Connector line
            if i > 0 {
                if idx <= current_idx {
                    row = row.child(
                        div()
                            .flex_1()
                            .max_w(px(80.))
                            .h(px(2.))
                            .bg(cx.theme().success.opacity(0.5)),
                    );
                } else {
                    row = row.child(
                        div()
                            .flex_1()
                            .max_w(px(80.))
                            .border_t_2()
                            .border_dashed()
                            .border_color(cx.theme().muted_foreground),
                    );
                }
            }

            // Circle content
            let circle_content = if is_completed {
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().success_foreground)
                    .child("✓")
                    .into_any_element()
            } else {
                let num_color = if is_current {
                    match step_status {
                        FilingStatus::Draft => cx.theme().warning_foreground,
                        FilingStatus::Queued => cx.theme().primary_foreground,
                        FilingStatus::Submitted => cx.theme().info_foreground,
                        FilingStatus::Confirmed => cx.theme().success_foreground,
                        FilingStatus::Paid => cx.theme().success_foreground,
                    }
                } else {
                    cx.theme().muted_foreground
                };
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(num_color)
                    .child(format!("{}", step_num))
                    .into_any_element()
            };

            // Circle colors
            let (circle_bg, circle_border) = if is_completed {
                (cx.theme().success, cx.theme().success)
            } else if is_current {
                let color = match step_status {
                    FilingStatus::Draft => cx.theme().warning,
                    FilingStatus::Queued => cx.theme().primary,
                    FilingStatus::Submitted => cx.theme().info,
                    FilingStatus::Confirmed => cx.theme().success,
                    FilingStatus::Paid => cx.theme().success,
                };
                (color, color)
            } else {
                (gpui::rgba(0x00000000).into(), cx.theme().muted_foreground)
            };

            // Label color
            let label_color = if is_completed {
                cx.theme().success
            } else if is_current {
                match step_status {
                    FilingStatus::Draft => cx.theme().warning,
                    FilingStatus::Queued => cx.theme().primary,
                    FilingStatus::Submitted => cx.theme().info,
                    FilingStatus::Confirmed => cx.theme().success,
                    FilingStatus::Paid => cx.theme().success,
                }
            } else {
                cx.theme().muted_foreground
            };

            let mut circle = div()
                .size(px(28.))
                .rounded_full()
                .border_2()
                .border_color(circle_border)
                .bg(circle_bg)
                .flex()
                .items_center()
                .justify_center()
                .child(circle_content);

            if !is_completed && !is_current {
                circle = circle.border_dashed();
            }

            let mut step_group = div()
                .id(format!("step_{}", idx))
                .flex()
                .items_center()
                .gap_2();

            step_group = step_group.child(circle).child(
                div()
                    .text_xs()
                    .text_color(label_color)
                    .font_weight(if is_current {
                        FontWeight::BOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .child(*label),
            );

            if matches!(step_status, FilingStatus::Submitted)
                || matches!(step_status, FilingStatus::Confirmed)
            {
                let date_opt = match step_status {
                    FilingStatus::Submitted => self.submitted_at(),
                    FilingStatus::Confirmed => self.confirmed_at(),
                    _ => None,
                };
                if let Some(date_str) = date_opt {
                    let formatted_date =
                        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_str) {
                            dt.format("%B %e, %Y %I:%M %p").to_string()
                        } else if let Ok(dt) =
                            chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S")
                        {
                            dt.format("%B %e, %Y %I:%M %p").to_string()
                        } else {
                            date_str.to_string()
                        };

                    let tooltip_text = match step_status {
                        FilingStatus::Submitted => format!("Submitted on {}", formatted_date),
                        FilingStatus::Confirmed => format!("Confirmed on {}", formatted_date),
                        _ => unreachable!(),
                    };

                    step_group = step_group.tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(tooltip_text.clone())
                            .build(window, cx)
                    });
                }
            }

            row = row.child(step_group);
        }

        row
    }

    /// Render the standardized header with title and actions
    fn render_header(&self, cx: &Context<Self>) -> gpui::Div {
        let mut action_buttons = div().flex().justify_end().gap_2().w_full();

        if matches!(
            self.current_status(),
            FilingStatus::Confirmed | FilingStatus::Paid
        ) {
            action_buttons = action_buttons.child(
                gpui_component::button::Button::new("print_confirmation_btn")
                    .label("View Confirmation")
                    .outline()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.print_confirmation(window, cx);
                    })),
            );
        }

        // Dev-only: Inspect button (saves first, then previews) — stripped from release builds
        #[cfg(feature = "dev-tools")]
        {
            action_buttons = action_buttons.child(
                gpui_component::button::Button::new("inspect_btn")
                    .label("Inspect")
                    .icon(
                        gpui_component::Icon::empty()
                            .path("svg/target.svg")
                            .small(),
                    )
                    .outline()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.save_draft(window, cx);
                        this.preview_pdf(window, cx);
                    })),
            );
        }

        // Production: PDF Viewer only available after submission
        if !matches!(
            self.current_status(),
            FilingStatus::Draft | FilingStatus::Queued
        ) {
            action_buttons = action_buttons.child(
                gpui_component::button::Button::new("print_btn")
                    .label("PDF Viewer")
                    .icon(
                        gpui_component::Icon::empty()
                            .path("svg/printer.svg")
                            .small(),
                    )
                    .outline()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.preview_pdf(window, cx);
                    })),
            );
        }

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(action_buttons)
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BLACK)
                            .text_color(cx.theme().foreground)
                            .child(self.form_title()),
                    )
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child(self.form_subtitle()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.form_version()),
                    ),
            )
    }
}
