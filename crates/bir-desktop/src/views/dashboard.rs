use bir_core::db::Database;
use bir_core::forms::form_2551q::{FormFilingProgress, QuarterState};
use bir_core::forms::registry::FilingFrequency;
use bir_core::profile::TaxpayerProfile;
use chrono::{Datelike, Local};
use gpui::*;
use gpui_component::*;
use std::sync::{Arc, Mutex};

use crate::components::filter_bar::{FilterBar, FilterEvent, FilterState};
use crate::components::smart_date_filter::{
    SmartDateFilter, SmartDateFilterEvent, SmartDateFilterState,
};

pub enum DashboardEvent {
    FileForm {
        form_code: String,
        year: u16,
        quarter: u8,
    },
    Reload,
}

impl EventEmitter<DashboardEvent> for DashboardView {}

pub struct DashboardView {
    active_profile: Option<TaxpayerProfile>,
    db: Arc<Mutex<Database>>,
    selected_year: i32,
    smart_date_filter: Entity<SmartDateFilterState>,
    filter_state: Entity<FilterState>,
    /// Cached filing progress per form code -> per year
    filing_progress:
        std::collections::HashMap<String, std::collections::HashMap<u16, FormFilingProgress>>,
}

impl DashboardView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let now = Local::now().date_naive();

        let smart_date_filter = cx.new(|cx| SmartDateFilterState::new(window, cx));

        cx.subscribe(
            &smart_date_filter,
            |this: &mut Self, _, event: &SmartDateFilterEvent, cx| {
                this.selected_year = event.year;
                if let Some(profile) = this.active_profile.clone() {
                    this.reload_filing_progress(&profile);
                }
                cx.notify();
            },
        )
        .detach();

        let filter_state = cx.new(|cx| FilterState::new(window, cx));
        cx.subscribe(&filter_state, |_, _, _event: &FilterEvent, cx| {
            cx.notify();
        })
        .detach();

        Self {
            active_profile: None,
            db,
            selected_year: now.year(),
            smart_date_filter,
            filter_state,
            filing_progress: std::collections::HashMap::new(),
        }
    }

    pub fn set_profile(&mut self, profile: TaxpayerProfile, cx: &mut Context<Self>) {
        self.filter_state.update(cx, |state, cx| {
            state.update_for_profile(&profile.taxpayer_type, profile.is_vat_registered, cx);
        });
        self.reload_filing_progress(&profile);
        self.active_profile = Some(profile);
        cx.notify();
    }

    /// Query the DB for all form progress for this profile in the selected year.
    fn reload_filing_progress(&mut self, profile: &TaxpayerProfile) {
        self.filing_progress.clear();
        if let Ok(db) = self.db.lock() {
            let form_codes = ["2551Q", "1702Q", "1702RT", "1701Q", "1701", "2550M"];
            let year = self.selected_year as u16;

            for code in form_codes {
                let mut year_progress = std::collections::HashMap::new();
                if let Ok(progress) = db.get_form_filing_progress(&profile.tin.full(), code, year) {
                    year_progress.insert(year, progress);
                }
                self.filing_progress.insert(code.to_string(), year_progress);
            }
        }
    }

    /// Colors and labels for a given QuarterState
    fn state_style(
        state: &QuarterState,
        cx: &Context<DashboardView>,
    ) -> (Hsla, Hsla, Hsla, &'static str, &'static str) {
        // Returns (bg_color, border_color, accent_color, icon, status_label)
        match state {
            QuarterState::Paid => (
                gpui::rgba(0x10b98120).into(),
                gpui::rgba(0x10b981ff).into(),
                gpui::rgba(0x10b981ff).into(),
                "$",
                "Paid",
            ),
            QuarterState::Confirmed => (
                gpui::rgba(0x22c55e20).into(),
                gpui::rgba(0x22c55eff).into(),
                gpui::rgba(0x22c55eff).into(),
                "✓",
                "Filed",
            ),
            QuarterState::Submitted => (
                gpui::rgba(0x3b82f620).into(),
                gpui::rgba(0x3b82f6ff).into(),
                gpui::rgba(0x3b82f6ff).into(),
                "◐",
                "Sent",
            ),
            QuarterState::Queued => (
                gpui::rgba(0xa855f720).into(),
                gpui::rgba(0xa855f7ff).into(),
                gpui::rgba(0xa855f7ff).into(),
                "↻",
                "Queued",
            ),
            QuarterState::Draft => (
                gpui::rgba(0xfacc1520).into(),
                gpui::rgba(0xfacc15ff).into(),
                gpui::rgba(0xfacc15ff).into(),
                "~",
                "Draft",
            ),
            QuarterState::NotStarted => (
                gpui::transparent_black().into(),
                cx.theme().border,
                cx.theme().muted_foreground,
                "+",
                "New",
            ),
        }
    }

    /// Hover background for a given QuarterState
    fn state_hover_bg(state: &QuarterState, cx: &Context<DashboardView>) -> Hsla {
        match state {
            QuarterState::Paid => gpui::rgba(0x10b98140).into(),
            QuarterState::Confirmed => gpui::rgba(0x22c55e40).into(),
            QuarterState::Submitted => gpui::rgba(0x3b82f640).into(),
            QuarterState::Queued => gpui::rgba(0xa855f740).into(),
            QuarterState::Draft => gpui::rgba(0xfacc1540).into(),
            QuarterState::NotStarted => cx.theme().accent,
        }
    }
}

impl Render for DashboardView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let Some(profile) = &self.active_profile else {
            return div()
                .flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_xl()
                        .text_color(cx.theme().muted_foreground)
                        .child("Select a Taxpayer Profile from the Sidebar"),
                )
                .into_any_element();
        };

        let year = self.selected_year as u16;
        let current_type = &profile.taxpayer_type;
        let is_vat_registered = profile.is_vat_registered;

        // Get forms for this taxpayer type using the registry
        let mut available_forms: Vec<&bir_core::forms::registry::FormDefinition> =
            bir_core::forms::registry::FORM_REGISTRY
                .iter()
                .filter(|f| f.taxpayer_types.contains(current_type))
                .filter(|f| {
                    if is_vat_registered {
                        f.code != "2551Q"
                    } else {
                        f.code != "2550M"
                    }
                })
                .collect();

        let filter_chips = self.filter_state.read(cx).active_chips.clone();
        let query = self
            .filter_state
            .read(cx)
            .input_state
            .read(cx)
            .value()
            .to_string()
            .to_lowercase();

        // Extract period filter chips
        let active_quarters: Vec<u8> = filter_chips
            .iter()
            .filter(|c| c.group == "Quarter")
            .filter_map(|c| c.label.strip_prefix('Q').and_then(|n| n.parse::<u8>().ok()))
            .collect();
        let has_quarter_filter = !active_quarters.is_empty();

        let active_months: Vec<u8> = filter_chips
            .iter()
            .filter(|c| c.group == "Month")
            .filter_map(|c| Self::month_label_to_num(&c.label))
            .collect();
        let has_month_filter = !active_months.is_empty();

        // When quarter filter is active, only show Quarterly forms
        // When month filter is active, only show Monthly forms
        if has_quarter_filter {
            available_forms.retain(|f| matches!(f.frequency, FilingFrequency::Quarterly));
        }
        if has_month_filter {
            available_forms.retain(|f| matches!(f.frequency, FilingFrequency::Monthly));
        }

        // Apply form type and text search filters
        if !filter_chips.is_empty() || !query.is_empty() {
            available_forms.retain(|f| {
                let form_type_chips: Vec<_> = filter_chips
                    .iter()
                    .filter(|c| c.group == "Form Type")
                    .collect();
                let matches_form_type = if form_type_chips.is_empty() {
                    true
                } else {
                    form_type_chips
                        .iter()
                        .any(|c| f.code.eq_ignore_ascii_case(&c.label))
                };

                let matches_query = if query.is_empty() {
                    true
                } else {
                    f.code.to_lowercase().contains(&query)
                        || f.title.to_lowercase().contains(&query)
                };

                matches_form_type && matches_query
            });
        }

        let mut forms_ui = div().flex().flex_row().flex_wrap().gap_6().w_full();
        let mut cards_rendered = 0;

        for form_def in &available_forms {
            let code = form_def.code.to_string();
            let progress = self
                .filing_progress
                .get(&code)
                .and_then(|y| y.get(&year))
                .cloned();

            let card = match &form_def.frequency {
                FilingFrequency::Quarterly => {
                    let quarters = progress.as_ref().map(|p| p.quarters.clone()).unwrap_or([
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                    ]);
                    let filed = quarters
                        .iter()
                        .filter(|s| {
                            **s == QuarterState::Submitted
                                || **s == QuarterState::Confirmed
                                || **s == QuarterState::Paid
                        })
                        .count();

                    // Interactive quarter dots — each one is clickable
                    let mut quarter_dots = div().flex().gap_2().w_full();
                    for (idx, q_state) in quarters.iter().enumerate() {
                        let q_num = (idx + 1) as u8;
                        let should_dim = has_quarter_filter && !active_quarters.contains(&q_num);

                        if should_dim {
                            quarter_dots = quarter_dots.child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .gap_1()
                                    .opacity(0.2)
                                    .py_3()
                                    .rounded_xl()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(gpui::transparent_black())
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("Q{}", q_num)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("-"),
                                    ),
                            );
                        } else {
                            let (bg, border_clr, accent, _icon, status_label) =
                                Self::state_style(q_state, cx);
                            let hover_bg = Self::state_hover_bg(q_state, cx);
                            let code_click = code.clone();

                            quarter_dots = quarter_dots.child(
                                div()
                                    .id(format!("q_{}_{}_{}", form_def.code, year, q_num))
                                    .flex()
                                    .flex_1()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .gap_1()
                                    .py_3()
                                    .cursor_pointer()
                                    .rounded_xl()
                                    .border_1()
                                    .border_color(border_clr)
                                    .bg(bg)
                                    .hover(move |s| s.bg(hover_bg).border_color(accent))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if matches!(q_state, QuarterState::NotStarted) { cx.theme().foreground } else { accent })
                                            .child(format!("Q{}", q_num)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(accent)
                                            .child(status_label),
                                    )
                                    .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                        cx.emit(DashboardEvent::FileForm {
                                            form_code: code_click.clone(),
                                            year,
                                            quarter: q_num,
                                        });
                                    })),
                            );
                        }
                    }

                    Self::build_card(form_def, year, cx).child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("Year {}:", year)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child(format!("{}/4 filed", filed)),
                                    ),
                            )
                            .child(quarter_dots),
                    )
                }
                FilingFrequency::Monthly => {
                    let months = progress.as_ref().map(|p| p.months.clone()).unwrap_or([
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                        QuarterState::NotStarted,
                    ]);
                    let filed = months
                        .iter()
                        .filter(|s| {
                            **s == QuarterState::Submitted
                                || **s == QuarterState::Confirmed
                                || **s == QuarterState::Paid
                        })
                        .count();

                    let month_names = [
                        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct",
                        "Nov", "Dec",
                    ];
                    let mut month_dots = div().flex().flex_col().gap_2().w_full();

                    for chunk in months.chunks(4).enumerate() {
                        let (chunk_idx, chunk) = chunk;
                        let mut row = div().flex().gap_2().w_full();

                        for (idx, m_state) in chunk.iter().enumerate() {
                            let absolute_idx = chunk_idx * 4 + idx;
                            let m_num = (absolute_idx + 1) as u8;
                            let should_dim = has_month_filter && !active_months.contains(&m_num);

                            if should_dim {
                                row = row.child(
                                    div()
                                        .flex()
                                        .flex_1()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .gap_1()
                                        .opacity(0.2)
                                        .py_3()
                                        .rounded_xl()
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .bg(gpui::transparent_black())
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().muted_foreground)
                                                .child(month_names[absolute_idx]),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child("-")
                                        ),
                                );
                            } else {
                                let (bg, border_clr, accent, _icon, status_label) =
                                    Self::state_style(m_state, cx);
                                let hover_bg = Self::state_hover_bg(m_state, cx);
                                let code_click = code.clone();

                                row = row.child(
                                    div()
                                        .id(format!("m_{}_{}_{}", form_def.code, year, m_num))
                                        .flex()
                                        .flex_1()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .gap_1()
                                        .py_3()
                                        .cursor_pointer()
                                        .rounded_xl()
                                        .border_1()
                                        .border_color(border_clr)
                                        .bg(bg)
                                        .hover(move |s| s.bg(hover_bg).border_color(accent))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(if matches!(m_state, QuarterState::NotStarted) { cx.theme().foreground } else { accent })
                                                .child(month_names[absolute_idx]),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(accent)
                                                .child(status_label)
                                        )
                                        .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                            cx.emit(DashboardEvent::FileForm {
                                                form_code: code_click.clone(),
                                                year,
                                                quarter: m_num,
                                            });
                                        })),
                                );
                            }
                        }
                        month_dots = month_dots.child(row);
                    }

                    Self::build_card(form_def, year, cx).child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("Year {}:", year)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child(format!("{}/12 filed", filed)),
                                    ),
                            )
                            .child(month_dots),
                    )
                }
                FilingFrequency::Annual => {
                    let status = progress
                        .as_ref()
                        .map(|p| p.annual_status.clone())
                        .unwrap_or(QuarterState::NotStarted);

                    let (bg, border_clr, accent, icon, _action_label) =
                        Self::state_style(&status, cx);
                    let hover_bg = Self::state_hover_bg(&status, cx);
                    let code_click = code.clone();

                    Self::build_card(form_def, year, cx)
                        .child(
                            div().flex().justify_between().items_center().child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("Year {}:", year)),
                            ),
                        )
                        .child(
                            div()
                                .id(format!("annual_{}_{}", form_def.code, year))
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap_3()
                                .py_3()
                                .px_6()
                                .rounded_full()
                                .border_1()
                                .border_color(border_clr)
                                .bg(bg)
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg).border_color(accent))
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(accent)
                                        .child(icon),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(if matches!(&status, QuarterState::NotStarted) { cx.theme().foreground } else { accent })
                                        .child(match &status {
                                            QuarterState::Paid => "View Paid Return",
                                            QuarterState::Confirmed => "View Filed Return",
                                            QuarterState::Submitted => "View Submission",
                                            QuarterState::Queued => "View Queued Return",
                                            QuarterState::Draft => "Resume Draft",
                                            QuarterState::NotStarted => "File Annual Return",
                                        }),
                                )
                                .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                    cx.emit(DashboardEvent::FileForm {
                                        form_code: code_click.clone(),
                                        year,
                                        quarter: 0,
                                    });
                                })),
                        )
                }
                FilingFrequency::OpenEnded => {
                    let count = progress.as_ref().map(|p| p.open_ended_count).unwrap_or(0);
                    let code_click = code.clone();
                    let hover_bg = cx.theme().accent;
                    let primary = cx.theme().primary;

                    Self::build_card(form_def, year, cx)
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("Year {}:", year)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(cx.theme().foreground)
                                        .child(format!("{} filed", count)),
                                ),
                        )
                        .child(
                            div()
                                .id(format!("monthly_{}_{}", form_def.code, year))
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap_3()
                                .py_3()
                                .px_6()
                                .rounded_full()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(gpui::transparent_black())
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg).border_color(primary))
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(cx.theme().foreground)
                                        .child("+"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(cx.theme().foreground)
                                        .child("File New Return"),
                                )
                                .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                    cx.emit(DashboardEvent::FileForm {
                                        form_code: code_click.clone(),
                                        year,
                                        quarter: 0,
                                    });
                                })),
                        )
                }
            };

            forms_ui = forms_ui.child(card);
            cards_rendered += 1;
        }

        let main_content = if cards_rendered == 0 {
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .w_full()
                .py_24()
                .gap_4()
                .child(
                    Icon::new(IconName::Search)
                        .size(px(48.))
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().foreground)
                        .child("No forms match your filters"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("Try adjusting your form type filters or year selection."),
                )
        } else {
            forms_ui
        };

        // Build subtitle with active period filters
        let period_desc = if has_quarter_filter {
            let qs: Vec<String> = active_quarters.iter().map(|q| format!("Q{}", q)).collect();
            format!("Tax Year {} • {}", self.selected_year, qs.join(", "))
        } else if has_month_filter {
            let ms: Vec<String> = active_months
                .iter()
                .filter_map(|m| Self::month_num_to_label(*m))
                .collect();
            format!("Tax Year {} • {}", self.selected_year, ms.join(", "))
        } else {
            format!("Tax Year {}", self.selected_year)
        };

        div()
            .id("dashboard-scroll")
            .size_full()
            .flex()
            .flex_col()
            .justify_start()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .justify_start()
                    .gap_6()
                    .p_6()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_3xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child(format!("{} Dashboard", profile.full_name)),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "TIN: {} • Type: {:?} • {} • {}",
                                        profile.tin.full(),
                                        profile.taxpayer_type,
                                        period_desc,
                                        if profile.is_vat_registered {
                                            "VAT"
                                        } else {
                                            "Non-VAT"
                                        }
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(div().flex_grow().child(FilterBar::new(&self.filter_state)))
                            .child(SmartDateFilter::new(&self.smart_date_filter)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Tax Form Library"),
                            )
                            .child(main_content),
                    ),
            )
            .into_any_element()
    }
}

impl DashboardView {
    /// Build the common card shell (header + title). Caller appends progress + action sections.
    fn build_card(
        form_def: &bir_core::forms::registry::FormDefinition,
        _year: u16,
        cx: &Context<Self>,
    ) -> Div {
        div()
            .flex_1()
            .min_w(px(340.))
            .max_w(px(460.))
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_xl()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .hover(|style| style.border_color(cx.theme().primary))
            .shadow_sm()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(FontWeight::BLACK)
                            .text_color(cx.theme().primary)
                            .child(form_def.code),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .bg(cx.theme().secondary)
                            .px_3()
                            .py_1()
                            .rounded_full()
                            .text_color(cx.theme().secondary_foreground)
                            .child(form_def.category),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .line_height(relative(1.3))
                    .text_color(cx.theme().foreground)
                    .child(form_def.title),
            )
    }

    fn month_label_to_num(label: &str) -> Option<u8> {
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

    fn month_num_to_label(num: u8) -> Option<String> {
        match num {
            1 => Some("Jan".into()),
            2 => Some("Feb".into()),
            3 => Some("Mar".into()),
            4 => Some("Apr".into()),
            5 => Some("May".into()),
            6 => Some("Jun".into()),
            7 => Some("Jul".into()),
            8 => Some("Aug".into()),
            9 => Some("Sep".into()),
            10 => Some("Oct".into()),
            11 => Some("Nov".into()),
            12 => Some("Dec".into()),
            _ => None,
        }
    }
}
