//! Exact April 2024 semantic HTML provider.
//!
//! The official form and reviewed editable-save pair establish exactly two
//! rows for Schedules 1, 3, and 4. Additional sheets are mentioned by the
//! official labels, but their geometry and transport contract are not present
//! in the source pack. Consequently this provider is deliberately fixed at two
//! pages and refuses to invent continuation pages.

use bir_core::{
    forms::{
        form_2550q::{
            Form2550QAdvanceVatRow, Form2550QCapitalGoodRow, Form2550QCreditableVatRow,
            Form2550QDate, Form2550QDraft, Form2550QFilingBasis, Form2550QTaxpayerClassification,
        },
        FormValidator,
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
        file_name: "2550q-2024-page-1.png",
        sha256: "abfa7be4cb205a9a7488e865279c9b83b50b8b581b6019318c5a98646b74cceb",
    },
    VisualReferencePage {
        page: 2,
        file_name: "2550q-2024-page-2.png",
        sha256: "6bcf45b6780789601b4a6a659cad12ef8777f3a61179ee82d9f93c62c85b97f4",
    },
];

// Pinned by scripts/prepare_chromium_reference.mjs from the same official
// PDF bytes; see references/2550q-2024-chromium-source.json
// for the full pdf -> svg -> png provenance chain.
const CHROMIUM_REFERENCES: super::ChromiumReferenceSet = super::ChromiumReferenceSet {
    generator: super::ChromiumReferenceGenerator {
        pdftocairo_version: "26.07.0",
        playwright_version: "1.58.2",
        chromium_version: "145.0.7632.6",
    },
    pages: &[
        super::ChromiumVisualReference {
            page: 1,
            file_name: "2550q-2024-page-1-chromium.png",
            sha256: "22a50febd1b25e3ced56f4869ba041d840ed4d6eaa5b764870305c102134a57e",
            vector_svg_sha256: "3614563af7db10835468b8dde1aceca358a531268530796a44374a49cc2748a7",
            noise_floor_changed_pixels: 71_122,
        },
        super::ChromiumVisualReference {
            page: 2,
            file_name: "2550q-2024-page-2-chromium.png",
            sha256: "6481138b3fc7b42b677d8139299ffc48fe9822ae2fff59d0fc6b3fd240dde698",
            vector_svg_sha256: "39567d0cc880251651423f138b1db38d42f83f4019e3ff09e8e89d5a596bdfbd",
            noise_floor_changed_pixels: 83_946,
        },
    ],
};

pub(super) const PROVIDER: RenderFormProvider = RenderFormProvider {
    code: "2550Q",
    revision: "2024",
    form_id: "2550Qv2024",
    title: "Quarterly Value-Added Tax Return",
    page_width_pt: RenderPageGeometry::FOURTEEN_INCH.width_points,
    page_height_pt: RenderPageGeometry::FOURTEEN_INCH.height_points,
    expected_base_page_count: 2,
    schedules: &[],
    visual_fixture_file_name: "2550q-normal.json",
    visual_fixture_sha256: "9869638cf5f9dd1fcb4bc83f87d1ae8adfef274905ec8e462358c238e07abf13",
    official_source: "https://bir-cdn.bir.gov.ph/BIR/pdf/2550Q%20%20April%202024%20ENCS_Final.pdf",
    official_source_sha256: "18eb16925010fdda820cef958221ba2c0d073066efa93a898113e39b31135a25",
    reference_dpi: 144,
    reference_width_px: 1_224,
    reference_height_px: 2_016,
    visual_reference_pages: VISUAL_REFERENCE_PAGES,
    chromium_references: Some(&CHROMIUM_REFERENCES),
    machine_readable_artwork: super::MachineReadableArtworkEvidence::Present,
    runtime_discrete_assets,
    fixtures,
    generated_artifacts,
};

fn runtime_discrete_assets() -> Vec<serde_json::Value> {
    vec![
        json!({
            "asset": "government_seal",
            "bits_per_component": 8,
            "color_space": "DeviceRGB",
            "derived_grayscale_pixel_sha256": "63ad9322d1e235ff6cc791e247f44f519f27e7e6932d95873146135138a7d311",
            "derived_png_sha256": "e4259300bed379aa06e686a31638b1943232ade6476a72b115e8ed357890b6d5",
            "embedded_as": "lossless_grayscale_collapse_of_equal_rgb_channels",
            "embedded_color_space": "DeviceGray",
            "embedded_in": "packages/form-renderer/src/forms/assets/2550q-seal.png",
            "source_bbox_top_left_points": [230.8, 17.2, 31.1, 29.7],
            "source_channels_equal": true,
            "source_ctm_points": [31.1, 0.0, 0.0, 29.7, 230.8, 961.1],
            "source_decoded_rgb_sha256": "991fbb7de591a700d380e7030767a429be31a52d6d3ab292de7fda7e51e165e1",
            "source_page": 1,
            "source_pdf_object_id": [37, 0],
            "source_pixel_dimensions": [86, 82],
            "source_png_sha256": "a37e501e37914f52e6598f8b2e7a182213e5fa9da532760dd50d1c9857e2c310",
            "source_stream_sha256": "942805bf03921db3320db705830f31d884e1c3bdccd748cbea0967a5f66ac5b8",
            "treatment": "lossless extraction of the exact official PDF image XObject; equal RGB channels collapsed to one native grayscale channel without crop, resampling, recoloring, thresholding, or substitution"
        }),
        json!({
            "asset": "static_form_pdf417_page_1",
            "caption_bbox_top_left_points": [509.4, 99.656, 80.408, 7.4],
            "caption_font": "Arial",
            "caption_font_size_points": 8.0,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "2550Q 04/24ENCS P1",
            "decoded_payload": "2550Q 04/24ENCS P1",
            "decoder_evidence": [
                {"decoder": "ZXing-C++ 3.1.0", "payload": "2550Q 04/24ENCS P1", "symbology": "PDF417"}
            ],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official2550QAssets.ts",
            "encoder_proof": {
                "columns": 3,
                "encoding": "ISO-8859-1",
                "error_correction_level": 2,
                "implementation": "pdf417gen 0.8.1",
                "module_differences": 0,
                "rows": 7
            },
            "logical_black_modules": 478,
            "logical_dimensions": [120, 7],
            "logical_matrix_sha256": "5a8d7f411686eb62d9d45b25a0430703a087068d43cd34cf6e63c35d6fce435f",
            "logical_path_sha256": "59064ade808ae9a05497ebe5794e15f4da1b77f9c61009ca2ba85b5b2716625f",
            "source_active_bbox_top_left_points": [434.3, 51.3, 156.2, 46.7],
            "source_active_pixel_bounds": [0, 0, 240, 63],
            "source_bbox_top_left_points": [434.3, 51.3, 156.2, 46.7],
            "source_ctm_points": [156.2, 0.0, 0.0, 46.7, 434.3, 910.0],
            "source_decoded_rgb_sha256": "8478c0f20e3cbae53d993bab29cc45156e74d30fd221ddb2544218ad4596da03",
            "source_page": 1,
            "source_pdf_object_id": [38, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 0, "right": 0},
            "source_pixel_dimensions": [240, 63],
            "source_png_sha256": "34d7cac99a6e212ddcf3aed400de37691a40bf7e9917aefbf71f49866ef1ef9c",
            "source_stream_sha256": "1698de046f9ad0535ed16a57fa1dbae5bb2d4003e7650ea081df69a2aae7ce2c",
            "symbology": "PDF417"
        }),
        json!({
            "asset": "static_form_pdf417_page_2",
            "caption_bbox_top_left_points": [505.5, 70.756, 80.408, 7.4],
            "caption_font": "Arial",
            "caption_font_size_points": 8.0,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "2550Q 04/24ENCS P2",
            "decoded_payload": "2550Q 04/24ENCS P2",
            "decoder_evidence": [
                {"decoder": "ZXing-C++ 3.1.0", "payload": "2550Q 04/24ENCS P2", "symbology": "PDF417"}
            ],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official2550QAssets.ts",
            "encoder_proof": {
                "columns": 3,
                "encoding": "ISO-8859-1",
                "error_correction_level": 2,
                "implementation": "pdf417gen 0.8.1",
                "module_differences": 0,
                "rows": 7
            },
            "logical_black_modules": 479,
            "logical_dimensions": [120, 7],
            "logical_matrix_sha256": "95368d60c40a9767d65c09836f3c36152adfd42cd70c740dea33c19e75fe86e6",
            "logical_path_sha256": "4b76223a10321504a7fde4e652b651f06418779c085b1e31c83fcc05082ee810",
            "source_active_bbox_top_left_points": [433.9, 24.4, 153.9, 45.1],
            "source_active_pixel_bounds": [0, 0, 240, 63],
            "source_bbox_top_left_points": [433.9, 24.4, 153.9, 45.1],
            "source_ctm_points": [153.9, 0.0, 0.0, 45.1, 433.9, 938.5],
            "source_decoded_rgb_sha256": "d70e5dc587ea67a573141866e6d96c8d8b65465dd14e9f4c44dee3211c9cf997",
            "source_page": 2,
            "source_pdf_object_id": [47, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 0, "right": 0},
            "source_pixel_dimensions": [240, 63],
            "source_png_sha256": "883cf46213814de3670edf662411b3c88d334188f05877e175e77fa19ba02b1a",
            "source_stream_sha256": "34d016e1171a32c0446a10e1cbe924ef9db3d0cba0faac70766011c825556c61",
            "symbology": "PDF417"
        }),
    ]
}

impl From<&Form2550QDraft> for RenderEnvelopeV1 {
    fn from(draft: &Form2550QDraft) -> Self {
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
                month: Some(draft.year_end_month),
                quarter: draft.quarter_number(),
                label: format!(
                    "Q{} year ended {:02}/{}",
                    draft.quarter_number().unwrap_or_default(),
                    draft.year_end_month,
                    draft.taxable_year
                ),
            },
        );

        insert_text(
            &mut envelope,
            "filing_basis",
            match draft.filing_basis {
                Form2550QFilingBasis::Calendar => "calendar",
                Form2550QFilingBasis::Fiscal => "fiscal",
            },
        );
        insert_integer(
            &mut envelope,
            "year_end_month",
            i64::from(draft.year_end_month),
        );
        if let Some(quarter) = draft.quarter_number() {
            insert_integer(&mut envelope, "quarter", i64::from(quarter));
        }
        insert_date(
            &mut envelope,
            "return_period_from",
            draft.return_period_from,
        );
        insert_date(&mut envelope, "return_period_to", draft.return_period_to);
        insert_bool(&mut envelope, "is_amended", draft.is_amended);
        insert_bool(
            &mut envelope,
            "is_short_period_return",
            draft.is_short_period_return,
        );
        if let Some(classification) = draft.taxpayer_classification {
            insert_text(
                &mut envelope,
                "taxpayer_classification",
                classification_key(classification),
            );
        }
        insert_bool(
            &mut envelope,
            "is_availing_tax_relief",
            draft.is_availing_tax_relief,
        );
        insert_text(
            &mut envelope,
            "tax_relief_details",
            &draft.tax_relief_details,
        );

        map_part_two(&mut envelope, draft);
        map_part_four(&mut envelope, draft);
        map_schedule_one(&mut envelope, draft);
        map_schedule_two(&mut envelope, draft);
        map_schedule_three(&mut envelope, draft);
        map_schedule_four(&mut envelope, draft);
        map_local_print_fields(&mut envelope, draft);

        envelope.validation = draft
            .validate()
            .into_iter()
            .map(|(field_path, message)| RenderValidationMessage {
                field_path,
                code: "invalid".to_string(),
                message,
                severity: RenderValidationSeverity::Error,
                rule_version: "2550q-2024-domain-v1".to_string(),
            })
            .collect();
        envelope
    }
}

fn map_part_two(envelope: &mut RenderEnvelopeV1, draft: &Form2550QDraft) {
    let part = &draft.part_ii;
    insert_optional_decimal(envelope, "item_15", part.item_15_net_vat_payable_or_excess);
    insert_optional_decimal(envelope, "item_16", part.item_16_creditable_vat_withheld);
    insert_optional_decimal(envelope, "item_17", part.item_17_advance_vat_payments);
    insert_optional_decimal(envelope, "item_18", part.item_18_paid_on_previous_return);
    insert_text(envelope, "item_19_description", &part.item_19_description);
    insert_optional_decimal(envelope, "item_19", part.item_19_other_credit_or_payment);
    insert_optional_decimal(envelope, "item_20", part.item_20_total_credits_or_payments);
    insert_optional_decimal(
        envelope,
        "item_21",
        part.item_21_tax_payable_or_excess_credits,
    );
    insert_optional_decimal(envelope, "item_22", part.item_22_surcharge);
    insert_optional_decimal(envelope, "item_23", part.item_23_interest);
    insert_optional_decimal(envelope, "item_24", part.item_24_compromise);
    insert_optional_decimal(envelope, "item_25", part.item_25_total_penalties);
    insert_optional_decimal(
        envelope,
        "item_26",
        part.item_26_total_amount_payable_or_excess,
    );
}

fn map_part_four(envelope: &mut RenderEnvelopeV1, draft: &Form2550QDraft) {
    let part = &draft.part_iv;
    for (key, value) in [
        ("item_31a", part.item_31a_vatable_sales),
        ("item_31b", part.item_31b_output_tax),
        ("item_32a", part.item_32a_zero_rated_sales),
        ("item_33a", part.item_33a_exempt_sales),
        ("item_34a", part.item_34a_total_sales),
        ("item_34b", part.item_34b_output_tax_due),
        ("item_35b", part.item_35b_less_output_vat_uncollected),
        ("item_36b", part.item_36b_add_output_vat_recovered),
        ("item_37b", part.item_37b_adjusted_output_tax_due),
        ("item_38b", part.item_38b_input_tax_carried),
        ("item_39b", part.item_39b_input_tax_deferred),
        ("item_40b", part.item_40b_transitional_input_tax),
        ("item_41b", part.item_41b_presumptive_input_tax),
        ("item_42b", part.item_42b_other_input_tax),
        ("item_43b", part.item_43b_total_prior_input_tax),
        ("item_44a", part.item_44a_domestic_purchases),
        ("item_44b", part.item_44b_domestic_input_tax),
        ("item_45a", part.item_45a_nonresident_services),
        ("item_45b", part.item_45b_nonresident_service_input_tax),
        ("item_46a", part.item_46a_importations),
        ("item_46b", part.item_46b_import_input_tax),
        ("item_47a", part.item_47a_other_purchases),
        ("item_47b", part.item_47b_other_input_tax),
        ("item_48a", part.item_48a_domestic_purchases_no_input_tax),
        ("item_49a", part.item_49a_vat_exempt_importations),
        ("item_50a", part.item_50a_total_current_purchases),
        ("item_50b", part.item_50b_total_current_input_tax),
        ("item_51b", part.item_51b_total_available_input_tax),
        ("item_52b", part.item_52b_deferred_capital_goods_input_tax),
        (
            "item_53b",
            part.item_53b_input_tax_attributable_to_exempt_sales,
        ),
        ("item_54b", part.item_54b_vat_refund_or_tcc_claimed),
        ("item_55b", part.item_55b_input_vat_on_unpaid_payables),
        ("item_56b", part.item_56b_other_deduction),
        ("item_57b", part.item_57b_total_deductions),
        ("item_58b", part.item_58b_input_vat_on_settled_payables),
        ("item_59b", part.item_59b_adjusted_deductions),
        ("item_60b", part.item_60b_total_allowable_input_tax),
        ("item_61b", part.item_61b_net_vat_payable_or_excess),
    ] {
        insert_optional_decimal(envelope, key, value);
    }
    insert_text(envelope, "item_42_description", &part.item_42_description);
    insert_text(envelope, "item_47_description", &part.item_47_description);
    insert_text(envelope, "item_56_description", &part.item_56_description);
}

fn map_schedule_one(envelope: &mut RenderEnvelopeV1, draft: &Form2550QDraft) {
    for index in 0..2 {
        let empty = Form2550QCapitalGoodRow::default();
        let row = draft.schedule_1.get(index).unwrap_or(&empty);
        let prefix = format!("schedule_1_{}", index + 1);
        insert_date(
            envelope,
            &format!("{prefix}_date"),
            row.purchase_or_import_date,
        );
        insert_text(envelope, &format!("{prefix}_source"), &row.source_code);
        insert_text(envelope, &format!("{prefix}_description"), &row.description);
        insert_optional_decimal(
            envelope,
            &format!("{prefix}_amount"),
            row.purchase_or_import_amount,
        );
        insert_optional_decimal(envelope, &format!("{prefix}_input_tax"), row.input_tax);
        insert_optional_integer(
            envelope,
            &format!("{prefix}_estimated_life_months"),
            row.estimated_life_months,
        );
        insert_optional_integer(
            envelope,
            &format!("{prefix}_recognized_life_months"),
            row.recognized_life_months,
        );
        insert_optional_decimal(
            envelope,
            &format!("{prefix}_allowable_input_tax"),
            row.allowable_input_tax_for_period,
        );
        insert_optional_decimal(
            envelope,
            &format!("{prefix}_balance"),
            row.balance_to_next_period,
        );
    }
}

fn map_schedule_two(envelope: &mut RenderEnvelopeV1, draft: &Form2550QDraft) {
    let schedule = &draft.schedule_2;
    for (key, value) in [
        (
            "schedule_2_direct_input_tax",
            schedule.input_tax_directly_attributable_to_exempt_sales,
        ),
        ("schedule_2_exempt_sales", schedule.vat_exempt_sales),
        (
            "schedule_2_indirect_input_tax",
            schedule.input_tax_not_directly_attributable,
        ),
        ("schedule_2_total_sales", schedule.total_sales),
        ("schedule_2_ratable_input_tax", schedule.ratable_input_tax),
        (
            "schedule_2_total_attributable_input_tax",
            schedule.total_input_tax_attributable_to_exempt_sales,
        ),
    ] {
        insert_optional_decimal(envelope, key, value);
    }
}

fn map_schedule_three(envelope: &mut RenderEnvelopeV1, draft: &Form2550QDraft) {
    for index in 0..2 {
        let empty = Form2550QCreditableVatRow::default();
        let row = draft.schedule_3.get(index).unwrap_or(&empty);
        let prefix = format!("schedule_3_{}", index + 1);
        insert_date(envelope, &format!("{prefix}_from"), row.period_from);
        insert_date(envelope, &format!("{prefix}_to"), row.period_to);
        insert_text(
            envelope,
            &format!("{prefix}_agent"),
            &row.withholding_agent_name,
        );
        insert_optional_decimal(
            envelope,
            &format!("{prefix}_income_payment"),
            row.income_payment,
        );
        insert_optional_decimal(
            envelope,
            &format!("{prefix}_tax_withheld"),
            row.tax_withheld,
        );
    }
    insert_optional_decimal(
        envelope,
        "schedule_3_total_income_payment",
        sum_optional(draft.schedule_3.iter().map(|row| row.income_payment)),
    );
    insert_optional_decimal(
        envelope,
        "schedule_3_total_tax_withheld",
        draft.part_ii.item_16_creditable_vat_withheld,
    );
}

fn map_schedule_four(envelope: &mut RenderEnvelopeV1, draft: &Form2550QDraft) {
    for index in 0..2 {
        let empty = Form2550QAdvanceVatRow::default();
        let row = draft.schedule_4.get(index).unwrap_or(&empty);
        let prefix = format!("schedule_4_{}", index + 1);
        insert_date(envelope, &format!("{prefix}_from"), row.period_from);
        insert_date(envelope, &format!("{prefix}_to"), row.period_to);
        insert_text(envelope, &format!("{prefix}_miller"), &row.miller_name);
        insert_text(envelope, &format!("{prefix}_taxpayer"), &row.taxpayer_name);
        insert_text(
            envelope,
            &format!("{prefix}_receipt"),
            &row.official_receipt_number,
        );
        insert_optional_decimal(envelope, &format!("{prefix}_amount"), row.amount_paid);
    }
    insert_optional_decimal(
        envelope,
        "schedule_4_total_amount",
        draft.part_ii.item_17_advance_vat_payments,
    );
}

fn map_local_print_fields(envelope: &mut RenderEnvelopeV1, draft: &Form2550QDraft) {
    let fields = &draft.local_print_fields;
    for (key, value) in [
        (
            "signature_taxpayer_or_representative",
            fields.taxpayer_or_authorized_representative.as_str(),
        ),
        (
            "signature_representative_title",
            fields.representative_title.as_str(),
        ),
        (
            "signature_non_individual_officer",
            fields.non_individual_authorized_officer.as_str(),
        ),
        (
            "tax_agent_accreditation_or_roll_number",
            fields.tax_agent_accreditation_or_roll_number.as_str(),
        ),
        (
            "tax_agent_date_of_issue",
            fields.tax_agent_date_of_issue.as_str(),
        ),
        (
            "tax_agent_date_of_expiry",
            fields.tax_agent_date_of_expiry.as_str(),
        ),
        ("payment_check_bank", fields.check_bank.as_str()),
        ("payment_check_number", fields.check_number.as_str()),
        ("payment_check_date", fields.check_date.as_str()),
        (
            "payment_tax_debit_memo_number",
            fields.tax_debit_memo_number.as_str(),
        ),
        (
            "payment_tax_debit_memo_date",
            fields.tax_debit_memo_date.as_str(),
        ),
        (
            "payment_other_description",
            fields.other_payment_description.as_str(),
        ),
        ("payment_other_bank", fields.other_payment_bank.as_str()),
        ("payment_other_number", fields.other_payment_number.as_str()),
        ("payment_other_date", fields.other_payment_date.as_str()),
        (
            "machine_validation_or_receipt_details",
            fields.machine_validation_or_receipt_details.as_str(),
        ),
    ] {
        insert_text(envelope, key, value);
    }
    for (key, value) in [
        (
            "payment_cash_or_bank_debit_advice_amount",
            fields.cash_or_bank_debit_advice_amount,
        ),
        ("payment_check_amount", fields.check_amount),
        (
            "payment_tax_debit_memo_amount",
            fields.tax_debit_memo_amount,
        ),
        ("payment_other_amount", fields.other_payment_amount),
    ] {
        insert_optional_decimal(envelope, key, value);
    }
}

fn classification_key(value: Form2550QTaxpayerClassification) -> &'static str {
    match value {
        Form2550QTaxpayerClassification::Micro => "micro",
        Form2550QTaxpayerClassification::Small => "small",
        Form2550QTaxpayerClassification::Medium => "medium",
        Form2550QTaxpayerClassification::Large => "large",
    }
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

fn insert_optional_integer(envelope: &mut RenderEnvelopeV1, key: &str, value: Option<u16>) {
    if let Some(value) = value {
        insert_integer(envelope, key, i64::from(value));
    }
}

fn insert_optional_decimal(envelope: &mut RenderEnvelopeV1, key: &str, value: Option<f64>) {
    if let Some(value) = value {
        envelope
            .fields
            .insert(key.to_string(), RenderValue::Decimal(value));
    }
}

fn insert_date(envelope: &mut RenderEnvelopeV1, key: &str, value: Option<Form2550QDate>) {
    insert_text(
        envelope,
        key,
        value.map_or_else(String::new, |date| date.to_string()),
    );
}

fn sum_optional(values: impl IntoIterator<Item = Option<f64>>) -> Option<f64> {
    values
        .into_iter()
        .try_fold(0.0, |total, value| value.map(|amount| total + amount))
        .map(|value| (value * 100.0).round() / 100.0)
}

fn fixtures() -> Result<Vec<RenderContractFixture>, RenderProviderError> {
    Ok(vec![
        fixture(
            "2550q-minimum.json",
            RenderFixtureKind::Minimum,
            true,
            minimum_fixture()?,
        ),
        fixture(
            "2550q-normal.json",
            RenderFixtureKind::Normal,
            true,
            normal_fixture()?,
        ),
        fixture(
            "2550q-long-values.json",
            RenderFixtureKind::LongValues,
            true,
            long_values_fixture()?,
        ),
        fixture(
            "2550q-validation-edge.json",
            RenderFixtureKind::ValidationEdge,
            false,
            validation_edge_fixture()?,
        ),
        fixture(
            "2550q-two-row-capacity.json",
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
    draft: Form2550QDraft,
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
        "full_name": "JUAN DELA CRUZ TRADING",
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
        "email": "renderer.2550q@example.com",
        "default_form_type": "2550Qv2024",
        "taxpayer_type": "Individual",
        "tax_classification": "SelfEmployed",
        "is_vat_registered": true,
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

fn minimum_fixture() -> Result<Form2550QDraft, RenderProviderError> {
    Ok(Form2550QDraft::new_from_profile(&profile()?, 2026, 1))
}

fn normal_fixture() -> Result<Form2550QDraft, RenderProviderError> {
    let mut draft = Form2550QDraft::new_from_profile(&profile()?, 2025, 1);
    draft.is_amended = true;
    draft.is_short_period_return = true;
    draft.is_availing_tax_relief = true;
    draft.tax_relief_details = "REVIEWED SPECIAL LAW".to_string();

    let part = &mut draft.part_iv;
    part.item_31a_vatable_sales = Some(125_000.0);
    part.item_32a_zero_rated_sales = Some(10_000.0);
    part.item_33a_exempt_sales = Some(5_000.0);
    part.item_35b_less_output_vat_uncollected = Some(250.0);
    part.item_36b_add_output_vat_recovered = Some(100.0);
    part.item_38b_input_tax_carried = Some(2_000.0);
    part.item_40b_transitional_input_tax = Some(0.0);
    part.item_41b_presumptive_input_tax = Some(0.0);
    part.item_42_description = "OTHER REVIEWED INPUT TAX".to_string();
    part.item_42b_other_input_tax = Some(300.0);
    part.item_44a_domestic_purchases = Some(50_000.0);
    part.item_44b_domestic_input_tax = Some(6_000.0);
    part.item_45a_nonresident_services = Some(0.0);
    part.item_45b_nonresident_service_input_tax = Some(0.0);
    part.item_46a_importations = Some(10_000.0);
    part.item_46b_import_input_tax = Some(1_200.0);
    part.item_47_description = "OTHER CURRENT PURCHASES".to_string();
    part.item_47a_other_purchases = Some(2_500.0);
    part.item_47b_other_input_tax = Some(300.0);
    part.item_48a_domestic_purchases_no_input_tax = Some(1_000.0);
    part.item_49a_vat_exempt_importations = Some(750.0);
    part.item_54b_vat_refund_or_tcc_claimed = Some(0.0);
    part.item_55b_input_vat_on_unpaid_payables = Some(0.0);
    part.item_56_description = "OTHER INPUT TAX DEDUCTION".to_string();
    part.item_56b_other_deduction = Some(100.0);
    part.item_58b_input_vat_on_settled_payables = Some(200.0);

    draft.part_ii.item_18_paid_on_previous_return = Some(500.0);
    draft.part_ii.item_19_description = "PRIOR REVIEWED PAYMENT".to_string();
    draft.part_ii.item_19_other_credit_or_payment = Some(250.0);
    draft.part_ii.item_22_surcharge = Some(100.0);
    draft.part_ii.item_23_interest = Some(25.0);
    draft.part_ii.item_24_compromise = Some(1_000.0);

    draft.schedule_1[0] = capital_good_row(
        date(2025, 1, 15)?,
        "D",
        "COMPUTER SERVER",
        120_000.0,
        14_400.0,
        60,
        12,
        2_880.0,
        11_520.0,
    );
    draft
        .schedule_2
        .input_tax_directly_attributable_to_exempt_sales = Some(100.0);
    draft.schedule_2.vat_exempt_sales = Some(5_000.0);
    draft.schedule_2.input_tax_not_directly_attributable = Some(1_000.0);
    draft.schedule_3[0] = creditable_row(
        date(2025, 1, 1)?,
        date(2025, 3, 31)?,
        "REVIEWED WITHHOLDING AGENT",
        10_000.0,
        500.0,
    );
    draft.schedule_4[0] = advance_row(
        date(2025, 1, 1)?,
        date(2025, 3, 31)?,
        "REVIEWED MILLER",
        "JUAN DELA CRUZ TRADING",
        "OR-2025-0001",
        250.0,
    );
    draft
        .local_print_fields
        .taxpayer_or_authorized_representative = "JUAN DELA CRUZ".to_string();
    draft.local_print_fields.representative_title = "OWNER".to_string();
    draft.local_print_fields.check_bank = "DEVELOPMENT BANK".to_string();
    draft.local_print_fields.check_number = "000123".to_string();
    draft.local_print_fields.check_date = "04/25/2025".to_string();
    draft.local_print_fields.check_amount = Some(1_125.0);
    draft
        .local_print_fields
        .machine_validation_or_receipt_details = "AAB RECEIPT 2550Q-2025-0001".to_string();
    draft.recompute();
    Ok(draft)
}

fn long_values_fixture() -> Result<Form2550QDraft, RenderProviderError> {
    let mut draft = normal_fixture()?;
    draft.taxpayer_name = "A DELIBERATELY LONG REGISTERED VAT TAXPAYER NAME THAT MUST SWITCH FROM COMB CELLS TO A REVIEWED PLAIN BOX WITHOUT LOSING CHARACTERS INCORPORATED".to_string();
    draft.registered_address = "UNIT 1201, A DELIBERATELY LONG REGISTERED ADDRESS THAT MUST REMAIN COMPLETE, BUILDING ONE, BUSINESS DISTRICT, NEW CABALAN, OLONGAPO CITY, ZAMBALES, PHILIPPINES".to_string();
    draft.email = "long.2550q.renderer.verification.address@example.test".to_string();
    draft.tax_relief_details = "A DELIBERATELY LONG SPECIAL LAW OR INTERNATIONAL TAX TREATY DESCRIPTION THAT MUST NOT BE TRUNCATED".to_string();
    draft.part_ii.item_19_description =
        "A DELIBERATELY LONG OTHER TAX CREDIT OR PAYMENT DESCRIPTION".to_string();
    draft.part_iv.item_42_description =
        "A DELIBERATELY LONG OTHER PRIOR INPUT TAX DESCRIPTION".to_string();
    draft.part_iv.item_47_description =
        "A DELIBERATELY LONG OTHER CURRENT PURCHASE DESCRIPTION".to_string();
    draft.part_iv.item_56_description =
        "A DELIBERATELY LONG OTHER INPUT TAX DEDUCTION DESCRIPTION".to_string();
    draft.schedule_1[0].description =
        "CAPITAL GOODS DESCRIPTION THAT EXCEEDS THE OFFICIAL COMB CAPACITY".to_string();
    draft.schedule_3[0].withholding_agent_name =
        "A DELIBERATELY LONG WITHHOLDING AGENT REGISTERED NAME".to_string();
    draft.schedule_4[0].miller_name = "A DELIBERATELY LONG MILLER REGISTERED NAME".to_string();
    draft.schedule_4[0].taxpayer_name = "A DELIBERATELY LONG TAXPAYER REGISTERED NAME".to_string();
    draft.schedule_4[0].official_receipt_number =
        "OFFICIAL-RECEIPT-IDENTIFIER-THAT-MUST-REMAIN-TEXT".to_string();
    draft.local_print_fields.machine_validation_or_receipt_details = "A DELIBERATELY LONG MACHINE VALIDATION OR REVENUE OFFICIAL RECEIPT DESCRIPTION THAT MUST NOT BE TRUNCATED".to_string();
    draft.recompute();
    Ok(draft)
}

fn validation_edge_fixture() -> Result<Form2550QDraft, RenderProviderError> {
    let mut draft = minimum_fixture()?;
    draft.taxpayer_classification = None;
    draft.taxpayer_name.clear();
    draft.email = "not-an-email".to_string();
    draft.part_iv.item_31a_vatable_sales = None;
    draft.schedule_1.truncate(1);
    draft.recompute();
    Ok(draft)
}

fn capacity_fixture() -> Result<Form2550QDraft, RenderProviderError> {
    let mut draft = normal_fixture()?;
    draft.schedule_1[1] = capital_good_row(
        date(2025, 2, 1)?,
        "I",
        "IMPORTED EQUIPMENT",
        240_000.0,
        28_800.0,
        120,
        6,
        1_440.0,
        27_360.0,
    );
    draft.schedule_3[1] = creditable_row(
        date(2025, 2, 1)?,
        date(2025, 2, 28)?,
        "SECOND WITHHOLDING AGENT",
        20_000.0,
        1_000.0,
    );
    draft.schedule_4[1] = advance_row(
        date(2025, 2, 1)?,
        date(2025, 2, 28)?,
        "SECOND MILLER",
        "JUAN DELA CRUZ TRADING",
        "OR-2025-0002",
        750.0,
    );
    draft.ensure_repeating_row_ids().map_err(|error| {
        RenderProviderError::Fixture(format!(
            "failed to restore stable 2550Q renderer row identities: {error}"
        ))
    })?;
    draft.recompute();
    Ok(draft)
}

#[allow(clippy::too_many_arguments)]
fn capital_good_row(
    date: Form2550QDate,
    source: &str,
    description: &str,
    amount: f64,
    input_tax: f64,
    estimated_life: u16,
    recognized_life: u16,
    allowable: f64,
    balance: f64,
) -> Form2550QCapitalGoodRow {
    Form2550QCapitalGoodRow {
        instance_id: None,
        purchase_or_import_date: Some(date),
        source_code: source.to_string(),
        description: description.to_string(),
        purchase_or_import_amount: Some(amount),
        input_tax: Some(input_tax),
        estimated_life_months: Some(estimated_life),
        recognized_life_months: Some(recognized_life),
        allowable_input_tax_for_period: Some(allowable),
        balance_to_next_period: Some(balance),
    }
}

fn creditable_row(
    from: Form2550QDate,
    to: Form2550QDate,
    agent: &str,
    income_payment: f64,
    tax_withheld: f64,
) -> Form2550QCreditableVatRow {
    Form2550QCreditableVatRow {
        instance_id: None,
        period_from: Some(from),
        period_to: Some(to),
        withholding_agent_name: agent.to_string(),
        income_payment: Some(income_payment),
        tax_withheld: Some(tax_withheld),
    }
}

fn advance_row(
    from: Form2550QDate,
    to: Form2550QDate,
    miller: &str,
    taxpayer: &str,
    receipt: &str,
    amount: f64,
) -> Form2550QAdvanceVatRow {
    Form2550QAdvanceVatRow {
        instance_id: None,
        period_from: Some(from),
        period_to: Some(to),
        miller_name: miller.to_string(),
        taxpayer_name: taxpayer.to_string(),
        official_receipt_number: receipt.to_string(),
        amount_paid: Some(amount),
    }
}

fn date(year: u16, month: u8, day: u8) -> Result<Form2550QDate, RenderProviderError> {
    Form2550QDate::new(year, month, day).map_err(RenderProviderError::Fixture)
}

fn generated_artifacts() -> Result<Vec<GeneratedContractArtifact>, RenderProviderError> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_maps_exact_geometry_identity_and_negative_credit_without_recalculation() {
        let draft = normal_fixture().expect("normal fixture");
        let envelope = RenderEnvelopeV1::from(&draft);

        assert_eq!(
            PROVIDER.page_geometry().unwrap(),
            RenderPageGeometry::FOURTEEN_INCH
        );
        assert_eq!(PROVIDER.expected_page_count(&envelope).unwrap(), 2);
        assert_eq!(envelope.form.code, "2550Q");
        assert_eq!(envelope.form.version, "2024");
        assert_eq!(
            envelope.fields["item_61b"],
            RenderValue::Decimal(draft.part_iv.item_61b_net_vat_payable_or_excess.unwrap())
        );
        assert_eq!(
            envelope.fields["item_21"],
            RenderValue::Decimal(draft.part_ii.item_21_tax_payable_or_excess_credits.unwrap())
        );
        assert!(envelope.validation.is_empty(), "{:?}", envelope.validation);
    }

    #[test]
    fn provider_preserves_blank_optional_amounts_and_fixed_schedule_capacity() {
        let mut draft = minimum_fixture().expect("minimum fixture");
        draft.local_print_fields.check_amount = None;
        let envelope = RenderEnvelopeV1::from(&draft);

        assert!(!envelope.fields.contains_key("payment_check_amount"));
        for schedule in ["schedule_1", "schedule_3", "schedule_4"] {
            assert!(envelope
                .fields
                .keys()
                .any(|key| key.starts_with(&format!("{schedule}_1_"))));
            assert!(envelope
                .fields
                .keys()
                .any(|key| key.starts_with(&format!("{schedule}_2_"))));
        }
        assert!(envelope.schedules.is_empty());
    }

    #[test]
    fn fixtures_cover_required_two_page_matrix() {
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
            assert_eq!(PROVIDER.expected_page_count(&fixture.envelope).unwrap(), 2);
            assert_eq!(
                fixture.expected_form_valid,
                fixture.envelope.validation.is_empty(),
                "{} validity mismatch: {:?}",
                fixture.file_name,
                fixture.envelope.validation
            );
        }
    }

    #[test]
    fn discrete_artwork_provenance_matches_verified_official_xobjects() {
        let assets = runtime_discrete_assets();
        assert_eq!(assets.len(), 3);

        let seal = &assets[0];
        assert_eq!(seal["asset"], json!("government_seal"));
        assert_eq!(seal["source_pdf_object_id"], json!([37, 0]));
        assert_eq!(seal["source_pixel_dimensions"], json!([86, 82]));
        assert_eq!(seal["source_channels_equal"], json!(true));
        assert_eq!(
            seal["derived_png_sha256"],
            json!("e4259300bed379aa06e686a31638b1943232ade6476a72b115e8ed357890b6d5")
        );

        for (asset, expected) in assets[1..].iter().zip([
            (
                "static_form_pdf417_page_1",
                1,
                38,
                "2550Q 04/24ENCS P1",
                478,
                "5a8d7f411686eb62d9d45b25a0430703a087068d43cd34cf6e63c35d6fce435f",
                "59064ade808ae9a05497ebe5794e15f4da1b77f9c61009ca2ba85b5b2716625f",
                json!([434.3, 51.3, 156.2, 46.7]),
            ),
            (
                "static_form_pdf417_page_2",
                2,
                47,
                "2550Q 04/24ENCS P2",
                479,
                "95368d60c40a9767d65c09836f3c36152adfd42cd70c740dea33c19e75fe86e6",
                "4b76223a10321504a7fde4e652b651f06418779c085b1e31c83fcc05082ee810",
                json!([433.9, 24.4, 153.9, 45.1]),
            ),
        ]) {
            let (name, page, object, payload, black_modules, matrix_hash, path_hash, active_bbox) =
                expected;
            assert_eq!(asset["asset"], json!(name));
            assert_eq!(asset["source_page"], json!(page));
            assert_eq!(asset["source_pdf_object_id"], json!([object, 0]));
            assert_eq!(asset["source_pixel_dimensions"], json!([240, 63]));
            assert_eq!(asset["source_active_pixel_bounds"], json!([0, 0, 240, 63]));
            assert_eq!(asset["source_active_bbox_top_left_points"], active_bbox);
            assert_eq!(asset["decoded_payload"], json!(payload));
            assert_eq!(asset["symbology"], json!("PDF417"));
            assert_eq!(asset["logical_black_modules"], json!(black_modules));
            assert_eq!(asset["logical_dimensions"], json!([120, 7]));
            assert_eq!(asset["logical_matrix_sha256"], json!(matrix_hash));
            assert_eq!(asset["logical_path_sha256"], json!(path_hash));
            assert_eq!(asset["source_module_scale_pixels"], json!([2, 9]));
            assert_eq!(
                asset["source_padding_pixels"],
                json!({"bottom": 0, "right": 0})
            );
            assert_eq!(asset["encoder_proof"]["module_differences"], json!(0));
        }
    }
}
