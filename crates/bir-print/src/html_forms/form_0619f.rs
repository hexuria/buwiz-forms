use bir_core::{
    forms::{
        form_0619f::{Form0619FDraft, Form0619FPaymentRow, WithholdingAgentCategory},
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
    file_name: "0619f-2018-page-1.png",
    sha256: "9ca59837f330ad026f1d90e5b19f7d524945b66314b09ded2644efc508f2b627",
}];

pub(super) const PROVIDER: RenderFormProvider = RenderFormProvider {
    code: "0619F",
    revision: "2018",
    form_id: "0619Fv2018",
    title: "Monthly Remittance Form of Final Income Taxes Withheld",
    page_width_pt: RenderPageGeometry::LETTER.width_points,
    page_height_pt: RenderPageGeometry::LETTER.height_points,
    expected_base_page_count: 1,
    schedules: &[],
    visual_fixture_file_name: "0619f-normal.json",
    visual_fixture_sha256: "e56153e3b269da028de8042dc59f5aa93df142c6b24204299ba9dd137b3e72e6",
    official_source: "https://bir-cdn.bir.gov.ph/local/pdf/0619-F%20Jan%202018%20rev%20final.pdf",
    official_source_sha256: "edd7357390b1f0d95f2a38c9bb76252341c15b54b82bffd338bd540452ff15e1",
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
            "crop_box_px": [464, 31, 524, 86],
            "derived_png_sha256": "b9e962368786a3a08702234fc3e5b6ec98ce89879ef11662d18b9b09753868d2",
            "embedded_in": "packages/form-renderer/src/forms/assets/0619f-seal.png",
            "source_page": 1,
            "treatment": "exact lossless crop from official 144 DPI source raster"
        }),
        json!({
            "asset": "static_form_barcode_page_1",
            "crop_box_px": [907, 99, 1187, 183],
            "derived_png_sha256": "c50c4fc8c50e0136117a9cfc32e905ba9afef1fe5365e0dcbfaebcd6c1855ad8",
            "embedded_in": "packages/form-renderer/src/forms/assets/0619f-barcode-page-1.png",
            "source_page": 1,
            "treatment": "exact lossless crop from official 144 DPI source raster"
        }),
    ]
}

impl From<&Form0619FDraft> for RenderEnvelopeV1 {
    fn from(draft: &Form0619FDraft) -> Self {
        let due_date = draft.due_day.map_or_else(String::new, |day| {
            let (month, year) = draft.due_month_and_year();
            format!("{month:02}/{day:02}/{year}")
        });
        let category = match draft.withholding_agent_category {
            WithholdingAgentCategory::Private => "private",
            WithholdingAgentCategory::Government => "government",
        };

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
                "is_amended".to_string(),
                RenderValue::Boolean(draft.is_amended),
            ),
            (
                "any_taxes_withheld".to_string(),
                RenderValue::Boolean(draft.any_taxes_withheld),
            ),
            ("due_date".to_string(), RenderValue::Text(due_date)),
            (
                "item_13_atc".to_string(),
                RenderValue::Text(draft.item_13_atc_code().to_string()),
            ),
            (
                "item_14_atc".to_string(),
                RenderValue::Text(draft.item_14_atc_code().to_string()),
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
                "item_13_interest_final_tax_withheld".to_string(),
                RenderValue::Decimal(draft.item_13_interest_final_tax_withheld),
            ),
            (
                "item_14_other_final_tax_withheld".to_string(),
                RenderValue::Decimal(draft.item_14_other_final_tax_withheld),
            ),
            (
                "item_15_total".to_string(),
                RenderValue::Decimal(draft.item_15_total),
            ),
            (
                "item_16_remitted_previously".to_string(),
                RenderValue::Decimal(draft.item_16_remitted_previously),
            ),
            (
                "item_17_net_amount_of_remittance".to_string(),
                RenderValue::Decimal(draft.item_17_net_amount_of_remittance),
            ),
            (
                "item_18a_surcharge".to_string(),
                RenderValue::Decimal(draft.item_18a_surcharge),
            ),
            (
                "item_18b_interest".to_string(),
                RenderValue::Decimal(draft.item_18b_interest),
            ),
            (
                "item_18c_compromise".to_string(),
                RenderValue::Decimal(draft.item_18c_compromise),
            ),
            (
                "item_18d_total_penalties".to_string(),
                RenderValue::Decimal(draft.item_18d_total_penalties),
            ),
            (
                "item_19_total_amount_of_remittance".to_string(),
                RenderValue::Decimal(draft.item_19_total_amount_of_remittance),
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
                "payment_23_particular".to_string(),
                RenderValue::Text(draft.payment_details.others_description.clone()),
            ),
        ]);

        insert_payment_row(
            &mut envelope,
            "payment_20",
            &draft.payment_details.cash_or_bank_debit_memo,
        );
        insert_payment_row(&mut envelope, "payment_21", &draft.payment_details.check);
        insert_payment_row(
            &mut envelope,
            "payment_22",
            &draft.payment_details.tax_debit_memo,
        );
        insert_payment_row(&mut envelope, "payment_23", &draft.payment_details.others);

        envelope.validation = draft
            .validate()
            .into_iter()
            .map(|(field_path, message)| RenderValidationMessage {
                field_path,
                code: "invalid".to_string(),
                message,
                severity: RenderValidationSeverity::Error,
                rule_version: "0619f-main-v1".to_string(),
            })
            .collect();
        envelope
    }
}

fn insert_payment_row(envelope: &mut RenderEnvelopeV1, prefix: &str, row: &Form0619FPaymentRow) {
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
            "0619f-minimum.json",
            RenderFixtureKind::Minimum,
            true,
            minimum_fixture()?,
        ),
        fixture(
            "0619f-normal.json",
            RenderFixtureKind::Normal,
            true,
            normal_fixture()?,
        ),
        fixture(
            "0619f-long-values.json",
            RenderFixtureKind::LongValues,
            true,
            long_values_fixture()?,
        ),
        fixture(
            "0619f-validation-edge.json",
            RenderFixtureKind::ValidationEdge,
            false,
            validation_edge_fixture()?,
        ),
        fixture(
            "0619f-all-payments.json",
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
    draft: Form0619FDraft,
) -> RenderContractFixture {
    RenderContractFixture {
        file_name,
        kind,
        expected_form_valid,
        envelope: RenderEnvelopeV1::from(&draft),
    }
}

fn base_fixture() -> Result<Form0619FDraft, RenderProviderError> {
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
        "email": "renderer.0619f@example.com",
        "default_form_type": "0619Fv2018",
        "taxpayer_type": "Individual"
    }))?;
    let mut draft = Form0619FDraft::new_from_profile(&profile, 2026, 4);
    draft.due_day = Some(10);
    draft.registered_address_2 = "OLONGAPO CITY, ZAMBALES".to_string();
    draft.recompute();
    Ok(draft)
}

fn minimum_fixture() -> Result<Form0619FDraft, RenderProviderError> {
    base_fixture()
}

fn normal_fixture() -> Result<Form0619FDraft, RenderProviderError> {
    let mut draft = base_fixture()?;
    draft.is_amended = true;
    draft.any_taxes_withheld = true;
    draft.item_13_interest_final_tax_withheld = 80_000.0;
    draft.item_14_other_final_tax_withheld = 45_000.0;
    draft.item_16_remitted_previously = 5_000.0;
    draft.item_18a_surcharge = 2_500.0;
    draft.item_18b_interest = 750.0;
    draft.item_18c_compromise = 1_000.0;
    draft.payment_details.cash_or_bank_debit_memo = Form0619FPaymentRow {
        drawee_bank_or_agency: "AAB 018".to_string(),
        number: "PAY-0619F-001".to_string(),
        date: "05/10/2026".to_string(),
        amount: Some(124_250.0),
    };
    draft.tax_agent_accreditation_number = "12-3456-789".to_string();
    draft.tax_agent_date_of_issue = "01/15/2024".to_string();
    draft.tax_agent_date_of_expiry = "01/15/2027".to_string();
    draft.recompute();
    Ok(draft)
}

fn long_values_fixture() -> Result<Form0619FDraft, RenderProviderError> {
    let mut draft = normal_fixture()?;
    draft.taxpayer_name = "LONG REGISTERED WITHHOLDING AGENT NAME THAT EXCEEDS THE OFFICIAL CHARACTER COMB CAPACITY INCORPORATED"
        .to_string();
    draft.registered_address = "UNIT 1201, A DELIBERATELY LONG REGISTERED ADDRESS USED TO PROVE THAT THE SEMANTIC HTML RENDERER PRESERVES EVERY VALID CHARACTER"
        .to_string();
    draft.registered_address_2 =
        "BARANGAY NEW CABALAN, OLONGAPO CITY, ZAMBALES, PHILIPPINES".to_string();
    draft.email = "long.final.withholding.renderer.verification.address@example.test".to_string();
    draft.line_of_business =
        "SOFTWARE DEVELOPMENT AND INFORMATION TECHNOLOGY CONSULTING SERVICES".to_string();
    draft
        .payment_details
        .cash_or_bank_debit_memo
        .drawee_bank_or_agency = "AUTHORIZED AGENT BANK LONG BRANCH NAME".to_string();
    draft.payment_details.cash_or_bank_debit_memo.number =
        "PAYMENT-REFERENCE-0619F-2026-0000000001".to_string();
    draft.tax_agent_accreditation_number =
        "LONG-TAX-AGENT-ACCREDITATION-REFERENCE-0619F".to_string();
    draft.item_13_interest_final_tax_withheld = 9_999_999_999_999.0;
    draft.item_14_other_final_tax_withheld = 8_888_888_888_888.0;
    draft.item_16_remitted_previously = 1_111_111_111_111.0;
    draft.item_18a_surcharge = 222_222_222_222.0;
    draft.item_18b_interest = 33_333_333_333.0;
    draft.item_18c_compromise = 4_444_444_444.0;
    draft.recompute();
    draft.payment_details.cash_or_bank_debit_memo.amount =
        Some(draft.item_19_total_amount_of_remittance);
    Ok(draft)
}

fn validation_edge_fixture() -> Result<Form0619FDraft, RenderProviderError> {
    let mut draft = base_fixture()?;
    draft.due_day = None;
    draft.any_taxes_withheld = false;
    draft.is_amended = true;
    draft.item_13_interest_final_tax_withheld = 100.0;
    draft.item_16_remitted_previously = 150.0;
    draft.recompute();
    Ok(draft)
}

fn payment_fixture() -> Result<Form0619FDraft, RenderProviderError> {
    let mut draft = normal_fixture()?;
    draft.payment_details.cash_or_bank_debit_memo = payment_row("AAB", "BDM-001", 31_000.0);
    draft.payment_details.check = payment_row("DBP", "CHECK-002", 31_000.0);
    draft.payment_details.tax_debit_memo = payment_row("BIR", "TDM-003", 31_000.0);
    draft.payment_details.others = payment_row("RCO", "OTHER-004", 31_250.0);
    draft.payment_details.others_description = "REVENUE COLLECTION OFFICER".to_string();
    Ok(draft)
}

fn payment_row(agency: &str, number: &str, amount: f64) -> Form0619FPaymentRow {
    Form0619FPaymentRow {
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

        assert_eq!(envelope.form.code, "0619F");
        assert_eq!(envelope.form.version, "2018");
        assert_eq!(envelope.period.month, Some(4));
        assert_eq!(
            envelope.fields["due_date"],
            RenderValue::Text("05/10/2026".into())
        );
        assert_eq!(
            envelope.fields["item_13_atc"],
            RenderValue::Text("WMF10".into())
        );
        assert_eq!(
            envelope.fields["item_14_atc"],
            RenderValue::Text("WMF20".into())
        );
        assert_eq!(
            envelope.fields["tax_type_code"],
            RenderValue::Text("WB".into())
        );
        assert_eq!(
            envelope.fields["item_17_net_amount_of_remittance"],
            RenderValue::Decimal(120_000.0)
        );
        assert_eq!(
            envelope.fields["item_18d_total_penalties"],
            RenderValue::Decimal(4_250.0)
        );
        assert_eq!(
            envelope.fields["item_19_total_amount_of_remittance"],
            RenderValue::Decimal(124_250.0)
        );
        assert!(envelope.schedules.is_empty());
        assert!(envelope.validation.is_empty());
    }

    #[test]
    fn payment_presence_distinguishes_official_blank_from_explicit_zero() {
        let mut draft = minimum_fixture().expect("minimum fixture");
        draft.payment_details.check.amount = Some(0.0);
        let envelope = RenderEnvelopeV1::from(&draft);

        assert_eq!(
            envelope.fields["payment_20_amount_present"],
            RenderValue::Boolean(false)
        );
        assert_eq!(
            envelope.fields["payment_21_amount_present"],
            RenderValue::Boolean(true)
        );
        assert_eq!(
            envelope.fields["payment_21_amount"],
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
