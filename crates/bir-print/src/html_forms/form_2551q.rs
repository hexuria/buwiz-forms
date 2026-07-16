use std::collections::BTreeMap;

use bir_core::forms::{
    form_2551q::{
        AnnualIncomeTaxElection, Form2551QDraft, Item13Election, OverpaymentDisposition,
        Schedule1Row, TaxPeriodBasis,
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

const SCHEDULES: &[RenderSchedulePolicy] = &[RenderSchedulePolicy {
    id: "schedule_1",
    minimum_rows: 6,
    first_page_rows: 6,
    continuation_page_rows: 12,
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
    visual_fixture_sha256: "f3d49ddab5cdd7c1d889a7b2cbd519babf7556c186702f0232b9f18257f7a5b7",
    official_source:
        "https://bir-cdn.bir.gov.ph/local/pdf/2551Q%20Jan%202018%20ENCS%20final%20rev%203_copy.pdf",
    official_source_sha256: "1f270ecf66d778836a14697863e420ff65d5ed0a5576a6cf58b97c9a8e8c9b24",
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
            "crop_box_px": [485, 42, 540, 100],
            "derived_png_sha256": "9ac67b7ae9b242f0edfea91e8c68f67740fdf3b1e106e853f47e126f10e99d1b",
            "embedded_in": "packages/form-renderer/src/forms/official2551QAssets.ts",
            "source_page": 1,
            "treatment": "16-color grayscale quantization"
        }),
        json!({
            "asset": "static_form_barcode_page_1",
            "crop_box_px": [845, 111, 1170, 212],
            "derived_png_sha256": "875dc5520cdbd5c9be0c57980ac87db6e48c1b675d76881624a18f87df0e1683",
            "embedded_in": "packages/form-renderer/src/forms/official2551QAssets.ts",
            "source_page": 1,
            "treatment": "monochrome threshold at 180"
        }),
        json!({
            "asset": "static_form_barcode_page_2",
            "crop_box_px": [845, 90, 1170, 195],
            "derived_png_sha256": "9db0d92ee3216af46eedab3993ca83ed1550b8c792b8bd7ad06fd2348b553dd8",
            "embedded_in": "packages/form-renderer/src/forms/official2551QAssets.ts",
            "source_page": 2,
            "treatment": "monochrome threshold at 180"
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
        "taxpayer_name": "Renderer Fixture Corporation",
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
    draft.recompute(None);
    Ok(draft)
}

fn minimum_fixture() -> Result<Form2551QDraft, RenderProviderError> {
    let mut draft = fixture_with_rows(1)?;
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
