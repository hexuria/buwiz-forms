use bir_core::{
    forms::{
        form_0605::{
            Form0605ApprovalSelection, Form0605Date, Form0605Draft, Form0605FilingBasis,
            Form0605MannerOfPayment, Form0605ReviewedAtc, Form0605ReviewedTaxType,
            Form0605TaxpayerClassification, Form0605TypeOfPayment,
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
        file_name: "0605-1999-page-1.png",
        sha256: "419d85b55b05fba3e789ab9ded877cdd0514239ef998f9a15258d9ff78251f84",
    },
    VisualReferencePage {
        page: 2,
        file_name: "0605-1999-page-2.png",
        sha256: "d1066f9ece7ba7b670f547310851bad94605ba766668d7b9f2097bfefcd02792",
    },
];

pub(super) const PROVIDER: RenderFormProvider = RenderFormProvider {
    code: "0605",
    revision: "1999",
    form_id: "0605v1999",
    title: "Payment Form",
    page_width_pt: RenderPageGeometry::LEGAL.width_points,
    page_height_pt: RenderPageGeometry::LEGAL.height_points,
    expected_base_page_count: 2,
    schedules: &[],
    visual_fixture_file_name: "0605-normal.json",
    visual_fixture_sha256: "3f76b5f1ae518f81a625b07cd20976b7620bee7b0b1cead224f44af4d20887db",
    official_source: "https://bir-cdn.bir.gov.ph/local/pdf/0605version1999_09.02.2022_copy.pdf",
    official_source_sha256: "de04419766c59bf27fdeb854c0f7c3f98601900caa20630442e671e2313e536f",
    reference_dpi: 144,
    reference_width_px: 1_224,
    reference_height_px: 1_872,
    visual_reference_pages: VISUAL_REFERENCE_PAGES,
    runtime_discrete_assets,
    fixtures,
    generated_artifacts,
};

fn runtime_discrete_assets() -> Vec<serde_json::Value> {
    vec![json!({
        "asset": "government_seal",
        "crop_box_px": [38, 108, 102, 167],
        "derived_png_sha256": "83e123363cded65b1037cfa124e8236db5cf3b93943cf31ac9042084667591b1",
        "embedded_in": "packages/form-renderer/src/forms/assets/0605-seal.png",
        "source_page": 1,
        "treatment": "exact lossless crop from official 144 DPI source raster"
    })]
}

impl From<&Form0605Draft> for RenderEnvelopeV1 {
    fn from(draft: &Form0605Draft) -> Self {
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
                quarter: Some(draft.quarter),
                label: format!(
                    "Q{} year ended {:02}/{}",
                    draft.quarter, draft.year_end_month, draft.taxable_year
                ),
            },
        );

        envelope.fields.extend([
            (
                "filing_basis".to_string(),
                RenderValue::Text(
                    match draft.filing_basis {
                        Form0605FilingBasis::Calendar => "calendar",
                        Form0605FilingBasis::Fiscal => "fiscal",
                    }
                    .to_string(),
                ),
            ),
            (
                "quarter".to_string(),
                RenderValue::Integer(i64::from(draft.quarter)),
            ),
            (
                "year_end_month".to_string(),
                RenderValue::Integer(i64::from(draft.year_end_month)),
            ),
            (
                "due_date".to_string(),
                RenderValue::Text(
                    draft
                        .due_date
                        .map_or_else(String::new, |date| date.to_string()),
                ),
            ),
            (
                "return_period".to_string(),
                RenderValue::Text(
                    draft
                        .return_period
                        .map_or_else(String::new, |date| date.to_string()),
                ),
            ),
            (
                "number_of_sheets".to_string(),
                RenderValue::Integer(i64::from(draft.number_of_sheets)),
            ),
            (
                "atc".to_string(),
                RenderValue::Text(
                    draft
                        .atc
                        .as_ref()
                        .map_or_else(String::new, |selection| selection.code().to_string()),
                ),
            ),
            (
                "tax_type_code".to_string(),
                RenderValue::Text(
                    draft
                        .tax_type
                        .as_ref()
                        .map_or_else(String::new, |selection| selection.code().to_string()),
                ),
            ),
            (
                "taxpayer_classification".to_string(),
                RenderValue::Text(
                    match draft.classification {
                        Form0605TaxpayerClassification::Individual => "individual",
                        Form0605TaxpayerClassification::NonIndividual => "non_individual",
                    }
                    .to_string(),
                ),
            ),
            (
                "line_of_business".to_string(),
                RenderValue::Text(draft.line_of_business.clone()),
            ),
            (
                "manner_of_payment".to_string(),
                RenderValue::Text(manner_of_payment_value(draft.manner_of_payment).to_string()),
            ),
            (
                "other_manner_description".to_string(),
                RenderValue::Text(draft.other_manner_description.clone()),
            ),
            (
                "type_of_payment".to_string(),
                RenderValue::Text(type_of_payment_value(draft.type_of_payment).to_string()),
            ),
            (
                "number_of_installments_present".to_string(),
                RenderValue::Boolean(draft.number_of_installments.is_some()),
            ),
            (
                "number_of_installments".to_string(),
                RenderValue::Integer(i64::from(draft.number_of_installments.unwrap_or_default())),
            ),
            (
                "item_19_basic_tax_or_payment".to_string(),
                RenderValue::Decimal(draft.item_19_basic_tax_or_payment),
            ),
            (
                "item_20a_surcharge".to_string(),
                RenderValue::Decimal(draft.item_20a_surcharge),
            ),
            (
                "item_20b_interest".to_string(),
                RenderValue::Decimal(draft.item_20b_interest),
            ),
            (
                "item_20c_compromise".to_string(),
                RenderValue::Decimal(draft.item_20c_compromise),
            ),
            (
                "item_20d_total_penalties".to_string(),
                RenderValue::Decimal(draft.item_20d_total_penalties),
            ),
            (
                "item_21_total_amount_payable".to_string(),
                RenderValue::Decimal(draft.item_21_total_amount_payable),
            ),
            (
                "approval_selection".to_string(),
                RenderValue::Text(
                    match draft.approval_selection {
                        Form0605ApprovalSelection::None => "none",
                        Form0605ApprovalSelection::XmlOption1 => "xml_option_1",
                        Form0605ApprovalSelection::XmlOption2 => "xml_option_2",
                    }
                    .to_string(),
                ),
            ),
            (
                "signature_taxpayer_or_representative".to_string(),
                RenderValue::Text(
                    draft
                        .signatures
                        .taxpayer_or_authorized_representative
                        .clone(),
                ),
            ),
            (
                "signature_title_or_position".to_string(),
                RenderValue::Text(draft.signatures.title_or_position.clone()),
            ),
            (
                "signature_head_of_office".to_string(),
                RenderValue::Text(draft.signatures.head_of_office.clone()),
            ),
            (
                "payment_23_amount_present".to_string(),
                RenderValue::Boolean(
                    draft
                        .payment_details
                        .cash_or_bank_debit_memo_amount
                        .is_some(),
                ),
            ),
            (
                "payment_23_amount".to_string(),
                RenderValue::Decimal(
                    draft
                        .payment_details
                        .cash_or_bank_debit_memo_amount
                        .unwrap_or_default(),
                ),
            ),
            (
                "machine_validation_or_receipt_details".to_string(),
                RenderValue::Text(
                    draft
                        .payment_details
                        .machine_validation_or_receipt_details
                        .clone(),
                ),
            ),
        ]);

        insert_payment_row(
            &mut envelope,
            "payment_24",
            &draft.payment_details.check.drawee_bank_or_agency,
            &draft.payment_details.check.number,
            &draft.payment_details.check.date,
            draft.payment_details.check.amount,
        );
        insert_payment_row(
            &mut envelope,
            "payment_25",
            "",
            &draft.payment_details.tax_debit_memo.number,
            &draft.payment_details.tax_debit_memo.date,
            draft.payment_details.tax_debit_memo.amount,
        );
        insert_payment_row(
            &mut envelope,
            "payment_26",
            &draft.payment_details.others.drawee_bank_or_agency,
            &draft.payment_details.others.number,
            &draft.payment_details.others.date,
            draft.payment_details.others.amount,
        );

        envelope.validation = draft
            .validate()
            .into_iter()
            .map(|(field_path, message)| RenderValidationMessage {
                field_path,
                code: "invalid".to_string(),
                message,
                severity: RenderValidationSeverity::Error,
                rule_version: "0605-main-v1".to_string(),
            })
            .collect();
        envelope
    }
}

fn manner_of_payment_value(value: Option<Form0605MannerOfPayment>) -> &'static str {
    match value {
        None => "",
        Some(Form0605MannerOfPayment::SelfAssessment) => "self_assessment",
        Some(Form0605MannerOfPayment::TaxDepositOrAdvancePayment) => "tax_deposit",
        Some(Form0605MannerOfPayment::IncomeTaxSecondInstallmentIndividual) => {
            "income_tax_second_installment"
        }
        Some(Form0605MannerOfPayment::Penalties) => "penalties",
        Some(Form0605MannerOfPayment::Others) => "others",
        Some(Form0605MannerOfPayment::PreliminaryOrFinalAssessmentOrDeficiencyTax) => {
            "assessment_or_deficiency"
        }
        Some(Form0605MannerOfPayment::AccountsReceivableOrDelinquentAccount) => {
            "accounts_receivable_or_delinquent"
        }
    }
}

fn type_of_payment_value(value: Option<Form0605TypeOfPayment>) -> &'static str {
    match value {
        None => "",
        Some(Form0605TypeOfPayment::Installment) => "installment",
        Some(Form0605TypeOfPayment::PartialPayment) => "partial",
        Some(Form0605TypeOfPayment::FullPayment) => "full",
    }
}

fn insert_payment_row(
    envelope: &mut RenderEnvelopeV1,
    prefix: &str,
    drawee_bank_or_agency: &str,
    number: &str,
    date: &str,
    amount: Option<f64>,
) {
    envelope.fields.insert(
        format!("{prefix}_drawee_bank_or_agency"),
        RenderValue::Text(drawee_bank_or_agency.to_string()),
    );
    envelope.fields.insert(
        format!("{prefix}_number"),
        RenderValue::Text(number.to_string()),
    );
    envelope.fields.insert(
        format!("{prefix}_date"),
        RenderValue::Text(date.to_string()),
    );
    envelope.fields.insert(
        format!("{prefix}_amount_present"),
        RenderValue::Boolean(amount.is_some()),
    );
    envelope.fields.insert(
        format!("{prefix}_amount"),
        RenderValue::Decimal(amount.unwrap_or_default()),
    );
}

fn fixtures() -> Result<Vec<RenderContractFixture>, RenderProviderError> {
    Ok(vec![
        fixture(
            "0605-minimum.json",
            RenderFixtureKind::Minimum,
            true,
            minimum_fixture()?,
        ),
        fixture(
            "0605-normal.json",
            RenderFixtureKind::Normal,
            true,
            normal_fixture()?,
        ),
        fixture(
            "0605-long-values.json",
            RenderFixtureKind::LongValues,
            true,
            long_values_fixture()?,
        ),
        fixture(
            "0605-validation-edge.json",
            RenderFixtureKind::ValidationEdge,
            false,
            validation_edge_fixture()?,
        ),
        fixture(
            "0605-variant.json",
            RenderFixtureKind::ScheduleCapacity,
            true,
            variant_fixture()?,
        ),
    ])
}

fn fixture(
    file_name: &'static str,
    kind: RenderFixtureKind,
    expected_form_valid: bool,
    draft: Form0605Draft,
) -> RenderContractFixture {
    RenderContractFixture {
        file_name,
        kind,
        expected_form_valid,
        envelope: RenderEnvelopeV1::from(&draft),
    }
}

fn base_fixture() -> Result<Form0605Draft, RenderProviderError> {
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
        "registered_address": "53 SANTOL EXTENSION, NEW CABALAN, OLONGAPO CITY",
        "zip_code": "2200",
        "phone": "09123456789",
        "email": "renderer.0605@example.com",
        "default_form_type": "0605v1999",
        "taxpayer_type": "Individual"
    }))?;
    let mut draft = Form0605Draft::new_from_profile(&profile, 2025, 1);
    draft.quarter = 4;
    draft.year_end_month = 12;
    draft.due_date = Some(date(2026, 1, 31)?);
    draft.return_period = Some(date(2025, 12, 31)?);
    draft.select_reviewed_atc(Form0605ReviewedAtc::Fp010);
    draft.select_reviewed_tax_type(Form0605ReviewedTaxType::Do);
    draft.manner_of_payment = Some(Form0605MannerOfPayment::SelfAssessment);
    draft.type_of_payment = Some(Form0605TypeOfPayment::FullPayment);
    draft.recompute();
    Ok(draft)
}

fn minimum_fixture() -> Result<Form0605Draft, RenderProviderError> {
    base_fixture()
}

fn normal_fixture() -> Result<Form0605Draft, RenderProviderError> {
    let mut draft = base_fixture()?;
    draft.filing_basis = Form0605FilingBasis::Fiscal;
    draft.quarter = 2;
    draft.year_end_month = 6;
    draft.due_date = Some(date(2026, 7, 15)?);
    draft.return_period = Some(date(2026, 6, 30)?);
    draft.taxable_year = 2026;
    draft.number_of_sheets = 2;
    draft.select_reviewed_atc(Form0605ReviewedAtc::Ii011);
    draft.select_reviewed_tax_type(Form0605ReviewedTaxType::It);
    draft.item_19_basic_tax_or_payment = 125_000.0;
    draft.item_20a_surcharge = 2_500.0;
    draft.item_20b_interest = 750.0;
    draft.item_20c_compromise = 1_000.0;
    draft.signatures.taxpayer_or_authorized_representative = "JUAN DELA CRUZ".to_string();
    draft.signatures.title_or_position = "OWNER".to_string();
    draft.payment_details.cash_or_bank_debit_memo_amount = Some(129_250.0);
    draft.payment_details.machine_validation_or_receipt_details =
        "AAB RECEIPT 0605-2026-0001".to_string();
    draft.recompute();
    Ok(draft)
}

fn long_values_fixture() -> Result<Form0605Draft, RenderProviderError> {
    let mut draft = normal_fixture()?;
    draft.taxpayer_name = "A DELIBERATELY LONG REGISTERED TAXPAYER NAME THAT EXCEEDS THE OFFICIAL CHARACTER COMB CAPACITY INCORPORATED"
        .to_string();
    draft.line_of_business = "SOFTWARE DEVELOPMENT, INFORMATION TECHNOLOGY CONSULTING, DATA PROCESSING, AND RELATED PROFESSIONAL SERVICES"
        .to_string();
    draft.registered_address = "UNIT 1201, A DELIBERATELY LONG REGISTERED ADDRESS THAT MUST REMAIN COMPLETE, BUILDING ONE, NEW CABALAN, OLONGAPO CITY, ZAMBALES"
        .to_string();
    draft.email = "long.payment.form.renderer.verification.address@example.test".to_string();
    draft.signatures.taxpayer_or_authorized_representative =
        "A DELIBERATELY LONG AUTHORIZED REPRESENTATIVE NAME".to_string();
    draft.signatures.title_or_position =
        "AUTHORIZED REPRESENTATIVE AND FINANCE OFFICER".to_string();
    draft.payment_details.machine_validation_or_receipt_details = "A DELIBERATELY LONG MACHINE VALIDATION OR REVENUE OFFICIAL RECEIPT DESCRIPTION THAT MUST NOT BE TRUNCATED"
        .to_string();
    Ok(draft)
}

fn validation_edge_fixture() -> Result<Form0605Draft, RenderProviderError> {
    let mut draft = base_fixture()?;
    draft.due_date = None;
    draft.return_period = None;
    draft.atc = None;
    draft.tax_type = None;
    draft.manner_of_payment = Some(Form0605MannerOfPayment::Others);
    draft.other_manner_description.clear();
    draft.type_of_payment = Some(Form0605TypeOfPayment::Installment);
    draft.number_of_installments = None;
    draft.item_20a_surcharge = 100.0;
    draft.item_20d_total_penalties = 0.0;
    draft.item_21_total_amount_payable = 0.0;
    Ok(draft)
}

fn variant_fixture() -> Result<Form0605Draft, RenderProviderError> {
    let mut draft = normal_fixture()?;
    draft.classification = Form0605TaxpayerClassification::NonIndividual;
    draft.manner_of_payment = Some(Form0605MannerOfPayment::Others);
    draft.other_manner_description = "VOLUNTARY PAYMENT FOR REVIEWED TRANSACTION".to_string();
    draft.type_of_payment = Some(Form0605TypeOfPayment::Installment);
    draft.number_of_installments = Some(3);
    draft.approval_selection = Form0605ApprovalSelection::XmlOption1;
    draft.signatures.head_of_office = "HEAD OF INVESTIGATING OFFICE".to_string();
    draft.payment_details.cash_or_bank_debit_memo_amount = Some(25_000.0);
    draft.payment_details.check.drawee_bank_or_agency = "DEVELOPMENT BANK".to_string();
    draft.payment_details.check.number = "CHECK-0605-0001".to_string();
    draft.payment_details.check.date = "07/15/2026".to_string();
    draft.payment_details.check.amount = Some(25_000.0);
    draft.payment_details.tax_debit_memo.number = "TDM-0605-0002".to_string();
    draft.payment_details.tax_debit_memo.date = "07/15/2026".to_string();
    draft.payment_details.tax_debit_memo.amount = Some(25_000.0);
    draft.payment_details.others.drawee_bank_or_agency = "REVENUE COLLECTION OFFICER".to_string();
    draft.payment_details.others.number = "OTHER-0605-0003".to_string();
    draft.payment_details.others.date = "07/15/2026".to_string();
    draft.payment_details.others.amount = Some(54_250.0);
    Ok(draft)
}

fn date(year: u16, month: u8, day: u8) -> Result<Form0605Date, RenderProviderError> {
    Form0605Date::new(year, month, day).map_err(RenderProviderError::Fixture)
}

fn generated_artifacts() -> Result<Vec<GeneratedContractArtifact>, RenderProviderError> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_maps_rust_owned_identity_dates_choices_and_formulas() {
        let draft = normal_fixture().expect("normal fixture");
        let envelope = RenderEnvelopeV1::from(&draft);

        assert_eq!(envelope.form.code, "0605");
        assert_eq!(envelope.form.version, "1999");
        assert_eq!(envelope.period.quarter, Some(2));
        assert_eq!(
            envelope.fields["due_date"],
            RenderValue::Text("07/15/2026".into())
        );
        assert_eq!(envelope.fields["atc"], RenderValue::Text("II011".into()));
        assert_eq!(
            envelope.fields["tax_type_code"],
            RenderValue::Text("IT".into())
        );
        assert_eq!(
            envelope.fields["item_20d_total_penalties"],
            RenderValue::Decimal(4_250.0)
        );
        assert_eq!(
            envelope.fields["item_21_total_amount_payable"],
            RenderValue::Decimal(129_250.0)
        );
        assert!(envelope.schedules.is_empty());
        assert!(envelope.validation.is_empty());
    }

    #[test]
    fn payment_presence_distinguishes_official_blank_from_entered_zero() {
        let mut draft = minimum_fixture().expect("minimum fixture");
        draft.payment_details.check.amount = Some(0.0);
        let envelope = RenderEnvelopeV1::from(&draft);

        assert_eq!(
            envelope.fields["payment_23_amount_present"],
            RenderValue::Boolean(false)
        );
        assert_eq!(
            envelope.fields["payment_24_amount_present"],
            RenderValue::Boolean(true)
        );
        assert_eq!(
            envelope.fields["payment_24_amount"],
            RenderValue::Decimal(0.0)
        );
    }

    #[test]
    fn fixtures_cover_required_two_page_matrix_without_schedules() {
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
                "{} validity mismatch",
                fixture.file_name
            );
        }
    }

    #[test]
    fn validation_edge_is_exposed_without_typescript_repair() {
        let envelope =
            RenderEnvelopeV1::from(&validation_edge_fixture().expect("validation-edge fixture"));
        for field in [
            "due_date",
            "return_period",
            "atc",
            "tax_type",
            "other_manner_description",
            "number_of_installments",
            "item_20d_total_penalties",
        ] {
            assert!(
                envelope
                    .validation
                    .iter()
                    .any(|issue| issue.field_path == field),
                "missing validation for {field}"
            );
        }
    }
}
