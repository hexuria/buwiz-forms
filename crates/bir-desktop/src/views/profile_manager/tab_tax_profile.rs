//! Tax Profile tab — basic identity, TIN, RDO, address, and tax classification fields.

use super::*;
use gpui_rsx::rsx;

impl ProfileManagerView {
    /// Render the "Tax Profile" tab (tab index 0).
    ///
    /// Contains: TIN input, duplicate TIN error, RDO/type row, classification/EOPT
    /// row, line-of-business, name, address, zip/phone row, email, and dates.
    /// Filing-form routing lives on the yearly Forms Set checklist, not on
    /// VAT/withholding toggles.
    pub(super) fn render_tax_profile_tab(
        &self,
        is_individual: bool,
        is_cooperative: bool,
        date_label: &'static str,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        if self.active_tab != 0 {
            return div().into_any_element();
        }

        let tax_class_val = self.tax_classification_select.read(cx).selected_value(cx);
        let is_purely_compensation = is_individual && tax_class_val == "Purely Compensation";

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .id(crate::agent::ids::PROFILE_TIN)
                    .child(self.tin_input.clone()),
            )
            .when(self.tin_duplicate_error.is_some(), |this| {
                let msg = self.tin_duplicate_error.clone().unwrap_or_default();
                this.child(rsx! {
                    <div
                        px_2
                        py_1
                        rounded_md
                        bg={gpui::rgba(0xef444415)}
                        border_1
                        border_color={gpui::Hsla::from(gpui::rgba(0xef444460))}
                        flex
                        items_center
                        gap_2
                    >
                        <div
                            text_sm
                            font_weight={FontWeight::BOLD}
                            text_color={gpui::Hsla::from(gpui::rgba(0xef4444ff))}
                        >
                            {"⚠"}
                        </div>
                        <div text_sm text_color={gpui::Hsla::from(gpui::rgba(0xef4444ff))}>
                            {msg}
                        </div>
                    </div>
                })
            })
            .child(
                // Row: RDO + Taxpayer Type (50/50)
                rsx! {
                    <div flex gap_4 w_full>
                        <div flex_1 min_w_0 id={crate::agent::ids::PROFILE_RDO}>
                            {Self::field_label("Revenue District Office (RDO)", cx)}
                            {Combobox::new(&self.rdo_select)}
                            {self.field_error("rdo_code", cx)}
                        </div>
                        <div flex_1 min_w_0>
                            {Self::field_label("Taxpayer Type", cx)}
                            {Combobox::new(&self.type_select)}
                        </div>
                    </div>
                },
            )
            .when(is_individual, |this| {
                this.child(
                    // Row: Tax Classification + EOPT Tier (50/50)
                    rsx! {
                        <div flex gap_4 w_full>
                            <div flex_1 min_w_0>
                                {Self::field_label("Tax Classification", cx)}
                                {Combobox::new(&self.tax_classification_select)}
                            </div>
                            <div flex_1 min_w_0>
                                {Self::field_label("EOPT Tier", cx)}
                                {Combobox::new(&self.eopt_tier_select)}
                            </div>
                        </div>
                    },
                )
            })
            .when(is_cooperative, |this| {
                this.child(
                    // Row: Cooperative Tax Treatment + EOPT Tier (50/50)
                    rsx! {
                        <div flex gap_4 w_full>
                            <div flex_1 min_w_0>
                                {Self::field_label("Cooperative Tax Treatment", cx)}
                                {Combobox::new(&self.cooperative_treatment_select)}
                            </div>
                            <div flex_1 min_w_0>
                                {Self::field_label("EOPT Tier", cx)}
                                {Combobox::new(&self.eopt_tier_select)}
                            </div>
                        </div>
                    },
                )
            })
            .child(
                // Line of Business (full width)
                rsx! {
                    <div base={v_flex()} w_full id={crate::agent::ids::PROFILE_LOB}>
                        {Self::field_label("Line of Business", cx)}
                        {Input::new(&self.line_of_business)}
                        {self.field_error("line_of_business", cx)}
                    </div>
                },
            )
            .child(
                // Taxpayer's Name (full width)
                rsx! {
                    <div base={v_flex()} w_full id={crate::agent::ids::PROFILE_NAME}>
                        {Self::field_label("Taxpayer's Name", cx)}
                        {Input::new(&self.name_input)}
                        {self.field_error("full_name", cx)}
                    </div>
                },
            )
            .child(
                // Registered Address (full width)
                rsx! {
                    <div base={v_flex()} w_full id={crate::agent::ids::PROFILE_ADDRESS}>
                        {Self::field_label("Registered Address", cx)}
                        {Input::new(&self.address_input)}
                        {self.field_error("registered_address", cx)}
                    </div>
                },
            )
            .child(
                // Row: Zip Code + Phone (50/50)
                rsx! {
                    <div flex gap_4 w_full>
                        <div flex_1 min_w_0 id={crate::agent::ids::PROFILE_ZIP}>
                            {Self::field_label("Zip Code", cx)}
                            {Combobox::new(&self.zip_select)}
                            {self.field_error("zip_code", cx)}
                        </div>
                        <div flex_1 min_w_0 id={crate::agent::ids::PROFILE_PHONE}>
                            {Self::field_label("Phone / Telephone No.", cx)}
                            {Input::new(&self.tel_input)}
                            {self.field_error("phone", cx)}
                        </div>
                    </div>
                },
            )
            .child(
                // Email Address (full width)
                rsx! {
                    <div base={v_flex()} w_full id={crate::agent::ids::PROFILE_EMAIL}>
                        {Self::field_label("Email Address", cx)}
                        {Input::new(&self.email_input)}
                        {self.field_error("email", cx)}
                    </div>
                },
            )
            .when(is_individual, |this| {
                this.child(rsx! {
                    <div base={v_flex()} w_full>
                        {Self::field_label("Birth Date", cx)}
                        {DateInput::new(&self.birth_date_input)}
                        {self.field_error("birth_date", cx)}
                    </div>
                })
            })
            .when(!is_purely_compensation, |this| {
                this.child(rsx! {
                    <div base={v_flex()} w_full>
                        {Self::field_label("Business Start Date", cx)}
                        {DateInput::new(&self.business_start_input)}
                        {self.field_error("business_start_date", cx)}
                    </div>
                })
            })
            .child(rsx! {
                <div flex flex_col gap_1 pt_4>
                    <div text_lg font_weight={FontWeight::BOLD}>
                        {"Forms Set"}
                    </div>
                    <div text_xs text_color={cx.theme().muted_foreground}>
                        {"Check the forms this taxpayer files for the selected year. Only checked Manual entries appear on the dashboard. Queue and live BIR submit stay limited to forms that already support them."}
                    </div>
                </div>
            })
            .child(self.render_active_forms_tab(cx))
            .child({
                let profile = self.current_profile(cx);
                let eligible =
                    profile.eligible_for_income_tax_election_in_year(self.forms_editor_year);
                self.render_tax_election_section(eligible, cx)
            })
            .into_any_element()
    }
    /// Render user-confirmed per-year income-tax elections.
    ///
    /// `add_allowed` reflects eligibility for the currently selected Forms
    /// Set year; existing elections are always listed and removable.
    fn render_tax_election_section(&self, add_allowed: bool, cx: &Context<Self>) -> Div {
        let elections = &self.stored_tax_elections;

        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child("Annual Income Tax Election (not extracted from COR)"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Choose a taxable year and election, then press Apply or Save Profile. Save Profile also applies this pending row before writing the profile."),
            )
            // ── Existing elections table ──
            .when(!elections.is_empty(), |this| {
                let mut table = div().flex().flex_col().gap_1().mt_2();
                for election in elections {
                    let label = match &election.election {
                        bir_core::profile::IncomeTaxElection::EightPercent => "8% Flat Rate",
                        bir_core::profile::IncomeTaxElection::GraduatedUnspecified => {
                            "Graduated (deduction method not yet selected)"
                        }
                        bir_core::profile::IncomeTaxElection::GraduatedOsd => "Graduated + OSD",
                        bir_core::profile::IncomeTaxElection::GraduatedItemized => {
                            "Graduated + Itemized"
                        }
                    };
                    let year = election.taxable_year;
                    table = table.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child(format!("{}", year)),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(label),
                                    ),
                            )
                            .child(
                                div()
                                    .id(format!("remove_election_{}", year))
                                    .text_xs()
                                    .text_color(cx.theme().danger)
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.stored_tax_elections
                                            .retain(|e| e.taxable_year != year);
                                        this.mark_profile_changed();
                                        cx.notify();
                                    }))
                                    .child("Remove"),
                            ),
                    );
                }
                this.child(table)
            })
            // ── Add new election row ──
            .when(!add_allowed, |this| {
                this.child(
                    div()
                        .mt_2()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            "The selected profile-year is not an Individual registered as Self-Employed or Mixed Income, so new elections cannot be added. Existing elections above remain in effect and can be removed.",
                        ),
                )
            })
            .when(add_allowed, |this| {
                this.child(
                    div()
                        .flex()
                        .items_end()
                        .gap_3()
                        .mt_2()
                        .child(
                            div()
                                .w(px(80.))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .mb_1()
                                        .child("Year"),
                                )
                                .child(Input::new(&self.tax_election_year_input)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .mb_1()
                                        .child("Election"),
                                )
                                .child(Combobox::new(&self.tax_election_select)),
                        )
                        .child(
                            gpui_component::button::Button::new("apply_election_btn")
                                .label("Apply")
                                .small()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if let Err(message) =
                                        this.apply_pending_tax_election(window, cx)
                                    {
                                        this.pending_notification = Some((
                                            gpui_component::notification::NotificationType::Error,
                                            message,
                                        ));
                                        cx.notify();
                                    }
                                })),
                        ),
                )
            })
    }

    /// Reusable checkbox rendering helper.
    fn render_checkbox(
        id: &'static str,
        label: &'static str,
        checked: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        rsx! {
            <div
                id={id}
                flex
                items_center
                gap_2
                cursor_pointer
                on_click={cx.listener(move |this, _, _, cx| {
                match id {
                    "wh_compensation_toggle" => {
                        this.withholds_compensation = !this.withholds_compensation
                    }
                    "wh_expanded_toggle" => this.withholds_expanded = !this.withholds_expanded,
                    "wh_final_toggle" => this.withholds_final = !this.withholds_final,
                    "wh_top_agent_toggle" => {
                        this.is_top_withholding_agent = !this.is_top_withholding_agent
                    }
                    "wh_govt_toggle" => {
                        this.is_government_withholding_entity =
                            !this.is_government_withholding_entity
                    }
                    "gpp_partner_toggle" => this.is_gpp_partner = !this.is_gpp_partner,
                    "single_employer_toggle" => {
                        this.has_single_employer = !this.has_single_employer
                    }
                    "dormant_toggle" => this.is_dormant = !this.is_dormant,
                    _ => {}
                }
                this.mark_profile_changed();
                cx.notify();
            })}
            >
                <div
                    w_4
                    h_4
                    rounded_sm
                    border_1
                    border_color={cx.theme().border}
                    bg={if checked {
                        cx.theme().primary
                    } else {
                        cx.theme().background
                    }}
                    flex
                    items_center
                    justify_center
                >
                    {if checked {
                        div()
                            .text_xs()
                            .text_color(cx.theme().primary_foreground)
                            .child("✓")
                    } else {
                        div()
                    }}
                </div>
                <div text_sm text_color={cx.theme().foreground}>{label}</div>
            </div>
        }
    }

    fn seed_default_forms(
        &self,
        year: u16,
        _cx: &Context<Self>,
    ) -> bir_core::forms::PerYearFormsSet {
        // V1: the user picks forms. Do not seed from COR / tax-type inference.
        bir_core::forms::PerYearFormsSet::new(year)
    }

    fn form_set_source_label(source: bir_core::forms::FormSetSource) -> &'static str {
        match source {
            bir_core::forms::FormSetSource::Manual => "Manual override",
            bir_core::forms::FormSetSource::CorAi => "Legacy COR review",
            bir_core::forms::FormSetSource::ReviewedCor => "Reviewed COR",
            bir_core::forms::FormSetSource::InferredTaxType => "Inferred tax type",
            bir_core::forms::FormSetSource::MigrationBackfill => "Migration review",
        }
    }

    fn form_support_label(form_code: &str) -> &'static str {
        bir_core::forms::form_support_level(form_code).action_label()
    }

    fn form_set_entry_provenance(entry: &bir_core::forms::FormSetEntry) -> (String, String) {
        let periods = entry
            .conflict
            .as_ref()
            .map(|conflict| {
                conflict
                    .competing_suggestions
                    .iter()
                    .map(|suggestion| (suggestion.effective_from, suggestion.effective_until))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![(entry.effective_from, entry.effective_until)]);
        let periods = periods
            .into_iter()
            .map(|(effective_from, effective_until)| {
                let start = effective_from
                    .map(|date| date.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "unresolved".to_string());
                let end = effective_until
                    .map(|date| date.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "open ended".to_string());
                format!("{start} to {end}")
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("; ");
        let references = entry
            .conflict
            .as_ref()
            .map(|conflict| {
                conflict
                    .competing_suggestions
                    .iter()
                    .filter_map(|suggestion| suggestion.source_reference.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| entry.source_reference.clone().into_iter().collect());
        let references = references
            .into_iter()
            .filter(|reference| !reference.trim().is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");

        (
            periods,
            if references.is_empty() {
                "No stored evidence reference".to_string()
            } else {
                references
            },
        )
    }

    /// A taxpayer files exactly one annual ITR per year. Refuse to activate a
    /// second member of the same annual-ITR group instead of warning after
    /// the conflict exists.
    fn block_conflicting_itr_activation(
        &mut self,
        set: &bir_core::forms::PerYearFormsSet,
        code: &str,
        year: u16,
        cx: &mut Context<Self>,
    ) -> bool {
        let conflicts = bir_core::integration::conflicting_active_annual_itrs(&set.entries, code);
        if conflicts.is_empty() {
            return false;
        }
        self.pending_notification = Some((
            gpui_component::notification::NotificationType::Error,
            format!(
                "{code} was not activated: {} is already active for {year}. A taxpayer files only one annual ITR per year — exclude the other form first.",
                conflicts.join(", ")
            ),
        ));
        cx.notify();
        true
    }

    fn toggle_form_obligation(&mut self, code: String, cx: &mut Context<Self>) {
        let year = self.forms_editor_year;
        let mut set = self
            .stored_per_year_forms
            .get(&year)
            .cloned()
            .unwrap_or_else(|| self.seed_default_forms(year, cx));

        let Some(entry) = set.entries.iter().find(|entry| entry.form_code == code) else {
            if self.block_conflicting_itr_activation(&set, &code, year, cx) {
                return;
            }
            Self::apply_manual_form_decision(&mut set, code, true, year);
            self.stored_per_year_forms.insert(year, set);
            self.mark_profile_changed();
            cx.notify();
            return;
        };
        if entry.needs_review() {
            return;
        }

        let next_active = !entry.is_filing_active();
        if next_active && self.block_conflicting_itr_activation(&set, &code, year, cx) {
            return;
        }
        Self::apply_manual_form_decision(&mut set, code, next_active, year);
        self.stored_per_year_forms.insert(year, set);
        self.mark_profile_changed();
        cx.notify();
    }

    fn decide_form_obligation(&mut self, code: String, active: bool, cx: &mut Context<Self>) {
        let year = self.forms_editor_year;
        let mut set = self
            .stored_per_year_forms
            .get(&year)
            .cloned()
            .unwrap_or_else(|| self.seed_default_forms(year, cx));

        Self::apply_manual_form_decision(&mut set, code, active, year);
        self.stored_per_year_forms.insert(year, set);
        self.mark_profile_changed();
        cx.notify();
    }

    fn apply_manual_form_decision(
        set: &mut bir_core::forms::PerYearFormsSet,
        code: String,
        active: bool,
        year: u16,
    ) {
        if let Some(entry) = set.entries.iter_mut().find(|e| e.form_code == code) {
            let prior_source = Self::form_set_source_label(entry.source);
            entry.apply_manual_decision(
                active,
                Some(format!(
                    "Manually {} for {} (previous source: {})",
                    if active { "included" } else { "excluded" },
                    year,
                    prior_source
                )),
            );
        } else {
            let mut new_entry = bir_core::forms::FormSetEntry::from_code(
                code,
                bir_core::forms::FormSetSource::Manual,
            );
            new_entry.active = active;
            new_entry.reason = Some(format!(
                "Manually {} for {year}",
                if active { "included" } else { "excluded" }
            ));
            set.entries.push(new_entry);
        }
    }

    fn add_custom_form(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let code = if self.forms_editor_custom_code_mode {
            let raw_code = self
                .forms_editor_new_code_input
                .read(cx)
                .value()
                .trim()
                .to_uppercase();
            if raw_code.is_empty() {
                return;
            }
            bir_core::forms::registry::canonical_form_code(&raw_code)
        } else {
            // Registry mode: the combobox text may be a raw typed filter, not
            // just a clicked option, so canonicalize it and require an exact
            // registry match — a typo or alias must either resolve to the
            // official code or be rejected, never stored verbatim.
            let selected = self
                .forms_editor_registry_form_select
                .read(cx)
                .selected_value(cx);
            let raw = selected
                .split(" - ")
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if raw.is_empty() {
                return;
            }
            let code = bir_core::forms::registry::canonical_form_code(&raw);
            if bir_core::forms::registry::find_form(&code).is_none() {
                self.pending_notification = Some((
                    gpui_component::notification::NotificationType::Error,
                    format!(
                        "'{raw}' does not match an official form in the registry. Pick a form from the list, or switch to Custom code."
                    ),
                ));
                cx.notify();
                return;
            }
            code
        };

        let reason = self
            .forms_editor_new_reason_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let reason_opt = if reason.is_empty() {
            None
        } else {
            Some(reason)
        };

        let freq_str = self
            .forms_editor_new_frequency_select
            .read(cx)
            .selected_value(cx);
        let frequency = match freq_str.as_str() {
            "Monthly" => bir_core::forms::FilingFrequency::Monthly,
            "Quarterly" => bir_core::forms::FilingFrequency::Quarterly,
            "Annual" => bir_core::forms::FilingFrequency::Annual,
            "Open Ended / Event" => bir_core::forms::FilingFrequency::OpenEnded,
            _ => bir_core::forms::registry::find_form(&code)
                .map(|definition| definition.frequency.clone())
                .unwrap_or(bir_core::forms::FilingFrequency::OpenEnded),
        };

        let year = self.forms_editor_year;
        let mut set = self
            .stored_per_year_forms
            .get(&year)
            .cloned()
            .unwrap_or_else(|| self.seed_default_forms(year, cx));

        if self.block_conflicting_itr_activation(&set, &code, year, cx) {
            return;
        }

        if let Some(entry) = set.entries.iter_mut().find(|entry| entry.form_code == code) {
            let next_reason = reason_opt.or_else(|| Some(format!("Manually included for {year}")));
            let changed = !entry.is_filing_active()
                || entry.source != bir_core::forms::FormSetSource::Manual
                || entry.reason != next_reason
                || entry.needs_review();
            entry.apply_manual_decision(true, next_reason);
            self.stored_per_year_forms.insert(year, set);
            if changed {
                self.mark_profile_changed();
            }
        } else {
            let custom = bir_core::forms::registry::find_form(&code).is_none();
            let mut entry = bir_core::forms::FormSetEntry::from_code(
                code.clone(),
                bir_core::forms::FormSetSource::Manual,
            );
            entry.frequency = frequency;
            entry.custom = custom;
            entry.reason = reason_opt.or_else(|| Some(format!("Manually included for {year}")));
            set.entries.push(entry);
            self.stored_per_year_forms.insert(year, set);
            self.mark_profile_changed();
        }

        self.forms_editor_new_code_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.forms_editor_registry_form_select
            .update(cx, |select, cx| select.set_selected_value("", window, cx));
        self.forms_editor_new_reason_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.forms_editor_new_frequency_select
            .update(cx, |select, cx| select.set_selected_value("", window, cx));
        cx.notify();
    }

    fn delete_custom_form(&mut self, code: String, cx: &mut Context<Self>) {
        let year = self.forms_editor_year;
        let mut removed = false;
        if let Some(set) = self.stored_per_year_forms.get_mut(&year) {
            removed = set.remove_manual_custom_entry(&code);
        }
        if self.forms_editor_selected_code.as_ref() == Some(&code) {
            self.forms_editor_selected_code = None;
        }
        if removed {
            self.mark_profile_changed();
        }
        cx.notify();
    }

    pub(super) fn render_active_forms_tab(&self, cx: &Context<Self>) -> gpui::AnyElement {
        if self.active_tab != 0 {
            return div().into_any_element();
        }

        let selected_year = self.forms_editor_year;
        let taxpayer_profile = self.current_profile(cx);
        let resolved_profile = taxpayer_profile.resolve_tax_profile_for_year(selected_year);
        let resolution_issues = resolved_profile
            .issues
            .iter()
            .map(|issue| issue.message.clone())
            .collect::<Vec<_>>();
        let forms_set = self
            .stored_per_year_forms
            .get(&selected_year)
            .cloned()
            .unwrap_or_else(|| self.seed_default_forms(selected_year, cx));

        // Gap R4 UI: compute annual ITR conflicts for this year's form set
        let itr_conflicts = bir_core::integration::check_annual_itr_conflicts(&forms_set.entries);

        div()
            .flex()
            .flex_col()
            .gap_4()
            .w_full()
            .h_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .gap_3()
                    .child(self.render_forms_editor_header(cx))
                    .child(rsx! {
                        <div
                            flex
                            flex_col
                            gap_1
                            px_3
                            py_2
                            rounded_md
                            border_1
                            border_color={cx.theme().border}
                            bg={cx.theme().secondary}
                        >
                            <div text_xs font_weight={FontWeight::SEMIBOLD}>
                                {"One filing authority"}
                            </div>
                            <div text_xs text_color={cx.theme().muted_foreground}>
                                {"Check the forms this taxpayer files for the selected year. Only checked Manual entries appear on the dashboard. Queue and live BIR submit stay limited to forms that already support them."}
                            </div>
                        </div>
                    })
                    .child(self.render_inventory_forms_checklist(&forms_set, cx))
                    .when(!resolution_issues.is_empty(), |this| {
                        this.child(rsx! {
                            <div
                                flex
                                flex_col
                                gap_1
                                px_3
                                py_2
                                rounded_md
                                bg={cx.theme().danger.opacity(0.08)}
                                border_1
                                border_color={cx.theme().danger.opacity(0.5)}
                            >
                                <div
                                    text_xs
                                    font_weight={FontWeight::BOLD}
                                    text_color={cx.theme().danger}
                                >
                                    {"Needs review before this year's profile can be used"}
                                </div>
                                {...resolution_issues.into_iter().map(|message| {
                                    rsx! {
                                        <div text_xs text_color={cx.theme().danger}>
                                            {message}
                                        </div>
                                    }
                                })}
                            </div>
                        })
                    })
                    // Conflict warning banner (only rendered when there are conflicts)
                    .when(!itr_conflicts.is_empty(), |this| {
                        let conflict_msg = itr_conflicts
                            .iter()
                            .map(|i| i.message.clone())
                            .collect::<Vec<_>>()
                            .join(" ");
                        this.child(rsx! {
                            <div
                                flex
                                items_start
                                gap_2
                                px_3
                                py_2
                                rounded_md
                                bg={cx.theme().warning.opacity(0.12)}
                                border_1
                                border_color={cx.theme().warning.opacity(0.45)}
                            >
                                <div
                                    text_sm
                                    font_weight={FontWeight::BOLD}
                                    text_color={cx.theme().warning}
                                >
                                    {"⚠"}
                                </div>
                                <div
                                    text_xs
                                    text_color={crate::theme::warning_on_tint(cx.theme())}
                                >
                                    {conflict_msg}
                                </div>
                            </div>
                        })
                    })
                    .child(self.render_custom_form_creator(cx))
                    .child(self.render_obligations_table(&forms_set, cx)),
            )
            .when(self.forms_editor_selected_code.is_some(), |this| {
                this.child(rsx! {
                    <div w_full border_t_1 border_color={cx.theme().border} pt_4>
                        {self.render_obligation_details(&forms_set, cx)}
                    </div>
                })
            })
            .into_any_element()
    }

    fn render_inventory_forms_checklist(
        &self,
        forms_set: &bir_core::forms::PerYearFormsSet,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let codes: Vec<&'static str> = bir_core::forms::inventory_codes().collect();
        let checklist = rsx! {
            <div
                id={crate::agent::ids::FORMS_YEAR_PICKER}
                flex
                flex_col
                gap_2
                p_3
                rounded_lg
                border_1
                border_color={cx.theme().border}
            >
                <div text_xs font_weight={FontWeight::SEMIBOLD}>
                    {"Forms for this year"}
                </div>
                <div text_xs text_color={cx.theme().muted_foreground}>
                    {format!(
                        "All {} inventory pages. Checking a code adds a Manual include for {}.",
                        codes.len(),
                        self.forms_editor_year
                    )}
                </div>
                <div flex flex_wrap gap_x={px(16.)} gap_y={px(8.)}>
                    {...codes.into_iter().map(|code| {
                        let active = forms_set.contains_active(code);
                        let pick_id = crate::agent::ids::form_pick_id(code);
                        rsx! {
                            <div
                                id={pick_id}
                                flex
                                items_center
                                gap_2
                                cursor_pointer
                                on_click={cx.listener({
                                    let code = code.to_string();
                                    move |this, _, _, cx| {
                                        this.toggle_form_obligation(code.clone(), cx);
                                    }
                                })}
                            >
                                <div
                                    w_4
                                    h_4
                                    rounded_sm
                                    border_1
                                    border_color={cx.theme().border}
                                    bg={if active {
                                        cx.theme().primary
                                    } else {
                                        cx.theme().background
                                    }}
                                    flex
                                    items_center
                                    justify_center
                                >
                                    {if active {
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().primary_foreground)
                                            .child("✓")
                                    } else {
                                        div()
                                    }}
                                </div>
                                <div text_xs text_color={cx.theme().foreground}>
                                    {code.to_string()}
                                </div>
                            </div>
                        }
                    })}
                </div>
            </div>
        };
        checklist.into_any()
    }

    fn render_forms_editor_header(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let selected_year = self.forms_editor_year;
        let prior_year = self
            .current_profile(cx)
            .closest_prior_forms_year(selected_year);

        let root = rsx! {
            <div flex items_center justify_between w_full>
                <div flex flex_col gap_1>
                    <div font_weight={FontWeight::BOLD} text_lg>
                        {format!("{} Forms Set", selected_year)}
                    </div>
                    <div text_xs text_color={cx.theme().muted_foreground}>
                        {"Check the forms to file this year. This saved set—not inferred tax types—is the dashboard list."}
                    </div>
                </div>
                <div
                    flex
                    items_center
                    gap_2
                    whenSome={(prior_year, |this, prior_year| {
                        this.child(
                            gpui_component::button::Button::new("copy_prior_year_btn")
                                .label(format!("Copy from {}", prior_year))
                                .small()
                                .ghost()
                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                    // Collect active entries from prior year
                                    let source_entries: Vec<bir_core::forms::FormSetEntry> = {
                                        let from_local = this
                                            .stored_per_year_forms
                                            .get(&prior_year)
                                            .map(|s| s.entries.clone());
                                        let from_profile = this
                                            .current_profile(cx)
                                            .per_year_forms
                                            .get(&prior_year)
                                            .map(|s| s.entries.clone());
                                        from_local.or(from_profile).unwrap_or_default()
                                    };

                                    // Only copy active entries; re-tag them as Manual
                                    let to_copy: Vec<bir_core::forms::FormSetEntry> =
                                        source_entries
                                            .into_iter()
                                            .filter(|e| e.is_filing_active())
                                            .map(|mut e| {
                                                e.apply_manual_decision(
                                                    true,
                                                    Some(format!("Copied from {}", prior_year)),
                                                );
                                                e
                                            })
                                            .collect();

                                    if to_copy.is_empty() {
                                        return;
                                    }

                                    // Merge into current year's set (skip duplicates)
                                    let dest = this
                                        .stored_per_year_forms
                                        .entry(selected_year)
                                        .or_insert_with(|| {
                                            bir_core::forms::PerYearFormsSet::new(selected_year)
                                        });
                                    let mut added = false;
                                    let mut skipped_itrs: Vec<String> = Vec::new();
                                    for entry in to_copy {
                                        let already_present = dest
                                            .entries
                                            .iter()
                                            .any(|e| e.form_code == entry.form_code);
                                        if already_present {
                                            continue;
                                        }
                                        // Copying must not create an annual
                                        // ITR conflict in the target year.
                                        if !bir_core::integration::conflicting_active_annual_itrs(
                                            &dest.entries,
                                            &entry.form_code,
                                        )
                                        .is_empty()
                                        {
                                            skipped_itrs.push(entry.form_code.clone());
                                            continue;
                                        }
                                        dest.entries.push(entry);
                                        added = true;
                                    }
                                    if added {
                                        this.mark_profile_changed();
                                    }
                                    if !skipped_itrs.is_empty() {
                                        this.pending_notification = Some((
                                            gpui_component::notification::NotificationType::Warning,
                                            format!(
                                                "Skipped {}: only one annual ITR can be active per year.",
                                                skipped_itrs.join(", ")
                                            ),
                                        ));
                                    }
                                    cx.notify();
                                })),
                        )
                    })}
                >
                    <div text_sm text_color={cx.theme().muted_foreground}>{"Year:"}</div>
                    <div w={px(100.)}>{Combobox::new(&self.forms_editor_year_select)}</div>
                </div>
            </div>
        };
        root.into_any()
    }

    fn render_custom_form_creator(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let custom_mode = self.forms_editor_custom_code_mode;
        let root = rsx! {
            <div
                flex
                flex_col
                gap_2
                p_3
                border_1
                border_color={cx.theme().border}
                rounded_lg
                bg={cx.theme().muted}
            >
                <div flex items_center justify_between gap_2>
                    <div
                        text_xs
                        font_weight={FontWeight::SEMIBOLD}
                        text_color={cx.theme().foreground}
                    >
                        {if custom_mode {
                            "Manually include a custom form code"
                        } else {
                            "Manually include an official form"
                        }}
                    </div>
                    {gpui_component::button::Button::new("toggle_custom_form_code_mode")
                        .label(if custom_mode {
                            "Choose from official forms"
                        } else {
                            "Custom code…"
                        })
                        .small()
                        .ghost()
                        .on_click(cx.listener(|this, _ev, _window, cx| {
                            this.forms_editor_custom_code_mode =
                                !this.forms_editor_custom_code_mode;
                            cx.notify();
                        }))}
                </div>
                <div text_xs text_color={cx.theme().muted_foreground}>
                    {if custom_mode {
                        "Custom codes are not validated against the official registry and become real filing obligations. Use this only for an obligation the registry does not list."
                    } else {
                        "Type to filter the official form registry. Only a selected registry form can be added here; use Custom code for anything else."
                    }}
                </div>
                <div
                    flex
                    items_center
                    gap_2
                    when={(!custom_mode, |this| {
                        this.child(rsx! {
                            <div flexGrow>
                                {Combobox::new(&self.forms_editor_registry_form_select)}
                            </div>
                        })
                    })}
                    when={(custom_mode, |this| {
                        this.child(rsx! {
                            <div flexGrow>
                                {gpui_component::input::Input::new(
                                    &self.forms_editor_new_code_input,
                                )}
                            </div>
                        })
                    })}
                >
                    <div w={px(180.)}>
                        {Combobox::new(&self.forms_editor_new_frequency_select)}
                    </div>
                    <div flexGrow>
                        {gpui_component::input::Input::new(
                            &self.forms_editor_new_reason_input,
                        )}
                    </div>
                    {gpui_component::button::Button::new("add_custom_form_btn")
                        .label("Add")
                        .on_click(cx.listener(|this, _ev, window, cx| {
                            this.add_custom_form(window, cx);
                        }))}
                </div>
            </div>
        };
        root.into_any()
    }

    fn render_obligations_table(
        &self,
        forms_set: &bir_core::forms::PerYearFormsSet,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let header_style = |cx: &Context<Self>| {
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(cx.theme().muted_foreground)
                .py_2()
                .px_3()
                .bg(cx.theme().muted)
        };

        let mut rows = vec![];

        for entry in &forms_set.entries {
            let code = entry.form_code.clone();
            let reason = entry.reason.clone().unwrap_or_default();
            let is_user_created_custom = entry.is_user_created_custom();
            let description = bir_core::forms::registry::find_form(&code)
                .map(|f| f.title)
                .unwrap_or("Custom Obligation");

            let frequency_str = match entry.frequency {
                bir_core::forms::FilingFrequency::Monthly => "Monthly",
                bir_core::forms::FilingFrequency::Quarterly => "Quarterly",
                bir_core::forms::FilingFrequency::Annual => "Annual",
                bir_core::forms::FilingFrequency::OpenEnded => "Open Ended / Event",
            };

            let source_str = if entry.needs_review() {
                "Needs review"
            } else {
                Self::form_set_source_label(entry.source)
            };
            let support_str = Self::form_support_label(&code);

            let is_selected = self.forms_editor_selected_code.as_ref() == Some(&code);
            let active = entry.is_filing_active();

            let checkbox_elem = rsx! {
                <div
                    id={format!("checkbox_{}", code)}
                    w_4
                    h_4
                    rounded_sm
                    border_1
                    border_color={cx.theme().border}
                    bg={if active {
                        cx.theme().primary
                    } else {
                        cx.theme().background
                    }}
                    flex
                    items_center
                    justify_center
                    when={(!entry.needs_review(), |this| {
                        this.cursor_pointer().on_click(cx.listener({
                            let code = code.clone();
                            move |this, _, _, cx| {
                                this.toggle_form_obligation(code.clone(), cx);
                            }
                        }))
                    })}
                >
                    {if entry.needs_review() {
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::rgb(0xd97706))
                            .child("!")
                    } else if active {
                        div()
                            .text_xs()
                            .text_color(cx.theme().primary_foreground)
                            .child("✓")
                    } else {
                        div()
                    }}
                </div>
            };

            let row_elem = rsx! {
                <div
                    id={format!("row_{}", code)}
                    flex
                    items_center
                    w_full
                    py_2
                    border_b_1
                    border_color={cx.theme().border}
                    cursor_pointer
                    when={(is_selected, |this| this.bg(cx.theme().muted))}
                    on_click={cx.listener({
                        let code = code.clone();
                        let reason = reason.clone();
                        move |this, _, window, cx| {
                            this.forms_editor_selected_code = Some(code.clone());
                            this.forms_editor_active_note_input
                                .update(cx, |input, cx| input.set_value(&reason, window, cx));
                            cx.notify();
                        }
                    })}
                >
                    <div w={px(50.)} flex justify_center>{checkbox_elem}</div>
                    <div
                        w={px(100.)}
                        px_3
                        text_sm
                        font_weight={FontWeight::SEMIBOLD}
                        text_color={cx.theme().foreground}
                    >
                        {code.clone()}
                    </div>
                    <div flexGrow px_3 text_sm text_color={cx.theme().foreground}>
                        {description}
                    </div>
                    <div w={px(120.)} px_3 text_sm text_color={cx.theme().muted_foreground}>
                        {frequency_str}
                    </div>
                    <div w={px(110.)} px_3 text_sm text_color={cx.theme().muted_foreground}>
                        {source_str}
                    </div>
                    <div w={px(110.)} px_3 text_xs text_color={cx.theme().muted_foreground}>
                        {support_str}
                    </div>
                    <div w={px(80.)} px_3>
                        {if entry.custom {
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_full()
                                .bg(gpui::rgba(0xf59e0b15))
                                .border_1()
                                .border_color(gpui::rgba(0xf59e0b33))
                                .flex()
                                .justify_center()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(gpui::rgb(0xd97706))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(if is_user_created_custom {
                                            "Custom"
                                        } else {
                                            "Uncatalogued"
                                        }),
                                )
                        } else {
                            div()
                        }}
                    </div>
                    <div w={px(80.)} px_3 flex justify_center>
                        {if is_user_created_custom {
                            div()
                                .id(format!("delete_{}", code))
                                .cursor_pointer()
                                .on_click(cx.listener({
                                    let code = code.clone();
                                    move |this, _, _, cx| {
                                        this.delete_custom_form(code.clone(), cx);
                                    }
                                }))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(gpui::rgb(0xef4444))
                                        .child("Delete"),
                                )
                                .into_any()
                        } else {
                            div().into_any()
                        }}
                    </div>
                </div>
            };

            rows.push(row_elem);
        }

        let table_body = if rows.is_empty() {
            rsx! {
                <div flex justify_center py_8 text_sm text_color={cx.theme().muted_foreground}>
                    {"No filing obligations configured for this year."}
                </div>
            }
        } else {
            rsx! { <div flex flex_col>{...rows}</div> }
        };

        let root = rsx! {
            <div flex flex_col border_1 border_color={cx.theme().border} rounded_lg overflow_hidden>
                <div flex w_full>
                    <div base={header_style(cx)} w={px(50.)} flex justify_center>
                        {div()}
                    </div>
                    <div base={header_style(cx)} w={px(100.)}>{"Code"}</div>
                    <div base={header_style(cx)} flexGrow>{"Description"}</div>
                    <div base={header_style(cx)} w={px(120.)}>{"Frequency"}</div>
                    <div base={header_style(cx)} w={px(110.)}>{"Source"}</div>
                    <div base={header_style(cx)} w={px(110.)}>{"Support"}</div>
                    <div base={header_style(cx)} w={px(80.)}>{"Custom"}</div>
                    <div base={header_style(cx)} w={px(80.)} flex justify_center>
                        {"Actions"}
                    </div>
                </div>
                {table_body}
            </div>
        };
        root.into_any()
    }

    fn render_obligation_details(
        &self,
        forms_set: &bir_core::forms::PerYearFormsSet,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let Some(selected_code) = &self.forms_editor_selected_code else {
            let root = rsx! {
                <div flex flex_col items_center justify_center h_full py_12 gap_2>
                    <div text_sm text_color={cx.theme().muted_foreground}>
                        {"Select an obligation to view details."}
                    </div>
                </div>
            };
            return root.into_any();
        };

        let Some(entry) = forms_set.entry(selected_code) else {
            let root = rsx! {
                <div text_sm text_color={cx.theme().muted_foreground}>
                    {"Selected form obligation not found."}
                </div>
            };
            return root.into_any();
        };

        let title = bir_core::forms::registry::find_form(selected_code)
            .map(|f| f.title)
            .unwrap_or("Custom Obligation");

        let category = bir_core::forms::registry::find_form(selected_code)
            .map(|f| f.category.to_string())
            .unwrap_or_else(|| "Custom".to_string());

        let frequency_str = match entry.frequency {
            bir_core::forms::FilingFrequency::Monthly => "Monthly",
            bir_core::forms::FilingFrequency::Quarterly => "Quarterly",
            bir_core::forms::FilingFrequency::Annual => "Annual",
            bir_core::forms::FilingFrequency::OpenEnded => "Open Ended / Event",
        };
        let source_label = Self::form_set_source_label(entry.source);
        let support_label = Self::form_support_label(selected_code);
        let (effective_period, evidence_reference) = Self::form_set_entry_provenance(entry);
        let review_message = entry
            .needs_review()
            .then(|| {
                entry
                    .conflict
                    .as_ref()
                    .map(|conflict| conflict.message.clone())
            })
            .flatten();
        let selected_code_for_toggle = selected_code.clone();
        let selected_code_for_include = selected_code.clone();
        let selected_code_for_exclude = selected_code.clone();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(rsx! {
                <div flex flex_col gap_1>
                    <div
                        text_xs
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().muted_foreground}
                    >
                        {category.to_uppercase()}
                    </div>
                    <div
                        text_lg
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().foreground}
                    >
                        {format!("Form {}", selected_code)}
                    </div>
                    <div text_sm text_color={cx.theme().foreground}>{title}</div>
                </div>
            })
            .child(if entry.needs_review() {
                div()
                    .flex()
                    .gap_2()
                    .child(
                        gpui_component::button::Button::new("include_reviewed_form")
                            .label("Include for this year")
                            .small()
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.decide_form_obligation(
                                    selected_code_for_include.clone(),
                                    true,
                                    cx,
                                );
                            })),
                    )
                    .child(
                        gpui_component::button::Button::new("exclude_reviewed_form")
                            .label("Exclude for this year")
                            .small()
                            .ghost()
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.decide_form_obligation(
                                    selected_code_for_exclude.clone(),
                                    false,
                                    cx,
                                );
                            })),
                    )
                    .into_any()
            } else {
                div()
                    .child(
                        gpui_component::button::Button::new("toggle_selected_form")
                            .label(if entry.is_filing_active() {
                                "Exclude for this year"
                            } else {
                                "Include for this year"
                            })
                            .small()
                            .ghost()
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.toggle_form_obligation(
                                    selected_code_for_toggle.clone(),
                                    cx,
                                );
                            })),
                    )
                    .into_any()
            })
            .when_some(review_message, |this, message| {
                this.child(rsx! {
                    <div
                        flex
                        flex_col
                        gap_1
                        px_3
                        py_2
                        rounded_md
                        border_1
                        border_color={cx.theme().warning.opacity(0.45)}
                        bg={cx.theme().warning.opacity(0.12)}
                    >
                        <div
                            text_xs
                            font_weight={FontWeight::BOLD}
                            text_color={crate::theme::warning_on_tint(cx.theme())}
                        >
                            {"Needs review before filing"}
                        </div>
                        <div
                            text_xs
                            text_color={crate::theme::warning_on_tint(cx.theme())}
                        >
                            {message}
                        </div>
                    </div>
                })
            })
            .child(rsx! {
                <div flex flex_col gap_2>
                    <div flex justify_between>
                        <div text_xs text_color={cx.theme().muted_foreground}>
                            {"Frequency"}
                        </div>
                        <div
                            text_xs
                            font_weight={FontWeight::SEMIBOLD}
                            text_color={cx.theme().foreground}
                        >
                            {frequency_str}
                        </div>
                    </div>
                    <div flex justify_between>
                        <div text_xs text_color={cx.theme().muted_foreground}>
                            {"Source"}
                        </div>
                        <div
                            text_xs
                            font_weight={FontWeight::SEMIBOLD}
                            text_color={cx.theme().foreground}
                        >
                            {source_label}
                        </div>
                    </div>
                    <div flex justify_between>
                        <div text_xs text_color={cx.theme().muted_foreground}>
                            {"Decision"}
                        </div>
                        <div
                            text_xs
                            font_weight={FontWeight::SEMIBOLD}
                            text_color={cx.theme().foreground}
                        >
                            {if entry.needs_review() {
                                "Needs review"
                            } else if entry.active {
                                "Included"
                            } else {
                                "Excluded"
                            }}
                        </div>
                    </div>
                    <div flex justify_between>
                        <div text_xs text_color={cx.theme().muted_foreground}>
                            {"App support"}
                        </div>
                        <div
                            text_xs
                            font_weight={FontWeight::SEMIBOLD}
                            text_color={cx.theme().foreground}
                        >
                            {support_label}
                        </div>
                    </div>
                    <div flex flex_col gap_1>
                        <div text_xs text_color={cx.theme().muted_foreground}>
                            {"Effective profile evidence"}
                        </div>
                        <div
                            text_xs
                            font_weight={FontWeight::SEMIBOLD}
                            text_color={cx.theme().foreground}
                        >
                            {effective_period}
                        </div>
                    </div>
                    <div flex flex_col gap_1>
                        <div text_xs text_color={cx.theme().muted_foreground}>
                            {"Evidence reference"}
                        </div>
                        <div
                            text_xs
                            font_weight={FontWeight::SEMIBOLD}
                            text_color={cx.theme().foreground}
                        >
                            {evidence_reference}
                        </div>
                    </div>
                </div>
            })
            .child(rsx! {
                <div flex flex_col gap_2>
                    <div
                        text_xs
                        font_weight={FontWeight::SEMIBOLD}
                        text_color={cx.theme().foreground}
                    >
                        {"Reason / Note"}
                    </div>
                    <div w_full>
                        {gpui_component::input::Input::new(
                            &self.forms_editor_active_note_input,
                        )}
                    </div>
                    <div text_xs text_color={cx.theme().muted_foreground}>
                        {"The note and manual include/exclude decision remain pending until Save Profile succeeds."}
                    </div>
                </div>
            })
            .into_any()
    }
}
