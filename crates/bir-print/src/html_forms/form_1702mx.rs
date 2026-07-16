use bir_core::{
    forms::{
        form_1702mx::{
            Form1702MXDeductionMethod, Form1702MXDraft, Form1702MXFilingBasis,
            Form1702MXOverpaymentDisposition, Form1702MXRegimeAmounts, PercentInput, WholePeso,
            WholePesoInput,
        },
        FilingStatus, FormValidator,
    },
    TaxpayerProfile,
};
use serde_json::json;

use crate::html::{
    RenderEnvelopeV1, RenderFormIdentity, RenderPeriod, RenderTaxpayer, RenderValidationMessage,
    RenderValidationSeverity, RenderValue,
};

use super::{
    GeneratedContractArtifact, RenderContractFixture, RenderFixtureKind, RenderFormProvider,
    RenderPageGeometry, RenderProviderError, VisualReferencePage,
};

const VISUAL_REFERENCE_PAGES: &[VisualReferencePage] = &[
    VisualReferencePage {
        page: 1,
        file_name: "1702mx-2018c-page-1.png",
        sha256: "c5c5788a1f0d6e3ae74e77099c53c8c77a233e929b3ab274599ef951b0e6af40",
    },
    VisualReferencePage {
        page: 2,
        file_name: "1702mx-2018c-page-2.png",
        sha256: "a6f00d14e7c10e0d96b3c7377625a376f7a3c5a1f2f9f9013ef14500e7d62207",
    },
    VisualReferencePage {
        page: 3,
        file_name: "1702mx-2018c-page-3.png",
        sha256: "34f425674f33e303a29b906a413c7e408a072742ecae0d91ca30d01a2a9938a4",
    },
    VisualReferencePage {
        page: 4,
        file_name: "1702mx-2018c-page-4.png",
        sha256: "f915402e6aee6bd9b4bfeefe0f7d1f2ed6c767c1284b492f0f67c364feb9065f",
    },
];

/// Exact January 2018C four-page mixed-income corporate return.
///
/// The reviewed source pack also contains a distinct two-page mandatory
/// attachment. That companion is deliberately not part of this provider's
/// pagination policy: no reviewed attachment transport contract exists, and
/// silently appending it would turn a conditional document into base-return
/// pages five and six.
pub(super) const PROVIDER: RenderFormProvider = RenderFormProvider {
    code: "1702MX",
    revision: "2018C",
    form_id: "1702MXv2018C",
    title: "Annual Income Tax Return for Mixed-Income Corporations",
    page_width_pt: RenderPageGeometry::LEGAL.width_points,
    page_height_pt: RenderPageGeometry::LEGAL.height_points,
    expected_base_page_count: 4,
    schedules: &[],
    visual_fixture_file_name: "1702mx-normal.json",
    visual_fixture_sha256: "d2f70e503945974783358b321a772e0ca4025736c93e961e29e9191779ab4ec2",
    official_source: "https://bir-cdn.bir.gov.ph/local/pdf/1702-MX%20Jan%202018%20ENCS%20Final%20with%20OSDv2.pdf",
    official_source_sha256: "81c05fffadde6c0b4098aeba8547a9820a0806c6be9b0c6ceac5597cab4263d2",
    reference_dpi: 144,
    reference_width_px: 1_224,
    reference_height_px: 1_872,
    visual_reference_pages: VISUAL_REFERENCE_PAGES,
    runtime_discrete_assets,
    fixtures,
    generated_artifacts,
};

fn runtime_discrete_assets() -> Vec<serde_json::Value> {
    let mut assets = vec![json!({
        "asset": "government_seal",
        "bits_per_component": 8,
        "color_space": "DeviceRGB",
        "derived_grayscale_pixel_sha256": "2ca1abd3e633cc7f5805e95642dee3ee0d8840336ec9163cb11eb1e138bba01c",
        "derived_png_sha256": "92b7a3fd81ee9db5705482563925d79842d1e961a5a0a931fc6d838ec7a1402e",
        "embedded_as": "lossless_grayscale_collapse_of_equal_rgb_channels",
        "embedded_color_space": "DeviceGray",
        "embedded_in": "packages/form-renderer/src/forms/assets/1702mx-seal.png",
        "source_bbox_top_left_points": [228.52, 28.968, 33.682, 28.152],
        "source_channels_equal": true,
        "source_ctm_points": [33.682, 0.0, 0.0, 28.152, 228.52, 878.88],
        "source_decoded_rgb_sha256": "80ad6c1fa30a29d7795d475da6533459d1242ee2226b6cb23cf8ef6709cba5ce",
        "source_page": 1,
        "source_pdf_object_id": [42, 0],
        "source_pixel_dimensions": [119, 102],
        "source_stream_sha256": "46f193f83e79047c3a1a4444b2004687938ae982a5a281e082db2a96c5074d04",
        "treatment": "lossless extraction of the exact official PDF image XObject; equal RGB channels collapsed to one native grayscale channel without crop, resampling, recoloring, or substitution"
    })];

    for (
        page,
        object,
        payload,
        black_modules,
        matrix_hash,
        path_hash,
        source_bbox,
        source_ctm,
        active_width,
        caption_bbox,
        stream_hash,
        decoded_hash,
        grayscale_hash,
    ) in [
        (
            1,
            43,
            "1702-MX 01/18ENCS P1",
            551,
            "58fe10021bc79bf3c031e1dcecb264a3d643f4fa8a5e54fff0e8279106f2fde2",
            "6fb048d1d2d6d2b307bc43721e734a89763ed0a9a63113409c952fba02843518",
            [432.36, 67.32, 158.04, 45.36],
            [158.04, 0.0, 0.0, 45.36, 432.36, 823.32],
            157.384232,
            [502.06, 112.2638, 89.0702, 8.9646],
            "37e87ea21c9287193f93b852b2a23d3a519103cbbf013bf41f5040ef6320fac3",
            "654c81d802b7f6a89785b554068aa6a1f783b3e333038c78b760cb076cf8eecf",
            "047e789f021779b6f960ad181b34620e67665187b583d053a35edb817b5e65f9",
        ),
        (
            2,
            50,
            "1702-MX 01/18ENCS P2",
            561,
            "38f0943e4710bb7c87585e94cbf82e6b4a338379b0d4f95edeb25128364dae25",
            "3b075e1128792795dbfeff0eb22e1046fa8e9fc019d823c1d652dea4a9ecf6f4",
            [431.4, 31.2, 160.8, 39.6],
            [160.8, 0.0, 0.0, 39.6, 431.4, 865.2],
            160.13278,
            [501.94, 72.6438, 89.0702, 8.9646],
            "6e2f7206d5bc94bc0ae177acfecf2cfcc3faa10b1373d8cc19f20a540f03700c",
            "71b3480c3947f05f4086a4bba117fc806f5cbe045b1d8e655ebefc34b338be17",
            "e295e9064b6cb7e3b96fed65d780d1ec6e3dedd7401c10457a762d3cbfca49bd",
        ),
        (
            3,
            53,
            "1702-MX 01/18ENCS P3",
            555,
            "447a3cd3ae20ed9fcf9527180fb539ef9fbcf5dcd2aeebd6aa272ccd7d4f7602",
            "a79be84a5d683b5dc4cdb59ed0b61a02eb45ca28380b1a455577443f0e5bb809",
            [430.56, 48.72, 161.04, 42.72],
            [161.04, 0.0, 0.0, 42.72, 430.56, 844.56],
            160.371784,
            [502.78, 91.0238, 89.0702, 8.9646],
            "a39cb58432fe7d9f5b08083ceae6a2a34fd99c6bb91e6d646983c5e45210cd4b",
            "27884ec439e97b91b9fa8fa53b99e0adf5c10f2cffa867d495b686306a51b010",
            "ff769285cbf789be063453264b8df33c582e609a8172f9dca1aadc4bbd20857b",
        ),
        (
            4,
            56,
            "1702-MX 01/18ENCS P4",
            547,
            "4dce7fd23395f73f138612cade05ad4bf707ddd597293eedeac889cd3eda7a2d",
            "42ad85ee8e08ecf57ac20786c5f6d569bf51060c174e0e514c82c586dbdc3a5d",
            [431.4, 33.12, 160.2, 41.64],
            [160.2, 0.0, 0.0, 41.64, 431.4, 861.24],
            159.53527,
            [502.06, 74.9238, 89.0702, 8.9646],
            "fab6936a6a41f0f829593f4b53c5a83e47071bbad48045a426cfa2fdc4bd255b",
            "e009bc43f1e9311ff2d28f7c55c3bd177ba95f85ff72a8892489700ac89ecfac",
            "077498b2477aabe1eac23db1f623985bfcc8315d4854537b03c7413599a0bfa7",
        ),
    ] {
        assets.push(json!({
            "asset": format!("static_form_pdf417_page_{page}"),
            "caption_bbox_top_left_points": caption_bbox,
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": payload,
            "decoded_payload": payload,
            "decoder_evidence": [{"decoder": "ZXing-C++ 3.1.0", "error_correction_percent": 33, "payload": payload, "symbology": "PDF417"}],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1702MXAssets.ts",
            "encoder_proof": {"columns": 3, "encoding": "ISO-8859-1", "error_correction_level": 2, "implementation": "pdf417gen 0.8.1", "module_differences": 0, "rows": 8},
            "logical_black_modules": black_modules,
            "logical_dimensions": [120, 8],
            "logical_matrix_sha256": matrix_hash,
            "svg_path_sha256": path_hash,
            "source_active_bbox_top_left_points": [source_bbox[0], source_bbox[1], active_width, source_bbox[3]],
            "source_active_pixel_bounds": [0, 0, 240, 72],
            "source_bbox_top_left_points": source_bbox,
            "source_ctm_points": source_ctm,
            "source_decoded_rgb_sha256": decoded_hash,
            "source_grayscale_pixel_sha256": grayscale_hash,
            "source_page": page,
            "source_pdf_object_id": [object, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_module_units": {"right": 0.5},
            "source_padding_pixels": {"bottom": 0, "right": 1},
            "source_pixel_dimensions": [241, 72],
            "source_stream_sha256": stream_hash,
            "symbology": "PDF417"
        }));
    }

    assets
}

impl From<&Form1702MXDraft> for RenderEnvelopeV1 {
    fn from(draft: &Form1702MXDraft) -> Self {
        let mut envelope = Self::new(
            RenderFormIdentity {
                code: PROVIDER.code.to_string(),
                version: PROVIDER.revision.to_string(),
            },
            RenderTaxpayer {
                tin: draft.tin.clone(),
                name: draft.taxpayer_name.clone(),
                rdo_code: draft.rdo_code.clone(),
                registered_address: draft.registered_address.clone(),
                zip_code: draft.zip_code.clone(),
                contact_number: draft.contact_number.clone(),
                email: draft.email.clone(),
            },
            RenderPeriod {
                taxable_year: draft.taxable_year,
                month: Some(draft.month),
                quarter: None,
                label: format!("Year ended {:02}/{}", draft.month, draft.taxable_year),
            },
        );

        insert_text(
            &mut envelope,
            "filing_basis",
            match draft.filing_basis {
                Form1702MXFilingBasis::Calendar => "calendar",
                Form1702MXFilingBasis::Fiscal => "fiscal",
            },
        );
        insert_bool(&mut envelope, "is_amended", draft.is_amended);
        insert_bool(&mut envelope, "is_short_period", draft.is_short_period);
        insert_bool(&mut envelope, "atc_mcit_selected", draft.atc.mcit_selected);
        insert_bool(
            &mut envelope,
            "atc_other_selected",
            draft.atc.other_selected,
        );
        insert_text(&mut envelope, "atc_other_code", &draft.atc.other_code);
        insert_text(
            &mut envelope,
            "deduction_method",
            match draft.deduction_method {
                Form1702MXDeductionMethod::Unresolved => "unresolved",
                Form1702MXDeductionMethod::Itemized => "itemized",
                Form1702MXDeductionMethod::OptionalStandard => "osd",
            },
        );
        insert_text(
            &mut envelope,
            "incorporation_date",
            &draft.incorporation_date,
        );
        for (index, value) in draft.registered_name_lines.iter().enumerate() {
            insert_text(
                &mut envelope,
                &format!("registered_name_line_{}", index + 1),
                value,
            );
        }
        for (index, value) in draft.registered_address_lines.iter().enumerate() {
            insert_text(
                &mut envelope,
                &format!("registered_address_line_{}", index + 1),
                value,
            );
        }

        map_part_two(&mut envelope, draft);
        map_relief_basis(&mut envelope, draft);
        map_regime_schedule(&mut envelope, "schedule_2", &draft.schedule_2.items);
        insert_optional_percent(
            &mut envelope,
            "schedule_2_item_14_special_rate",
            &draft.schedule_2.item_14_special_rate,
        );
        insert_optional_percent(
            &mut envelope,
            "schedule_2_item_14_regular_rate",
            &draft.schedule_2.item_14_regular_rate,
        );
        map_regime_schedule(
            &mut envelope,
            "schedule_3",
            &draft.schedule_3.items_20_to_33,
        );
        insert_text(
            &mut envelope,
            "schedule_3_item_30_description",
            &draft.schedule_3.item_30_description,
        );
        insert_text(
            &mut envelope,
            "schedule_3_item_31_description",
            &draft.schedule_3.item_31_description,
        );
        map_regime_schedule(&mut envelope, "schedule_4", &draft.schedule_4.items);
        map_regime_schedule(&mut envelope, "schedule_5", &draft.schedule_5.amounts);
        for (index, description) in draft
            .schedule_5
            .other_descriptions_17d_to_17i
            .iter()
            .enumerate()
        {
            insert_text(
                &mut envelope,
                &format!("schedule_5_other_description_{}", index + 1),
                description,
            );
        }
        for (index, row) in draft.schedule_6.rows.iter().enumerate() {
            let prefix = format!("schedule_6_row_{}", index + 1);
            insert_text(
                &mut envelope,
                &format!("{prefix}_description"),
                &row.description,
            );
            insert_text(
                &mut envelope,
                &format!("{prefix}_legal_basis"),
                &row.legal_basis,
            );
            map_regime_amounts(&mut envelope, &prefix, &row.amounts);
        }
        map_regime_amounts(
            &mut envelope,
            "schedule_6_item_5",
            &draft.schedule_6.item_5_total,
        );

        map_nolco_computation(&mut envelope, "schedule_7", &draft.regular_nolco);
        map_nolco_table(&mut envelope, "schedule_7_1", &draft.schedule_7_1);
        map_nolco_computation(&mut envelope, "schedule_8", &draft.special_nolco);
        map_nolco_table(&mut envelope, "schedule_8_1", &draft.schedule_8_1);
        for (index, row) in draft.schedule_9.rows.iter().enumerate() {
            let prefix = format!("schedule_9_row_{}", index + 1);
            insert_text(&mut envelope, &format!("{prefix}_year"), &row.year);
            for (suffix, value) in [
                ("normal_income_tax", &row.normal_income_tax),
                ("mcit", &row.mcit),
                ("excess_mcit", &row.excess_mcit),
                ("applied_previous_years", &row.applied_previous_years),
                ("expired", &row.expired),
                ("applied_current_year", &row.applied_current_year),
                ("balance", &row.balance),
            ] {
                insert_optional_whole(&mut envelope, &format!("{prefix}_{suffix}"), value);
            }
        }
        insert_optional_whole(
            &mut envelope,
            "schedule_9_item_4_total",
            &draft.schedule_9.item_4_total_applied_current_year,
        );
        map_regime_schedule(&mut envelope, "schedule_10", &draft.schedule_10.items);
        for (index, description) in draft.schedule_10.descriptions.iter().enumerate() {
            insert_text(
                &mut envelope,
                &format!("schedule_10_description_{}", index + 1),
                description,
            );
        }

        map_signatures_and_payment(&mut envelope, draft);
        map_attachment_boundary(&mut envelope, draft);

        envelope.validation = draft
            .validate()
            .into_iter()
            .map(|(field_path, message)| RenderValidationMessage {
                field_path,
                code: "invalid".to_string(),
                message,
                severity: RenderValidationSeverity::Error,
                rule_version: "1702mx-2018c-domain-v1".to_string(),
            })
            .collect();
        envelope
    }
}

fn map_part_two(envelope: &mut RenderEnvelopeV1, draft: &Form1702MXDraft) {
    let part = &draft.part_ii;
    for (key, value) in [
        ("item_14", part.item_14_total_tax_due_or_overpayment),
        ("item_15", part.item_15_total_tax_credits),
        ("item_16", part.item_16_net_tax_payable_or_overpayment),
    ] {
        insert_whole(envelope, key, value);
    }
    for (key, value) in [
        ("item_17", &part.item_17_surcharge),
        ("item_18", &part.item_18_interest),
        ("item_19", &part.item_19_compromise),
        ("item_20", &part.item_20_total_penalties),
        ("item_21", &part.item_21_total_amount_payable_or_overpayment),
    ] {
        insert_optional_whole(envelope, key, value);
    }
    insert_text(
        envelope,
        "overpayment_disposition",
        match part.overpayment_disposition {
            None => "",
            Some(Form1702MXOverpaymentDisposition::Refund) => "refund",
            Some(Form1702MXOverpaymentDisposition::TaxCreditCertificate) => "tcc",
            Some(Form1702MXOverpaymentDisposition::CarryOver) => "carry_over",
        },
    );
}

fn map_relief_basis(envelope: &mut RenderEnvelopeV1, draft: &Form1702MXDraft) {
    insert_bool(
        envelope,
        "relief_single_activity",
        draft.relief_basis.instruction_single_activity,
    );
    insert_bool(
        envelope,
        "relief_multiple_activities",
        draft.relief_basis.instruction_multiple_activities,
    );
    insert_optional_percent(
        envelope,
        "relief_special_tax_rate",
        &draft.relief_basis.special_tax_rate,
    );
}

fn map_regime_schedule(
    envelope: &mut RenderEnvelopeV1,
    prefix: &str,
    rows: &[Form1702MXRegimeAmounts],
) {
    for (index, row) in rows.iter().enumerate() {
        map_regime_amounts(envelope, &format!("{prefix}_item_{}", index + 1), row);
    }
}

fn map_regime_amounts(
    envelope: &mut RenderEnvelopeV1,
    prefix: &str,
    values: &Form1702MXRegimeAmounts,
) {
    for (suffix, value) in [
        ("exempt", &values.exempt),
        ("special", &values.special),
        ("regular", &values.regular),
        ("total", &values.total),
    ] {
        insert_optional_whole(envelope, &format!("{prefix}_{suffix}"), value);
    }
}

fn map_nolco_computation(
    envelope: &mut RenderEnvelopeV1,
    prefix: &str,
    values: &bir_core::forms::form_1702mx::Form1702MXNolcoComputation,
) {
    for (suffix, value) in [
        ("item_1", &values.item_1_gross_income),
        ("item_2", &values.item_2_ordinary_itemized_deductions),
        ("item_3", &values.item_3_net_operating_loss),
    ] {
        insert_optional_whole(envelope, &format!("{prefix}_{suffix}"), value);
    }
}

fn map_nolco_table(
    envelope: &mut RenderEnvelopeV1,
    prefix: &str,
    table: &bir_core::forms::form_1702mx::Form1702MXNolcoTable,
) {
    for (index, row) in table.rows.iter().enumerate() {
        let row_prefix = format!("{prefix}_row_{}", index + 4);
        insert_text(envelope, &format!("{row_prefix}_year"), &row.year_incurred);
        for (suffix, value) in [
            ("amount", &row.amount),
            ("applied_previous_years", &row.applied_previous_years),
            ("expired", &row.expired),
            ("applied_current_year", &row.applied_current_year),
            ("unapplied", &row.unapplied),
        ] {
            insert_optional_whole(envelope, &format!("{row_prefix}_{suffix}"), value);
        }
    }
    insert_optional_whole(
        envelope,
        &format!("{prefix}_item_8_total"),
        &table.item_8_total_applied_current_year,
    );
}

fn map_signatures_and_payment(envelope: &mut RenderEnvelopeV1, draft: &Form1702MXDraft) {
    for (key, value) in [
        (
            "authorized_representative",
            draft.authorized_representative.as_str(),
        ),
        ("treasurer", draft.treasurer.as_str()),
        (
            "number_of_attachments",
            draft.number_of_attachments.as_str(),
        ),
        ("president_title", draft.president_title.as_str()),
        ("president_tin", draft.president_tin.as_str()),
        ("treasurer_title", draft.treasurer_title.as_str()),
        ("treasurer_tin", draft.treasurer_tin.as_str()),
    ] {
        insert_text(envelope, key, value);
    }
    for (index, row) in draft.payment_details.iter().enumerate() {
        let prefix = format!("payment_{}", index + 23);
        insert_text(envelope, &format!("{prefix}_particulars"), &row.particulars);
        insert_text(envelope, &format!("{prefix}_drawee"), &row.drawee);
        insert_text(envelope, &format!("{prefix}_number"), &row.number);
        insert_text(
            envelope,
            &format!("{prefix}_date_or_amount"),
            &row.date_or_amount,
        );
    }
}

fn map_attachment_boundary(envelope: &mut RenderEnvelopeV1, draft: &Form1702MXDraft) {
    let attachment = &draft.mandatory_attachment;
    let has_values = !attachment.current_index.is_empty()
        || !attachment.total_count.is_empty()
        || attachment.exempt_activity
        || attachment.special_rate_activity
        || !attachment.schedule_a_effectivity_from.is_empty()
        || !attachment.schedule_a_effectivity_until.is_empty()
        || !attachment.schedule_d_other_description.is_empty()
        || !attachment.schedule_f_year.is_empty()
        || attachment
            .descriptions_20_to_24
            .iter()
            .any(|value| !value.is_empty());
    insert_text(
        envelope,
        "mandatory_attachment_document_kind",
        "separate_two_page_conditional_companion",
    );
    insert_text(
        envelope,
        "mandatory_attachment_source_sha256",
        "36c02d4c84919d2e5b94cd31b339490019be80afa622f5681ce252c8ec3dec26",
    );
    insert_integer(envelope, "mandatory_attachment_page_count", 2);
    insert_bool(envelope, "mandatory_attachment_transport_supported", false);
    insert_bool(envelope, "mandatory_attachment_has_values", has_values);
}

fn insert_text(envelope: &mut RenderEnvelopeV1, key: &str, value: impl Into<String>) {
    envelope
        .fields
        .insert(key.to_string(), RenderValue::Text(value.into()));
}

fn insert_bool(envelope: &mut RenderEnvelopeV1, key: &str, value: bool) {
    envelope
        .fields
        .insert(key.to_string(), RenderValue::Boolean(value));
}

fn insert_integer(envelope: &mut RenderEnvelopeV1, key: &str, value: i64) {
    envelope
        .fields
        .insert(key.to_string(), RenderValue::Integer(value));
}

fn insert_whole(envelope: &mut RenderEnvelopeV1, key: &str, value: WholePeso) {
    insert_integer(envelope, key, value.0);
}

fn insert_optional_whole(envelope: &mut RenderEnvelopeV1, key: &str, value: &WholePesoInput) {
    if let Some(value) = value.amount {
        insert_whole(envelope, key, value);
    }
}

fn insert_optional_percent(envelope: &mut RenderEnvelopeV1, key: &str, value: &PercentInput) {
    if let Some(hundredths) = value.hundredths {
        envelope.fields.insert(
            key.to_string(),
            RenderValue::Decimal(f64::from(hundredths) / 100.0),
        );
    }
}

fn fixtures() -> Result<Vec<RenderContractFixture>, RenderProviderError> {
    Ok(vec![
        fixture(
            "1702mx-minimum.json",
            RenderFixtureKind::Minimum,
            true,
            minimum_fixture()?,
        ),
        fixture(
            "1702mx-normal.json",
            RenderFixtureKind::Normal,
            true,
            normal_fixture()?,
        ),
        fixture(
            "1702mx-long-values.json",
            RenderFixtureKind::LongValues,
            true,
            long_values_fixture()?,
        ),
        fixture(
            "1702mx-validation-edge.json",
            RenderFixtureKind::ValidationEdge,
            false,
            validation_edge_fixture()?,
        ),
        fixture(
            "1702mx-fixed-capacity.json",
            RenderFixtureKind::ScheduleCapacity,
            true,
            capacity_fixture()?,
        ),
    ])
}

fn fixture(
    file_name: &'static str,
    kind: RenderFixtureKind,
    expected_form_valid: bool,
    draft: Form1702MXDraft,
) -> RenderContractFixture {
    RenderContractFixture {
        file_name,
        kind,
        expected_form_valid,
        envelope: RenderEnvelopeV1::from(&draft),
    }
}

fn profile() -> Result<TaxpayerProfile, RenderProviderError> {
    serde_json::from_value(json!({
        "id": null,
        "full_name": "REVIEWED MIXED INCOME CORPORATION",
        "tin": {
            "segment1": "123",
            "segment2": "456",
            "segment3": "789",
            "branch": "00000"
        },
        "rdo_code": "018",
        "line_of_business": "SOFTWARE DEVELOPMENT",
        "registered_address": "53 SANTOL EXTENSION, NEW CABALAN, OLONGAPO CITY",
        "zip_code": "2200",
        "phone": "09123456789",
        "email": "renderer.1702mx@example.com",
        "default_form_type": "1702MXv2018C",
        "taxpayer_type": "Corporation",
        "tax_classification": "Corporation",
        "is_vat_registered": false,
        "eopt_tier": "Medium",
        "withholding_obligations": [],
        "excise_tax_liabilities": [],
        "atc_codes": [],
        "business_start_date": null,
        "birth_date": null,
        "registration_activity_status": "Active",
        "is_dormant_entity": false,
        "is_government_withholding_entity": false,
        "is_gpp_partner": false,
        "is_top_withholding_agent": false,
        "income_tax_elections": { "elections": {} },
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
    }))
    .map_err(|error| RenderProviderError::Fixture(error.to_string()))
}

fn base_fixture() -> Result<Form1702MXDraft, RenderProviderError> {
    let mut draft = Form1702MXDraft::new_from_profile(&profile()?, 2025);
    draft.atc.mcit_selected = true;
    draft.deduction_method = Form1702MXDeductionMethod::Itemized;
    draft.relief_basis.instruction_single_activity = true;
    draft.incorporation_date = "01/15/2018".to_string();
    draft.number_of_attachments = "00".to_string();
    draft.status = FilingStatus::Draft;
    draft.recompute();
    Ok(draft)
}

fn minimum_fixture() -> Result<Form1702MXDraft, RenderProviderError> {
    base_fixture()
}

fn normal_fixture() -> Result<Form1702MXDraft, RenderProviderError> {
    let mut draft = base_fixture()?;
    draft.is_amended = true;
    draft.atc.other_selected = true;
    draft.atc.other_code = "IC 057".to_string();
    draft.schedule_2.item_14_special_rate = PercentInput::from_hundredths(1_000);
    draft.schedule_2.item_14_regular_rate = PercentInput::from_hundredths(2_500);
    set_regime(&mut draft.schedule_2.items[0], 100_000, 200_000, 300_000);
    set_regime(&mut draft.schedule_2.items[1], 5_000, 10_000, 15_000);
    set_regime(&mut draft.schedule_2.items[3], 20_000, 30_000, 40_000);
    set_regime(&mut draft.schedule_2.items[5], 2_000, 3_000, 4_000);
    set_regime(&mut draft.schedule_2.items[7], 10_000, 20_000, 30_000);
    set_regime(&mut draft.schedule_2.items[8], 1_000, 2_000, 3_000);
    set_regime(&mut draft.schedule_2.items[9], 500, 1_000, 1_500);
    set_regime(&mut draft.schedule_2.items[14], 0, 12_500, 65_000);
    set_regime(&mut draft.schedule_2.items[15], 0, 500, 1_000);
    set_regime(&mut draft.schedule_3.items_20_to_33[0], 0, 1_000, 2_000);
    set_regime(&mut draft.schedule_3.items_20_to_33[4], 0, 250, 500);
    draft.schedule_3.item_30_description = "OTHER REVIEWED CREDIT".to_string();
    set_regime(&mut draft.schedule_3.items_20_to_33[10], 0, 100, 200);
    draft.schedule_3.item_31_description = "SECOND REVIEWED CREDIT".to_string();
    set_regime(&mut draft.schedule_3.items_20_to_33[11], 0, 50, 100);
    draft.part_ii.item_17_surcharge = amount(1_000);
    draft.part_ii.item_18_interest = amount(500);
    draft.part_ii.item_19_compromise = amount(250);
    draft.authorized_representative = "REVIEWED PRESIDENT".to_string();
    draft.treasurer = "REVIEWED TREASURER".to_string();
    draft.president_title = "PRESIDENT".to_string();
    draft.president_tin = "12345678900000".to_string();
    draft.treasurer_title = "TREASURER".to_string();
    draft.treasurer_tin = "98765432100000".to_string();
    draft.payment_details[0].particulars = "CASH/BANK DEBIT MEMO".to_string();
    draft.payment_details[0].drawee = "AUTHORIZED AGENT BANK".to_string();
    draft.payment_details[0].number = "BDM-1702MX-0001".to_string();
    draft.payment_details[0].date_or_amount = "04/15/2026 / 75,000".to_string();
    draft.recompute();
    Ok(draft)
}

fn long_values_fixture() -> Result<Form1702MXDraft, RenderProviderError> {
    let mut draft = normal_fixture()?;
    draft.taxpayer_name = "A DELIBERATELY LONG REGISTERED MIXED INCOME CORPORATION NAME THAT MUST SWITCH FROM OFFICIAL COMB CELLS TO A REVIEWED PLAIN BOX WITHOUT LOSING CHARACTERS INC".to_string();
    draft.registered_address = "UNIT 1201, A DELIBERATELY LONG REGISTERED ADDRESS THAT MUST REMAIN COMPLETE IN THE SEMANTIC HTML DOCUMENT, BUILDING ONE, CENTRAL BUSINESS DISTRICT, NEW CABALAN, OLONGAPO CITY, ZAMBALES, PHILIPPINES".to_string();
    draft.email =
        "long.mixed.income.corporate.renderer.verification.address@example.test".to_string();
    draft.schedule_3.item_30_description =
        "A DELIBERATELY LONG OTHER TAX CREDIT OR PAYMENT DESCRIPTION THAT MUST REMAIN COMPLETE"
            .to_string();
    draft.schedule_5.other_descriptions_17d_to_17i[0] =
        "A DELIBERATELY LONG ORDINARY ITEMIZED DEDUCTION DESCRIPTION THAT MUST REMAIN COMPLETE"
            .to_string();
    draft.schedule_6.rows[0].description =
        "A DELIBERATELY LONG SPECIAL ALLOWABLE ITEMIZED DEDUCTION DESCRIPTION".to_string();
    draft.schedule_6.rows[0].legal_basis =
        "REVIEWED SPECIAL LAW SECTION WITH A LONG CITATION".to_string();
    draft.schedule_10.descriptions[1] =
        "A DELIBERATELY LONG NON-DEDUCTIBLE EXPENSE DESCRIPTION".to_string();
    draft.payment_details[0].drawee =
        "AUTHORIZED AGENT BANK WITH A DELIBERATELY LONG REGISTERED BRANCH NAME".to_string();
    draft.recompute();
    Ok(draft)
}

fn validation_edge_fixture() -> Result<Form1702MXDraft, RenderProviderError> {
    let mut draft = base_fixture()?;
    draft.tin.clear();
    draft.rdo_code.clear();
    draft.taxpayer_name.clear();
    draft.registered_address.clear();
    draft.atc.mcit_selected = false;
    draft.deduction_method = Form1702MXDeductionMethod::Unresolved;
    draft.relief_basis.instruction_single_activity = true;
    draft.relief_basis.instruction_multiple_activities = true;
    draft.recompute();
    Ok(draft)
}

fn capacity_fixture() -> Result<Form1702MXDraft, RenderProviderError> {
    let mut draft = normal_fixture()?;
    for (index, row) in draft.schedule_5.amounts.iter_mut().enumerate() {
        set_regime(
            row,
            (index as i64 + 1) * 100,
            (index as i64 + 1) * 200,
            (index as i64 + 1) * 300,
        );
    }
    for (index, row) in draft.schedule_6.rows.iter_mut().enumerate() {
        row.description = format!("SPECIAL DEDUCTION ROW {}", index + 1);
        row.legal_basis = format!("REVIEWED LEGAL BASIS {}", index + 1);
        set_regime(
            &mut row.amounts,
            (index as i64 + 1) * 400,
            (index as i64 + 1) * 500,
            (index as i64 + 1) * 600,
        );
    }
    draft.regular_nolco.item_1_gross_income = amount(50_000);
    draft.regular_nolco.item_2_ordinary_itemized_deductions = amount(60_000);
    for (index, row) in draft.schedule_7_1.rows.iter_mut().enumerate() {
        row.year_incurred = (2020 + index).to_string();
        row.amount = amount(10_000 + index as i64 * 1_000);
        row.applied_previous_years = amount(1_000);
        row.expired = amount(500);
        row.applied_current_year = amount(750);
    }
    draft.special_nolco.item_1_gross_income = amount(30_000);
    draft.special_nolco.item_2_ordinary_itemized_deductions = amount(40_000);
    for (index, row) in draft.schedule_8_1.rows.iter_mut().enumerate() {
        row.year_incurred = (2020 + index).to_string();
        row.amount = amount(8_000 + index as i64 * 1_000);
        row.applied_previous_years = amount(800);
        row.expired = amount(400);
        row.applied_current_year = amount(600);
    }
    for (index, row) in draft.schedule_9.rows.iter_mut().enumerate() {
        row.year = (2022 + index).to_string();
        row.normal_income_tax = amount(20_000 + index as i64 * 1_000);
        row.mcit = amount(25_000 + index as i64 * 1_000);
        row.applied_previous_years = amount(500);
        row.expired = amount(250);
        row.applied_current_year = amount(750);
    }
    for (index, row) in draft.schedule_10.items.iter_mut().enumerate() {
        draft.schedule_10.descriptions[index] = format!("RECONCILIATION ITEM {}", index + 1);
        set_regime(
            row,
            (index as i64 + 1) * 700,
            (index as i64 + 1) * 800,
            (index as i64 + 1) * 900,
        );
    }
    draft.recompute();
    Ok(draft)
}

fn amount(value: i64) -> WholePesoInput {
    WholePesoInput::from_amount(WholePeso(value))
}

fn set_regime(row: &mut Form1702MXRegimeAmounts, exempt: i64, special: i64, regular: i64) {
    row.exempt = amount(exempt);
    row.special = amount(special);
    row.regular = amount(regular);
}

fn generated_artifacts() -> Result<Vec<GeneratedContractArtifact>, RenderProviderError> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_keeps_base_return_and_mandatory_attachment_distinct() {
        let draft = normal_fixture().expect("normal fixture");
        let envelope = RenderEnvelopeV1::from(&draft);

        assert_eq!(PROVIDER.page_geometry().unwrap(), RenderPageGeometry::LEGAL);
        assert_eq!(PROVIDER.expected_page_count(&envelope).unwrap(), 4);
        assert_eq!(envelope.form.code, "1702MX");
        assert_eq!(envelope.form.version, "2018C");
        assert_eq!(
            envelope.fields["mandatory_attachment_document_kind"],
            RenderValue::Text("separate_two_page_conditional_companion".to_string())
        );
        assert_eq!(
            envelope.fields["mandatory_attachment_page_count"],
            RenderValue::Integer(2)
        );
        assert_eq!(
            envelope.fields["mandatory_attachment_transport_supported"],
            RenderValue::Boolean(false)
        );
        assert!(envelope.schedules.is_empty());
    }

    #[test]
    fn adapter_preserves_blank_inputs_and_rust_owned_whole_pesos() {
        let minimum = RenderEnvelopeV1::from(&minimum_fixture().expect("minimum fixture"));
        assert!(!minimum.fields.contains_key("schedule_5_item_1_exempt"));

        let normal = normal_fixture().expect("normal fixture");
        let envelope = RenderEnvelopeV1::from(&normal);
        assert_eq!(
            envelope.fields["item_14"],
            RenderValue::Integer(normal.part_ii.item_14_total_tax_due_or_overpayment.0)
        );
        assert_eq!(
            envelope.fields["schedule_2_item_1_regular"],
            RenderValue::Integer(300_000)
        );
    }

    #[test]
    fn discrete_artwork_provenance_matches_verified_official_xobjects() {
        let assets = runtime_discrete_assets();
        assert_eq!(assets.len(), 5);

        let seal = &assets[0];
        assert_eq!(seal["asset"], json!("government_seal"));
        assert_eq!(seal["source_pdf_object_id"], json!([42, 0]));
        assert_eq!(seal["source_pixel_dimensions"], json!([119, 102]));
        assert_eq!(seal["source_channels_equal"], json!(true));
        assert_eq!(
            seal["source_ctm_points"],
            json!([33.682, 0.0, 0.0, 28.152, 228.52, 878.88])
        );
        assert_eq!(
            seal["derived_png_sha256"],
            json!("92b7a3fd81ee9db5705482563925d79842d1e961a5a0a931fc6d838ec7a1402e")
        );

        for (asset, expected) in assets[1..].iter().zip([
            (
                1,
                43,
                "1702-MX 01/18ENCS P1",
                551,
                "58fe10021bc79bf3c031e1dcecb264a3d643f4fa8a5e54fff0e8279106f2fde2",
            ),
            (
                2,
                50,
                "1702-MX 01/18ENCS P2",
                561,
                "38f0943e4710bb7c87585e94cbf82e6b4a338379b0d4f95edeb25128364dae25",
            ),
            (
                3,
                53,
                "1702-MX 01/18ENCS P3",
                555,
                "447a3cd3ae20ed9fcf9527180fb539ef9fbcf5dcd2aeebd6aa272ccd7d4f7602",
            ),
            (
                4,
                56,
                "1702-MX 01/18ENCS P4",
                547,
                "4dce7fd23395f73f138612cade05ad4bf707ddd597293eedeac889cd3eda7a2d",
            ),
        ]) {
            let (page, object, payload, black_modules, matrix_hash) = expected;
            assert_eq!(asset["source_page"], json!(page));
            assert_eq!(asset["source_pdf_object_id"], json!([object, 0]));
            assert_eq!(asset["source_pixel_dimensions"], json!([241, 72]));
            assert_eq!(asset["source_active_pixel_bounds"], json!([0, 0, 240, 72]));
            assert_eq!(asset["decoded_payload"], json!(payload));
            assert_eq!(asset["logical_black_modules"], json!(black_modules));
            assert_eq!(asset["logical_dimensions"], json!([120, 8]));
            assert_eq!(asset["logical_matrix_sha256"], json!(matrix_hash));
            assert_eq!(asset["source_module_scale_pixels"], json!([2, 9]));
            assert_eq!(
                asset["source_padding_pixels"],
                json!({"bottom": 0, "right": 1})
            );
            assert_eq!(asset["source_padding_module_units"], json!({"right": 0.5}));
            assert_eq!(asset["encoder_proof"]["module_differences"], json!(0));
        }
    }

    #[test]
    fn fixtures_cover_every_required_four_page_state() {
        let fixtures = fixtures().expect("fixtures");
        for required in [
            RenderFixtureKind::Minimum,
            RenderFixtureKind::Normal,
            RenderFixtureKind::LongValues,
            RenderFixtureKind::ValidationEdge,
            RenderFixtureKind::ScheduleCapacity,
        ] {
            assert!(fixtures.iter().any(|fixture| fixture.kind == required));
        }
        for fixture in &fixtures {
            assert_eq!(PROVIDER.expected_page_count(&fixture.envelope).unwrap(), 4);
            assert_eq!(
                fixture.expected_form_valid,
                fixture.envelope.validation.is_empty(),
                "{} validity mismatch: {:?}",
                fixture.file_name,
                fixture.envelope.validation
            );
        }
    }
}
