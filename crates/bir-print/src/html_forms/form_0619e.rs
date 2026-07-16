use bir_core::{
    forms::{
        form_0619e::{Form0619EDraft, Form0619EPaymentRow, WithholdingAgentCategory},
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

const VISUAL_REFERENCE_PAGES: &[VisualReferencePage] = &[VisualReferencePage {
    page: 1,
    file_name: "0619e-2018-page-1.png",
    sha256: "924db11df80f2ee4e5df63838ac6b93980725d677632c9739690c9ec9aacb83a",
}];

pub(super) const PROVIDER: RenderFormProvider = RenderFormProvider {
    code: "0619E",
    revision: "2018",
    form_id: "0619Ev2018",
    title: "Monthly Remittance Form of Creditable Income Taxes Withheld (Expanded)",
    page_width_pt: RenderPageGeometry::LETTER.width_points,
    page_height_pt: RenderPageGeometry::LETTER.height_points,
    expected_base_page_count: 1,
    schedules: &[],
    visual_fixture_file_name: "0619e-normal.json",
    visual_fixture_sha256: "f08bd7cbb1eee63fdfdab025d880094319b949cd566be2c82203385e5f0c41fe",
    official_source: "https://bir-cdn.bir.gov.ph/local/pdf/0619-E%20Jan%202018%20rev%20final.pdf",
    official_source_sha256: "0418160d63d4e6f68c34f2bad553273a5d148c3686d8562d338d35fcdd0c5215",
    reference_dpi: 144,
    reference_width_px: 1_224,
    reference_height_px: 1_584,
    visual_reference_pages: VISUAL_REFERENCE_PAGES,
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
            "derived_png_sha256": "6cd637975f8088cc8b56aa67c26794db23331864a45c08517c66b70f53ff2610",
            "embedded_as": "lossless_official_pdf_xobject_png",
            "embedded_in": "packages/form-renderer/src/forms/assets/0619e-seal.png",
            "source_bbox_top_left_points": [232.2, 24.84, 31.08, 28.2],
            "source_ctm_points": [31.08, 0.0, 0.0, 28.2, 232.2, 738.96],
            "source_decoded_rgb_sha256": "5eae06e03b3e0c0365397cd2d401292678acb69fecdf35f5afe5fecd342cf85d",
            "source_page": 1,
            "source_pdf_object_id": [41, 0],
            "source_pixel_dimensions": [86, 78],
            "source_png_sha256": "6cd637975f8088cc8b56aa67c26794db23331864a45c08517c66b70f53ff2610",
            "source_stream_sha256": "39613d13cd9b910b039ef6287ce4ef059e03661acf565719fdb740d751771ba1",
            "treatment": "lossless extraction of the exact official PDF image XObject; no crop, resampling, recoloring, or substitution"
        }),
        json!({
            "asset": "static_form_pdf417_page_1",
            "caption_bbox_top_left_points": [531.1, 99.9038, 62.2351, 8.9646],
            "caption_font": "Arial",
            "caption_font_size_points": 8.04,
            "caption_render_font": "eBIRForms Arimo",
            "caption_text": "0619-E 01/18 P1",
            "decoded_payload": "0619-E 01/18 P1",
            "decoder_evidence": [
                {"decoder": "ZXing-C++ 3.1.0", "payload": "0619-E 01/18 P1", "symbology": "PDF417"},
                {"decoder": "pdf417decoder 1.0.8", "payload": "0619-E 01/18 P1", "data_columns": 3, "data_rows": 7, "error_correction_length": 8}
            ],
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": "packages/form-renderer/src/forms/official0619EAssets.ts",
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
            "logical_matrix_sha256": "0238537d56f19276b790a8395c429a9f7645ac570eecc097064051b456dc5dfa",
            "source_active_pixel_bounds": [0, 0, 240, 63],
            "source_bbox_top_left_points": [454.8, 65.04, 136.68, 36.0],
            "source_ctm_points": [136.68, 0.0, 0.0, 36.0, 454.8, 690.96],
            "source_decoded_rgb_sha256": "43f7b0378e905db2f776353f19d276fb616d07084d51537e1c50236e02ae0ceb",
            "source_page": 1,
            "source_pdf_object_id": [42, 0],
            "source_module_scale_pixels": [2, 9],
            "source_padding_pixels": {"bottom": 1, "right": 3},
            "source_pixel_dimensions": [243, 64],
            "source_png_sha256": "d741fca6b5b75409f277ff96754b157ffb8200b89f80aad7558f0014536102e6",
            "source_stream_sha256": "5217f1767917305c0a989e324074151d3d170db648f8777fbe5de8ace0245cd9",
            "symbology": "PDF417"
        }),
    ]
}

impl From<&Form0619EDraft> for RenderEnvelopeV1 {
    fn from(draft: &Form0619EDraft) -> Self {
        let due_date = draft.due_day.map_or_else(String::new, |day| {
            let (month, year) = draft.due_month_and_year();
            format!("{month:02}/{day:02}/{year}")
        });
        let category = match draft.withholding_agent_category {
            WithholdingAgentCategory::Private => "private",
            WithholdingAgentCategory::Government => "government",
        };
        let [tin_segment_1, tin_segment_2, tin_segment_3, tin_branch] =
            draft.printable_tin_segments().unwrap_or_default();

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
                label: format!("{:02}/{}", draft.month, draft.taxable_year),
            },
        );

        envelope.fields.extend([
            (
                "printable_tin_segment_1".to_string(),
                RenderValue::Text(tin_segment_1),
            ),
            (
                "printable_tin_segment_2".to_string(),
                RenderValue::Text(tin_segment_2),
            ),
            (
                "printable_tin_segment_3".to_string(),
                RenderValue::Text(tin_segment_3),
            ),
            (
                "printable_tin_branch".to_string(),
                RenderValue::Text(tin_branch),
            ),
            (
                "is_amended".to_string(),
                RenderValue::Boolean(draft.is_amended),
            ),
            (
                "any_taxes_withheld".to_string(),
                RenderValue::Boolean(draft.any_taxes_withheld),
            ),
            ("due_date".to_string(), RenderValue::Text(due_date)),
            (
                "atc".to_string(),
                RenderValue::Text(draft.atc_code().to_string()),
            ),
            (
                "tax_type_code".to_string(),
                RenderValue::Text(draft.tax_type_code().to_string()),
            ),
            (
                "line_of_business".to_string(),
                RenderValue::Text(draft.line_of_business.clone()),
            ),
            (
                "registered_address_2".to_string(),
                RenderValue::Text(draft.registered_address_2.clone()),
            ),
            (
                "withholding_agent_category".to_string(),
                RenderValue::Text(category.to_string()),
            ),
            (
                "item_14_amount_of_remittance".to_string(),
                RenderValue::Decimal(draft.item_14_amount_of_remittance),
            ),
            (
                "item_15_amount_remitted_previously".to_string(),
                RenderValue::Decimal(draft.item_15_amount_remitted_previously),
            ),
            (
                "item_16_net_amount_of_remittance".to_string(),
                RenderValue::Decimal(draft.item_16_net_amount_of_remittance),
            ),
            (
                "item_17a_surcharge".to_string(),
                RenderValue::Decimal(draft.item_17a_surcharge),
            ),
            (
                "item_17b_interest".to_string(),
                RenderValue::Decimal(draft.item_17b_interest),
            ),
            (
                "item_17c_compromise".to_string(),
                RenderValue::Decimal(draft.item_17c_compromise),
            ),
            (
                "item_17d_total_penalties".to_string(),
                RenderValue::Decimal(draft.item_17d_total_penalties),
            ),
            (
                "item_18_total_amount_of_remittance".to_string(),
                RenderValue::Decimal(draft.item_18_total_amount_of_remittance),
            ),
            (
                "tax_agent_accreditation_number".to_string(),
                RenderValue::Text(draft.tax_agent_accreditation_number.clone()),
            ),
            (
                "tax_agent_date_of_issue".to_string(),
                RenderValue::Text(draft.tax_agent_date_of_issue.clone()),
            ),
            (
                "tax_agent_date_of_expiry".to_string(),
                RenderValue::Text(draft.tax_agent_date_of_expiry.clone()),
            ),
            (
                "payment_22_particular".to_string(),
                RenderValue::Text(draft.payment_details.others_description.clone()),
            ),
        ]);

        insert_payment_row(
            &mut envelope,
            "payment_19",
            &draft.payment_details.cash_or_bank_debit_memo,
        );
        insert_payment_row(&mut envelope, "payment_20", &draft.payment_details.check);
        insert_payment_row(
            &mut envelope,
            "payment_21",
            &draft.payment_details.tax_debit_memo,
        );
        insert_payment_row(&mut envelope, "payment_22", &draft.payment_details.others);

        envelope.validation = draft
            .validate()
            .into_iter()
            .map(|(field_path, message)| RenderValidationMessage {
                field_path,
                code: "invalid".to_string(),
                message,
                severity: RenderValidationSeverity::Error,
                rule_version: "0619e-main-v1".to_string(),
            })
            .collect();
        envelope
    }
}

fn insert_payment_row(envelope: &mut RenderEnvelopeV1, prefix: &str, row: &Form0619EPaymentRow) {
    envelope.fields.insert(
        format!("{prefix}_drawee_bank_or_agency"),
        RenderValue::Text(row.drawee_bank_or_agency.clone()),
    );
    envelope.fields.insert(
        format!("{prefix}_number"),
        RenderValue::Text(row.number.clone()),
    );
    envelope.fields.insert(
        format!("{prefix}_date"),
        RenderValue::Text(row.date.clone()),
    );
    envelope.fields.insert(
        format!("{prefix}_amount_present"),
        RenderValue::Boolean(row.amount.is_some()),
    );
    envelope.fields.insert(
        format!("{prefix}_amount"),
        RenderValue::Decimal(row.amount.unwrap_or_default()),
    );
}

fn fixtures() -> Result<Vec<RenderContractFixture>, RenderProviderError> {
    Ok(vec![
        fixture(
            "0619e-minimum.json",
            RenderFixtureKind::Minimum,
            true,
            minimum_fixture()?,
        ),
        fixture(
            "0619e-normal.json",
            RenderFixtureKind::Normal,
            true,
            normal_fixture()?,
        ),
        fixture(
            "0619e-long-values.json",
            RenderFixtureKind::LongValues,
            true,
            long_values_fixture()?,
        ),
        fixture(
            "0619e-validation-edge.json",
            RenderFixtureKind::ValidationEdge,
            false,
            validation_edge_fixture()?,
        ),
        fixture(
            "0619e-payment.json",
            RenderFixtureKind::ScheduleCapacity,
            true,
            payment_fixture()?,
        ),
    ])
}

fn fixture(
    file_name: &'static str,
    kind: RenderFixtureKind,
    expected_form_valid: bool,
    draft: Form0619EDraft,
) -> RenderContractFixture {
    RenderContractFixture {
        file_name,
        kind,
        expected_form_valid,
        envelope: RenderEnvelopeV1::from(&draft),
    }
}

fn base_fixture() -> Result<Form0619EDraft, RenderProviderError> {
    let profile: TaxpayerProfile = serde_json::from_value(json!({
        "id": null,
        "full_name": "JUAN DELA CRUZ",
        "tin": {
            "segment1": "123",
            "segment2": "456",
            "segment3": "789",
            "branch": "00000"
        },
        "rdo_code": "018",
        "line_of_business": "SOFTWARE DEVELOPMENT",
        "registered_address": "53 SANTOL EXTENSION, NEW CABALAN",
        "zip_code": "2200",
        "phone": "09123456789",
        "email": "renderer.0619e@example.com",
        "default_form_type": "0619Ev2018",
        "taxpayer_type": "Individual"
    }))?;
    let mut draft = Form0619EDraft::new_from_profile(&profile, 2026, 4);
    draft.due_day = Some(10);
    draft.registered_address_2 = "OLONGAPO CITY, ZAMBALES".to_string();
    draft.recompute();
    Ok(draft)
}

fn minimum_fixture() -> Result<Form0619EDraft, RenderProviderError> {
    base_fixture()
}

fn normal_fixture() -> Result<Form0619EDraft, RenderProviderError> {
    let mut draft = base_fixture()?;
    draft.is_amended = true;
    draft.any_taxes_withheld = true;
    draft.item_14_amount_of_remittance = 125_000.0;
    draft.item_15_amount_remitted_previously = 5_000.0;
    draft.item_17a_surcharge = 2_500.0;
    draft.item_17b_interest = 750.0;
    draft.item_17c_compromise = 1_000.0;
    draft.payment_details.cash_or_bank_debit_memo = Form0619EPaymentRow {
        drawee_bank_or_agency: "AAB 018".to_string(),
        number: "PAY-0619E-001".to_string(),
        date: "05/10/2026".to_string(),
        amount: Some(124_250.0),
    };
    draft.tax_agent_accreditation_number = "12-3456-789".to_string();
    draft.tax_agent_date_of_issue = "01/15/2024".to_string();
    draft.tax_agent_date_of_expiry = "01/15/2027".to_string();
    draft.recompute();
    Ok(draft)
}

fn long_values_fixture() -> Result<Form0619EDraft, RenderProviderError> {
    let mut draft = normal_fixture()?;
    draft.taxpayer_name = "LONG REGISTERED WITHHOLDING AGENT NAME THAT EXCEEDS THE OFFICIAL CHARACTER COMB CAPACITY INCORPORATED"
        .to_string();
    draft.registered_address = "UNIT 1201, A DELIBERATELY LONG REGISTERED ADDRESS USED TO PROVE THAT THE SEMANTIC HTML RENDERER PRESERVES EVERY VALID CHARACTER"
        .to_string();
    draft.registered_address_2 =
        "BARANGAY NEW CABALAN, OLONGAPO CITY, ZAMBALES, PHILIPPINES".to_string();
    draft.email =
        "long.expanded.withholding.renderer.verification.address@example.test".to_string();
    draft.line_of_business =
        "SOFTWARE DEVELOPMENT AND INFORMATION TECHNOLOGY CONSULTING SERVICES".to_string();
    draft
        .payment_details
        .cash_or_bank_debit_memo
        .drawee_bank_or_agency = "AUTHORIZED AGENT BANK LONG BRANCH NAME".to_string();
    draft.payment_details.cash_or_bank_debit_memo.number =
        "PAYMENT-REFERENCE-0619E-2026-0000000001".to_string();
    draft.tax_agent_accreditation_number =
        "LONG-TAX-AGENT-ACCREDITATION-REFERENCE-0619E".to_string();
    draft.recompute();
    Ok(draft)
}

fn validation_edge_fixture() -> Result<Form0619EDraft, RenderProviderError> {
    let mut draft = base_fixture()?;
    draft.due_day = None;
    draft.any_taxes_withheld = false;
    draft.is_amended = true;
    draft.item_14_amount_of_remittance = 100.0;
    draft.item_15_amount_remitted_previously = 150.0;
    draft.recompute();
    Ok(draft)
}

fn payment_fixture() -> Result<Form0619EDraft, RenderProviderError> {
    let mut draft = normal_fixture()?;
    draft.payment_details.cash_or_bank_debit_memo = payment_row("AAB", "BDM-001", 31_000.0);
    draft.payment_details.check = payment_row("DBP", "CHECK-002", 31_000.0);
    draft.payment_details.tax_debit_memo = payment_row("BIR", "TDM-003", 31_000.0);
    draft.payment_details.others = payment_row("RCO", "OTHER-004", 31_250.0);
    draft.payment_details.others_description = "REVENUE COLLECTION OFFICER".to_string();
    Ok(draft)
}

fn payment_row(agency: &str, number: &str, amount: f64) -> Form0619EPaymentRow {
    Form0619EPaymentRow {
        drawee_bank_or_agency: agency.to_string(),
        number: number.to_string(),
        date: "05/10/2026".to_string(),
        amount: Some(amount),
    }
}

fn generated_artifacts() -> Result<Vec<GeneratedContractArtifact>, RenderProviderError> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_maps_rust_owned_identity_fixed_codes_and_formulas() {
        let draft = normal_fixture().expect("normal fixture");
        let envelope = RenderEnvelopeV1::from(&draft);

        assert_eq!(envelope.form.code, "0619E");
        assert_eq!(envelope.form.version, "2018");
        assert_eq!(envelope.period.month, Some(4));
        assert_eq!(
            envelope.fields["due_date"],
            RenderValue::Text("05/10/2026".into())
        );
        assert_eq!(envelope.fields["atc"], RenderValue::Text("WME10".into()));
        assert_eq!(
            envelope.fields["tax_type_code"],
            RenderValue::Text("WE".into())
        );
        assert_eq!(
            envelope.fields["item_16_net_amount_of_remittance"],
            RenderValue::Decimal(120_000.0)
        );
        assert_eq!(
            envelope.fields["item_17d_total_penalties"],
            RenderValue::Decimal(4_250.0)
        );
        assert_eq!(
            envelope.fields["item_18_total_amount_of_remittance"],
            RenderValue::Decimal(124_250.0)
        );
        assert!(envelope.schedules.is_empty());
        assert!(envelope.validation.is_empty());
    }

    #[test]
    fn discrete_artwork_provenance_matches_verified_official_xobjects() {
        let assets = runtime_discrete_assets();
        assert_eq!(assets.len(), 2);

        let seal = &assets[0];
        assert_eq!(seal["asset"], json!("government_seal"));
        assert_eq!(seal["source_pdf_object_id"], json!([41, 0]));
        assert_eq!(seal["source_pixel_dimensions"], json!([86, 78]));
        assert_eq!(
            seal["source_ctm_points"],
            json!([31.08, 0.0, 0.0, 28.2, 232.2, 738.96])
        );
        assert_eq!(
            seal["source_png_sha256"],
            json!("6cd637975f8088cc8b56aa67c26794db23331864a45c08517c66b70f53ff2610")
        );

        let pdf417 = &assets[1];
        assert_eq!(pdf417["asset"], json!("static_form_pdf417_page_1"));
        assert_eq!(pdf417["source_pdf_object_id"], json!([42, 0]));
        assert_eq!(pdf417["source_pixel_dimensions"], json!([243, 64]));
        assert_eq!(pdf417["source_active_pixel_bounds"], json!([0, 0, 240, 63]));
        assert_eq!(
            pdf417["source_ctm_points"],
            json!([136.68, 0.0, 0.0, 36.0, 454.8, 690.96])
        );
        assert_eq!(pdf417["decoded_payload"], json!("0619-E 01/18 P1"));
        assert_eq!(pdf417["symbology"], json!("PDF417"));
        assert_eq!(pdf417["logical_black_modules"], json!(480));
        assert_eq!(pdf417["logical_dimensions"], json!([120, 7]));
        assert_eq!(
            pdf417["logical_matrix_sha256"],
            json!("0238537d56f19276b790a8395c429a9f7645ac570eecc097064051b456dc5dfa")
        );
        assert_eq!(pdf417["source_module_scale_pixels"], json!([2, 9]));
        assert_eq!(pdf417["encoder_proof"]["module_differences"], json!(0));
    }

    #[test]
    fn printable_tin_segments_match_xml_for_supported_lengths() {
        for (raw_tin, expected) in [
            ("123-456-789-123", ["123", "456", "789", "00123"]),
            ("123-456-789-1234", ["123", "456", "789", "01234"]),
            ("123-456-789-12345", ["123", "456", "789", "12345"]),
        ] {
            let mut draft = normal_fixture().expect("normal fixture");
            draft.tin = raw_tin.to_string();
            let envelope = RenderEnvelopeV1::from(&draft);
            let xml_fields = draft.to_bir_field_map();

            for ((render_key, xml_key), expected_segment) in [
                ("printable_tin_segment_1", "frm0619E:txtTIN1"),
                ("printable_tin_segment_2", "frm0619E:txtTIN2"),
                ("printable_tin_segment_3", "frm0619E:txtTIN3"),
                ("printable_tin_branch", "frm0619E:txtBranchCode"),
            ]
            .into_iter()
            .zip(expected)
            {
                assert_eq!(
                    envelope.fields[render_key],
                    RenderValue::Text(expected_segment.to_string())
                );
                assert_eq!(xml_fields[xml_key], expected_segment);
            }
            assert_eq!(draft.tin, raw_tin);
            assert!(envelope.validation.is_empty());
        }
    }

    #[test]
    fn invalid_tin_lengths_blank_printable_segments_and_fail_validation() {
        for raw_tin in ["12345678912", "123456789123456"] {
            let mut draft = normal_fixture().expect("normal fixture");
            draft.tin = raw_tin.to_string();
            let envelope = RenderEnvelopeV1::from(&draft);

            for key in [
                "printable_tin_segment_1",
                "printable_tin_segment_2",
                "printable_tin_segment_3",
                "printable_tin_branch",
            ] {
                assert_eq!(envelope.fields[key], RenderValue::Text(String::new()));
            }
            assert!(envelope
                .validation
                .iter()
                .any(|message| message.field_path == "tin"));
            assert_eq!(draft.tin, raw_tin);
        }
    }

    #[test]
    fn payment_presence_distinguishes_official_blank_from_explicit_zero() {
        let mut draft = minimum_fixture().expect("minimum fixture");
        draft.payment_details.check.amount = Some(0.0);
        let envelope = RenderEnvelopeV1::from(&draft);

        assert_eq!(
            envelope.fields["payment_19_amount_present"],
            RenderValue::Boolean(false)
        );
        assert_eq!(
            envelope.fields["payment_20_amount_present"],
            RenderValue::Boolean(true)
        );
        assert_eq!(
            envelope.fields["payment_20_amount"],
            RenderValue::Decimal(0.0)
        );
    }

    #[test]
    fn fixtures_cover_required_one_page_matrix() {
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
            assert_eq!(PROVIDER.expected_page_count(&fixture.envelope).unwrap(), 1);
            assert_eq!(
                fixture.expected_form_valid,
                fixture.envelope.validation.is_empty()
            );
        }
    }
}
