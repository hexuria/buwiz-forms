//! Taxpayer profile management.

use crate::naming::Tin;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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
    /// The taxpayer chose the graduated-rate Item 13 option on 2551Q, but that
    /// form does not identify whether OSD or itemized deductions will be used.
    /// Keep the legally meaningful election without inventing a deduction
    /// method; a later income-tax return may refine it.
    GraduatedUnspecified,
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
    TemporalSuggestion,
    #[default]
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
    /// VAT and other percentage taxes withheld and remitted on Form 1600.
    /// This is distinct from withholding on employee compensation.
    WithholdingVatAndPercentage,
    ExciseTax,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VatRegistrationTextClassification {
    VatRegistered,
    NonVat,
    Unknown,
}

/// Classifies explicit VAT-registration language without treating the `VAT`
/// token inside `NON-VAT` as positive evidence.
pub fn classify_vat_registration_text(text: &str) -> VatRegistrationTextClassification {
    let normalized = text
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let compact = normalized.replace(' ', "");
    let is_explicitly_non_vat = normalized.contains("NON VAT")
        || compact.contains("NONVAT")
        || normalized.contains("NOT VAT REGISTERED")
        || normalized.contains("NOT REGISTERED FOR VAT");
    if is_explicitly_non_vat {
        return VatRegistrationTextClassification::NonVat;
    }

    if normalized.contains("VAT REGISTERED")
        || normalized.contains("REGISTERED FOR VAT")
        || normalized.contains("VALUE ADDED TAX REGISTERED")
    {
        VatRegistrationTextClassification::VatRegistered
    } else {
        VatRegistrationTextClassification::Unknown
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaxProfileResolutionIssueKind {
    UndatedConfirmedVersion,
    InvalidEffectiveRange,
    OverlappingConfirmedVersions,
    NoEffectiveVersionForPeriod,
    AmbiguousEffectiveVersionsForPeriod,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfileResolutionIssue {
    pub kind: TaxProfileResolutionIssueKind,
    pub version_ids: Vec<String>,
    pub message: String,
}

/// Confirmed, unambiguous profile segments that apply to one taxable year.
///
/// Confirmed versions without an effective start date and overlapping
/// timelines are reported and excluded instead of being guessed into the
/// year.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedTaxProfileForYear {
    pub taxable_year: u16,
    pub effective_segments: Vec<TaxProfileVersion>,
    pub issues: Vec<TaxProfileResolutionIssue>,
}

impl ResolvedTaxProfileForYear {
    pub fn has_blocking_issues(&self) -> bool {
        !self.issues.is_empty()
    }
}

/// One confirmed profile segment resolved for an exact filing period.
///
/// Forms must consume this result instead of the compatibility fields on
/// [`TaxpayerProfile`]. A period with no complete segment, multiple segments,
/// or any unresolved timeline issue deliberately has no effective segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedTaxProfileForPeriod {
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub effective_segment: Option<TaxProfileVersion>,
    pub issues: Vec<TaxProfileResolutionIssue>,
}

impl ResolvedTaxProfileForPeriod {
    pub fn has_blocking_issues(&self) -> bool {
        self.effective_segment.is_none() || !self.issues.is_empty()
    }
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

    /// Per-year Forms Set. SKIP serialization since it is stored in a separate DB table.
    #[serde(skip, default)]
    pub per_year_forms: std::collections::BTreeMap<u16, crate::forms::PerYearFormsSet>,
}

impl TaxpayerProfile {
    pub fn forms_set_for_year(&self, year: u16) -> Option<&crate::forms::PerYearFormsSet> {
        self.per_year_forms.get(&year)
    }

    /// Returns the closest earlier year with at least one active form, but only
    /// when the destination year has not been configured yet.
    pub fn closest_prior_forms_year(&self, year: u16) -> Option<u16> {
        if self
            .per_year_forms
            .get(&year)
            .is_some_and(|set| !set.entries.is_empty())
        {
            return None;
        }

        self.per_year_forms
            .range(..year)
            .rev()
            .find_map(|(prior_year, set)| {
                set.entries
                    .iter()
                    .any(crate::forms::FormSetEntry::is_filing_active)
                    .then_some(*prior_year)
            })
    }

    /// Returns active codes from the stored Forms Set only.
    ///
    /// An unconfigured year fails closed with an empty list; callers that need
    /// suggestions must explicitly reconcile and persist a Forms Set first.
    pub fn active_form_codes_for_year(&self, year: u16) -> Vec<String> {
        use std::collections::BTreeSet;

        self.per_year_forms
            .get(&year)
            .into_iter()
            .flat_map(|set| set.entries.iter())
            .filter(|entry| entry.is_filing_active())
            .map(|entry| crate::forms::registry::canonical_form_code(&entry.form_code))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

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
                        Some(c.clone())
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

    /// Returns BIR form codes applicable to this taxpayer.
    ///
    /// This resolves the taxpayer's dynamic forms list based on tax
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
        if self.profile_versions.is_empty() {
            self.profile_versions
                .push(TaxProfileVersion::from_profile_backfill(self));
        }
    }

    pub(crate) fn validate_confirmed_profile_timeline(&mut self) -> Result<(), String> {
        let has_dated_confirmed_version = self.profile_versions.iter().any(|version| {
            version.status == TaxProfileVersionStatus::Confirmed
                && version.source != TaxProfileVersionSource::MigrationBackfill
                && version.effective_from.is_some()
        });
        if has_dated_confirmed_version {
            for version in &mut self.profile_versions {
                if version.status == TaxProfileVersionStatus::Confirmed
                    && version.source == TaxProfileVersionSource::MigrationBackfill
                    && version.effective_from.is_none()
                {
                    version.status = TaxProfileVersionStatus::Archived;
                }
            }
        }

        let mut confirmed = self
            .profile_versions
            .iter()
            .filter(|version| version.status == TaxProfileVersionStatus::Confirmed)
            .collect::<Vec<_>>();
        for version in &confirmed {
            let Some(effective_from) = version.effective_from else {
                if version.source == TaxProfileVersionSource::MigrationBackfill {
                    continue;
                }
                return Err(format!(
                    "Confirmed profile version '{}' must have an effective start date",
                    version.label
                ));
            };
            if version
                .effective_until
                .is_some_and(|effective_until| effective_until < effective_from)
            {
                return Err(format!(
                    "Confirmed profile version '{}' ends before it starts",
                    version.label
                ));
            }
        }

        confirmed.retain(|version| version.effective_from.is_some());
        confirmed.sort_by(|left, right| {
            left.effective_from
                .cmp(&right.effective_from)
                .then(left.id.cmp(&right.id))
        });
        if let Some(pair) = confirmed.windows(2).find(|pair| {
            let next_start = pair[1]
                .effective_from
                .expect("dated confirmed versions were retained above");
            pair[0]
                .effective_until
                .is_none_or(|previous_end| previous_end >= next_start)
        }) {
            return Err(format!(
                "Confirmed profile versions '{}' and '{}' overlap",
                pair[0].label, pair[1].label
            ));
        }

        Ok(())
    }

    pub fn compliance_mode(&self) -> ComplianceSourceMode {
        self.compliance_source_mode.clone()
    }

    pub fn confirmed_profile_versions(&self) -> Vec<TaxProfileVersion> {
        let mut versions: Vec<_> = self
            .profile_versions
            .iter()
            .filter(|version| version.status == TaxProfileVersionStatus::Confirmed)
            .cloned()
            .collect();

        if versions.is_empty() {
            if self.compliance_source_mode == ComplianceSourceMode::CorVersioned {
                return vec![];
            }
            return vec![TaxProfileVersion::from_profile_backfill(self)];
        }

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
        let versions = self
            .confirmed_profile_versions()
            .into_iter()
            .filter(|version| version.overlaps_period(period_start, period_end))
            .collect::<Vec<_>>();

        if versions.windows(2).any(|pair| {
            pair[0]
                .effective_until
                .is_none_or(|end| end >= pair[1].effective_from.unwrap_or(period_start))
        }) {
            Vec::new()
        } else {
            versions
        }
    }

    pub fn active_profile_versions_for_year(&self, year: u16) -> Vec<TaxProfileVersion> {
        self.resolve_tax_profile_for_year(year).effective_segments
    }

    pub fn resolve_tax_profile_for_year(&self, year: u16) -> ResolvedTaxProfileForYear {
        let year_start = NaiveDate::from_ymd_opt(i32::from(year), 1, 1)
            .expect("u16 taxable year is representable by chrono");
        let year_end = NaiveDate::from_ymd_opt(i32::from(year), 12, 31)
            .expect("u16 taxable year is representable by chrono");
        let mut issues = Vec::new();
        let mut segments = Vec::new();

        for version in self.confirmed_profile_versions() {
            let Some(effective_from) = version.effective_from else {
                issues.push(TaxProfileResolutionIssue {
                    kind: TaxProfileResolutionIssueKind::UndatedConfirmedVersion,
                    version_ids: vec![version.id.clone()],
                    message: format!(
                        "Confirmed profile version '{}' needs an effective start date",
                        version.label
                    ),
                });
                continue;
            };
            if version
                .effective_until
                .is_some_and(|effective_until| effective_until < effective_from)
            {
                issues.push(TaxProfileResolutionIssue {
                    kind: TaxProfileResolutionIssueKind::InvalidEffectiveRange,
                    version_ids: vec![version.id.clone()],
                    message: format!(
                        "Confirmed profile version '{}' ends before it starts",
                        version.label
                    ),
                });
                continue;
            }
            if effective_from <= year_end
                && version
                    .effective_until
                    .is_none_or(|effective_until| effective_until >= year_start)
            {
                segments.push(version);
            }
        }

        segments.sort_by(|left, right| {
            left.effective_from
                .cmp(&right.effective_from)
                .then(left.id.cmp(&right.id))
        });
        let mut overlapping_ids = BTreeSet::new();
        for pair in segments.windows(2) {
            let previous = &pair[0];
            let next = &pair[1];
            let next_start = next
                .effective_from
                .expect("dated versions are required before sorting");
            if previous
                .effective_until
                .is_none_or(|previous_end| previous_end >= next_start)
            {
                overlapping_ids.insert(previous.id.clone());
                overlapping_ids.insert(next.id.clone());
                issues.push(TaxProfileResolutionIssue {
                    kind: TaxProfileResolutionIssueKind::OverlappingConfirmedVersions,
                    version_ids: vec![previous.id.clone(), next.id.clone()],
                    message: format!(
                        "Confirmed profile versions '{}' and '{}' overlap",
                        previous.label, next.label
                    ),
                });
            }
        }
        segments.retain(|version| !overlapping_ids.contains(&version.id));

        ResolvedTaxProfileForYear {
            taxable_year: year,
            effective_segments: segments,
            issues,
        }
    }

    /// Resolve exactly one confirmed profile version for a filing period.
    ///
    /// The yearly resolver remains the timeline authority. This narrower
    /// resolver combines every calendar year touched by a fiscal period and
    /// then requires one stored, confirmed version to cover the entire period.
    /// Synthetic flat-profile fallbacks are intentionally excluded.
    pub fn resolve_tax_profile_for_period(
        &self,
        period_start: NaiveDate,
        period_end: NaiveDate,
    ) -> ResolvedTaxProfileForPeriod {
        let mut issues = Vec::new();
        let mut segments = Vec::new();
        let stored_confirmed_ids = self
            .profile_versions
            .iter()
            .filter(|version| version.status == TaxProfileVersionStatus::Confirmed)
            .map(|version| version.id.as_str())
            .collect::<BTreeSet<_>>();

        if period_start <= period_end {
            for calendar_year in period_start.year()..=period_end.year() {
                let Ok(year) = u16::try_from(calendar_year) else {
                    continue;
                };
                let resolved = self.resolve_tax_profile_for_year(year);
                for issue in resolved.issues {
                    let applies_to_period = match issue.kind {
                        TaxProfileResolutionIssueKind::OverlappingConfirmedVersions => {
                            issue.version_ids.iter().all(|version_id| {
                                self.profile_versions
                                    .iter()
                                    .find(|version| version.id == *version_id)
                                    .is_some_and(|version| {
                                        version.overlaps_period(period_start, period_end)
                                    })
                            })
                        }
                        _ => true,
                    };
                    if applies_to_period && !issues.contains(&issue) {
                        issues.push(issue);
                    }
                }
            }
        }

        for segment in self.profile_versions.iter().filter(|version| {
            stored_confirmed_ids.contains(version.id.as_str())
                && version.overlaps_period(period_start, period_end)
        }) {
            if !segments
                .iter()
                .any(|existing: &TaxProfileVersion| existing.id == segment.id)
            {
                segments.push(segment.clone());
            }
        }

        segments.sort_by(|left, right| {
            left.effective_from
                .cmp(&right.effective_from)
                .then(left.id.cmp(&right.id))
        });

        let covering_segments = segments
            .iter()
            .filter(|version| {
                version
                    .effective_from
                    .is_some_and(|start| start <= period_start)
                    && version.effective_until.is_none_or(|end| end >= period_end)
            })
            .cloned()
            .collect::<Vec<_>>();

        if segments.len() > 1 || covering_segments.len() > 1 {
            issues.push(TaxProfileResolutionIssue {
                kind: TaxProfileResolutionIssueKind::AmbiguousEffectiveVersionsForPeriod,
                version_ids: segments.iter().map(|version| version.id.clone()).collect(),
                message: format!(
                    "Multiple confirmed taxpayer-profile versions touch the filing period {period_start} through {period_end}"
                ),
            });
        } else if covering_segments.is_empty() {
            issues.push(TaxProfileResolutionIssue {
                kind: TaxProfileResolutionIssueKind::NoEffectiveVersionForPeriod,
                version_ids: segments.iter().map(|version| version.id.clone()).collect(),
                message: format!(
                    "No confirmed taxpayer-profile version covers the complete filing period {period_start} through {period_end}"
                ),
            });
        }

        let effective_segment = if issues.is_empty() && covering_segments.len() == 1 {
            covering_segments.into_iter().next()
        } else {
            None
        };

        ResolvedTaxProfileForPeriod {
            period_start,
            period_end,
            effective_segment,
            issues,
        }
    }

    pub fn current_cor_version(&self, as_of_year: u16) -> Option<TaxProfileVersion> {
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
        projected.compliance_source_mode = ComplianceSourceMode::CorVersioned;
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

        let Some(effective_from) = self.effective_from else {
            return false;
        };
        if self
            .effective_until
            .is_some_and(|effective_until| effective_until < effective_from)
        {
            return false;
        }

        let starts_before_period_ends = effective_from <= period_end;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_profile() -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Flat compatibility name",
            "tin": {
                "segment1": "123",
                "segment2": "456",
                "segment3": "789",
                "branch": "000"
            },
            "rdo_code": "000",
            "line_of_business": "Services",
            "registered_address": "Flat compatibility address",
            "zip_code": "1000",
            "phone": "09170000000",
            "email": "flat@example.com",
            "default_form_type": "2551Qv2018"
        }))
        .expect("minimal profile fixture must deserialize")
    }

    fn confirmed_version(
        profile: &TaxpayerProfile,
        id: &str,
        name: &str,
        effective_from: Option<NaiveDate>,
        effective_until: Option<NaiveDate>,
    ) -> TaxProfileVersion {
        let mut version = TaxProfileVersion::from_profile_backfill(profile);
        version.id = id.to_string();
        version.label = name.to_string();
        version.source = TaxProfileVersionSource::ManualCor;
        version.status = TaxProfileVersionStatus::Confirmed;
        version.effective_from = effective_from;
        version.effective_until = effective_until;
        version.needs_effective_date_review = effective_from.is_none();
        version.cor.registered_name = name.to_string();
        version
    }

    #[test]
    fn vat_text_classification_prefers_non_vat_negation() {
        assert_eq!(
            classify_vat_registration_text("Taxpayer classification: NON-VAT registered"),
            VatRegistrationTextClassification::NonVat
        );
    }

    #[test]
    fn vat_text_classification_requires_explicit_positive_phrase() {
        assert_eq!(
            classify_vat_registration_text("Monthly remittance return of VAT withheld"),
            VatRegistrationTextClassification::Unknown
        );
    }

    #[test]
    fn exact_period_resolution_selects_sequential_mid_year_versions() {
        let mut profile = test_profile();
        profile.profile_versions = vec![
            confirmed_version(
                &profile,
                "first-half",
                "First Half Taxpayer",
                NaiveDate::from_ymd_opt(2026, 1, 1),
                NaiveDate::from_ymd_opt(2026, 6, 30),
            ),
            confirmed_version(
                &profile,
                "second-half",
                "Second Half Taxpayer",
                NaiveDate::from_ymd_opt(2026, 7, 1),
                None,
            ),
        ];

        let q1 = profile.resolve_tax_profile_for_period(
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
        );
        let q3 = profile.resolve_tax_profile_for_period(
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        );

        assert!(!q1.has_blocking_issues());
        assert_eq!(
            q1.effective_segment
                .as_ref()
                .map(|version| version.id.as_str()),
            Some("first-half")
        );
        assert!(!q3.has_blocking_issues());
        assert_eq!(
            q3.effective_segment
                .as_ref()
                .map(|version| version.id.as_str()),
            Some("second-half")
        );
    }

    #[test]
    fn exact_period_resolution_rejects_a_version_change_inside_the_quarter() {
        let mut profile = test_profile();
        profile.profile_versions = vec![
            confirmed_version(
                &profile,
                "before-change",
                "Before Change",
                NaiveDate::from_ymd_opt(2026, 1, 1),
                NaiveDate::from_ymd_opt(2026, 5, 14),
            ),
            confirmed_version(
                &profile,
                "after-change",
                "After Change",
                NaiveDate::from_ymd_opt(2026, 5, 15),
                None,
            ),
        ];

        let resolved = profile.resolve_tax_profile_for_period(
            NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
        );

        assert!(resolved.effective_segment.is_none());
        assert!(resolved.issues.iter().any(|issue| {
            issue.kind == TaxProfileResolutionIssueKind::AmbiguousEffectiveVersionsForPeriod
        }));
    }

    #[test]
    fn exact_period_resolution_never_uses_a_synthetic_flat_profile() {
        let mut profile = test_profile();
        profile.compliance_source_mode = ComplianceSourceMode::TemporalSuggestion;
        profile.business_start_date = NaiveDate::from_ymd_opt(2020, 1, 1);

        let resolved = profile.resolve_tax_profile_for_period(
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
        );

        assert!(resolved.effective_segment.is_none());
        assert!(resolved.issues.iter().any(|issue| {
            issue.kind == TaxProfileResolutionIssueKind::NoEffectiveVersionForPeriod
        }));
    }

    #[test]
    fn exact_period_resolution_propagates_undated_and_overlapping_timeline_issues() {
        let mut undated_profile = test_profile();
        undated_profile.profile_versions = vec![confirmed_version(
            &undated_profile,
            "undated",
            "Undated COR",
            None,
            None,
        )];
        let undated = undated_profile.resolve_tax_profile_for_period(
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
        );
        assert!(undated.effective_segment.is_none());
        assert!(
            undated.issues.iter().any(|issue| {
                issue.kind == TaxProfileResolutionIssueKind::UndatedConfirmedVersion
            })
        );

        let mut overlapping_profile = test_profile();
        overlapping_profile.profile_versions = vec![
            confirmed_version(
                &overlapping_profile,
                "one",
                "One",
                NaiveDate::from_ymd_opt(2025, 1, 1),
                NaiveDate::from_ymd_opt(2026, 2, 15),
            ),
            confirmed_version(
                &overlapping_profile,
                "two",
                "Two",
                NaiveDate::from_ymd_opt(2026, 2, 1),
                None,
            ),
        ];
        let overlapping = overlapping_profile.resolve_tax_profile_for_period(
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
        );
        assert!(overlapping.effective_segment.is_none());
        assert!(overlapping.issues.iter().any(|issue| {
            issue.kind == TaxProfileResolutionIssueKind::OverlappingConfirmedVersions
        }));

        let after_overlap = overlapping_profile.resolve_tax_profile_for_period(
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        );
        assert!(!after_overlap.has_blocking_issues());
        assert_eq!(
            after_overlap
                .effective_segment
                .as_ref()
                .map(|version| version.id.as_str()),
            Some("two")
        );
    }
}
