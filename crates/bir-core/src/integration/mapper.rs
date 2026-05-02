//! Form Mapper Engine — translates `UniversalTaxPayload` into strongly-typed form drafts.
//!
//! Each BIR form has a dedicated mapper that understands how to convert
//! generalized financial data into the specific form's data model.

use crate::forms::atc::find_atc;
use crate::forms::form_2551q::{Form2551QDraft, Schedule1Row};
use crate::integration::models::{IncomeCategory, IncomeSource, UniversalTaxPayload};
use crate::profile::TaxpayerProfile;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MapperError {
    #[error("No applicable form for this payload and profile combination")]
    NoApplicableForm,
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Invalid period: could not derive quarter from period_end")]
    InvalidPeriod,
    #[error("ATC code not found: {0}")]
    AtcNotFound(String),
    #[error("Profile TIN mismatch: expected {expected}, got {actual}")]
    TinMismatch { expected: String, actual: String },
    #[error("Validation failed: {0:?}")]
    ValidationFailed(Vec<(String, String)>),
}

/// Output of a form mapper — a strongly-typed draft ready for computation and persistence.
#[derive(Debug, Clone)]
pub enum FormDraftOutput {
    /// A populated 2551Q (Quarterly Percentage Tax Return) draft.
    Form2551Q(Form2551QDraft),
    // Future variants:
    // Form1701Q(Form1701QDraft),
    // Form1701(Form1701Draft),
    // Form2550M(Form2550MDraft),
}

/// Trait for form-specific mappers.
///
/// Each mapper takes a `UniversalTaxPayload` and a `TaxpayerProfile`,
/// and produces a strongly-typed form draft with all derived values computed.
pub trait FormMapper {
    /// The form schema ID this mapper targets (e.g., "2551Qv2018").
    fn target_schema_id(&self) -> &'static str;

    /// The form code this mapper targets (e.g., "2551Q").
    fn target_form_code(&self) -> &'static str;

    /// Map a universal payload into a form draft output.
    ///
    /// The mapper:
    /// 1. Creates a draft from the profile (pre-filling header fields)
    /// 2. Overlays income sources as schedule rows
    /// 3. Calls `recompute()` to derive all calculated fields
    /// 4. Returns the populated draft
    fn map(
        &self,
        payload: &UniversalTaxPayload,
        profile: &TaxpayerProfile,
    ) -> Result<FormDraftOutput, MapperError>;
}

/// Mapper for BIR Form 2551Q — Quarterly Percentage Tax Return.
///
/// Maps `UniversalTaxPayload` income sources to Schedule 1 ATC rows,
/// applies creditable withholdings, and triggers auto-computation of
/// tax due, penalties, and total amount payable.
pub struct Mapper2551Q;

impl Mapper2551Q {
    /// Determines the ATC code for an income source based on the category
    /// and taxpayer profile.
    fn resolve_atc_code(source: &IncomeSource) -> &'static str {
        // If the source has an explicit override, validate and use it
        if let Some(ref override_code) = source.atc_code_override
            && find_atc(override_code).is_some()
        {
            // We can't return a reference to a String field, so we match
            // against known codes. For unknown overrides, fall through.
            return match override_code.as_str() {
                "PT010" => "PT010",
                "PT040" => "PT040",
                "PT050" => "PT050",
                "PT060" => "PT060",
                "PT070" => "PT070",
                "PT080" => "PT080",
                "PT090" => "PT090",
                "PT100" => "PT100",
                "PT110" => "PT110",
                "PT120" => "PT120",
                "PT130" => "PT130",
                "PT140" => "PT140",
                "PT150" => "PT150",
                "PT160" => "PT160",
                _ => "PT010", // Unknown override — fall back to default
            };
        }

        // Auto-detect based on income category
        match source.category {
            IncomeCategory::BusinessNonVat => "PT010",
            IncomeCategory::ProfessionalServices => "PT010",
            IncomeCategory::PassiveIncome => "PT080", // Bank/financial intermediary rate
            IncomeCategory::CapitalGains => "PT140",  // Stock transactions
            IncomeCategory::Other(_) => "PT010",      // Default to Sec. 116
            // These categories don't typically map to percentage tax forms,
            // but we provide a sensible default rather than failing.
            IncomeCategory::Compensation => "PT010",
            IncomeCategory::BusinessVat => "PT010",
        }
    }

    /// Creates Schedule 1 rows from the payload's income sources.
    fn build_schedule_rows(sources: &[IncomeSource]) -> Result<Vec<Schedule1Row>, MapperError> {
        if sources.is_empty() {
            // Default to an empty PT010 row so the draft is valid
            return Ok(vec![Schedule1Row::default_pt010()]);
        }

        let mut rows = Vec::with_capacity(sources.len());

        for source in sources {
            let atc_code = Self::resolve_atc_code(source);
            let mut row = Schedule1Row::new(atc_code)
                .ok_or_else(|| MapperError::AtcNotFound(atc_code.to_string()))?;

            row.taxable_amount = source.gross_amount;

            // Apply tax rate override if provided
            if let Some(rate) = source.tax_rate_override {
                row.tax_rate = rate;
            }

            row.recompute();
            rows.push(row);
        }

        Ok(rows)
    }
}

impl FormMapper for Mapper2551Q {
    fn target_schema_id(&self) -> &'static str {
        "2551Qv2018"
    }

    fn target_form_code(&self) -> &'static str {
        "2551Q"
    }

    fn map(
        &self,
        payload: &UniversalTaxPayload,
        profile: &TaxpayerProfile,
    ) -> Result<FormDraftOutput, MapperError> {
        // Validate TIN match
        let profile_tin = profile.tin.full();
        if profile_tin != payload.tin {
            return Err(MapperError::TinMismatch {
                expected: profile_tin,
                actual: payload.tin.clone(),
            });
        }

        // Derive period
        let year = payload.taxable_year();
        let quarter = payload.quarter().ok_or(MapperError::InvalidPeriod)?;

        // Create draft from profile (pre-fills RDO, name, address, etc.)
        let mut draft = Form2551QDraft::new_from_profile(profile, year, quarter);

        // Overlay payload data
        draft.is_amended = payload.is_amended;
        draft.creditable_tax_withheld = payload.creditable_withholdings;

        if payload.is_amended {
            draft.tax_paid_previous = payload.previous_tax_paid;
        }

        // Build Schedule 1 from income sources
        draft.schedule_1 = Self::build_schedule_rows(&payload.income_sources)?;

        // Trigger full recomputation (tax due, penalties, totals)
        draft.recompute();

        Ok(FormDraftOutput::Form2551Q(draft))
    }
}

/// Resolves the appropriate mappers for a given payload and profile.
///
/// If `target_form` is specified, returns only that mapper.
/// Otherwise, auto-detects applicable forms based on income categories.
pub fn resolve_mappers(payload: &UniversalTaxPayload) -> Vec<Box<dyn FormMapper>> {
    let mut mappers: Vec<Box<dyn FormMapper>> = Vec::new();

    if let Some(ref target) = payload.target_form {
        if target.as_str() == "2551Q" {
            mappers.push(Box::new(Mapper2551Q));
            // Future: "1701Q" => mappers.push(Box::new(Mapper1701Q)),
        } // Unknown form — returns empty, caller handles the error
    } else {
        // Auto-detect: check if any income sources are percentage-tax eligible
        let has_percentage_tax_income = payload.income_sources.iter().any(|s| {
            matches!(
                s.category,
                IncomeCategory::BusinessNonVat
                    | IncomeCategory::ProfessionalServices
                    | IncomeCategory::PassiveIncome
                    | IncomeCategory::CapitalGains
            )
        });

        if has_percentage_tax_income {
            mappers.push(Box::new(Mapper2551Q));
        }

        // Future: check for income tax eligibility
        // let has_income_tax_income = payload.income_sources.iter().any(|s| {
        //     matches!(s.category, IncomeCategory::Compensation | IncomeCategory::ProfessionalServices)
        // });
        // if has_income_tax_income { mappers.push(Box::new(Mapper1701Q)); }
    }

    mappers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;
    use chrono::NaiveDate;

    fn test_profile() -> TaxpayerProfile {
        TaxpayerProfile {
            id: Some(1),
            full_name: "JUAN DELA CRUZ".to_string(),
            tin: Tin {
                segment1: "010".into(),
                segment2: "558".into(),
                segment3: "054".into(),
                branch: "000".into(),
            },
            rdo_code: "039".to_string(),
            line_of_business: "Consulting Services".to_string(),
            registered_address: "123 Rizal Street, Quezon City".to_string(),
            zip_code: "1100".to_string(),
            phone: "09156837000".to_string(),
            email: "juan@example.com".to_string(),
            default_form_type: "2551Q".to_string(),
            taxpayer_type: crate::profile::TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
            email_tracking_enabled: false,
            email_auth_method: crate::profile::EmailAuthMethod::AppPassword,
            imap_email: None,
            imap_host: None,
            _imap_enabled_compat: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            tax_classification: None,
            opted_for_8_percent_flat_rate: false,
        }
    }

    fn test_payload() -> UniversalTaxPayload {
        UniversalTaxPayload {
            tin: "010558054000".to_string(),
            target_form: Some("2551Q".to_string()),
            period_start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            period_end: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
            is_amended: false,
            income_sources: vec![IncomeSource {
                category: IncomeCategory::BusinessNonVat,
                gross_amount: 500_000.0,
                is_vat_exempt: true,
                atc_code_override: None,
                tax_rate_override: None,
            }],
            creditable_withholdings: 15_000.0,
            previous_tax_paid: 0.0,
            metadata: Default::default(),
        }
    }

    #[test]
    fn test_mapper_2551q_basic() {
        let mapper = Mapper2551Q;
        let profile = test_profile();
        let payload = test_payload();

        let result = mapper.map(&payload, &profile).unwrap();

        match result {
            FormDraftOutput::Form2551Q(draft) => {
                assert_eq!(draft.tin, "010558054000");
                assert_eq!(draft.taxable_year, 2026);
                assert_eq!(draft.quarter, 1);
                assert_eq!(draft.taxpayer_name, "JUAN DELA CRUZ");
                assert_eq!(draft.rdo_code, "039");
                assert!(!draft.is_amended);

                // Schedule 1 should have one PT010 row
                assert_eq!(draft.schedule_1.len(), 1);
                assert_eq!(draft.schedule_1[0].atc, "PT010");
                assert_eq!(draft.schedule_1[0].taxable_amount, 500_000.0);
                assert_eq!(draft.schedule_1[0].tax_rate, 0.03);
                assert_eq!(draft.schedule_1[0].tax_due, 15_000.0);

                // Creditable withholdings
                assert_eq!(draft.creditable_tax_withheld, 15_000.0);

                // Auto-computed totals
                assert_eq!(draft.total_tax_due, 15_000.0);
                // tax_payable = 15000 - 15000 = 0
                assert_eq!(draft.tax_payable, 0.0);
            }
        }
    }

    #[test]
    fn test_mapper_2551q_amended() {
        let mapper = Mapper2551Q;
        let profile = test_profile();
        let mut payload = test_payload();
        payload.is_amended = true;
        payload.previous_tax_paid = 5_000.0;

        let result = mapper.map(&payload, &profile).unwrap();

        match result {
            FormDraftOutput::Form2551Q(draft) => {
                assert!(draft.is_amended);
                assert_eq!(draft.tax_paid_previous, 5_000.0);
            }
        }
    }

    #[test]
    fn test_mapper_2551q_tin_mismatch() {
        let mapper = Mapper2551Q;
        let profile = test_profile();
        let mut payload = test_payload();
        payload.tin = "999999999000".to_string();

        let result = mapper.map(&payload, &profile);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MapperError::TinMismatch { .. }
        ));
    }

    #[test]
    fn test_mapper_2551q_multiple_sources() {
        let mapper = Mapper2551Q;
        let profile = test_profile();
        let mut payload = test_payload();
        payload.income_sources = vec![
            IncomeSource {
                category: IncomeCategory::BusinessNonVat,
                gross_amount: 300_000.0,
                is_vat_exempt: true,
                atc_code_override: None,
                tax_rate_override: None,
            },
            IncomeSource {
                category: IncomeCategory::ProfessionalServices,
                gross_amount: 200_000.0,
                is_vat_exempt: false,
                atc_code_override: None,
                tax_rate_override: None,
            },
        ];

        let result = mapper.map(&payload, &profile).unwrap();

        match result {
            FormDraftOutput::Form2551Q(draft) => {
                assert_eq!(draft.schedule_1.len(), 2);
                // Both should map to PT010 (3%)
                let total_tax: f64 = draft.schedule_1.iter().map(|r| r.tax_due).sum();
                assert_eq!(total_tax, 15_000.0); // (300k + 200k) * 3%
            }
        }
    }

    #[test]
    fn test_mapper_2551q_atc_override() {
        let mapper = Mapper2551Q;
        let profile = test_profile();
        let mut payload = test_payload();
        payload.income_sources = vec![IncomeSource {
            category: IncomeCategory::BusinessNonVat,
            gross_amount: 100_000.0,
            is_vat_exempt: true,
            atc_code_override: Some("PT070".to_string()), // Gas/water utility at 2%
            tax_rate_override: None,
        }];

        let result = mapper.map(&payload, &profile).unwrap();

        match result {
            FormDraftOutput::Form2551Q(draft) => {
                assert_eq!(draft.schedule_1[0].atc, "PT070");
                assert_eq!(draft.schedule_1[0].tax_rate, 0.02);
                assert_eq!(draft.schedule_1[0].tax_due, 2_000.0); // 100k * 2%
            }
        }
    }

    #[test]
    fn test_resolve_mappers_targeted() {
        let payload = test_payload();
        let mappers = resolve_mappers(&payload);
        assert_eq!(mappers.len(), 1);
        assert_eq!(mappers[0].target_form_code(), "2551Q");
    }

    #[test]
    fn test_resolve_mappers_auto_detect() {
        let mut payload = test_payload();
        payload.target_form = None; // Auto-detect

        let mappers = resolve_mappers(&payload);
        assert_eq!(mappers.len(), 1);
        assert_eq!(mappers[0].target_form_code(), "2551Q");
    }

    #[test]
    fn test_resolve_mappers_unknown_form() {
        let mut payload = test_payload();
        payload.target_form = Some("9999X".to_string());

        let mappers = resolve_mappers(&payload);
        assert!(mappers.is_empty());
    }
}
