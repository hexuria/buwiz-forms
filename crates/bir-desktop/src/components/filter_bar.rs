use chrono::Datelike;
use gpui::prelude::*;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::popover::Popover;
use gpui_component::tag::Tag;
use gpui_component::{ActiveTheme, Sizable};
use gpui_component::{Icon, IconName};
use gpui_rsx::rsx;

#[derive(Clone, PartialEq, Eq)]
pub struct FilterChip {
    pub id: String,
    pub label: String,
    pub group: String,
}

#[allow(dead_code)]
pub struct FilterState {
    pub active_chips: Vec<FilterChip>,
    pub input_state: Entity<InputState>,
    pub popover_open: bool,
    pub available_form_types: Vec<String>,
    pub available_quarters: Vec<String>,
    pub available_months: Vec<String>,
    pub focus_handle: FocusHandle,
    pub profile_registration_date: Option<chrono::NaiveDate>,
    pub selected_year: u16,
}

#[allow(dead_code)]
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
            available_form_types: bir_core::integration::all_form_codes(),
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
            profile_registration_date: None,
            selected_year: chrono::Local::now().date_naive().year() as u16,
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

    /// Check if the Annual chip is active
    pub fn has_annual_filter(&self) -> bool {
        self.active_chips.iter().any(|c| c.group == "Annual")
    }

    pub fn month_label_to_num(label: &str) -> Option<u32> {
        match label {
            "Jan" => Some(1),
            "Feb" => Some(2),
            "Mar" => Some(3),
            "Apr" => Some(4),
            "May" => Some(5),
            "Jun" => Some(6),
            "Jul" => Some(7),
            "Aug" => Some(8),
            "Sep" => Some(9),
            "Oct" => Some(10),
            "Nov" => Some(11),
            "Dec" => Some(12),
            _ => None,
        }
    }

    /// Update the available form types based on the active profile.
    pub fn update_for_profile(
        &mut self,
        profile: &bir_core::profile::TaxpayerProfile,
        year: u16,
        available_form_types: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        self.profile_registration_date = profile.business_start_date;
        self.selected_year = year;

        self.available_form_types = available_form_types;

        // Prune chips that reference forms no longer applicable, or months/quarters before registration
        let before_len = self.active_chips.len();
        self.active_chips.retain(|c| {
            if c.group == "Form Type" {
                return self.available_form_types.contains(&c.label);
            }
            if let Some(reg_d) = self.profile_registration_date {
                use chrono::Datelike;
                if self.selected_year == reg_d.year() as u16 {
                    if c.group == "Quarter" {
                        let q_num = c
                            .label
                            .strip_prefix('Q')
                            .and_then(|n| n.parse::<u32>().ok())
                            .unwrap_or(1);
                        let reg_q = (reg_d.month() - 1) / 3 + 1;
                        if q_num < reg_q {
                            return false;
                        }
                    }
                    if c.group == "Month" {
                        let m_num = Self::month_label_to_num(&c.label).unwrap_or(1);
                        if m_num < reg_d.month() {
                            return false;
                        }
                    }
                }
            }
            true
        });

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
            self.add_chip(
                FilterChip {
                    id: format!("quarter_{}", matched),
                    label: matched.clone(),
                    group: "Quarter".to_string(),
                },
                cx,
            );
        } else if let Some(matched) = self
            .available_months
            .iter()
            .find(|m| m.eq_ignore_ascii_case(&query))
        {
            self.add_chip(
                FilterChip {
                    id: format!("month_{}", matched),
                    label: matched.clone(),
                    group: "Month".to_string(),
                },
                cx,
            );
        } else if query.eq_ignore_ascii_case("annual") || query.eq_ignore_ascii_case("year") {
            self.add_chip(
                FilterChip {
                    id: "annual".to_string(),
                    label: "Annual".to_string(),
                    group: "Annual".to_string(),
                },
                cx,
            );
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
        let has_annual = state.has_annual_filter();

        rsx! {
            <div flex flex_col id={"filter_popover_scroll"} w={px(300.)} max_h={px(500.)} overflow_y_scroll p_3 gap_4>
                // Form Type section
                <div flex flex_col gap_2>
                    <div text_xs font_weight={FontWeight::SEMIBOLD} text_color={cx.theme().muted_foreground}>
                        {"FORM TYPE"}
                    </div>
                    {...form_types.into_iter().map(|t| {
                        let is_active = active_chips
                            .iter()
                            .any(|c| c.id == format!("form_type_{}", t));
                        let t_clone = t.clone();
                        rsx! {
                            <div
                                flex
                                id={format!("popover_ft_{}", t)}
                                items_center
                                gap_2
                                py_0p5
                                cursor_pointer
                                hover={|s| s.bg(cx.theme().secondary).rounded_md()}
                                on_click={{
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
                                }}
                            >
                                <div
                                    w_4
                                    h_4
                                    border_1
                                    border_color={cx.theme().border}
                                    rounded_sm
                                    when={(is_active, |s| {
                                        s.bg(cx.theme().primary).border_color(cx.theme().primary)
                                    })}
                                    when={(!is_active, |s| s.bg(cx.theme().background))}
                                />
                                <div text_sm>{t}</div>
                            </div>
                        }
                    })}
                </div>
                // Annual section
                <div flex flex_col gap_2>
                    <div text_xs font_weight={FontWeight::SEMIBOLD} text_color={cx.theme().muted_foreground}>
                        {"ANNUAL"}
                    </div>
                    <div
                        id={"popover_annual"}
                        flex
                        justify_center
                        py_1p5
                        text_sm
                        rounded_md
                        cursor_pointer
                        when={(has_annual, |s| {
                            s.bg(cx.theme().primary)
                                .text_color(cx.theme().primary_foreground)
                                .font_weight(FontWeight::BOLD)
                        })}
                        when={(!has_annual, |s| {
                            s.bg(cx.theme().secondary)
                                .text_color(cx.theme().secondary_foreground)
                                .hover(|s| s.bg(cx.theme().accent))
                        })}
                        on_click={{
                            let entity = entity.clone();
                            move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    if has_annual {
                                        this.remove_chip("annual", cx);
                                    } else {
                                        this.add_chip(
                                            FilterChip {
                                                id: "annual".to_string(),
                                                label: "Annual".to_string(),
                                                group: "Annual".to_string(),
                                            },
                                            cx,
                                        );
                                    }
                                });
                            }
                        }}
                    >
                        {"Annual"}
                    </div>
                </div>
                // Quarter section
                <div flex flex_col gap_2>
                    <div text_xs font_weight={FontWeight::SEMIBOLD} text_color={cx.theme().muted_foreground}>
                        {"QUARTER"}
                    </div>
                    <div flex items_center gap_1>
                        {...quarters.into_iter().map(|q| {
                            let is_active = active_chips
                                .iter()
                                .any(|c| c.id == format!("quarter_{}", q));
                            let q_clone = q.clone();
                            let mut is_disabled = false;
                            if let Some(reg_d) = state.profile_registration_date {
                                if state.selected_year == reg_d.year() as u16 {
                                    let q_num = q
                                        .strip_prefix('Q')
                                        .and_then(|n| n.parse::<u32>().ok())
                                        .unwrap_or(1);
                                    let reg_q = (reg_d.month() - 1) / 3 + 1;
                                    if q_num < reg_q {
                                        is_disabled = true;
                                    }
                                }
                            }
                            rsx! {
                                <div
                                    id={format!("popover_q_{}", q)}
                                    flex_1
                                    flex
                                    justify_center
                                    py_1p5
                                    text_sm
                                    rounded_md
                                    when={(is_disabled, |s| {
                                        s.text_color(cx.theme().border)
                                            .bg(cx.theme().secondary.opacity(0.4))
                                    })}
                                    when={(!is_disabled, |s| {
                                        s.cursor_pointer()
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
                                                                    id: format!(
                                                                        "quarter_{}",
                                                                        q_clone
                                                                    ),
                                                                    label: q_clone.clone(),
                                                                    group: "Quarter".to_string(),
                                                                },
                                                                cx,
                                                            );
                                                        }
                                                    });
                                                }
                                            })
                                    })}
                                >
                                    {q}
                                </div>
                            }
                        })}
                    </div>
                </div>
                // Month section
                <div flex flex_col gap_2>
                    <div text_xs font_weight={FontWeight::SEMIBOLD} text_color={cx.theme().muted_foreground}>
                        {"MONTH"}
                    </div>
                    <div flex flex_wrap gap_1>
                        {...months.into_iter().map(|m| {
                            let is_active =
                                active_chips.iter().any(|c| c.id == format!("month_{}", m));
                            let m_clone = m.clone();
                            let mut is_disabled = false;
                            if let Some(reg_d) = state.profile_registration_date {
                                if state.selected_year == reg_d.year() as u16 {
                                    let m_num = Self::month_label_to_num(&m).unwrap_or(1);
                                    if m_num < reg_d.month() {
                                        is_disabled = true;
                                    }
                                }
                            }
                            rsx! {
                                <div
                                    id={format!("popover_m_{}", m)}
                                    w={px(60.)}
                                    flex
                                    justify_center
                                    py_1p5
                                    text_xs
                                    rounded_md
                                    when={(is_disabled, |s| {
                                        s.text_color(cx.theme().border)
                                            .bg(cx.theme().secondary.opacity(0.4))
                                    })}
                                    when={(!is_disabled, |s| {
                                        s.cursor_pointer()
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
                                                                    id: format!(
                                                                        "month_{}",
                                                                        m_clone
                                                                    ),
                                                                    label: m_clone.clone(),
                                                                    group: "Month".to_string(),
                                                                },
                                                                cx,
                                                            );
                                                        }
                                                    });
                                                }
                                            })
                                    })}
                                >
                                    {m}
                                </div>
                            }
                        })}
                    </div>
                </div>
            </div>
        }
    }
}

impl Render for FilterState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        rsx! {
            <div flex w_full items_center gap_2>
                {div()
                    .flex()
                    .flex_grow()
                    .items_center()
                    .gap_1()
                    .px_3()
                    .h_11()
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
                    .child(rsx! {
                        <div
                            flex_grow
                            on_key_down={cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                                let key = event.keystroke.key.as_str();
                                if (key == "backspace" || key == "delete")
                                    && this.input_state.read(cx).value().is_empty()
                                    && let Some(chip) = this.active_chips.last()
                                {
                                    let id = chip.id.clone();
                                    this.remove_chip(&id, cx);
                                }
                            })}
                        >
                            {Input::new(&self.input_state).appearance(false)}
                        </div>
                    })
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
                    )}
                {Popover::new("filter-popover")
                    .open(self.popover_open)
                    .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                        this.set_popover_open(*open, cx);
                    }))
                    .trigger(
                        Button::new("filter-btn")
                            .ghost()
                            .label("Filters")
                            .large()
                            .h_11(),
                    )
                    .content({
                        let entity = cx.entity().clone();
                        move |_, _, cx| Self::render_popover(&entity, cx).into_any_element()
                    })}
            </div>
        }
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
