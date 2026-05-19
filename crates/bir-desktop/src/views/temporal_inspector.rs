//! Temporal Tax Engine — Debug Inspector View
//!
//! A developer-only view that shows the compiled snapshot contents
//! and allows testing rule evaluation against arbitrary profiles/years.
//!
//! Gated behind `#[cfg(debug_assertions)]`.

use bir_core::temporal::snapshot_loader::compiled_snapshot;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::tab::{Tab, TabBar};
use gpui_component::*;

/// Active tab in the debug inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorTab {
    Overview,
    Eras,
    Artifacts,
    Rules,
    Rates,
    Simulator,
}

impl InspectorTab {
    fn label(&self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Eras => "Eras",
            Self::Artifacts => "Artifacts",
            Self::Rules => "Rules",
            Self::Rates => "Rates",
            Self::Simulator => "Simulator",
        }
    }

    fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Overview,
            1 => Self::Eras,
            2 => Self::Artifacts,
            3 => Self::Rules,
            4 => Self::Rates,
            5 => Self::Simulator,
            _ => Self::Overview,
        }
    }
}

pub struct TemporalInspectorView {
    active_tab: InspectorTab,
    /// Year used for the simulator tab
    sim_year: u16,
    /// Simulated profile type index
    sim_profile_idx: usize,
    /// Cached simulator results
    sim_results: Vec<(String, String, String)>, // (form_code, title, eligibility)
}

pub enum TemporalInspectorEvent {}
impl EventEmitter<TemporalInspectorEvent> for TemporalInspectorView {}

impl TemporalInspectorView {
    pub fn new(_window: &mut Window, _cx: &mut Context<'_, Self>) -> Self {
        Self {
            active_tab: InspectorTab::Overview,
            sim_year: 2024,
            sim_profile_idx: 0,
            sim_results: Vec::new(),
        }
    }

    fn run_simulation(&mut self) {
        use bir_core::naming::Tin;
        use bir_core::profile::{TaxClassification, TaxpayerProfile, TaxpayerType};
        use bir_core::temporal::context::TemporalContext;
        use bir_core::temporal::engine::TemporalEngine;

        let profiles: Vec<(TaxpayerType, Option<TaxClassification>, &str, bool)> = vec![
            (
                TaxpayerType::Individual,
                Some(TaxClassification::SelfEmployed),
                "Individual (NonVAT Self-Employed)",
                false,
            ),
            (
                TaxpayerType::Individual,
                Some(TaxClassification::SelfEmployed),
                "Individual (VAT Self-Employed)",
                true,
            ),
            (
                TaxpayerType::Individual,
                Some(TaxClassification::PurelyCompensation),
                "Individual (Purely Compensation)",
                false,
            ),
            (
                TaxpayerType::Corporation,
                Some(TaxClassification::Corporation),
                "Corporation",
                true,
            ),
        ];

        let (tp_type, classification, _label, sim_is_vat) =
            &profiles[self.sim_profile_idx.min(profiles.len() - 1)];

        let profile = TaxpayerProfile {
            id: Some(999),
            full_name: "Simulator Profile".into(),
            tin: Tin {
                segment1: "000".into(),
                segment2: "000".into(),
                segment3: "000".into(),
                branch: "000".into(),
            },
            rdo_code: "000".into(),
            line_of_business: "Simulation".into(),
            registered_address: "Test".into(),
            zip_code: "0000".into(),
            phone: "0000000".into(),
            email: "sim@test.com".into(),
            default_form_type: "2551Q".into(),
            taxpayer_type: tp_type.clone(),
            is_vat_registered: *sim_is_vat,
            business_start_date: None,
            birth_date: None,
            tax_classification: classification.clone(),
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_versions: vec![],
            compliance_source_mode: Default::default(),
        };

        let engine = TemporalEngine::default();
        let context = TemporalContext::current_compliance(self.sim_year);
        let decisions = engine.evaluate_with_context(&profile, &context);

        self.sim_results = decisions
            .iter()
            .map(|d| {
                (
                    d.form_code.clone(),
                    d.title.clone(),
                    format!("{:?}", d.eligibility),
                )
            })
            .collect();
    }

    // ── Render helpers ──

    fn render_overview(&self, cx: &mut Context<Self>) -> Div {
        let snapshot = compiled_snapshot();

        let stat_pill = |label: &str, value: usize, cx: &Context<Self>| -> Div {
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .p_4()
                .bg(cx.theme().muted)
                .border_1()
                .border_color(cx.theme().border)
                .rounded_xl()
                .min_w(px(120.))
                .child(
                    div()
                        .text_3xl()
                        .font_weight(FontWeight::BLACK)
                        .text_color(cx.theme().primary)
                        .child(format!("{}", value)),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child(label.to_string()),
                )
        };

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.kv_row("Snapshot ID", &snapshot.snapshot_id, cx))
                    .child(self.kv_row("Content Hash", &snapshot.content_hash[..16], cx))
                    .child(self.kv_row(
                        "Schema Version",
                        &format!("{}", snapshot.schema_version),
                        cx,
                    ))
                    .child(self.kv_row("Generated", &snapshot.generated_at, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .child(stat_pill("Eras", snapshot.eras.len(), cx))
                    .child(stat_pill("Artifacts", snapshot.form_artifacts.len(), cx))
                    .child(stat_pill("Rules", snapshot.rules.len(), cx))
                    .child(stat_pill("Rate Tables", snapshot.rate_tables.len(), cx))
                    .child(stat_pill("Formulas", snapshot.formulas.len(), cx))
                    .child(stat_pill("Citations", snapshot.citations.len(), cx)),
            )
    }

    fn render_eras(&self, cx: &mut Context<Self>) -> Div {
        let snapshot = compiled_snapshot();
        let mut col = div().flex().flex_col().gap_3();

        for era in &snapshot.eras {
            let until = era
                .effective_until_year
                .map(|y| format!("{}", y))
                .unwrap_or_else(|| "present".into());
            col = col.child(
                div()
                    .p_4()
                    .bg(cx.theme().muted)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_xl()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child(era.title.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .px_2()
                                    .py_0p5()
                                    .bg(cx.theme().primary.opacity(0.15))
                                    .text_color(cx.theme().primary)
                                    .rounded_md()
                                    .font_weight(FontWeight::BOLD)
                                    .child(era.era_id.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} – {} • {} • {}",
                                era.effective_from_year,
                                until,
                                era.jurisdiction,
                                if era.is_overlay { "Overlay" } else { "Primary" }
                            )),
                    ),
            );
        }

        col
    }

    fn render_artifacts(&self, cx: &mut Context<Self>) -> Div {
        let snapshot = compiled_snapshot();
        let mut col = div().flex().flex_col().gap_2();

        // Group by category
        let mut categories: std::collections::BTreeMap<&str, Vec<_>> =
            std::collections::BTreeMap::new();
        for art in &snapshot.form_artifacts {
            categories.entry(&art.category).or_default().push(art);
        }

        for (cat, arts) in &categories {
            col = col.child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().primary)
                    .mt_2()
                    .child(format!("{} ({})", cat, arts.len())),
            );

            for art in arts {
                let lifecycle_color = match format!("{:?}", art.lifecycle).as_str() {
                    "Active" => gpui::rgba(0x22c55eff).into(),
                    "Abolished" => gpui::rgba(0xef4444ff).into(),
                    _ => cx.theme().muted_foreground,
                };

                col = col.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .py_1()
                        .px_3()
                        .rounded_lg()
                        .hover(|s| s.bg(cx.theme().muted))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BLACK)
                                .text_color(cx.theme().foreground)
                                .min_w(px(70.))
                                .child(art.form_code.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .flex_1()
                                .overflow_hidden()
                                .child(art.title.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(lifecycle_color)
                                .child(format!("{:?}", art.lifecycle)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{:?}", art.frequency)),
                        ),
                );
            }
        }

        col
    }

    fn render_rules(&self, cx: &mut Context<Self>) -> Div {
        let snapshot = compiled_snapshot();
        let mut col = div().flex().flex_col().gap_3();

        for rule in &snapshot.rules {
            let mutation_count = rule.mutations.len();
            col = col.child(
                div()
                    .p_4()
                    .bg(cx.theme().muted)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_xl()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child(rule.title.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .px_2()
                                    .py_0p5()
                                    .bg(cx.theme().secondary)
                                    .text_color(cx.theme().secondary_foreground)
                                    .rounded_md()
                                    .font_weight(FontWeight::BOLD)
                                    .child(rule.era_id.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "Phase: {:?} • Priority: {} • {} mutation(s)",
                                rule.phase, rule.priority, mutation_count
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Problem: {}", rule.problem)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().primary)
                            .child(format!("Solution: {}", rule.solution)),
                    ),
            );
        }

        col
    }

    fn render_rates(&self, cx: &mut Context<Self>) -> Div {
        let snapshot = compiled_snapshot();
        let mut col = div().flex().flex_col().gap_3();

        for table in &snapshot.rate_tables {
            let until = table.effective_until.as_deref().unwrap_or("present");
            let mut entries = div().flex().flex_col().gap_1();
            for entry in &table.rates {
                entries = entries.child(
                    div()
                        .flex()
                        .gap_3()
                        .text_xs()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().foreground)
                                .min_w(px(100.))
                                .child(entry.key.clone()),
                        )
                        .child(
                            div()
                                .text_color(cx.theme().primary)
                                .font_weight(FontWeight::BLACK)
                                .child(entry.rate.clone()),
                        ),
                );
            }

            col = col.child(
                div()
                    .p_4()
                    .bg(cx.theme().muted)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_xl()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(table.title.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} • {} – {}",
                                table.tax_type, table.effective_from, until
                            )),
                    )
                    .child(entries),
            );
        }

        col
    }

    fn render_simulator(&mut self, cx: &mut Context<Self>) -> Div {
        let profile_labels = [
            "Individual (NonVAT Sole Prop)",
            "Individual (VAT Sole Prop)",
            "Individual (Purely Compensation)",
            "Corporation",
        ];

        let year_buttons = [2015u16, 2017, 2018, 2020, 2023, 2024, 2025, 2026];

        // Year selector
        let mut year_row = div().flex().flex_wrap().gap_2();
        for yr in year_buttons {
            let is_active = self.sim_year == yr;
            year_row = year_row.child(
                div()
                    .id(format!("sim-yr-{}", yr))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .when(is_active, |s| {
                        s.bg(cx.theme().primary)
                            .text_color(cx.theme().primary_foreground)
                    })
                    .when(!is_active, |s| {
                        s.bg(cx.theme().muted)
                            .text_color(cx.theme().muted_foreground)
                            .hover(|s| s.bg(cx.theme().accent))
                    })
                    .child(format!("{}", yr))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sim_year = yr;
                        this.run_simulation();
                        cx.notify();
                    })),
            );
        }

        // Profile selector
        let mut profile_row = div().flex().flex_wrap().gap_2();
        for (idx, label) in profile_labels.iter().enumerate() {
            let is_active = self.sim_profile_idx == idx;
            profile_row = profile_row.child(
                div()
                    .id(format!("sim-prof-{}", idx))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .when(is_active, |s| {
                        s.bg(cx.theme().primary)
                            .text_color(cx.theme().primary_foreground)
                    })
                    .when(!is_active, |s| {
                        s.bg(cx.theme().muted)
                            .text_color(cx.theme().muted_foreground)
                            .hover(|s| s.bg(cx.theme().accent))
                    })
                    .child(label.to_string())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sim_profile_idx = idx;
                        this.run_simulation();
                        cx.notify();
                    })),
            );
        }

        // Results table
        let mut results = div().flex().flex_col();
        for (code, _title, elig) in &self.sim_results {
            let is_visible = !elig.contains("Suppressed")
                && !elig.contains("Deprecated")
                && !elig.contains("Archived");
            let status_color: Hsla = if elig.contains("Required") {
                gpui::rgba(0x22c55eff).into()
            } else if elig.contains("Applicable") {
                gpui::rgba(0x3b82f6ff).into()
            } else if elig.contains("Optional") {
                gpui::rgba(0xa855f7ff).into()
            } else if elig.contains("Recommended") {
                gpui::rgba(0xf59e0bff).into()
            } else {
                gpui::rgba(0xef4444ff).into()
            };

            results = results.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .py_1()
                    .px_3()
                    .rounded_lg()
                    .opacity(if is_visible { 1.0 } else { 0.4 })
                    .hover(|s| s.bg(cx.theme().muted))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BLACK)
                            .text_color(cx.theme().foreground)
                            .min_w(px(70.))
                            .child(code.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(status_color)
                            .flex_1()
                            .child(elig.clone()),
                    ),
            );
        }

        let visible_count = self
            .sim_results
            .iter()
            .filter(|(_, _, e)| {
                !e.contains("Suppressed") && !e.contains("Deprecated") && !e.contains("Archived")
            })
            .count();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("TAX YEAR"),
                    )
                    .child(year_row),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("PROFILE TYPE"),
                    )
                    .child(profile_row),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child(format!(
                        "Results: {} total, {} visible",
                        self.sim_results.len(),
                        visible_count
                    )),
            )
            .child(results)
    }

    fn kv_row(&self, key: &str, value: &str, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().muted_foreground)
                    .min_w(px(110.))
                    .child(key.to_string()),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(value.to_string()),
            )
    }
}

impl Render for TemporalInspectorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let tab_content = match self.active_tab {
            InspectorTab::Overview => self.render_overview(cx),
            InspectorTab::Eras => self.render_eras(cx),
            InspectorTab::Artifacts => self.render_artifacts(cx),
            InspectorTab::Rules => self.render_rules(cx),
            InspectorTab::Rates => self.render_rates(cx),
            InspectorTab::Simulator => self.render_simulator(cx),
        };

        div()
            .id("temporal-inspector")
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .px_8()
                    .py_6()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Temporal Tax Engine Inspector"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Debug view — compiled snapshot and rule simulator"),
                            ),
                    )
                    .child(
                        TabBar::new("inspector-tabs")
                            .underline()
                            .selected_index(self.active_tab as usize)
                            .on_click(cx.listener(|this, index, _, cx| {
                                this.active_tab = InspectorTab::from_index(*index);
                                if this.active_tab == InspectorTab::Simulator
                                    && this.sim_results.is_empty()
                                {
                                    this.run_simulation();
                                }
                                cx.notify();
                            }))
                            .child(Tab::new().label("Overview"))
                            .child(Tab::new().label("Eras"))
                            .child(Tab::new().label("Artifacts"))
                            .child(Tab::new().label("Rules"))
                            .child(Tab::new().label("Rates"))
                            .child(Tab::new().label("Simulator")),
                    )
                    .child(tab_content),
            )
            .into_any_element()
    }
}
