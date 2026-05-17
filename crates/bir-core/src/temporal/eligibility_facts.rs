//! EligibilityFacts — a derived, engine-internal view of the taxpayer profile.
//!
//! The engine should never read raw UI labels or overloaded classification values
//! from the profile directly. Instead, it operates on these structured facts which
//! are derived deterministically from the profile's stored fields.
//!
//! This decouples the engine evaluation from `TaxClassification` (the UI-facing enum)
//! and produces clean boolean/enum signals that rules consume.

use crate::profile::{
    EoptTier, ExciseTaxCategory, IncomeTaxElection, RegistrationActivityStatus, TaxClassification,
    TaxpayerProfile, TaxpayerType,
};

/// What kind of income does this Individual earn?
///
/// Only meaningful for `TaxpayerType::Individual`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndividualIncomeKind {
    CompensationOnly,
    BusinessOrProfessionOnly,
    MixedIncome,
}

/// How is this Cooperative taxed?
///
/// Only meaningful for `TaxpayerType::Cooperative`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CooperativeTaxTreatment {
    Exempt,
    Taxable,
    Mixed,
}

/// Year-scoped income tax election.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearElection {
    pub taxable_year: u16,
    pub election: IncomeTaxElection,
}

/// Derived eligibility facts consumed by the temporal engine.
///
/// Built from `TaxpayerProfile` via `EligibilityFacts::from_profile()`.
/// The engine reads only this struct — never the raw profile labels.
#[derive(Debug, Clone)]
pub struct EligibilityFacts {
    // ── Entity Identity ──
    pub taxpayer_type: TaxpayerType,

    // ── Individual-specific ──
    pub individual_income_kind: Option<IndividualIncomeKind>,

    // ── Cooperative-specific ──
    pub cooperative_tax_treatment: Option<CooperativeTaxTreatment>,

    // ── Registration Obligations ──
    pub is_vat_registered: bool,

    // ── Business Activity ──
    /// True when the taxpayer has business/professional activity (not purely compensation).
    pub has_business_activity: bool,

    // ── Withholding (granular — FIND-009) ──
    /// Withholds compensation taxes from employee salaries.
    pub withholds_compensation: bool,
    /// Withholds expanded taxes from payments to contractors/suppliers.
    pub withholds_expanded: bool,
    /// Withholds final taxes on passive income (interest, dividends, etc).
    pub withholds_final: bool,
    /// Top withholding agent designated by BIR.
    pub is_top_withholding_agent: bool,
    /// Government entity required to withhold.
    pub is_government_withholding_entity: bool,

    // ── Legacy compat (old coarse flags, still read by some rules) ──
    pub has_employees: bool,
    pub is_expanded_withholding_agent: bool,

    // ── Excise ──
    pub excise_tax_categories: Vec<ExciseTaxCategory>,

    // ── Elections ──
    pub tax_elections: Vec<YearElection>,

    // ── Status ──
    pub is_dormant: bool,
    pub registration_activity_status: RegistrationActivityStatus,
    pub has_single_employer: bool,
    pub is_gpp_partner: bool,

    // ── EOPT ──
    pub eopt_tier: Option<EoptTier>,

    // ── Legacy compat: the effective_classification so rules that still
    //    reference TaxClassification keep working during migration. ──
    pub effective_classification: Option<TaxClassification>,
}

impl EligibilityFacts {
    /// Derive eligibility facts from a taxpayer profile.
    ///
    /// This is the single canonical bridge between the UI/persistence layer
    /// and the temporal engine's evaluation logic.
    pub fn from_profile(profile: &TaxpayerProfile) -> Self {
        let effective_classification = profile.effective_classification();

        let individual_income_kind = match profile.taxpayer_type {
            TaxpayerType::Individual => match &effective_classification {
                Some(TaxClassification::PurelyCompensation) => {
                    Some(IndividualIncomeKind::CompensationOnly)
                }
                Some(TaxClassification::SelfEmployed) => {
                    Some(IndividualIncomeKind::BusinessOrProfessionOnly)
                }
                Some(TaxClassification::MixedIncome) => Some(IndividualIncomeKind::MixedIncome),
                _ => None, // Not yet configured
            },
            _ => None,
        };

        let cooperative_tax_treatment = match profile.taxpayer_type {
            TaxpayerType::Cooperative => match &effective_classification {
                Some(TaxClassification::CooperativeExempt) => Some(CooperativeTaxTreatment::Exempt),
                Some(TaxClassification::CooperativeTaxable) => {
                    Some(CooperativeTaxTreatment::Taxable)
                }
                Some(TaxClassification::CooperativeMixed) => Some(CooperativeTaxTreatment::Mixed),
                _ => Some(CooperativeTaxTreatment::Taxable), // safe default
            },
            _ => None,
        };

        let has_business_activity = matches!(
            individual_income_kind,
            Some(IndividualIncomeKind::BusinessOrProfessionOnly)
                | Some(IndividualIncomeKind::MixedIncome)
        ) || matches!(
            profile.taxpayer_type,
            TaxpayerType::Corporation | TaxpayerType::Partnership | TaxpayerType::Cooperative
        );

        let tax_elections = profile
            .tax_elections
            .iter()
            .map(|h| YearElection {
                taxable_year: h.taxable_year,
                election: h.election.clone(),
            })
            .collect();

        // Granular withholding: prefer the new flags, but auto-derive from old
        // flags for backward compat (profiles that only have has_employees set).
        let withholds_compensation = profile.withholds_compensation || profile.has_employees;
        let withholds_expanded = profile.withholds_expanded
            || profile.is_expanded_withholding_agent
            || profile.is_top_withholding_agent
            || profile.is_government_withholding_entity;
        let withholds_final = profile.withholds_final;
        let is_dormant = profile.is_dormant
            || matches!(
                profile.registration_activity_status,
                RegistrationActivityStatus::DormantOperational
                    | RegistrationActivityStatus::TemporarilyInactive
            );

        Self {
            taxpayer_type: profile.taxpayer_type.clone(),
            individual_income_kind,
            cooperative_tax_treatment,
            is_vat_registered: profile.is_vat_registered,
            has_business_activity,
            withholds_compensation,
            withholds_expanded,
            withholds_final,
            is_top_withholding_agent: profile.is_top_withholding_agent,
            is_government_withholding_entity: profile.is_government_withholding_entity,
            has_employees: profile.has_employees,
            is_expanded_withholding_agent: profile.is_expanded_withholding_agent,
            excise_tax_categories: profile.excise_tax_categories.clone(),
            tax_elections,
            is_dormant,
            registration_activity_status: profile.registration_activity_status.clone(),
            has_single_employer: profile.has_single_employer,
            is_gpp_partner: profile.is_gpp_partner,
            eopt_tier: profile.eopt_tier.clone(),
            effective_classification,
        }
    }

    /// Returns true if the 8% flat rate election is active for the given taxable year.
    pub fn has_8_percent_election(&self, year: u16) -> bool {
        self.tax_elections.iter().any(|e| {
            e.taxable_year == year && matches!(e.election, IncomeTaxElection::EightPercent)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;
    use crate::profile::TaxElectionHistory;

    fn test_tin() -> Tin {
        Tin {
            segment1: "010".into(),
            segment2: "558".into(),
            segment3: "054".into(),
            branch: "000".into(),
        }
    }

    fn minimal_profile(tp: TaxpayerType, tc: Option<TaxClassification>) -> TaxpayerProfile {
        TaxpayerProfile {
            id: Some(1),
            full_name: "Test".into(),
            tin: test_tin(),
            rdo_code: "039".into(),
            line_of_business: "Test".into(),
            registered_address: "QC".into(),
            zip_code: "1100".into(),
            phone: "09156837000".into(),
            email: "test@example.com".into(),
            default_form_type: "2551Q".into(),
            taxpayer_type: tp,
            is_vat_registered: false,
            business_start_date: None,
            tax_classification: tc,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: false,
            is_dormant: false,
            registration_activity_status: Default::default(),
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_versions: vec![],
            compliance_source_mode: Default::default(),
        }
    }

    #[test]
    fn test_individual_compensation_facts() {
        let profile = minimal_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::PurelyCompensation),
        );
        let facts = EligibilityFacts::from_profile(&profile);
        assert_eq!(
            facts.individual_income_kind,
            Some(IndividualIncomeKind::CompensationOnly)
        );
        assert!(!facts.has_business_activity);
        assert!(facts.cooperative_tax_treatment.is_none());
    }

    #[test]
    fn test_individual_self_employed_facts() {
        let profile = minimal_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::SelfEmployed),
        );
        let facts = EligibilityFacts::from_profile(&profile);
        assert_eq!(
            facts.individual_income_kind,
            Some(IndividualIncomeKind::BusinessOrProfessionOnly)
        );
        assert!(facts.has_business_activity);
    }

    #[test]
    fn test_individual_mixed_income_facts() {
        let profile = minimal_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::MixedIncome),
        );
        let facts = EligibilityFacts::from_profile(&profile);
        assert_eq!(
            facts.individual_income_kind,
            Some(IndividualIncomeKind::MixedIncome)
        );
        assert!(facts.has_business_activity);
    }

    #[test]
    fn test_corporation_facts() {
        let profile = minimal_profile(TaxpayerType::Corporation, None);
        let facts = EligibilityFacts::from_profile(&profile);
        assert!(facts.individual_income_kind.is_none());
        assert!(facts.has_business_activity);
        assert_eq!(
            facts.effective_classification,
            Some(TaxClassification::Corporation)
        );
    }

    #[test]
    fn test_cooperative_exempt_facts() {
        let profile = minimal_profile(
            TaxpayerType::Cooperative,
            Some(TaxClassification::CooperativeExempt),
        );
        let facts = EligibilityFacts::from_profile(&profile);
        assert_eq!(
            facts.cooperative_tax_treatment,
            Some(CooperativeTaxTreatment::Exempt)
        );
        assert!(facts.has_business_activity);
    }

    #[test]
    fn test_cooperative_defaults_to_taxable() {
        let profile = minimal_profile(TaxpayerType::Cooperative, None);
        let facts = EligibilityFacts::from_profile(&profile);
        assert_eq!(
            facts.cooperative_tax_treatment,
            Some(CooperativeTaxTreatment::Taxable)
        );
    }

    #[test]
    fn test_estate_trust_facts() {
        let profile = minimal_profile(TaxpayerType::Estate, None);
        let facts = EligibilityFacts::from_profile(&profile);
        assert!(facts.individual_income_kind.is_none());
        assert!(!facts.has_business_activity);
        assert_eq!(
            facts.effective_classification,
            Some(TaxClassification::EstateOrTrust)
        );
    }

    #[test]
    fn test_8_percent_election_lookup() {
        let mut profile = minimal_profile(
            TaxpayerType::Individual,
            Some(TaxClassification::SelfEmployed),
        );
        profile.tax_elections.push(TaxElectionHistory {
            taxable_year: 2024,
            election: IncomeTaxElection::EightPercent,
            elected_at: chrono::NaiveDateTime::default(),
            source_form: "test".to_string(),
        });
        let facts = EligibilityFacts::from_profile(&profile);
        assert!(facts.has_8_percent_election(2024));
        assert!(!facts.has_8_percent_election(2023));
    }
}
