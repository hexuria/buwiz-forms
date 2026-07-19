use std::collections::BTreeMap;

use bir_core::forms::{
    form_2551q::{
        AnnualIncomeTaxElection, Form2551QDraft, Item13Election, OverpaymentDisposition,
        Schedule1Row, TaxPeriodBasis, FORM_2551Q_SCHEDULE_CONTINUATION_ROW_CAPACITY,
        FORM_2551Q_XML_SCHEDULE_ROW_CAPACITY,
    },
    FormValidator, ATC_TABLE_2551Q,
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

const FIXTURE_ATC_CODES: [&str; 10] = [
    "PT010", "PT040", "PT041", "PT060", "PT070", "PT090", "PT140", "PT150", "PT160", "PT170",
];
const LONG_ITEM_17_DESCRIPTION: &str = "Reviewed prior-payment credit supported by official receipt and validation reference 2551Q Item 17";

const SCHEDULES: &[RenderSchedulePolicy] = &[RenderSchedulePolicy {
    id: "schedule_1",
    minimum_rows: FORM_2551Q_XML_SCHEDULE_ROW_CAPACITY,
    first_page_rows: FORM_2551Q_XML_SCHEDULE_ROW_CAPACITY,
    continuation_page_rows: FORM_2551Q_SCHEDULE_CONTINUATION_ROW_CAPACITY,
    repeat_header: true,
    final_totals_on_last_page: true,
}];

const VISUAL_REFERENCE_PAGES: &[VisualReferencePage] = &[
    VisualReferencePage {
        page: 1,
        file_name: "2551q-2018-page-1.png",
        sha256: "8f93ea4f937ca9bdb48ecc5b57de30e5130ab35608fdfc4609cb8dbf03f3f3d3",
    },
    VisualReferencePage {
        page: 2,
        file_name: "2551q-2018-page-2.png",
        sha256: "67d0d81fd14e5130e6e93f7e9eedc5f9219028a1cea5ecf5674dfce3c87f31da",
    },
];

// Pinned by scripts/prepare_chromium_reference.mjs from the same official
// PDF bytes; see references/2551q-2018-chromium-source.json for the full
// pdf -> svg -> png provenance chain.
const CHROMIUM_REFERENCES: super::ChromiumReferenceSet = super::ChromiumReferenceSet {
    generator: super::ChromiumReferenceGenerator {
        pdftocairo_version: "26.07.0",
        playwright_version: "1.58.2",
        chromium_version: "145.0.7632.6",
    },
    pages: &[
        super::ChromiumVisualReference {
            page: 1,
            file_name: "2551q-2018-page-1-chromium.png",
            sha256: "aa9c7c0a0a1de802bff03fca8a9c7b02f01aa64f00a090d58cd9b46a8e161f8c",
            vector_svg_sha256: "b6a83a36d88338b02aee285004992f95ddc78586d396d2ccc5096981902703f6",
            noise_floor_changed_pixels: 82_725,
        },
        super::ChromiumVisualReference {
            page: 2,
            file_name: "2551q-2018-page-2-chromium.png",
            sha256: "667b8c2176534ea735b6d67bdb5ebd9882fad6af33d2cb0a189a88c8c827b371",
            vector_svg_sha256: "cf5c0322e3d6b274a51190f267306e0ca7a7fddc4e8a257e774d42d350db9ae8",
            noise_floor_changed_pixels: 70_321,
        },
    ],
};

pub(super) const PROVIDER: RenderFormProvider = RenderFormProvider {
    code: "2551Q",
    revision: "2018",
    form_id: "2551Qv2018",
    title: "Quarterly Percentage Tax Return",
    page_width_pt: RenderPageGeometry::LEGAL.width_points,
    page_height_pt: RenderPageGeometry::LEGAL.height_points,
    expected_base_page_count: 2,
    schedules: SCHEDULES,
    visual_fixture_file_name: "2551q-6-rows.json",
    visual_fixture_sha256: "f7ccf4db8b7f57ed9897351887bd34987dbfa8f088767eb120391694d53c0777",
    official_source:
        "https://bir-cdn.bir.gov.ph/local/pdf/2551Q%20Jan%202018%20ENCS%20final%20rev%203_copy.pdf",
    official_source_sha256: "1f270ecf66d778836a14697863e420ff65d5ed0a5576a6cf58b97c9a8e8c9b24",
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
            "derived_png_sha256": "7db0df0c022263481d219eebc2077631866f626521b4fe93967910a6a9422a4f",
            "embedded_as": "lossless_official_pdf_xobject_png",
            "embedded_in": "packages/form-renderer/src/forms/assets/2551q-seal.png",
            "source_bbox_top_left_points": [244.8, 21.84, 28.8, 25.2],
            "source_channels_equal": true,
            "source_ctm_points": [28.8, 0.0, 0.0, 25.2, 244.8, 888.96],
            "source_decoded_rgb_sha256": "b826fbf397a21aa19a40db88b2d6edfdef8eb606f1b2f03578b4d02ebad549fe",
            "source_page": 1,
            "source_pdf_object_id": [9, 0],
            "source_pixel_dimensions": [95, 83],
            "source_png_sha256": "7db0df0c022263481d219eebc2077631866f626521b4fe93967910a6a9422a4f",
            "source_stream_sha256": "464ad6cea97d3210a5843eda9ae29e162e416cc5d2a73d2b57696508d79b9b2e",
            "treatment": "lossless extraction of the exact official equal-channel PDF image XObject; no crop, resampling, recoloring, or substitution"
        }),
        json!({
            "asset": "static_form_pdf417_page_1",
            "caption_bbox_top_left_points": [504.22, 96.663803, 80.670197, 8.9646],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "2551Q 01/18ENCS P1",
            "decoded_payload": "2551Q 01/18ENCS P1",
            "decoder_evidence": [{"decoder": "ZXing 3.5.3", "payload": "2551Q 01/18ENCS P1", "symbology": "PDF417"}],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official2551QAssets.ts",
            "encoder_proof": {"columns": 3, "encoding": "ISO-8859-1", "error_correction_level": 2, "implementation": "pdf417gen 0.8.1", "module_differences": 0, "rows": 7},
            "logical_black_modules": 491,
            "logical_dimensions": [120, 7],
            "logical_matrix_sha256": "0b22b8418dc0dadb2043dd6022cb337e6fd60193b50105a3e7c47de3e565b9ca",
            "logical_path_sha256": "7ecda284c8c42dc6c64631313ff25e705c11450b9a1139223978f43e7d8168c2",
            "source_ctm_points": [161.28, 0.0, 0.0, 37.08, 425.76, 839.52],
            "source_decoded_rgb_sha256": "1162d0ddcb503c1ff41dfa01255c8f0fc2ede5a76d84b8d774941c1685959da3",
            "source_pdf_object_id": [35, 0],
            "source_page": 1,
            "source_pixel_dimensions": [240, 63],
            "source_png_sha256": "2fde50ef1089a8b5ec262b2927ea4d6d26323e9fab95832b22ca3b4ab602eae3",
            "source_stream_sha256": "889982f23eeedeb85d61a4686e2737e2eddf66705df230afc71a5aca22656a9b",
            "symbology": "PDF417"
        }),
        json!({
            "asset": "static_form_pdf417_page_2",
            "caption_bbox_top_left_points": [504.22, 86.943771, 80.670197, 8.9646],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "2551Q 01/18ENCS P2",
            "decoded_payload": "2551Q 01/18ENCS P2",
            "decoder_evidence": [{"decoder": "ZXing 3.5.3", "payload": "2551Q 01/18ENCS P2", "symbology": "PDF417"}],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official2551QAssets.ts",
            "encoder_proof": {"columns": 3, "encoding": "ISO-8859-1", "error_correction_level": 2, "implementation": "pdf417gen 0.8.1", "module_differences": 0, "rows": 7},
            "logical_black_modules": 484,
            "logical_dimensions": [120, 7],
            "logical_matrix_sha256": "b9257f08de39c25c64cbb7b8cbef835c060a5e347866c67b4cee558669f3233f",
            "logical_path_sha256": "d8c1d2fb417d8c4f83c147a5af7c3aee41f811beaa56ca39e20f77117c2c923e",
            "source_ctm_points": [163.2, 0.0, 0.0, 37.56, 424.32, 849.84],
            "source_decoded_rgb_sha256": "df505160a42c47d7d64d7590ec8146124360aa0062950aca9ede808b92396cc6",
            "source_pdf_object_id": [38, 0],
            "source_page": 2,
            "source_pixel_dimensions": [240, 63],
            "source_png_sha256": "8c89b401f443252642d6820164c6801ab00ba0dcd04715e9fa80eb05861f53d9",
            "source_stream_sha256": "27639c545c7eeee18bcac00bd28e5b1dda3a7d9f7c5c3d11569c1a38d7423c69",
            "symbology": "PDF417"
        }),
    ]
}

impl From<&Form2551QDraft> for RenderEnvelopeV1 {
    fn from(draft: &Form2551QDraft) -> Self {
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
                quarter: Some(draft.quarter),
                label: format!(
                    "Q{} year ended {:02}/{}",
                    draft.quarter, draft.year_end_month, draft.taxable_year
                ),
            },
        );

        envelope.fields.extend([
            (
                "tax_period_basis".to_string(),
                RenderValue::Text(
                    match draft.tax_period_basis {
                        TaxPeriodBasis::Calendar => "calendar",
                        TaxPeriodBasis::Fiscal => "fiscal",
                    }
                    .to_string(),
                ),
            ),
            (
                "is_amended".to_string(),
                RenderValue::Boolean(draft.is_amended),
            ),
            (
                "number_of_attached_sheets".to_string(),
                RenderValue::Integer(i64::from(draft.number_of_attached_sheets)),
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
                "item_13_election".to_string(),
                RenderValue::Text(
                    match draft.item_13_election {
                        Item13Election::Unanswered => "unanswered",
                        Item13Election::NotApplicable => "not_applicable",
                        Item13Election::Graduated => "graduated",
                        Item13Election::EightPercent => "eight_percent",
                    }
                    .to_string(),
                ),
            ),
            (
                "total_tax_due".to_string(),
                RenderValue::Decimal(draft.total_tax_due),
            ),
            (
                "creditable_tax_withheld".to_string(),
                RenderValue::Decimal(draft.creditable_tax_withheld),
            ),
            (
                "tax_paid_previous".to_string(),
                RenderValue::Decimal(if draft.is_amended {
                    draft.tax_paid_previous
                } else {
                    0.0
                }),
            ),
            (
                "other_tax_credit".to_string(),
                RenderValue::Decimal(draft.other_tax_credit),
            ),
            (
                "other_tax_credit_description".to_string(),
                RenderValue::Text(draft.other_tax_credit_description.clone()),
            ),
            (
                "total_tax_credits".to_string(),
                RenderValue::Decimal(draft.total_tax_credits),
            ),
            (
                "tax_payable".to_string(),
                RenderValue::Decimal(draft.tax_payable),
            ),
            (
                "surcharge".to_string(),
                RenderValue::Decimal(draft.surcharge),
            ),
            ("interest".to_string(), RenderValue::Decimal(draft.interest)),
            (
                "compromise".to_string(),
                RenderValue::Decimal(draft.compromise),
            ),
            (
                "total_penalties".to_string(),
                RenderValue::Decimal(draft.total_penalties),
            ),
            (
                "total_amount_payable".to_string(),
                RenderValue::Decimal(draft.total_amount_payable),
            ),
            (
                "overpayment_disposition".to_string(),
                RenderValue::Text(
                    match draft.overpayment_disposition {
                        OverpaymentDisposition::None => "none",
                        OverpaymentDisposition::Refund => "refund",
                        OverpaymentDisposition::TaxCreditCertificate => "tax_credit_certificate",
                    }
                    .to_string(),
                ),
            ),
        ]);

        if draft.schedule_1.len() > SCHEDULES[0].first_page_rows {
            let page_2_subtotal = (draft
                .schedule_1
                .iter()
                .take(SCHEDULES[0].first_page_rows)
                .map(|row| row.tax_due)
                .sum::<f64>()
                * 100.0)
                .round()
                / 100.0;
            envelope.fields.insert(
                "schedule_1_page_2_subtotal".to_string(),
                RenderValue::Decimal(page_2_subtotal),
            );
        }

        let rows = draft
            .schedule_1
            .iter()
            .enumerate()
            .map(|(index, row)| RenderRow {
                key: format!("schedule-1-{}-{}", index + 1, row.atc),
                cells: BTreeMap::from([
                    ("atc".to_string(), RenderValue::Text(row.atc.clone())),
                    (
                        "description".to_string(),
                        RenderValue::Text(row.atc_description.clone()),
                    ),
                    (
                        "taxable_amount".to_string(),
                        RenderValue::Decimal(row.taxable_amount),
                    ),
                    ("tax_rate".to_string(), RenderValue::Decimal(row.tax_rate)),
                    ("tax_due".to_string(), RenderValue::Decimal(row.tax_due)),
                ]),
            })
            .collect();

        envelope.schedules.push(RenderSchedule {
            id: SCHEDULES[0].id.to_string(),
            columns: vec![
                column("atc", "ATC", RenderAlignment::Left),
                column("description", "Tax Type", RenderAlignment::Left),
                column("taxable_amount", "Taxable Amount", RenderAlignment::Right),
                column("tax_rate", "Tax Rate", RenderAlignment::Right),
                column("tax_due", "Tax Due", RenderAlignment::Right),
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
                rule_version: "2551q-main-v1".to_string(),
            })
            .collect();

        envelope
    }
}

fn fixtures() -> Result<Vec<RenderContractFixture>, RenderProviderError> {
    let normal = fixture_with_rows(2)?;
    let schedule_capacity = fixture_with_rows(6)?;
    let schedule_overflow = fixture_with_rows(10)?;
    let minimum = minimum_fixture()?;
    let long_values = long_values_fixture()?;
    let validation_edge = validation_edge_fixture()?;

    Ok(vec![
        fixture("2551q-normal.json", RenderFixtureKind::Normal, true, normal),
        fixture(
            "2551q-6-rows.json",
            RenderFixtureKind::ScheduleCapacity,
            true,
            schedule_capacity,
        ),
        fixture(
            "2551q-10-rows.json",
            RenderFixtureKind::ValidationEdge,
            false,
            schedule_overflow,
        ),
        fixture(
            "2551q-minimum.json",
            RenderFixtureKind::Minimum,
            true,
            minimum,
        ),
        fixture(
            "2551q-long-values.json",
            RenderFixtureKind::LongValues,
            true,
            long_values,
        ),
        fixture(
            "2551q-validation-edge.json",
            RenderFixtureKind::ValidationEdge,
            false,
            validation_edge,
        ),
        fixture(
            "2551q-fiscal-period.json",
            RenderFixtureKind::Variant,
            true,
            fiscal_period_fixture()?,
        ),
        fixture(
            "2551q-tax-relief.json",
            RenderFixtureKind::Variant,
            true,
            tax_relief_fixture()?,
        ),
        fixture(
            "2551q-item13-eight-percent.json",
            RenderFixtureKind::Variant,
            true,
            eight_percent_fixture()?,
        ),
        fixture(
            "2551q-overpayment-refund.json",
            RenderFixtureKind::Variant,
            true,
            overpayment_fixture(OverpaymentDisposition::Refund)?,
        ),
        fixture(
            "2551q-overpayment-tcc.json",
            RenderFixtureKind::Variant,
            true,
            overpayment_fixture(OverpaymentDisposition::TaxCreditCertificate)?,
        ),
    ])
}

fn generated_artifacts() -> Result<Vec<GeneratedContractArtifact>, RenderProviderError> {
    Ok(vec![GeneratedContractArtifact {
        relative_path: "src/generated/2551q-atc-reference.json",
        value: json!({
            "schema_version": 1,
            "form_code": PROVIDER.code,
            "revision": PROVIDER.revision,
            "entries": ATC_TABLE_2551Q.iter().map(|entry| json!({
                "code": entry.code,
                "description": entry.description,
                "rate": entry.rate
            })).collect::<Vec<_>>()
        }),
    }])
}

fn fixture(
    file_name: &'static str,
    kind: RenderFixtureKind,
    expected_form_valid: bool,
    draft: Form2551QDraft,
) -> RenderContractFixture {
    RenderContractFixture {
        file_name,
        kind,
        expected_form_valid,
        envelope: RenderEnvelopeV1::from(&draft),
    }
}

fn fixture_with_rows(row_count: usize) -> Result<Form2551QDraft, RenderProviderError> {
    let mut draft: Form2551QDraft = serde_json::from_value(json!({
        "id": null,
        "tin": "12345678900000",
        "taxpayer_type": "Individual",
        "business_start_date": "2010-01-01",
        "taxable_year": 2026,
        "quarter": 1,
        "tax_period_basis": "calendar",
        "year_end_month": 12,
        "eopt_tier": null,
        "is_amended": true,
        "original_return_filed_and_paid_on_time": true,
        "number_of_attached_sheets": 0,
        "tax_relief": false,
        "tax_relief_specification": "",
        "item_13_election": "graduated",
        "annual_income_tax_election": "unrecorded",
        "rdo_code": "018",
        "taxpayer_name": "Andrea Mae Renderer Galang",
        "registered_address": "53 Santol Extension, New Cabalan, Olongapo City",
        "zip_code": "2200",
        "contact_number": "09123456789",
        "email": "renderer@example.com",
        "schedule_1": [],
        "total_tax_due": 0.0,
        "creditable_tax_withheld": 125.0,
        "tax_paid_previous": 50.0,
        "other_tax_credit": 25.0,
        "other_tax_credit_description": "Validated prior payment",
        "total_tax_credits": 200.0,
        "tax_payable": 0.0,
        "auto_compute_penalties": false,
        "surcharge": 10.0,
        "interest": 5.0,
        "compromise": 1000.0,
        "total_penalties": 1015.0,
        "total_amount_payable": 0.0,
        "overpayment_disposition": "none",
        "status": "Draft",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z",
        "submitted_at": null,
        "confirmed_at": null,
        "submission_filename": null,
        "receipt_id": null,
        "queued_submission_fingerprint": null,
        "submission_attempts": 0,
        "next_retry_at": null,
        "last_error": null,
        "carried_forward_from": null,
        "payment_receipt_path": null
    }))?;
    if row_count > FIXTURE_ATC_CODES.len() {
        return Err(RenderProviderError::Fixture(format!(
            "2551Q fixture requested {row_count} rows; only {} reviewed ATCs are defined",
            FIXTURE_ATC_CODES.len()
        )));
    }
    draft.schedule_1 = FIXTURE_ATC_CODES
        .iter()
        .take(row_count)
        .enumerate()
        .map(|(index, atc)| {
            let mut row = Schedule1Row::new(atc).ok_or_else(|| {
                RenderProviderError::Fixture(format!("unknown reviewed 2551Q fixture ATC {atc}"))
            })?;
            let intended_tax_due = (index + 1) as f64 * 300.0;
            row.taxable_amount = ((intended_tax_due / row.tax_rate) * 100.0).round() / 100.0;
            row.recompute();
            if row.tax_due != intended_tax_due {
                return Err(RenderProviderError::Fixture(format!(
                    "2551Q fixture ATC {atc} recomputed to {}, expected {intended_tax_due}",
                    row.tax_due
                )));
            }
            Ok(row)
        })
        .collect::<Result<Vec<_>, _>>()?;
    draft.ensure_required_schedule_attachment_sheets();
    draft.recompute(None);
    Ok(draft)
}

fn minimum_fixture() -> Result<Form2551QDraft, RenderProviderError> {
    let mut draft = fixture_with_rows(1)?;
    draft.taxpayer_name = "Short Taxpayer".to_string();
    draft.is_amended = false;
    draft.original_return_filed_and_paid_on_time = false;
    draft.number_of_attached_sheets = 0;
    draft.tax_relief = false;
    draft.tax_relief_specification.clear();
    draft.item_13_election = Item13Election::Graduated;
    draft.schedule_1 = vec![Schedule1Row::default_pt010()];
    draft.creditable_tax_withheld = 0.0;
    draft.tax_paid_previous = 0.0;
    draft.other_tax_credit = 0.0;
    draft.other_tax_credit_description.clear();
    draft.surcharge = 0.0;
    draft.interest = 0.0;
    draft.compromise = 0.0;
    draft.overpayment_disposition = OverpaymentDisposition::None;
    draft.recompute(None);
    Ok(draft)
}

fn long_values_fixture() -> Result<Form2551QDraft, RenderProviderError> {
    let mut draft = minimum_fixture()?;
    draft.taxpayer_name =
        "Long Registered Taxpayer Name That Exceeds The Official Comb".to_string();
    draft.registered_address = "Unit 1201, A Deliberately Long Registered Address For Renderer Overflow Verification, Olongapo City".to_string();
    draft.contact_number = "+639123456789".to_string();
    draft.email = "long.renderer.verification.address@example.test".to_string();
    draft.tax_relief = true;
    draft.tax_relief_specification =
        "Reviewed special-law description beyond the official comb".to_string();
    draft.schedule_1[0].taxable_amount = 100_000.0;
    draft.schedule_1[0].recompute();
    draft.other_tax_credit = 25.0;
    draft.other_tax_credit_description = LONG_ITEM_17_DESCRIPTION.to_string();
    draft.recompute(None);
    Ok(draft)
}

fn validation_edge_fixture() -> Result<Form2551QDraft, RenderProviderError> {
    let mut draft = minimum_fixture()?;
    draft.schedule_1.clear();
    draft.recompute(None);
    Ok(draft)
}

fn fiscal_period_fixture() -> Result<Form2551QDraft, RenderProviderError> {
    let mut draft = minimum_fixture()?;
    draft.tax_period_basis = TaxPeriodBasis::Fiscal;
    draft.year_end_month = 6;
    draft.quarter = 3;
    draft.annual_income_tax_election = Some(AnnualIncomeTaxElection::Graduated);
    draft.item_13_election = Item13Election::NotApplicable;
    Ok(draft)
}

fn tax_relief_fixture() -> Result<Form2551QDraft, RenderProviderError> {
    let mut draft = minimum_fixture()?;
    draft.tax_relief = true;
    draft.tax_relief_specification = "Special Law 123".to_string();
    Ok(draft)
}

fn eight_percent_fixture() -> Result<Form2551QDraft, RenderProviderError> {
    let mut draft = minimum_fixture()?;
    draft.item_13_election = Item13Election::EightPercent;
    Ok(draft)
}

fn overpayment_fixture(
    disposition: OverpaymentDisposition,
) -> Result<Form2551QDraft, RenderProviderError> {
    let mut draft = minimum_fixture()?;
    draft.schedule_1[0].taxable_amount = 100_000.0;
    draft.creditable_tax_withheld = 5_000.0;
    draft.recompute(None);
    if draft.total_amount_payable >= 0.0 {
        return Err(RenderProviderError::Fixture(
            "2551Q overpayment fixture did not produce an overpayment".to_string(),
        ));
    }
    draft.overpayment_disposition = disposition;
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
    fn taxpayer_name_fixtures_cover_short_exact_and_overflow_comb_states() {
        let short = minimum_fixture().expect("short fixture");
        let exact = fixture_with_rows(6).expect("exact-capacity fixture");
        let overflow = long_values_fixture().expect("overflow fixture");

        assert_eq!(short.taxpayer_name.chars().count(), 14);
        assert_eq!(exact.taxpayer_name.chars().count(), 26);
        assert!(overflow.taxpayer_name.chars().count() > 26);
    }

    #[test]
    fn ten_row_fixture_retains_every_atc_and_counts_its_continuation_sheet() {
        let draft = fixture_with_rows(10).expect("ten-row fixture");
        let envelope = RenderEnvelopeV1::from(&draft);
        let schedule = envelope
            .schedules
            .iter()
            .find(|schedule| schedule.id == "schedule_1")
            .expect("Schedule 1 contract");

        assert_eq!(draft.number_of_attached_sheets, 1);
        assert_eq!(schedule.rows.len(), 10);
        assert_eq!(draft.total_tax_due, 16_500.0);
        assert_eq!(
            envelope.fields["schedule_1_page_2_subtotal"],
            RenderValue::Decimal(6_300.0)
        );
        assert!(envelope.validation.iter().any(|issue| {
            issue.field_path == "schedule_1"
                && issue.message.contains("cannot be queued or submitted")
        }));
    }

    #[test]
    fn pdf417_runtime_provenance_matches_verified_official_xobjects() {
        let assets = runtime_discrete_assets();

        let seal = &assets[0];
        assert_eq!(seal["asset"], json!("government_seal"));
        assert_eq!(seal["source_pdf_object_id"], json!([9, 0]));
        assert_eq!(seal["source_pixel_dimensions"], json!([95, 83]));
        assert_eq!(seal["source_channels_equal"], json!(true));
        assert_eq!(
            seal["source_ctm_points"],
            json!([28.8, 0.0, 0.0, 25.2, 244.8, 888.96])
        );
        assert_eq!(
            seal["source_png_sha256"],
            json!("7db0df0c022263481d219eebc2077631866f626521b4fe93967910a6a9422a4f")
        );

        let expected = [
            (
                "static_form_pdf417_page_1",
                1,
                json!([35, 0]),
                "2551Q 01/18ENCS P1",
                491,
                "0b22b8418dc0dadb2043dd6022cb337e6fd60193b50105a3e7c47de3e565b9ca",
                "7ecda284c8c42dc6c64631313ff25e705c11450b9a1139223978f43e7d8168c2",
                "889982f23eeedeb85d61a4686e2737e2eddf66705df230afc71a5aca22656a9b",
            ),
            (
                "static_form_pdf417_page_2",
                2,
                json!([38, 0]),
                "2551Q 01/18ENCS P2",
                484,
                "b9257f08de39c25c64cbb7b8cbef835c060a5e347866c67b4cee558669f3233f",
                "d8c1d2fb417d8c4f83c147a5af7c3aee41f811beaa56ca39e20f77117c2c923e",
                "27639c545c7eeee18bcac00bd28e5b1dda3a7d9f7c5c3d11569c1a38d7423c69",
            ),
        ];

        for (asset, expected) in assets[1..].iter().zip(expected) {
            let (
                name,
                page,
                object_id,
                payload,
                black_modules,
                matrix_hash,
                path_hash,
                stream_hash,
            ) = expected;
            assert_eq!(asset["asset"], json!(name));
            assert_eq!(asset["source_page"], json!(page));
            assert_eq!(asset["source_pdf_object_id"], object_id);
            assert_eq!(asset["decoded_payload"], json!(payload));
            assert_eq!(asset["caption_text"], json!(payload));
            assert_eq!(asset["symbology"], json!("PDF417"));
            assert_eq!(asset["logical_dimensions"], json!([120, 7]));
            assert_eq!(asset["logical_black_modules"], json!(black_modules));
            assert_eq!(asset["logical_matrix_sha256"], json!(matrix_hash));
            assert_eq!(asset["logical_path_sha256"], json!(path_hash));
            assert_eq!(asset["source_stream_sha256"], json!(stream_hash));
            assert_eq!(
                asset["embedded_as"],
                json!("reviewed_inline_svg_module_matrix_with_live_caption")
            );
            assert_eq!(asset["caption_render_font"], json!("eBIRForms Arimo"));
            assert_eq!(asset["encoder_proof"]["module_differences"], json!(0));
            assert_eq!(
                asset["decoder_evidence"][0]["decoder"],
                json!("ZXing 3.5.3")
            );
        }
    }
}
