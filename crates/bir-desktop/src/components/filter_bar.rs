use gpui::prelude::*;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::popover::Popover;
use gpui_component::tag::Tag;
use gpui_component::{ActiveTheme, Sizable};
use gpui_component::{Icon, IconName};

use bir_core::forms::registry::FORM_REGISTRY;
use bir_core::profile::TaxpayerType;

#[derive(Clone, PartialEq, Eq)]
pub struct FilterChip {
    pub id: String,
    pub label: String,
    pub group: String,
}

pub struct FilterState {
    pub active_chips: Vec<FilterChip>,
    pub input_state: Entity<InputState>,
    pub popover_open: bool,
    pub available_form_types: Vec<String>,
    pub available_quarters: Vec<String>,
    pub available_months: Vec<String>,
    pub focus_handle: FocusHandle,
}

pub struct FilterEvent {
    pub chips_changed: Vec<FilterChip>,
    pub query_changed: String,
}

impl EventEmitter<FilterEvent> for FilterState {}

impl FilterState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Filter forms...")
                .clean_on_escape()
        });

        cx.subscribe_in(
            &input_state,
            window,
            |this, _input, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    this.handle_enter(window, cx);
                }
                InputEvent::Change => {
                    let query = this.input_state.read(cx).value().to_string();
                    cx.emit(FilterEvent {
                        chips_changed: this.active_chips.clone(),
                        query_changed: query,
                    });
                }
                _ => {}
            },
        )
        .detach();

        Self {
            active_chips: Vec::new(),
            input_state,
            popover_open: false,
            available_form_types: FORM_REGISTRY.iter().map(|f| f.code.to_string()).collect(),
            available_quarters: vec![
                "Q1".to_string(),
                "Q2".to_string(),
                "Q3".to_string(),
                "Q4".to_string(),
            ],
            available_months: vec![
                "Jan".to_string(),
                "Feb".to_string(),
                "Mar".to_string(),
                "Apr".to_string(),
                "May".to_string(),
                "Jun".to_string(),
                "Jul".to_string(),
                "Aug".to_string(),
                "Sep".to_string(),
                "Oct".to_string(),
                "Nov".to_string(),
                "Dec".to_string(),
            ],
            focus_handle: cx.focus_handle(),
        }
    }

    /// Check if any Quarter chips are active
    pub fn has_quarter_filter(&self) -> bool {
        self.active_chips.iter().any(|c| c.group == "Quarter")
    }

    /// Check if any Month chips are active
    pub fn has_month_filter(&self) -> bool {
        self.active_chips.iter().any(|c| c.group == "Month")
    }

    /// Update the available form types based on the active profile.
    pub fn update_for_profile(
        &mut self,
        taxpayer_type: &TaxpayerType,
        is_vat: bool,
        cx: &mut Context<Self>,
    ) {
        self.available_form_types = FORM_REGISTRY
            .iter()
            .filter(|f| f.taxpayer_types.contains(taxpayer_type))
            .filter(|f| {
                if is_vat {
                    f.code != "2551Q"
                } else {
                    f.code != "2550M"
                }
            })
            .map(|f| f.code.to_string())
            .collect();

        // Prune chips that reference forms no longer applicable
        let before_len = self.active_chips.len();
        self.active_chips
            .retain(|c| c.group != "Form Type" || self.available_form_types.contains(&c.label));

        if self.active_chips.len() != before_len {
            let query = self.input_state.read(cx).value().to_string();
            cx.emit(FilterEvent {
                chips_changed: self.active_chips.clone(),
                query_changed: query,
            });
        }

        cx.notify();
    }

    fn handle_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self
            .input_state
            .read(cx)
            .value()
            .to_string()
            .trim()
            .to_string();
        if query.is_empty() {
            return;
        }

        if let Some(matched) = self
            .available_form_types
            .iter()
            .find(|t| t.eq_ignore_ascii_case(&query))
        {
            self.add_chip(
                FilterChip {
                    id: format!("form_type_{}", matched),
                    label: matched.clone(),
                    group: "Form Type".to_string(),
                },
                cx,
            );
        } else if let Some(matched) = self
            .available_quarters
            .iter()
            .find(|q| q.eq_ignore_ascii_case(&query))
        {
            if !self.has_month_filter() {
                self.add_chip(
                    FilterChip {
                        id: format!("quarter_{}", matched),
                        label: matched.clone(),
                        group: "Quarter".to_string(),
                    },
                    cx,
                );
            }
        } else if let Some(matched) = self
            .available_months
            .iter()
            .find(|m| m.eq_ignore_ascii_case(&query))
        {
            if !self.has_quarter_filter() {
                self.add_chip(
                    FilterChip {
                        id: format!("month_{}", matched),
                        label: matched.clone(),
                        group: "Month".to_string(),
                    },
                    cx,
                );
            }
        } else {
            // Unmatched query acts as generic text search (matches form code + title)
            cx.emit(FilterEvent {
                chips_changed: self.active_chips.clone(),
                query_changed: query.clone(),
            });
            return;
        }

        self.input_state.update(cx, |s, cx| {
            s.set_value(String::new(), window, cx);
        });

        cx.emit(FilterEvent {
            chips_changed: self.active_chips.clone(),
            query_changed: "".to_string(),
        });
        cx.notify();
    }

    pub fn add_chip(&mut self, chip: FilterChip, cx: &mut Context<Self>) {
        if !self.active_chips.iter().any(|c| c.id == chip.id) {
            self.active_chips.push(chip);
            let query = self.input_state.read(cx).value().to_string();
            cx.emit(FilterEvent {
                chips_changed: self.active_chips.clone(),
                query_changed: query,
            });
            cx.notify();
        }
    }

    pub fn remove_chip(&mut self, id: &str, cx: &mut Context<Self>) {
        self.active_chips.retain(|c| c.id != id);
        let query = self.input_state.read(cx).value().to_string();
        cx.emit(FilterEvent {
            chips_changed: self.active_chips.clone(),
            query_changed: query,
        });
        cx.notify();
    }

    pub fn clear_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.active_chips.clear();
        self.input_state.update(cx, |s, cx| {
            s.set_value(String::new(), window, cx);
        });
        cx.emit(FilterEvent {
            chips_changed: vec![],
            query_changed: String::new(),
        });
        cx.notify();
    }

    pub fn set_popover_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.popover_open = open;
        cx.notify();
    }

    fn render_popover<T>(entity: &Entity<Self>, cx: &mut Context<T>) -> impl IntoElement {
        let state = entity.read(cx);
        let form_types = state.available_form_types.clone();
        let quarters = state.available_quarters.clone();
        let months = state.available_months.clone();
        let active_chips = state.active_chips.clone();
        let has_quarter = state.has_quarter_filter();
        let has_month = state.has_month_filter();

        div()
            .flex()
            .flex_col()
            .id("filter_popover_scroll")
            .w(px(300.))
            .max_h(px(500.))
            .overflow_y_scroll()
            .p_3()
            .gap_4()
            // Form Type section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("FORM TYPE"),
                    )
                    .children(form_types.into_iter().map(|t| {
                        let is_active = active_chips
                            .iter()
                            .any(|c| c.id == format!("form_type_{}", t));
                        let t_clone = t.clone();
                        div()
                            .flex()
                            .id(format!("popover_ft_{}", t))
                            .items_center()
                            .gap_2()
                            .py_0p5()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().secondary).rounded_md())
                            .on_click({
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        if is_active {
                                            this.remove_chip(&format!("form_type_{}", t_clone), cx);
                                        } else {
                                            this.add_chip(
                                                FilterChip {
                                                    id: format!("form_type_{}", t_clone),
                                                    label: t_clone.clone(),
                                                    group: "Form Type".to_string(),
                                                },
                                                cx,
                                            );
                                        }
                                    });
                                }
                            })
                            .child(
                                div()
                                    .w_4()
                                    .h_4()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_sm()
                                    .when(is_active, |s| {
                                        s.bg(cx.theme().primary).border_color(cx.theme().primary)
                                    })
                                    .when(!is_active, |s| s.bg(cx.theme().background)),
                            )
                            .child(div().text_sm().child(t))
                    })),
            )
            // Quarter section — hidden if any Month filter is active
            .when(!has_month, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().muted_foreground)
                                .child("QUARTER"),
                        )
                        .child(div().flex().items_center().gap_1().children(
                            quarters.into_iter().map(|q| {
                                let is_active = active_chips
                                    .iter()
                                    .any(|c| c.id == format!("quarter_{}", q));
                                let q_clone = q.clone();
                                div()
                                    .id(format!("popover_q_{}", q))
                                    .flex_1()
                                    .flex()
                                    .justify_center()
                                    .py_1p5()
                                    .text_sm()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .when(is_active, |s| {
                                        s.bg(cx.theme().primary)
                                            .text_color(cx.theme().primary_foreground)
                                            .font_weight(FontWeight::BOLD)
                                    })
                                    .when(!is_active, |s| {
                                        s.bg(cx.theme().secondary)
                                            .text_color(cx.theme().secondary_foreground)
                                            .hover(|s| s.bg(cx.theme().accent))
                                    })
                                    .on_click({
                                        let entity = entity.clone();
                                        move |_, _, cx| {
                                            entity.update(cx, |this, cx| {
                                                if is_active {
                                                    this.remove_chip(
                                                        &format!("quarter_{}", q_clone),
                                                        cx,
                                                    );
                                                } else {
                                                    this.add_chip(
                                                        FilterChip {
                                                            id: format!("quarter_{}", q_clone),
                                                            label: q_clone.clone(),
                                                            group: "Quarter".to_string(),
                                                        },
                                                        cx,
                                                    );
                                                }
                                            });
                                        }
                                    })
                                    .child(q)
                            }),
                        )),
                )
            })
            // Month section — hidden if any Quarter filter is active
            .when(!has_quarter, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().muted_foreground)
                                .child("MONTH"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_1()
                                .children(months.into_iter().map(|m| {
                                    let is_active =
                                        active_chips.iter().any(|c| c.id == format!("month_{}", m));
                                    let m_clone = m.clone();
                                    div()
                                        .id(format!("popover_m_{}", m))
                                        .w(px(60.))
                                        .flex()
                                        .justify_center()
                                        .py_1p5()
                                        .text_xs()
                                        .rounded_md()
                                        .cursor_pointer()
                                        .when(is_active, |s| {
                                            s.bg(cx.theme().primary)
                                                .text_color(cx.theme().primary_foreground)
                                                .font_weight(FontWeight::BOLD)
                                        })
                                        .when(!is_active, |s| {
                                            s.bg(cx.theme().secondary)
                                                .text_color(cx.theme().secondary_foreground)
                                                .hover(|s| s.bg(cx.theme().accent))
                                        })
                                        .on_click({
                                            let entity = entity.clone();
                                            move |_, _, cx| {
                                                entity.update(cx, |this, cx| {
                                                    if is_active {
                                                        this.remove_chip(
                                                            &format!("month_{}", m_clone),
                                                            cx,
                                                        );
                                                    } else {
                                                        this.add_chip(
                                                            FilterChip {
                                                                id: format!("month_{}", m_clone),
                                                                label: m_clone.clone(),
                                                                group: "Month".to_string(),
                                                            },
                                                            cx,
                                                        );
                                                    }
                                                });
                                            }
                                        })
                                        .child(m)
                                })),
                        ),
                )
            })
    }
}

impl Render for FilterState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_grow()
                    .items_center()
                    .gap_1()
                    .px_3()
                    .py_1p5()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_lg()
                    .bg(cx.theme().background)
                    .child(
                        Icon::new(IconName::Search)
                            .small()
                            .text_color(cx.theme().muted_foreground),
                    )
                    // Chips: click the whole chip to remove it (no X icon)
                    .children(self.active_chips.iter().map(|chip| {
                        let id = chip.id.clone();
                        Tag::primary().small().rounded_full().child(
                            div()
                                .id(format!("chip_{}", id))
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                .child(chip.label.clone())
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.remove_chip(&id, cx);
                                    }),
                                ),
                        )
                    }))
                    .child(
                        div()
                            .flex_grow()
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                                let key = event.keystroke.key.as_str();
                                if (key == "backspace" || key == "delete")
                                    && this.input_state.read(cx).value().is_empty()
                                    && let Some(chip) = this.active_chips.last()
                                {
                                    let id = chip.id.clone();
                                    this.remove_chip(&id, cx);
                                }
                            }))
                            .child(Input::new(&self.input_state).appearance(false)),
                    )
                    .when(
                        !self.active_chips.is_empty()
                            || !self.input_state.read(cx).value().is_empty(),
                        |this| {
                            this.child(
                                div()
                                    .id("clear_all_filters")
                                    .cursor_pointer()
                                    .child(
                                        Icon::new(IconName::Close)
                                            .small()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .hover(|s| s.text_color(cx.theme().foreground))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            cx.stop_propagation();
                                            this.clear_all(window, cx);
                                        }),
                                    ),
                            )
                        },
                    ),
            )
            .child(
                Popover::new("filter-popover")
                    .open(self.popover_open)
                    .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                        this.set_popover_open(*open, cx);
                    }))
                    .trigger(Button::new("filter-btn").ghost().label("Filters"))
                    .content({
                        let entity = cx.entity().clone();
                        move |_, _, cx| Self::render_popover(&entity, cx).into_any_element()
                    }),
            )
    }
}

#[derive(IntoElement)]
pub struct FilterBar {
    state: Entity<FilterState>,
}

impl FilterBar {
    pub fn new(state: &Entity<FilterState>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl RenderOnce for FilterBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.state.clone().into_any_element()
    }
}
