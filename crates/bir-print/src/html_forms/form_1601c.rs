use std::collections::BTreeMap;

use bir_core::forms::{
    form_1601c::{Form1601CDraft, Form1601CSchedule1Row, MAX_SCHEDULE_1_ROWS},
    FormValidator,
};
use serde_json::json;

use crate::html::{
    RenderAlignment, RenderColumn, RenderEnvelopeV1, RenderFormIdentity, RenderPeriod, RenderRow,
    RenderSchedule, RenderTaxpayer, RenderValidationMessage, RenderValidationSeverity, RenderValue,
};

use super::{
    GeneratedContractArtifact, RenderContractFixture, RenderFixtureKind, RenderFormProvider,
    RenderPageGeometry, RenderProviderError, RenderSchedulePolicy, VisualReferencePage,
};

const SCHEDULES: &[RenderSchedulePolicy] = &[RenderSchedulePolicy {
    id: "schedule_1",
    minimum_rows: MAX_SCHEDULE_1_ROWS,
    first_page_rows: MAX_SCHEDULE_1_ROWS,
    // The verified 2018 XML contract accepts only three rows. Keep this value
    // non-zero for the generic policy validator, while domain validation
    // rejects any envelope that would require an additional schedule page.
    continuation_page_rows: MAX_SCHEDULE_1_ROWS,
    repeat_header: true,
    final_totals_on_last_page: true,
}];

const VISUAL_REFERENCE_PAGES: &[VisualReferencePage] = &[
    VisualReferencePage {
        page: 1,
        file_name: "1601c-2018-page-1.png",
        sha256: "923cc850191e49ea69874dff55a7257fa0b3428b38a3f0387911279f1fabf023",
    },
    VisualReferencePage {
        page: 2,
        file_name: "1601c-2018-page-2.png",
        sha256: "26af686719726ac98f160b0665c8606e6501e46f95722ef150951ee7e976418a",
    },
];

// Pinned by scripts/prepare_chromium_reference.mjs from the same official
// PDF bytes; see references/1601c-2018-chromium-source.json
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
            file_name: "1601c-2018-page-1-chromium.png",
            sha256: "dbf35a39ca207a75792317eea000db6bdaa5710600d470665299a083e44230bc",
            vector_svg_sha256: "9e1b07f5580328d51b5820a0a127550ab5ea3267719af8772b8128bfeec6f2b0",
            noise_floor_changed_pixels: 86_220,
        },
        super::ChromiumVisualReference {
            page: 2,
            file_name: "1601c-2018-page-2-chromium.png",
            sha256: "92cf727b8ac1373da18da9e5b455c16643edec4b8a360f1fd6ea4ff93a4619eb",
            vector_svg_sha256: "4969b23016fe5da7afe1105ddac720862db433b4c1642c80bd2268d5fe7746dc",
            noise_floor_changed_pixels: 103_802,
        },
    ],
};

pub(super) const PROVIDER: RenderFormProvider = RenderFormProvider {
    code: "1601C",
    revision: "2018",
    form_id: "1601Cv2018",
    title: "Monthly Remittance Return of Income Taxes Withheld on Compensation",
    page_width_pt: RenderPageGeometry::LEGAL.width_points,
    page_height_pt: RenderPageGeometry::LEGAL.height_points,
    expected_base_page_count: 2,
    schedules: SCHEDULES,
    visual_fixture_file_name: "1601c-normal.json",
    visual_fixture_sha256: "ae115b4e97c9b2ffc9b59b5820e255be3e180efba09996d72030f0c6d607ea34",
    official_source:
        "https://bir-cdn.bir.gov.ph/local/pdf/1601C%20final%20Jan%202018%20with%20DPA.pdf",
    official_source_sha256: "c8faaa71015337a73b4ceb96bfb265c539589ab5e10eb27899bb81f87f417397",
    reference_dpi: 144,
    reference_width_px: 1_224,
    reference_height_px: 1_872,
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
            "derived_grayscale_pixel_sha256": "14e4bd2fc6895f0a6e2435af0d1fd4c236cbfdd84ac3890c8862171465e399d0",
            "derived_png_sha256": "de602852cef008b3182bb77b03c06d1ec3f0a6ea2484d3d25c0d161df56f270b",
            "embedded_as": "lossless_grayscale_collapse_of_equal_rgb_channels",
            "embedded_color_space": "DeviceGray",
            "embedded_in": "packages/form-renderer/src/forms/assets/1601c-seal.png",
            "source_bbox_top_left_points": [228.36, 9.84, 31.2, 28.2],
            "source_channels_equal": true,
            "source_ctm_points": [31.2, 0.0, 0.0, 28.2, 228.36, 897.96],
            "source_decoded_rgb_sha256": "5eae06e03b3e0c0365397cd2d401292678acb69fecdf35f5afe5fecd342cf85d",
            "source_page": 1,
            "source_pdf_object_id": [41, 0],
            "source_pixel_dimensions": [86, 78],
            "source_png_sha256": "6cd637975f8088cc8b56aa67c26794db23331864a45c08517c66b70f53ff2610",
            "source_stream_sha256": "39613d13cd9b910b039ef6287ce4ef059e03661acf565719fdb740d751771ba1",
            "treatment": "lossless extraction of the exact official PDF image XObject; equal RGB channels collapsed to one native grayscale channel without crop, resampling, recoloring, or substitution"
        }),
        json!({
            "asset": "static_form_pdf417_page_1",
            "caption_bbox_top_left_points": [509.86, 83.7038, 82.7903, 8.9646],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_source_span_bbox_top_left_points": [509.86, 83.7038, 84.9151, 8.9646],
            "caption_source_span_trailing_space": true,
            "caption_text": "1601-C 01/18ENCS P1",
            "decoded_payload": "1601-C 01/18ENCS P1",
            "decoder_evidence": [
                {"decoder": "ZXing-C++ 3.1.0", "payload": "1601-C 01/18ENCS P1", "symbology": "PDF417"}
            ],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1601CAssets.ts",
            "encoder_proof": {
                "columns": 3,
                "encoding": "ISO-8859-1",
                "error_correction_level": 2,
                "implementation": "pdf417gen 0.8.1",
                "module_differences": 0,
                "rows": 7
            },
            "logical_black_modules": 474,
            "logical_dimensions": [120, 7],
            "logical_matrix_sha256": "7e4a3607ef9e721686f43cef71aba5b7426e2727b830149e37866e1d35be9a45",
            "logical_path_sha256": "799174be5028c217427e7eaa4355a5ed578253aea8e02cd3bb61b81c57c0dcce",
            "source_active_bbox_top_left_points": [441.12, 48.72, 150.84, 34.92],
            "source_active_pixel_bounds": [0, 0, 240, 63],
            "source_bbox_top_left_points": [441.12, 48.72, 150.84, 34.92],
            "source_ctm_points": [150.84, 0.0, 0.0, 34.92, 441.12, 852.36],
            "source_decoded_rgb_sha256": "f9af877d7cd2d238885f4554afd0211f2ebe83d3de1163b6a84f160d09fed56f",
            "source_page": 1,
            "source_pdf_object_id": [42, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 0, "right": 0},
            "source_padding_points": {"bottom": 0.0, "right": 0.0},
            "source_pixel_dimensions": [240, 63],
            "source_png_sha256": "726f5550edc3b41a3ccf73ebbd1ccbc22f6a168a85327bb85802cd434d8abfb5",
            "source_stream_sha256": "97c0d9925968373eda7225c87e4a273a99779939b22ac0dcc900b0e9e6478fa8",
            "symbology": "PDF417"
        }),
        json!({
            "asset": "static_form_pdf417_page_2",
            "caption_bbox_top_left_points": [509.26, 68.0838, 82.7902, 8.9646],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_source_span_bbox_top_left_points": [509.26, 68.0838, 84.9151, 8.9646],
            "caption_source_span_trailing_space": true,
            "caption_text": "1601-C 01/18ENCS P2",
            "decoded_payload": "1601-C 01/18ENCS P2",
            "decoder_evidence": [
                {"decoder": "ZXing-C++ 3.1.0", "payload": "1601-C 01/18ENCS P2", "symbology": "PDF417"}
            ],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1601CAssets.ts",
            "encoder_proof": {
                "columns": 3,
                "encoding": "ISO-8859-1",
                "error_correction_level": 2,
                "implementation": "pdf417gen 0.8.1",
                "module_differences": 0,
                "rows": 7
            },
            "logical_black_modules": 476,
            "logical_dimensions": [120, 7],
            "logical_matrix_sha256": "af50f07764d907447d25e8a18fde78e407679bfbffab5b32eb07fc82aff851c4",
            "logical_path_sha256": "243d0773c45f3a9c38d0823a07eebbee19218953601d522c546f97e52c2bb313",
            "source_active_bbox_top_left_points": [439.92, 34.32, 150.96, 34.32],
            "source_active_pixel_bounds": [0, 0, 240, 63],
            "source_bbox_top_left_points": [439.92, 34.32, 150.96, 34.32],
            "source_ctm_points": [150.96, 0.0, 0.0, 34.32, 439.92, 867.36],
            "source_decoded_rgb_sha256": "b057c7676068b3db15962b8a630a2582b6506256eac3165277d047575d9713c3",
            "source_page": 2,
            "source_pdf_object_id": [55, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 0, "right": 0},
            "source_padding_points": {"bottom": 0.0, "right": 0.0},
            "source_pixel_dimensions": [240, 63],
            "source_png_sha256": "5c6435f8f719cfdc2f35c8608d30cecf7217d133baa074ed2f09a6204d4c8870",
            "source_stream_sha256": "4a21defb53bb6364abedc524a53891b07de61a105a18c43e9e06504866123c61",
            "symbology": "PDF417"
        }),
    ]
}

impl From<&Form1601CDraft> for RenderEnvelopeV1 {
    fn from(draft: &Form1601CDraft) -> Self {
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
                email: draft.email_address.clone(),
            },
            RenderPeriod {
                taxable_year: draft.taxable_year,
                month: Some(draft.month),
                quarter: None,
                label: format!("{:02}/{}", draft.month, draft.taxable_year),
            },
        );

        envelope.fields.extend([
            (
                "is_amended".to_string(),
                RenderValue::Boolean(draft.is_amended),
            ),
            (
                "any_taxes_withheld".to_string(),
                RenderValue::Boolean(draft.any_taxes_withheld),
            ),
            (
                "number_of_sheets".to_string(),
                RenderValue::Integer(i64::from(draft.number_of_sheets)),
            ),
            ("atc".to_string(), RenderValue::Text(draft.atc.clone())),
            (
                "line_of_business".to_string(),
                RenderValue::Text(draft.line_of_business.clone()),
            ),
            (
                "registered_address_2".to_string(),
                RenderValue::Text(draft.registered_address_2.clone()),
            ),
            (
                "category_of_agent".to_string(),
                RenderValue::Text(draft.category_of_agent.clone()),
            ),
            (
                "tax_relief".to_string(),
                RenderValue::Boolean(draft.tax_relief),
            ),
            (
                "tax_relief_specification".to_string(),
                RenderValue::Text(if draft.tax_relief {
                    draft.tax_relief_specification.clone()
                } else {
                    String::new()
                }),
            ),
            (
                "tax_14_total_compensation".to_string(),
                RenderValue::Decimal(draft.tax_14_total_compensation),
            ),
            (
                "tax_15_statutory_minimum_wage".to_string(),
                RenderValue::Decimal(draft.tax_15_statutory_minimum_wage),
            ),
            (
                "tax_16_holiday_pay".to_string(),
                RenderValue::Decimal(draft.tax_16_holiday_pay),
            ),
            (
                "tax_17_13th_month_pay".to_string(),
                RenderValue::Decimal(draft.tax_17_13th_month_pay),
            ),
            (
                "tax_18_de_minimis".to_string(),
                RenderValue::Decimal(draft.tax_18_de_minimis),
            ),
            (
                "tax_19_sss_gsis".to_string(),
                RenderValue::Decimal(draft.tax_19_sss_gsis),
            ),
            (
                "tax_20_other_name".to_string(),
                RenderValue::Text(draft.tax_20_other_name.clone()),
            ),
            (
                "tax_20_other_amount".to_string(),
                RenderValue::Decimal(draft.tax_20_other_amount),
            ),
            (
                "tax_21_total_non_taxable".to_string(),
                RenderValue::Decimal(draft.tax_21_total_non_taxable),
            ),
            (
                "tax_22_total_taxable".to_string(),
                RenderValue::Decimal(draft.tax_22_total_taxable),
            ),
            (
                "tax_23_not_subject".to_string(),
                RenderValue::Decimal(draft.tax_23_not_subject),
            ),
            (
                "tax_24_net_taxable".to_string(),
                RenderValue::Decimal(draft.tax_24_net_taxable),
            ),
            (
                "tax_25_total_taxes_withheld".to_string(),
                RenderValue::Decimal(draft.tax_25_total_taxes_withheld),
            ),
            (
                "tax_26_adjustment".to_string(),
                RenderValue::Decimal(draft.tax_26_adjustment),
            ),
            (
                "tax_27_taxes_withheld_for_remittance".to_string(),
                RenderValue::Decimal(draft.tax_27_taxes_withheld_for_remittance),
            ),
            (
                "tax_28_tax_remitted_previously".to_string(),
                RenderValue::Decimal(draft.tax_28_tax_remitted_previously),
            ),
            (
                "tax_29_other_remittances_name".to_string(),
                RenderValue::Text(draft.tax_29_other_remittances_name.clone()),
            ),
            (
                "tax_29_other_remittances_amount".to_string(),
                RenderValue::Decimal(draft.tax_29_other_remittances_amount),
            ),
            (
                "tax_30_total_tax_remittances".to_string(),
                RenderValue::Decimal(draft.tax_30_total_tax_remittances),
            ),
            (
                "tax_31_tax_still_due".to_string(),
                RenderValue::Decimal(draft.tax_31_tax_still_due),
            ),
            (
                "tax_32_surcharge".to_string(),
                RenderValue::Decimal(draft.tax_32_surcharge),
            ),
            (
                "tax_33_interest".to_string(),
                RenderValue::Decimal(draft.tax_33_interest),
            ),
            (
                "tax_34_compromise".to_string(),
                RenderValue::Decimal(draft.tax_34_compromise),
            ),
            (
                "tax_35_total_penalties".to_string(),
                RenderValue::Decimal(draft.tax_35_total_penalties),
            ),
            (
                "tax_36_total_amount_payable".to_string(),
                RenderValue::Decimal(draft.tax_36_total_amount_payable),
            ),
        ]);

        let rows = draft
            .schedule_1
            .iter()
            .enumerate()
            .map(|(index, row)| RenderRow {
                key: format!("schedule-1-{}", index + 1),
                cells: BTreeMap::from([
                    (
                        "previous_month".to_string(),
                        RenderValue::Text(row.previous_month.clone()),
                    ),
                    (
                        "date_paid".to_string(),
                        RenderValue::Text(row.date_paid.clone()),
                    ),
                    (
                        "drawee_bank_code_or_agency".to_string(),
                        RenderValue::Text(row.drawee_bank_code_or_agency.clone()),
                    ),
                    (
                        "payment_number".to_string(),
                        RenderValue::Text(row.payment_number.clone()),
                    ),
                    ("tax_paid".to_string(), RenderValue::Decimal(row.tax_paid)),
                    (
                        "should_be_tax_due".to_string(),
                        RenderValue::Decimal(row.should_be_tax_due),
                    ),
                    (
                        "adjustment".to_string(),
                        RenderValue::Decimal(row.adjustment),
                    ),
                ]),
            })
            .collect();

        envelope.schedules.push(RenderSchedule {
            id: SCHEDULES[0].id.to_string(),
            columns: vec![
                column("previous_month", "Previous Month", RenderAlignment::Center),
                column("date_paid", "Date Paid", RenderAlignment::Center),
                column(
                    "drawee_bank_code_or_agency",
                    "Drawee Bank/Bank Code/Agency",
                    RenderAlignment::Left,
                ),
                column("payment_number", "Number", RenderAlignment::Left),
                column("tax_paid", "Tax Paid", RenderAlignment::Right),
                column(
                    "should_be_tax_due",
                    "Should Be Tax Due",
                    RenderAlignment::Right,
                ),
                column("adjustment", "Adjustment", RenderAlignment::Right),
            ],
            rows,
        });

        envelope.validation = draft
            .validate()
            .into_iter()
            .map(|(field_path, message)| RenderValidationMessage {
                field_path,
                code: "invalid".to_string(),
                message,
                severity: RenderValidationSeverity::Error,
                rule_version: "1601c-main-v1".to_string(),
            })
            .collect();

        envelope
    }
}

fn fixtures() -> Result<Vec<RenderContractFixture>, RenderProviderError> {
    Ok(vec![
        fixture(
            "1601c-minimum.json",
            RenderFixtureKind::Minimum,
            true,
            minimum_fixture()?,
        ),
        fixture(
            "1601c-normal.json",
            RenderFixtureKind::Normal,
            true,
            fixture_with_rows(2)?,
        ),
        fixture(
            "1601c-long-values.json",
            RenderFixtureKind::LongValues,
            true,
            long_values_fixture()?,
        ),
        fixture(
            "1601c-validation-edge.json",
            RenderFixtureKind::ValidationEdge,
            false,
            validation_edge_fixture()?,
        ),
        fixture(
            "1601c-3-rows.json",
            RenderFixtureKind::ScheduleCapacity,
            true,
            fixture_with_rows(MAX_SCHEDULE_1_ROWS)?,
        ),
    ])
}

fn generated_artifacts() -> Result<Vec<GeneratedContractArtifact>, RenderProviderError> {
    Ok(Vec::new())
}

fn fixture(
    file_name: &'static str,
    kind: RenderFixtureKind,
    expected_form_valid: bool,
    draft: Form1601CDraft,
) -> RenderContractFixture {
    RenderContractFixture {
        file_name,
        kind,
        expected_form_valid,
        envelope: RenderEnvelopeV1::from(&draft),
    }
}

fn fixture_with_rows(row_count: usize) -> Result<Form1601CDraft, RenderProviderError> {
    if row_count > MAX_SCHEDULE_1_ROWS {
        return Err(RenderProviderError::Fixture(format!(
            "1601C fixture requested {row_count} rows; verified capacity is {MAX_SCHEDULE_1_ROWS}"
        )));
    }

    let mut draft: Form1601CDraft = serde_json::from_value(json!({
        "id": null,
        "tin": "12345678900000",
        "taxable_year": 2026,
        "month": 6,
        "is_amended": true,
        "any_taxes_withheld": true,
        "number_of_sheets": 2,
        "atc": "WW010",
        "rdo_code": "018",
        "line_of_business": "Software Development",
        "taxpayer_name": "Renderer Withholding Agent Corporation",
        "contact_number": "09123456789",
        "registered_address": "53 Santol Extension, New Cabalan",
        "registered_address_2": "Olongapo City",
        "zip_code": "2200",
        "category_of_agent": "P",
        "email_address": "withholding.renderer@example.com",
        "tax_relief": true,
        "tax_relief_specification": "Special Law 123",
        "tax_14_total_compensation": 500000.0,
        "tax_15_statutory_minimum_wage": 20000.0,
        "tax_16_holiday_pay": 5000.0,
        "tax_17_13th_month_pay": 30000.0,
        "tax_18_de_minimis": 10000.0,
        "tax_19_sss_gsis": 15000.0,
        "tax_20_other_name": "Other exempt compensation",
        "tax_20_other_amount": 2000.0,
        "tax_21_total_non_taxable": 0.0,
        "tax_22_total_taxable": 0.0,
        "tax_23_not_subject": 20000.0,
        "tax_24_net_taxable": 0.0,
        "tax_25_total_taxes_withheld": 65000.0,
        "tax_26_adjustment": 0.0,
        "schedule_1": [],
        "tax_27_taxes_withheld_for_remittance": 0.0,
        "tax_28_tax_remitted_previously": 5000.0,
        "tax_29_other_remittances_name": "Prior remittance",
        "tax_29_other_remittances_amount": 500.0,
        "tax_30_total_tax_remittances": 0.0,
        "tax_31_tax_still_due": 0.0,
        "auto_compute_penalties": false,
        "tax_32_surcharge": 0.0,
        "tax_33_interest": 0.0,
        "tax_34_compromise": 0.0,
        "tax_35_total_penalties": 0.0,
        "tax_36_total_amount_payable": 0.0,
        "status": "Draft",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z",
        "submission_attempts": 0,
        "submission_error": null,
        "next_retry_at": null
    }))?;

    draft.schedule_1 = (0..row_count)
        .map(|index| Form1601CSchedule1Row {
            previous_month: format!("{:02}/2026", 3 + index),
            date_paid: format!("05/{:02}/2026", 10 + index),
            drawee_bank_code_or_agency: format!("AAB-{:03}", index + 1),
            payment_number: format!("PAY-1601C-{:03}", index + 1),
            tax_paid: 10_000.0 + index as f64 * 1_000.0,
            should_be_tax_due: 10_500.0 + index as f64 * 1_100.0,
            adjustment: 0.0,
        })
        .collect();
    draft.compute();
    Ok(draft)
}

fn minimum_fixture() -> Result<Form1601CDraft, RenderProviderError> {
    let mut draft = fixture_with_rows(0)?;
    draft.is_amended = false;
    draft.any_taxes_withheld = false;
    draft.number_of_sheets = 0;
    draft.tax_relief = false;
    draft.tax_relief_specification.clear();
    draft.tax_14_total_compensation = 0.0;
    draft.tax_15_statutory_minimum_wage = 0.0;
    draft.tax_16_holiday_pay = 0.0;
    draft.tax_17_13th_month_pay = 0.0;
    draft.tax_18_de_minimis = 0.0;
    draft.tax_19_sss_gsis = 0.0;
    draft.tax_20_other_name.clear();
    draft.tax_20_other_amount = 0.0;
    draft.tax_23_not_subject = 0.0;
    draft.tax_25_total_taxes_withheld = 0.0;
    draft.tax_28_tax_remitted_previously = 0.0;
    draft.tax_29_other_remittances_name.clear();
    draft.tax_29_other_remittances_amount = 0.0;
    draft.schedule_1.clear();
    draft.compute();
    Ok(draft)
}

fn long_values_fixture() -> Result<Form1601CDraft, RenderProviderError> {
    let mut draft = fixture_with_rows(MAX_SCHEDULE_1_ROWS)?;
    draft.taxpayer_name =
        "Long Registered Withholding Agent Name That Exceeds The Official Comb Capacity Incorporated"
            .to_string();
    draft.registered_address = "Unit 1201, A Deliberately Long Registered Address Used To Prove The Renderer Never Truncates Taxpayer Data"
        .to_string();
    draft.registered_address_2 =
        "Barangay New Cabalan, Olongapo City, Zambales, Philippines".to_string();
    draft.contact_number = "+639123456789".to_string();
    draft.email_address = "long.withholding.renderer.verification.address@example.test".to_string();
    draft.line_of_business =
        "Software Development and Information Technology Consulting Services".to_string();
    // Exercise the reviewed comb-to-plain transition without inventing values
    // that cannot fit the official field at the renderer's readable 8 px floor.
    draft.tax_relief_specification = "Reviewed International Tax Treaty Filing".to_string();
    draft.tax_20_other_name = "Other non-taxable compensation details".to_string();
    draft.tax_29_other_remittances_name = "Other remittance from prior filing".to_string();
    for (index, row) in draft.schedule_1.iter_mut().enumerate() {
        row.drawee_bank_code_or_agency = format!("AAB-{:02}", index + 1);
        row.payment_number = format!("PAY-{:03}", index + 1);
    }
    draft.compute();
    Ok(draft)
}

fn validation_edge_fixture() -> Result<Form1601CDraft, RenderProviderError> {
    let mut draft = minimum_fixture()?;
    draft.any_taxes_withheld = true;
    draft.tax_relief = true;
    draft.tax_relief_specification.clear();
    draft.compute();
    Ok(draft)
}

fn column(key: &str, label: &str, alignment: RenderAlignment) -> RenderColumn {
    RenderColumn {
        key: key.to_string(),
        label: label.to_string(),
        alignment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_maps_rust_owned_1601c_values_and_formulas() {
        let draft = fixture_with_rows(2).expect("fixture");
        let envelope = RenderEnvelopeV1::from(&draft);

        assert_eq!(envelope.form.code, "1601C");
        assert_eq!(envelope.form.version, "2018");
        assert_eq!(envelope.period.month, Some(6));
        assert_eq!(envelope.schedules[0].rows.len(), 2);
        assert_eq!(
            envelope.schedules[0].rows[0].cells["adjustment"],
            RenderValue::Decimal(500.0)
        );
        assert_eq!(
            envelope.fields["tax_26_adjustment"],
            RenderValue::Decimal(1_100.0)
        );
        assert_eq!(
            envelope.fields["tax_27_taxes_withheld_for_remittance"],
            RenderValue::Decimal(66_100.0)
        );
        assert!(envelope.validation.is_empty());
    }

    #[test]
    fn discrete_artwork_provenance_matches_verified_official_xobjects() {
        let assets = runtime_discrete_assets();
        assert_eq!(assets.len(), 3);

        let seal = &assets[0];
        assert_eq!(seal["asset"], json!("government_seal"));
        assert_eq!(seal["source_pdf_object_id"], json!([41, 0]));
        assert_eq!(seal["source_pixel_dimensions"], json!([86, 78]));
        assert_eq!(seal["source_channels_equal"], json!(true));
        assert_eq!(
            seal["source_ctm_points"],
            json!([31.2, 0.0, 0.0, 28.2, 228.36, 897.96])
        );
        assert_eq!(
            seal["derived_png_sha256"],
            json!("de602852cef008b3182bb77b03c06d1ec3f0a6ea2484d3d25c0d161df56f270b")
        );

        for (asset, expected) in assets[1..].iter().zip([
            (
                "static_form_pdf417_page_1",
                1,
                42,
                "1601-C 01/18ENCS P1",
                474,
                "7e4a3607ef9e721686f43cef71aba5b7426e2727b830149e37866e1d35be9a45",
                json!([441.12, 48.72, 150.84, 34.92]),
            ),
            (
                "static_form_pdf417_page_2",
                2,
                55,
                "1601-C 01/18ENCS P2",
                476,
                "af50f07764d907447d25e8a18fde78e407679bfbffab5b32eb07fc82aff851c4",
                json!([439.92, 34.32, 150.96, 34.32]),
            ),
        ]) {
            let (name, page, object, payload, black_modules, matrix_hash, active_bbox) = expected;
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
            assert_eq!(asset["source_module_scale_pixels"], json!([2, 9]));
            assert_eq!(
                asset["source_padding_pixels"],
                json!({"bottom": 0, "right": 0})
            );
            assert_eq!(asset["encoder_proof"]["module_differences"], json!(0));
        }
    }

    #[test]
    fn fixtures_cover_required_matrix_and_invalid_edge() {
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
        let edge = fixtures
            .iter()
            .find(|fixture| fixture.file_name == "1601c-validation-edge.json")
            .expect("edge fixture");
        assert!(!edge.expected_form_valid);
        assert!(!edge.envelope.validation.is_empty());
    }
}
