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
    runtime_discrete_assets,
    fixtures,
    generated_artifacts,
};

fn runtime_discrete_assets() -> Vec<serde_json::Value> {
    vec![
        json!({
            "asset": "government_seal",
            "crop_box_px": [457, 20, 519, 78],
            "derived_png_sha256": "e566c3870a95ead4cee13705ec41104aa7dba1fb92c0667b79e2b0666976f060",
            "embedded_in": "packages/form-renderer/src/forms/assets/1601c-seal.png",
            "source_page": 1,
            "treatment": "exact lossless crop from official 144 DPI source raster"
        }),
        json!({
            "asset": "static_form_barcode_page_1",
            "crop_box_px": [881, 96, 1187, 186],
            "derived_png_sha256": "f71a72e73bcbee1e8002e84c103741c2bb640da789fac154dbba556c2d8709cf",
            "embedded_in": "packages/form-renderer/src/forms/assets/1601c-barcode-page-1.png",
            "source_page": 1,
            "treatment": "exact lossless crop from official 144 DPI source raster"
        }),
        json!({
            "asset": "static_form_barcode_page_2",
            "crop_box_px": [879, 68, 1185, 154],
            "derived_png_sha256": "60582db03ad03295798bce5f73ea16c05bc1f0b85906dbc975c4c2bc2feeca03",
            "embedded_in": "packages/form-renderer/src/forms/assets/1601c-barcode-page-2.png",
            "source_page": 2,
            "treatment": "exact lossless crop from official 144 DPI source raster"
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
    draft.tax_relief_specification =
        "Reviewed special-law or international tax treaty description beyond the official comb"
            .to_string();
    draft.tax_20_other_name =
        "Other non-taxable compensation description beyond the official printed line".to_string();
    draft.tax_29_other_remittances_name =
        "Other remittance description beyond the official printed line".to_string();
    for (index, row) in draft.schedule_1.iter_mut().enumerate() {
        row.drawee_bank_code_or_agency =
            format!("Authorized Agent Bank Branch Code Number {:02}", index + 1);
        row.payment_number = format!("PAYMENT-REFERENCE-1601C-2026-{:02}", index + 1);
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
