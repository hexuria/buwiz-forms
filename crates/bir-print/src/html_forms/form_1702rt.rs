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

/// Exact January 2018 semantic HTML provider for the regular-rate corporate return.
///
/// The reviewed editable save proves the fixed capacities represented by the
/// typed draft. It does not prove an electronic submission contract or any
/// continuation-sheet geometry, so this provider always emits exactly four
/// pages and no renderer-owned continuation schedules.
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
    runtime_discrete_assets,
    fixtures,
    generated_artifacts,
};

fn runtime_discrete_assets() -> Vec<serde_json::Value> {
    vec![
        json!({
            "asset": "government_seal",
            "crop_box_px": [454, 56, 518, 120],
            "derived_png_sha256": "b2589da93414a92b35398350d9b399d92b4519e0dd754de347bb3e69f4c6a5cf",
            "embedded_in": "packages/form-renderer/src/forms/assets/1702rt-seal.png",
            "source_page": 1,
            "treatment": "exact lossless crop from official 144 DPI source raster"
        }),
        json!({
            "asset": "static_form_barcode_page_1",
            "crop_box_px": [860, 132, 1190, 232],
            "derived_png_sha256": "82f01bc21a58c1a6f24759bd169682058f125732b741e4c939bb7aaade4b191e",
            "embedded_in": "packages/form-renderer/src/forms/assets/1702rt-barcode-page-1.png",
            "source_page": 1,
            "treatment": "exact lossless crop from official 144 DPI source raster"
        }),
        json!({
            "asset": "static_form_barcode_page_2",
            "crop_box_px": [860, 176, 1190, 276],
            "derived_png_sha256": "b7bf73efa81a194b511f192b4b82ab2c4e7b236b633b6040687615c54fdb640a",
            "embedded_in": "packages/form-renderer/src/forms/assets/1702rt-barcode-page-2.png",
            "source_page": 2,
            "treatment": "exact lossless crop from official 144 DPI source raster"
        }),
        json!({
            "asset": "static_form_barcode_page_3",
            "crop_box_px": [860, 75, 1190, 175],
            "derived_png_sha256": "788994dd2130921883c7203d12f9041f1db948c9e8e09854684f746f18fbc604",
            "embedded_in": "packages/form-renderer/src/forms/assets/1702rt-barcode-page-3.png",
            "source_page": 3,
            "treatment": "exact lossless crop from official 144 DPI source raster"
        }),
        json!({
            "asset": "static_form_barcode_page_4",
            "crop_box_px": [860, 75, 1190, 175],
            "derived_png_sha256": "0c2af9a1af251a034d7f2e5c18930c9a6f03712c3ee23ff7b16532675e46414d",
            "embedded_in": "packages/form-renderer/src/forms/assets/1702rt-barcode-page-4.png",
            "source_page": 4,
            "treatment": "exact lossless crop from official 144 DPI source raster"
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
