#![allow(dead_code)]
use gpui::*;
use gpui_component::input::*;

pub struct CurrencyInput {
    pub label: String,
    pub input_state: Entity<InputState>,
}

impl CurrencyInput {
    pub fn new(
        label: impl Into<String>,
        amount: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let amount = amount.into();
        Self {
            label: label.into(),
            input_state: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(amount)
                    .mask_pattern(MaskPattern::Number {
                        separator: Some(','),
                        fraction: Some(2),
                    })
            }),
        }
    }
}

impl Render for CurrencyInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        use gpui_component::ActiveTheme as _;
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child(self.label.clone()),
            )
            .child(
                Input::new(&self.input_state).text_right().prefix(
                    div()
                        .p_2()
                        .border_r_1()
                        .border_color(cx.theme().border)
                        .text_color(cx.theme().muted_foreground)
                        .child("₱"),
                ),
            )
    }
}
