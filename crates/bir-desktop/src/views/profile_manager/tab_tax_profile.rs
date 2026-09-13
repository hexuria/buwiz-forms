//! Tax Profile tab — basic identity, TIN, RDO, address, and tax classification fields.

use super::*;
use gpui_rsx::rsx;

impl ProfileManagerView {
    /// Render the "Tax Profile" tab (tab index 0).
    ///
    /// Contains: TIN input, duplicate TIN error, RDO/type row, classification/EOPT
    /// row, line-of-business, name, address, zip/phone row, email, and dates.
    /// Filing-form routing lives on the yearly Forms Set picker, not on
    /// VAT/withholding toggles.
    pub(super) fn render_tax_profile_tab(
        &self,
        is_individual: bool,
        is_cooperative: bool,
        _date_label: &'static str,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        if self.active_tab != 0 {
            return div().into_any_element();
        }

        let tax_class_val = self.tax_classification_select.read(cx).selected_value(cx);
        let is_purely_compensation = is_individual && tax_class_val == "Purely Compensation";

        let identity = div()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
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
            });

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(identity)
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
                    .child("Annual Income Tax Election"),
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

    fn forms_set_for_year(&self, year: u16) -> bir_core::forms::PerYearFormsSet {
        self.stored_per_year_forms
            .get(&year)
            .cloned()
            .unwrap_or_else(|| bir_core::forms::PerYearFormsSet::new(year))
    }

    pub(super) fn sync_forms_set_picker(&mut self, cx: &mut Context<Self>) {
        let codes = self
            .forms_set_for_year(self.forms_editor_year)
            .active_form_codes();
        self.forms_editor_forms_select.update(cx, |select, cx| {
            select.set_selected_ids(codes, cx);
        });
    }

    /// Attach the selected official forms to this year's tax profile.
    /// A second annual ITR in the same group is skipped, never stored.
    pub(super) fn apply_forms_set_selection(
        &mut self,
        selected: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let year = self.forms_editor_year;
        let mut accepted: Vec<String> = Vec::new();
        let mut skipped: Vec<String> = Vec::new();
        for raw in selected {
            let code = bir_core::forms::registry::canonical_form_code(&raw);
            if bir_core::forms::registry::find_form(&code).is_none() {
                continue;
            }
            if accepted.iter().any(|existing| existing == &code) {
                continue;
            }
            let probe = bir_core::forms::PerYearFormsSet::from_codes(
                year,
                accepted.clone(),
                bir_core::forms::FormSetSource::Manual,
            );
            if !bir_core::integration::conflicting_active_annual_itrs(&probe.entries, &code)
                .is_empty()
            {
                skipped.push(code);
                continue;
            }
            accepted.push(code);
        }

        let mut previous = self.forms_set_for_year(year).active_form_codes();
        previous.sort();
        let mut next = accepted.clone();
        next.sort();
        if previous != next {
            self.stored_per_year_forms.insert(
                year,
                bir_core::forms::PerYearFormsSet::from_codes(
                    year,
                    accepted,
                    bir_core::forms::FormSetSource::Manual,
                ),
            );
            self.mark_profile_changed();
        }
        if !skipped.is_empty() {
            self.pending_notification = Some((
                gpui_component::notification::NotificationType::Warning,
                format!(
                    "Skipped {}: only one annual ITR can be active per year.",
                    skipped.join(", ")
                ),
            ));
        }
        self.sync_forms_set_picker(cx);
        cx.notify();
    }

    fn copy_forms_from_prior_year(&mut self, prior_year: u16, cx: &mut Context<Self>) {
        let source_entries = self
            .stored_per_year_forms
            .get(&prior_year)
            .map(|set| set.entries.clone())
            .or_else(|| {
                self.current_profile(cx)
                    .per_year_forms
                    .get(&prior_year)
                    .map(|set| set.entries.clone())
            })
            .unwrap_or_default();
        let mut codes = self
            .forms_set_for_year(self.forms_editor_year)
            .active_form_codes();
        for entry in source_entries {
            if entry.is_filing_active() && !codes.iter().any(|code| code == &entry.form_code) {
                codes.push(entry.form_code);
            }
        }
        self.apply_forms_set_selection(codes, cx);
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
        let forms_set = self.forms_set_for_year(selected_year);
        let itr_conflicts = bir_core::integration::check_annual_itr_conflicts(&forms_set.entries);

        div()
            .flex()
            .flex_col()
            .gap_4()
            .w_full()
            .child(self.render_forms_set_picker(&forms_set, cx))
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
            .when(!itr_conflicts.is_empty(), |this| {
                let conflict_msg = itr_conflicts
                    .iter()
                    .map(|issue| issue.message.clone())
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
            .into_any_element()
    }

    fn render_forms_set_picker(
        &self,
        forms_set: &bir_core::forms::PerYearFormsSet,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let selected_year = self.forms_editor_year;
        let prior_year = self
            .current_profile(cx)
            .closest_prior_forms_year(selected_year);
        let mut selected_codes = forms_set.active_form_codes();
        selected_codes.sort();
        selected_codes.dedup();

        let mut chips = div().flex().flex_wrap().gap_1();
        for code in &selected_codes {
            let code_for_remove = code.clone();
            chips = chips.child(rsx! {
                <div
                    id={crate::agent::ids::form_pick_id(code)}
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
                                let mut codes = this
                                    .forms_set_for_year(this.forms_editor_year)
                                    .active_form_codes();
                                codes.retain(|existing| existing != &code_for_remove);
                                this.apply_forms_set_selection(codes, cx);
                            }),
                        )
                        .child(Icon::new(IconName::Close).xsmall())}
                </div>
            });
        }

        rsx! {
            <div
                id={crate::agent::ids::FORMS_YEAR_PICKER}
                flex
                flex_col
                gap_3
                w_full
            >
                <div flex items_center justify_between w_full>
                    <div flex flex_col gap_1>
                        <div font_weight={FontWeight::BOLD} text_lg>
                            {format!("{selected_year} Forms Set")}
                        </div>
                        <div text_xs text_color={cx.theme().muted_foreground}>
                            {"Pick the official forms to file this year. This saved set is the dashboard list."}
                        </div>
                    </div>
                    <div
                        flex
                        items_center
                        gap_2
                        whenSome={(prior_year, |this, prior_year| {
                            this.child(
                                gpui_component::button::Button::new("copy_prior_year_btn")
                                    .label(format!("Copy from {prior_year}"))
                                    .small()
                                    .ghost()
                                    .on_click(cx.listener(move |this, _ev, _window, cx| {
                                        this.copy_forms_from_prior_year(prior_year, cx);
                                    })),
                            )
                        })}
                    >
                    </div>
                </div>
                {Self::field_label("Forms", cx)}
                {MultiSelect::new(&self.forms_editor_forms_select)}
                {chips}
                <div text_xs text_color={cx.theme().muted_foreground}>
                    {"Type to filter, then check one form, several, or all official forms."}
                </div>
            </div>
        }
        .into_any()
    }
}
