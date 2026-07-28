use bir_core::calendar_rules::{
    DeadlineOverride, DeadlineResolver, OfficialRule, ResolvedTaxDeadline,
};
use bir_core::db::Database;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement;
use gpui_component::*;
use gpui_rsx::rsx;
use std::sync::{Arc, Mutex};

use crate::components::date_input::{DateInput, DateInputState};
use crate::components::form_multi_select;
use crate::components::multi_select::{MultiSelect, MultiSelectEvent, MultiSelectState};

pub enum AdminCalendarEvent {
    OverridesChanged,
}

impl EventEmitter<AdminCalendarEvent> for AdminCalendarDashboard {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AdminTab {
    Deadlines,
    Rules,
    Overrides,
}

#[derive(Default)]
struct OverrideEditorFields {
    title: String,
    source: String,
    form_codes: Vec<String>,
    original: Option<chrono::NaiveDate>,
    adjusted: Option<chrono::NaiveDate>,
}

pub struct AdminCalendarDashboard {
    db: Arc<Mutex<Database>>,
    active_tab: AdminTab,
    selected_year: i32,
    rules: Vec<OfficialRule>,
    deadlines: Vec<ResolvedTaxDeadline>,
    overrides: Vec<DeadlineOverride>,
    // Override editor modal
    override_editor_open: bool,
    editing_override_id: Option<String>,
    override_editor_error: Option<String>,
    new_override_title: Entity<InputState>,
    new_override_source: Entity<InputState>,
    new_override_forms_select: Entity<MultiSelectState>,
    new_override_form_codes: Vec<String>,
    new_override_orig_date: Entity<DateInputState>,
    new_override_adj_date: Entity<DateInputState>,
    status_message: Option<String>,
}

impl AdminCalendarDashboard {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let year = chrono::Local::now()
            .format("%Y")
            .to_string()
            .parse()
            .unwrap_or(2025);
        let rules = DeadlineResolver::official_rules();
        let mut overrides = db
            .lock()
            .map(|d| d.get_deadline_overrides())
            .unwrap_or_default();

        // Auto-seed holiday overrides if none exist
        if overrides.is_empty() {
            let seed = DeadlineResolver::seed_2026_holiday_overrides();
            if let Ok(db_lock) = db.lock() {
                let _ = db_lock.set_deadline_overrides(&seed);
            }
            overrides = seed;
        }

        let deadlines =
            DeadlineResolver::resolve_deadline_calendar_year_with_overrides(year, &overrides);

        let new_override_title =
            cx.new(|cx| InputState::new(window, cx).placeholder("Title (e.g. Holiday Extension)"));
        let new_override_source = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Source (e.g. BIR Advisory No. 2026-01)")
        });
        let new_override_forms_select = cx.new(|cx| {
            form_multi_select::registry_form_multi_select("Search form codes...", window, cx)
        });
        let new_override_orig_date = cx.new(|cx| DateInputState::new(window, cx));
        let new_override_adj_date = cx.new(|cx| DateInputState::new(window, cx));

        cx.subscribe(
            &new_override_forms_select,
            |this: &mut Self, _, event: &MultiSelectEvent, cx| {
                this.new_override_form_codes = event.selected.clone();
                cx.notify();
            },
        )
        .detach();

        Self {
            db,
            active_tab: AdminTab::Deadlines,
            selected_year: year,
            rules,
            deadlines,
            overrides,
            override_editor_open: false,
            editing_override_id: None,
            override_editor_error: None,
            new_override_title,
            new_override_source,
            new_override_forms_select,
            new_override_form_codes: Vec::new(),
            new_override_orig_date,
            new_override_adj_date,
            status_message: None,
        }
    }

    fn reload(&mut self) {
        self.overrides = self
            .db
            .lock()
            .map(|d| d.get_deadline_overrides())
            .unwrap_or_default();
        self.deadlines = DeadlineResolver::resolve_deadline_calendar_year_with_overrides(
            self.selected_year,
            &self.overrides,
        );
    }

    fn change_year(&mut self, delta: i32, cx: &mut Context<'_, Self>) {
        self.selected_year += delta;
        self.reload();
        cx.notify();
    }

    fn set_editor_fields(
        &mut self,
        fields: OverrideEditorFields,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let OverrideEditorFields {
            title,
            source,
            form_codes,
            original,
            adjusted,
        } = fields;
        self.new_override_title
            .update(cx, |s, cx| s.set_value(title, window, cx));
        self.new_override_source
            .update(cx, |s, cx| s.set_value(source, window, cx));
        self.new_override_form_codes = form_codes.clone();
        self.new_override_forms_select
            .update(cx, |select, cx| select.set_selected_ids(form_codes, cx));
        self.new_override_orig_date
            .update(cx, |s, cx| s.set_date(original, window, cx));
        self.new_override_adj_date
            .update(cx, |s, cx| s.set_date(adjusted, window, cx));
    }

    fn open_new_override_editor(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.set_editor_fields(OverrideEditorFields::default(), window, cx);
        self.editing_override_id = None;
        self.override_editor_error = None;
        self.override_editor_open = true;
        cx.notify();
    }

    fn open_edit_override_editor(
        &mut self,
        override_id: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(existing) = self
            .overrides
            .iter()
            .find(|item| item.id == override_id)
            .cloned()
        else {
            self.status_message = Some("Override was not found.".into());
            cx.notify();
            return;
        };
        self.set_editor_fields(
            OverrideEditorFields {
                title: existing.title,
                source: existing.source_reference,
                form_codes: existing.affected_form_codes,
                original: Some(existing.original_deadline),
                adjusted: Some(existing.adjusted_deadline),
            },
            window,
            cx,
        );
        self.editing_override_id = Some(existing.id);
        self.override_editor_error = None;
        self.override_editor_open = true;
        cx.notify();
    }

    fn close_override_editor(&mut self, cx: &mut Context<'_, Self>) {
        self.override_editor_open = false;
        self.override_editor_error = None;
        cx.notify();
    }

    fn save_override(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        let title = self.new_override_title.read(cx).value().trim().to_string();
        let source = self.new_override_source.read(cx).value().trim().to_string();
        let mut form_codes = self.new_override_form_codes.clone();
        form_codes.sort();
        form_codes.dedup();

        if title.is_empty() || source.is_empty() || form_codes.is_empty() {
            self.override_editor_error =
                Some("Title, source, and at least one form code are required.".into());
            cx.notify();
            return;
        }

        if self.new_override_orig_date.read(cx).has_invalid_value(cx) {
            self.override_editor_error = Some("Original deadline must use MM/DD/YYYY.".into());
            cx.notify();
            return;
        }
        if self.new_override_adj_date.read(cx).has_invalid_value(cx) {
            self.override_editor_error = Some("Adjusted deadline must use MM/DD/YYYY.".into());
            cx.notify();
            return;
        }
        let Some(orig) = self.new_override_orig_date.read(cx).date else {
            self.override_editor_error = Some("Original deadline is required.".into());
            cx.notify();
            return;
        };
        let Some(adj) = self.new_override_adj_date.read(cx).date else {
            self.override_editor_error = Some("Adjusted deadline is required.".into());
            cx.notify();
            return;
        };

        let saved_title = title.clone();
        if let Some(editing_id) = self.editing_override_id.clone() {
            let Some(existing) = self.overrides.iter_mut().find(|item| item.id == editing_id)
            else {
                self.override_editor_error =
                    Some("The override being edited no longer exists.".into());
                cx.notify();
                return;
            };
            existing.title = title;
            existing.source_reference = source;
            existing.affected_form_codes = form_codes;
            existing.original_deadline = orig;
            existing.adjusted_deadline = adj;
        } else {
            let id = format!("override-{}", chrono::Local::now().timestamp_millis());
            self.overrides.push(DeadlineOverride {
                id,
                title,
                source_reference: source,
                affected_form_codes: form_codes,
                original_deadline: orig,
                adjusted_deadline: adj,
                affected_regions: Vec::new(),
                affected_taxpayer_types: Vec::new(),
                effective_from: None,
                effective_until: None,
                expires_at: None,
            });
        }

        if let Ok(db) = self.db.lock() {
            let _ = db.set_deadline_overrides(&self.overrides);
        }

        self.reload();
        self.status_message = Some(if self.editing_override_id.is_some() {
            format!("Override '{saved_title}' updated.")
        } else {
            format!("Override '{saved_title}' added.")
        });
        self.override_editor_open = false;
        self.editing_override_id = None;
        self.override_editor_error = None;
        cx.emit(AdminCalendarEvent::OverridesChanged);
        cx.notify();
    }

    fn delete_override(&mut self, idx: usize, cx: &mut Context<'_, Self>) {
        if idx < self.overrides.len() {
            self.overrides.remove(idx);
            if let Ok(db) = self.db.lock() {
                let _ = db.set_deadline_overrides(&self.overrides);
            }
            self.reload();
            self.status_message = Some("Override removed.".into());
            cx.emit(AdminCalendarEvent::OverridesChanged);
            cx.notify();
        }
    }

    fn render_tab_button(
        &self,
        tab: AdminTab,
        label: &'static str,
        cx: &mut Context<'_, Self>,
    ) -> impl IntoElement {
        let is_active = self.active_tab == tab;
        rsx! {
            <div
                id={SharedString::from(format!("tab-{label}"))}
                px_4
                py_2
                bg={if is_active {
                    cx.theme().primary
                } else {
                    cx.theme().secondary
                }}
                text_color={if is_active {
                    cx.theme().primary_foreground
                } else {
                    cx.theme().foreground
                }}
                rounded_md
                font_weight={FontWeight::BOLD}
                cursor_pointer
                hover={|s| s.opacity(0.8)}
                on_click={cx.listener(move |this, _, _, cx| {
                    this.active_tab = tab;
                    cx.notify();
                })}
            >
                {label}
            </div>
        }
    }

    fn render_deadlines_tab(&self, cx: &mut Context<'_, Self>) -> Div {
        rsx! {
            <div flex flex_col gap_4 pb={px(96.)}>
                <div flex items_center gap_4>
                    {gpui_component::button::Button::new("prev-year")
                        .icon(IconName::ChevronLeft)
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| this.change_year(-1, cx)))}
                    <div
                        text_xl
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().foreground}
                    >
                        {format!("{} Calendar-Year Deadlines", self.selected_year)}
                    </div>
                    {gpui_component::button::Button::new("next-year")
                        .icon(IconName::ChevronRight)
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| this.change_year(1, cx)))}
                </div>
                {...self.deadlines.iter().enumerate().map(|(idx, d)| {
                    rsx! {
                        <div
                            id={SharedString::from(format!("deadline-{}", idx))}
                            w_full
                            p_4
                            bg={cx.theme().background}
                            border_1
                            border_color={cx.theme().border}
                            rounded_xl
                            shadow_sm
                            flex
                            justify_between
                            items_center
                        >
                            <div flex flex_col gap_1>
                                <div
                                    text_lg
                                    font_weight={FontWeight::BOLD}
                                    text_color={cx.theme().foreground}
                                >
                                    {format!("{} - {}", d.display_form_no, d.form_name)}
                                </div>
                                <div text_sm text_color={cx.theme().muted_foreground}>
                                    {d.period.label()}
                                </div>
                            </div>
                            <div flex flex_col items_end gap_1>
                                <div
                                    text_lg
                                    font_weight={FontWeight::BOLD}
                                    text_color={cx.theme().primary}
                                >
                                    {format!("Due: {}", d.final_deadline_string())}
                                </div>
                                <div
                                    px_2
                                    py_1
                                    bg={cx.theme().secondary}
                                    text_color={cx.theme().foreground}
                                    rounded_full
                                    text_xs
                                >
                                    {d.status.label()}
                                </div>
                            </div>
                        </div>
                    }
                })}
            </div>
        }
    }

    fn render_rules_tab(&self, cx: &mut Context<'_, Self>) -> Div {
        rsx! {
            <div flex flex_col gap_4 pb={px(96.)}>
                <div
                    text_xl
                    font_weight={FontWeight::BOLD}
                    text_color={cx.theme().foreground}
                >
                    {"Official BIR Tax Rules (Compile-Time)"}
                </div>
                <div text_sm text_color={cx.theme().muted_foreground}>
                    {"These rules are seeded statically from official BIR guidelines and cannot be modified dynamically."}
                </div>
                {...self.rules.iter().enumerate().map(|(idx, rule)| {
                    rsx! {
                        <div
                            id={SharedString::from(format!("rule-{}", idx))}
                            w_full
                            p_4
                            bg={cx.theme().background}
                            border_1
                            border_color={cx.theme().border}
                            rounded_xl
                            shadow_sm
                            flex
                            flex_col
                            gap_2
                        >
                            <div
                                text_lg
                                font_weight={FontWeight::BOLD}
                                text_color={cx.theme().foreground}
                            >
                                {rule.form_name}
                            </div>
                            <div text_sm text_color={cx.theme().muted_foreground}>
                                {format!("Forms: {}", rule.form_nos.join(", "))}
                            </div>
                            <div flex gap_2 items_center>
                                <div
                                    px_2
                                    py_1
                                    bg={cx.theme().primary}
                                    text_color={cx.theme().primary_foreground}
                                    rounded_md
                                    text_xs
                                >
                                    {format!("{:?}", rule.frequency)}
                                </div>
                                <div text_sm text_color={cx.theme().foreground}>
                                    {rule.description}
                                </div>
                            </div>
                        </div>
                    }
                })}
            </div>
        }
    }

    fn render_overrides_tab(&self, cx: &mut Context<'_, Self>) -> Div {
        let mut container = div().flex().flex_col().gap_4().pb(px(96.));

        container = container.child(rsx! {
            <div flex items_center justify_between gap_3>
                <div
                    text_xl
                    font_weight={FontWeight::BOLD}
                    text_color={cx.theme().foreground}
                >
                    {"Deadline Overrides (BIR Advisories)"}
                </div>
                {gpui_component::button::Button::new("new-override")
                    .label("New Override")
                    .primary()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_new_override_editor(window, cx);
                    }))}
            </div>
        });

        container = container.child(rsx! {
            <div text_sm text_color={cx.theme().muted_foreground}>
                {"Holiday moves and deadline extensions issued by the BIR. These override the compiled base rules for matching form codes and dates."}
            </div>
        });

        // Status message
        if let Some(msg) = &self.status_message {
            container = container.child(rsx! {
                <div
                    p_3
                    bg={cx.theme().primary.opacity(0.1)}
                    border_1
                    border_color={cx.theme().primary.opacity(0.2)}
                    rounded_lg
                    text_sm
                    text_color={cx.theme().primary}
                >
                    {msg.clone()}
                </div>
            });
        }

        // Existing overrides
        for (idx, ovr) in self.overrides.iter().enumerate() {
            let delete_idx = idx;
            let edit_id = ovr.id.clone();
            container = container.child(rsx! {
                <div
                    id={SharedString::from(format!("override-{}", idx))}
                    w_full
                    p_4
                    bg={cx.theme().background}
                    border_1
                    border_color={cx.theme().border}
                    rounded_xl
                    shadow_sm
                    flex
                    justify_between
                    items_center
                >
                    <div flex flex_col gap_1>
                        <div
                            text_lg
                            font_weight={FontWeight::BOLD}
                            text_color={cx.theme().foreground}
                        >
                            {ovr.title.clone()}
                        </div>
                        <div text_sm text_color={cx.theme().muted_foreground}>
                            {format!(
                                "Source: {} | Forms: {}",
                                ovr.source_reference,
                                ovr.affected_form_codes.join(", ")
                            )}
                        </div>
                        <div text_sm text_color={cx.theme().muted_foreground}>
                            {format!(
                                "{} → {}",
                                ovr.original_deadline.format("%Y-%m-%d"),
                                ovr.adjusted_deadline.format("%Y-%m-%d")
                            )}
                        </div>
                    </div>
                    <div flex items_center gap_2>
                        {gpui_component::button::Button::new(SharedString::from(
                            format!("edit-override-{}", idx),
                        ))
                        .label("Edit")
                        .small()
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.open_edit_override_editor(&edit_id, window, cx);
                            },
                        ))}
                        {gpui_component::button::Button::new(SharedString::from(
                            format!("del-override-{}", idx),
                        ))
                        .icon(IconName::Close)
                        .ghost()
                        .small()
                        .on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.delete_override(delete_idx, cx);
                            },
                        ))}
                    </div>
                </div>
            });
        }

        if self.overrides.is_empty() {
            container = container.child(rsx! {
                <div w_full py_8 flex justify_center>
                    <div text_sm text_color={cx.theme().muted_foreground}>
                        {"No overrides configured. Base rules apply."}
                    </div>
                </div>
            });
        }

        container
    }

    fn render_override_editor_modal(&self, cx: &mut Context<'_, Self>) -> Div {
        let is_editing = self.editing_override_id.is_some();
        let mut selected_codes = self.new_override_form_codes.clone();
        selected_codes.sort();
        selected_codes.dedup();

        let mut chips = div().flex().flex_wrap().gap_1();
        for code in &selected_codes {
            let code_for_remove = code.clone();
            chips = chips.child(rsx! {
                <div
                    id={SharedString::from(format!("override-form-chip-{code}"))}
                    flex
                    items_center
                    gap_1
                    px_2
                    py_1
                    bg={cx.theme().secondary}
                    border_1
                    border_color={cx.theme().border}
                    rounded_md
                    text_xs
                    text_color={cx.theme().foreground}
                >
                    {code.clone()}
                    {div()
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.new_override_form_codes
                                    .retain(|existing| existing != &code_for_remove);
                                let codes = this.new_override_form_codes.clone();
                                this.new_override_forms_select.update(cx, |select, cx| {
                                    select.set_selected_ids(codes, cx);
                                });
                                cx.notify();
                            }),
                        )
                        .child(Icon::new(IconName::Close).xsmall())}
                </div>
            });
        }

        let field_label = |text: &'static str, cx: &Context<'_, Self>| {
            rsx! {
                <div text_xs text_color={cx.theme().muted_foreground} mb_1>
                    {text}
                </div>
            }
        };

        rsx! {
            <div
                absolute
                inset_0
                occlude
                bg={gpui::rgba(0x000000b2)}
                flex
                items_center
                justify_center
            >
                {div()
                    .w_full()
                    .max_w(px(640.))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_xl()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .shadow_lg()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(if is_editing {
                                "Edit Deadline Override"
                            } else {
                                "New Deadline Override"
                            }),
                    )
                    .when_some(self.override_editor_error.clone(), |this, message| {
                        this.child(rsx! {
                            <div
                                p_3
                                rounded_md
                                bg={cx.theme().danger.opacity(0.08)}
                                border_1
                                border_color={cx.theme().danger.opacity(0.5)}
                                text_sm
                                text_color={cx.theme().danger}
                            >
                                {message}
                            </div>
                        })
                    })
                    .child(rsx! {
                        <div flex gap_3>
                            <div flex_1>
                                {field_label("Title", cx)}
                                {Input::new(&self.new_override_title)}
                            </div>
                            <div flex_1>
                                {field_label("Source Reference", cx)}
                                {Input::new(&self.new_override_source)}
                            </div>
                        </div>
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(field_label("Affected Forms", cx))
                            .child(MultiSelect::new(&self.new_override_forms_select))
                            .when(!selected_codes.is_empty(), |this| {
                                this.child(rsx! {
                                    <div mt_2>
                                        {chips}
                                    </div>
                                })
                            })
                            .child(rsx! {
                                <div mt_1 text_xs text_color={cx.theme().muted_foreground}>
                                    {"Type to filter and check every official form this advisory covers."}
                                </div>
                            }),
                    )
                    .child(rsx! {
                        <div flex gap_3>
                            <div flex_1>
                                {field_label("Original Deadline", cx)}
                                {DateInput::new(&self.new_override_orig_date)}
                            </div>
                            <div flex_1>
                                {field_label("Adjusted Deadline", cx)}
                                {DateInput::new(&self.new_override_adj_date)}
                            </div>
                        </div>
                    })
                    .child(rsx! {
                        <div flex justify_end gap_2>
                            {gpui_component::button::Button::new("cancel-override-editor")
                                .label("Cancel")
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.close_override_editor(cx);
                                }))}
                            {gpui_component::button::Button::new("save-override")
                                .label(if is_editing {
                                    "Save Changes"
                                } else {
                                    "Add Override"
                                })
                                .primary()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.save_override(window, cx);
                                }))}
                        </div>
                    })}
            </div>
        }
    }
}

impl Render for AdminCalendarDashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(rsx! {
                <div
                    w_full
                    p_6
                    border_b_1
                    border_color={cx.theme().border}
                    flex
                    justify_between
                    items_center
                >
                    <div
                        text_2xl
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().foreground}
                    >
                        {"Tax Calendar Explorer"}
                    </div>
                    <div flex gap_2>
                        {self.render_tab_button(AdminTab::Deadlines, "Deadlines", cx)}
                        {self.render_tab_button(AdminTab::Rules, "Rules", cx)}
                        {self.render_tab_button(AdminTab::Overrides, "Overrides", cx)}
                    </div>
                </div>
            })
            .child(div().w_full().flex_1().overflow_y_scrollbar().p_6().child(
                match self.active_tab {
                    AdminTab::Deadlines => self.render_deadlines_tab(cx),
                    AdminTab::Rules => self.render_rules_tab(cx),
                    AdminTab::Overrides => self.render_overrides_tab(cx),
                },
            ))
            .when(self.override_editor_open, |this| {
                this.child(self.render_override_editor_modal(cx))
            })
    }
}
