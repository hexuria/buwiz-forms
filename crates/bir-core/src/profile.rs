//! Taxpayer profile management.

use crate::naming::Tin;
use chrono::{Duration, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TaxpayerType {
    #[default]
    Individual,
    Corporation,
    Partnership,
    Cooperative,
    Estate,
    Trust,
}

/// Refined tax classification that drives filing behavior.
///
/// This is a refinement of `TaxpayerType` — it specifies *how* the taxpayer
/// files (which forms are required, which ATC/tax rules apply), whereas
/// `TaxpayerType` specifies *what kind* of entity they are.
///
/// For Individual taxpayers, the user picks one of:
///   - `PurelyCompensation` (salary only)
///   - `SelfEmployed` (freelancer, professional, sole proprietor)
///   - `MixedIncome` (both salary AND business)
///
/// For non-Individual types, the classification is auto-derived from the
/// `TaxpayerType` (see `TaxpayerProfile::effective_classification()`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaxClassification {
    /// Individual with only employment income — files 1700/1701 only.
    PurelyCompensation,
    /// Self-employed professional, freelancer, or sole proprietor.
    /// VAT routing is handled separately by `is_vat_registered`.
    /// 8% election is handled by `TrainLaw8PercentRule`.
    #[serde(alias = "ProfessionalOrFreelancer")]
    #[serde(alias = "SoleProprietorNonVat")]
    #[serde(alias = "SoleProprietorVat")]
    SelfEmployed,
    /// Individual with both compensation and business/professional income.
    MixedIncome,
    /// Corporation or Partnership — files 1702Q, 1702RT.
    Corporation,
    /// Tax-exempt cooperative — files 1702-EX.
    CooperativeExempt,
    /// Taxable cooperative — files 1702-RT.
    CooperativeTaxable,
    /// Mixed-income cooperative — files 1702-MX.
    CooperativeMixed,
    /// Estate or Trust — files 1701/1701Q (same as Individual).
    EstateOrTrust,
}

/// How the app authenticates to the user's mail server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum EmailAuthMethod {
    /// Standard IMAP LOGIN with an App Password stored in the OS Keychain.
    #[default]
    AppPassword,
    /// Google OAuth2 PKCE flow — tokens stored in the OS Keychain.
    GoogleOAuth,
}

/// Ease of Paying Taxes (EOPT) Act Taxpayer Classification Tiers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EoptTier {
    Micro,
    Small,
    Medium,
    Large,
}

/// Optional Income Tax Elections made by the taxpayer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IncomeTaxElection {
    GraduatedOsd,
    GraduatedItemized,
    EightPercent,
}

/// Categories of Excise Taxes a taxpayer might be liable for.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExciseTaxCategory {
    Alcohol,
    AutomobilesAndNonEssential,
    Mineral,
    Petroleum,
    Tobacco,
    SweetenedBeverages,
    CoalAndCoke,
}

/// Registration and operational activity status.
///
/// Separates dormant from temporarily inactive from officially closed
/// per FIND-011. Only open registration and tax-type obligations should
/// generate NIL filings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RegistrationActivityStatus {
    /// Normal active taxpayer.
    #[default]
    Active,
    /// Dormant but operationally registered — may still need NIL filings.
    DormantOperational,
    /// Temporarily inactive — suspended operations.
    TemporarilyInactive,
    /// Officially closed with BIR — no further filing obligations.
    OfficiallyClosed,
}

/// A ledger of historical tax regime elections made by the taxpayer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxElectionHistory {
    pub taxable_year: u16,
    pub election: IncomeTaxElection,
    pub elected_at: chrono::NaiveDateTime,
    pub source_form: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaxProfileVersionStatus {
    Draft,
    Confirmed,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ComplianceSourceMode {
    #[default]
    TemporalSuggestion,
    CorVersioned,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaxProfileVersionSource {
    ManualCor,
    OcrCor,
    UserOverride,
    MigrationBackfill,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RegisteredTaxType {
    IncomeTax,
    ValueAddedTax,
    PercentageTax,
    RegistrationFee,
    WithholdingExpanded,
    WithholdingCompensation,
    WithholdingFinal,
    ExciseTax,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorRegistrationFacts {
    #[serde(default)]
    pub tin: Option<String>,
    #[serde(default)]
    pub registration_date: Option<NaiveDate>,
    #[serde(default)]
    pub registered_name: String,
    #[serde(default)]
    pub trade_name: Option<String>,
    #[serde(default)]
    pub registered_address: String,
    #[serde(default)]
    pub rdo_code: String,
    #[serde(default)]
    pub line_of_business_code: Option<String>,
    #[serde(default)]
    pub line_of_business_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorDocumentRef {
    pub id: String,
    pub file_name: String,
    pub stored_path: String,
    #[serde(default)]
    pub uploaded_at: Option<NaiveDateTime>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub document_type: Option<String>,
    #[serde(default)]
    pub extracted_form_codes: Vec<String>,
    #[serde(default)]
    pub ocr_text: Option<String>,
    #[serde(default)]
    pub ocr_confidence: Option<f32>,
    #[serde(default)]
    pub field_bboxes: std::collections::HashMap<String, [u16; 4]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ManualObligationOverrideAction {
    Include,
    Exclude,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualObligationOverride {
    pub form_code: String,
    pub action: ManualObligationOverrideAction,
    pub reason: String,
    #[serde(default)]
    pub source_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDeadlineOverride {
    pub id: String,
    pub title: String,
    pub source_reference: String,
    pub affected_form_codes: Vec<String>,
    pub original_deadline: NaiveDate,
    pub adjusted_deadline: NaiveDate,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Effective-dated COR/manual profile configuration.
///
/// The flat `TaxpayerProfile` fields are kept for compatibility and form
/// prefills. Dashboard compliance resolves through confirmed versions first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxProfileVersion {
    pub id: String,
    pub label: String,
    pub status: TaxProfileVersionStatus,
    pub source: TaxProfileVersionSource,
    #[serde(default)]
    pub effective_from: Option<NaiveDate>,
    #[serde(default)]
    pub effective_until: Option<NaiveDate>,
    #[serde(default)]
    pub needs_effective_date_review: bool,
    pub cor: CorRegistrationFacts,
    #[serde(default)]
    pub registered_tax_types: Vec<RegisteredTaxType>,
    pub taxpayer_type: TaxpayerType,
    #[serde(default)]
    pub tax_classification: Option<TaxClassification>,
    #[serde(default)]
    pub eopt_tier: Option<EoptTier>,
    #[serde(default)]
    pub is_vat_registered: bool,
    #[serde(default)]
    pub is_gpp_partner: bool,
    #[serde(default)]
    pub withholds_compensation: bool,
    #[serde(default)]
    pub withholds_expanded: bool,
    #[serde(default)]
    pub withholds_final: bool,
    #[serde(default)]
    pub is_top_withholding_agent: bool,
    #[serde(default)]
    pub is_government_withholding_entity: bool,
    #[serde(default)]
    pub excise_tax_categories: Vec<ExciseTaxCategory>,
    #[serde(default)]
    pub registration_activity_status: RegistrationActivityStatus,
    #[serde(default)]
    pub evidence: Vec<CorDocumentRef>,
    #[serde(default)]
    pub obligation_overrides: Vec<ManualObligationOverride>,
    #[serde(default)]
    pub deadline_overrides: Vec<ProfileDeadlineOverride>,
}

/// Taxpayer profile stored in encrypted SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxpayerProfile {
    pub id: Option<i64>,
    pub full_name: String,
    pub tin: Tin,
    pub rdo_code: String,
    pub line_of_business: String,
    pub registered_address: String,
    pub zip_code: String,
    pub phone: String,
    pub email: String,
    pub default_form_type: String,
    #[serde(default)]
    pub taxpayer_type: TaxpayerType,
    #[serde(default)]
    pub is_vat_registered: bool,
    #[serde(default)]
    pub business_start_date: Option<NaiveDate>,
    #[serde(default)]
    pub birth_date: Option<NaiveDate>,

    /// Refined classification that drives form applicability and ATC rules.
    /// Optional — existing profiles default to None until the user configures it.
    #[serde(default)]
    pub tax_classification: Option<TaxClassification>,

    /// Ease of Paying Taxes (EOPT) Act Tier (Micro, Small, Medium, Large)
    #[serde(default)]
    pub eopt_tier: Option<EoptTier>,

    /// Special Entity Flags
    #[serde(default)]
    pub is_bmbe: bool,
    #[serde(default)]
    pub is_gpp_partner: bool,
    #[serde(default)]
    pub is_create_msme: bool,
    #[serde(default)]
    pub is_expanded_withholding_agent: bool,

    /// Array of Alphanumeric Tax Codes mapping to the taxpayer's business activities.
    #[serde(default)]
    pub atc_codes: Vec<String>,

    /// Excise tax categories the taxpayer is liable for.
    #[serde(default)]
    pub excise_tax_categories: Vec<ExciseTaxCategory>,

    /// Historical ledger of tax regime elections (OSD vs Itemized vs 8%).
    #[serde(default)]
    pub tax_elections: Vec<TaxElectionHistory>,

    /// Soft delete flag. If true, the profile is archived and can be exported/hard-deleted.
    #[serde(default)]
    pub is_archived: bool,

    /// Profile specific 4-digit PIN hash
    #[serde(default)]
    pub profile_pin_hash: Option<String>,

    /// Profile specific TOTP secret for Authenticator apps
    #[serde(default)]
    pub totp_secret: Option<String>,

    // Email Tracking Settings
    /// Master toggle — whether automatic BIR receipt checking is enabled.
    #[serde(default)]
    pub email_tracking_enabled: bool,
    /// Which authentication method to use (App Password or Google OAuth2).
    #[serde(default)]
    pub email_auth_method: EmailAuthMethod,
    /// Email address used for IMAP login (defaults to profile email if None).
    #[serde(default)]
    pub imap_email: Option<String>,
    /// IMAP server hostname (only needed for App Password mode; defaults to imap.gmail.com).
    #[serde(default)]
    pub imap_host: Option<String>,

    /// Toggle to enable/disable test OS notifications every minute.
    #[serde(default)]
    pub test_notification_enabled: bool,

    /// Securely stored App Password (encrypted inside DB)
    #[serde(default)]
    pub imap_app_password: Option<String>,

    /// Securely stored OAuth Access Token (encrypted inside DB)
    #[serde(default)]
    pub oauth_access_token: Option<String>,

    /// Securely stored OAuth Refresh Token (encrypted inside DB)
    #[serde(default)]
    pub oauth_refresh_token: Option<String>,

    /// Whether the taxpayer has employees (determines withholding form applicability:
    /// 1601C, 1601E, 1601F, 1602, 1603, 1604CF, 1604E).
    #[serde(default)]
    pub has_employees: bool,

    /// Whether the taxpayer is dormant/no operations (triggers NIL filing for all required forms)
    #[serde(default)]
    pub is_dormant: bool,

    /// Whether a PurelyCompensation earner has exactly one employer (triggers Substituted Filing)
    #[serde(default)]
    pub has_single_employer: bool,

    // ── Granular Withholding Triggers (FIND-009) ──
    /// Withholds compensation taxes from employee salaries.
    #[serde(default)]
    pub withholds_compensation: bool,

    /// Withholds expanded taxes from payments to contractors/suppliers.
    #[serde(default)]
    pub withholds_expanded: bool,

    /// Withholds final taxes on passive income (interest, dividends, etc).
    #[serde(default)]
    pub withholds_final: bool,

    /// Top withholding agent designated by BIR.
    #[serde(default)]
    pub is_top_withholding_agent: bool,

    /// Government entity required to withhold.
    #[serde(default)]
    pub is_government_withholding_entity: bool,

    // ── Registration Activity Status (FIND-011) ──
    /// The taxpayer's current registration/operational status.
    #[serde(default)]
    pub registration_activity_status: RegistrationActivityStatus,

    /// Effective-dated COR/manual profile configuration ledger.
    #[serde(default)]
    pub profile_versions: Vec<TaxProfileVersion>,

    /// Selects whether compliance uses the flat profile/TTCE projection or the
    /// confirmed COR/manual version ledger.
    #[serde(default)]
    pub compliance_source_mode: ComplianceSourceMode,
}

impl TaxpayerProfile {
    /// Returns the effective TaxClassification for the rule engine.
    ///
    /// For Individual taxpayers, this returns the user-selected classification.
    /// For non-Individual types, it auto-derives from the TaxpayerType.
    pub fn effective_classification(&self) -> Option<TaxClassification> {
        match self.taxpayer_type {
            TaxpayerType::Individual => self.tax_classification.clone(),
            TaxpayerType::Corporation | TaxpayerType::Partnership => {
                Some(TaxClassification::Corporation)
            }
            TaxpayerType::Cooperative => {
                // Use user-specified coop sub-type, or default to Taxable
                match self.tax_classification {
                    Some(ref c)
                        if matches!(
                            c,
                            TaxClassification::CooperativeExempt
                                | TaxClassification::CooperativeTaxable
                                | TaxClassification::CooperativeMixed
                        ) =>
                    {
                        self.tax_classification.clone()
                    }
                    _ => Some(TaxClassification::CooperativeTaxable),
                }
            }
            TaxpayerType::Estate | TaxpayerType::Trust => Some(TaxClassification::EstateOrTrust),
        }
    }

    /// Returns true if the 8% flat rate election is active for the given taxable year.
    /// Checks the historical `tax_elections` ledger (populated by migration v4 for old profiles).
    pub fn has_8_percent_election(&self, year: u16) -> bool {
        self.tax_elections.iter().any(|h| {
            h.taxable_year == year && matches!(h.election, IncomeTaxElection::EightPercent)
        })
    }

    /// Returns true if email tracking is active.
    pub fn is_email_tracking_active(&self) -> bool {
        self.email_tracking_enabled
    }

    /// Returns BIR form codes applicable to this taxpayer based on their
    /// classification, VAT status, and employee status.
    ///
    /// Uses the current year. Prefer `applicable_forms_for_year(year)` when
    /// the target year is known.
    pub fn applicable_forms(&self) -> Vec<String> {
        crate::integration::applicable_forms_for_profile(self)
    }

    /// Returns BIR form codes applicable to this taxpayer for a specific year.
    pub fn applicable_forms_for_year(&self, year: u16) -> Vec<String> {
        crate::integration::applicable_forms_for_profile_and_year(self, year)
    }

    pub fn ensure_profile_version_ledger(&mut self) {
        if self.compliance_source_mode == ComplianceSourceMode::CorVersioned
            && self.profile_versions.is_empty()
        {
            self.profile_versions
                .push(TaxProfileVersion::from_profile_backfill(self));
        }
    }

    pub fn compliance_mode(&self) -> ComplianceSourceMode {
        self.compliance_source_mode.clone()
    }

    pub fn confirmed_profile_versions(&self) -> Vec<TaxProfileVersion> {
        if self.compliance_source_mode == ComplianceSourceMode::TemporalSuggestion {
            return vec![TaxProfileVersion::from_profile_backfill(self)];
        }

        let mut versions: Vec<_> = self
            .profile_versions
            .iter()
            .filter(|version| version.status == TaxProfileVersionStatus::Confirmed)
            .cloned()
            .collect();

        versions.sort_by(|a, b| {
            a.effective_from
                .cmp(&b.effective_from)
                .then(a.id.cmp(&b.id))
        });
        versions
    }

    pub fn active_profile_versions_for_period(
        &self,
        period_start: NaiveDate,
        period_end: NaiveDate,
    ) -> Vec<TaxProfileVersion> {
        self.confirmed_profile_versions()
            .into_iter()
            .filter(|version| version.overlaps_period(period_start, period_end))
            .collect()
    }

    pub fn active_profile_versions_for_year(&self, year: u16) -> Vec<TaxProfileVersion> {
        let start = NaiveDate::from_ymd_opt(year as i32, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(year as i32, 12, 31).unwrap();
        self.active_profile_versions_for_period(start, end)
    }

    pub fn current_cor_version(&self, as_of_year: u16) -> Option<TaxProfileVersion> {
        if self.compliance_source_mode != ComplianceSourceMode::CorVersioned {
            return None;
        }

        self.active_profile_versions_for_year(as_of_year)
            .into_iter()
            .rfind(|version| version.source != TaxProfileVersionSource::MigrationBackfill)
    }

    pub fn preview_obligations_for_year(
        &self,
        year: u16,
    ) -> crate::integration::ResolvedProfileObligations {
        crate::integration::resolve_profile_obligations_for_year(self, year)
    }

    pub fn auto_close_previous_confirmed_version(&mut self, new_effective_from: NaiveDate) {
        let close_at = new_effective_from - Duration::days(1);
        for version in &mut self.profile_versions {
            if version.status == TaxProfileVersionStatus::Confirmed
                && version.effective_until.is_none()
                && (version.source == TaxProfileVersionSource::MigrationBackfill
                    || version
                        .effective_from
                        .is_none_or(|start| start < new_effective_from))
            {
                version.effective_until = Some(close_at);
            }
        }
    }

    pub fn set_profile_version_confirmed(
        &mut self,
        version_id: &str,
        effective_from: NaiveDate,
    ) -> bool {
        self.auto_close_previous_confirmed_version(effective_from);
        if let Some(version) = self
            .profile_versions
            .iter_mut()
            .find(|version| version.id == version_id)
        {
            version.status = TaxProfileVersionStatus::Confirmed;
            version.effective_from = Some(effective_from);
            version.effective_until = None;
            version.needs_effective_date_review = false;
            self.compliance_source_mode = ComplianceSourceMode::CorVersioned;
            true
        } else {
            false
        }
    }

    pub fn projection_for_version(&self, version: &TaxProfileVersion) -> TaxpayerProfile {
        let mut projected = self.clone();
        projected.full_name = version.cor.registered_name.clone();
        projected.rdo_code = version.cor.rdo_code.clone();
        projected.line_of_business = version.cor.line_of_business_description.clone();
        projected.registered_address = version.cor.registered_address.clone();
        projected.business_start_date = version.cor.registration_date;
        projected.taxpayer_type = version.taxpayer_type.clone();
        projected.tax_classification = version.tax_classification.clone();
        projected.eopt_tier = version.eopt_tier.clone();
        projected.is_vat_registered = version.is_vat_registered;
        projected.is_gpp_partner = version.is_gpp_partner;
        projected.withholds_compensation = version.withholds_compensation;
        projected.has_employees = version.withholds_compensation;
        projected.withholds_expanded = version.withholds_expanded;
        projected.is_expanded_withholding_agent = version.withholds_expanded
            || version.is_top_withholding_agent
            || version.is_government_withholding_entity;
        projected.withholds_final = version.withholds_final;
        projected.is_top_withholding_agent = version.is_top_withholding_agent;
        projected.is_government_withholding_entity = version.is_government_withholding_entity;
        projected.excise_tax_categories = version.excise_tax_categories.clone();
        projected.registration_activity_status = version.registration_activity_status.clone();
        projected.profile_versions = Vec::new();
        projected.compliance_source_mode = ComplianceSourceMode::TemporalSuggestion;
        projected
    }

    pub fn inferred_registered_tax_types(&self) -> Vec<RegisteredTaxType> {
        let mut tax_types = Vec::new();
        tax_types.push(RegisteredTaxType::IncomeTax);

        let has_business_activity = !matches!(
            self.effective_classification(),
            Some(TaxClassification::PurelyCompensation)
        );

        if has_business_activity {
            if self.is_vat_registered {
                tax_types.push(RegisteredTaxType::ValueAddedTax);
            } else {
                tax_types.push(RegisteredTaxType::PercentageTax);
            }
            tax_types.push(RegisteredTaxType::RegistrationFee);
        }

        if self.withholds_compensation || self.has_employees {
            tax_types.push(RegisteredTaxType::WithholdingCompensation);
        }
        if self.withholds_expanded
            || self.is_expanded_withholding_agent
            || self.is_top_withholding_agent
            || self.is_government_withholding_entity
        {
            tax_types.push(RegisteredTaxType::WithholdingExpanded);
        }
        if self.withholds_final {
            tax_types.push(RegisteredTaxType::WithholdingFinal);
        }
        if !self.excise_tax_categories.is_empty() {
            tax_types.push(RegisteredTaxType::ExciseTax);
        }

        tax_types.sort();
        tax_types.dedup();
        tax_types
    }
}

impl TaxProfileVersion {
    pub fn from_profile_backfill(profile: &TaxpayerProfile) -> Self {
        let effective_from = profile.business_start_date;
        Self {
            id: "legacy-current-profile".to_string(),
            label: "Current profile".to_string(),
            status: TaxProfileVersionStatus::Confirmed,
            source: TaxProfileVersionSource::MigrationBackfill,
            effective_from,
            effective_until: None,
            needs_effective_date_review: effective_from.is_none(),
            cor: CorRegistrationFacts {
                tin: Some(profile.tin.formatted()),
                registration_date: effective_from,
                registered_name: profile.full_name.clone(),
                trade_name: None,
                registered_address: profile.registered_address.clone(),
                rdo_code: profile.rdo_code.clone(),
                line_of_business_code: None,
                line_of_business_description: profile.line_of_business.clone(),
            },
            registered_tax_types: profile.inferred_registered_tax_types(),
            taxpayer_type: profile.taxpayer_type.clone(),
            tax_classification: profile.tax_classification.clone(),
            eopt_tier: profile.eopt_tier.clone(),
            is_vat_registered: profile.is_vat_registered,
            is_gpp_partner: profile.is_gpp_partner,
            withholds_compensation: profile.withholds_compensation || profile.has_employees,
            withholds_expanded: profile.withholds_expanded
                || profile.is_expanded_withholding_agent
                || profile.is_top_withholding_agent
                || profile.is_government_withholding_entity,
            withholds_final: profile.withholds_final,
            is_top_withholding_agent: profile.is_top_withholding_agent,
            is_government_withholding_entity: profile.is_government_withholding_entity,
            excise_tax_categories: profile.excise_tax_categories.clone(),
            registration_activity_status: profile.registration_activity_status.clone(),
            evidence: Vec::new(),
            obligation_overrides: Vec::new(),
            deadline_overrides: Vec::new(),
        }
    }

    pub fn overlaps_period(&self, period_start: NaiveDate, period_end: NaiveDate) -> bool {
        if self.status != TaxProfileVersionStatus::Confirmed {
            return false;
        }

        let starts_before_period_ends = self
            .effective_from
            .is_none_or(|effective_from| effective_from <= period_end);
        let ends_after_period_starts = self
            .effective_until
            .is_none_or(|effective_until| effective_until >= period_start);

        starts_before_period_ends && ends_after_period_starts
    }
}

impl Drop for TaxpayerProfile {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        if let Some(ref mut pw) = self.imap_app_password {
            pw.zeroize();
        }
        if let Some(ref mut t) = self.oauth_access_token {
            t.zeroize();
        }
        if let Some(ref mut t) = self.oauth_refresh_token {
            t.zeroize();
        }
        if let Some(ref mut h) = self.profile_pin_hash {
            h.zeroize();
        }
    }
}
