use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme,
    input::{Input, InputEvent, InputState},
};

pub struct ComboboxEvent {
    pub selected: Option<String>,
}

pub struct ComboboxState {
    pub input: Entity<InputState>,
    pub options: Vec<String>,
    pub filtered_options: Vec<String>,
    pub open: bool,
    pub selected_index: Option<usize>,
    pub focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ComboboxEvent> for ComboboxState {}

impl ComboboxState {
    pub fn new(options: Vec<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        let focus_handle = cx.focus_handle();

        let _subscriptions = vec![cx.subscribe_in(
            &input,
            window,
            |this, _input, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
                    let query = _input.read(cx).value().to_string();
                    if query.is_empty() {
                        this.filtered_options = this.options.clone();
                    } else {
                        let query_lower = query.to_lowercase();
                        this.filtered_options = this
                            .options
                            .iter()
                            .filter(|o| o.to_lowercase().contains(&query_lower))
                            .cloned()
                            .collect();
                    }
                    this.selected_index = if this.filtered_options.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                    this.open = true;
                    cx.notify();
                } else if let InputEvent::Blur = event {
                    // Close popover on blur after a short delay to allow clicks
                    cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(150))
                            .await;
                        let _ = cx.update(|cx| {
                            if let Some(this) = this.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.open = false;
                                    cx.notify();
                                });
                            }
                        });
                    })
                    .detach();
                } else if let InputEvent::PressEnter { .. } = event {
                    if this.open {
                        if let Some(idx) = this.selected_index {
                            if let Some(option) = this.filtered_options.get(idx).cloned() {
                                this.select_item(&option, window, cx);
                            }
                        }
                    }
                } else if let InputEvent::Focus = event {
                    this.open = true;
                    cx.notify();
                }
            },
        )];

        Self {
            input,
            options: options.clone(),
            filtered_options: options,
            open: false,
            selected_index: None,
            focus_handle,
            _subscriptions,
        }
    }

    pub fn set_selected_value(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            input.set_value(value.to_string(), window, cx);
        });
        self.open = false;
        cx.notify();
    }

    pub fn selected_value(&self, cx: &gpui::App) -> String {
        self.input.read(cx).value().to_string()
    }

    fn select_item(&mut self, item: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            input.set_value(item.to_string(), window, cx);
        });
        self.open = false;
        cx.emit(ComboboxEvent {
            selected: Some(item.to_string()),
        });
        cx.notify();
    }
}

impl Focusable for ComboboxState {
    fn focus_handle(&self, cx: &gpui::App) -> gpui::FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for ComboboxState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let input_view = self.input.clone();
        let options = self.filtered_options.clone();
        let is_open = self.open && !options.is_empty();

        div()
            .relative()
            .w_full()
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                    if !this.open {
                        return;
                    }
                    match event.keystroke.key.as_str() {
                        "up" => {
                            if let Some(idx) = this.selected_index {
                                this.selected_index = Some(idx.saturating_sub(1));
                            }
                            cx.notify();
                        }
                        "down" => {
                            let max_idx = this.filtered_options.len().saturating_sub(1);
                            if let Some(idx) = this.selected_index {
                                this.selected_index = Some((idx + 1).min(max_idx));
                            } else {
                                this.selected_index = Some(0);
                            }
                            cx.notify();
                        }
                        "escape" => {
                            this.open = false;
                            cx.notify();
                        }
                        _ => {}
                    }
                }),
            )
            .child(Input::new(&input_view))
            .when(is_open, |this| {
                this.child(
                    deferred(
                        anchored().snap_to_window_with_margin(px(8.)).child(
                            div()
                                .occlude()
                                .mt_1p5()
                                .max_h(px(300.))
                                .overflow_hidden()
                                .w_full()
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_lg()
                                .rounded_md()
                                .bg(cx.theme().popover)
                                .text_color(cx.theme().popover_foreground)
                                .children(options.into_iter().enumerate().map(|(i, option)| {
                                    let is_selected = self.selected_index == Some(i);
                                    div()
                                        .id(("option", i))
                                        .px_3()
                                        .py_2()
                                        .cursor_pointer()
                                        .when(is_selected, |d| d.bg(cx.theme().secondary))
                                        .hover(|d| d.bg(cx.theme().secondary.opacity(0.8)))
                                        .child(option.clone())
                                        .on_click(cx.listener({
                                            let option = option.clone();
                                            move |this, _, window, cx| {
                                                this.select_item(&option, window, cx);
                                            }
                                        }))
                                })),
                        ),
                    )
                    .with_priority(2),
                )
            })
    }
}

#[derive(IntoElement)]
pub struct Combobox {
    state: Entity<ComboboxState>,
}

impl Combobox {
    pub fn new(state: &Entity<ComboboxState>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl RenderOnce for Combobox {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.state.clone().into_any_element()
    }
}
