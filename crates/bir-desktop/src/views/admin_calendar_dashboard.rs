use bir_core::calendar_rules::{
    DeadlineOverride, DeadlineResolver, OfficialRule, ResolvedTaxDeadline,
};
use bir_core::db::Database;
use chrono::NaiveDate;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement;
use gpui_component::*;
use std::sync::{Arc, Mutex};

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

pub struct AdminCalendarDashboard {
    db: Arc<Mutex<Database>>,
    active_tab: AdminTab,
    selected_year: i32,
    rules: Vec<OfficialRule>,
    deadlines: Vec<ResolvedTaxDeadline>,
    overrides: Vec<DeadlineOverride>,
    // Override creation form
    new_override_title: Entity<InputState>,
    new_override_source: Entity<InputState>,
    new_override_forms: Entity<InputState>,
    new_override_orig_date: Entity<InputState>,
    new_override_adj_date: Entity<InputState>,
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
        let new_override_forms = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Form codes, comma-separated (e.g. 1701Q,2551Q)")
        });
        let new_override_orig_date =
            cx.new(|cx| InputState::new(window, cx).placeholder("Original deadline (YYYY-MM-DD)"));
        let new_override_adj_date =
            cx.new(|cx| InputState::new(window, cx).placeholder("Adjusted deadline (YYYY-MM-DD)"));

        Self {
            db,
            active_tab: AdminTab::Deadlines,
            selected_year: year,
            rules,
            deadlines,
            overrides,
            new_override_title,
            new_override_source,
            new_override_forms,
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

    fn save_override(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let title = self.new_override_title.read(cx).value().trim().to_string();
        let source = self.new_override_source.read(cx).value().trim().to_string();
        let forms_str = self.new_override_forms.read(cx).value().trim().to_string();
        let orig_str = self
            .new_override_orig_date
            .read(cx)
            .value()
            .trim()
            .to_string();
        let adj_str = self
            .new_override_adj_date
            .read(cx)
            .value()
            .trim()
            .to_string();

        if title.is_empty() || source.is_empty() || forms_str.is_empty() {
            self.status_message = Some("Title, source, and form codes are required.".into());
            cx.notify();
            return;
        }

        let orig = match NaiveDate::parse_from_str(&orig_str, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                self.status_message =
                    Some("Invalid original deadline format. Use YYYY-MM-DD.".into());
                cx.notify();
                return;
            }
        };
        let adj = match NaiveDate::parse_from_str(&adj_str, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                self.status_message =
                    Some("Invalid adjusted deadline format. Use YYYY-MM-DD.".into());
                cx.notify();
                return;
            }
        };

        let form_codes: Vec<String> = forms_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let id = format!("override-{}", chrono::Local::now().timestamp_millis());

        let new_override = DeadlineOverride {
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
        };

        self.overrides.push(new_override);

        if let Ok(db) = self.db.lock() {
            let _ = db.set_deadline_overrides(&self.overrides);
        }

        // Clear form
        self.new_override_title
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.new_override_source
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.new_override_forms
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.new_override_orig_date
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.new_override_adj_date
            .update(cx, |s, cx| s.set_value("", window, cx));

        self.reload();
        self.status_message = Some("Override saved.".into());
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
        div()
            .id(SharedString::from(format!("tab-{label}")))
            .px_4()
            .py_2()
            .bg(if is_active {
                cx.theme().primary
            } else {
                cx.theme().secondary
            })
            .text_color(if is_active {
                cx.theme().primary_foreground
            } else {
                cx.theme().foreground
            })
            .rounded_md()
            .font_weight(FontWeight::BOLD)
            .cursor_pointer()
            .hover(|s| s.opacity(0.8))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.active_tab = tab;
                cx.notify();
            }))
            .child(label)
    }

    fn render_deadlines_tab(&self, cx: &mut Context<'_, Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .pb(px(96.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        gpui_component::button::Button::new("prev-year")
                            .icon(IconName::ChevronLeft)
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| this.change_year(-1, cx))),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(format!("{} Calendar-Year Deadlines", self.selected_year)),
                    )
                    .child(
                        gpui_component::button::Button::new("next-year")
                            .icon(IconName::ChevronRight)
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| this.change_year(1, cx))),
                    ),
            )
            .children(self.deadlines.iter().enumerate().map(|(idx, d)| {
                div()
                    .id(SharedString::from(format!("deadline-{}", idx)))
                    .w_full()
                    .p_4()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_xl()
                    .shadow_sm()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child(format!("{} - {}", d.display_form_no, d.form_name)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(d.period.label()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().primary)
                                    .child(format!("Due: {}", d.final_deadline_string())),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .bg(cx.theme().secondary)
                                    .text_color(cx.theme().foreground)
                                    .rounded_full()
                                    .text_xs()
                                    .child(d.status.label()),
                            ),
                    )
            }))
    }

    fn render_rules_tab(&self, cx: &mut Context<'_, Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .pb(px(96.))
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child("Official BIR Tax Rules (Compile-Time)"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("These rules are seeded statically from official BIR guidelines and cannot be modified dynamically."),
            )
            .children(self.rules.iter().enumerate().map(|(idx, rule)| {
                div()
                    .id(SharedString::from(format!("rule-{}", idx)))
                    .w_full()
                    .p_4()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_xl()
                    .shadow_sm()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(rule.form_name),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Forms: {}", rule.form_nos.join(", "))),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .bg(cx.theme().primary)
                                    .text_color(cx.theme().primary_foreground)
                                    .rounded_md()
                                    .text_xs()
                                    .child(format!("{:?}", rule.frequency)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().foreground)
                                    .child(rule.description),
                            ),
                    )
            }))
    }

    fn render_overrides_tab(&self, window: &mut Window, cx: &mut Context<'_, Self>) -> Div {
        let mut container = div().flex().flex_col().gap_4().pb(px(96.));

        container = container.child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .text_color(cx.theme().foreground)
                .child("Deadline Overrides (BIR Advisories)"),
        );

        container = container.child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("Add deadline extensions issued by the BIR. These override the compiled base rules for matching form codes and dates."),
        );

        // Status message
        if let Some(msg) = &self.status_message {
            container = container.child(
                div()
                    .p_3()
                    .bg(cx.theme().primary.opacity(0.1))
                    .border_1()
                    .border_color(cx.theme().primary.opacity(0.2))
                    .rounded_lg()
                    .text_sm()
                    .text_color(cx.theme().primary)
                    .child(msg.clone()),
            );
        }

        // Override creation form
        container = container.child(
            div()
                .w_full()
                .p_4()
                .bg(cx.theme().secondary)
                .border_1()
                .border_color(cx.theme().border)
                .rounded_xl()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().foreground)
                        .child("New Override"),
                )
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(div().flex_1().child(Input::new(&self.new_override_title)))
                        .child(div().flex_1().child(Input::new(&self.new_override_source))),
                )
                .child(Input::new(&self.new_override_forms))
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .child(Input::new(&self.new_override_orig_date)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(Input::new(&self.new_override_adj_date)),
                        ),
                )
                .child(
                    gpui_component::button::Button::new("save-override")
                        .label("Save Override")
                        .primary()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.save_override(window, cx);
                        })),
                ),
        );

        // Existing overrides
        for (idx, ovr) in self.overrides.iter().enumerate() {
            let delete_idx = idx;
            container = container.child(
                div()
                    .id(SharedString::from(format!("override-{}", idx)))
                    .w_full()
                    .p_4()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_xl()
                    .shadow_sm()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child(ovr.title.clone()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "Source: {} | Forms: {}",
                                        ovr.source_reference,
                                        ovr.affected_form_codes.join(", ")
                                    )),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "{} → {}",
                                        ovr.original_deadline.format("%Y-%m-%d"),
                                        ovr.adjusted_deadline.format("%Y-%m-%d")
                                    )),
                            ),
                    )
                    .child(
                        gpui_component::button::Button::new(SharedString::from(format!(
                            "del-override-{}",
                            idx
                        )))
                        .icon(IconName::Close)
                        .ghost()
                        .small()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.delete_override(delete_idx, cx);
                        })),
                    ),
            );
        }

        if self.overrides.is_empty() {
            container = container.child(
                div().w_full().py_8().flex().justify_center().child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("No overrides configured. Base rules apply."),
                ),
            );
        }

        container
    }
}

impl Render for AdminCalendarDashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(
                div()
                    .w_full()
                    .p_6()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child("Tax Calendar Explorer"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(self.render_tab_button(AdminTab::Deadlines, "Deadlines", cx))
                            .child(self.render_tab_button(AdminTab::Rules, "Rules", cx))
                            .child(self.render_tab_button(AdminTab::Overrides, "Overrides", cx)),
                    ),
            )
            .child(div().w_full().flex_1().overflow_y_scrollbar().p_6().child(
                match self.active_tab {
                    AdminTab::Deadlines => self.render_deadlines_tab(cx),
                    AdminTab::Rules => self.render_rules_tab(cx),
                    AdminTab::Overrides => self.render_overrides_tab(window, cx),
                },
            ))
    }
}
