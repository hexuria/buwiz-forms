//! Exact January 2018 semantic HTML provider for the regular-rate corporate return.
//!
//! The reviewed editable save proves the fixed capacities represented by the
//! typed draft. It does not prove an electronic submission contract or any
//! continuation-sheet geometry, so this provider always emits exactly four
//! pages and no renderer-owned continuation schedules.

use bir_core::forms::{
    form_1702rt::{
        reviewed_alternate_atc_description, Form1702RTAtcSelection, Form1702RTDate,
        Form1702RTDeductionMethod, Form1702RTDraft, Form1702RTFilingBasis, Form1702RTNamedAmount,
        Form1702RTOverpaymentDisposition, Form1702RTPaymentDetail, Form1702RTSpecialDeductionRow,
        WholePeso, FORM_CODE, FORM_REVISION, OFFICIAL_FORM_SHA256,
    },
    FilingStatus, FormValidator,
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
        file_name: "1702rt-2018c-page-1.png",
        sha256: "25d7d93484fa8d13735dec7f9794dbc42155187b530029afdc969f4009fc203f",
    },
    VisualReferencePage {
        page: 2,
        file_name: "1702rt-2018c-page-2.png",
        sha256: "9c421bcac8b3aee9faf03b243e8ff0c3054c010da48e754d30d01fde8ad4699b",
    },
    VisualReferencePage {
        page: 3,
        file_name: "1702rt-2018c-page-3.png",
        sha256: "d0fef4856013f4645c4bd6dc8b635b1d76e155ae544c0bf82e339035e52c2d53",
    },
    VisualReferencePage {
        page: 4,
        file_name: "1702rt-2018c-page-4.png",
        sha256: "f9471368a54efcbe2fcab6744dfbf616111cf690d39438998fe83f9f9fe500a9",
    },
];

// Pinned by scripts/prepare_chromium_reference.mjs from the same official
// PDF bytes; see references/1702rt-2018c-chromium-source.json
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
            file_name: "1702rt-2018c-page-1-chromium.png",
            sha256: "92d9b7c133ac1d51c59f4248115ce2ef61ed57117b2458ba9b52d87a9a62ae80",
            vector_svg_sha256: "5f4849a5a103a0b77b48b36e2a7063c7142159f200663abfdf7383b3e7529c19",
            noise_floor_changed_pixels: 71_385,
        },
        super::ChromiumVisualReference {
            page: 2,
            file_name: "1702rt-2018c-page-2-chromium.png",
            sha256: "24b50f9ef8102a87d85fa6925d8f23c9891bb015b91516655f88831fe44f0771",
            vector_svg_sha256: "64cb9a1b4a1cdd583d6051498d2fa098a44ead6f14541bdfc700cacba70d523d",
            noise_floor_changed_pixels: 65_764,
        },
        super::ChromiumVisualReference {
            page: 3,
            file_name: "1702rt-2018c-page-3-chromium.png",
            sha256: "51fb0b5be80854e54b6732014944fde293b529559175ac112f686dd222f433f3",
            vector_svg_sha256: "be611f8c36cef1bea46e7880872f31a038efee724bd6433ccc848b2becb47e66",
            noise_floor_changed_pixels: 55_332,
        },
        super::ChromiumVisualReference {
            page: 4,
            file_name: "1702rt-2018c-page-4-chromium.png",
            sha256: "76f1b663d084ddbba131160e4475520d08ef9a37a52455969372ff72cad6ab04",
            vector_svg_sha256: "17270273c3c794349ea5e875d3f6160898625bdbbc3830b34a4994a7b6162ed3",
            noise_floor_changed_pixels: 54_257,
        },
    ],
};

pub(super) const PROVIDER: RenderFormProvider = RenderFormProvider {
    code: "1702RT",
    revision: "2018C",
    form_id: "1702RTv2018C",
    title: "Annual Income Tax Return for Corporation, Partnership and Other Non-Individual Taxpayer Subject Only to Regular Income Tax Rate",
    page_width_pt: RenderPageGeometry::LEGAL.width_points,
    page_height_pt: RenderPageGeometry::LEGAL.height_points,
    expected_base_page_count: 4,
    schedules: &[],
    visual_fixture_file_name: "1702rt-normal.json",
    visual_fixture_sha256: "c2506735870f32d9da7b3235022ccd6057b4d2ecfe40a2a8e23dd1122d294fd3",
    official_source: "https://bir-cdn.bir.gov.ph/local/pdf/1702-RT%20Jan%202018%20ENCS%20Final%20v3.pdf",
    official_source_sha256: OFFICIAL_FORM_SHA256,
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
            "derived_grayscale_pixel_sha256": "2ca1abd3e633cc7f5805e95642dee3ee0d8840336ec9163cb11eb1e138bba01c",
            "derived_png_sha256": "50d1fc573146e251138b78074b5790dd569f6dbde335feea908334adef4dd7b0",
            "embedded_as": "lossless_grayscale_collapse_of_equal_rgb_channels",
            "embedded_color_space": "DeviceGray",
            "embedded_in": "packages/form-renderer/src/forms/assets/1702rt-seal.png",
            "source_bbox_top_left_points": [228.52, 28.968, 33.682, 28.152],
            "source_channels_equal": true,
            "source_ctm_points": [33.682, 0.0, 0.0, 28.152, 228.52, 878.88],
            "source_decoded_rgb_sha256": "80ad6c1fa30a29d7795d475da6533459d1242ee2226b6cb23cf8ef6709cba5ce",
            "source_grayscale_level_count": 15,
            "source_page": 1,
            "source_pdf_object_id": [40, 0],
            "source_pixel_dimensions": [119, 102],
            "source_png_sha256": "e667f9a75d1b1ab2d929c1ab0feb19edb3bca765336056054491a1cb8c25245a",
            "source_stream_sha256": "46f193f83e79047c3a1a4444b2004687938ae982a5a281e082db2a96c5074d04",
            "source_raw_stream_sha256": "46f193f83e79047c3a1a4444b2004687938ae982a5a281e082db2a96c5074d04",
            "treatment": "lossless extraction of the exact official PDF image XObject; equal RGB channels collapsed to one native grayscale channel without crop, resampling, recoloring, thresholding, or substitution"
        }),
        json!({
            "asset": "static_form_pdf417_page_1",
            "caption_baseline_top_left_points": 116.659973,
            "caption_bbox_top_left_points": [503.380005, 109.383774, 87.750183, 8.964599],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "1702-RT 01/18ENCS P1",
            "decoded_payload": "1702-RT 01/18ENCS P1",
            "decoder_evidence": [
                {"decoder": "ZXing-C++ 3.1.0", "payload": "1702-RT 01/18ENCS P1", "symbology": "PDF417"}
            ],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1702RTAssets.ts",
            "encoder_proof": {
                "columns": 3,
                "encoding": "ISO-8859-1",
                "error_correction_level": 2,
                "implementation": "pdf417gen 0.8.1",
                "module_differences": 0,
                "rows": 8
            },
            "logical_black_modules": 555,
            "logical_dimensions": [120, 8],
            "logical_matrix_sha256": "362d6c13fc51ae71f86168da68bd2dfeadee77d82a0c68f5234a006adfaf6200",
            "logical_path_sha256": "82ad803cb29fb6886dd1bd0158c9528df4960c07016f7d807a1a54e6b761eb04",
            "source_active_bbox_top_left_points": [432.6, 70.2, 157.384232, 38.347397],
            "source_active_pixel_bounds": [0, 0, 240, 72],
            "source_bbox_top_left_points": [432.6, 70.2, 158.04, 38.88],
            "source_ctm_points": [158.04, 0.0, 0.0, 38.88, 432.6, 826.92],
            "source_decoded_rgb_sha256": "6755b04a9d0be0decaf57bd78eb31300ea917678eb9fee0f7049c598fe6a6bbf",
            "source_page": 1,
            "source_pdf_object_id": [41, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 1, "right": 1},
            "source_pixel_dimensions": [241, 73],
            "source_png_sha256": "e31dbc17fd55068af2edce3456fc14bafe324bf622405327f7378e65b83a9261",
            "source_stream_sha256": "7c6c6eed6c797324e189a2cefa42ff68e41e5061f0cc14ea4d8b22e1d9caab62",
            "source_raw_stream_sha256": "7c6c6eed6c797324e189a2cefa42ff68e41e5061f0cc14ea4d8b22e1d9caab62",
            "symbology": "PDF417"
        }),
        json!({
            "asset": "static_form_pdf417_page_2",
            "caption_baseline_top_left_points": 135.619995,
            "caption_bbox_top_left_points": [504.100006, 128.343796, 87.750214, 8.964599],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "1702-RT 01/18ENCS P2",
            "decoded_payload": "1702-RT 01/18ENCS P2",
            "decoder_evidence": [
                {"decoder": "ZXing-C++ 3.1.0", "payload": "1702-RT 01/18ENCS P2", "symbology": "PDF417"}
            ],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1702RTAssets.ts",
            "encoder_proof": {
                "columns": 3,
                "encoding": "ISO-8859-1",
                "error_correction_level": 2,
                "implementation": "pdf417gen 0.8.1",
                "module_differences": 0,
                "rows": 8
            },
            "logical_black_modules": 538,
            "logical_dimensions": [120, 8],
            "logical_matrix_sha256": "aaf9a1d313b3804a9ab4507407a98d8d5b32eb4711e8779cae8f1e2e7dbdc849",
            "logical_path_sha256": "352ade9e021b50d016521709013b6d278b130bf8cb2eaaec0a599e311cfd1dfc",
            "source_active_bbox_top_left_points": [433.2, 91.8, 157.742739, 35.980274],
            "source_active_pixel_bounds": [0, 0, 240, 72],
            "source_bbox_top_left_points": [433.2, 91.8, 158.4, 36.48],
            "source_ctm_points": [158.4, 0.0, 0.0, 36.48, 433.2, 807.72],
            "source_decoded_rgb_sha256": "6c8db6c55755e4d40fde871756aacfebebe5e1603bb66e35ec4ad82e9581266c",
            "source_page": 2,
            "source_pdf_object_id": [48, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 1, "right": 1},
            "source_pixel_dimensions": [241, 73],
            "source_png_sha256": "4c3d7ed213b3ac6a6c21a6c8f058469b5ab96299c9d2741a16e698a0b8628665",
            "source_stream_sha256": "c1fe6b467fc3cc48fedb022ab6700f0e97f88a2e3583a2bc73827348c3eca431",
            "source_raw_stream_sha256": "c1fe6b467fc3cc48fedb022ab6700f0e97f88a2e3583a2bc73827348c3eca431",
            "symbology": "PDF417"
        }),
        json!({
            "asset": "static_form_pdf417_page_3",
            "caption_baseline_top_left_points": 86.780029,
            "caption_bbox_top_left_points": [504.100006, 79.50383, 87.750214, 8.9646],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "1702-RT 01/18ENCS P3",
            "decoded_payload": "1702-RT 01/18ENCS P3",
            "decoder_evidence": [
                {"decoder": "ZXing-C++ 3.1.0", "payload": "1702-RT 01/18ENCS P3", "symbology": "PDF417"}
            ],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1702RTAssets.ts",
            "encoder_proof": {
                "columns": 3,
                "encoding": "ISO-8859-1",
                "error_correction_level": 2,
                "implementation": "pdf417gen 0.8.1",
                "module_differences": 0,
                "rows": 8
            },
            "logical_black_modules": 556,
            "logical_dimensions": [120, 8],
            "logical_matrix_sha256": "3ad58b30bbd02dab3dcfe7b896c861669a6f21693f60a04250e9c4e83000fa5f",
            "logical_path_sha256": "f8ca0babf7a0c25535927f096d9f7beeb68aaa9d1f4962c62b85158b0255f1b2",
            "source_active_bbox_top_left_points": [432.84, 41.16, 157.623237, 37.045479],
            "source_active_pixel_bounds": [0, 0, 240, 72],
            "source_bbox_top_left_points": [432.84, 41.16, 158.28, 37.56],
            "source_ctm_points": [158.28, 0.0, 0.0, 37.56, 432.84, 857.28],
            "source_decoded_rgb_sha256": "7bb3cdaec12a1471cabb7cb7bc5cce451be3c5411254278ac663799923770f3c",
            "source_page": 3,
            "source_pdf_object_id": [51, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 1, "right": 1},
            "source_pixel_dimensions": [241, 73],
            "source_png_sha256": "2ae9ed22ff2558984877dcab2ec74e6537aaba8f83bb45cb3254da93d5de4954",
            "source_stream_sha256": "335a41612975234b8f8a1d82af748f8a10c3d2e92f44bd83bb26fc96c5f90f61",
            "source_raw_stream_sha256": "335a41612975234b8f8a1d82af748f8a10c3d2e92f44bd83bb26fc96c5f90f61",
            "symbology": "PDF417"
        }),
        json!({
            "asset": "static_form_pdf417_page_4",
            "caption_baseline_top_left_points": 86.780029,
            "caption_bbox_top_left_points": [503.380005, 79.50383, 87.750183, 8.9646],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "1702-RT 01/18ENCS P4",
            "decoded_payload": "1702-RT 01/18ENCS P4",
            "decoder_evidence": [
                {"decoder": "ZXing-C++ 3.1.0", "payload": "1702-RT 01/18ENCS P4", "symbology": "PDF417"}
            ],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1702RTAssets.ts",
            "encoder_proof": {
                "columns": 3,
                "encoding": "ISO-8859-1",
                "error_correction_level": 2,
                "implementation": "pdf417gen 0.8.1",
                "module_differences": 0,
                "rows": 8
            },
            "logical_black_modules": 558,
            "logical_dimensions": [120, 8],
            "logical_matrix_sha256": "94b3aaadd3c8dcfbfca4ba4fb64347ceef5d1a7693f7d7d96a9f4d1e9bf5a8db",
            "logical_path_sha256": "e52b63b2670d75324b1724a2ae3cc627c024197c5dc20288419f1a2eb9525d4b",
            "source_active_bbox_top_left_points": [432.6, 43.2, 157.742739, 36.216986],
            "source_active_pixel_bounds": [0, 0, 240, 72],
            "source_bbox_top_left_points": [432.6, 43.2, 158.4, 36.72],
            "source_ctm_points": [158.4, 0.0, 0.0, 36.72, 432.6, 856.08],
            "source_decoded_rgb_sha256": "8012fa7463000ecf9ebe5a64679448c6de63c8f92eecfa24d1a7bf601025ce2c",
            "source_page": 4,
            "source_pdf_object_id": [54, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 1, "right": 1},
            "source_pixel_dimensions": [241, 73],
            "source_png_sha256": "4dda31b5ce31029f31a6595882f3aa034b4e43239761cfcd9c0dc34115c57de4",
            "source_stream_sha256": "195e85f9243181e7a20f1d818156bd48b00962c07c7ef1bdf55ace9cd3a3150f",
            "source_raw_stream_sha256": "195e85f9243181e7a20f1d818156bd48b00962c07c7ef1bdf55ace9cd3a3150f",
            "symbology": "PDF417"
        }),
    ]
}

impl From<&Form1702RTDraft> for RenderEnvelopeV1 {
    fn from(draft: &Form1702RTDraft) -> Self {
        let mut envelope = Self::new(
            RenderFormIdentity {
                code: FORM_CODE.to_string(),
                version: FORM_REVISION.to_string(),
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
                Form1702RTFilingBasis::Calendar => "calendar",
                Form1702RTFilingBasis::Fiscal => "fiscal",
            },
        );
        insert_bool(&mut envelope, "is_amended", draft.is_amended);
        insert_bool(&mut envelope, "is_short_period", draft.is_short_period);
        insert_bool(
            &mut envelope,
            "atc_mcit_selected",
            draft.atc.printed_mcit_selected,
        );
        insert_bool(
            &mut envelope,
            "atc_other_selected",
            draft.atc.other_selected,
        );
        insert_text(&mut envelope, "atc_other_code", &draft.atc.other_code);
        insert_text(
            &mut envelope,
            "atc_other_description",
            if draft.atc.other_selected {
                reviewed_alternate_atc_description(&draft.atc.other_code).unwrap_or_default()
            } else {
                ""
            },
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
        insert_text(
            &mut envelope,
            "incorporation_date",
            draft
                .incorporation_date
                .map(|date| date.to_string())
                .unwrap_or_default(),
        );
        insert_text(
            &mut envelope,
            "deduction_method",
            match draft.deduction_method {
                Form1702RTDeductionMethod::Itemized => "itemized",
                Form1702RTDeductionMethod::OptionalStandard => "osd",
                Form1702RTDeductionMethod::Unresolved => "unresolved",
            },
        );
        insert_text(
            &mut envelope,
            "number_of_attachments",
            &draft.number_of_attachments,
        );
        insert_text(
            &mut envelope,
            "president_signature",
            &draft.president_signature,
        );
        insert_text(
            &mut envelope,
            "treasurer_signature",
            &draft.treasurer_signature,
        );
        insert_text(
            &mut envelope,
            "president_signatory_title",
            &draft.president_signatory_title,
        );
        insert_text(
            &mut envelope,
            "president_signatory_tin",
            &draft.president_signatory_tin,
        );
        insert_text(
            &mut envelope,
            "treasurer_signatory_title",
            &draft.treasurer_signatory_title,
        );
        insert_text(
            &mut envelope,
            "treasurer_signatory_tin",
            &draft.treasurer_signatory_tin,
        );

        map_part_two(&mut envelope, draft);
        map_payment_details(&mut envelope, draft);
        map_part_four(&mut envelope, draft);
        map_part_five(&mut envelope, draft);
        map_schedule_one(&mut envelope, draft);
        map_schedule_two(&mut envelope, draft);
        map_schedule_three(&mut envelope, draft);
        map_schedule_four(&mut envelope, draft);
        map_schedule_five(&mut envelope, draft);

        envelope.validation = draft
            .validate()
            .into_iter()
            .map(|(field_path, message)| RenderValidationMessage {
                field_path,
                code: "invalid".to_string(),
                message,
                severity: RenderValidationSeverity::Error,
                rule_version: "1702rt-2018c-domain-v1".to_string(),
            })
            .collect();
        envelope
    }
}

fn map_part_two(envelope: &mut RenderEnvelopeV1, draft: &Form1702RTDraft) {
    let part = &draft.part_ii;
    for (item, amount) in [
        (14, part.item_14_tax_due),
        (15, part.item_15_total_tax_credits),
        (16, part.item_16_net_tax_payable_or_overpayment),
        (17, part.item_17_surcharge),
        (18, part.item_18_interest),
        (19, part.item_19_compromise),
        (20, part.item_20_total_penalties),
        (21, part.item_21_total_amount_payable_or_overpayment),
    ] {
        insert_money(envelope, &format!("item_{item}"), amount);
    }
    insert_text(
        envelope,
        "overpayment_disposition",
        match part.overpayment_disposition {
            Some(Form1702RTOverpaymentDisposition::Refund) => "refund",
            Some(Form1702RTOverpaymentDisposition::TaxCreditCertificate) => "tcc",
            Some(Form1702RTOverpaymentDisposition::CarryOver) => "carry_over",
            None => "",
        },
    );
}

fn map_payment_details(envelope: &mut RenderEnvelopeV1, draft: &Form1702RTDraft) {
    for (item, row) in (23..=24).zip(draft.payment_details[..2].iter()) {
        let prefix = format!("payment_{item}");
        insert_text(
            envelope,
            &format!("{prefix}_bank"),
            &row.drawee_bank_or_agency,
        );
        insert_text(envelope, &format!("{prefix}_number"), &row.number);
        insert_text(
            envelope,
            &format!("{prefix}_date"),
            row.date.map(|date| date.to_string()).unwrap_or_default(),
        );
        insert_money(envelope, &format!("{prefix}_amount"), row.amount);
    }

    let tax_debit = &draft.payment_details[2];
    insert_text(envelope, "payment_25_number", &tax_debit.number);
    insert_text(
        envelope,
        "payment_25_date",
        tax_debit
            .date
            .map(|date| date.to_string())
            .unwrap_or_default(),
    );
    insert_money(envelope, "payment_25_amount", tax_debit.amount);

    let other = &draft.payment_details[3];
    insert_text(envelope, "payment_26_specification", &other.specification);
    insert_text(envelope, "payment_26_bank", &other.drawee_bank_or_agency);
    insert_text(envelope, "payment_26_number", &other.number);
    insert_text(
        envelope,
        "payment_26_date",
        other.date.map(|date| date.to_string()).unwrap_or_default(),
    );
    insert_money(envelope, "payment_26_amount", other.amount);
}

fn map_part_four(envelope: &mut RenderEnvelopeV1, draft: &Form1702RTDraft) {
    let part = &draft.part_iv;
    for (item, amount) in [
        (27, part.item_27_sales),
        (28, part.item_28_sales_returns),
        (29, part.item_29_net_sales),
        (30, part.item_30_cost_of_sales_or_services),
        (31, part.item_31_gross_income_from_operations),
        (32, part.item_32_other_taxable_income),
        (33, part.item_33_total_taxable_income),
        (34, part.item_34_ordinary_itemized_deductions),
        (35, part.item_35_special_itemized_deductions),
        (36, part.item_36_nolco),
        (37, part.item_37_total_itemized_deductions),
        (38, part.item_38_optional_standard_deduction),
        (39, part.item_39_net_taxable_income_or_loss),
        (41, part.item_41_normal_income_tax_due),
        (42, part.item_42_mcit_due),
        (43, part.item_43_tax_due),
        (44, part.tax_credits.item_44_prior_year_excess_credits),
        (45, part.tax_credits.item_45_previous_quarter_mcit_payments),
        (
            46,
            part.tax_credits.item_46_previous_quarter_regular_payments,
        ),
        (47, part.tax_credits.item_47_excess_mcit_applied),
        (48, part.tax_credits.item_48_previous_quarter_withholding),
        (49, part.tax_credits.item_49_fourth_quarter_withholding),
        (50, part.tax_credits.item_50_foreign_tax_credits),
        (51, part.tax_credits.item_51_tax_paid_on_previous_return),
        (52, part.tax_credits.item_52_special_tax_credits),
        (53, part.tax_credits.item_53_other.amount),
        (54, part.tax_credits.item_54_other.amount),
        (55, part.tax_credits.item_55_total),
        (56, part.item_56_net_tax_payable_or_overpayment),
    ] {
        insert_money(envelope, &format!("item_{item}"), amount);
    }
    insert_integer(
        envelope,
        "item_40_rate_percent",
        i64::from(part.item_40_income_tax_rate_percent),
    );
    insert_text(
        envelope,
        "item_53_description",
        &part.tax_credits.item_53_other.description,
    );
    insert_text(
        envelope,
        "item_54_description",
        &part.tax_credits.item_54_other.description,
    );
}

fn map_part_five(envelope: &mut RenderEnvelopeV1, draft: &Form1702RTDraft) {
    for (item, amount) in [
        (
            57,
            draft.part_v.item_57_special_allowable_deductions_tax_effect,
        ),
        (58, draft.part_v.item_58_special_tax_credits),
        (59, draft.part_v.item_59_total_tax_relief),
    ] {
        insert_money(envelope, &format!("item_{item}"), amount);
    }
}

fn map_schedule_one(envelope: &mut RenderEnvelopeV1, draft: &Form1702RTDraft) {
    let source_amounts = draft.schedule_1.source_amounts();
    for (item, amount) in source_amounts[..16].iter().copied().enumerate() {
        insert_money(envelope, &format!("schedule_1_item_{}", item + 1), amount);
    }
    for (suffix, amount) in ['a', 'b', 'c']
        .into_iter()
        .zip(source_amounts[16..].iter().copied())
    {
        insert_money(envelope, &format!("schedule_1_item_17{suffix}"), amount);
    }
    for (index, row) in draft.schedule_1.other.iter().enumerate() {
        let suffix = char::from(b'd' + u8::try_from(index).unwrap_or_default());
        insert_named_amount(envelope, &format!("schedule_1_item_17{suffix}"), row);
    }
    insert_money(
        envelope,
        "schedule_1_item_18",
        draft.schedule_1.item_18_total,
    );
}

fn map_schedule_two(envelope: &mut RenderEnvelopeV1, draft: &Form1702RTDraft) {
    for (index, row) in draft.schedule_2.rows.iter().enumerate() {
        let prefix = format!("schedule_2_item_{}", index + 1);
        insert_text(envelope, &format!("{prefix}_description"), &row.description);
        insert_text(envelope, &format!("{prefix}_legal_basis"), &row.legal_basis);
        insert_money(envelope, &format!("{prefix}_amount"), row.amount);
    }
    insert_money(envelope, "schedule_2_item_5", draft.schedule_2.item_5_total);
}

fn map_schedule_three(envelope: &mut RenderEnvelopeV1, draft: &Form1702RTDraft) {
    for (item, amount) in [
        (1, draft.schedule_3.item_1_gross_income),
        (2, draft.schedule_3.item_2_ordinary_deductions),
        (3, draft.schedule_3.item_3_net_operating_loss),
    ] {
        insert_money(envelope, &format!("schedule_3_item_{item}"), amount);
    }
    for (index, row) in draft.schedule_3.rows.iter().enumerate() {
        let item = index + 4;
        let prefix = format!("schedule_3_item_{item}");
        insert_text(envelope, &format!("{prefix}_year"), &row.year_incurred);
        insert_money(envelope, &format!("{prefix}_amount"), row.amount);
        insert_money(
            envelope,
            &format!("{prefix}_applied_previous"),
            row.applied_previous_years,
        );
        insert_money(envelope, &format!("{prefix}_expired"), row.expired);
        insert_money(
            envelope,
            &format!("{prefix}_applied_current"),
            row.applied_current_year,
        );
        insert_money(
            envelope,
            &format!("{prefix}_unapplied"),
            row.unapplied_balance,
        );
    }
    insert_money(
        envelope,
        "schedule_3_item_8",
        draft.schedule_3.item_8_total_applied_current_year,
    );
}

fn map_schedule_four(envelope: &mut RenderEnvelopeV1, draft: &Form1702RTDraft) {
    for (index, row) in draft.schedule_4.rows.iter().enumerate() {
        let prefix = format!("schedule_4_item_{}", index + 1);
        insert_text(envelope, &format!("{prefix}_year"), &row.year);
        insert_money(
            envelope,
            &format!("{prefix}_normal_tax"),
            row.normal_income_tax,
        );
        insert_money(envelope, &format!("{prefix}_mcit"), row.mcit);
        insert_money(envelope, &format!("{prefix}_excess"), row.excess_mcit);
        insert_money(
            envelope,
            &format!("{prefix}_applied_previous"),
            row.applied_previous_years,
        );
        insert_money(envelope, &format!("{prefix}_expired"), row.expired);
        insert_money(
            envelope,
            &format!("{prefix}_applied_current"),
            row.applied_current_year,
        );
        insert_money(
            envelope,
            &format!("{prefix}_balance"),
            row.allowable_balance,
        );
    }
    insert_money(
        envelope,
        "schedule_4_item_4",
        draft.schedule_4.item_4_total_applied_current_year,
    );
}

fn map_schedule_five(envelope: &mut RenderEnvelopeV1, draft: &Form1702RTDraft) {
    insert_money(
        envelope,
        "schedule_5_item_1",
        draft.schedule_5.item_1_net_income_or_loss_per_books,
    );
    for (offset, row) in draft.schedule_5.additions.iter().enumerate() {
        insert_named_amount(envelope, &format!("schedule_5_item_{}", offset + 2), row);
    }
    insert_money(envelope, "schedule_5_item_4", draft.schedule_5.item_4_total);
    for (offset, row) in draft.schedule_5.non_taxable_income.iter().enumerate() {
        insert_named_amount(envelope, &format!("schedule_5_item_{}", offset + 5), row);
    }
    for (offset, row) in draft.schedule_5.special_deductions.iter().enumerate() {
        insert_named_amount(envelope, &format!("schedule_5_item_{}", offset + 7), row);
    }
    insert_money(envelope, "schedule_5_item_9", draft.schedule_5.item_9_total);
    insert_money(
        envelope,
        "schedule_5_item_10",
        draft.schedule_5.item_10_net_taxable_income_or_loss,
    );
}

fn insert_named_amount(envelope: &mut RenderEnvelopeV1, prefix: &str, row: &Form1702RTNamedAmount) {
    insert_text(envelope, &format!("{prefix}_description"), &row.description);
    insert_money(envelope, &format!("{prefix}_amount"), row.amount);
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

fn insert_money(envelope: &mut RenderEnvelopeV1, key: &str, value: WholePeso) {
    insert_integer(envelope, key, value.0);
}

fn fixtures() -> Result<Vec<RenderContractFixture>, RenderProviderError> {
    Ok(vec![
        fixture(
            "1702rt-minimum.json",
            RenderFixtureKind::Minimum,
            true,
            minimum_fixture(),
        ),
        fixture(
            "1702rt-normal.json",
            RenderFixtureKind::Normal,
            true,
            normal_fixture(),
        ),
        fixture(
            "1702rt-long-values.json",
            RenderFixtureKind::LongValues,
            true,
            long_values_fixture(),
        ),
        fixture(
            "1702rt-validation-edge.json",
            RenderFixtureKind::ValidationEdge,
            false,
            validation_edge_fixture(),
        ),
        fixture(
            "1702rt-schedule-capacity.json",
            RenderFixtureKind::ScheduleCapacity,
            true,
            schedule_capacity_fixture(),
        ),
    ])
}

fn fixture(
    file_name: &'static str,
    kind: RenderFixtureKind,
    expected_form_valid: bool,
    draft: Form1702RTDraft,
) -> RenderContractFixture {
    RenderContractFixture {
        file_name,
        kind,
        expected_form_valid,
        envelope: RenderEnvelopeV1::from(&draft),
    }
}

fn base_fixture() -> Form1702RTDraft {
    let mut draft = Form1702RTDraft {
        tin: "12345678900000".to_string(),
        taxable_year: 2026,
        month: 12,
        filing_basis: Form1702RTFilingBasis::Calendar,
        atc: Form1702RTAtcSelection {
            printed_mcit_selected: true,
            ..Default::default()
        },
        rdo_code: "018".to_string(),
        taxpayer_name: "GOLDCODERS CORPORATION".to_string(),
        registered_name_lines: [
            "GOLDCODERS CORPORATION".to_string(),
            String::new(),
            String::new(),
        ],
        registered_address: "81 SAN NICOLAS STREET, OLONGAPO CITY".to_string(),
        registered_address_lines: [
            "81 SAN NICOLAS STREET".to_string(),
            "OLONGAPO CITY, ZAMBALES".to_string(),
            String::new(),
        ],
        zip_code: "2200".to_string(),
        contact_number: "09123456789".to_string(),
        email: "corporate.renderer@example.com".to_string(),
        deduction_method: Form1702RTDeductionMethod::Itemized,
        number_of_attachments: "000".to_string(),
        xml_final_flag: "1".to_string(),
        status: FilingStatus::Draft,
        ..Default::default()
    };
    draft.part_iv.item_40_income_tax_rate_percent = 30;
    draft.recompute();
    draft
}

fn minimum_fixture() -> Form1702RTDraft {
    base_fixture()
}

fn normal_fixture() -> Form1702RTDraft {
    let mut draft = base_fixture();
    draft.atc.other_selected = true;
    draft.atc.other_code = "IC010".to_string();
    draft.incorporation_date = Form1702RTDate::new(2019, 12, 10).ok();
    draft.number_of_attachments = "003".to_string();
    draft.schedule_1.salaries_wages_allowances = WholePeso(240_000);
    draft.schedule_1.rental = WholePeso(120_000);
    draft.schedule_1.taxes_and_licenses = WholePeso(30_000);
    draft.schedule_1.professional_fees = WholePeso(40_000);
    draft.schedule_2.rows[0] = Form1702RTSpecialDeductionRow {
        description: "TRAINING INCENTIVE".to_string(),
        legal_basis: "SPECIAL LAW".to_string(),
        amount: WholePeso(25_000),
    };
    draft.part_iv.item_27_sales = WholePeso(2_400_000);
    draft.part_iv.item_28_sales_returns = WholePeso(100_000);
    draft.part_iv.item_30_cost_of_sales_or_services = WholePeso(700_000);
    draft.part_iv.item_32_other_taxable_income = WholePeso(50_000);
    draft.part_iv.tax_credits.item_44_prior_year_excess_credits = WholePeso(20_000);
    draft.part_iv.tax_credits.item_49_fourth_quarter_withholding = WholePeso(45_000);
    draft.part_ii.item_17_surcharge = WholePeso(1_000);
    draft.payment_details[0] = Form1702RTPaymentDetail {
        drawee_bank_or_agency: "AUTHORIZED AGENT BANK 018".to_string(),
        number: "BDM-1702RT-001".to_string(),
        date: Form1702RTDate::new(2027, 4, 15).ok(),
        amount: WholePeso(100_000),
        ..Default::default()
    };
    draft.payment_details[2] = Form1702RTPaymentDetail {
        number: "TDM-1702RT-025".to_string(),
        date: Form1702RTDate::new(2027, 4, 16).ok(),
        amount: WholePeso(25_000),
        ..Default::default()
    };
    draft.payment_details[3] = Form1702RTPaymentDetail {
        specification: "OTHER REVIEWED PAYMENT".to_string(),
        drawee_bank_or_agency: "AUTHORIZED AGENT BANK 026".to_string(),
        number: "OTHER-1702RT-026".to_string(),
        date: Form1702RTDate::new(2027, 4, 17).ok(),
        amount: WholePeso(26_000),
    };
    draft.president_signatory_title = "PRESIDENT".to_string();
    draft.president_signatory_tin = "12345678900000".to_string();
    draft.treasurer_signatory_title = "TREASURER".to_string();
    draft.treasurer_signatory_tin = "98765432100000".to_string();
    draft.recompute();
    draft
}

fn long_values_fixture() -> Form1702RTDraft {
    let mut draft = normal_fixture();
    draft.taxpayer_name = "GOLDCODERS TECHNOLOGY CONSULTING AND BUSINESS PROCESS SERVICES CORPORATION WITH A VALID REGISTERED NAME LONGER THAN THE OFFICIAL COMB".to_string();
    draft.registered_name_lines = [
        draft.taxpayer_name.clone(),
        "A SECOND REVIEWED NAME LINE THAT MUST NEVER BE TRUNCATED BY THE HTML RENDERER".to_string(),
        "FINAL REGISTERED NAME LINE".to_string(),
    ];
    draft.registered_address = "UNIT 1201, A DELIBERATELY LONG REGISTERED CORPORATE ADDRESS USED TO PROVE THAT EVERY VALID CHARACTER SURVIVES PREVIEW, PRINT, AND PDF EXPORT".to_string();
    draft.registered_address_lines = [
        draft.registered_address.clone(),
        "BARANGAY NEW CABALAN, OLONGAPO CITY, ZAMBALES, PHILIPPINES".to_string(),
        "ADDITIONAL REVIEWED ADDRESS LINE".to_string(),
    ];
    draft.email =
        "annual.regular.rate.corporate.renderer.verification.address@example.test".to_string();
    draft.schedule_1.other[0] = Form1702RTNamedAmount {
        description: "A VALID OTHER DEDUCTION DESCRIPTION LONGER THAN THE OFFICIAL COMB CAPACITY"
            .to_string(),
        amount: WholePeso(12_345),
    };
    draft.schedule_2.rows[0] = Form1702RTSpecialDeductionRow {
        description: "A LONG SPECIAL ALLOWABLE ITEMIZED DEDUCTION DESCRIPTION".to_string(),
        legal_basis: "REVIEWED SPECIAL LAW LEGAL BASIS WITH A LONG CAPTION".to_string(),
        amount: WholePeso(22_222),
    };
    draft.payment_details[0].drawee_bank_or_agency =
        "AUTHORIZED AGENT BANK WITH A LONG REGISTERED CORPORATE BRANCH NAME".to_string();
    draft.part_iv.tax_credits.item_53_other = Form1702RTNamedAmount {
        description: "OTHER REVIEWED CREDIT DESCRIPTION THAT EXCEEDS THE PRINTED COMB".to_string(),
        amount: WholePeso(10_000),
    };
    draft.recompute();
    draft
}

fn validation_edge_fixture() -> Form1702RTDraft {
    Form1702RTDraft {
        taxable_year: 2018,
        month: 12,
        number_of_attachments: "000".to_string(),
        xml_final_flag: "1".to_string(),
        status: FilingStatus::Draft,
        ..Default::default()
    }
}

fn schedule_capacity_fixture() -> Form1702RTDraft {
    let mut draft = normal_fixture();
    for (index, row) in draft.schedule_1.other.iter_mut().enumerate() {
        row.description = format!("OTHER DEDUCTION {}", index + 1);
        row.amount = WholePeso(i64::try_from(index + 1).unwrap_or_default() * 1_000);
    }
    for (index, row) in draft.schedule_2.rows.iter_mut().enumerate() {
        row.description = format!("SPECIAL DEDUCTION {}", index + 1);
        row.legal_basis = format!("LEGAL BASIS {}", index + 1);
        row.amount = WholePeso(i64::try_from(index + 1).unwrap_or_default() * 2_000);
    }
    for (index, row) in draft.schedule_3.rows.iter_mut().enumerate() {
        row.year_incurred = (2022 + index).to_string();
        row.amount = WholePeso(60_000 + i64::try_from(index).unwrap_or_default() * 10_000);
        row.applied_previous_years = WholePeso(5_000);
        row.expired = WholePeso(1_000);
        row.applied_current_year = WholePeso(2_000);
    }
    for (index, row) in draft.schedule_4.rows.iter_mut().enumerate() {
        row.year = (2023 + index).to_string();
        row.normal_income_tax = WholePeso(100_000);
        row.mcit = WholePeso(120_000);
        row.excess_mcit = WholePeso(20_000);
        row.applied_previous_years = WholePeso(2_000);
        row.expired = WholePeso(1_000);
        row.applied_current_year = WholePeso(3_000);
    }
    draft.schedule_5.item_1_net_income_or_loss_per_books = WholePeso(900_000);
    for (index, row) in draft.schedule_5.additions.iter_mut().enumerate() {
        row.description = format!("NON-DEDUCTIBLE EXPENSE {}", index + 1);
        row.amount = WholePeso(10_000);
    }
    for (index, row) in draft.schedule_5.non_taxable_income.iter_mut().enumerate() {
        row.description = format!("NON-TAXABLE INCOME {}", index + 1);
        row.amount = WholePeso(5_000);
    }
    for (index, row) in draft.schedule_5.special_deductions.iter_mut().enumerate() {
        row.description = format!("SPECIAL DEDUCTION {}", index + 1);
        row.amount = WholePeso(3_000);
    }
    draft.recompute();
    draft
}

fn generated_artifacts() -> Result<Vec<GeneratedContractArtifact>, RenderProviderError> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_maps_rust_owned_whole_peso_values_without_renderer_schedules() {
        let draft = normal_fixture();
        let envelope = RenderEnvelopeV1::from(&draft);
        assert_eq!(envelope.form.code, "1702RT");
        assert_eq!(envelope.form.version, "2018C");
        assert_eq!(
            envelope.fields["item_43"],
            RenderValue::Integer(draft.part_iv.item_43_tax_due.0)
        );
        assert_eq!(
            envelope.fields["schedule_1_item_18"],
            RenderValue::Integer(draft.schedule_1.item_18_total.0)
        );
        assert!(envelope.schedules.is_empty());
    }

    #[test]
    fn adapter_emits_only_the_official_page_one_payment_cells_and_title_bindings() {
        let draft = normal_fixture();
        let envelope = RenderEnvelopeV1::from(&draft);

        assert!(!envelope.fields.contains_key("payment_25_bank"));
        assert!(!envelope.fields.contains_key("payment_25_specification"));
        assert_eq!(
            envelope.fields["payment_25_number"],
            RenderValue::Text("TDM-1702RT-025".to_string())
        );
        assert_eq!(
            envelope.fields["payment_26_specification"],
            RenderValue::Text("OTHER REVIEWED PAYMENT".to_string())
        );
        assert_eq!(
            envelope.fields["payment_26_bank"],
            RenderValue::Text("AUTHORIZED AGENT BANK 026".to_string())
        );
        assert_eq!(
            envelope.fields["atc_other_description"],
            RenderValue::Text("CORPORATION IN GENERAL - JAN 1, 2009 (2009)".to_string())
        );
        assert_eq!(
            envelope.fields["president_signatory_title"],
            RenderValue::Text("PRESIDENT".to_string())
        );
        assert!(!envelope.fields.contains_key("president_signatory_name"));
        assert_eq!(
            envelope.fields["treasurer_signatory_title"],
            RenderValue::Text("TREASURER".to_string())
        );
        assert!(!envelope.fields.contains_key("treasurer_signatory_name"));
    }

    #[test]
    fn runtime_assets_record_native_seal_and_eight_row_pdf417_evidence() {
        let summaries = runtime_discrete_assets()
            .into_iter()
            .map(|asset| {
                json!({
                    "asset": asset["asset"],
                    "derived_png_sha256": asset["derived_png_sha256"],
                    "logical_dimensions": asset["logical_dimensions"],
                    "logical_matrix_sha256": asset["logical_matrix_sha256"],
                    "logical_path_sha256": asset["logical_path_sha256"],
                })
            })
            .collect::<Vec<_>>();

        assert_eq!(
            summaries,
            vec![
                json!({
                    "asset": "government_seal",
                    "derived_png_sha256": "50d1fc573146e251138b78074b5790dd569f6dbde335feea908334adef4dd7b0",
                    "logical_dimensions": null,
                    "logical_matrix_sha256": null,
                    "logical_path_sha256": null,
                }),
                json!({
                    "asset": "static_form_pdf417_page_1",
                    "derived_png_sha256": null,
                    "logical_dimensions": [120, 8],
                    "logical_matrix_sha256": "362d6c13fc51ae71f86168da68bd2dfeadee77d82a0c68f5234a006adfaf6200",
                    "logical_path_sha256": "82ad803cb29fb6886dd1bd0158c9528df4960c07016f7d807a1a54e6b761eb04",
                }),
                json!({
                    "asset": "static_form_pdf417_page_2",
                    "derived_png_sha256": null,
                    "logical_dimensions": [120, 8],
                    "logical_matrix_sha256": "aaf9a1d313b3804a9ab4507407a98d8d5b32eb4711e8779cae8f1e2e7dbdc849",
                    "logical_path_sha256": "352ade9e021b50d016521709013b6d278b130bf8cb2eaaec0a599e311cfd1dfc",
                }),
                json!({
                    "asset": "static_form_pdf417_page_3",
                    "derived_png_sha256": null,
                    "logical_dimensions": [120, 8],
                    "logical_matrix_sha256": "3ad58b30bbd02dab3dcfe7b896c861669a6f21693f60a04250e9c4e83000fa5f",
                    "logical_path_sha256": "f8ca0babf7a0c25535927f096d9f7beeb68aaa9d1f4962c62b85158b0255f1b2",
                }),
                json!({
                    "asset": "static_form_pdf417_page_4",
                    "derived_png_sha256": null,
                    "logical_dimensions": [120, 8],
                    "logical_matrix_sha256": "94b3aaadd3c8dcfbfca4ba4fb64347ceef5d1a7693f7d7d96a9f4d1e9bf5a8db",
                    "logical_path_sha256": "e52b63b2670d75324b1724a2ae3cc627c024197c5dc20288419f1a2eb9525d4b",
                }),
            ]
        );
    }

    #[test]
    fn fixtures_cover_fixed_four_page_layout_and_domain_validity() {
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
                "{}: {:?}",
                fixture.file_name,
                fixture.envelope.validation
            );
        }
    }
}
