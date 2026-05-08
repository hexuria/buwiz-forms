use gpui::*;
use gpui_component::input::{Input, InputEvent as ComponentInputEvent, InputState};
use gpui_component::*;

pub struct TinInput {
    pub inputs: [Entity<InputState>; 4],
    prev_lengths: [usize; 4],
}

impl EventEmitter<ComponentInputEvent> for TinInput {}

impl TinInput {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let inputs = [
            cx.new(|cx| InputState::new(window, cx).placeholder("000")),
            cx.new(|cx| InputState::new(window, cx).placeholder("000")),
            cx.new(|cx| InputState::new(window, cx).placeholder("000")),
            cx.new(|cx| InputState::new(window, cx).placeholder("00000")),
        ];

        let mut _self = Self {
            inputs: inputs.clone(),
            prev_lengths: [0; 4],
        };

        for (i, input) in inputs.iter().enumerate() {
            let input_clone = input.clone();
            cx.subscribe_in(
                &input_clone,
                window,
                move |this: &mut Self, _entity, event, window, cx| {
                    if matches!(event, ComponentInputEvent::Change) {
                        this.on_change(i, window, cx);
                    }
                },
            )
            .detach();
        }

        _self
    }

    fn on_change(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.inputs[index].read(cx).value().to_string();
        let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
        let max_len = if index == 3 { 5 } else { 3 };

        // Handle paste across all boxes
        if index == 0 && digits.len() > 3 {
            let mut remaining = digits.clone();
            for i in 0..4 {
                let chunk_size = if i == 3 { 5 } else { 3 };
                let take = remaining.chars().take(chunk_size).collect::<String>();
                self.inputs[i].update(cx, |input, cx| {
                    input.set_value(take.clone(), window, cx);
                });
                self.prev_lengths[i] = take.len();
                if remaining.len() > chunk_size {
                    remaining = remaining[chunk_size..].to_string();
                } else {
                    remaining = String::new();
                }
            }
            let target = if digits.len() >= 9 {
                3
            } else if digits.len() >= 6 {
                2
            } else if digits.len() >= 3 {
                1
            } else {
                0
            };
            self.inputs[target].update(cx, |input, cx| input.focus(window, cx));
            cx.emit(ComponentInputEvent::Change);
            return;
        }

        if digits != text {
            self.inputs[index].update(cx, |input, cx| {
                input.set_value(digits.clone(), window, cx);
            });
        }

        let len = digits.len();
        let prev_len = self.prev_lengths[index];

        if len > max_len {
            let truncated: String = digits.chars().take(max_len).collect();
            self.inputs[index].update(cx, |input, cx| {
                input.set_value(truncated, window, cx);
            });
            if index < 3 {
                self.inputs[index + 1].update(cx, |input, cx| input.focus(window, cx));
            }
        } else if len == max_len && prev_len < max_len {
            if index < 3 {
                self.inputs[index + 1].update(cx, |input, cx| input.focus(window, cx));
            }
        } else if len == 0 && prev_len > 0 && index > 0 {
            self.inputs[index - 1].update(cx, |input, cx| input.focus(window, cx));
        }

        self.prev_lengths[index] = std::cmp::min(len, max_len);
        cx.emit(ComponentInputEvent::Change);
        cx.notify();
    }

    pub fn value(&self, cx: &gpui::App) -> String {
        let v1 = self.inputs[0].read(cx).value().to_string();
        let v2 = self.inputs[1].read(cx).value().to_string();
        let v3 = self.inputs[2].read(cx).value().to_string();
        let v4 = self.inputs[3].read(cx).value().to_string();
        format!("{}{}{}{}", v1, v2, v3, v4)
    }

    pub fn formatted_value(&self, cx: &gpui::App) -> String {
        let v1 = self.inputs[0].read(cx).value().to_string();
        let v2 = self.inputs[1].read(cx).value().to_string();
        let v3 = self.inputs[2].read(cx).value().to_string();
        let v4 = self.inputs[3].read(cx).value().to_string();

        let mut parts = vec![];
        if !v1.is_empty() {
            parts.push(v1);
        }
        if !v2.is_empty() {
            parts.push(v2);
        }
        if !v3.is_empty() {
            parts.push(v3);
        }
        if !v4.is_empty() {
            parts.push(v4);
        }

        parts.join("-")
    }

    pub fn is_valid(&self, cx: &gpui::App) -> bool {
        let val = self.value(cx);
        val.len() >= 12 && val.len() <= 14
    }

    pub fn set_from_tin(
        &mut self,
        tin: &bir_core::naming::Tin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let parts = [
            tin.segment1.clone(),
            tin.segment2.clone(),
            tin.segment3.clone(),
            tin.branch.clone(),
        ];
        for (i, part) in parts.into_iter().enumerate() {
            self.inputs[i].update(cx, |input, cx| {
                input.set_value(part.clone(), window, cx);
            });
            self.prev_lengths[i] = part.len();
        }
        cx.emit(ComponentInputEvent::Change);
        cx.notify();
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for i in 0..4 {
            self.inputs[i].update(cx, |input, cx| {
                input.set_value(String::new(), window, cx);
            });
            self.prev_lengths[i] = 0;
        }
        cx.emit(ComponentInputEvent::Change);
        cx.notify();
    }
}

impl Render for TinInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let is_valid = self.is_valid(cx);
        let val = self.value(cx);
        let show_error = !is_valid && !val.is_empty();

        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child("Taxpayer Identification Number (TIN)"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(if show_error {
                                gpui::rgba(0xFF0000FF).into()
                            } else {
                                cx.theme().transparent
                            })
                            .child("Invalid TIN format (14 digits: 3-3-3-5)"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(Input::new(&self.inputs[0]).w(px(70.)).text_center())
                    .child(
                        div()
                            .text_lg()
                            .text_color(cx.theme().muted_foreground)
                            .child("-"),
                    )
                    .child(Input::new(&self.inputs[1]).w(px(70.)).text_center())
                    .child(
                        div()
                            .text_lg()
                            .text_color(cx.theme().muted_foreground)
                            .child("-"),
                    )
                    .child(Input::new(&self.inputs[2]).w(px(70.)).text_center())
                    .child(
                        div()
                            .text_lg()
                            .text_color(cx.theme().muted_foreground)
                            .child("-"),
                    )
                    .child(Input::new(&self.inputs[3]).w(px(90.)).text_center()),
            )
    }
}
