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
        let mut rows = div()
            .flex()
            .flex_col()
            .border_1()
            .border_color(cx.theme().border);
        if self.stored_profile_versions.is_empty() {
            rows = rows.child(
                div()
                    .p_4()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("No COR uploads or manual drafts yet."),
            );
        } else {
            rows = rows.child(
                div()
                    .flex()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .bg(cx.theme().secondary)
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().muted_foreground)
                    .child(div().w(px(240.)).flex_shrink_0().child("Document Name"))
                    .child(div().flex_1().flex_basis(px(0.)).child("Upload Date"))
                    .child(div().flex_1().flex_basis(px(0.)).child("Extracted Forms"))
                    .child(div().flex_1().flex_basis(px(0.)).child("Status"))
                    .child(div().flex_1().flex_basis(px(0.)).child("Actions")),
            );

            for version in &self.stored_profile_versions {
                let version_id = version.id.clone();
                let version_id_for_review = version.id.clone();
                let version_id_for_archive = version.id.clone();
                let version_id_for_delete = version.id.clone();
                let first_document = version.evidence.first();
                let document_name = first_document
                    .map(|document| document.file_name.clone())
                    .unwrap_or_else(|| version.label.clone());
                let uploaded_at = first_document
                    .and_then(|document| document.uploaded_at)
                    .map(|uploaded_at| uploaded_at.format("%b %d, %Y %H:%M").to_string())
                    .unwrap_or_else(|| "Manual draft".to_string());
                let extracted_forms = first_document
                    .map(|document| document.extracted_form_codes.join(", "))
                    .filter(|forms| !forms.trim().is_empty())
                    .unwrap_or_else(|| {
                        let mut forms = version
                            .registered_tax_types
                            .iter()
                            .map(Self::registered_tax_type_label)
                            .collect::<Vec<_>>();
                        if forms.is_empty() {
                            "No forms captured".to_string()
                        } else {
                            forms.sort();
                            forms.join(", ")
                        }
                    });

                let (status_label, bg_color, text_color) = match version.status {
                    bir_core::profile::TaxProfileVersionStatus::Draft => (
                        "Review Needed",
                        gpui::rgba(0xfce8e8ff),
                        gpui::rgba(0xcb2424ff),
                    ),
                    bir_core::profile::TaxProfileVersionStatus::Confirmed => {
                        ("Completed", gpui::rgba(0xe8fce8ff), gpui::rgba(0x24cb24ff))
                    }
                    bir_core::profile::TaxProfileVersionStatus::Archived => {
                        ("Archived", gpui::rgba(0xe0e0e0ff), gpui::rgba(0x606060ff))
                    }
                };

                let is_selected = self.ocr_selected_version_id.as_deref() == Some(&version.id);
                rows = rows.child(
                    div()
                        .flex()
                        .gap_2()
                        .px_3()
                        .py_3()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .items_center()
                        .when(is_selected, |this| this.bg(cx.theme().accent))
                        .child(
                            div()
                                .w(px(240.))
                                .flex_shrink_0()
                                .flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().foreground)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(document_name),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex_basis(px(0.))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(uploaded_at),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex_basis(px(0.))
                                .text_sm()
                                .text_color(cx.theme().foreground)
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(extracted_forms),
                        )
                        .child(
                            div().flex_1().flex_basis(px(0.)).flex().child(
                                div().px_2().py_1().rounded_full().bg(bg_color).child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(text_color)
                                        .child(status_label),
                                ),
                            ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex_basis(px(0.))
                                .flex()
                                .gap_2()
                                .child(
                                    div()
                                        .id(format!("action_{}", version_id_for_review))
                                        .cursor_pointer()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().primary)
                                        .hover(|this| this.text_color(cx.theme().primary_hover))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.ocr_selected_version_id =
                                                Some(version_id_for_review.clone());
                                            this.sync_document_viewer(cx);
                                            if let Err(message) = this.load_cor_version_editor(
                                                &version_id_for_review,
                                                window,
                                                cx,
                                            ) {
                                                this.save_message = Some(message);
                                            }
                                            cx.notify();
                                        }))
                                        .child(
                                            if version.status
                                                == bir_core::profile::TaxProfileVersionStatus::Draft
                                            {
                                                "Review"
                                            } else {
                                                "View Data"
                                            },
                                        ),
                                )
                                .child(
                                    div()
                                        .id(format!("delete_{}", version_id_for_delete))
                                        .cursor_pointer()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(gpui::Hsla::from(gpui::rgba(0xef4444ff)))
                                        .hover(|this| {
                                            this.text_color(gpui::Hsla::from(gpui::rgba(
                                                0xdc2626ff,
                                            )))
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            // Delete evidence files from disk
                                            if let Some(version) = this
                                                .stored_profile_versions
                                                .iter()
                                                .find(|v| v.id == version_id_for_delete)
                                            {
                                                for evidence in &version.evidence {
                                                    let _ =
                                                        std::fs::remove_file(&evidence.stored_path);
                                                }
                                            }
                                            this.stored_profile_versions
                                                .retain(|v| v.id != version_id_for_delete);
                                            // Clear selection if we deleted the active one
                                            if this.ocr_selected_version_id.as_deref()
                                                == Some(&version_id_for_delete)
                                            {
                                                this.ocr_selected_version_id = None;
                                                this.interactive_document_viewer = None;
                                            }
                                            if this.cor_editing_version_id.as_deref()
                                                == Some(&version_id_for_delete)
                                            {
                                                this.cor_editing_version_id = None;
                                            }
                                            this.save_message = Some(
                                                "COR version deleted. Save the profile to persist."
                                                    .into(),
                                            );
                                            cx.notify();
                                        }))
                                        .child("Delete"),
                                ),
                        ),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child("Taxpayer Profile - OCR Extraction Timeline"),
            )
            .child(self.render_cor_ocr_settings(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .p_10()
                    .gap_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary)
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Upload New Document (PDF, JPG, PNG) - Max 10MB"),
                    )
                    .when(self.is_uploading_cor, |this| {
                        this.child(
                            div().flex().items_center().gap_2().child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().primary)
                                    .child("Uploading & Extracting..."),
                            ),
                        )
                    })
                    .when(!self.is_uploading_cor, |this| {
                        this.child(
                            gpui_component::button::Button::new("ocr_upload_cor_timeline")
                                .label("Browse Files")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.upload_cor_document(window, cx);
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("OCR Upload History"),
                    )
                    .child(rows),
            )
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
                gpui::rgba(0xe8fce8ff),
                gpui::rgba(0x24cb24ff),
            ),
            bir_core::profile::TaxProfileVersionStatus::Confirmed => {
                ("Committed", gpui::rgba(0xe8fce8ff), gpui::rgba(0x24cb24ff))
            }
            bir_core::profile::TaxProfileVersionStatus::Archived => {
                ("Archived", gpui::rgba(0xe0e0e0ff), gpui::rgba(0x606060ff))
            }
        };

        let version_id_for_confirm = version.id.clone();
        let version_id_for_review = version.id.clone();
        let document_id_for_open = first_document.map(|doc| doc.id.clone());

        let header = div()
            .flex()
            .flex_col()
            .gap_4()
            .pb_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        div()
                            .id("back_to_timeline")
                            .cursor_pointer()
                            .hover(|this| this.text_color(cx.theme().foreground))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.ocr_selected_version_id = None;
                                this.sync_document_viewer(cx);
                                cx.notify();
                            }))
                            .child("← Back to Timeline")
                    )
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .gap_3()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child(document_name)
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_1()
                                            .rounded_full()
                                            .bg(bg_color)
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(text_color)
                                                    .child(status_label)
                                            )
                                    )
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("Uploaded on {} • Processed by Gemini OCR", uploaded_at))
                            )
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .when(
                                version.status != bir_core::profile::TaxProfileVersionStatus::Confirmed,
                                |this| {
                                    this.child(
                                        gpui_component::button::Button::new("ocr_commit_detail")
                                            .label("Commit to Profile")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                match this.confirm_cor_version(&version_id_for_confirm, window, cx) {
                                                    Ok(()) => {
                                                        this.save_message = Some("COR version confirmed. Save the profile to persist it.".into());
                                                        this.compliance_source_mode = Self::derive_compliance_source_mode(&this.stored_profile_versions);
                                                    }
                                                    Err(message) => this.save_message = Some(message),
                                                }
                                                cx.notify();
                                            }))
                                    )
                                }
                            )
                    )
            );

        let left_col = div().flex().flex_col().flex_1().w_full().h_full().child(
            div()
                .flex_1()
                .w_full()
                .h_full()
                .bg(cx.theme().secondary)
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .child(
                    if let Some(viewer) = self.interactive_document_viewer.as_ref() {
                        viewer.clone().into_any_element()
                    } else {
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Document preview not available in-app yet."),
                            )
                            .when(document_id_for_open.is_some(), |this| {
                                let doc_id = document_id_for_open.unwrap();
                                let ver_id = version_id_for_review.clone();
                                this.child(
                                    gpui_component::button::Button::new("ocr_open_ext")
                                        .label("Open Externally")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.open_cor_document(&ver_id, &doc_id);
                                            cx.notify();
                                        })),
                                )
                            })
                            .into_any_element()
                    },
                ),
        );

        let mut identified_forms = div().flex().flex_col().gap_3();
        if let Some(first_doc) = version.evidence.first() {
            if first_doc.extracted_form_codes.is_empty() {
                identified_forms = identified_forms.child(
                    div()
                        .p_4()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("No specific tax obligations identified from this document."),
                );
            } else {
                for form_code in &first_doc.extracted_form_codes {
                    identified_forms = identified_forms.child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .p_4()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_base()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(cx.theme().foreground)
                                                    .child(format!("Form {}", form_code)),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_full()
                                                    .bg(gpui::rgba(0xffe4e6ff))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(gpui::rgba(0xbe123cff))
                                                            .child("NEW"),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "Extracted obligation for {}",
                                                form_code
                                            )),
                                    ),
                            ),
                    );
                }
            }
        }

        // ── Right column: directly editable fields (no manual override toggle) ──
        let right_col = div()
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
            // Editable fields using the existing cor_*_input entities
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
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
                                            this.focus_ocr_field("tin", cx);
                                        }),
                                    )
                                    .child(Self::field_label("TIN", cx))
                                    .child(Input::new(&self.cor_tin_input)),
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
                                            this.focus_ocr_field("rdo_code", cx);
                                        }),
                                    )
                                    .child(Self::field_label("RDO Code", cx))
                                    .child(Input::new(&self.cor_rdo_code_input)),
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
                            .child(Self::field_label("Extracted Applicable Forms", cx))
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
                                    .child(Self::field_label("Registration Date", cx))
                                    .child(Input::new(&self.cor_registration_date_input)),
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
                                    .child(Input::new(&self.cor_effective_from_input)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(Self::field_label("Effective Until", cx))
                                    .child(Input::new(&self.cor_effective_until_input)),
                            ),
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
            );

        div().flex().flex_col().gap_6().child(header).child(
            // Responsive 2-column layout using flex instead of grid
            div()
                .flex()
                .gap_6()
                .child(div().flex_1().min_w_0().child(left_col))
                .child(div().flex_1().min_w_0().child(right_col)),
        )
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

        div()
            .flex()
            .flex_col()
            .gap_1()
            .cursor_pointer()
            .on_mouse_down(
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
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .when(is_focused, |this| this.text_color(cx.theme().primary))
                    .when(!is_focused, |this| {
                        this.text_color(cx.theme().muted_foreground)
                    })
                    .child(label.to_string()),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .when(is_focused, |this| {
                        this.bg(cx.theme().primary.opacity(0.1))
                            .border_1()
                            .border_color(cx.theme().primary)
                    })
                    .when(!is_focused, |this| {
                        this.bg(cx.theme().secondary)
                            .border_1()
                            .border_color(gpui::transparent_black())
                    })
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(value.to_string()),
            )
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
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
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
        };

        let draft_preview_label = draft_version_id.and_then(|version_id| {
            preview_profile
                .profile_versions
                .iter_mut()
                .find(|version| {
                    version.id == version_id
                        && version.status == bir_core::profile::TaxProfileVersionStatus::Draft
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

    fn render_compliance_source_section(&self, cx: &Context<Self>) -> Div {
        let is_smart = self.compliance_source_mode
            == bir_core::profile::ComplianceSourceMode::TemporalSuggestion;
        let is_cor =
            self.compliance_source_mode == bir_core::profile::ComplianceSourceMode::CorVersioned;

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
                    .child("Compliance Source"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .id("compliance_source_smart")
                            .flex_1()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .cursor_pointer()
                            .text_sm()
                            .when(is_smart, |s| {
                                s.bg(cx.theme().primary)
                                    .text_color(cx.theme().primary_foreground)
                                    .font_weight(FontWeight::BOLD)
                            })
                            .when(!is_smart, |s| {
                                s.bg(cx.theme().background)
                                    .text_color(cx.theme().foreground)
                                    .hover(|s| s.bg(cx.theme().accent))
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.compliance_source_mode =
                                    bir_core::profile::ComplianceSourceMode::TemporalSuggestion;
                                cx.notify();
                            }))
                            .child("Use Smart Suggestions"),
                    )
                    .child(
                        div()
                            .id("compliance_source_cor")
                            .flex_1()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .cursor_pointer()
                            .text_sm()
                            .when(is_cor, |s| {
                                s.bg(cx.theme().primary)
                                    .text_color(cx.theme().primary_foreground)
                                    .font_weight(FontWeight::BOLD)
                            })
                            .when(!is_cor, |s| {
                                s.bg(cx.theme().background)
                                    .text_color(cx.theme().foreground)
                                    .hover(|s| s.bg(cx.theme().accent))
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.compliance_source_mode =
                                    bir_core::profile::ComplianceSourceMode::CorVersioned;
                                cx.notify();
                            }))
                            .child("Manage COR Timeline"),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if is_smart {
                        "Dashboard compliance is generated from the current flat profile and temporal tax rules."
                    } else {
                        "Confirmed COR/manual versions become the authority for dashboard forms and deadlines."
                    }),
            )
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
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
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
        };
        preview_profile.profile_versions = self.stored_profile_versions.clone();
        let draft_preview = self
            .cor_editing_version_id
            .as_ref()
            .and_then(|editing_id| {
                self.stored_profile_versions.iter().find(|version| {
                    version.id == *editing_id
                        && version.status == bir_core::profile::TaxProfileVersionStatus::Draft
                })
            })
            .or_else(|| {
                self.stored_profile_versions.iter().find(|version| {
                    version.status == bir_core::profile::TaxProfileVersionStatus::Draft
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
                    evidence_list =
                        evidence_list.child(
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
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.open_cor_document(
                                                    &version_id_for_open,
                                                    &document_id_for_open,
                                                );
                                                cx.notify();
                                            })),
                                        )
                                        .child(
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
                                        ),
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
                                    .label("Edit Details")
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
                                    version.status != bir_core::profile::TaxProfileVersionStatus::Confirmed,
                                    |this| {
                                        this.child(
                                            gpui_component::button::Button::new(format!(
                                                "confirm_cor_{}",
                                                id_for_confirm
                                            ))
                                            .label("Confirm")
                                            .small()
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                match this.confirm_cor_version(&id_for_confirm, window, cx) {
                                                    Ok(()) => this.save_message =
                                                        Some("COR version confirmed. Save the profile to persist it.".into()),
                                                    Err(message) => this.save_message = Some(message),
                                                }
                                                cx.notify();
                                            })),
                                        )
                                    },
                                )
                                .when(
                                    version.status != bir_core::profile::TaxProfileVersionStatus::Archived,
                                    |this| {
                                        this.child(
                                            gpui_component::button::Button::new(format!(
                                                "archive_cor_{}",
                                                id_for_archive
                                            ))
                                            .label("Archive")
                                            .small()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let Some(version) = this
                                                    .stored_profile_versions
                                                    .iter_mut()
                                                    .find(|version| version.id == id_for_archive)
                                                {
                                                    version.status = bir_core::profile::TaxProfileVersionStatus::Archived;
                                                    this.save_message = Some(
                                                        "COR version archived. Save the profile to persist it.".into(),
                                                    );
                                                }
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
                                        this.upload_cor_document(window, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("sync_cor_from_profile")
                                    .label("Create Draft From Profile")
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.sync_current_profile_to_cor_draft(cx);
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
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
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
                                    .child("COR OCR"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        "Default stays local/manual. Gemini runs only when enabled and consented for an upload.",
                                    ),
                            ),
                    )
                    .child(Self::render_checkbox(
                        "gemini_ocr_enabled_toggle",
                        "Enable Gemini OCR",
                        self.gemini_ocr_enabled,
                        cx,
                    )),
            )
            .when(self.gemini_ocr_enabled, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
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
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(cx.theme().muted_foreground)
                                                .child("API Key"),
                                        )
                                        .child(Input::new(&self.gemini_ocr_api_key_input)),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(cx.theme().muted_foreground)
                                                .child("Model"),
                                        )
                                        .child(Combobox::new(&self.gemini_ocr_model_select)),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(cx.theme().muted_foreground)
                                                .child("Custom Model ID"),
                                        )
                                        .child(Input::new(&self.gemini_ocr_custom_model_input)),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(
                                    "BYOK only. Free tier, paid tier, quota, and availability depend on your Google account and selected Gemini model.",
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    gpui_component::button::Button::new("save_cor_ocr_settings")
                                        .label("Save OCR Settings")
                                        .small()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.save_cor_ocr_settings(window, cx);
                                        })),
                                )
                                .child(
                                    gpui_component::button::Button::new("remove_cor_ocr_key")
                                        .label("Remove Key")
                                        .small()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.remove_gemini_ocr_key(window, cx);
                                        })),
                                ),
                        )
                        .child(Self::render_checkbox(
                            "gemini_ocr_consent_toggle",
                            "For the next COR upload, send the selected document to Google Gemini using my API key",
                            self.gemini_ocr_cloud_consent,
                            cx,
                        ))
                        .when(self.gemini_ocr_status.is_some(), |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(self.gemini_ocr_status.clone().unwrap_or_default()),
                            )
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
                            .child("COR Version Details"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Editing: {}", version.label)),
                    ),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap_2()
                    .child(Input::new(&self.cor_version_label_input))
                    .child(Input::new(&self.cor_tin_input))
                    .child(Input::new(&self.cor_registration_date_input))
                    .child(Input::new(&self.cor_effective_from_input))
                    .child(Input::new(&self.cor_effective_until_input))
                    .child(Input::new(&self.cor_registered_name_input))
                    .child(Input::new(&self.cor_trade_name_input))
                    .child(Input::new(&self.cor_rdo_code_input))
                    .child(Input::new(&self.cor_registered_address_input))
                    .child(Input::new(&self.cor_lob_code_input))
                    .child(Input::new(&self.cor_lob_description_input)),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap_2()
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
            .child(
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
                            .child("Registered tax types"),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(2)
                            .gap_2()
                            .child(self.render_registered_tax_type_toggle(
                                &version_id,
                                bir_core::profile::RegisteredTaxType::IncomeTax,
                                cx,
                            ))
                            .child(self.render_registered_tax_type_toggle(
                                &version_id,
                                bir_core::profile::RegisteredTaxType::ValueAddedTax,
                                cx,
                            ))
                            .child(self.render_registered_tax_type_toggle(
                                &version_id,
                                bir_core::profile::RegisteredTaxType::PercentageTax,
                                cx,
                            ))
                            .child(self.render_registered_tax_type_toggle(
                                &version_id,
                                bir_core::profile::RegisteredTaxType::RegistrationFee,
                                cx,
                            ))
                            .child(self.render_registered_tax_type_toggle(
                                &version_id,
                                bir_core::profile::RegisteredTaxType::WithholdingExpanded,
                                cx,
                            ))
                            .child(self.render_registered_tax_type_toggle(
                                &version_id,
                                bir_core::profile::RegisteredTaxType::WithholdingCompensation,
                                cx,
                            ))
                            .child(self.render_registered_tax_type_toggle(
                                &version_id,
                                bir_core::profile::RegisteredTaxType::WithholdingFinal,
                                cx,
                            ))
                            .child(self.render_registered_tax_type_toggle(
                                &version_id,
                                bir_core::profile::RegisteredTaxType::ExciseTax,
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        gpui_component::button::Button::new("apply_cor_version_editor")
                            .label("Apply Details")
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Err(message) = this.apply_cor_version_editor(window, cx) {
                                    this.save_message = Some(message);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        gpui_component::button::Button::new("close_cor_version_editor")
                            .label("Close")
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cor_editing_version_id = None;
                                this.clear_cor_version_editor(window, cx);
                                cx.notify();
                            })),
                    ),
            )
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

        div()
            .id(format!("cor_tax_type_{}_{}", version_id, label))
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .when(checked, |this| this.bg(cx.theme().accent))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_cor_registered_tax_type(&id, tax_type.clone());
                cx.notify();
            }))
            .child(
                div()
                    .size_4()
                    .rounded_sm()
                    .border_1()
                    .border_color(cx.theme().border)
                    .when(checked, |this| this.bg(cx.theme().primary)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().foreground)
                    .child(label),
            )
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
            for (index, override_rule) in version.obligation_overrides.iter().enumerate() {
                let version_id = version.id.clone();
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
                        )
                        .child(
                            gpui_component::button::Button::new(format!(
                                "remove_obligation_override_{}_{}",
                                version.id, index
                            ))
                            .label("Remove")
                            .small()
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.remove_cor_obligation_override(&version_id, index);
                                    cx.notify();
                                },
                            )),
                        ),
                );
            }
        }

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
                                move |this, _, _, cx| {
                                    this.remove_cor_deadline_override(&version_id, index);
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
                            .child("Manual obligation override"),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(3)
                            .gap_2()
                            .child(Input::new(&self.cor_obligation_form_input))
                            .child(Input::new(&self.cor_obligation_reason_input))
                            .child(Input::new(&self.cor_obligation_source_input)),
                    )
                    .child(obligation_overrides)
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                gpui_component::button::Button::new("add_cor_include_override")
                                    .label("Force Include")
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if let Err(message) = this.add_cor_obligation_override(
                                            bir_core::profile::ManualObligationOverrideAction::Include,
                                            window,
                                            cx,
                                        ) {
                                            this.save_message = Some(message);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("add_cor_exclude_override")
                                    .label("Force Exclude")
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if let Err(message) = this.add_cor_obligation_override(
                                            bir_core::profile::ManualObligationOverrideAction::Exclude,
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
            .child(
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
                                if let Err(message) = this.add_cor_deadline_override(window, cx) {
                                    this.save_message = Some(message);
                                }
                                cx.notify();
                            })),
                    ),
            )
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
            bir_core::profile::RegisteredTaxType::ExciseTax => "Excise Tax",
        }
    }

    fn profile_version_status_label(
        status: &bir_core::profile::TaxProfileVersionStatus,
    ) -> &'static str {
        match status {
            bir_core::profile::TaxProfileVersionStatus::Draft => "Draft",
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
