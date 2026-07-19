use bir_core::forms::form_1701q::{
    Form1701QAtc, Form1701QDeductionMethod, Form1701QFilerType, Form1701QParty,
    Form1701QPaymentRow, Form1701QSpouseType, Form1701QTaxRate,
};
use bir_core::forms::{Form1701QDraft, FormValidator};
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
        file_name: "1701q-2018-page-1.png",
        sha256: "dcd98d06dde89732f4d340d6add017e2bf4b7040f3eda755a618ba43f01172ef",
    },
    VisualReferencePage {
        page: 2,
        file_name: "1701q-2018-page-2.png",
        sha256: "d69c01b30c6f86a92d090b3f81965a6e5f1464444012c425cece6a1ac84e1bf8",
    },
];

/// Layout-only experimental provider for the locked January 2018 revision.
///
/// The reviewed source pack has no exact-revision saved XML. Consequently this
/// provider supplies semantic preview data but does not make an XML, queue, or
/// filing-readiness claim. Those gates remain false in the capability manifest.
pub(super) const PROVIDER: RenderFormProvider = RenderFormProvider {
    code: "1701Q",
    revision: "2018",
    form_id: "1701Qv2018",
    title: "Quarterly Income Tax Return for Individuals, Estates and Trusts",
    page_width_pt: RenderPageGeometry::LEGAL.width_points,
    page_height_pt: RenderPageGeometry::LEGAL.height_points,
    expected_base_page_count: 2,
    schedules: &[],
    visual_fixture_file_name: "1701q-normal.json",
    visual_fixture_sha256: "ca8a90050a22d5f18eaab4fcada5dfce5a074ec053c86ffa87d2694b9f9f12e8",
    official_source:
        "https://bir-cdn.bir.gov.ph/local/pdf/1701Q%20Jan%202018%20final%20rev2_copy.pdf",
    official_source_sha256: "c731d3f12556e6f19ab81f6113ca7c4a23f7ed099675c03451ac0074d96b85ed",
    reference_dpi: 144,
    reference_width_px: 1_224,
    reference_height_px: 1_872,
    visual_reference_pages: VISUAL_REFERENCE_PAGES,
    chromium_references: None,
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
            "derived_grayscale_pixel_sha256": "2ca1abd3e633cc7f5805e95642dee3ee0d8840336ec9163cb11eb1e138bba01c",
            "derived_png_sha256": "92b7a3fd81ee9db5705482563925d79842d1e961a5a0a931fc6d838ec7a1402e",
            "embedded_as": "lossless_grayscale_collapse_of_equal_rgb_channels",
            "embedded_color_space": "DeviceGray",
            "embedded_in": "packages/form-renderer/src/forms/assets/1701q-seal.png",
            "source_bbox_top_left_points": [236.07, 7.614, 32.373, 29.106],
            "source_channels_equal": true,
            "source_ctm_points": [32.373, 0.0, 0.0, 29.106, 236.07, 899.28],
            "source_decoded_rgb_sha256": "80ad6c1fa30a29d7795d475da6533459d1242ee2226b6cb23cf8ef6709cba5ce",
            "source_page": 1,
            "source_pdf_object_id": [53, 0],
            "source_pixel_dimensions": [119, 102],
            "source_png_sha256": "e667f9a75d1b1ab2d929c1ab0feb19edb3bca765336056054491a1cb8c25245a",
            "source_stream_sha256": "46f193f83e79047c3a1a4444b2004687938ae982a5a281e082db2a96c5074d04",
            "treatment": "lossless extraction of the exact official PDF image XObject; equal RGB channels collapsed to one native grayscale channel without crop, resampling, recoloring, thresholding, or substitution"
        }),
        json!({
            "asset": "static_form_pdf417_page_1",
            "caption_bbox_top_left_points": [512.02, 82.80728, 80.6508, 7.437],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "1701Q 01/18ENCS P1",
            "decoded_payload": "1701Q 01/18ENCS P1",
            "decoder_evidence": [
                {"decoder": "ZXing-C++ 3.1.0", "payload": "1701Q 01/18ENCS P1", "symbology": "PDF417"}
            ],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1701QAssets.ts",
            "encoder_proof": {
                "columns": 3,
                "encoding": "ISO-8859-1",
                "error_correction_level": 2,
                "implementation": "pdf417gen 0.8.1",
                "module_differences": 0,
                "rows": 7
            },
            "logical_black_modules": 480,
            "logical_dimensions": [120, 7],
            "logical_matrix_sha256": "81ffb136537f07b8b526cb6f5968802f20734fd3026269c1a4a0945a653788df",
            "logical_path_sha256": "6a2099665e11c70aad1943b919b44537946ca1fe59d19ab5f6926a45cc9af315",
            "source_active_bbox_top_left_points": [441.24, 46.68, 149.096296, 34.02],
            "source_active_pixel_bounds": [0, 0, 240, 63],
            "source_bbox_top_left_points": [441.24, 46.68, 150.96, 34.56],
            "source_ctm_points": [150.96, 0.0, 0.0, 34.56, 441.24, 854.76],
            "source_decoded_rgb_sha256": "b68a0106361e1ba84ddf9c224d0411060203178571540623184108eac11acca9",
            "source_page": 1,
            "source_pdf_object_id": [52, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 1, "right": 3},
            "source_pixel_dimensions": [243, 64],
            "source_png_sha256": "d5205e353f1259349d8217527540a3b12a3c24381cc52a51b773a3fc7c0be8f5",
            "source_stream_sha256": "d1b9fe8e37a75de40317f7e4cfed38f28988157f538e1742569aea4a6b0afc58",
            "symbology": "PDF417"
        }),
        json!({
            "asset": "static_form_pdf417_page_2",
            "caption_bbox_top_left_points": [514.66, 80.16728, 80.6508, 7.437],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "1701Q 01/18ENCS P2",
            "decoded_payload": "1701Q 01/18ENCS P2",
            "decoder_evidence": [
                {"decoder": "ZXing-C++ 3.1.0", "payload": "1701Q 01/18ENCS P2", "symbology": "PDF417"}
            ],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1701QAssets.ts",
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
            "logical_matrix_sha256": "9457e35a53d6a2a04442d9a0346a19b079ac11bda065e554e3f31d97f31d1120",
            "logical_path_sha256": "0cf6c87d5dd4f3a559689c8a99c10367c0038c3b1e0ebf11c67b9ab801c09ad5",
            "source_active_bbox_top_left_points": [444.24, 37.8, 149.333333, 39.2175],
            "source_active_pixel_bounds": [0, 0, 240, 63],
            "source_bbox_top_left_points": [444.24, 37.8, 151.2, 39.84],
            "source_ctm_points": [151.2, 0.0, 0.0, 39.84, 444.24, 858.36],
            "source_decoded_rgb_sha256": "520c2227868c9333bc1be1a2d7c4a6cf53f023d826b376cd411087954be3dd41",
            "source_page": 2,
            "source_pdf_object_id": [56, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 1, "right": 3},
            "source_pixel_dimensions": [243, 64],
            "source_png_sha256": "69ab932b888ff0a85aa1193675a9cf5b5870b8f7b0ae1da7c39bfdf663936665",
            "source_stream_sha256": "15f6981f745b30e6c9d0ffe25b6c9840ba0f9ab6eeab33a3203aeea70a5de24b",
            "symbology": "PDF417"
        }),
    ]
}

impl From<&Form1701QDraft> for RenderEnvelopeV1 {
    fn from(draft: &Form1701QDraft) -> Self {
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
                taxable_year: draft.taxable_year_u16(),
                month: None,
                quarter: Some(draft.quarter_u8()),
                label: draft.period_code(),
            },
        );

        insert_text(
            &mut envelope,
            "attached_sheets",
            format!("{:02}", draft.number_of_sheets),
        );
        insert_bool(&mut envelope, "is_amended", draft.is_amended);
        insert_bool(&mut envelope, "has_spouse", draft.has_spouse);
        if let Some(value) = draft.filer_type {
            insert_text(&mut envelope, "filer_type", filer_type_key(value));
        }
        if let Some(value) = draft.atc {
            insert_text(&mut envelope, "atc", value.code());
        }
        insert_text(&mut envelope, "date_of_birth", &draft.date_of_birth);
        insert_text(&mut envelope, "citizenship", &draft.citizenship);
        insert_text(
            &mut envelope,
            "foreign_tax_number",
            &draft.foreign_tax_number,
        );
        insert_optional_bool(
            &mut envelope,
            "claims_foreign_tax_credit",
            draft.claims_foreign_tax_credits,
        );
        insert_optional_tax_rate(&mut envelope, "tax_rate_choice", draft.tax_rate);
        insert_optional_deduction(&mut envelope, "deduction_method", draft.deduction_method);
        insert_text(
            &mut envelope,
            "registered_address_2",
            &draft.registered_address_2,
        );
        insert_text(
            &mut envelope,
            "taxpayer_last_name",
            &draft.taxpayer_last_name,
        );

        insert_text(&mut envelope, "spouse_tin", &draft.spouse_tin);
        insert_text(&mut envelope, "spouse_rdo_code", &draft.spouse_rdo_code);
        if let Some(value) = draft.spouse_type {
            insert_text(&mut envelope, "spouse_filer_type", spouse_type_key(value));
        }
        if let Some(value) = draft.spouse_atc {
            insert_text(&mut envelope, "spouse_atc", value.code());
        }
        insert_text(&mut envelope, "spouse_name", &draft.spouse_name);
        insert_text(
            &mut envelope,
            "spouse_citizenship",
            &draft.spouse_citizenship,
        );
        insert_text(
            &mut envelope,
            "spouse_foreign_tax_number",
            &draft.spouse_foreign_tax_number,
        );
        insert_optional_bool(
            &mut envelope,
            "spouse_claims_foreign_tax_credit",
            draft.spouse_claims_foreign_tax_credits,
        );
        insert_optional_tax_rate(
            &mut envelope,
            "spouse_tax_rate_choice",
            draft.spouse_tax_rate,
        );
        insert_optional_deduction(
            &mut envelope,
            "spouse_deduction_method",
            draft.spouse_deduction_method,
        );

        for item in (26..=30).chain(36..=68) {
            insert_optional_decimal(
                &mut envelope,
                &format!("item_{item}_taxpayer"),
                draft.amount(item, Form1701QParty::Taxpayer),
            );
            insert_optional_decimal(
                &mut envelope,
                &format!("item_{item}_spouse"),
                draft.amount(item, Form1701QParty::Spouse),
            );
        }
        insert_optional_decimal(
            &mut envelope,
            "item_31",
            draft.item_31_aggregate_amount_payable,
        );
        insert_text(
            &mut envelope,
            "item_43_description",
            &draft.item_43_non_operating_income_description,
        );
        insert_text(
            &mut envelope,
            "item_48_description",
            &draft.item_48_non_operating_income_description,
        );
        insert_text(
            &mut envelope,
            "item_61_description",
            &draft.item_61_other_tax_credit_description,
        );

        insert_payment_row(
            &mut envelope,
            "payment_32",
            &draft.payment_details.item_32_cash_or_bank_debit_memo,
        );
        insert_payment_row(
            &mut envelope,
            "payment_33",
            &draft.payment_details.item_33_check,
        );
        insert_payment_row(
            &mut envelope,
            "payment_34",
            &draft.payment_details.item_34_tax_debit_memo,
        );
        insert_payment_row(
            &mut envelope,
            "payment_35",
            &draft.payment_details.item_35_others,
        );
        insert_text(
            &mut envelope,
            "payment_35_particular",
            &draft.payment_details.item_35_others_description,
        );
        insert_text(
            &mut envelope,
            "machine_validation_or_receipt_details",
            &draft.payment_details.machine_validation_or_receipt_details,
        );

        envelope.validation = draft
            .validate()
            .into_iter()
            .map(|(field_path, message)| RenderValidationMessage {
                field_path,
                code: "invalid".to_string(),
                message,
                severity: RenderValidationSeverity::Error,
                rule_version: "1701q-2018-domain-v1".to_string(),
            })
            .collect();
        envelope
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

fn insert_optional_bool(envelope: &mut RenderEnvelopeV1, key: &str, value: Option<bool>) {
    if let Some(value) = value {
        insert_bool(envelope, key, value);
    }
}

fn insert_decimal(envelope: &mut RenderEnvelopeV1, key: &str, value: f64) {
    envelope
        .fields
        .insert(key.to_string(), RenderValue::Decimal(value));
}

fn insert_optional_decimal(envelope: &mut RenderEnvelopeV1, key: &str, value: Option<f64>) {
    if let Some(value) = value {
        insert_decimal(envelope, key, value);
    }
}

fn insert_optional_tax_rate(
    envelope: &mut RenderEnvelopeV1,
    key: &str,
    value: Option<Form1701QTaxRate>,
) {
    if let Some(value) = value {
        insert_text(
            envelope,
            key,
            match value {
                Form1701QTaxRate::Graduated => "graduated",
                Form1701QTaxRate::EightPercent => "eight_percent",
            },
        );
    }
}

fn insert_optional_deduction(
    envelope: &mut RenderEnvelopeV1,
    key: &str,
    value: Option<Form1701QDeductionMethod>,
) {
    if let Some(value) = value {
        insert_text(
            envelope,
            key,
            match value {
                Form1701QDeductionMethod::Itemized => "itemized",
                Form1701QDeductionMethod::Osd => "osd",
            },
        );
    }
}

const fn filer_type_key(value: Form1701QFilerType) -> &'static str {
    match value {
        Form1701QFilerType::SingleProprietor => "single_proprietor",
        Form1701QFilerType::Professional => "professional",
        Form1701QFilerType::Estate => "estate",
        Form1701QFilerType::Trust => "trust",
    }
}

const fn spouse_type_key(value: Form1701QSpouseType) -> &'static str {
    match value {
        Form1701QSpouseType::SingleProprietor => "single_proprietor",
        Form1701QSpouseType::Professional => "professional",
        Form1701QSpouseType::CompensationEarner => "compensation_earner",
    }
}

fn insert_payment_row(envelope: &mut RenderEnvelopeV1, key: &str, row: &Form1701QPaymentRow) {
    insert_text(envelope, &format!("{key}_bank"), &row.drawee_bank_or_agency);
    insert_text(envelope, &format!("{key}_number"), &row.number);
    insert_text(envelope, &format!("{key}_date"), &row.date);
    insert_optional_decimal(envelope, &format!("{key}_amount"), row.amount);
}

fn fixtures() -> Result<Vec<RenderContractFixture>, RenderProviderError> {
    Ok(vec![
        fixture(
            "1701q-minimum.json",
            RenderFixtureKind::Minimum,
            true,
            minimum_fixture(),
            FixtureFlavor::Minimum,
        ),
        fixture(
            "1701q-normal.json",
            RenderFixtureKind::Normal,
            true,
            normal_fixture(),
            FixtureFlavor::Normal,
        ),
        fixture(
            "1701q-long-values.json",
            RenderFixtureKind::LongValues,
            true,
            long_values_fixture(),
            FixtureFlavor::LongValues,
        ),
        fixture(
            "1701q-validation-edge.json",
            RenderFixtureKind::ValidationEdge,
            false,
            validation_edge_fixture(),
            FixtureFlavor::Minimum,
        ),
        fixture(
            "1701q-all-lines.json",
            RenderFixtureKind::ScheduleCapacity,
            true,
            normal_fixture(),
            FixtureFlavor::AllLines,
        ),
    ])
}

#[derive(Clone, Copy)]
enum FixtureFlavor {
    Minimum,
    Normal,
    LongValues,
    AllLines,
}

fn fixture(
    file_name: &'static str,
    kind: RenderFixtureKind,
    expected_form_valid: bool,
    draft: Form1701QDraft,
    flavor: FixtureFlavor,
) -> RenderContractFixture {
    let mut envelope = RenderEnvelopeV1::from(&draft);
    enrich_fixture(&mut envelope, flavor);
    RenderContractFixture {
        file_name,
        kind,
        expected_form_valid,
        envelope,
    }
}

fn base_fixture() -> Form1701QDraft {
    let mut draft = Form1701QDraft {
        tin: "12345678900000".to_string(),
        rdo_code: "018".to_string(),
        taxpayer_name: "JUAN DELA CRUZ".to_string(),
        taxpayer_last_name: "DELA CRUZ".to_string(),
        registered_address: "53 SANTOL EXTENSION, NEW CABALAN".to_string(),
        zip_code: "2200".to_string(),
        contact_number: "09123456789".to_string(),
        email: "renderer.1701q@example.com".to_string(),
        taxable_year: 2026,
        quarter: 2,
        filer_type: Some(Form1701QFilerType::SingleProprietor),
        atc: Some(Form1701QAtc::Ii012),
        claims_foreign_tax_credits: Some(false),
        tax_rate: Some(Form1701QTaxRate::Graduated),
        deduction_method: Some(Form1701QDeductionMethod::Osd),
        status: bir_core::forms::FilingStatus::Draft,
        ..Default::default()
    };
    draft.set_amount(36, Form1701QParty::Taxpayer, Some(500_000.0));
    draft.recompute();
    draft
}

fn minimum_fixture() -> Form1701QDraft {
    base_fixture()
}

fn normal_fixture() -> Form1701QDraft {
    base_fixture()
}

fn long_values_fixture() -> Form1701QDraft {
    let mut draft = base_fixture();
    draft.taxpayer_name = "JUAN MIGUEL ALEJANDRO DELA CRUZ-SANTOS WITH A VALID REGISTERED NAME LONGER THAN THE OFFICIAL COMB"
        .to_string();
    draft.taxpayer_last_name =
        "DELA CRUZ-SANTOS WITH A VALID LAST NAME LONGER THAN THE OFFICIAL COMB".to_string();
    draft.registered_address = "UNIT 1201, A DELIBERATELY LONG REGISTERED ADDRESS USED TO PROVE THAT THE HTML FORM PRESERVES EVERY VALID CHARACTER"
        .to_string();
    draft.email =
        "long.quarterly.income.tax.renderer.verification.address@example.test".to_string();
    draft
}

fn validation_edge_fixture() -> Form1701QDraft {
    Form1701QDraft {
        // Keep the exact revision/period renderable while exercising missing
        // identity and election validation. Unsupported periods are rejected
        // at the renderer boundary and therefore belong in contract tests,
        // not a geometry fixture.
        taxable_year: 2018,
        quarter: 3,
        status: bir_core::forms::FilingStatus::Draft,
        ..Default::default()
    }
}

fn enrich_fixture(envelope: &mut RenderEnvelopeV1, flavor: FixtureFlavor) {
    if matches!(flavor, FixtureFlavor::Minimum) {
        return;
    }

    insert_text(envelope, "attached_sheets", "02");
    insert_text(envelope, "date_of_birth", "01/15/1990");
    insert_text(envelope, "citizenship", "FILIPINO");
    insert_bool(envelope, "claims_foreign_tax_credit", false);
    insert_text(envelope, "registered_address_2", "OLONGAPO CITY, ZAMBALES");
    insert_text(envelope, "spouse_tin", "98765432100000");
    insert_text(envelope, "spouse_rdo_code", "018");
    insert_text(envelope, "spouse_filer_type", "professional");
    insert_text(envelope, "spouse_atc", "II014");
    insert_text(envelope, "spouse_name", "MARIA DELA CRUZ");
    insert_text(envelope, "spouse_citizenship", "FILIPINO");
    insert_bool(envelope, "spouse_claims_foreign_tax_credit", false);
    insert_text(envelope, "spouse_tax_rate_choice", "graduated");
    insert_text(envelope, "spouse_deduction_method", "itemized");

    let fill_all = matches!(flavor, FixtureFlavor::AllLines);
    for item in 36..=68 {
        if matches!(item, 39 | 40 | 46 | 54 | 62 | 63 | 67 | 68) || fill_all {
            insert_decimal(
                envelope,
                &format!("item_{item}_taxpayer"),
                f64::from(item) * 1_250.0,
            );
            insert_decimal(
                envelope,
                &format!("item_{item}_spouse"),
                f64::from(item) * 625.0,
            );
        }
    }
    for item in 26..=30 {
        insert_decimal(
            envelope,
            &format!("item_{item}_taxpayer"),
            f64::from(item) * 1_000.0,
        );
        insert_decimal(
            envelope,
            &format!("item_{item}_spouse"),
            f64::from(item) * 500.0,
        );
    }
    insert_decimal(envelope, "item_31", 45_000.0);

    insert_text(envelope, "payment_32_bank", "AAB 018");
    insert_text(envelope, "payment_32_number", "BDM-1701Q-001");
    insert_text(envelope, "payment_32_date", "08/15/2026");
    insert_decimal(envelope, "payment_32_amount", 45_000.0);

    if matches!(flavor, FixtureFlavor::LongValues) {
        insert_text(
            envelope,
            "registered_address_2",
            "BARANGAY NEW CABALAN, OLONGAPO CITY, ZAMBALES, PHILIPPINES",
        );
        insert_text(
            envelope,
            "spouse_name",
            "MARIA CONSOLACION REYES DELA CRUZ-SANTOS WITH A LONG REGISTERED NAME",
        );
        insert_text(
            envelope,
            "payment_32_bank",
            "AUTHORIZED AGENT BANK WITH A LONG REGISTERED BRANCH NAME",
        );
    }
}

fn generated_artifacts() -> Result<Vec<GeneratedContractArtifact>, RenderProviderError> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_maps_rust_owned_values_and_keeps_unentered_lines_absent() {
        let draft = normal_fixture();
        let envelope = RenderEnvelopeV1::from(&draft);
        assert_eq!(envelope.form.code, "1701Q");
        assert_eq!(envelope.form.version, "2018");
        assert_eq!(envelope.period.quarter, Some(2));
        assert_eq!(
            envelope.fields["item_26_taxpayer"],
            RenderValue::Decimal(
                draft
                    .amount(26, Form1701QParty::Taxpayer)
                    .expect("computed Item 26"),
            )
        );
        assert_eq!(
            envelope.fields["item_36_taxpayer"],
            RenderValue::Decimal(500_000.0)
        );
        assert!(!envelope.fields.contains_key("item_47_taxpayer"));
        assert!(envelope.schedules.is_empty());
    }

    #[test]
    fn fixtures_cover_two_page_layout_without_claiming_xml_readiness() {
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
                "{}: {:?}",
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
        assert_eq!(seal["source_pdf_object_id"], json!([53, 0]));
        assert_eq!(seal["source_pixel_dimensions"], json!([119, 102]));
        assert_eq!(seal["source_channels_equal"], json!(true));
        assert_eq!(
            seal["derived_png_sha256"],
            json!("92b7a3fd81ee9db5705482563925d79842d1e961a5a0a931fc6d838ec7a1402e")
        );

        for (asset, expected) in assets[1..].iter().zip([
            (
                "static_form_pdf417_page_1",
                1,
                52,
                "1701Q 01/18ENCS P1",
                480,
                "81ffb136537f07b8b526cb6f5968802f20734fd3026269c1a4a0945a653788df",
                "6a2099665e11c70aad1943b919b44537946ca1fe59d19ab5f6926a45cc9af315",
                json!([441.24, 46.68, 149.096296, 34.02]),
            ),
            (
                "static_form_pdf417_page_2",
                2,
                56,
                "1701Q 01/18ENCS P2",
                476,
                "9457e35a53d6a2a04442d9a0346a19b079ac11bda065e554e3f31d97f31d1120",
                "0cf6c87d5dd4f3a559689c8a99c10367c0038c3b1e0ebf11c67b9ab801c09ad5",
                json!([444.24, 37.8, 149.333333, 39.2175]),
            ),
        ]) {
            let (name, page, object, payload, black_modules, matrix_hash, path_hash, active_bbox) =
                expected;
            assert_eq!(asset["asset"], json!(name));
            assert_eq!(asset["source_page"], json!(page));
            assert_eq!(asset["source_pdf_object_id"], json!([object, 0]));
            assert_eq!(asset["source_pixel_dimensions"], json!([243, 64]));
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
                json!({"bottom": 1, "right": 3})
            );
            assert_eq!(asset["encoder_proof"]["module_differences"], json!(0));
        }
    }
}
