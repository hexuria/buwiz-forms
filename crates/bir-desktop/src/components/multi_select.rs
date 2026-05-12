//! Multi-Select Combobox Component
//!
//! A reusable multi-select dropdown with checkboxes, search filtering,
//! tag chips, Select All / Clear All, and keyboard navigation.
//! Designed for enterprise tax compliance software.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt, h_flex,
    input::{Input, InputEvent, InputState},
    tag::Tag,
    v_flex,
};

/// Emitted when the selection changes.
#[allow(dead_code)]
pub struct MultiSelectEvent {
    pub selected: Vec<String>,
}

/// An individual option in the multi-select dropdown.
#[derive(Clone, Debug)]
pub struct MultiSelectOption {
    pub id: String,
    pub label: String,
}

impl MultiSelectOption {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// The state for a MultiSelectCombobox.
#[allow(dead_code)]
pub struct MultiSelectState {
    /// All available options.
    options: Vec<MultiSelectOption>,
    /// IDs of currently selected options (preserves insertion order).
    selected_ids: Vec<String>,
    /// Filtered options based on search query.
    filtered_indices: Vec<usize>,
    /// Search input entity.
    search_input: Entity<InputState>,
    /// Whether the dropdown is open.
    open: bool,
    /// Keyboard-highlighted index within filtered_indices.
    highlighted_index: Option<usize>,
    /// Focus handle.
    focus_handle: FocusHandle,
    /// Placeholder text shown when no items are selected.
    placeholder: String,
    /// Maximum number of chips to show before collapsing.
    max_visible_chips: usize,
    /// Subscriptions.
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<MultiSelectEvent> for MultiSelectState {}

impl MultiSelectState {
    pub fn new(
        options: Vec<MultiSelectOption>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search...")
                .clean_on_escape()
        });
        let focus_handle = cx.focus_handle();
        let filtered_indices = (0..options.len()).collect();

        let _subscriptions = vec![cx.subscribe_in(
            &search_input,
            window,
            |this, _input, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => {
                    let query = _input.read(cx).value().to_string();
                    this.apply_filter(&query);
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    // Toggle the highlighted item on Enter
                    if let Some(idx) = this.highlighted_index {
                        if let Some(&option_idx) = this.filtered_indices.get(idx) {
                            let id = this.options[option_idx].id.clone();
                            this.toggle_selection(&id);
                            cx.emit(MultiSelectEvent {
                                selected: this.selected_ids.clone(),
                            });
                            cx.notify();
                        }
                    }
                }
                InputEvent::Blur => {
                    // Close on blur with delay (allows click events to fire first)
                    cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(200))
                            .await;
                        cx.update(|cx| {
                            if let Some(this) = this.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.open = false;
                                    cx.notify();
                                });
                            }
                        });
                    })
                    .detach();
                }
                _ => {}
            },
        )];

        Self {
            options,
            selected_ids: Vec::new(),
            filtered_indices,
            search_input,
            open: false,
            highlighted_index: None,
            focus_handle,
            placeholder: "Select items...".to_string(),
            max_visible_chips: 3,
            _subscriptions,
        }
    }

    /// Set placeholder text.
    #[allow(dead_code)]
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Set maximum visible chips before collapsing to count.
    #[allow(dead_code)]
    pub fn max_visible_chips(mut self, count: usize) -> Self {
        self.max_visible_chips = count;
        self
    }

    /// Get the currently selected IDs.
    pub fn selected_ids(&self) -> &[String] {
        &self.selected_ids
    }

    /// Get the currently selected labels.
    #[allow(dead_code)]
    pub fn selected_labels(&self) -> Vec<String> {
        self.selected_ids
            .iter()
            .filter_map(|id| {
                self.options
                    .iter()
                    .find(|o| o.id == *id)
                    .map(|o| o.label.clone())
            })
            .collect()
    }

    /// Set the selected IDs programmatically.
    pub fn set_selected_ids(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        self.selected_ids = ids;
        cx.notify();
    }

    /// Toggle an option by ID.
    fn toggle_selection(&mut self, id: &str) {
        if let Some(pos) = self.selected_ids.iter().position(|s| s == id) {
            self.selected_ids.remove(pos);
        } else {
            self.selected_ids.push(id.to_string());
        }
    }

    /// Check if an option is selected.
    fn is_selected(&self, id: &str) -> bool {
        self.selected_ids.iter().any(|s| s == id)
    }

    /// Apply search filter.
    fn apply_filter(&mut self, query: &str) {
        if query.is_empty() {
            self.filtered_indices = (0..self.options.len()).collect();
        } else {
            let query_lower = query.to_lowercase();
            self.filtered_indices = self
                .options
                .iter()
                .enumerate()
                .filter(|(_, o)| o.label.to_lowercase().contains(&query_lower))
                .map(|(i, _)| i)
                .collect();
        }
        self.highlighted_index = if self.filtered_indices.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Select all visible (filtered) options.
    fn select_all(&mut self) {
        for &idx in &self.filtered_indices {
            let id = self.options[idx].id.clone();
            if !self.is_selected(&id) {
                self.selected_ids.push(id);
            }
        }
    }

    /// Clear all selections.
    fn clear_all(&mut self) {
        self.selected_ids.clear();
    }

    /// Remove a single selection by ID.
    fn remove_selection(&mut self, id: &str) {
        self.selected_ids.retain(|s| s != id);
    }
}

impl Focusable for MultiSelectState {
    fn focus_handle(&self, cx: &gpui::App) -> gpui::FocusHandle {
        self.search_input.focus_handle(cx)
    }
}

impl Render for MultiSelectState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_count = self.selected_ids.len();
        let is_open = self.open;
        let filtered = self.filtered_indices.clone();
        let all_selected = !filtered.is_empty()
            && filtered
                .iter()
                .all(|&i| self.is_selected(&self.options[i].id));

        // -- Build the trigger area (chips or placeholder) --
        let trigger_content = if selected_count == 0 {
            // Placeholder
            h_flex()
                .gap_1()
                .flex_wrap()
                .items_center()
                .py_1()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(self.placeholder.clone()),
                )
                .into_any_element()
        } else if selected_count > self.max_visible_chips {
            // Collapsed count badge
            h_flex()
                .gap_1()
                .items_center()
                .py_1()
                .child(
                    Tag::primary()
                        .small()
                        .child(format!("{} selected", selected_count)),
                )
                .into_any_element()
        } else {
            // Individual chips
            let chip_ids: Vec<(String, String)> = self
                .selected_ids
                .iter()
                .filter_map(|id| {
                    self.options
                        .iter()
                        .find(|o| o.id == *id)
                        .map(|o| (o.id.clone(), o.label.clone()))
                })
                .collect();

            h_flex()
                .gap_1()
                .flex_wrap()
                .items_center()
                .py_1()
                .children(chip_ids.into_iter().map(|(id, label)| {
                    // Each chip: label + ✕ remove button
                    div()
                        .id(SharedString::from(format!("chip_{}", id)))
                        .flex()
                        .items_center()
                        .gap_0p5()
                        .pl_2()
                        .pr_1()
                        .py_0p5()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .text_xs()
                        .text_color(cx.theme().secondary_foreground)
                        .child(label)
                        .child(
                            div()
                                .id(SharedString::from(format!("chip_remove_{}", id)))
                                .cursor_pointer()
                                .rounded_sm()
                                .p_0p5()
                                .hover(|d| d.bg(cx.theme().danger.opacity(0.15)))
                                .child(
                                    Icon::new(IconName::Close)
                                        .xsmall()
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    move |this, _, _, cx| {
                                        this.remove_selection(&id);
                                        cx.emit(MultiSelectEvent {
                                            selected: this.selected_ids.clone(),
                                        });
                                        cx.notify();
                                    }
                                })),
                        )
                }))
                .into_any_element()
        };

        // -- Build the dropdown options --
        let dropdown = if is_open {
            let option_rows: Vec<AnyElement> = filtered
                .iter()
                .enumerate()
                .map(|(filter_idx, &option_idx)| {
                    let option = &self.options[option_idx];
                    let checked = self.is_selected(&option.id);
                    let is_highlighted = self.highlighted_index == Some(filter_idx);
                    let id = option.id.clone();
                    let label = option.label.clone();

                    div()
                        .id(SharedString::from(format!("ms_opt_{}", id)))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py(px(6.))
                        .cursor_pointer()
                        .when(is_highlighted, |d| d.bg(cx.theme().secondary))
                        .hover(|d| d.bg(cx.theme().secondary.opacity(0.7)))
                        .on_click(cx.listener({
                            let id = id.clone();
                            move |this, _, _, cx| {
                                this.toggle_selection(&id);
                                cx.emit(MultiSelectEvent {
                                    selected: this.selected_ids.clone(),
                                });
                                cx.notify();
                            }
                        }))
                        // Checkbox indicator
                        .child(
                            div()
                                .w(px(16.))
                                .h(px(16.))
                                .rounded_sm()
                                .border_1()
                                .border_color(if checked {
                                    cx.theme().primary
                                } else {
                                    cx.theme().border
                                })
                                .bg(if checked {
                                    cx.theme().primary
                                } else {
                                    gpui::transparent_black()
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(checked, |d| {
                                    d.child(
                                        Icon::new(IconName::Check)
                                            .xsmall()
                                            .text_color(cx.theme().primary_foreground),
                                    )
                                }),
                        )
                        // Label
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().foreground)
                                .child(label),
                        )
                        .into_any_element()
                })
                .collect();

            // "Select All" / "Clear All" action bar
            let action_bar = h_flex()
                .px_3()
                .py(px(6.))
                .justify_between()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .id("ms_select_all")
                        .cursor_pointer()
                        .text_xs()
                        .text_color(cx.theme().primary)
                        .hover(|d| d.text_color(cx.theme().link.opacity(0.8)))
                        .child(if all_selected {
                            "Deselect All"
                        } else {
                            "Select All"
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if all_selected {
                                this.clear_all();
                            } else {
                                this.select_all();
                            }
                            cx.emit(MultiSelectEvent {
                                selected: this.selected_ids.clone(),
                            });
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id("ms_clear_all")
                        .cursor_pointer()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .hover(|d| d.text_color(cx.theme().danger))
                        .when(selected_count > 0, |d| d.child("Clear All"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.clear_all();
                            cx.emit(MultiSelectEvent {
                                selected: this.selected_ids.clone(),
                            });
                            cx.notify();
                        })),
                );

            // Empty state
            let empty_state = if option_rows.is_empty() {
                Some(
                    div()
                        .px_3()
                        .py_4()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("No matching options"),
                )
            } else {
                None
            };

            Some(
                deferred(
                    div().absolute().top(px(0.)).left_0().w_full().child(
                        v_flex()
                            .occlude()
                            .mt_1p5()
                            .max_h(px(320.))
                            .w_full()
                            .border_1()
                            .border_color(cx.theme().border)
                            .shadow_md()
                            .rounded_md()
                            .bg(cx.theme().popover)
                            .text_color(cx.theme().popover_foreground)
                            .overflow_hidden()
                            // Sticky search bar
                            .child(
                                div()
                                    .px_2()
                                    .py_1p5()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        Input::new(&self.search_input)
                                            .small()
                                            .prefix(
                                                Icon::new(IconName::Search)
                                                    .xsmall()
                                                    .text_color(cx.theme().muted_foreground),
                                            )
                                            .appearance(false),
                                    ),
                            )
                            // Action bar
                            .child(action_bar)
                            // Options list (scrollable)
                            .child(
                                v_flex()
                                    .max_h(px(220.))
                                    .overflow_hidden()
                                    .children(option_rows)
                                    .when_some(empty_state, |d, empty| d.child(empty)),
                            ),
                    ),
                )
                .with_priority(3),
            )
        } else {
            None
        };

        // -- Main component layout --
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
                            if let Some(idx) = this.highlighted_index {
                                this.highlighted_index = Some(idx.saturating_sub(1));
                            }
                            cx.notify();
                        }
                        "down" => {
                            let max_idx = this.filtered_indices.len().saturating_sub(1);
                            if let Some(idx) = this.highlighted_index {
                                this.highlighted_index = Some((idx + 1).min(max_idx));
                            } else {
                                this.highlighted_index = Some(0);
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
            // The trigger / input area
            .child(
                div()
                    .id("ms_trigger")
                    .w_full()
                    .min_h(px(36.))
                    .px_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_1()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .cursor_pointer()
                    .hover(|d| d.border_color(cx.theme().ring))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open = !this.open;
                        if this.open {
                            // Reset filter when opening
                            this.search_input.update(cx, |input, cx| {
                                input.set_value("".to_string(), window, cx);
                            });
                            this.filtered_indices = (0..this.options.len()).collect();
                            this.highlighted_index = if this.options.is_empty() {
                                None
                            } else {
                                Some(0)
                            };
                            // Focus the search input
                            this.search_input.update(cx, |_input, cx| {
                                cx.focus_self(window);
                            });
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .child(trigger_content),
                    )
                    .child(
                        Icon::new(IconName::ChevronDown)
                            .xsmall()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            // Dropdown
            .when_some(dropdown, |d, dropdown| d.child(dropdown))
    }
}

// -- Wrapper component for ergonomic use --

/// A multi-select combobox element (IntoElement wrapper).
#[derive(IntoElement)]
pub struct MultiSelect {
    state: Entity<MultiSelectState>,
}

impl MultiSelect {
    pub fn new(state: &Entity<MultiSelectState>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl RenderOnce for MultiSelect {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.state.clone().into_any_element()
    }
}
