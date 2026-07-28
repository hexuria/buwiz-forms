//! Tax Profile tab — basic identity, TIN, RDO, address, and tax classification fields.

use super::*;
use gpui_rsx::rsx;

impl ProfileManagerView {
    /// Render the "Tax Profile" tab (tab index 0).
    ///
    /// Contains: TIN input, duplicate TIN error, RDO/type row, classification/EOPT row,
    /// line-of-business, name, address, zip/phone row, email, date, VAT toggle,
    /// granular withholding obligation switches and excise tax multi-select.
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
                        <div flex_1 min_w_0>
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
                    <div base={v_flex()} w_full>
                        {Self::field_label("Line of Business", cx)}
                        {Input::new(&self.line_of_business)}
                        {self.field_error("line_of_business", cx)}
                    </div>
                },
            )
            .child(
                // Taxpayer's Name (full width)
                rsx! {
                    <div base={v_flex()} w_full>
                        {Self::field_label("Taxpayer's Name", cx)}
                        {Input::new(&self.name_input)}
                        {self.field_error("full_name", cx)}
                    </div>
                },
            )
            .child(
                // Registered Address (full width)
                rsx! {
                    <div base={v_flex()} w_full>
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
                        <div flex_1 min_w_0>
                            {Self::field_label("Zip Code", cx)}
                            {Combobox::new(&self.zip_select)}
                            {self.field_error("zip_code", cx)}
                        </div>
                        <div flex_1 min_w_0>
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
                    <div base={v_flex()} w_full>
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
            // ── Registration & Status ──
            .child(rsx! {
                <div flex gap_4 w_full>
                    <div flex_1 min_w_0>
                        {Self::field_label("Registration Activity Status", cx)}
                        {Combobox::new(&self.registration_activity_status_select)}
                    </div>
                    <div flex_1 min_w_0 flex items_end>
                        {Self::render_checkbox(
                            "dormant_toggle",
                            "Dormant Entity",
                            self.is_dormant,
                            cx,
                        )}
                    </div>
                </div>
            })
            // ── VAT Registration (only for entities with business activity) ──
            .when(has_business_activity, |this| {
                this.child(rsx! {
                    <div
                        id={"vat_toggle"}
                        flex
                        items_center
                        gap_2
                        cursor_pointer
                        on_click={cx.listener(|this, _, _, cx| {
                            this.is_vat_registered = !this.is_vat_registered;
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
                            bg={if self.is_vat_registered {
                                cx.theme().primary
                            } else {
                                cx.theme().background
                            }}
                            flex
                            items_center
                            justify_center
                        >
                            {if self.is_vat_registered {
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().primary_foreground)
                                    .child("✓")
                            } else {
                                div()
                            }}
                        </div>
                        <div text_sm text_color={cx.theme().foreground}>
                            {"VAT registered taxpayer"}
                        </div>
                    </div>
                })
            })
            // ── Granular Withholding Obligations ──
            .child(rsx! {
                <div flex flex_col gap_2>
                    <div
                        text_sm
                        font_weight={FontWeight::SEMIBOLD}
                        text_color={cx.theme().foreground}
                    >
                        {"Withholding Obligations"}
                    </div>
                    <div flex flex_wrap gap_x={px(24.)} gap_y={px(8.)}>
                        {Self::render_checkbox(
                            "wh_compensation_toggle",
                            "Compensation",
                            self.withholds_compensation,
                            cx,
                        )}
                        {Self::render_checkbox(
                            "wh_expanded_toggle",
                            "Expanded",
                            self.withholds_expanded,
                            cx,
                        )}
                        {Self::render_checkbox(
                            "wh_final_toggle",
                            "Final",
                            self.withholds_final,
                            cx,
                        )}
                        {Self::render_checkbox(
                            "wh_top_agent_toggle",
                            "Top Withholding Agent",
                            self.is_top_withholding_agent,
                            cx,
                        )}
                        {Self::render_checkbox(
                            "wh_govt_toggle",
                            "Government Entity",
                            self.is_government_withholding_entity,
                            cx,
                        )}
                    </div>
                </div>
            })
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
            .child(rsx! {
                <div base={v_flex()} w_full mt_4>
                    {Self::field_label("Excise Tax Liabilities", cx)}
                    {MultiSelect::new(&self.excise_select)}
                </div>
            })
            .into_any_element()
    }

    /// Render the OCR extraction and COR review workflow (tab index 1).
    pub(super) fn render_ocr_tab(&self, cx: &Context<Self>) -> gpui::AnyElement {
        if self.active_tab != 1 {
            return div().into_any_element();
        }

        if self.ocr_selected_version_id.is_some() {
            let preview_year_text = self
                .cor_preview_year_input
                .read(cx)
                .value()
                .trim()
                .to_string();
            let selected_year = preview_year_text
                .parse::<u16>()
                .ok()
                .filter(|year| (1900..=2200).contains(year))
                .unwrap_or_else(|| chrono::Local::now().year() as u16);
            self.render_ocr_detail_view(selected_year, cx)
                .into_any_element()
        } else {
            self.render_ocr_timeline_view(cx).into_any_element()
        }
    }

    fn render_ocr_timeline_view(&self, cx: &Context<Self>) -> Div {
        // ── Sub-tab bar ─────────────────────────────────────────────────────
        let sub_tab_bar = rsx! {
            <div flex items_center gap_1 p_1 rounded_lg bg={cx.theme().secondary}>
                <div
                    id={"cor_sub_tab_0"}
                    px_4
                    py_1p5
                    rounded_md
                    cursor_pointer
                    text_sm
                    when={(self.cor_sub_tab == 0, |s| {
                        s.bg(cx.theme().background)
                            .shadow_sm()
                            .text_color(cx.theme().foreground)
                            .font_weight(FontWeight::SEMIBOLD)
                    })}
                    when={(self.cor_sub_tab != 0, |s| {
                        s.hover(|s| s.bg(cx.theme().muted))
                            .text_color(cx.theme().muted_foreground)
                            .font_weight(FontWeight::MEDIUM)
                    })}
                    on_click={cx.listener(|this, _, _, cx| {
                        this.cor_sub_tab = 0;
                        cx.notify();
                    })}
                >
                    {"Uploads"}
                </div>
                <div
                    id={"cor_sub_tab_1"}
                    px_4
                    py_1p5
                    rounded_md
                    cursor_pointer
                    text_sm
                    when={(self.cor_sub_tab == 1, |s| {
                        s.bg(cx.theme().background)
                            .shadow_sm()
                            .text_color(cx.theme().foreground)
                            .font_weight(FontWeight::SEMIBOLD)
                    })}
                    when={(self.cor_sub_tab != 1, |s| {
                        s.hover(|s| s.bg(cx.theme().muted))
                            .text_color(cx.theme().muted_foreground)
                            .font_weight(FontWeight::MEDIUM)
                    })}
                    on_click={cx.listener(|this, _, _, cx| {
                        this.cor_sub_tab = 1;
                        cx.notify();
                    })}
                >
                    {"Forms & Elections"}
                </div>
                <div
                    id={"cor_sub_tab_2"}
                    px_4
                    py_1p5
                    rounded_md
                    cursor_pointer
                    text_sm
                    when={(self.cor_sub_tab == 2, |s| {
                        s.bg(cx.theme().background)
                            .shadow_sm()
                            .text_color(cx.theme().foreground)
                            .font_weight(FontWeight::SEMIBOLD)
                    })}
                    when={(self.cor_sub_tab != 2, |s| {
                        s.hover(|s| s.bg(cx.theme().muted))
                            .text_color(cx.theme().muted_foreground)
                            .font_weight(FontWeight::MEDIUM)
                    })}
                    on_click={cx.listener(|this, _, _, cx| {
                        this.cor_sub_tab = 2;
                        cx.notify();
                    })}
                >
                    {"OCR Settings"}
                </div>
            </div>
        };

        // ── Sub-tab 2: OCR Settings ─────────────────────────────────────────
        if self.cor_sub_tab == 2 {
            return rsx! {
                <div flex flex_col gap_6>
                    <div
                        text_2xl
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().foreground}
                    >
                        {"COR — Gemini OCR Settings"}
                    </div>
                    {sub_tab_bar}
                    {self.render_cor_ocr_settings(cx)}
                </div>
            };
        }

        // ── Sub-tab 1: Forms Set and annual elections ──────────────────────
        if self.cor_sub_tab == 1 {
            let profile = self.current_profile(cx);
            // Eligibility is a per-year fact resolved from the confirmed
            // segments effective in the selected Forms Set year — not from
            // the current flat profile. Fails closed on zero or conflicting
            // segments.
            let eligible_for_income_tax_election =
                profile.eligible_for_income_tax_election_in_year(self.forms_editor_year);

            return div()
                .flex()
                .flex_col()
                .gap_6()
                .child(rsx! {
                    <div
                        text_2xl
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().foreground}
                    >
                        {"COR — Forms Set & Annual Elections"}
                    </div>
                })
                .child(sub_tab_bar)
                .child(rsx! {
                    <div
                        p_3
                        rounded_md
                        border_1
                        border_color={cx.theme().border}
                        bg={cx.theme().secondary}
                        text_xs
                        text_color={cx.theme().muted_foreground}
                    >
                        {"Reviewed COR codes and confirmed registration facts suggest forms. The saved yearly Forms Set is what the app files. Annual income-tax elections are user-confirmed choices for that year; they are shown here with the COR workflow but are not claimed as text extracted from the COR."}
                    </div>
                })
                .when(
                    // Recorded elections keep driving obligation gating even
                    // when the selected year is not election-eligible, so the
                    // table (and its Remove actions) must stay reachable;
                    // only adding new elections is gated per year.
                    eligible_for_income_tax_election || !self.stored_tax_elections.is_empty(),
                    |this| {
                        this.child(
                            self.render_tax_election_section(
                                eligible_for_income_tax_election,
                                cx,
                            ),
                        )
                    },
                )
                .child(self.render_active_forms_tab(cx));
        }

        // ── Sub-tab 0: Uploads (Card View) ──────────────────────────────────
        let mut cards = div().flex().flex_col().gap_3();

        if self.stored_profile_versions.is_empty() {
            cards = cards.child(rsx! {
                <div
                    p_6
                    rounded_lg
                    border_1
                    border_color={cx.theme().border}
                    bg={cx.theme().secondary.opacity(0.5)}
                    flex
                    flex_col
                    items_center
                    justify_center
                    gap_2
                >
                    <div text_sm text_color={cx.theme().muted_foreground}>
                        {"No COR uploads or manual drafts yet."}
                    </div>
                    <div text_xs text_color={cx.theme().muted_foreground}>
                        {"Use \"New from Tax Profile +\" to create a reviewable COR draft."}
                    </div>
                </div>
            });
        } else {
            for version in &self.stored_profile_versions {
                let version_id_for_review = version.id.clone();
                let version_id_for_delete = version.id.clone();
                let version_id_for_correction = version.id.clone();
                let version_facts_editable =
                    Self::profile_version_facts_are_editable(&version.status);
                let registration_fee_only = version.registered_tax_types.len() == 1
                    && version
                        .registered_tax_types
                        .contains(&RegisteredTaxType::RegistrationFee);
                let first_document = version.evidence.first();
                let document_name = first_document
                    .map(|document| document.file_name.clone())
                    .unwrap_or_else(|| version.label.clone());
                let uploaded_at = first_document
                    .and_then(|document| document.uploaded_at)
                    .map(|uploaded_at| uploaded_at.format("%b %d, %Y %H:%M").to_string())
                    .unwrap_or_else(|| "Manual draft".to_string());

                let extracted_forms: Vec<String> = first_document
                    .map(|document| document.extracted_form_codes.clone())
                    .filter(|forms| !forms.is_empty())
                    .unwrap_or_else(|| {
                        let mut forms = version
                            .registered_tax_types
                            .iter()
                            .map(Self::registered_tax_type_label)
                            .map(|tax_type| format!("Tax type: {tax_type}"))
                            .collect::<Vec<_>>();
                        forms.sort();
                        forms
                    });

                let (status_label, bg_color, text_color) = match version.status {
                    bir_core::profile::TaxProfileVersionStatus::Draft => (
                        "Review Needed",
                        cx.theme().danger.opacity(0.12),
                        crate::theme::danger_on_tint(cx.theme()),
                    ),
                    bir_core::profile::TaxProfileVersionStatus::NeedsReview => (
                        "Effective Date Review Needed",
                        cx.theme().warning.opacity(0.15),
                        crate::theme::warning_on_tint(cx.theme()),
                    ),
                    bir_core::profile::TaxProfileVersionStatus::Confirmed => (
                        "Completed",
                        cx.theme().success.opacity(0.15),
                        crate::theme::success_on_tint(cx.theme()),
                    ),
                    bir_core::profile::TaxProfileVersionStatus::Archived => (
                        "Archived",
                        cx.theme().secondary,
                        cx.theme().muted_foreground,
                    ),
                };

                let action_label = if matches!(
                    version.status,
                    bir_core::profile::TaxProfileVersionStatus::Draft
                        | bir_core::profile::TaxProfileVersionStatus::NeedsReview
                ) {
                    "Review"
                } else {
                    "View Data"
                };

                // Form tag chips
                let form_chips = div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .children(extracted_forms.iter().map(|form_code| {
                        rsx! {
                            <div
                                px_2
                                py={px(2.)}
                                rounded_md
                                bg={cx.theme().secondary}
                                border_1
                                border_color={cx.theme().border}
                            >
                                <div
                                    text_xs
                                    font_weight={FontWeight::MEDIUM}
                                    text_color={cx.theme().foreground}
                                >
                                    {form_code.clone()}
                                </div>
                            </div>
                        }
                    }))
                    .when(extracted_forms.is_empty(), |this| {
                        this.child(rsx! {
                            <div text_xs text_color={cx.theme().muted_foreground}>
                                {"No forms captured"}
                            </div>
                        })
                    });

                cards = cards.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .hover(|s| s.bg(cx.theme().accent))
                        // Row 1: Document name + status badge
                        .child(rsx! {
                            <div flex items_center justify_between gap_2>
                                <div flex items_center gap_2 min_w_0>
                                    {Icon::new(IconName::File)
                                        .small()
                                        .text_color(cx.theme().muted_foreground)}
                                    <div
                                        text_sm
                                        font_weight={FontWeight::SEMIBOLD}
                                        text_color={cx.theme().foreground}
                                        overflow_hidden
                                        text_ellipsis
                                    >
                                        {document_name}
                                    </div>
                                </div>
                                <div flex_shrink_0 px_2 py_1 rounded_full bg={bg_color}>
                                    <div
                                        text_xs
                                        font_weight={FontWeight::SEMIBOLD}
                                        text_color={text_color}
                                    >
                                        {status_label}
                                    </div>
                                </div>
                            </div>
                        })
                        // Row 2: Upload date
                        .child(rsx! {
                            <div text_xs text_color={cx.theme().muted_foreground}>
                                {uploaded_at}
                            </div>
                        })
                        // Row 3: Extracted forms chips
                        .child(form_chips)
                        .when(registration_fee_only, |this| {
                            this.child(rsx! {
                                <div
                                    px_3
                                    py_2
                                    rounded_md
                                    border_1
                                    border_color={cx.theme().warning}
                                    bg={cx.theme().warning.opacity(0.12)}
                                    text_xs
                                    text_color={crate::theme::warning_on_tint(cx.theme())}
                                >
                                    {"Needs review: Registration Fee by itself is incomplete for an active business profile. Create a correction draft and review the COR tax types and exact form codes."}
                                </div>
                            })
                        })
                        // Row 4: Actions
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_3()
                                .pt_1()
                                .border_t_1()
                                .border_color(cx.theme().border)
                                .child(rsx! {
                                    <div
                                        id={format!("action_{}", version_id_for_review)}
                                        cursor_pointer
                                        text_sm
                                        font_weight={FontWeight::SEMIBOLD}
                                        text_color={cx.theme().primary}
                                        hover={|this| this.text_color(cx.theme().primary_hover)}
                                        on_click={cx.listener(move |this, _, window, cx| {
                                            if let Err(message) = this.load_cor_version_editor(
                                                &version_id_for_review,
                                                window,
                                                cx,
                                            ) {
                                                this.save_message = Some(message);
                                            }
                                            cx.notify();
                                        })}
                                    >
                                        {action_label}
                                    </div>
                                })
                                .when(version_facts_editable, |this| {
                                    this.child(rsx! {
                                        <div
                                            id={format!("delete_{}", version_id_for_delete)}
                                            cursor_pointer
                                            text_sm
                                            font_weight={FontWeight::SEMIBOLD}
                                            text_color={gpui::Hsla::from(gpui::rgba(0xef4444ff))}
                                            hover={|this| {
                                                this.text_color(gpui::Hsla::from(gpui::rgba(
                                                    0xdc2626ff,
                                                )))
                                            }}
                                            on_click={cx.listener(move |this, _, window, cx| {
                                                this.delete_cor_version(
                                                    &version_id_for_delete,
                                                    window,
                                                    cx,
                                                );
                                            })}
                                        >
                                            {"Delete"}
                                        </div>
                                    })
                                })
                                .when(!version_facts_editable, |this| {
                                    this.child(rsx! {
                                        <div
                                            id={format!(
                                                "correct_{}",
                                                version_id_for_correction
                                            )}
                                            cursor_pointer
                                            text_sm
                                            font_weight={FontWeight::SEMIBOLD}
                                            text_color={cx.theme().primary}
                                            hover={|this| {
                                                this.text_color(cx.theme().primary_hover)
                                            }}
                                            on_click={cx.listener(
                                                move |this, _, window, cx| {
                                                    if let Err(message) = this
                                                        .create_cor_correction_draft(
                                                            &version_id_for_correction,
                                                            window,
                                                            cx,
                                                        )
                                                    {
                                                        this.save_message = Some(message);
                                                    }
                                                    cx.notify();
                                                },
                                            )}
                                        >
                                            {"Create correction draft"}
                                        </div>
                                    })
                                }),
                        ),
                );
            }
        }

        rsx! {
            <div flex flex_col gap_6>
                <div text_2xl font_weight={FontWeight::BOLD} text_color={cx.theme().foreground}>
                    {"COR — Upload / Manual Timeline"}
                </div>
                {sub_tab_bar}
                <div flex flex_col gap_3>
                    <div flex justify_between items_center>
                        <div
                            text_xl
                            font_weight={FontWeight::SEMIBOLD}
                            text_color={cx.theme().foreground}
                        >
                            {"COR Upload History"}
                        </div>
                        {gpui_component::button::Button::new("add_manual_cor_btn")
                            .label("New from Tax Profile +")
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.sync_current_profile_to_cor_draft(window, cx);
                                this.save_message = Some(
                                    "COR draft created from current profile fields. Review and confirm it before saving.".into(),
                                );
                                cx.notify();
                            }))}
                    </div>
                    {cards}
                </div>
            </div>
        }
    }

    fn render_ocr_detail_view(&self, selected_year: u16, cx: &Context<Self>) -> Div {
        let selected_version = self
            .ocr_selected_version_id
            .as_ref()
            .and_then(|version_id| {
                self.stored_profile_versions
                    .iter()
                    .find(|version| &version.id == version_id)
            })
            .or_else(|| self.stored_profile_versions.first());

        let Some(version) = selected_version else {
            return div();
        };

        let first_document = version.evidence.first();
        let document_name = first_document
            .map(|document| document.file_name.clone())
            .unwrap_or_else(|| version.label.clone());
        let uploaded_at = first_document
            .and_then(|document| document.uploaded_at)
            .map(|uploaded_at| uploaded_at.format("%b %d, %Y at %H:%M").to_string())
            .unwrap_or_else(|| "Manual draft".to_string());

        let (status_label, bg_color, text_color) = match version.status {
            bir_core::profile::TaxProfileVersionStatus::Draft => (
                "Extraction Completed",
                cx.theme().success.opacity(0.15),
                crate::theme::success_on_tint(cx.theme()),
            ),
            bir_core::profile::TaxProfileVersionStatus::NeedsReview => (
                "Effective Date Review Required",
                cx.theme().warning.opacity(0.15),
                crate::theme::warning_on_tint(cx.theme()),
            ),
            bir_core::profile::TaxProfileVersionStatus::Confirmed => (
                "Committed",
                cx.theme().success.opacity(0.15),
                crate::theme::success_on_tint(cx.theme()),
            ),
            bir_core::profile::TaxProfileVersionStatus::Archived => (
                "Archived",
                cx.theme().secondary,
                cx.theme().muted_foreground,
            ),
        };

        let version_id_for_confirm = version.id.clone();
        let version_id_for_review = version.id.clone();
        let version_id_for_correction = version.id.clone();
        let version_facts_editable = Self::profile_version_facts_are_editable(&version.status);
        let document_id_for_open = first_document.map(|doc| doc.id.clone());

        let header = rsx! {
            <div w_full flex flex_col gap_4 pb_4 border_b_1 border_color={cx.theme().border}>
                <div
                    w_full
                    flex
                    gap_2
                    items_center
                    text_sm
                    text_color={cx.theme().muted_foreground}
                >
                    <div
                        id={"back_to_timeline"}
                        cursor_pointer
                        hover={|this| this.text_color(cx.theme().foreground)}
                        on_click={cx.listener(|this, _, _, cx| {
                            this.ocr_selected_version_id = None;
                            this.sync_document_viewer(cx);
                            cx.notify();
                        })}
                    >
                        {"← Back to Timeline"}
                    </div>
                </div>
                <div w_full flex flex_col gap_3>
                    // Row 1: Title
                    <div
                        w_full
                        text_2xl
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().foreground}
                    >
                        {document_name}
                    </div>
                    {
                        // Row 2: Badge + Commit to Profile Button
                        div()
                            .w_full()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_4()
                            .child(
                                // Status Badge
                                rsx! {
                                    <div px_2 py_1 rounded_full bg={bg_color}>
                                        <div
                                            text_xs
                                            font_weight={FontWeight::SEMIBOLD}
                                            text_color={text_color}
                                        >
                                            {status_label}
                                        </div>
                                    </div>
                                },
                            )
                            .when(
                                matches!(
                                    version.status,
                                    bir_core::profile::TaxProfileVersionStatus::Draft
                                        | bir_core::profile::TaxProfileVersionStatus::NeedsReview
                                ),
                                |this| {
                                    this.child(
                                        gpui_component::button::Button::new("ocr_commit_detail")
                                            .label("Commit to Profile")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.request_cor_version_confirmation(
                                                    &version_id_for_confirm,
                                                    window,
                                                    cx,
                                                );
                                            })),
                                    )
                                },
                            )
                    }
                    // Row 3: Uploaded on
                    <div w_full text_sm text_color={cx.theme().muted_foreground}>
                        {format!(
                            "Uploaded on {} • Processed by Gemini OCR",
                            uploaded_at
                        )}
                    </div>
                </div>
            </div>
        };

        let left_col = rsx! {
            <div flex flex_col flex_1 w_full h_full>
                <div
                    flex_1
                    w_full
                    h_full
                    min_h={px(600.)}
                    bg={cx.theme().secondary}
                    rounded_md
                    border_1
                    border_color={cx.theme().border}
                    flex
                    items_center
                    justify_center
                    overflow_hidden
                >
                    {
                    if let Some(viewer) = self.interactive_document_viewer.as_ref() {
                        viewer.clone().into_any_element()
                    } else {
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_4()
                            .when(
                                document_id_for_open.is_none() && version_facts_editable,
                                |this| {
                                    let ver_id = version_id_for_review.clone();
                                    this.child(rsx! {
                                        <div flex flex_col items_center gap_3>
                                            {gpui_component::button::Button::new(
                                                "ocr_center_upload",
                                            )
                                            .label("Upload COR")
                                            .on_click(
                                                cx.listener(move |this, _, window, cx| {
                                                    this.upload_cor_document(
                                                        Some(ver_id.clone()),
                                                        window,
                                                        cx,
                                                    );
                                                    cx.notify();
                                                }),
                                            )}
                                        </div>
                                    })
                                },
                            )
                            .when(
                                document_id_for_open.is_none() && !version_facts_editable,
                                |this| {
                                    this.child(rsx! {
                                        <div text_sm text_color={cx.theme().muted_foreground}>
                                            {Self::immutable_cor_version_message()}
                                        </div>
                                    })
                                },
                            )
                            .when(document_id_for_open.is_some(), |this| {
                                let doc_id = document_id_for_open.unwrap();
                                let ver_id = version_id_for_review.clone();
                                this.child(rsx! {
                                    <div flex flex_col items_center gap_4>
                                        <div text_sm text_color={cx.theme().muted_foreground}>
                                            {"Document preview not available in-app yet."}
                                        </div>
                                        {gpui_component::button::Button::new("ocr_open_ext")
                                            .label("Open Externally")
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.open_cor_document(&ver_id, &doc_id);
                                                cx.notify();
                                            }))}
                                    </div>
                                })
                            })
                            .into_any_element()
                    }
                    }
                </div>
            </div>
        };

        let mut identified_forms = div().flex().flex_col().gap_3();
        if let Some(first_doc) = version.evidence.first() {
            if first_doc.extracted_form_codes.is_empty() {
                identified_forms = identified_forms.child(rsx! {
                    <div
                        p_4
                        rounded_md
                        border_1
                        border_color={cx.theme().border}
                        text_sm
                        text_color={cx.theme().muted_foreground}
                    >
                        {"No specific tax obligations identified from this document."}
                    </div>
                });
            } else {
                for form_code in &first_doc.extracted_form_codes {
                    identified_forms = identified_forms.child(rsx! {
                        <div
                            flex
                            justify_between
                            items_center
                            p_4
                            rounded_md
                            border_1
                            border_color={cx.theme().border}
                            bg={cx.theme().background}
                        >
                            <div flex flex_col gap_1>
                                <div flex gap_2 items_center>
                                    <div
                                        text_base
                                        font_weight={FontWeight::SEMIBOLD}
                                        text_color={cx.theme().foreground}
                                    >
                                        {format!("Form {}", form_code)}
                                    </div>
                                    <div px_2 py_0p5 rounded_full bg={gpui::rgba(0xffe4e6ff)}>
                                        <div
                                            text_xs
                                            font_weight={FontWeight::BOLD}
                                            text_color={gpui::rgba(0xbe123cff)}
                                        >
                                            {"NEW"}
                                        </div>
                                    </div>
                                </div>
                                <div text_sm text_color={cx.theme().muted_foreground}>
                                    {format!(
                                        "Extracted obligation for {}",
                                        form_code
                                    )}
                                </div>
                            </div>
                        </div>
                    });
                }
            }
        }

        // ── Right column: only drafts/review records are editable ──
        let right_col = if !Self::profile_version_facts_are_editable(&version.status) {
            rsx! {
                <div
                    flex
                    flex_col
                    gap_4
                    p_4
                    rounded_lg
                    border_1
                    border_color={cx.theme().border}
                    bg={cx.theme().background}
                >
                    <div
                        text_lg
                        font_weight={FontWeight::SEMIBOLD}
                        text_color={cx.theme().foreground}
                    >
                        {"Extracted Entity Data (read only)"}
                    </div>
                    <div text_sm text_color={cx.theme().muted_foreground}>
                        {Self::immutable_cor_version_message()}
                    </div>
                    <div text_sm text_color={cx.theme().foreground}>
                        {format!(
                            "TIN: {}\nName: {}\nRDO: {}\nAddress: {}\nLine of business: {}",
                            version.cor.tin.as_deref().unwrap_or("not captured"),
                            version.cor.registered_name,
                            version.cor.rdo_code,
                            version.cor.registered_address,
                            version.cor.line_of_business_description
                        )}
                    </div>
                    <div flex flex_col gap_1>
                        {Self::field_label("Version Label", cx)}
                        {Input::new(&self.cor_version_label_input)}
                    </div>
                    {gpui_component::button::Button::new("ocr_save_locked_label")
                        .label("Save Label")
                        .small()
                        .on_click(cx.listener(|this, _, window, cx| {
                            if let Err(message) = this.apply_cor_version_editor(window, cx) {
                                this.save_message = Some(message);
                            }
                            cx.notify();
                        }))}
                    {gpui_component::button::Button::new("ocr_create_correction")
                        .label("Create Correction Draft")
                        .small()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Err(message) = this.create_cor_correction_draft(
                                &version_id_for_correction,
                                window,
                                cx,
                            ) {
                                this.save_message = Some(message);
                            }
                            cx.notify();
                        }))}
                </div>
            }
        } else {
            let current_profile_preset_id = version.id.clone();
            let non_vat_preset_id = version.id.clone();
            let vat_preset_id = version.id.clone();
            div()
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
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child("Extracted Entity Data"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Edit fields directly. Changes are kept in memory until you Save Profile.",
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("Starting presets"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "Presets only populate a draft. They are not COR evidence and never confirm themselves; review every selected tax type and exact form code.",
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                gpui_component::button::Button::new(
                                    "cor_preset_current_profile",
                                )
                                .label("Use Current Tax Profile")
                                .small()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if let Err(message) = this.apply_cor_profile_preset(
                                        &current_profile_preset_id,
                                        CorProfilePreset::CurrentTaxProfile,
                                        window,
                                        cx,
                                    ) {
                                        this.save_message = Some(message);
                                    }
                                    cx.notify();
                                })),
                            )
                            .child(
                                gpui_component::button::Button::new(
                                    "cor_preset_non_vat_business",
                                )
                                .label("Non-VAT Business")
                                .small()
                                .ghost()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if let Err(message) = this.apply_cor_profile_preset(
                                        &non_vat_preset_id,
                                        CorProfilePreset::NonVatBusiness,
                                        window,
                                        cx,
                                    ) {
                                        this.save_message = Some(message);
                                    }
                                    cx.notify();
                                })),
                            )
                            .child(
                                gpui_component::button::Button::new(
                                    "cor_preset_vat_business",
                                )
                                .label("VAT Business")
                                .small()
                                .ghost()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if let Err(message) = this.apply_cor_profile_preset(
                                        &vat_preset_id,
                                        CorProfilePreset::VatBusiness,
                                        window,
                                        cx,
                                    ) {
                                        this.save_message = Some(message);
                                    }
                                    cx.notify();
                                })),
                            ),
                    ),
            )
            // Editable fields using the existing cor_*_input entities
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.focus_ocr_field("tin", cx);
                                        }),
                                    )
                                    .child(Self::field_label("TIN", cx))
                                    .child(self.cor_tin_input.clone().into_any_element()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.focus_ocr_field("rdo_code", cx);
                                        }),
                                    )
                                    .child(Self::field_label("RDO Code", cx))
                                    .child(Combobox::new(&self.cor_rdo_select)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.focus_ocr_field("registered_name", cx);
                                }),
                            )
                            .child(Self::field_label("Registered Name", cx))
                            .child(Input::new(&self.cor_registered_name_input)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.focus_ocr_field("trade_name", cx);
                                }),
                            )
                            .child(Self::field_label("Trade Name", cx))
                            .child(Input::new(&self.cor_trade_name_input)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.focus_ocr_field("registered_address", cx);
                                }),
                            )
                            .child(Self::field_label("Registered Address", cx))
                            .child(Input::new(&self.cor_registered_address_input)),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.focus_ocr_field("line_of_business_code", cx);
                                        }),
                                    )
                                    .child(Self::field_label("Line of Business Code", cx))
                                    .child(Input::new(&self.cor_lob_code_input)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.focus_ocr_field("line_of_business_description", cx);
                                        }),
                                    )
                                    .child(Self::field_label("Line of Business", cx))
                                    .child(Input::new(&self.cor_lob_description_input)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Self::field_label(
                                &format!(
                                    "Reviewed COR form-code evidence ({} official choices)",
                                    bir_core::forms::registry::FORM_REGISTRY.len()
                                ),
                                cx,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap_2()
                                            .children(self.cor_extracted_forms.iter().map(|form_code| {
                                                let code = form_code.clone();
                                                let code_for_close = form_code.clone();
                                                div()
                                                    .bg(cx.theme().secondary)
                                                    .border_1()
                                                    .border_color(cx.theme().border)
                                                    .rounded_md()
                                                    .px_2()
                                                    .py_1()
                                                    .child(
                                                    div()
                                                        .flex()
                                                        .gap_2()
                                                        .items_center()
                                                        .child(
                                                            div()
                                                                .cursor_pointer()
                                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                                    this.focused_ocr_field = Some(code.clone());
                                                                    if let Some(viewer) = &this.interactive_document_viewer {
                                                                        viewer.update(cx, |viewer, cx| viewer.set_active_field(Some(code.clone()), cx));
                                                                    }
                                                                }))
                                                                .child(form_code.clone())
                                                        )
                                                        .child(
                                                            div()
                                                                .cursor_pointer()
                                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                                    this.cor_extracted_forms.retain(|c| c != &code_for_close);
                                                                    let forms = this.cor_extracted_forms.clone();
                                                                    this.cor_extracted_forms_select.update(cx, |select, cx| {
                                                                        select.set_selected_ids(forms, cx);
                                                                    });
                                                                    cx.notify();
                                                                }))
                                                                .child(Icon::new(IconName::Close).xsmall())
                                                        )
                                                )
                                            }))
                                    )
                                    .child(MultiSelect::new(&self.cor_extracted_forms_select))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "Type to filter and check one or more of all {} official form codes. These selections are evidence from this COR version. Confirming the version reconciles them into the yearly Forms Set; it does not silently replace manual include/exclude decisions.",
                                                bir_core::forms::registry::FORM_REGISTRY.len()
                                            )),
                                    )
                                    .when(self.editing_id.is_some(), |this| {
                                        this.child(
                                            gpui_component::button::Button::new(
                                                "open_forms_set_from_cor",
                                            )
                                            .label("Review authoritative Forms Set")
                                            .small()
                                            .ghost()
                                            .on_click(cx.listener(
                                                |this, _event, window, cx| {
                                                    if let Some(year) = this
                                                        .cor_effective_from_input
                                                        .read(cx)
                                                        .date
                                                        .map(|date| date.year() as u16)
                                                    {
                                                        this.forms_editor_year = year;
                                                        this.forms_editor_year_select.update(
                                                            cx,
                                                            |select, cx| {
                                                                select.set_selected_value(
                                                                    &year.to_string(),
                                                                    window,
                                                                    cx,
                                                                );
                                                            },
                                                        );
                                                    }
                                                    this.forms_editor_selected_code = None;
                                                    this.ocr_selected_version_id = None;
                                                    this.active_tab = 1;
                                                    this.cor_sub_tab = 1;
                                                    cx.notify();
                                                },
                                            )),
                                        )
                                    })
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(Self::field_label("Version Label", cx))
                                    .child(Input::new(&self.cor_version_label_input)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.focus_ocr_field("registration_date", cx);
                                        }),
                                    )
                                    .child(Self::field_label("Registration Date", cx))
                                    .child(DateInput::new(&self.cor_registration_date_input)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(Self::field_label("Effective From", cx))
                                    .child(DateInput::new(&self.cor_effective_from_input)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(Self::field_label("Effective Until", cx))
                                    .child(DateInput::new(&self.cor_effective_until_input)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .pt_3()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Tax Classification & Status"),
                            )
                            .child(
                                div()
                                    .grid()
                                    .grid_cols(2)
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(Self::field_label("Taxpayer Type", cx))
                                            .child(Combobox::new(&self.cor_taxpayer_type_select)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(Self::field_label("Tax Classification", cx))
                                            .child(Combobox::new(&self.cor_tax_classification_select)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(Self::field_label("EOPT Tier", cx))
                                            .child(Combobox::new(&self.cor_eopt_tier_select)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(Self::field_label("Registration Status", cx))
                                            .child(Combobox::new(&self.cor_registration_status_select)),
                                    ),
                            )
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .pt_3()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Registered Tax Types"),
                            )
                            .child(
                                div()
                                    .grid()
                                    .grid_cols(2)
                                    .gap_2()
                                    .child(self.render_registered_tax_type_toggle(
                                        &version.id,
                                        bir_core::profile::RegisteredTaxType::IncomeTax,
                                        cx,
                                    ))
                                    .child(self.render_registered_tax_type_toggle(
                                        &version.id,
                                        bir_core::profile::RegisteredTaxType::ValueAddedTax,
                                        cx,
                                    ))
                                    .child(self.render_registered_tax_type_toggle(
                                        &version.id,
                                        bir_core::profile::RegisteredTaxType::PercentageTax,
                                        cx,
                                    ))
                                    .child(self.render_registered_tax_type_toggle(
                                        &version.id,
                                        bir_core::profile::RegisteredTaxType::RegistrationFee,
                                        cx,
                                    ))
                                    .child(self.render_registered_tax_type_toggle(
                                        &version.id,
                                        bir_core::profile::RegisteredTaxType::WithholdingExpanded,
                                        cx,
                                    ))
                                    .child(self.render_registered_tax_type_toggle(
                                        &version.id,
                                        bir_core::profile::RegisteredTaxType::WithholdingCompensation,
                                        cx,
                                    ))
                                    .child(self.render_registered_tax_type_toggle(
                                        &version.id,
                                        bir_core::profile::RegisteredTaxType::WithholdingFinal,
                                        cx,
                                    ))
                                    .child(self.render_registered_tax_type_toggle(
                                        &version.id,
                                        bir_core::profile::RegisteredTaxType::WithholdingVatAndPercentage,
                                        cx,
                                    ))
                                    .child(self.render_registered_tax_type_toggle(
                                        &version.id,
                                        bir_core::profile::RegisteredTaxType::ExciseTax,
                                        cx,
                                    )),
                            )
                    ),
            )
            // Apply button to update in-memory version
            .child(
                div().flex().gap_2().child(
                    gpui_component::button::Button::new("ocr_apply_fields")
                        .label("Apply Changes")
                        .small()
                        .on_click(cx.listener(|this, _, window, cx| {
                            if let Err(message) = this.apply_cor_version_editor(window, cx) {
                                this.save_message = Some(message);
                            }
                            cx.notify();
                        })),
                ),
            )
        };

        rsx! {
            <div flex flex_col gap_6 w_full>
                {header}
                // Responsive 2-column layout using flex instead of grid
                <div flex flex_wrap gap_6 w_full>
                    <div flex_1 min_w={px(400.)} h_full>{left_col}</div>
                    <div flex_1 min_w={px(400.)}>{right_col}</div>
                </div>
            </div>
        }
    }
    fn ocr_field(
        &self,
        field_id: &'static str,
        label: &str,
        value: Option<&str>,
        cx: &Context<Self>,
    ) -> Div {
        let value = value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Needs review");

        let is_focused = self.focused_ocr_field.as_deref() == Some(field_id);

        rsx! {
            <div
                flex
                flex_col
                gap_1
                cursor_pointer
                on_mouse_down={(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.focused_ocr_field = Some(field_id.to_string());
                        if let Some(viewer) = &this.interactive_document_viewer {
                            viewer.update(cx, |v, cx| {
                                v.set_active_field(Some(field_id.to_string()), cx);
                            });
                        }
                        cx.notify();
                    }),
                )}
            >
                <div
                    text_xs
                    font_weight={FontWeight::SEMIBOLD}
                    when={(is_focused, |this| this.text_color(cx.theme().primary))}
                    when={(!is_focused, |this| {
                        this.text_color(cx.theme().muted_foreground)
                    })}
                >
                    {label.to_string()}
                </div>
                <div
                    px_3
                    py_2
                    rounded_md
                    when={(is_focused, |this| {
                        this.bg(cx.theme().primary.opacity(0.1))
                            .border_1()
                            .border_color(cx.theme().primary)
                    })}
                    when={(!is_focused, |this| {
                        this.bg(cx.theme().secondary)
                            .border_1()
                            .border_color(gpui::transparent_black())
                    })}
                    text_sm
                    text_color={cx.theme().foreground}
                >
                    {value.to_string()}
                </div>
            </div>
        }
    }

    fn render_ocr_preview(
        &self,
        selected_year: u16,
        draft_version_id: Option<&str>,
        cx: &Context<Self>,
    ) -> Div {
        let mut preview_profile = TaxpayerProfile {
            id: self.editing_id,
            full_name: String::new(),
            tin: Tin {
                segment1: String::new(),
                segment2: String::new(),
                segment3: String::new(),
                branch: String::new(),
            },
            rdo_code: String::new(),
            line_of_business: String::new(),
            registered_address: String::new(),
            zip_code: String::new(),
            phone: String::new(),
            email: String::new(),
            default_form_type: String::new(),
            taxpayer_type: bir_core::profile::TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            birth_date: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: self.stored_atc_codes.clone(),
            excise_tax_categories: vec![],
            tax_elections: self.stored_tax_elections.clone(),
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
            profile_versions: self.stored_profile_versions.clone(),
            compliance_source_mode: bir_core::profile::ComplianceSourceMode::CorVersioned,
            per_year_forms: std::collections::BTreeMap::new(),
        };

        let draft_preview_label = draft_version_id.and_then(|version_id| {
            preview_profile
                .profile_versions
                .iter_mut()
                .find(|version| {
                    version.id == version_id
                        && matches!(
                            version.status,
                            bir_core::profile::TaxProfileVersionStatus::Draft
                                | bir_core::profile::TaxProfileVersionStatus::NeedsReview
                        )
                })
                .map(|version| {
                    let assumed_effective_from =
                        chrono::NaiveDate::from_ymd_opt(selected_year as i32, 1, 1).unwrap();
                    let effective_from = version
                        .effective_from
                        .or(version.cor.registration_date)
                        .unwrap_or(assumed_effective_from);
                    version.effective_from = Some(effective_from);
                    version.status = bir_core::profile::TaxProfileVersionStatus::Confirmed;
                    version.label.clone()
                })
        });

        let global_deadline_overrides = self
            .db
            .lock()
            .map(|db| db.get_deadline_overrides())
            .unwrap_or_default();
        let preview =
            bir_core::integration::resolve_profile_obligations_for_year_with_global_overrides(
                &preview_profile,
                selected_year,
                &global_deadline_overrides,
            );
        let mut deadline_overrides = global_deadline_overrides;
        deadline_overrides.extend(bir_core::integration::profile_deadline_overrides_for_year(
            &preview_profile,
            selected_year,
        ));
        let preview_deadlines =
            bir_core::calendar_rules::DeadlineResolver::resolve_taxable_year_with_overrides(
                selected_year as i32,
                &deadline_overrides,
            )
            .into_iter()
            .filter(|deadline| {
                bir_core::integration::deadline_applies_to_profile(&preview_profile, deadline)
            })
            .collect::<Vec<_>>();

        let forms = if preview.form_codes.is_empty() {
            "No generated forms for this preview year.".to_string()
        } else {
            preview.form_codes.join(", ")
        };
        let active_versions = if preview.active_version_ids.is_empty() {
            "No active confirmed version for this preview year.".to_string()
        } else {
            format!(
                "Active version(s): {}",
                preview.active_version_ids.join(", ")
            )
        };

        let mut deadlines = div().flex().flex_col().gap_1();
        for deadline in preview_deadlines.iter().take(6) {
            deadlines = deadlines.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} due {} - {} - {}",
                        deadline.display_form_no,
                        deadline.final_deadline_string(),
                        deadline.period.label(),
                        deadline.status.label()
                    )),
            );
        }
        if preview_deadlines.len() > 6 {
            deadlines = deadlines.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} more deadline(s) hidden in preview.",
                        preview_deadlines.len() - 6
                    )),
            );
        }

        let mut issues = div().flex().flex_col().gap_1();
        for issue in preview.consistency_report.issues.iter().take(4) {
            let severity = Self::profile_consistency_severity_label(&issue.severity);
            let context = [
                issue
                    .version_id
                    .as_ref()
                    .map(|version_id| format!("version {version_id}")),
                issue
                    .form_code
                    .as_ref()
                    .map(|form_code| format!("form {form_code}")),
                issue
                    .source
                    .as_ref()
                    .map(|source| format!("source {source}")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" - ");
            let detail = if context.is_empty() {
                format!("{severity}: {} - {}", issue.code, issue.message)
            } else {
                format!("{severity}: {} - {} ({context})", issue.code, issue.message)
            };
            issues = issues.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(detail),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_2()
            .pt_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child(format!("Generated Forms Preview ({selected_year})")),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(forms),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(active_versions),
            )
            .when(draft_preview_label.is_some(), |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "Draft preview: {} is evaluated as if committed.",
                            draft_preview_label.clone().unwrap_or_default()
                        )),
                )
            })
            .child(
                div()
                    .mt_2()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child("Deadline Preview"),
            )
            .when(preview_deadlines.is_empty(), |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("No dated deadlines for this preview year."),
                )
            })
            .when(!preview_deadlines.is_empty(), |this| this.child(deadlines))
            .when(!preview.consistency_report.issues.is_empty(), |this| {
                this.child(
                    div()
                        .mt_2()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child("Diagnostics"),
                )
                .child(issues)
            })
    }

    fn render_cor_timeline_section(&self, cx: &Context<Self>) -> Div {
        let preview_year_text = self
            .cor_preview_year_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let selected_year = preview_year_text
            .parse::<u16>()
            .ok()
            .filter(|year| (1900..=2200).contains(year))
            .unwrap_or_else(|| chrono::Local::now().year() as u16);
        let preview_year_needs_review = preview_year_text
            .parse::<u16>()
            .map(|year| !(1900..=2200).contains(&year))
            .unwrap_or(true);
        let mut preview_profile = TaxpayerProfile {
            id: self.editing_id,
            full_name: String::new(),
            tin: Tin {
                segment1: String::new(),
                segment2: String::new(),
                segment3: String::new(),
                branch: String::new(),
            },
            rdo_code: String::new(),
            line_of_business: String::new(),
            registered_address: String::new(),
            zip_code: String::new(),
            phone: String::new(),
            email: String::new(),
            default_form_type: String::new(),
            taxpayer_type: bir_core::profile::TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            birth_date: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: self.stored_atc_codes.clone(),
            excise_tax_categories: vec![],
            tax_elections: self.stored_tax_elections.clone(),
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
            profile_versions: self.stored_profile_versions.clone(),
            compliance_source_mode: bir_core::profile::ComplianceSourceMode::CorVersioned,
            per_year_forms: std::collections::BTreeMap::new(),
        };
        preview_profile.profile_versions = self.stored_profile_versions.clone();
        let draft_preview = self
            .cor_editing_version_id
            .as_ref()
            .and_then(|editing_id| {
                self.stored_profile_versions.iter().find(|version| {
                    version.id == *editing_id
                        && matches!(
                            version.status,
                            bir_core::profile::TaxProfileVersionStatus::Draft
                                | bir_core::profile::TaxProfileVersionStatus::NeedsReview
                        )
                })
            })
            .or_else(|| {
                self.stored_profile_versions.iter().find(|version| {
                    matches!(
                        version.status,
                        bir_core::profile::TaxProfileVersionStatus::Draft
                            | bir_core::profile::TaxProfileVersionStatus::NeedsReview
                    )
                })
            })
            .map(|version| {
                (
                    version.id.clone(),
                    version.label.clone(),
                    version.effective_from.is_none(),
                )
            });
        if let Some((version_id, _, missing_effective_from)) = draft_preview.as_ref() {
            let assumed_effective_from =
                chrono::NaiveDate::from_ymd_opt(selected_year as i32, 1, 1).unwrap();
            let effective_from = preview_profile
                .profile_versions
                .iter_mut()
                .find(|version| version.id == *version_id)
                .map(|version| {
                    if version.effective_from.is_none() {
                        version.effective_from = Some(assumed_effective_from);
                        version.needs_effective_date_review = true;
                    }
                    version.effective_from.unwrap_or(assumed_effective_from)
                })
                .unwrap_or(assumed_effective_from);
            if *missing_effective_from {
                preview_profile
                    .profile_versions
                    .iter_mut()
                    .filter(|version| version.id == *version_id)
                    .for_each(|version| version.needs_effective_date_review = true);
            }
            preview_profile.set_profile_version_confirmed(version_id, effective_from);
        }
        let global_deadline_overrides = self
            .db
            .lock()
            .map(|db| db.get_deadline_overrides())
            .unwrap_or_default();
        let preview =
            bir_core::integration::resolve_profile_obligations_for_year_with_global_overrides(
                &preview_profile,
                selected_year,
                &global_deadline_overrides,
            );
        let mut preview_deadline_overrides = global_deadline_overrides;
        preview_deadline_overrides.extend(
            bir_core::integration::profile_deadline_overrides_for_year(
                &preview_profile,
                selected_year,
            ),
        );
        let preview_deadlines =
            bir_core::calendar_rules::DeadlineResolver::resolve_taxable_year_with_overrides(
                selected_year as i32,
                &preview_deadline_overrides,
            )
            .into_iter()
            .filter(|deadline| {
                bir_core::integration::deadline_applies_to_profile(&preview_profile, deadline)
            })
            .collect::<Vec<_>>();
        let preview_forms = if preview.form_codes.is_empty() {
            if draft_preview.is_some() {
                "No draft COR obligations for this preview year.".to_string()
            } else {
                "No confirmed COR obligations for this year.".to_string()
            }
        } else {
            preview.form_codes.join(", ")
        };
        let active_versions = if preview.active_version_ids.is_empty() {
            "No active confirmed COR version for this year.".to_string()
        } else {
            format!(
                "Active version(s): {}",
                preview.active_version_ids.join(", ")
            )
        };
        let draft_preview_message =
            draft_preview
                .as_ref()
                .map(|(_, label, missing_effective_from)| {
                    if *missing_effective_from {
                        format!(
                            "Draft preview: {label} is evaluated as if confirmed from {selected_year}-01-01 because its effective date is missing."
                        )
                    } else {
                        format!("Draft preview: {label} is evaluated as if confirmed.")
                    }
                });

        let mut versions = div().flex().flex_col().gap_2();
        if self.stored_profile_versions.is_empty() {
            versions = versions.child(
                div()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("No COR versions yet. Create a draft from the current profile fields, review it, then confirm it."),
            );
        } else {
            for version in &self.stored_profile_versions {
                let id_for_confirm = version.id.clone();
                let id_for_archive = version.id.clone();
                let id_for_edit = version.id.clone();
                let version_facts_editable =
                    Self::profile_version_facts_are_editable(&version.status);
                let details_action_label = if version_facts_editable {
                    "Edit Details"
                } else {
                    "View / Rename"
                };
                let mut evidence_list = div().flex().flex_col().gap_1();
                for document in &version.evidence {
                    let version_id_for_open = version.id.clone();
                    let document_id_for_open = document.id.clone();
                    let version_id_for_remove = version.id.clone();
                    let document_id_for_remove = document.id.clone();
                    let evidence_label = match document.ocr_confidence {
                        Some(confidence) => {
                            format!(
                                "Evidence: {} (OCR {:.0}%)",
                                document.file_name,
                                confidence * 100.0
                            )
                        }
                        None => format!("Evidence: {}", document.file_name),
                    };
                    evidence_list = evidence_list.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(evidence_label),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        gpui_component::button::Button::new(format!(
                                            "open_cor_doc_{}_{}",
                                            version.id, document.id
                                        ))
                                        .label("Open")
                                        .small()
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.open_cor_document(
                                                    &version_id_for_open,
                                                    &document_id_for_open,
                                                );
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                    .when(version_facts_editable, |this| {
                                        this.child(
                                            gpui_component::button::Button::new(format!(
                                                "remove_cor_doc_{}_{}",
                                                version.id, document.id
                                            ))
                                            .label("Remove")
                                            .small()
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.remove_cor_document(
                                                    &version_id_for_remove,
                                                    &document_id_for_remove,
                                                    window,
                                                    cx,
                                                );
                                                cx.notify();
                                            })),
                                        )
                                    }),
                            ),
                    );
                }
                let registered_tax_types = if version.registered_tax_types.is_empty() {
                    "No registered tax types".to_string()
                } else {
                    version
                        .registered_tax_types
                        .iter()
                        .map(Self::registered_tax_type_label)
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let effective = match (version.effective_from, version.effective_until) {
                    (Some(from), Some(until)) => format!("{} to {}", from, until),
                    (Some(from), None) => format!("{} onward", from),
                    (None, Some(until)) => format!("Until {}", until),
                    (None, None) => "Effective date needs review".to_string(),
                };
                let status = Self::profile_version_status_label(&version.status);
                let source = Self::profile_version_source_label(&version.source);
                let cor_tin = version.cor.tin.as_deref().unwrap_or("not captured");
                let registered_name = if version.cor.registered_name.trim().is_empty() {
                    "not captured"
                } else {
                    version.cor.registered_name.as_str()
                };
                let rdo_code = if version.cor.rdo_code.trim().is_empty() {
                    "not captured"
                } else {
                    version.cor.rdo_code.as_str()
                };
                let registration_date = version
                    .cor
                    .registration_date
                    .map(|date| date.to_string())
                    .unwrap_or_else(|| "not captured".to_string());
                versions = versions.child(
                    div()
                        .p_3()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_start()
                                .gap_3()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(cx.theme().foreground)
                                                .child(version.label.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!("{status} - {source} - {effective}")),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{} evidence file(s)", version.evidence.len())),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "COR: TIN {cor_tin} | Name {registered_name} | RDO {rdo_code} | Registered {registration_date}"
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(registered_tax_types),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "{} obligation override(s), {} deadline override(s)",
                                    version.obligation_overrides.len(),
                                    version.deadline_overrides.len()
                                )),
                        )
                        .when(!version.evidence.is_empty(), |this| this.child(evidence_list))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    gpui_component::button::Button::new(format!(
                                        "edit_cor_{}",
                                        id_for_edit
                                    ))
                                    .label(details_action_label)
                                    .small()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if let Err(message) = this.load_cor_version_editor(
                                            &id_for_edit,
                                            window,
                                            cx,
                                        ) {
                                            this.save_message = Some(message);
                                        }
                                        cx.notify();
                                    })),
                                )
                                .when(
                                    matches!(
                                        &version.status,
                                        bir_core::profile::TaxProfileVersionStatus::Draft
                                            | bir_core::profile::TaxProfileVersionStatus::NeedsReview
                                    ),
                                    |this| {
                                        this.child(
                                            gpui_component::button::Button::new(format!(
                                                "confirm_cor_{}",
                                                id_for_confirm
                                            ))
                                            .label("Confirm")
                                            .small()
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.request_cor_version_confirmation(
                                                    &id_for_confirm,
                                                    window,
                                                    cx,
                                                );
                                            })),
                                        )
                                    },
                                )
                                .when(
                                    matches!(
                                        &version.status,
                                        bir_core::profile::TaxProfileVersionStatus::Draft
                                            | bir_core::profile::TaxProfileVersionStatus::NeedsReview
                                    ),
                                    |this| {
                                        this.child(
                                            gpui_component::button::Button::new(format!(
                                                "archive_cor_{}",
                                                id_for_archive
                                            ))
                                            .label("Archive")
                                            .small()
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.archive_cor_version(
                                                    &id_for_archive,
                                                    window,
                                                    cx,
                                                );
                                                cx.notify();
                                            })),
                                        )
                                    },
                                ),
                        ),
                );
            }
        }

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
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("COR Timeline"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .w(px(120.))
                                    .child(Input::new(&self.cor_preview_year_input)),
                            )
                            .child(
                                gpui_component::button::Button::new("upload_cor_document")
                                    .label(if self.gemini_ocr_enabled {
                                        "Extract with Gemini using my API key"
                                    } else {
                                        "Upload COR"
                                    })
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.upload_cor_document(None, window, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("sync_cor_from_profile")
                                    .label("Create Draft From Profile")
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.sync_current_profile_to_cor_draft(window, cx);
                                        this.save_message = Some(
                                            "COR draft created from current profile fields. Review and confirm it before saving.".into(),
                                        );
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(self.render_cor_ocr_settings(cx))
            .child(versions)
            .when(self.cor_editing_version_id.is_some(), |this| {
                this.child(self.render_cor_version_editor(cx))
            })
            .child(
                div()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child(format!("Generated forms preview ({selected_year})")),
                    )
                    .when(preview_year_needs_review, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(gpui::Hsla::from(gpui::rgba(0xef4444ff)))
                                .child("Preview year must be between 1900 and 2200."),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(preview_forms),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(active_versions),
                    )
                    .when(draft_preview_message.is_some(), |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(draft_preview_message.clone().unwrap_or_default()),
                        )
                    })
                    .child(
                        div()
                            .mt_2()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("Deadline preview"),
                    )
                    .when(preview_deadlines.is_empty(), |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("No dated deadlines for this preview year."),
                        )
                    })
                    .when(!preview_deadlines.is_empty(), |this| {
                        let mut deadlines = div().flex().flex_col().gap_1();
                        for deadline in preview_deadlines.iter().take(6) {
                            let source = deadline
                                .source_reference
                                .as_ref()
                                .map(|source| format!(" - {source}"))
                                .unwrap_or_default();
                            deadlines = deadlines.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "{} due {} - {} - {}{}",
                                        deadline.display_form_no,
                                        deadline.final_deadline_string(),
                                        deadline.period.label(),
                                        deadline.status.label(),
                                        source
                                    )),
                            );
                        }
                        if preview_deadlines.len() > 6 {
                            deadlines = deadlines.child(
                                div().text_xs().text_color(cx.theme().muted_foreground).child(
                                    format!(
                                        "{} more deadline(s) hidden in preview.",
                                        preview_deadlines.len() - 6
                                    ),
                                ),
                            );
                        }
                        this.child(deadlines)
                    })
                    .when(!preview.consistency_report.issues.is_empty(), |this| {
                        let mut issues = div().flex().flex_col().gap_1().mt_2();
                        for issue in preview.consistency_report.issues.iter().take(4) {
                            let severity = Self::profile_consistency_severity_label(&issue.severity);
                            let context = [
                                issue
                                    .version_id
                                    .as_ref()
                                    .map(|version_id| format!("version {version_id}")),
                                issue
                                    .form_code
                                    .as_ref()
                                    .map(|form_code| format!("form {form_code}")),
                                issue.source.as_ref().map(|source| format!("source {source}")),
                            ]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .join(" - ");
                            let detail = if context.is_empty() {
                                format!("{severity}: {} - {}", issue.code, issue.message)
                            } else {
                                format!(
                                    "{severity}: {} - {} ({context})",
                                    issue.code, issue.message
                                )
                            };
                            issues = issues.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(detail)
                                    .when(issue.fix_hint.is_some(), |this| {
                                        this.child(
                                            div().child(format!(
                                                "Fix: {}",
                                                issue.fix_hint.clone().unwrap_or_default()
                                            )),
                                        )
                                    }),
                            );
                        }
                        this.child(issues)
                    }),
            )
            .when(!self.stored_profile_versions.is_empty(), |this| {
                this.child(self.render_cor_override_editor(cx))
            })
    }

    fn render_cor_ocr_settings(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(rsx! {
                <div flex flex_wrap items_center justify_between gap_3>
                    <div flex flex_col gap_1 flex_1 min_w={px(200.)}>
                        <div
                            text_sm
                            font_weight={FontWeight::SEMIBOLD}
                            text_color={cx.theme().foreground}
                        >
                            {"COR OCR"}
                        </div>
                        <div text_xs text_color={cx.theme().muted_foreground}>
                            {"Default stays local/manual. Gemini runs only when enabled and consented for an upload."}
                        </div>
                    </div>
                    <div flex_shrink_0>
                        {Self::render_checkbox(
                            "gemini_ocr_enabled_toggle",
                            "Enable Gemini OCR",
                            self.gemini_ocr_enabled,
                            cx,
                        )}
                    </div>
                </div>
            })
            .when(self.gemini_ocr_enabled, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(rsx! {
                            <div grid grid_cols={2} gap_3>
                                <div flex flex_col gap_1>
                                    <div
                                        text_xs
                                        font_weight={FontWeight::SEMIBOLD}
                                        text_color={cx.theme().muted_foreground}
                                    >
                                        {"API Key"}
                                    </div>
                                    {Input::new(&self.gemini_ocr_api_key_input)}
                                </div>
                                <div flex flex_col gap_1>
                                    <div
                                        text_xs
                                        font_weight={FontWeight::SEMIBOLD}
                                        text_color={cx.theme().muted_foreground}
                                    >
                                        {"Model"}
                                    </div>
                                    {Combobox::new(&self.gemini_ocr_model_select)}
                                </div>
                                <div flex flex_col gap_1>
                                    <div
                                        text_xs
                                        font_weight={FontWeight::SEMIBOLD}
                                        text_color={cx.theme().muted_foreground}
                                    >
                                        {"Custom Model ID"}
                                    </div>
                                    {Input::new(&self.gemini_ocr_custom_model_input)}
                                </div>
                            </div>
                        })
                        .child(rsx! {
                            <div text_xs text_color={cx.theme().muted_foreground}>
                                {"BYOK only. Free tier, paid tier, quota, and availability depend on your Google account and selected Gemini model."}
                            </div>
                        })
                        .child(rsx! {
                            <div flex items_center gap_2>
                                {gpui_component::button::Button::new("save_cor_ocr_settings")
                                    .label("Save OCR Settings")
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_cor_ocr_settings(window, cx);
                                    }))}
                                {gpui_component::button::Button::new("remove_cor_ocr_key")
                                    .label("Remove Key")
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.remove_gemini_ocr_key(window, cx);
                                    }))}
                            </div>
                        })
                        .child(Self::render_checkbox(
                            "gemini_ocr_consent_toggle",
                            "For the next COR upload, send the selected document to Google Gemini using my API key",
                            self.gemini_ocr_cloud_consent,
                            cx,
                        ))
                        .when(self.gemini_ocr_status.is_some(), |this| {
                            this.child(rsx! {
                                <div text_xs text_color={cx.theme().muted_foreground}>
                                    {self.gemini_ocr_status.clone().unwrap_or_default()}
                                </div>
                            })
                        }),
                )
            })
    }

    fn render_cor_version_editor(&self, cx: &Context<Self>) -> Div {
        let Some(version_id) = self.cor_editing_version_id.clone() else {
            return div();
        };
        let Some(version) = self
            .stored_profile_versions
            .iter()
            .find(|version| version.id == version_id)
        else {
            return div();
        };

        if !Self::profile_version_facts_are_editable(&version.status) {
            let effective = match (version.effective_from, version.effective_until) {
                (Some(from), Some(until)) => format!("{from} to {until}"),
                (Some(from), None) => format!("{from} onward"),
                (None, Some(until)) => format!("until {until}"),
                (None, None) => "date unavailable".to_string(),
            };
            let registered_tax_types = version
                .registered_tax_types
                .iter()
                .map(Self::registered_tax_type_label)
                .collect::<Vec<_>>()
                .join(", ");
            return rsx! {
                <div
                    flex
                    flex_col
                    gap_3
                    p_3
                    rounded_md
                    border_1
                    border_color={cx.theme().border}
                    bg={cx.theme().background}
                >
                    <div
                        text_sm
                        font_weight={FontWeight::SEMIBOLD}
                        text_color={cx.theme().foreground}
                    >
                        {"Confirmed or archived COR evidence (read only)"}
                    </div>
                    <div text_xs text_color={cx.theme().muted_foreground}>
                        {Self::immutable_cor_version_message()}
                    </div>
                    <div flex flex_col gap_1>
                        {Self::field_label("Version Label", cx)}
                        {Input::new(&self.cor_version_label_input)}
                    </div>
                    <div text_xs text_color={cx.theme().muted_foreground}>
                        {format!(
                            "Effective {effective} | TIN {} | Name {} | RDO {} | Tax types: {} | {} evidence file(s)",
                            version.cor.tin.as_deref().unwrap_or("not captured"),
                            version.cor.registered_name,
                            version.cor.rdo_code,
                            if registered_tax_types.is_empty() {
                                "none"
                            } else {
                                registered_tax_types.as_str()
                            },
                            version.evidence.len()
                        )}
                    </div>
                    <div flex gap_2>
                        {gpui_component::button::Button::new("save_locked_cor_label")
                            .label("Save Label")
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Err(message) =
                                    this.apply_cor_version_editor(window, cx)
                                {
                                    this.save_message = Some(message);
                                }
                                cx.notify();
                            }))}
                        {gpui_component::button::Button::new("close_locked_cor_version")
                            .label("Close")
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_cor_version_editor(window, cx);
                            }))}
                    </div>
                </div>
            };
        }

        rsx! {
            <div
                flex
                flex_col
                gap_3
                p_3
                rounded_md
                border_1
                border_color={cx.theme().border}
                bg={cx.theme().background}
            >
                <div flex items_center justify_between gap_3>
                    <div
                        text_xs
                        font_weight={FontWeight::SEMIBOLD}
                        text_color={cx.theme().foreground}
                    >
                        {"COR Version Details"}
                    </div>
                    <div text_xs text_color={cx.theme().muted_foreground}>
                        {format!("Editing: {}", version.label)}
                    </div>
                </div>
                <div flex flex_col gap_3>
                    <div grid grid_cols={2} gap_2>
                        <div flex flex_col gap_1>
                            {Self::field_label("Version Label", cx)}
                            {Input::new(&self.cor_version_label_input)}
                        </div>
                        <div flex flex_col gap_1>
                            {Self::field_label("Registration Date", cx)}
                            {DateInput::new(&self.cor_registration_date_input)}
                        </div>
                    </div>
                    <div flex flex_col gap_1>
                        {Self::field_label("TIN", cx)}
                        {self.cor_tin_input.clone().into_any_element()}
                    </div>
                    <div grid grid_cols={2} gap_2>
                        <div flex flex_col gap_1>
                            {Self::field_label("Effective From", cx)}
                            {DateInput::new(&self.cor_effective_from_input)}
                        </div>
                        <div flex flex_col gap_1>
                            {Self::field_label("Effective Until", cx)}
                            {DateInput::new(&self.cor_effective_until_input)}
                        </div>
                    </div>
                    <div grid grid_cols={2} gap_2>
                        <div flex flex_col gap_1>
                            {Self::field_label("Registered Name", cx)}
                            {Input::new(&self.cor_registered_name_input)}
                        </div>
                        <div flex flex_col gap_1>
                            {Self::field_label("Trade Name", cx)}
                            {Input::new(&self.cor_trade_name_input)}
                        </div>
                    </div>
                    <div grid grid_cols={2} gap_2>
                        <div flex flex_col gap_1>
                            {Self::field_label("RDO Code", cx)}
                            {Combobox::new(&self.cor_rdo_select)}
                        </div>
                        <div flex flex_col gap_1>
                            {Self::field_label("Registered Address", cx)}
                            {Input::new(&self.cor_registered_address_input)}
                        </div>
                    </div>
                    <div grid grid_cols={2} gap_2>
                        <div flex flex_col gap_1>
                            {Self::field_label("Line of Business Code", cx)}
                            {Input::new(&self.cor_lob_code_input)}
                        </div>
                        <div flex flex_col gap_1>
                            {Self::field_label("Line of Business Description", cx)}
                            {Input::new(&self.cor_lob_description_input)}
                        </div>
                    </div>
                </div>
                <div grid grid_cols={2} gap_2>
                    <div flex flex_col gap_1>
                        {Self::field_label("Taxpayer Type", cx)}
                        {Combobox::new(&self.cor_taxpayer_type_select)}
                    </div>
                    <div flex flex_col gap_1>
                        {Self::field_label("Tax Classification", cx)}
                        {Combobox::new(&self.cor_tax_classification_select)}
                    </div>
                    <div flex flex_col gap_1>
                        {Self::field_label("EOPT Tier", cx)}
                        {Combobox::new(&self.cor_eopt_tier_select)}
                    </div>
                    <div flex flex_col gap_1>
                        {Self::field_label("Registration Status", cx)}
                        {Combobox::new(&self.cor_registration_status_select)}
                    </div>
                </div>
                <div flex flex_col gap_2 pt_2 border_t_1 border_color={cx.theme().border}>
                    <div
                        text_xs
                        font_weight={FontWeight::SEMIBOLD}
                        text_color={cx.theme().muted_foreground}
                    >
                        {"Registered tax types"}
                    </div>
                    <div grid grid_cols={2} gap_2>
                        {self.render_registered_tax_type_toggle(
                            &version_id,
                            bir_core::profile::RegisteredTaxType::IncomeTax,
                            cx,
                        )}
                        {self.render_registered_tax_type_toggle(
                            &version_id,
                            bir_core::profile::RegisteredTaxType::ValueAddedTax,
                            cx,
                        )}
                        {self.render_registered_tax_type_toggle(
                            &version_id,
                            bir_core::profile::RegisteredTaxType::PercentageTax,
                            cx,
                        )}
                        {self.render_registered_tax_type_toggle(
                            &version_id,
                            bir_core::profile::RegisteredTaxType::RegistrationFee,
                            cx,
                        )}
                        {self.render_registered_tax_type_toggle(
                            &version_id,
                            bir_core::profile::RegisteredTaxType::WithholdingExpanded,
                            cx,
                        )}
                        {self.render_registered_tax_type_toggle(
                            &version_id,
                            bir_core::profile::RegisteredTaxType::WithholdingCompensation,
                            cx,
                        )}
                        {self.render_registered_tax_type_toggle(
                            &version_id,
                            bir_core::profile::RegisteredTaxType::WithholdingFinal,
                            cx,
                        )}
                        {self.render_registered_tax_type_toggle(
                            &version_id,
                            bir_core::profile::RegisteredTaxType::WithholdingVatAndPercentage,
                            cx,
                        )}
                        {self.render_registered_tax_type_toggle(
                            &version_id,
                            bir_core::profile::RegisteredTaxType::ExciseTax,
                            cx,
                        )}
                    </div>
                </div>
                <div flex gap_2>
                    {gpui_component::button::Button::new("apply_cor_version_editor")
                        .label("Apply Details")
                        .small()
                        .on_click(cx.listener(|this, _, window, cx| {
                            if let Err(message) = this.apply_cor_version_editor(window, cx) {
                                this.save_message = Some(message);
                            }
                            cx.notify();
                        }))}
                    {gpui_component::button::Button::new("close_cor_version_editor")
                        .label("Close")
                        .small()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_cor_version_editor(window, cx);
                        }))}
                </div>
            </div>
        }
    }

    fn render_registered_tax_type_toggle(
        &self,
        version_id: &str,
        tax_type: bir_core::profile::RegisteredTaxType,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let checked = self
            .stored_profile_versions
            .iter()
            .find(|version| version.id == version_id)
            .map(|version| version.registered_tax_types.contains(&tax_type))
            .unwrap_or(false);
        let label = Self::registered_tax_type_label(&tax_type);
        let id = version_id.to_string();

        rsx! {
            <div
                id={format!("cor_tax_type_{}_{}", version_id, label)}
                flex
                items_center
                gap_2
                cursor_pointer
                px_2
                py_1
                rounded_md
                border_1
                border_color={cx.theme().border}
                when={(checked, |this| this.bg(cx.theme().accent))}
                on_click={cx.listener(move |this, _, window, cx| {
                    this.toggle_cor_registered_tax_type(&id, tax_type.clone(), window, cx);
                    cx.notify();
                })}
            >
                <div
                    size_4
                    rounded_sm
                    border_1
                    border_color={cx.theme().border}
                    when={(checked, |this| this.bg(cx.theme().primary))}
                />
                <div text_xs text_color={cx.theme().foreground}>{label}</div>
            </div>
        }
    }

    fn render_cor_override_editor(&self, cx: &Context<Self>) -> Div {
        let target_version_id = self.cor_override_target_version_id();
        let target_version = target_version_id.as_ref().and_then(|version_id| {
            self.stored_profile_versions
                .iter()
                .find(|version| &version.id == version_id)
        });
        let target_label = target_version
            .map(|version| version.label.clone())
            .unwrap_or_else(|| "No editable COR version".to_string());

        let mut obligation_overrides = div().flex().flex_col().gap_1();
        if let Some(version) = target_version {
            for override_rule in &version.obligation_overrides {
                let action = match override_rule.action {
                    bir_core::profile::ManualObligationOverrideAction::Include => "Include",
                    bir_core::profile::ManualObligationOverrideAction::Exclude => "Exclude",
                };
                obligation_overrides = obligation_overrides.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "{} {}: {}",
                                    action, override_rule.form_code, override_rule.reason
                                )),
                        ),
                );
            }
        }
        let forms_set_year = target_version
            .and_then(|version| version.effective_from)
            .map(|date| date.year() as u16)
            .unwrap_or_else(|| chrono::Local::now().year() as u16);

        let mut deadline_overrides = div().flex().flex_col().gap_1();
        if let Some(version) = target_version {
            for (index, override_rule) in version.deadline_overrides.iter().enumerate() {
                let version_id = version.id.clone();
                deadline_overrides = deadline_overrides.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "{}: {} -> {} ({})",
                                    override_rule.title,
                                    override_rule.original_deadline,
                                    override_rule.adjusted_deadline,
                                    override_rule.affected_form_codes.join(", ")
                                )),
                        )
                        .child(
                            gpui_component::button::Button::new(format!(
                                "remove_deadline_override_{}_{}",
                                version.id, index
                            ))
                            .label("Remove")
                            .small()
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.remove_cor_deadline_override(
                                        &version_id,
                                        index,
                                        window,
                                        cx,
                                    );
                                    cx.notify();
                                },
                            )),
                        ),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("Profile Overrides"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Target: {target_label}")),
                    ),
            )
            .when(target_version_id.is_none(), |this| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Create or upload a COR draft before adding profile overrides."),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("Legacy obligation overrides (read only)"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "Existing version-level overrides remain visible for audit. Create all new per-year include/exclude decisions in the Forms Set.",
                            ),
                    )
                    .child(obligation_overrides)
                    .child(
                        gpui_component::button::Button::new("manage_forms_set_from_overrides")
                            .label(format!("Manage {} Forms Set", forms_set_year))
                            .small()
                            .ghost()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.forms_editor_year = forms_set_year;
                                this.forms_editor_selected_code = None;
                                this.forms_editor_year_select.update(cx, |select, cx| {
                                    select.set_selected_value(
                                        &forms_set_year.to_string(),
                                        window,
                                        cx,
                                    );
                                });
                                this.ocr_selected_version_id = None;
                                this.active_tab = 1;
                                this.cor_sub_tab = 1;
                                cx.notify();
                            })),
                    ),
            )
            .when(target_version_id.is_some(), |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .pt_2()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().muted_foreground)
                                .child("Profile deadline override"),
                        )
                        .child(
                            div()
                                .grid()
                                .grid_cols(2)
                                .gap_2()
                                .child(Input::new(&self.cor_deadline_title_input))
                                .child(Input::new(&self.cor_deadline_source_input))
                                .child(Input::new(&self.cor_deadline_forms_input))
                                .child(Input::new(&self.cor_deadline_reason_input))
                                .child(Input::new(&self.cor_deadline_original_input))
                                .child(Input::new(&self.cor_deadline_adjusted_input)),
                        )
                        .child(deadline_overrides)
                        .child(
                            gpui_component::button::Button::new("add_cor_deadline_override")
                                .label("Add Profile Deadline Override")
                                .small()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if let Err(message) = this.add_cor_deadline_override(window, cx)
                                    {
                                        this.save_message = Some(message);
                                    }
                                    cx.notify();
                                })),
                        ),
                )
            })
    }

    fn registered_tax_type_label(tax_type: &bir_core::profile::RegisteredTaxType) -> &'static str {
        match tax_type {
            bir_core::profile::RegisteredTaxType::IncomeTax => "Income Tax",
            bir_core::profile::RegisteredTaxType::ValueAddedTax => "VAT",
            bir_core::profile::RegisteredTaxType::PercentageTax => "Percentage Tax",
            bir_core::profile::RegisteredTaxType::RegistrationFee => "Registration Fee",
            bir_core::profile::RegisteredTaxType::WithholdingExpanded => "Expanded Withholding",
            bir_core::profile::RegisteredTaxType::WithholdingCompensation => {
                "Compensation Withholding"
            }
            bir_core::profile::RegisteredTaxType::WithholdingFinal => "Final Withholding",
            bir_core::profile::RegisteredTaxType::WithholdingVatAndPercentage => {
                "VAT / Percentage Tax Withheld (Form 1600)"
            }
            bir_core::profile::RegisteredTaxType::ExciseTax => "Excise Tax",
        }
    }

    fn profile_version_status_label(
        status: &bir_core::profile::TaxProfileVersionStatus,
    ) -> &'static str {
        match status {
            bir_core::profile::TaxProfileVersionStatus::Draft => "Draft",
            bir_core::profile::TaxProfileVersionStatus::NeedsReview => "Needs Review",
            bir_core::profile::TaxProfileVersionStatus::Confirmed => "Confirmed",
            bir_core::profile::TaxProfileVersionStatus::Archived => "Archived",
        }
    }

    fn profile_version_source_label(
        source: &bir_core::profile::TaxProfileVersionSource,
    ) -> &'static str {
        match source {
            bir_core::profile::TaxProfileVersionSource::ManualCor => "Manual COR",
            bir_core::profile::TaxProfileVersionSource::OcrCor => "OCR COR",
            bir_core::profile::TaxProfileVersionSource::UserOverride => "User Override",
            bir_core::profile::TaxProfileVersionSource::MigrationBackfill => "Legacy Backfill",
        }
    }

    fn profile_consistency_severity_label(
        severity: &bir_core::integration::ProfileConsistencySeverity,
    ) -> &'static str {
        match severity {
            bir_core::integration::ProfileConsistencySeverity::Info => "Info",
            bir_core::integration::ProfileConsistencySeverity::Warning => "Warning",
            bir_core::integration::ProfileConsistencySeverity::NeedsReview => "Needs review",
        }
    }

    /// Render user-confirmed per-year income-tax elections alongside COR reconciliation.
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
                            "The selected Forms Set year has no confirmed Individual segment registered as Self-Employed or Mixed Income, so new elections cannot be added. Existing elections above remain in effect and can be removed.",
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
                    "gemini_ocr_enabled_toggle" => {
                        this.gemini_ocr_enabled = !this.gemini_ocr_enabled;
                        if !this.gemini_ocr_enabled {
                            this.gemini_ocr_cloud_consent = false;
                        }
                        this.gemini_ocr_status = None;
                    }
                    "gemini_ocr_consent_toggle" => {
                        this.gemini_ocr_cloud_consent = !this.gemini_ocr_cloud_consent;
                    }
                    _ => {}
                }
                if !matches!(
                    id,
                    "gemini_ocr_enabled_toggle" | "gemini_ocr_consent_toggle"
                ) {
                    this.mark_profile_changed();
                }
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
        cx: &Context<Self>,
    ) -> bir_core::forms::PerYearFormsSet {
        let taxpayer_profile = self.current_profile(cx);
        let suggestions =
            bir_core::integration::form_suggestions_for_profile_year(&taxpayer_profile, year);
        bir_core::forms::reconcile_forms_set_for_year(
            year,
            taxpayer_profile.forms_set_for_year(year),
            &suggestions,
        )
        .forms_set
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
        if self.active_tab != 1 || self.cor_sub_tab != 1 || self.editing_id.is_none() {
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
                                {"Reviewed COR codes and registered tax types create suggestions here. Your include/exclude decisions become manual overrides and always win during reconciliation."}
                            </div>
                        </div>
                    })
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
                                    {"Needs review before suggestions can refresh"}
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
                        {"This saved per-year set—not the raw COR extraction—is the authoritative filing list."}
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
                            <div flex_grow>
                                {Combobox::new(&self.forms_editor_registry_form_select)}
                            </div>
                        })
                    })}
                    when={(custom_mode, |this| {
                        this.child(rsx! {
                            <div flex_grow>
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
                    <div flex_grow>
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
                    <div flex_grow px_3 text_sm text_color={cx.theme().foreground}>
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
                    <div base={header_style(cx)} flex_grow>{"Description"}</div>
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
