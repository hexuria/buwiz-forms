use bir_core::calendar_rules::{DeadlinePeriod, DeadlineResolver, ResolvedTaxDeadline};
use bir_core::db::{BirNotice, Database, NoticeSourceKind};
use bir_core::forms::registry::FilingFrequency;
use bir_core::forms::{
    FormFilingProgress, QuarterState, can_open_certification_draft, form_support_level,
};
use bir_core::profile::TaxpayerProfile;
use chrono::{Datelike, Local, NaiveDate};
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
    LogoutProfile(String),
    OpenProfileManager,
}

impl EventEmitter<DashboardEvent> for DashboardView {}

const SIDEBAR_FULL_WIDTH: f32 = 280.0;
const SIDEBAR_MINI_WIDTH: f32 = 70.0;
const CONTENT_PADDING_X: f32 = 64.0;
const GRID_GAP: f32 = 20.0;
const DESKTOP_3_COL_MIN_CONTENT: f32 = 1135.0; // 365 * 3 + 20 * 2
const TABLET_2_COL_MIN_CONTENT: f32 = 750.0; // 365 * 2 + 20 * 1

fn tax_form_grid_columns(window_width: f32, sidebar_width: f32) -> usize {
    let available_content_width = window_width - sidebar_width - CONTENT_PADDING_X;
    if available_content_width >= DESKTOP_3_COL_MIN_CONTENT {
        3
    } else if available_content_width >= TABLET_2_COL_MIN_CONTENT {
        2
    } else {
        1
    }
}

fn tax_form_card_width(window_width: f32, sidebar_width: f32, columns: usize) -> f32 {
    let available_content_width = window_width - sidebar_width - CONTENT_PADDING_X;
    if columns <= 1 {
        available_content_width
    } else {
        let total_gap_width = GRID_GAP * (columns as f32 - 1.0);
        (available_content_width - total_gap_width) / columns as f32
    }
}

pub enum ProfileTab {
    Calendar,
    Forms,
}

pub struct DashboardView {
    active_profile: Option<TaxpayerProfile>,
    db: Arc<Mutex<Database>>,
    selected_year: i32,
    smart_date_filter: Entity<SmartDateFilterState>,
    filter_state: Entity<FilterState>,
    /// Cached filing progress per form code -> per year
    filing_progress:
        std::collections::HashMap<String, std::collections::HashMap<u16, FormFilingProgress>>,
    is_mini_sidebar: bool,
    pub hide_tax_profiles: bool,
    active_tab: ProfileTab,
    deadlines: Vec<ResolvedTaxDeadline>,
    actionable_forms: Vec<(String, bir_core::forms::FormDraftSummary)>,
    upcoming_deadlines_list:
        Entity<crate::components::upcoming_deadlines_list::UpcomingDeadlinesList>,
    /// Consistency issues from the last resolve_profile_obligations_for_year call
    consistency_issues: Vec<bir_core::integration::ProfileConsistencyIssue>,
}

impl DashboardView {
    pub fn set_mini_sidebar(&mut self, is_mini: bool, cx: &mut Context<Self>) {
        self.is_mini_sidebar = is_mini;
        cx.notify();
    }

    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let now = Local::now().date_naive();

        let smart_date_filter = cx.new(|cx| SmartDateFilterState::new(window, cx));

        cx.subscribe(
            &smart_date_filter,
            |this: &mut Self, _, event: &SmartDateFilterEvent, cx| {
                this.selected_year = event.year;
                if let Some(profile) = this.active_profile.clone() {
                    this.reload_filing_progress(&profile, cx);
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

        // Auto-refresh when bir-daemon updates the database (e.g., form status changes)
        let bus = cx.global::<crate::events::GlobalEventBus>().0.clone();
        cx.subscribe(
            &bus,
            |this: &mut Self, _bus, event: &crate::events::AppEvent, cx| match event {
                crate::events::AppEvent::DatabaseChanged => {
                    this.reload_active_profile_from_database(cx);
                }
                crate::events::AppEvent::ProfileComplianceChanged { tin, .. } => {
                    if this
                        .active_profile
                        .as_ref()
                        .is_some_and(|profile| profile.tin.full() == *tin)
                    {
                        this.reload_active_profile_from_database(cx);
                    }
                }
            },
        )
        .detach();

        let upcoming_deadlines_list = cx.new(|cx| {
            crate::components::upcoming_deadlines_list::UpcomingDeadlinesList::new(window, cx)
        });

        cx.subscribe(
            &upcoming_deadlines_list,
            |this: &mut Self, _, event: &crate::components::upcoming_deadlines_list::UpcomingDeadlinesListEvent, cx| match event {
                crate::components::upcoming_deadlines_list::UpcomingDeadlinesListEvent::DeadlineClicked {
                    form_code,
                    year,
                    quarter,
                } => {
                    cx.emit(DashboardEvent::FileForm {
                        form_code: form_code.clone(),
                        year: *year,
                        quarter: *quarter,
                    });
                }
            },
        )
        .detach();

        Self {
            active_profile: None,
            db,
            selected_year: now.year(),
            smart_date_filter,
            filter_state,
            filing_progress: std::collections::HashMap::new(),
            is_mini_sidebar: false,
            hide_tax_profiles: false,
            active_tab: ProfileTab::Calendar,
            deadlines: Vec::new(),
            actionable_forms: Vec::new(),
            upcoming_deadlines_list,
            consistency_issues: Vec::new(),
        }
    }

    pub fn set_profile(&mut self, profile: TaxpayerProfile, cx: &mut Context<Self>) {
        let min_year = if let Some(start_date) = &profile.business_start_date {
            use chrono::Datelike;
            start_date.year()
        } else {
            1997
        };
        self.smart_date_filter.update(cx, |filter, cx| {
            filter.set_min_year(min_year, cx);
        });

        self.active_profile = Some(profile.clone());
        self.reload_filing_progress(&profile, cx);
        cx.notify();
    }

    /// Reload the active profile before recomputing Forms Set progress.
    ///
    /// `ProfileComplianceChanged` is emitted only after the profile and all
    /// affected yearly Forms Sets commit. Reusing the cached profile here
    /// would therefore recompute the dashboard from the pre-transaction facts.
    fn reload_active_profile_from_database(&mut self, cx: &mut Context<Self>) {
        let Some(tin) = self
            .active_profile
            .as_ref()
            .map(|profile| profile.tin.full())
        else {
            return;
        };
        let refreshed = self
            .db
            .lock()
            .ok()
            .and_then(|db| db.get_profile(&tin).ok().flatten());
        if let Some(profile) = refreshed {
            self.set_profile(profile, cx);
        }
    }

    /// Query the DB for all form progress for this profile in the selected year.
    fn reload_filing_progress(&mut self, profile: &TaxpayerProfile, cx: &mut Context<Self>) {
        self.filing_progress.clear();
        self.actionable_forms.clear();
        self.deadlines.clear();

        if let Ok(db) = self.db.lock() {
            let year = self.selected_year as u16;
            let obligations =
                bir_core::integration::resolve_profile_obligations_for_year(profile, year);
            let recurring_codes = obligations.form_codes;
            let applicable_codes = profile.active_form_codes_for_year(year);
            self.consistency_issues = obligations.consistency_report.issues;

            for code in applicable_codes.clone() {
                let mut year_progress = std::collections::HashMap::new();
                if let Ok(progress) = db.get_form_filing_progress(&profile.tin.full(), &code, year)
                {
                    year_progress.insert(year, progress);
                }
                self.filing_progress.insert(code, year_progress);
            }

            if let Ok(summaries) = db.list_draft_summaries(&profile.tin.full(), year) {
                // Only keep drafts that are applicable for this profile AND not yet done
                let codes_set_for_filter: std::collections::HashSet<String> =
                    applicable_codes.iter().cloned().collect();
                for sum in summaries {
                    // Skip forms not applicable to this profile
                    if !codes_set_for_filter.contains(&sum.form_code) {
                        continue;
                    }
                    // Only Paid means the taxpayer has complied. Submitted and
                    // Confirmed still need payment or explicit paid marking.
                    if matches!(sum.status, bir_core::forms::FilingStatus::Paid) {
                        continue;
                    }
                    self.actionable_forms.push((profile.full_name.clone(), sum));
                }
            }

            let mut overrides = db.get_deadline_overrides();
            overrides.extend(bir_core::integration::profile_deadline_overrides_for_year(
                profile, year,
            ));
            let deadlines_for_year =
                DeadlineResolver::deadlines_for_forms(&recurring_codes, year as i32, &overrides);
            self.deadlines = deadlines_for_year
                .into_iter()
                .filter(|deadline| {
                    bir_core::integration::deadline_applies_to_profile(profile, deadline)
                })
                .collect();

            // Calculate filtered available forms to sync with filter bar popover
            let mut available_codes = applicable_codes.clone();
            let monthly_quarterly_pairs: &[(&str, &str)] =
                &[("2550M", "2550Q"), ("2551M", "2551Q")];
            available_codes.retain(|code| {
                if let Some((_monthly, quarterly)) =
                    monthly_quarterly_pairs.iter().find(|(m, _)| m == code)
                {
                    if let Some(q_progress) = self
                        .filing_progress
                        .get(*quarterly)
                        .and_then(|y| y.get(&year))
                    {
                        let quarterly_has_filing = q_progress.quarters.iter().any(|q| {
                            matches!(
                                q,
                                QuarterState::Draft
                                    | QuarterState::Queued
                                    | QuarterState::Submitted
                                    | QuarterState::Confirmed
                                    | QuarterState::Paid
                            )
                        });
                        if quarterly_has_filing {
                            return false;
                        }
                    }
                }
                true
            });

            // Push to filter state
            self.filter_state.update(cx, |state, cx| {
                state.update_for_profile(profile, year, available_codes, cx);
            });
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
                "Confirmed",
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
                gpui::transparent_black(),
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

    fn deadline_matches_filters(
        deadline: &ResolvedTaxDeadline,
        months: &[u8],
        quarters: &[u8],
        include_annual: bool,
        has_period_filter: bool,
        current_month: u8,
    ) -> bool {
        let Some(date) = deadline.final_deadline_date() else {
            return false;
        };

        if !has_period_filter {
            return date.month() as u8 == current_month;
        }

        let is_annual = matches!(deadline.period, DeadlinePeriod::Annual { .. });
        if include_annual && is_annual {
            return true;
        }
        if is_annual {
            return false;
        }

        deadline.matches_period_filter(months, quarters)
    }

    fn quarter_matches_month_filter(quarter: u8, months: &[u8]) -> bool {
        let first_month = ((quarter - 1) * 3) + 1;
        let last_month = first_month + 2;
        months
            .iter()
            .any(|month| (first_month..=last_month).contains(month))
    }

    fn summary_matches_period_filters(
        summary: &bir_core::forms::FormDraftSummary,
        months: &[u8],
        quarters: &[u8],
        include_annual: bool,
        has_period_filter: bool,
    ) -> bool {
        if !has_period_filter {
            return true;
        }

        if let Some(month) = summary.month {
            let quarter = ((month - 1) / 3) + 1;
            return (!months.is_empty() && months.contains(&month))
                || (!quarters.is_empty() && quarters.contains(&quarter));
        }

        if let Some(quarter) = summary.quarter {
            return (!quarters.is_empty() && quarters.contains(&quarter))
                || (!months.is_empty() && Self::quarter_matches_month_filter(quarter, months));
        }

        include_annual
    }

    fn deadline_matches_summary(
        deadline: &ResolvedTaxDeadline,
        summary: &bir_core::forms::FormDraftSummary,
    ) -> bool {
        if deadline.form_code != summary.form_code {
            return false;
        }

        match deadline.period {
            DeadlinePeriod::Monthly {
                taxable_year,
                month,
            } => taxable_year == i32::from(summary.taxable_year) && summary.month == Some(month),
            DeadlinePeriod::Quarterly {
                taxable_year,
                quarter,
            } => {
                taxable_year == i32::from(summary.taxable_year) && summary.quarter == Some(quarter)
            }
            DeadlinePeriod::Annual { taxable_year } => {
                taxable_year == i32::from(summary.taxable_year)
                    && summary.month.is_none()
                    && summary.quarter.is_none()
            }
            DeadlinePeriod::EventBased => false,
        }
    }

    fn deadline_for_summary<'a>(
        deadlines: &'a [ResolvedTaxDeadline],
        summary: &bir_core::forms::FormDraftSummary,
    ) -> Option<&'a ResolvedTaxDeadline> {
        deadlines
            .iter()
            .find(|deadline| Self::deadline_matches_summary(deadline, summary))
    }

    fn summary_is_overdue(
        deadlines: &[ResolvedTaxDeadline],
        summary: &bir_core::forms::FormDraftSummary,
        today: NaiveDate,
    ) -> bool {
        Self::deadline_for_summary(deadlines, summary)
            .and_then(ResolvedTaxDeadline::final_deadline_date)
            .is_some_and(|deadline| deadline < today)
    }

    fn paid_state(state: &QuarterState) -> bool {
        matches!(state, QuarterState::Paid)
    }

    fn deadline_is_satisfied(
        deadline: &ResolvedTaxDeadline,
        filing_progress: &std::collections::HashMap<
            String,
            std::collections::HashMap<u16, FormFilingProgress>,
        >,
    ) -> bool {
        let Some(progress_by_year) = filing_progress.get(&deadline.form_code) else {
            return false;
        };

        match deadline.period {
            DeadlinePeriod::Monthly {
                taxable_year,
                month,
            } => progress_by_year
                .get(&(taxable_year as u16))
                .and_then(|progress| progress.months.get(usize::from(month - 1)))
                .is_some_and(Self::paid_state),
            DeadlinePeriod::Quarterly {
                taxable_year,
                quarter,
            } => progress_by_year
                .get(&(taxable_year as u16))
                .and_then(|progress| progress.quarters.get(usize::from(quarter - 1)))
                .is_some_and(Self::paid_state),
            DeadlinePeriod::Annual { taxable_year } => progress_by_year
                .get(&(taxable_year as u16))
                .is_some_and(|progress| Self::paid_state(&progress.annual_status)),
            DeadlinePeriod::EventBased => false,
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

        let mut available_forms =
            bir_core::integration::applicable_form_decisions_for_profile_and_year(profile, year);

        // ━━━ Monthly/Quarterly filing exclusion ━━━
        // If the user has already filed ANY quarterly return for a form category,
        // hide the monthly variant. Monthly forms are opt-in: shown by default,
        // but suppressed once the user commits to the quarterly cadence.
        //
        // Monthly→Quarterly pairs: (monthly_code, quarterly_code)
        let monthly_quarterly_pairs: &[(&str, &str)] = &[
            ("2550M", "2550Q"), // VAT: monthly vs quarterly
            ("2551M", "2551Q"), // Percentage Tax: monthly vs quarterly (pre-2018 only)
        ];

        let filing_progress = &self.filing_progress;
        available_forms.retain(|f| {
            // Check if this form is a monthly variant that has a quarterly counterpart
            if let Some((_monthly, quarterly)) = monthly_quarterly_pairs
                .iter()
                .find(|(m, _)| *m == f.form_code.as_str())
            {
                // If quarterly counterpart has any filed period, suppress the monthly
                if let Some(q_progress) = filing_progress.get(*quarterly).and_then(|y| y.get(&year))
                {
                    let quarterly_has_filing = q_progress.quarters.iter().any(|q| {
                        matches!(
                            q,
                            QuarterState::Draft
                                | QuarterState::Queued
                                | QuarterState::Submitted
                                | QuarterState::Confirmed
                                | QuarterState::Paid
                        )
                    });
                    if quarterly_has_filing {
                        return false; // Suppress monthly — user is filing quarterly
                    }
                }
            }
            true
        });
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
        let mut active_quarters: Vec<u8> = filter_chips
            .iter()
            .filter(|c| c.group == "Quarter")
            .filter_map(|c| c.label.strip_prefix('Q').and_then(|n| n.parse::<u8>().ok()))
            .collect();
        active_quarters.sort_unstable();
        active_quarters.dedup();
        let has_quarter_filter = !active_quarters.is_empty();

        let mut active_months: Vec<u8> = filter_chips
            .iter()
            .filter(|c| c.group == "Month")
            .filter_map(|c| Self::month_label_to_num(&c.label))
            .collect();
        active_months.sort_unstable();
        active_months.dedup();
        let has_month_filter = !active_months.is_empty();
        let has_annual_filter = filter_chips.iter().any(|c| c.group == "Annual");
        let has_period_filter = has_annual_filter || has_quarter_filter || has_month_filter;

        // Period filters are OR'd by filing family so Annual, Quarter, and Month
        // chips can be combined without hiding each other's form families.
        if has_period_filter {
            available_forms.retain(|f| match f.frequency {
                FilingFrequency::Annual => has_annual_filter,
                FilingFrequency::Quarterly => has_quarter_filter,
                FilingFrequency::Monthly => has_month_filter,
                FilingFrequency::OpenEnded => false,
            });
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
                        .any(|c| f.form_code.eq_ignore_ascii_case(&c.label))
                };

                let matches_query = if query.is_empty() {
                    true
                } else {
                    f.form_code.to_lowercase().contains(&query)
                        || f.title.to_lowercase().contains(&query)
                };

                matches_form_type && matches_query
            });
        }

        let vp_width: f32 = _window.viewport_size().width.into();
        let is_mini = self.is_mini_sidebar || vp_width < 768.0;
        let sidebar_width: f32 = if is_mini {
            SIDEBAR_MINI_WIDTH
        } else {
            SIDEBAR_FULL_WIDTH
        };

        let cols = tax_form_grid_columns(vp_width, sidebar_width);
        let card_width = tax_form_card_width(vp_width, sidebar_width, cols);

        let mut forms_ui = div().flex().flex_col().gap(px(GRID_GAP)).w_full();
        let mut cards_rendered = 0;

        for chunk in available_forms.chunks(cols) {
            let mut row = div().flex().flex_row().gap(px(GRID_GAP)).w_full();
            for form_def in chunk {
                let code = form_def.form_code.clone();
                let progress = self
                    .filing_progress
                    .get(&code)
                    .and_then(|y| y.get(&year))
                    .cloned();

                let support = form_support_level(&code);
                let is_release_fileable = support.is_fileable_in_app();
                let is_certification_draft = cfg!(any(debug_assertions, feature = "dev-tools"))
                    && can_open_certification_draft(&code);
                let is_fileable = is_release_fileable || is_certification_draft;
                let support_label = if is_certification_draft {
                    "Certification preview"
                } else {
                    support.action_label()
                };

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
                            let should_dim =
                                has_quarter_filter && !active_quarters.contains(&q_num);

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

                                let mut dot = div()
                                    .id(format!("q_{}_{}_{}", form_def.form_code, year, q_num))
                                    .flex()
                                    .flex_1()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .gap_1()
                                    .py_3()
                                    .rounded_xl()
                                    .border_1()
                                    .border_color(border_clr)
                                    .bg(bg)
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(
                                                if matches!(q_state, QuarterState::NotStarted) {
                                                    cx.theme().foreground
                                                } else {
                                                    accent
                                                },
                                            )
                                            .child(format!("Q{}", q_num)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(accent)
                                            .child(status_label),
                                    );

                                if is_fileable {
                                    dot = dot
                                        .cursor_pointer()
                                        .hover(move |s| s.bg(hover_bg).border_color(accent))
                                        .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                            cx.emit(DashboardEvent::FileForm {
                                                form_code: code_click.clone(),
                                                year,
                                                quarter: q_num,
                                            });
                                        }));
                                } else {
                                    dot = dot.cursor_default();
                                }

                                quarter_dots = quarter_dots.child(dot);
                            }
                        }

                        Self::build_card(form_def, year, card_width, cx).child(
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
                                                .child(if is_release_fileable {
                                                    format!("{}/4 filed", filed)
                                                } else {
                                                    support_label.to_string()
                                                }),
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
                                let should_dim =
                                    has_month_filter && !active_months.contains(&m_num);

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
                                                    .child("-"),
                                            ),
                                    );
                                } else {
                                    let (bg, border_clr, accent, _icon, status_label) =
                                        Self::state_style(m_state, cx);
                                    let hover_bg = Self::state_hover_bg(m_state, cx);
                                    let code_click = code.clone();

                                    let mut dot = div()
                                        .id(format!("m_{}_{}_{}", form_def.form_code, year, m_num))
                                        .flex()
                                        .flex_1()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .gap_1()
                                        .py_3()
                                        .rounded_xl()
                                        .border_1()
                                        .border_color(border_clr)
                                        .bg(bg)
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(
                                                    if matches!(m_state, QuarterState::NotStarted) {
                                                        cx.theme().foreground
                                                    } else {
                                                        accent
                                                    },
                                                )
                                                .child(month_names[absolute_idx]),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(accent)
                                                .child(status_label),
                                        );

                                    if is_fileable {
                                        dot = dot
                                            .cursor_pointer()
                                            .hover(move |s| s.bg(hover_bg).border_color(accent))
                                            .on_click(cx.listener(
                                                move |_this, _ev, _window, cx| {
                                                    cx.emit(DashboardEvent::FileForm {
                                                        form_code: code_click.clone(),
                                                        year,
                                                        quarter: m_num,
                                                    });
                                                },
                                            ));
                                    } else {
                                        dot = dot.cursor_default();
                                    }

                                    row = row.child(dot);
                                }
                            }
                            month_dots = month_dots.child(row);
                        }

                        Self::build_card(form_def, year, card_width, cx).child(
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
                                                .child(if is_release_fileable {
                                                    format!("{}/12 filed", filed)
                                                } else {
                                                    support_label.to_string()
                                                }),
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

                        Self::build_card(form_def, year, card_width, cx)
                            .child(
                                div().flex().justify_between().items_center().child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("Year {}:", year)),
                                ),
                            )
                            .child({
                                let mut dot = div()
                                    .id(format!("annual_{}_{}", form_def.form_code, year))
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
                                            .text_color(
                                                if matches!(&status, QuarterState::NotStarted) {
                                                    cx.theme().foreground
                                                } else {
                                                    accent
                                                },
                                            )
                                            .child(if is_release_fileable {
                                                match &status {
                                                    QuarterState::Paid => "View Paid Return",
                                                    QuarterState::Confirmed => {
                                                        "View Confirmed Return"
                                                    }
                                                    QuarterState::Submitted => "View Submission",
                                                    QuarterState::Queued => "View Queued Return",
                                                    QuarterState::Draft => "Resume Draft",
                                                    QuarterState::NotStarted => {
                                                        "File Annual Return"
                                                    }
                                                }
                                            } else if is_certification_draft {
                                                "Open certification draft"
                                            } else {
                                                support_label
                                            }),
                                    );

                                if is_fileable {
                                    dot = dot
                                        .cursor_pointer()
                                        .hover(move |s| s.bg(hover_bg).border_color(accent))
                                        .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                            cx.emit(DashboardEvent::FileForm {
                                                form_code: code_click.clone(),
                                                year,
                                                quarter: 0,
                                            });
                                        }));
                                } else {
                                    dot = dot.cursor_default();
                                }

                                dot
                            })
                    }
                    FilingFrequency::OpenEnded => {
                        let count = progress.as_ref().map(|p| p.open_ended_count).unwrap_or(0);
                        let next_open_ended_key =
                            count.saturating_add(1).min(u32::from(u8::MAX)) as u8;
                        let code_click = code.clone();
                        let hover_bg = cx.theme().accent;
                        let primary = cx.theme().primary;

                        Self::build_card(form_def, year, card_width, cx)
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
                            .child({
                                let mut dot = div()
                                    .id(format!("monthly_{}_{}", form_def.form_code, year))
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
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child(if is_release_fileable { "+" } else { "" }),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child(if is_release_fileable {
                                                "File New Return"
                                            } else if is_certification_draft {
                                                "Open certification draft"
                                            } else {
                                                support_label
                                            }),
                                    );

                                if is_fileable {
                                    dot = dot
                                        .cursor_pointer()
                                        .hover(move |s| s.bg(hover_bg).border_color(primary))
                                        .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                            cx.emit(DashboardEvent::FileForm {
                                                form_code: code_click.clone(),
                                                year,
                                                quarter: next_open_ended_key,
                                            });
                                        }));
                                } else {
                                    dot = dot.cursor_default();
                                }

                                dot
                            })
                    }
                };

                row = row.child(card);
                cards_rendered += 1;
            }

            // Add spacers to maintain identical column widths
            let empty_slots = cols - chunk.len();
            for _ in 0..empty_slots {
                row = row.child(div().w(px(card_width)).child(div().h(px(1.))));
            }

            forms_ui = forms_ui.child(row);
        }

        let forms_ui = if cards_rendered == 0 {
            // Determine WHY there are no forms: unconfigured profile vs. active filters
            let per_year_is_empty = profile
                .per_year_forms
                .get(&year)
                .is_none_or(|s| s.entries.is_empty());
            let no_active_filters =
                !has_period_filter && filter_chips.is_empty() && query.is_empty();

            if per_year_is_empty && no_active_filters {
                // Gap R1 nudge: profile has no form configuration for this year
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .py_24()
                    .gap_4()
                    .child(
                        Icon::new(IconName::File)
                            .size(px(48.))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(format!("No forms configured for {}", year)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "This profile has no active filing obligations set up for this year.",
                            ),
                    )
                    .child(
                        div()
                            .id("cta_open_profile_manager")
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(cx.theme().primary)
                            .text_color(cx.theme().primary_foreground)
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                cx.emit(DashboardEvent::OpenProfileManager);
                            }))
                            .child("Set up forms for this year"),
                    )
            } else {
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
            }
        } else {
            forms_ui
        };

        // Filter deadlines based on selected month/quarter
        // When no filter is active, default to current month
        let today = chrono::Local::now().date_naive();
        let current_month = today.month() as u8;

        // Explicit period filters use taxable-period metadata, not the adjusted
        // due date. The implicit no-filter state still uses the current due
        // month so the default dashboard remains focused on today's calendar.
        let all_matching_deadlines: Vec<_> = self
            .deadlines
            .iter()
            .filter(|d| {
                Self::deadline_matches_filters(
                    d,
                    &active_months,
                    &active_quarters,
                    has_annual_filter,
                    has_period_filter,
                    current_month,
                ) && !Self::deadline_is_satisfied(d, &self.filing_progress)
            })
            .cloned()
            .collect();

        // Split into upcoming (today or future) and overdue (past)
        let upcoming_deadlines: Vec<_> = all_matching_deadlines
            .iter()
            .filter(|d| d.final_deadline_date().is_some_and(|date| date >= today))
            .cloned()
            .collect();
        let mut overdue_deadlines: Vec<_> = all_matching_deadlines
            .iter()
            .filter(|d| d.final_deadline_date().is_some_and(|date| date < today))
            .cloned()
            .collect();

        let period_matching_actionable_forms: Vec<_> = self
            .actionable_forms
            .iter()
            .filter(|(_, summary)| {
                Self::summary_matches_period_filters(
                    summary,
                    &active_months,
                    &active_quarters,
                    has_annual_filter,
                    has_period_filter,
                )
            })
            .collect();
        let matching_actionable_forms: Vec<_> = period_matching_actionable_forms
            .into_iter()
            .filter(|(_, summary)| !Self::summary_is_overdue(&self.deadlines, summary, today))
            .collect();

        // A past-due draft remains overdue even when the default calendar view
        // is focused on the current due month. Include its exact deadline in
        // the Overdue column and remove it from Action Required.
        for (_, summary) in &self.actionable_forms {
            if !Self::summary_is_overdue(&self.deadlines, summary, today) {
                continue;
            }
            let Some(deadline) = Self::deadline_for_summary(&self.deadlines, summary) else {
                continue;
            };
            let already_present = overdue_deadlines.iter().any(|existing| {
                existing.form_code == deadline.form_code && existing.period == deadline.period
            });
            if !already_present {
                overdue_deadlines.push(deadline.clone());
            }
        }
        overdue_deadlines.sort_by_key(ResolvedTaxDeadline::final_deadline_date);

        self.upcoming_deadlines_list.update(cx, |list, _| {
            list.set_data(upcoming_deadlines, vec![]);
        });

        // Build dynamic header label based on filter
        let deadline_header = if has_period_filter {
            let mut parts = Vec::new();
            if has_annual_filter {
                parts.push("Annual".to_string());
            }
            parts.extend(active_quarters.iter().map(|q| format!("Q{}", q)));
            parts.extend(
                active_months
                    .iter()
                    .filter_map(|m| Self::month_num_to_full_label(*m)),
            );
            format!("{} Deadlines", parts.join(", "))
        } else {
            // Implicit current month
            Self::month_num_to_full_label(current_month)
                .map(|m| format!("{} Deadlines", m))
                .unwrap_or_else(|| "Deadlines".to_string())
        };

        let main_content = match self.active_tab {
            ProfileTab::Calendar => {
                // Build overdue column
                let mut overdue_inner = div().flex().flex_col().gap_3();

                if overdue_deadlines.is_empty() {
                    overdue_inner = overdue_inner.child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .py_8()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("No overdue deadlines. You're on track!"),
                            ),
                    );
                } else {
                    // Theme-aware overdue card colors
                    let is_dark = cx.theme().is_dark();
                    let card_bg = if is_dark {
                        gpui::hsla(0.0, 0.3, 0.14, 1.0)
                    } else {
                        gpui::hsla(0.0, 0.9, 0.97, 1.0)
                    };
                    let card_border = if is_dark {
                        gpui::hsla(0.0, 0.4, 0.25, 1.0)
                    } else {
                        gpui::hsla(0.0, 0.8, 0.85, 1.0)
                    };
                    let card_hover_bg = if is_dark {
                        gpui::hsla(0.0, 0.35, 0.18, 1.0)
                    } else {
                        gpui::hsla(0.0, 0.85, 0.94, 1.0)
                    };
                    let card_hover_border = if is_dark {
                        gpui::hsla(0.0, 0.5, 0.35, 1.0)
                    } else {
                        gpui::hsla(0.0, 0.8, 0.7, 1.0)
                    };
                    let badge_bg = if is_dark {
                        gpui::hsla(0.0, 0.35, 0.18, 1.0)
                    } else {
                        gpui::hsla(0.0, 0.8, 0.93, 1.0)
                    };
                    let badge_border = if is_dark {
                        gpui::hsla(0.0, 0.4, 0.28, 1.0)
                    } else {
                        gpui::hsla(0.0, 0.7, 0.85, 1.0)
                    };
                    let overdue_text = if is_dark {
                        gpui::hsla(0.0, 0.65, 0.65, 1.0)
                    } else {
                        gpui::hsla(0.0, 0.8, 0.4, 1.0)
                    };
                    let overdue_pill_bg = if is_dark {
                        gpui::hsla(0.0, 0.6, 0.4, 1.0)
                    } else {
                        gpui::hsla(0.0, 0.8, 0.5, 1.0)
                    };

                    for d in &overdue_deadlines {
                        let days_overdue = d
                            .final_deadline_date()
                            .map(|date| (today - date).num_days())
                            .unwrap_or(0);
                        let deadline_date = d.final_deadline_date();
                        let month_label = deadline_date
                            .map(|date| date.format("%b").to_string().to_uppercase())
                            .unwrap_or_else(|| "--".to_string());
                        let day_label = deadline_date
                            .map(|date| date.format("%d").to_string())
                            .unwrap_or_else(|| "--".to_string());

                        let route = d.route_year_quarter();
                        let form_code_click = d.form_code.clone();
                        let card_id =
                            format!("overdue-{}-{}", d.form_code, d.final_deadline_string());

                        let mut card = div()
                            .id(card_id)
                            .flex()
                            .items_center()
                            .gap_4()
                            .p_3()
                            .bg(card_bg)
                            .border_1()
                            .border_color(card_border)
                            .rounded_lg();

                        if let Some((year, quarter)) = route {
                            card = card
                                .cursor_pointer()
                                .hover(move |s| s.bg(card_hover_bg).border_color(card_hover_border))
                                .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                    cx.emit(DashboardEvent::FileForm {
                                        form_code: form_code_click.clone(),
                                        year,
                                        quarter,
                                    });
                                }));
                        }

                        overdue_inner = overdue_inner.child(
                            card.child(
                                // Date badge
                                div()
                                    .w(px(56.))
                                    .h(px(56.))
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .bg(badge_bg)
                                    .border_1()
                                    .border_color(badge_border)
                                    .rounded_md()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(overdue_text)
                                            .child(month_label),
                                    )
                                    .child(
                                        div()
                                            .text_xl()
                                            .font_weight(FontWeight::BLACK)
                                            .text_color(overdue_text)
                                            .child(day_label),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_base()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(overdue_text)
                                                    .child(d.display_form_no.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .px_2()
                                                    .py(px(2.))
                                                    .bg(overdue_pill_bg)
                                                    .text_color(gpui::white())
                                                    .rounded(px(4.))
                                                    .child(format!(
                                                        "{} day{} overdue",
                                                        days_overdue,
                                                        if days_overdue == 1 { "" } else { "s" }
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(d.form_name.clone()),
                                    ),
                            ),
                        );
                    }
                }

                let overdue_col = div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::hsla(0.0, 0.8, 0.5, 1.0))
                            .child("Overdue"),
                    )
                    .child(
                        div()
                            .w_full()
                            .p_4()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_xl()
                            .shadow_sm()
                            .child(overdue_inner),
                    );

                // Build action required column
                let mut action_inner = div().flex().flex_col().gap_3();

                if matching_actionable_forms.is_empty() {
                    action_inner = action_inner.child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .py_8()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("No pending forms. All caught up!"),
                            ),
                    );
                } else {
                    for (_p, f) in matching_actionable_forms {
                        let period_label = if let Some(q) = f.quarter {
                            format!("Q{}", q)
                        } else if let Some(m) = f.month {
                            Self::month_num_to_full_label(m).unwrap_or_else(|| format!("M{}", m))
                        } else {
                            "Annual".to_string()
                        };

                        let status_color = match f.status {
                            bir_core::forms::FilingStatus::Draft => gpui::hsla(0.13, 0.9, 0.5, 1.0),
                            bir_core::forms::FilingStatus::Queued => {
                                gpui::hsla(0.58, 0.7, 0.5, 1.0)
                            }
                            bir_core::forms::FilingStatus::Submitted => {
                                gpui::hsla(0.33, 0.7, 0.45, 1.0)
                            }
                            _ => cx.theme().muted_foreground,
                        };

                        // Action button label based on status
                        let action_label = match f.status {
                            bir_core::forms::FilingStatus::Draft => "Resume Draft",
                            bir_core::forms::FilingStatus::Queued => "Review Queued",
                            bir_core::forms::FilingStatus::Submitted => "View Submission",
                            bir_core::forms::FilingStatus::Confirmed => "View Confirmed",
                            bir_core::forms::FilingStatus::Paid => "View Paid",
                        };

                        // Look up the deadline date for this form
                        let deadline_date = self.deadlines.iter().find_map(|d| {
                            if d.form_code != f.form_code {
                                return None;
                            }
                            // Match by period
                            match &d.period {
                                bir_core::calendar_rules::DeadlinePeriod::Monthly {
                                    month, ..
                                } => {
                                    if f.month == Some(*month) {
                                        d.final_deadline_date()
                                    } else {
                                        None
                                    }
                                }
                                bir_core::calendar_rules::DeadlinePeriod::Quarterly {
                                    quarter,
                                    ..
                                } => {
                                    if f.quarter == Some(*quarter) {
                                        d.final_deadline_date()
                                    } else {
                                        None
                                    }
                                }
                                bir_core::calendar_rules::DeadlinePeriod::Annual { .. } => {
                                    d.final_deadline_date()
                                }
                                bir_core::calendar_rules::DeadlinePeriod::EventBased => None,
                            }
                        });

                        let month_label = deadline_date
                            .map(|date| date.format("%b").to_string().to_uppercase())
                            .unwrap_or_else(|| "--".to_string());
                        let day_label = deadline_date
                            .map(|date| date.format("%d").to_string())
                            .unwrap_or_else(|| "--".to_string());

                        // Route parameters for navigation
                        let form_code_click = f.form_code.clone();
                        let nav_year = f.taxable_year;
                        let nav_quarter = f.quarter.unwrap_or(f.month.unwrap_or(1));
                        let card_id = format!(
                            "action-{}-{}-{}",
                            f.form_code,
                            f.taxable_year,
                            f.quarter.or(f.month).unwrap_or(0)
                        );
                        let btn_form_code = f.form_code.clone();

                        action_inner = action_inner.child(
                            div()
                                .id(card_id)
                                .flex()
                                .items_center()
                                .gap_4()
                                .p_3()
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .rounded_lg()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(cx.theme().secondary)
                                        .border_color(cx.theme().primary.opacity(0.3))
                                })
                                .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                    cx.emit(DashboardEvent::FileForm {
                                        form_code: form_code_click.clone(),
                                        year: nav_year,
                                        quarter: nav_quarter,
                                    });
                                }))
                                .child(
                                    // Date badge
                                    div()
                                        .w(px(56.))
                                        .h(px(56.))
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .bg(cx.theme().secondary)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .rounded_md()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().muted_foreground)
                                                .child(month_label),
                                        )
                                        .child(
                                            div()
                                                .text_xl()
                                                .font_weight(FontWeight::BLACK)
                                                .text_color(cx.theme().foreground)
                                                .child(day_label),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .text_base()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(cx.theme().foreground)
                                                        .child(format!(
                                                            "{} - {} {}",
                                                            f.form_code,
                                                            f.taxable_year,
                                                            period_label
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .px_2()
                                                        .py(px(2.))
                                                        .bg(status_color)
                                                        .text_color(gpui::white())
                                                        .rounded(px(4.))
                                                        .child(format!("{:?}", f.status)),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!(
                                                    "Due: {}",
                                                    deadline_date
                                                        .map(|d| d.format("%b %d, %Y").to_string())
                                                        .unwrap_or_else(|| "N/A".to_string())
                                                )),
                                        ),
                                ),
                        );
                    }
                }

                let action_col = div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child("Action Required"),
                    )
                    .child(
                        div()
                            .w_full()
                            .p_4()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_xl()
                            .shadow_sm()
                            .child(action_inner),
                    );

                // Wrap the upcoming deadlines list in a column matching action/overdue
                let upcoming_col = div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(self.upcoming_deadlines_list.clone());

                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    // Full-width header row
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(deadline_header.clone()),
                    )
                    // Three equal columns below
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap_6()
                            .child(upcoming_col)
                            .child(action_col)
                            .child(overdue_col),
                    )
                    .into_any_element()
            }
            ProfileTab::Forms => {
                // Render consistency warnings (e.g., ITR conflicts) above the form grid
                let warnings: Vec<_> = self
                    .consistency_issues
                    .iter()
                    .filter(|i| {
                        matches!(
                            i.severity,
                            bir_core::integration::ProfileConsistencySeverity::Warning
                        )
                    })
                    .collect();

                if warnings.is_empty() {
                    forms_ui.into_any_element()
                } else {
                    let mut warnings_banner = div().flex().flex_col().gap_2().mb_4();
                    for issue in &warnings {
                        warnings_banner = warnings_banner.child(
                            div()
                                .flex()
                                .items_start()
                                .gap_2()
                                .p_3()
                                .rounded_lg()
                                .bg(gpui::rgba(0xfef3c715))
                                .border_1()
                                .border_color(gpui::rgba(0xf59e0b40))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(gpui::rgb(0x92400e))
                                        .child(format!("\u{26a0} {}", issue.message)),
                                ),
                        );
                    }
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(warnings_banner)
                        .child(forms_ui)
                        .into_any_element()
                }
            }
        };

        // Build subtitle with active period filters
        let period_desc = if has_period_filter {
            let mut parts = Vec::new();
            if has_annual_filter {
                parts.push("Annual".to_string());
            }
            parts.extend(active_quarters.iter().map(|q| format!("Q{}", q)));
            parts.extend(
                active_months
                    .iter()
                    .filter_map(|m| Self::month_num_to_label(*m)),
            );
            format!("Tax Year {} • {}", self.selected_year, parts.join(", "))
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
                    .px_8()
                    .py_6()
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
                                    .child(profile.full_name.clone()),
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
                                    .flex()
                                    .gap_4()
                                    .child(
                                        div()
                                            .id("tab_calendar")
                                            .p_2()
                                            .border_b_2()
                                            .border_color(
                                                if matches!(self.active_tab, ProfileTab::Calendar) {
                                                    cx.theme().primary
                                                } else {
                                                    gpui::transparent_black()
                                                },
                                            )
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.active_tab = ProfileTab::Calendar;
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(cx.theme().foreground)
                                                    .child("Calendar"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("tab_forms")
                                            .p_2()
                                            .border_b_2()
                                            .border_color(
                                                if matches!(self.active_tab, ProfileTab::Forms) {
                                                    cx.theme().primary
                                                } else {
                                                    gpui::transparent_black()
                                                },
                                            )
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.active_tab = ProfileTab::Forms;
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(cx.theme().foreground)
                                                    .child("Tax Form Library"),
                                            ),
                                    ),
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
        form_def: &bir_core::forms::FormCardData,
        _year: u16,
        card_width: f32,
        cx: &Context<Self>,
    ) -> Div {
        div()
            .w(px(card_width))
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_xl()
            .p_6()
            .overflow_hidden()
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
                            .child(form_def.form_code.clone()),
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
                            .child(form_def.category.clone()),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .line_height(relative(1.3))
                    .text_color(cx.theme().foreground)
                    .child(form_def.title.clone()),
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

    fn month_num_to_full_label(num: u8) -> Option<String> {
        match num {
            1 => Some("January".into()),
            2 => Some("February".into()),
            3 => Some("March".into()),
            4 => Some("April".into()),
            5 => Some("May".into()),
            6 => Some("June".into()),
            7 => Some("July".into()),
            8 => Some("August".into()),
            9 => Some("September".into()),
            10 => Some("October".into()),
            11 => Some("November".into()),
            12 => Some("December".into()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::calendar_rules::{DeadlineKind, DeadlineStatus};
    use bir_core::forms::{FilingStatus, FormDraftSummary};
    use chrono::NaiveDate;
    use std::collections::HashMap;

    fn quarterly_deadline(
        form_code: &str,
        taxable_year: i32,
        quarter: u8,
        final_deadline: NaiveDate,
    ) -> ResolvedTaxDeadline {
        ResolvedTaxDeadline {
            form_code: form_code.to_string(),
            display_form_no: form_code.to_string(),
            form_name: "Quarterly test deadline".to_string(),
            period: DeadlinePeriod::Quarterly {
                taxable_year,
                quarter,
            },
            period_start: None,
            period_end: None,
            deadline: DeadlineKind::Dated {
                original_deadline: final_deadline,
                final_deadline,
            },
            status: DeadlineStatus::Normal,
            description: "test".to_string(),
            source_reference: None,
        }
    }

    fn monthly_deadline(
        form_code: &str,
        taxable_year: i32,
        month: u8,
        final_deadline: NaiveDate,
    ) -> ResolvedTaxDeadline {
        ResolvedTaxDeadline {
            form_code: form_code.to_string(),
            display_form_no: form_code.to_string(),
            form_name: "Monthly test deadline".to_string(),
            period: DeadlinePeriod::Monthly {
                taxable_year,
                month,
            },
            period_start: None,
            period_end: None,
            deadline: DeadlineKind::Dated {
                original_deadline: final_deadline,
                final_deadline,
            },
            status: DeadlineStatus::Normal,
            description: "test".to_string(),
            source_reference: None,
        }
    }

    fn summary(form_code: &str, quarter: Option<u8>, month: Option<u8>) -> FormDraftSummary {
        FormDraftSummary {
            id: 1,
            tin: "123456789000".to_string(),
            form_code: form_code.to_string(),
            taxable_year: 2026,
            quarter,
            month,
            status: FilingStatus::Draft,
            updated_at: "2026-05-16T00:00:00Z".to_string(),
        }
    }

    #[::core::prelude::v1::test]
    fn calendar_filter_uses_taxable_period_when_filter_is_explicit() {
        let q1_due_in_may = quarterly_deadline(
            "1701Q",
            2026,
            1,
            NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        );

        assert!(DashboardView::deadline_matches_filters(
            &q1_due_in_may,
            &[],
            &[1],
            false,
            true,
            5,
        ));
        assert!(DashboardView::deadline_matches_filters(
            &q1_due_in_may,
            &[1],
            &[],
            false,
            true,
            5,
        ));
        assert!(DashboardView::deadline_matches_filters(
            &q1_due_in_may,
            &[3],
            &[],
            false,
            true,
            5,
        ));
        assert!(!DashboardView::deadline_matches_filters(
            &q1_due_in_may,
            &[4],
            &[],
            false,
            true,
            5,
        ));
        assert!(!DashboardView::deadline_matches_filters(
            &q1_due_in_may,
            &[5],
            &[],
            false,
            true,
            5,
        ));
    }

    #[::core::prelude::v1::test]
    fn default_calendar_filter_still_uses_current_deadline_month() {
        let q1_due_in_may = quarterly_deadline(
            "1701Q",
            2026,
            1,
            NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        );

        assert!(DashboardView::deadline_matches_filters(
            &q1_due_in_may,
            &[],
            &[],
            false,
            false,
            5,
        ));
        assert!(!DashboardView::deadline_matches_filters(
            &q1_due_in_may,
            &[],
            &[],
            false,
            false,
            4,
        ));
    }

    #[::core::prelude::v1::test]
    fn past_due_draft_is_classified_as_overdue_for_its_exact_period() {
        let january_deadline = monthly_deadline(
            "0619E",
            2026,
            1,
            NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
        );
        let january_draft = summary("0619E", None, Some(1));

        assert!(DashboardView::summary_is_overdue(
            &[january_deadline],
            &january_draft,
            NaiveDate::from_ymd_opt(2026, 6, 12).unwrap(),
        ));
    }

    #[::core::prelude::v1::test]
    fn draft_does_not_match_another_months_deadline() {
        let may_deadline = monthly_deadline(
            "0619E",
            2026,
            5,
            NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
        );
        let january_draft = summary("0619E", None, Some(1));

        assert!(!DashboardView::summary_is_overdue(
            &[may_deadline],
            &january_draft,
            NaiveDate::from_ymd_opt(2026, 6, 12).unwrap(),
        ));
    }

    #[::core::prelude::v1::test]
    fn only_paid_quarterly_deadline_is_satisfied() {
        let deadline = quarterly_deadline(
            "2551Q",
            2026,
            1,
            NaiveDate::from_ymd_opt(2026, 4, 27).unwrap(),
        );
        let mut progress = FormFilingProgress::new_empty("2551Q", 2026);
        progress.quarters[0] = QuarterState::Paid;

        let mut by_year = HashMap::new();
        by_year.insert(2026, progress);
        let mut progress_by_code = HashMap::new();
        progress_by_code.insert("2551Q".to_string(), by_year);

        assert!(DashboardView::deadline_is_satisfied(
            &deadline,
            &progress_by_code
        ));

        for state in [QuarterState::Submitted, QuarterState::Confirmed] {
            let mut progress = FormFilingProgress::new_empty("2551Q", 2026);
            progress.quarters[0] = state;

            let mut by_year = HashMap::new();
            by_year.insert(2026, progress);
            let mut progress_by_code = HashMap::new();
            progress_by_code.insert("2551Q".to_string(), by_year);

            assert!(!DashboardView::deadline_is_satisfied(
                &deadline,
                &progress_by_code
            ));
        }
    }

    #[::core::prelude::v1::test]
    fn actionable_summaries_use_taxable_period_filters() {
        let q1_summary = summary("1701Q", Some(1), None);
        let q2_summary = summary("1701Q", Some(2), None);

        assert!(DashboardView::summary_matches_period_filters(
            &q1_summary,
            &[],
            &[1],
            false,
            true,
        ));
        assert!(DashboardView::summary_matches_period_filters(
            &q1_summary,
            &[1],
            &[],
            false,
            true,
        ));
        assert!(!DashboardView::summary_matches_period_filters(
            &q1_summary,
            &[5],
            &[],
            false,
            true,
        ));
        assert!(!DashboardView::summary_matches_period_filters(
            &q2_summary,
            &[],
            &[1],
            false,
            true,
        ));
    }
}
