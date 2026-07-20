use bir_core::forms::form_1701::{
    Form1701AmountSection, Form1701Atc, Form1701CivilStatus, Form1701DeductionMethod,
    Form1701Draft, Form1701EmployerRow, Form1701JointFilingStatus, Form1701OverpaymentDisposition,
    Form1701Party, Form1701PaymentRow, Form1701SpouseType, Form1701TaxRate, Form1701TaxpayerType,
};
use bir_core::forms::FormValidator;
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
        file_name: "1701-2018-page-1.png",
        sha256: "37afb5ac1cfb36970601650e3d24a856e61fd0abc7afd0f36d6af9be48d5dd4a",
    },
    VisualReferencePage {
        page: 2,
        file_name: "1701-2018-page-2.png",
        sha256: "51a210c01bd188a7ad92dc52136afa98d2469810bf9cc0cfa2b95f7f69b23400",
    },
    VisualReferencePage {
        page: 3,
        file_name: "1701-2018-page-3.png",
        sha256: "d9f565209adafb59f71befef2a4523416f71abcd37d0d41ca5c16a490e5bb52c",
    },
    VisualReferencePage {
        page: 4,
        file_name: "1701-2018-page-4.png",
        sha256: "8597a626381d0a45164fcdc274e3706965b6290870bd320a2ef87723c2b930a0",
    },
];

/// Preview-only provider for the exact January 2018 four-page main return.
///
/// The reviewed source pack proves an exact 837-key editable-save round trip,
/// but not queue/final-flag semantics. The separate Part X attachment stays a
/// separate, fail-closed workflow and is never appended by this provider.

// Pinned by scripts/prepare_chromium_reference.mjs from the same official
// PDF bytes; see references/1701-2018-chromium-source.json
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
            file_name: "1701-2018-page-1-chromium.png",
            sha256: "ba3bc9a8b93cce3a41794f11627fe09dcd1cb9be8253ee02a1dd652c9b856df2",
            vector_svg_sha256: "960568ad43f2c47a2c3cb00d4fd84656f8130c0dc58f3da7efd8b86ef7d69d5f",
            noise_floor_changed_pixels: 90_280,
        },
        super::ChromiumVisualReference {
            page: 2,
            file_name: "1701-2018-page-2-chromium.png",
            sha256: "4c0609dec36d7a1c02f76cdc3310c8dc95f8b214b47b13fdb1fa38cc30ececa4",
            vector_svg_sha256: "e490106ebccc22ad59c70766f65fa46602ec1454e8b23bbfe55f4791e9ddf1a1",
            noise_floor_changed_pixels: 91_321,
        },
        super::ChromiumVisualReference {
            page: 3,
            file_name: "1701-2018-page-3-chromium.png",
            sha256: "b2105366a8a3cb56e74eed7f48072a097ad77524a720feccd40ac3464007a42f",
            vector_svg_sha256: "120e9b38c468847dbb0f6c2a0efd5f23ca43dc3cc6bef9280f7e527e26ab4105",
            noise_floor_changed_pixels: 78_956,
        },
        super::ChromiumVisualReference {
            page: 4,
            file_name: "1701-2018-page-4-chromium.png",
            sha256: "3cddad3456e67f93b9c6212eb25a1b2a2f75df4b043189eeeeb63c8a65460173",
            vector_svg_sha256: "acc47d664bd7d7acf99cbb6ab3b52078d6248a042a026785f56d4b0bd09ffd11",
            noise_floor_changed_pixels: 93_413,
        },
    ],
};

pub(super) const PROVIDER: RenderFormProvider = RenderFormProvider {
    code: "1701",
    revision: "2018",
    form_id: "1701v2018",
    title: "Annual Income Tax Return for Individuals, Estates and Trusts",
    page_width_pt: RenderPageGeometry::LEGAL.width_points,
    page_height_pt: RenderPageGeometry::LEGAL.height_points,
    expected_base_page_count: 4,
    schedules: &[],
    visual_fixture_file_name: "1701-normal.json",
    visual_fixture_sha256: "469be004c4f32ee8e27a4589731cedeb9b6dab61410be831ff0ff001f30dfe58",
    official_source:
        "https://bir-cdn.bir.gov.ph/local/pdf/1701%20Jan%202018%20final%20with%20rates.pdf",
    official_source_sha256: "19be91d78258eb7c255f2615610db2739f10c378f8ac97adc0887c1bf40d1b2e",
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
            "embedded_in": "packages/form-renderer/src/forms/assets/1701-seal.png",
            "source_bbox_top_left_points": [240.16, 14.088, 33.682, 28.152],
            "source_channels_equal": true,
            "source_ctm_points": [33.682, 0.0, 0.0, 28.152, 240.16, 893.76],
            "source_decoded_rgb_sha256": "80ad6c1fa30a29d7795d475da6533459d1242ee2226b6cb23cf8ef6709cba5ce",
            "source_page": 1,
            "source_pdf_object_id": [50, 0],
            "source_pixel_dimensions": [119, 102],
            "source_png_sha256": "e667f9a75d1b1ab2d929c1ab0feb19edb3bca765336056054491a1cb8c25245a",
            "source_stream_sha256": "46f193f83e79047c3a1a4444b2004687938ae982a5a281e082db2a96c5074d04",
            "treatment": "lossless extraction of the exact official PDF image XObject; equal RGB channels collapsed to one native grayscale channel without crop, resampling, recoloring, thresholding, or substitution"
        }),
        json!({
            "asset": "static_form_pdf417_page_1",
            "caption_bbox_top_left_points": [522.46, 90.4238, 74.43022, 8.9646],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "1701 01/18ENCS P1",
            "decoded_payload": "1701 01/18ENCS P1",
            "decoder_evidence": [{"decoder": "ZXing-C++ 3.1.0", "payload": "1701 01/18ENCS P1", "symbology": "PDF417"}],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1701Assets.ts",
            "encoder_proof": {"columns": 3, "encoding": "ISO-8859-1", "error_correction_level": 2, "implementation": "pdf417gen 0.8.1", "module_differences": 0, "rows": 7},
            "logical_black_modules": 483,
            "logical_dimensions": [120, 7],
            "logical_matrix_sha256": "a70883c8ea82b51527ab13d9d7a84acea444a1b941107cf0be13d3132066a068",
            "logical_path_sha256": "1e1efef2ff6bdc2eace68bfebbbd9eb493d255f7167e9ec4c70832902d51a64c",
            "source_active_bbox_top_left_points": [438.84, 49.44, 158.4, 41.4],
            "source_active_pixel_bounds": [0, 0, 240, 63],
            "source_bbox_top_left_points": [438.84, 49.44, 158.4, 41.4],
            "source_ctm_points": [158.4, 0.0, 0.0, 41.4, 438.84, 845.16],
            "source_decoded_rgb_sha256": "21beac9b84195511f37680d3d7b55afa1804bcf10271a79e028dfccd9c6dbf62",
            "source_page": 1,
            "source_pdf_object_id": [51, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 0, "right": 0},
            "source_pixel_dimensions": [240, 63],
            "source_png_sha256": "3617c72001e6fba856bf375cabbe994964fab6d41912e39345fbd6a2fdeb7ad7",
            "source_stream_sha256": "56a6358d0143ccf28b15a951d8908f689bd8f52cdba910b2bf0550b341048787",
            "symbology": "PDF417"
        }),
        json!({
            "asset": "static_form_pdf417_page_2",
            "caption_bbox_top_left_points": [522.34, 56.8038, 74.43022, 8.9646],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "1701 01/18ENCS P2",
            "decoded_payload": "1701 01/18ENCS P2",
            "decoder_evidence": [{"decoder": "ZXing-C++ 3.1.0", "payload": "1701 01/18ENCS P2", "symbology": "PDF417"}],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1701Assets.ts",
            "encoder_proof": {"columns": 3, "encoding": "ISO-8859-1", "error_correction_level": 2, "implementation": "pdf417gen 0.8.1", "module_differences": 0, "rows": 7},
            "logical_black_modules": 474,
            "logical_dimensions": [120, 7],
            "logical_matrix_sha256": "fc575f786121c3fdfd0b3acc6e7b0757fcf0098bef23b3beac77a359826d3354",
            "logical_path_sha256": "9c7aea5124cb0901d56b03e3a7b2b1acbaa69f0872e1596e8ffd52cc08c8925a",
            "source_active_bbox_top_left_points": [436.32, 21.12, 161.64, 35.4],
            "source_active_pixel_bounds": [0, 0, 240, 63],
            "source_bbox_top_left_points": [436.32, 21.12, 161.64, 35.4],
            "source_ctm_points": [161.64, 0.0, 0.0, 35.4, 436.32, 879.48],
            "source_decoded_rgb_sha256": "e861773aba90eed4187e5933484c1086ec7db5b05f8acc3d2e8957f0f6579048",
            "source_page": 2,
            "source_pdf_object_id": [54, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 0, "right": 0},
            "source_pixel_dimensions": [240, 63],
            "source_png_sha256": "5f44eedfa4f5629a80190dec43073aea01d1b9aced4242ace383f1ce9733b227",
            "source_stream_sha256": "d2b327ac5898f3166ad2afaa0f3078dac13c2f128087b1bc9c5c19f63483fb72",
            "symbology": "PDF417"
        }),
        json!({
            "asset": "static_form_pdf417_page_3",
            "caption_bbox_top_left_points": [522.58, 56.9238, 74.39844, 8.9646],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "1701 01/18ENCS P3",
            "decoded_payload": "1701 01/18ENCS P3",
            "decoder_evidence": [{"decoder": "ZXing-C++ 3.1.0", "payload": "1701 01/18ENCS P3", "symbology": "PDF417"}],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1701Assets.ts",
            "encoder_proof": {"columns": 3, "encoding": "ISO-8859-1", "error_correction_level": 2, "implementation": "pdf417gen 0.8.1", "module_differences": 0, "rows": 7},
            "logical_black_modules": 484,
            "logical_dimensions": [120, 7],
            "logical_matrix_sha256": "28f993cc6172d3b5891ebd5caa3c0e68696698ecf194c27a10acb774d99a4f6e",
            "logical_path_sha256": "191691ec2311ce0d786eff4aa9ac4c1ae3e465cc34251fb16a712bd4466ada46",
            "source_active_bbox_top_left_points": [433.8, 21.48, 163.2, 35.88],
            "source_active_pixel_bounds": [0, 0, 240, 63],
            "source_bbox_top_left_points": [433.8, 21.48, 163.2, 35.88],
            "source_ctm_points": [163.2, 0.0, 0.0, 35.88, 433.8, 878.64],
            "source_decoded_rgb_sha256": "98ca37c879c043e1bed7ba02469140437ebcd125d0e681b0d20bdad7b2238809",
            "source_page": 3,
            "source_pdf_object_id": [66, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 0, "right": 0},
            "source_pixel_dimensions": [240, 63],
            "source_png_sha256": "75fafcf0324308fa49d197c3d9a2b729677d3b69f377e81517f9cc89cd2f134d",
            "source_stream_sha256": "b05647648dda1446cdebe3e2ada028755be2aec7b49b1f71b3cea6d1fdd6bd05",
            "symbology": "PDF417"
        }),
        json!({
            "asset": "static_form_pdf417_page_4",
            "caption_bbox_top_left_points": [522.34, 55.2438, 74.39844, 8.9646],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "1701 01/18ENCS P4",
            "decoded_payload": "1701 01/18ENCS P4",
            "decoder_evidence": [{"decoder": "ZXing-C++ 3.1.0", "payload": "1701 01/18ENCS P4", "symbology": "PDF417"}],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official1701Assets.ts",
            "encoder_proof": {"columns": 3, "encoding": "ISO-8859-1", "error_correction_level": 2, "implementation": "pdf417gen 0.8.1", "module_differences": 0, "rows": 7},
            "logical_black_modules": 482,
            "logical_dimensions": [120, 7],
            "logical_matrix_sha256": "209e791883eab663776176777be715f5292b13b386a3a461c1f5bd829fbae1d6",
            "logical_path_sha256": "b3abe63985d02ec09423acfd1c71e14a394ec7398649201f0737b0d59ef84895",
            "source_active_bbox_top_left_points": [435.12, 21.12, 161.76, 34.32],
            "source_active_pixel_bounds": [0, 0, 240, 63],
            "source_bbox_top_left_points": [435.12, 21.12, 161.76, 34.32],
            "source_ctm_points": [161.76, 0.0, 0.0, 34.32, 435.12, 880.56],
            "source_decoded_rgb_sha256": "17c6b4269d1484203cab29ea7330ec74124fcc376709b9d365f9092d75b1276f",
            "source_page": 4,
            "source_pdf_object_id": [69, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 0, "right": 0},
            "source_pixel_dimensions": [240, 63],
            "source_png_sha256": "a07644120648f1810c64e3d5d1c53e65ed3ad6fc884e1d206bfbd58ee93cc8ad",
            "source_stream_sha256": "8c425c9b9ae69bef05130e691f03612805bc0b0b82af37511c272d1894771ff8",
            "symbology": "PDF417"
        }),
    ]
}

impl From<&Form1701Draft> for RenderEnvelopeV1 {
    fn from(draft: &Form1701Draft) -> Self {
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
                month: Some(draft.period_end_month),
                quarter: None,
                label: format!("{:04}-{:02}", draft.taxable_year, draft.period_end_month),
            },
        );

        insert_bool(&mut envelope, "is_amended", draft.is_amended);
        insert_bool(&mut envelope, "is_short_period", draft.is_short_period);
        insert_text(&mut envelope, "date_of_birth", &draft.date_of_birth);
        insert_text(&mut envelope, "citizenship", &draft.citizenship);
        insert_text(
            &mut envelope,
            "foreign_tax_number",
            &draft.foreign_tax_number,
        );
        insert_optional_bool(
            &mut envelope,
            "claims_foreign_tax_credits",
            draft.claims_foreign_tax_credits,
        );
        insert_optional_bool(&mut envelope, "has_exempt_income", draft.has_exempt_income);
        insert_optional_bool(
            &mut envelope,
            "has_special_rate_income",
            draft.has_special_rate_income,
        );
        insert_optional_bool(&mut envelope, "spouse_has_income", draft.spouse_has_income);
        insert_optional_decimal(
            &mut envelope,
            "number_of_attachments",
            draft.number_of_attachments.map(f64::from),
        );
        if let Some(value) = draft.taxpayer_type {
            insert_text(&mut envelope, "taxpayer_type", taxpayer_type_key(value));
        }
        if let Some(value) = draft.atc {
            insert_text(&mut envelope, "atc", value.code());
        }
        if let Some(value) = draft.civil_status {
            insert_text(&mut envelope, "civil_status", civil_status_key(value));
        }
        if let Some(value) = draft.joint_filing_status {
            insert_text(
                &mut envelope,
                "joint_filing_status",
                match value {
                    Form1701JointFilingStatus::Joint => "joint",
                    Form1701JointFilingStatus::Separate => "separate",
                },
            );
        }
        insert_optional_tax_rate(&mut envelope, "tax_rate", draft.tax_rate);
        insert_optional_deduction(&mut envelope, "deduction_method", draft.deduction_method);
        insert_text(
            &mut envelope,
            "overpayment_disposition",
            match draft.overpayment_disposition {
                Form1701OverpaymentDisposition::None => "none",
                Form1701OverpaymentDisposition::Refund => "refund",
                Form1701OverpaymentDisposition::TaxCreditCertificate => "tax_credit_certificate",
                Form1701OverpaymentDisposition::CarryOver => "carry_over",
            },
        );

        insert_bool(&mut envelope, "spouse_enabled", draft.spouse.enabled);
        insert_text(&mut envelope, "spouse_tin", &draft.spouse.tin);
        insert_text(&mut envelope, "spouse_rdo_code", &draft.spouse.rdo_code);
        insert_text(&mut envelope, "spouse_name", &draft.spouse.name);
        insert_text(
            &mut envelope,
            "spouse_contact_number",
            &draft.spouse.contact_number,
        );
        insert_text(
            &mut envelope,
            "spouse_citizenship",
            &draft.spouse.citizenship,
        );
        insert_text(
            &mut envelope,
            "spouse_foreign_tax_number",
            &draft.spouse.foreign_tax_number,
        );
        insert_optional_bool(
            &mut envelope,
            "spouse_claims_foreign_tax_credits",
            draft.spouse.claims_foreign_tax_credits,
        );
        insert_optional_bool(
            &mut envelope,
            "spouse_has_exempt_income",
            draft.spouse.has_exempt_income,
        );
        insert_optional_bool(
            &mut envelope,
            "spouse_has_special_rate_income",
            draft.spouse.has_special_rate_income,
        );
        if let Some(value) = draft.spouse.filer_type {
            insert_text(&mut envelope, "spouse_type", spouse_type_key(value));
        }
        if let Some(value) = draft.spouse.atc {
            insert_text(&mut envelope, "spouse_atc", value.code());
        }
        insert_optional_tax_rate(&mut envelope, "spouse_tax_rate", draft.spouse.tax_rate);
        insert_optional_deduction(
            &mut envelope,
            "spouse_deduction_method",
            draft.spouse.deduction_method,
        );

        for (index, row) in draft.employers.iter().enumerate() {
            insert_employer(&mut envelope, index + 1, row);
        }
        for (section, table) in [
            ("part_ii", &draft.computations.part_ii),
            ("schedule_2", &draft.computations.schedule_2),
            ("schedule_3", &draft.computations.schedule_3),
            ("schedule_4", &draft.computations.schedule_4),
            ("schedule_6", &draft.computations.schedule_6_summary),
            ("part_vi", &draft.computations.part_vi),
            ("part_vii", &draft.computations.part_vii),
            ("part_viii", &draft.computations.part_viii),
            ("part_ix", &draft.computations.part_ix),
        ] {
            for (item, pair) in table {
                insert_optional_decimal(
                    &mut envelope,
                    &format!("{section}_{item}_taxpayer"),
                    pair.taxpayer,
                );
                insert_optional_decimal(
                    &mut envelope,
                    &format!("{section}_{item}_spouse"),
                    pair.spouse,
                );
            }
        }
        insert_optional_decimal(
            &mut envelope,
            "part_ii_32_aggregate",
            draft.computations.part_ii_item_32_aggregate,
        );
        for (index, pair) in draft.computations.schedule_4_item_17.iter().enumerate() {
            let suffix = char::from(b'a' + index as u8);
            insert_optional_decimal(
                &mut envelope,
                &format!("schedule_4_17{suffix}_taxpayer"),
                pair.taxpayer,
            );
            insert_optional_decimal(
                &mut envelope,
                &format!("schedule_4_17{suffix}_spouse"),
                pair.spouse,
            );
        }
        insert_text(
            &mut envelope,
            "schedule_4_17d_description",
            &draft.computations.schedule_4_item_17d_description,
        );
        for (party, rows) in [
            ("taxpayer", &draft.computations.schedule_5_taxpayer),
            ("spouse", &draft.computations.schedule_5_spouse),
        ] {
            for (index, row) in rows.iter().enumerate() {
                let key = format!("schedule_5_{party}_{}", index + 1);
                insert_text(
                    &mut envelope,
                    &format!("{key}_description"),
                    &row.description,
                );
                insert_text(
                    &mut envelope,
                    &format!("{key}_legal_basis"),
                    &row.legal_basis,
                );
                insert_optional_decimal(&mut envelope, &format!("{key}_amount"), row.amount);
            }
        }
        insert_optional_decimal(
            &mut envelope,
            "schedule_5_total_taxpayer",
            draft.computations.schedule_5_total_taxpayer,
        );
        insert_optional_decimal(
            &mut envelope,
            "schedule_5_total_spouse",
            draft.computations.schedule_5_total_spouse,
        );
        for (party, rows) in [
            ("taxpayer", &draft.computations.schedule_6_taxpayer_nolco),
            ("spouse", &draft.computations.schedule_6_spouse_nolco),
        ] {
            for (index, row) in rows.iter().enumerate() {
                let key = format!("schedule_6_{party}_{}", index + 1);
                insert_text(&mut envelope, &format!("{key}_year"), &row.year_incurred);
                insert_optional_decimal(&mut envelope, &format!("{key}_amount"), row.amount);
                insert_optional_decimal(
                    &mut envelope,
                    &format!("{key}_previous"),
                    row.applied_previous_years,
                );
                insert_optional_decimal(&mut envelope, &format!("{key}_expired"), row.expired);
                insert_optional_decimal(
                    &mut envelope,
                    &format!("{key}_current"),
                    row.applied_current_year,
                );
                insert_optional_decimal(&mut envelope, &format!("{key}_unapplied"), row.unapplied);
            }
        }
        insert_optional_decimal(
            &mut envelope,
            "schedule_6_total_taxpayer",
            draft.computations.schedule_6_total_taxpayer,
        );
        insert_optional_decimal(
            &mut envelope,
            "schedule_6_total_spouse",
            draft.computations.schedule_6_total_spouse,
        );
        for (item, description) in &draft.computations.schedule_3_descriptions {
            insert_text(
                &mut envelope,
                &format!("schedule_3_{item}_description"),
                description,
            );
        }
        insert_text(
            &mut envelope,
            "part_vii_9_description",
            &draft.computations.part_vii_item_9_description,
        );
        for (item, description) in &draft.computations.part_ix_descriptions {
            insert_text(
                &mut envelope,
                &format!("part_ix_{item}_description"),
                description,
            );
        }

        insert_payment_row(
            &mut envelope,
            "payment_34",
            &draft.payment_details.item_34_cash_or_bank_debit_memo,
        );
        insert_payment_row(
            &mut envelope,
            "payment_35",
            &draft.payment_details.item_35_check,
        );
        insert_payment_row(
            &mut envelope,
            "payment_36",
            &draft.payment_details.item_36_tax_debit_memo,
        );
        insert_payment_row(
            &mut envelope,
            "payment_37",
            &draft.payment_details.item_37_others,
        );
        insert_text(
            &mut envelope,
            "payment_37_description",
            &draft.payment_details.item_37_others_description,
        );
        insert_text(
            &mut envelope,
            "machine_validation_or_receipt_details",
            &draft.machine_validation_or_receipt_details,
        );

        envelope.validation = draft
            .validate()
            .into_iter()
            .map(|(field_path, message)| RenderValidationMessage {
                field_path,
                code: "invalid".to_string(),
                message,
                severity: RenderValidationSeverity::Error,
                rule_version: "1701-2018-domain-v1".to_string(),
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
    value: Option<Form1701TaxRate>,
) {
    if let Some(value) = value {
        insert_text(
            envelope,
            key,
            match value {
                Form1701TaxRate::Graduated => "graduated",
                Form1701TaxRate::EightPercent => "eight_percent",
            },
        );
    }
}

fn insert_optional_deduction(
    envelope: &mut RenderEnvelopeV1,
    key: &str,
    value: Option<Form1701DeductionMethod>,
) {
    if let Some(value) = value {
        insert_text(
            envelope,
            key,
            match value {
                Form1701DeductionMethod::Itemized => "itemized",
                Form1701DeductionMethod::Osd => "osd",
            },
        );
    }
}

const fn taxpayer_type_key(value: Form1701TaxpayerType) -> &'static str {
    match value {
        Form1701TaxpayerType::SingleProprietor => "single_proprietor",
        Form1701TaxpayerType::Professional => "professional",
        Form1701TaxpayerType::Estate => "estate",
        Form1701TaxpayerType::Trust => "trust",
        Form1701TaxpayerType::CompensationEarner => "compensation_earner",
    }
}

const fn spouse_type_key(value: Form1701SpouseType) -> &'static str {
    match value {
        Form1701SpouseType::SingleProprietor => "single_proprietor",
        Form1701SpouseType::Professional => "professional",
        Form1701SpouseType::CompensationEarner => "compensation_earner",
    }
}

const fn civil_status_key(value: Form1701CivilStatus) -> &'static str {
    match value {
        Form1701CivilStatus::Single => "single",
        Form1701CivilStatus::Married => "married",
        Form1701CivilStatus::LegallySeparated => "legally_separated",
        Form1701CivilStatus::Widowed => "widowed",
    }
}

fn insert_employer(envelope: &mut RenderEnvelopeV1, index: usize, row: &Form1701EmployerRow) {
    if let Some(owner) = row.owner {
        insert_text(
            envelope,
            &format!("employer_{index}_owner"),
            match owner {
                Form1701Party::Taxpayer => "taxpayer",
                Form1701Party::Spouse => "spouse",
            },
        );
    }
    insert_text(
        envelope,
        &format!("employer_{index}_name"),
        &row.employer_name,
    );
    insert_text(
        envelope,
        &format!("employer_{index}_tin"),
        &row.employer_tin,
    );
    insert_optional_decimal(
        envelope,
        &format!("employer_{index}_compensation"),
        row.compensation_income,
    );
    insert_optional_decimal(
        envelope,
        &format!("employer_{index}_withheld"),
        row.tax_withheld,
    );
}

fn insert_payment_row(envelope: &mut RenderEnvelopeV1, key: &str, row: &Form1701PaymentRow) {
    insert_text(envelope, &format!("{key}_bank"), &row.drawee_bank_or_agency);
    insert_text(envelope, &format!("{key}_number"), &row.number);
    insert_text(envelope, &format!("{key}_date"), &row.date);
    insert_optional_decimal(envelope, &format!("{key}_amount"), row.amount);
}

fn fixtures() -> Result<Vec<RenderContractFixture>, RenderProviderError> {
    Ok(vec![
        fixture(
            "1701-minimum.json",
            RenderFixtureKind::Minimum,
            true,
            minimum_fixture(),
        ),
        fixture(
            "1701-normal.json",
            RenderFixtureKind::Normal,
            true,
            normal_fixture(),
        ),
        fixture(
            "1701-long-values.json",
            RenderFixtureKind::LongValues,
            true,
            long_values_fixture(),
        ),
        fixture(
            "1701-validation-edge.json",
            RenderFixtureKind::ValidationEdge,
            false,
            validation_edge_fixture(),
        ),
        fixture(
            "1701-fixed-capacity.json",
            RenderFixtureKind::ScheduleCapacity,
            true,
            fixed_capacity_fixture(),
        ),
    ])
}

fn fixture(
    file_name: &'static str,
    kind: RenderFixtureKind,
    expected_form_valid: bool,
    draft: Form1701Draft,
) -> RenderContractFixture {
    RenderContractFixture {
        file_name,
        kind,
        expected_form_valid,
        envelope: RenderEnvelopeV1::from(&draft),
    }
}

fn base_fixture() -> Form1701Draft {
    let mut draft = Form1701Draft {
        tin: "12345678900000".to_string(),
        taxable_year: 2026,
        period_end_month: 12,
        rdo_code: "018".to_string(),
        taxpayer_type: Some(Form1701TaxpayerType::SingleProprietor),
        atc: Some(Form1701Atc::Ii012),
        taxpayer_name: "JUAN MIGUEL DELA CRUZ".to_string(),
        registered_address: "53 SANTOL EXTENSION, NEW CABALAN, OLONGAPO CITY".to_string(),
        zip_code: "2200".to_string(),
        date_of_birth: "01/15/1990".to_string(),
        email: "renderer.1701@example.com".to_string(),
        citizenship: "FILIPINO".to_string(),
        claims_foreign_tax_credits: Some(false),
        contact_number: "09123456789".to_string(),
        civil_status: Some(Form1701CivilStatus::Single),
        has_exempt_income: Some(false),
        has_special_rate_income: Some(false),
        tax_rate: Some(Form1701TaxRate::Graduated),
        deduction_method: Some(Form1701DeductionMethod::Osd),
        number_of_attachments: Some(0),
        status: bir_core::forms::FilingStatus::Draft,
        ..Default::default()
    };
    draft.set_amount(
        Form1701AmountSection::Schedule3,
        8,
        Form1701Party::Taxpayer,
        Some(750_000.0),
    );
    draft.recompute();
    draft
}

fn minimum_fixture() -> Form1701Draft {
    base_fixture()
}

fn normal_fixture() -> Form1701Draft {
    let mut draft = base_fixture();
    draft.civil_status = Some(Form1701CivilStatus::Married);
    draft.spouse_has_income = Some(true);
    draft.joint_filing_status = Some(Form1701JointFilingStatus::Joint);
    draft.spouse.enabled = true;
    draft.spouse.tin = "98765432100000".to_string();
    draft.spouse.rdo_code = "018".to_string();
    draft.spouse.filer_type = Some(Form1701SpouseType::Professional);
    draft.spouse.atc = Some(Form1701Atc::Ii014);
    draft.spouse.name = "MARIA CONSOLACION DELA CRUZ".to_string();
    draft.spouse.contact_number = "09171234567".to_string();
    draft.spouse.citizenship = "FILIPINO".to_string();
    draft.spouse.claims_foreign_tax_credits = Some(false);
    draft.spouse.has_exempt_income = Some(false);
    draft.spouse.has_special_rate_income = Some(false);
    draft.spouse.tax_rate = Some(Form1701TaxRate::Graduated);
    draft.spouse.deduction_method = Some(Form1701DeductionMethod::Itemized);
    draft.employers[0] = Form1701EmployerRow {
        owner: Some(Form1701Party::Taxpayer),
        employer_name: "GOLDCODERS CORPORATION".to_string(),
        employer_tin: "00000000000000".to_string(),
        compensation_income: Some(480_000.0),
        tax_withheld: Some(21_000.0),
    };
    draft.employers[1] = Form1701EmployerRow {
        owner: Some(Form1701Party::Spouse),
        employer_name: "REVIEWED PROFESSIONAL PARTNERSHIP".to_string(),
        employer_tin: "11122233300000".to_string(),
        compensation_income: Some(320_000.0),
        tax_withheld: Some(12_000.0),
    };
    draft.set_amount(
        Form1701AmountSection::Schedule3,
        8,
        Form1701Party::Spouse,
        Some(420_000.0),
    );
    draft.set_amount(
        Form1701AmountSection::Schedule3,
        13,
        Form1701Party::Spouse,
        Some(75_000.0),
    );
    draft
        .computations
        .schedule_3_descriptions
        .insert(19, "ROYALTY INCOME NOT SUBJECT TO FINAL TAX".to_string());
    draft.set_amount(
        Form1701AmountSection::Schedule3,
        19,
        Form1701Party::Taxpayer,
        Some(15_000.0),
    );
    draft.recompute();
    draft
}

fn long_values_fixture() -> Form1701Draft {
    let mut draft = normal_fixture();
    draft.taxpayer_name = "JUAN MIGUEL ALEJANDRO REYES DELA CRUZ-SANTOS WITH A VALID REGISTERED NAME LONGER THAN THE OFFICIAL COMB".to_string();
    draft.registered_address = "UNIT 1201, A DELIBERATELY LONG REGISTERED ADDRESS USED TO PROVE THE ANNUAL RETURN PRESERVES EVERY VALID CHARACTER, NEW CABALAN, OLONGAPO CITY".to_string();
    draft.email = "long.annual.income.tax.renderer.verification.address@example.test".to_string();
    draft.spouse.name = "MARIA CONSOLACION REYES DELA CRUZ-SANTOS WITH A VALID REGISTERED NAME LONGER THAN THE OFFICIAL COMB".to_string();
    draft.employers[0].employer_name =
        "AUTHORIZED EMPLOYER WITH A DELIBERATELY LONG REGISTERED LEGAL NAME".to_string();
    draft.computations.schedule_3_descriptions.insert(
        20,
        "A VALID NON-OPERATING INCOME DESCRIPTION LONGER THAN THE OFFICIAL COMB CAPACITY"
            .to_string(),
    );
    draft.recompute();
    draft
}

fn fixed_capacity_fixture() -> Form1701Draft {
    let mut draft = normal_fixture();
    for (index, row) in draft
        .computations
        .schedule_5_taxpayer
        .iter_mut()
        .enumerate()
    {
        row.description = format!("SPECIAL TAXPAYER DEDUCTION {}", index + 1);
        row.legal_basis = format!("REVIEWED LEGAL BASIS {}", index + 1);
        row.amount = Some((index as f64 + 1.0) * 10_000.0);
    }
    for (index, row) in draft.computations.schedule_5_spouse.iter_mut().enumerate() {
        row.description = format!("SPECIAL SPOUSE DEDUCTION {}", index + 1);
        row.legal_basis = format!("REVIEWED LEGAL BASIS {}", index + 3);
        row.amount = Some((index as f64 + 1.0) * 5_000.0);
    }
    for (party_index, rows) in [
        &mut draft.computations.schedule_6_taxpayer_nolco,
        &mut draft.computations.schedule_6_spouse_nolco,
    ]
    .into_iter()
    .enumerate()
    {
        for (index, row) in rows.iter_mut().enumerate() {
            row.year_incurred = (2021 + index).to_string();
            row.amount = Some((party_index as f64 + 1.0) * (index as f64 + 1.0) * 20_000.0);
            row.applied_previous_years = Some(1_000.0);
            row.expired = Some(0.0);
            row.applied_current_year = Some(2_000.0);
        }
    }
    draft.recompute();
    draft
}

fn validation_edge_fixture() -> Form1701Draft {
    Form1701Draft {
        taxable_year: 2018,
        period_end_month: 12,
        status: bir_core::forms::FilingStatus::Draft,
        ..Default::default()
    }
}

fn generated_artifacts() -> Result<Vec<GeneratedContractArtifact>, RenderProviderError> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_maps_rust_owned_identity_calculations_and_fixed_rows() {
        let draft = normal_fixture();
        let envelope = RenderEnvelopeV1::from(&draft);
        assert_eq!(envelope.form.code, "1701");
        assert_eq!(envelope.form.version, "2018");
        assert_eq!(envelope.period.month, Some(12));
        assert_eq!(
            envelope.fields["schedule_3_8_taxpayer"],
            RenderValue::Decimal(750_000.0)
        );
        assert_eq!(
            envelope.fields["employer_1_name"],
            RenderValue::Text("GOLDCODERS CORPORATION".to_string())
        );
        assert!(envelope.schedules.is_empty());
    }

    #[test]
    fn fixtures_cover_four_page_layout_and_validation_states() {
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

    #[test]
    fn discrete_artwork_provenance_matches_verified_official_xobjects() {
        let assets = runtime_discrete_assets();
        assert_eq!(assets.len(), 5);

        let seal = &assets[0];
        assert_eq!(seal["asset"], json!("government_seal"));
        assert_eq!(seal["source_pdf_object_id"], json!([50, 0]));
        assert_eq!(seal["source_pixel_dimensions"], json!([119, 102]));
        assert_eq!(seal["source_channels_equal"], json!(true));
        assert_eq!(
            seal["derived_png_sha256"],
            json!("50d1fc573146e251138b78074b5790dd569f6dbde335feea908334adef4dd7b0")
        );

        for (asset, expected) in assets[1..].iter().zip([
            (
                "static_form_pdf417_page_1",
                1,
                51,
                "1701 01/18ENCS P1",
                483,
                "a70883c8ea82b51527ab13d9d7a84acea444a1b941107cf0be13d3132066a068",
                "1e1efef2ff6bdc2eace68bfebbbd9eb493d255f7167e9ec4c70832902d51a64c",
                json!([438.84, 49.44, 158.4, 41.4]),
            ),
            (
                "static_form_pdf417_page_2",
                2,
                54,
                "1701 01/18ENCS P2",
                474,
                "fc575f786121c3fdfd0b3acc6e7b0757fcf0098bef23b3beac77a359826d3354",
                "9c7aea5124cb0901d56b03e3a7b2b1acbaa69f0872e1596e8ffd52cc08c8925a",
                json!([436.32, 21.12, 161.64, 35.4]),
            ),
            (
                "static_form_pdf417_page_3",
                3,
                66,
                "1701 01/18ENCS P3",
                484,
                "28f993cc6172d3b5891ebd5caa3c0e68696698ecf194c27a10acb774d99a4f6e",
                "191691ec2311ce0d786eff4aa9ac4c1ae3e465cc34251fb16a712bd4466ada46",
                json!([433.8, 21.48, 163.2, 35.88]),
            ),
            (
                "static_form_pdf417_page_4",
                4,
                69,
                "1701 01/18ENCS P4",
                482,
                "209e791883eab663776176777be715f5292b13b386a3a461c1f5bd829fbae1d6",
                "b3abe63985d02ec09423acfd1c71e14a394ec7398649201f0737b0d59ef84895",
                json!([435.12, 21.12, 161.76, 34.32]),
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
