//! Tax Profile tab — basic identity, TIN, RDO, address, and tax classification fields.

use super::*;

impl ProfileManagerView {
    /// Render the "Tax Profile" tab (tab index 0).
    ///
    /// Contains: TIN input, duplicate TIN error, RDO/type row, classification/EOPT row,
    /// line-of-business, name, address, zip/phone row, email, date, VAT toggle,
    /// granular withholding obligation switches, tax election ledger, and excise tax multi-select.
    pub(super) fn render_tax_profile_tab(
        &self,
        is_individual: bool,
        is_cooperative: bool,
        is_eligible_for_election: bool,
        date_label: &'static str,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        if self.active_tab != 0 {
            return div().into_any_element();
        }

        let tax_class_val = self.tax_classification_select.read(cx).selected_value(cx);
        let is_purely_compensation = is_individual && tax_class_val == "Purely Compensation";
        // VAT is only relevant for entities with business/professional activity
        let has_business_activity = !is_purely_compensation
            && !matches!(
                self.type_select.read(cx).selected_value(cx).as_str(),
                "Estate" | "Trust"
            );
        // GPP partner only relevant for individual with business/professional or mixed income
        let is_individual_with_business = is_individual
            && matches!(
                tax_class_val.as_str(),
                "Self-Employed / Professional" | "Mixed Income"
            );

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.tin_input.clone().into_any_element())
            .when(self.tin_duplicate_error.is_some(), |this| {
                let msg = self.tin_duplicate_error.clone().unwrap_or_default();
                this.child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(gpui::rgba(0xef444415))
                        .border_1()
                        .border_color(gpui::Hsla::from(gpui::rgba(0xef444460)))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(gpui::Hsla::from(gpui::rgba(0xef4444ff)))
                                .child("⚠"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(gpui::Hsla::from(gpui::rgba(0xef4444ff)))
                                .child(msg),
                        ),
                )
            })
            .child(
                // Row: RDO + Taxpayer Type (50/50)
                div()
                    .flex()
                    .gap_4()
                    .w_full()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Self::field_label("Revenue District Office (RDO)", cx))
                            .child(Combobox::new(&self.rdo_select))
                            .child(self.field_error("rdo_code", cx)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Self::field_label("Taxpayer Type", cx))
                            .child(Combobox::new(&self.type_select)),
                    ),
            )
            .when(is_individual, |this| {
                this.child(
                    // Row: Tax Classification + EOPT Tier (50/50)
                    div()
                        .flex()
                        .gap_4()
                        .w_full()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(Self::field_label("Tax Classification", cx))
                                .child(Combobox::new(&self.tax_classification_select)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(Self::field_label("EOPT Tier", cx))
                                .child(Combobox::new(&self.eopt_tier_select)),
                        ),
                )
            })
            .when(is_cooperative, |this| {
                this.child(
                    // Row: Cooperative Tax Treatment + EOPT Tier (50/50)
                    div()
                        .flex()
                        .gap_4()
                        .w_full()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(Self::field_label("Cooperative Tax Treatment", cx))
                                .child(Combobox::new(&self.cooperative_treatment_select)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(Self::field_label("EOPT Tier", cx))
                                .child(Combobox::new(&self.eopt_tier_select)),
                        ),
                )
            })
            .child(
                // Line of Business (full width)
                v_flex()
                    .w_full()
                    .child(Self::field_label("Line of Business", cx))
                    .child(Input::new(&self.line_of_business))
                    .child(self.field_error("line_of_business", cx)),
            )
            .child(
                // Taxpayer's Name (full width)
                v_flex()
                    .w_full()
                    .child(Self::field_label("Taxpayer's Name", cx))
                    .child(Input::new(&self.name_input))
                    .child(self.field_error("full_name", cx)),
            )
            .child(
                // Registered Address (full width)
                v_flex()
                    .w_full()
                    .child(Self::field_label("Registered Address", cx))
                    .child(Input::new(&self.address_input))
                    .child(self.field_error("registered_address", cx)),
            )
            .child(
                // Row: Zip Code + Phone (50/50)
                div()
                    .flex()
                    .gap_4()
                    .w_full()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Self::field_label("Zip Code", cx))
                            .child(Combobox::new(&self.zip_select))
                            .child(self.field_error("zip_code", cx)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Self::field_label("Phone / Telephone No.", cx))
                            .child(Input::new(&self.tel_input))
                            .child(self.field_error("phone", cx)),
                    ),
            )
            .child(
                // Email Address (full width)
                v_flex()
                    .w_full()
                    .child(Self::field_label("Email Address", cx))
                    .child(Input::new(&self.email_input))
                    .child(self.field_error("email", cx)),
            )
            .child(
                // Date field (full width)
                v_flex()
                    .w_full()
                    .child(Self::field_label(date_label, cx))
                    .child(DateInput::new(&self.business_start_input))
                    .child(self.field_error("business_start_date", cx)),
            )
            // ── Registration & Status ──
            .child(
                div()
                    .flex()
                    .gap_4()
                    .w_full()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Self::field_label("Registration Activity Status", cx))
                            .child(Combobox::new(&self.registration_activity_status_select)),
                    )
                    .child(div().flex_1().min_w_0().flex().items_end().child(
                        Self::render_checkbox(
                            "dormant_toggle",
                            "Dormant Entity",
                            self.is_dormant,
                            cx,
                        ),
                    )),
            )
            // ── VAT Registration (only for entities with business activity) ──
            .when(has_business_activity, |this| {
                this.child(
                    div()
                        .id("vat_toggle")
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.is_vat_registered = !this.is_vat_registered;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .w_4()
                                .h_4()
                                .rounded_sm()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(if self.is_vat_registered {
                                    cx.theme().primary
                                } else {
                                    cx.theme().background
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(if self.is_vat_registered {
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().primary_foreground)
                                        .child("✓")
                                } else {
                                    div()
                                }),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().foreground)
                                .child("VAT registered taxpayer"),
                        ),
                )
            })
            // ── Granular Withholding Obligations ──
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("Withholding Obligations"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_x(px(24.))
                            .gap_y(px(8.))
                            .child(Self::render_checkbox(
                                "wh_compensation_toggle",
                                "Compensation",
                                self.withholds_compensation,
                                cx,
                            ))
                            .child(Self::render_checkbox(
                                "wh_expanded_toggle",
                                "Expanded",
                                self.withholds_expanded,
                                cx,
                            ))
                            .child(Self::render_checkbox(
                                "wh_final_toggle",
                                "Final",
                                self.withholds_final,
                                cx,
                            ))
                            .child(Self::render_checkbox(
                                "wh_top_agent_toggle",
                                "Top Withholding Agent",
                                self.is_top_withholding_agent,
                                cx,
                            ))
                            .child(Self::render_checkbox(
                                "wh_govt_toggle",
                                "Government Entity",
                                self.is_government_withholding_entity,
                                cx,
                            )),
                    ),
            )
            // ── GPP Partner (individual business/professional or mixed income only) ──
            .when(is_individual_with_business, |this| {
                this.child(Self::render_checkbox(
                    "gpp_partner_toggle",
                    "GPP Partner",
                    self.is_gpp_partner,
                    cx,
                ))
            })
            // ── Substituted filing (purely compensation only) ──
            .when(is_purely_compensation, |this| {
                this.child(Self::render_checkbox(
                    "single_employer_toggle",
                    "Single employer (eligible for substituted filing)",
                    self.has_single_employer,
                    cx,
                ))
            })
            // ── Tax Election Ledger (replaces 8% checkbox) ──
            .when(is_eligible_for_election, |this| {
                this.child(self.render_tax_election_section(cx))
            })
            .child(
                v_flex()
                    .w_full()
                    .mt_4()
                    .child(Self::field_label("Excise Tax Liabilities", cx))
                    .child(MultiSelect::new(&self.excise_select)),
            )
            .into_any_element()
    }

    /// Render the tax election ledger section — per-year election management.
    fn render_tax_election_section(&self, cx: &Context<Self>) -> Div {
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
                    .child("Income Tax Rate Elections"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Manage per-taxable-year income tax elections (e.g. 8% flat rate, graduated + OSD)."),
            )
            // ── Existing elections table ──
            .when(!elections.is_empty(), |this| {
                let mut table = div().flex().flex_col().gap_1().mt_2();
                for election in elections {
                    let label = match &election.election {
                        bir_core::profile::IncomeTaxElection::EightPercent => "8% Flat Rate",
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
                                    .text_color(gpui::Hsla::from(gpui::rgba(0xef4444ff)))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.stored_tax_elections
                                            .retain(|e| e.taxable_year != year);
                                        cx.notify();
                                    }))
                                    .child("Remove"),
                            ),
                    );
                }
                this.child(table)
            })
            // ── Add new election row ──
            .child(
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
                        gpui_component::button::Button::new("add_election_btn")
                            .label("Add")
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let year_str = this
                                    .tax_election_year_input
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .to_string();
                                let election_val = this
                                    .tax_election_select
                                    .read(cx)
                                    .selected_value(cx);
                                if let Ok(year) = year_str.parse::<u16>() {
                                    let election = match election_val.as_str() {
                                        "8% Flat Rate" => {
                                            bir_core::profile::IncomeTaxElection::EightPercent
                                        }
                                        "Graduated + OSD" => {
                                            bir_core::profile::IncomeTaxElection::GraduatedOsd
                                        }
                                        "Graduated + Itemized" => {
                                            bir_core::profile::IncomeTaxElection::GraduatedItemized
                                        }
                                        _ => return,
                                    };
                                    // Remove existing election for this year, then add new
                                    this.stored_tax_elections
                                        .retain(|e| e.taxable_year != year);
                                    this.stored_tax_elections
                                        .push(bir_core::profile::TaxElectionHistory {
                                            taxable_year: year,
                                            election,
                                            elected_at: chrono::Local::now().naive_local(),
                                            source_form: "profile_manager".to_string(),
                                        });
                                    // Sort by year descending for display
                                    this.stored_tax_elections
                                        .sort_by_key(|b| std::cmp::Reverse(b.taxable_year));
                                    cx.notify();
                                }
                            })),
                    ),
            )
    }

    /// Reusable checkbox rendering helper.
    fn render_checkbox(
        id: &'static str,
        label: &'static str,
        checked: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
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
                cx.notify();
            }))
            .child(
                div()
                    .w_4()
                    .h_4()
                    .rounded_sm()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(if checked {
                        cx.theme().primary
                    } else {
                        cx.theme().background
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if checked {
                        div()
                            .text_xs()
                            .text_color(cx.theme().primary_foreground)
                            .child("✓")
                    } else {
                        div()
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(label),
            )
    }
}
