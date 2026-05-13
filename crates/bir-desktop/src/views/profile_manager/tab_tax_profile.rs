//! Tax Profile tab — basic identity, TIN, RDO, address, and tax classification fields.

use super::*;

impl ProfileManagerView {
    /// Render the "Tax Profile" tab (tab index 0).
    ///
    /// Contains: TIN input, duplicate TIN error, RDO/type row, classification/EOPT row,
    /// line-of-business, name, address, zip/phone row, email, date, VAT/employees toggles,
    /// 8% election toggle, and excise tax multi-select.
    pub(super) fn render_tax_profile_tab(
        &self,
        is_individual: bool,
        is_cooperative: bool,
        is_eligible_for_8_percent: bool,
        date_label: &'static str,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        if self.active_tab != 0 {
            return div().into_any_element();
        }

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
            .child(
                div()
                    .id("vat_toggle")
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.is_vat_registered = !this.is_vat_registered;
                        this.enforce_8_percent_eligibility(cx);
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
            .child(
                div()
                    .flex()
                    .gap_6()
                    .child(
                        div()
                            .id("employees_toggle")
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.has_employees = !this.has_employees;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w_4()
                                    .h_4()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(if self.has_employees {
                                        cx.theme().primary
                                    } else {
                                        cx.theme().background
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(if self.has_employees {
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
                                    .child("Has Employees"),
                            ),
                    )
                    .child(
                        div()
                            .id("expanded_withholding_toggle")
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.is_expanded_withholding_agent =
                                    !this.is_expanded_withholding_agent;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w_4()
                                    .h_4()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(if self.is_expanded_withholding_agent {
                                        cx.theme().primary
                                    } else {
                                        cx.theme().background
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(if self.is_expanded_withholding_agent {
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
                                    .child("Expanded Withholding Agent"),
                            ),
                    )
                    .when(is_eligible_for_8_percent, |this| {
                        this.child(
                            div()
                                .id("8_percent_toggle")
                                .flex()
                                .items_center()
                                .gap_2()
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.is_8_percent_flat_rate = !this.is_8_percent_flat_rate;
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .w_4()
                                        .h_4()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .bg(if self.is_8_percent_flat_rate {
                                            cx.theme().primary
                                        } else {
                                            cx.theme().background
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(if self.is_8_percent_flat_rate {
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
                                        .child("8% Income Tax Rate Election"),
                                ),
                        )
                    }),
            )
            .child(
                v_flex()
                    .w_full()
                    .mt_4()
                    .child(Self::field_label("Excise Tax Liabilities", cx))
                    .child(MultiSelect::new(&self.excise_select)),
            )
            .into_any_element()
    }
}
