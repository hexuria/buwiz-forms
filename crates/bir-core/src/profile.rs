//! Taxpayer profile management.

use crate::naming::Tin;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
    /// Standard IMAP LOGIN with an App Password stored in the encrypted DB.
    #[default]
    AppPassword,
    /// Google OAuth2 PKCE flow — refresh tokens stored in the encrypted DB.
    ///
    /// One inbox shared across taxpayer profiles is one credential set.
    GoogleOAuth,
}

/// Canonical inbox key for matching and the `inbox_oauth_tokens` row.
pub fn normalize_inbox_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

/// Whether two addresses name the same mailbox for OAuth / IMAP polling.
pub fn inbox_emails_match(left: &str, right: &str) -> bool {
    let left = left.trim();
    let right = right.trim();
    !left.is_empty()
        && !right.is_empty()
        && normalize_inbox_email(left) == normalize_inbox_email(right)
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
    /// Migrated or imported profile facts that cannot participate in filing
    /// suggestions until their effective date has been reviewed explicitly.
    NeedsReview,
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

    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    let contains_tokens =
        |needle: &[&str]| tokens.windows(needle.len()).any(|window| window == needle);
    let is_explicitly_non_vat = tokens.contains(&"NONVAT")
        || contains_tokens(&["NON", "VAT"])
        || contains_tokens(&["NOT", "VAT", "REGISTERED"])
        || contains_tokens(&["NOT", "A", "VAT", "REGISTERED"])
        || contains_tokens(&["NOT", "REGISTERED", "FOR", "VAT"])
        || contains_tokens(&["NO", "VAT", "REGISTRATION"])
        || contains_tokens(&["VAT", "REGISTRATION", "NO"]);
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

/// Exact timeline change that will occur when a newer profile version is
/// confirmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxProfileVersionAutoCloseConsequence {
    pub version_id: String,
    pub version_label: String,
    pub effective_from: Option<NaiveDate>,
    pub effective_until: NaiveDate,
}

/// Immutable confirmation plan used to disclose and then apply a profile
/// timeline change without letting the warning drift from the mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxProfileVersionConfirmationPlan {
    pub version_id: String,
    pub version_label: String,
    pub effective_from: NaiveDate,
    pub auto_close_consequences: Vec<TaxProfileVersionAutoCloseConsequence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaxProfileResolutionIssueKind {
    UndatedConfirmedVersion,
    InvalidEffectiveRange,
    OverlappingConfirmedVersions,
    /// Kept for stored JSON. Runtime filing no longer uses effective-range
    /// coverage as a blocker (V1 looks up the profile-year clone instead).
    NoEffectiveVersionForPeriod,
    AmbiguousEffectiveVersionsForPeriod,
    /// No clone exists for the requested tax year.
    NoProfileYear,
    /// Requested year is before Business Start Date (incorporation /
    /// registration, or the date the TIN was obtained).
    ProfileYearBeforeBusinessStart,
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

/// Filing-facing fields cloned per tax year. Credentials, TIN, PIN, inbox
/// tokens, and the per-year forms table stay on [`TaxpayerProfile`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ProfileYearFacts {
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub rdo_code: String,
    #[serde(default)]
    pub line_of_business: String,
    #[serde(default)]
    pub registered_address: String,
    #[serde(default)]
    pub zip_code: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub taxpayer_type: TaxpayerType,
    #[serde(default)]
    pub tax_classification: Option<TaxClassification>,
    #[serde(default)]
    pub eopt_tier: Option<EoptTier>,
    #[serde(default)]
    pub business_start_date: Option<NaiveDate>,
    #[serde(default)]
    pub is_vat_registered: bool,
    #[serde(default)]
    pub is_bmbe: bool,
    #[serde(default)]
    pub is_gpp_partner: bool,
    #[serde(default)]
    pub is_create_msme: bool,
    #[serde(default)]
    pub is_expanded_withholding_agent: bool,
    #[serde(default)]
    pub atc_codes: Vec<String>,
    #[serde(default)]
    pub excise_tax_categories: Vec<ExciseTaxCategory>,
    #[serde(default)]
    pub has_employees: bool,
    #[serde(default)]
    pub is_dormant: bool,
    #[serde(default)]
    pub has_single_employer: bool,
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
    pub registration_activity_status: RegistrationActivityStatus,
}

impl ProfileYearFacts {
    pub fn year_id(year: u16) -> String {
        format!("year-{year}")
    }

    pub fn from_profile(profile: &TaxpayerProfile) -> Self {
        Self {
            full_name: profile.full_name.clone(),
            rdo_code: profile.rdo_code.clone(),
            line_of_business: profile.line_of_business.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            phone: profile.phone.clone(),
            email: profile.email.clone(),
            taxpayer_type: profile.taxpayer_type.clone(),
            tax_classification: profile.tax_classification.clone(),
            eopt_tier: profile.eopt_tier.clone(),
            business_start_date: profile.business_start_date,
            is_vat_registered: profile.is_vat_registered,
            is_bmbe: profile.is_bmbe,
            is_gpp_partner: profile.is_gpp_partner,
            is_create_msme: profile.is_create_msme,
            is_expanded_withholding_agent: profile.is_expanded_withholding_agent,
            atc_codes: profile.atc_codes.clone(),
            excise_tax_categories: profile.excise_tax_categories.clone(),
            has_employees: profile.has_employees,
            is_dormant: profile.is_dormant,
            has_single_employer: profile.has_single_employer,
            withholds_compensation: profile.withholds_compensation,
            withholds_expanded: profile.withholds_expanded,
            withholds_final: profile.withholds_final,
            is_top_withholding_agent: profile.is_top_withholding_agent,
            is_government_withholding_entity: profile.is_government_withholding_entity,
            registration_activity_status: profile.registration_activity_status.clone(),
        }
    }

    pub fn apply_to(&self, profile: &mut TaxpayerProfile) {
        profile.full_name = self.full_name.clone();
        profile.rdo_code = self.rdo_code.clone();
        profile.line_of_business = self.line_of_business.clone();
        profile.registered_address = self.registered_address.clone();
        profile.zip_code = self.zip_code.clone();
        profile.phone = self.phone.clone();
        profile.email = self.email.clone();
        profile.taxpayer_type = self.taxpayer_type.clone();
        profile.tax_classification = self.tax_classification.clone();
        profile.eopt_tier = self.eopt_tier.clone();
        profile.business_start_date = self.business_start_date;
        profile.is_vat_registered = self.is_vat_registered;
        profile.is_bmbe = self.is_bmbe;
        profile.is_gpp_partner = self.is_gpp_partner;
        profile.is_create_msme = self.is_create_msme;
        profile.is_expanded_withholding_agent = self.is_expanded_withholding_agent;
        profile.atc_codes = self.atc_codes.clone();
        profile.excise_tax_categories = self.excise_tax_categories.clone();
        profile.has_employees = self.has_employees;
        profile.is_dormant = self.is_dormant;
        profile.has_single_employer = self.has_single_employer;
        profile.withholds_compensation = self.withholds_compensation;
        profile.withholds_expanded = self.withholds_expanded;
        profile.withholds_final = self.withholds_final;
        profile.is_top_withholding_agent = self.is_top_withholding_agent;
        profile.is_government_withholding_entity = self.is_government_withholding_entity;
        profile.registration_activity_status = self.registration_activity_status.clone();
    }

    pub fn as_version(&self, year: u16, profile: &TaxpayerProfile) -> TaxProfileVersion {
        let mut projected = profile.clone();
        self.apply_to(&mut projected);
        let mut version = TaxProfileVersion::from_profile_backfill(&projected);
        let year_start = NaiveDate::from_ymd_opt(i32::from(year), 1, 1)
            .expect("u16 taxable year is representable by chrono");
        let year_end = NaiveDate::from_ymd_opt(i32::from(year), 12, 31)
            .expect("u16 taxable year is representable by chrono");
        version.id = Self::year_id(year);
        version.label = format!("{year} profile");
        version.status = TaxProfileVersionStatus::Confirmed;
        version.source = TaxProfileVersionSource::UserOverride;
        version.effective_from = Some(year_start);
        version.effective_until = Some(year_end);
        version.needs_effective_date_review = false;
        version
    }
}

/// Calendar years offered by the reused year selector: from Business Start
/// Date (when known) through next year. Missing start date does not invent a
/// TIN-obtained date; the historical 2018 lower bound stays.
pub fn profile_year_selector_range(
    business_start: Option<NaiveDate>,
    current_year: i32,
) -> std::ops::RangeInclusive<u16> {
    let upper = u16::try_from(current_year + 1).unwrap_or(u16::MAX);
    let lower = business_start
        .map(|date| u16::try_from(date.year()).unwrap_or(2018))
        .unwrap_or(2018)
        .min(upper);
    lower..=upper
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

    /// Per-tax-year clones. The clone for year Y is what forms read.
    /// Stored inside `profiles.data_json` (no dedicated table).
    #[serde(default)]
    pub profile_years: BTreeMap<u16, ProfileYearFacts>,
}

impl TaxpayerProfile {
    pub fn forms_set_for_year(&self, year: u16) -> Option<&crate::forms::PerYearFormsSet> {
        self.per_year_forms.get(&year)
    }

    /// Business Start Date year: incorporation/registration, or the date the
    /// TIN was obtained when that is the stored date. Missing date does not
    /// invent a substitute.
    pub fn earliest_allowed_profile_year(&self) -> Option<u16> {
        self.business_start_date
            .map(|date| u16::try_from(date.year()).unwrap_or(2018))
    }

    pub fn profile_year_allowed(&self, year: u16) -> Result<(), String> {
        if let Some(earliest) = self.earliest_allowed_profile_year()
            && year < earliest
        {
            return Err(format!(
                "year {year} is before Business Start Date ({earliest})"
            ));
        }
        Ok(())
    }

    pub fn capture_current_as_year(&mut self, year: u16) -> Result<(), String> {
        self.profile_year_allowed(year)?;
        self.profile_years
            .insert(year, ProfileYearFacts::from_profile(self));
        Ok(())
    }

    pub fn clone_profile_year(&mut self, from_year: u16, to_year: u16) -> Result<(), String> {
        self.profile_year_allowed(to_year)?;
        let facts = self
            .profile_years
            .get(&from_year)
            .cloned()
            .ok_or_else(|| format!("no {from_year} profile"))?;
        self.profile_years.insert(to_year, facts);
        Ok(())
    }

    pub fn profile_year_facts(&self, year: u16) -> Result<&ProfileYearFacts, String> {
        self.profile_year_allowed(year).map_err(|error| error)?;
        self.profile_years
            .get(&year)
            .ok_or_else(|| format!("no {year} profile"))
    }

    /// Project this taxpayer as the clone for `year`, leaving TIN and
    /// credentials on the parent. Missing year is an error, not a flat-field
    /// fallback.
    pub fn projection_for_year(&self, year: u16) -> Result<TaxpayerProfile, String> {
        let facts = self.profile_year_facts(year)?;
        let mut projected = self.clone();
        facts.apply_to(&mut projected);
        projected.profile_versions = Vec::new();
        Ok(projected)
    }

    /// Identity-cleared projection that keeps the TIN (the profile key) when
    /// the requested year has no clone.
    pub fn tin_only_projection(&self) -> TaxpayerProfile {
        let mut projected = self.clone();
        ProfileYearFacts::default().apply_to(&mut projected);
        projected.full_name.clear();
        projected.rdo_code.clear();
        projected.line_of_business.clear();
        projected.registered_address.clear();
        projected.zip_code.clear();
        projected.phone.clear();
        projected.email.clear();
        projected.profile_versions = Vec::new();
        projected
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

    /// The recorded income-tax election for a taxable year, if any.
    pub fn income_tax_election_for_year(&self, year: u16) -> Option<IncomeTaxElection> {
        self.tax_elections
            .iter()
            .find(|history| history.taxable_year == year)
            .map(|history| history.election.clone())
    }

    /// Whether the taxpayer may manage an annual income-tax election
    /// (8% flat rate, graduated + OSD, …) for the given taxable year.
    ///
    /// Eligibility is a per-year fact from the profile-year clone
    /// (Individual registered as Self-Employed or Mixed Income).
    pub fn eligible_for_income_tax_election_in_year(&self, year: u16) -> bool {
        let Ok(facts) = self.profile_year_facts(year) else {
            return false;
        };
        facts.taxpayer_type == TaxpayerType::Individual
            && matches!(
                facts.tax_classification,
                Some(TaxClassification::SelfEmployed) | Some(TaxClassification::MixedIncome)
            )
    }

    /// Returns true if email tracking is active.
    pub fn is_email_tracking_active(&self) -> bool {
        self.email_tracking_enabled
    }

    /// Mailbox used for BIR confirmation tracking, if one is configured.
    ///
    /// Prefers a non-empty IMAP login, then the profile email. Empty values
    /// are treated as unknown rather than invented.
    pub fn tracking_mailbox(&self) -> Option<&str> {
        let imap = self
            .imap_email
            .as_deref()
            .map(str::trim)
            .filter(|email| !email.is_empty());
        imap.or_else(|| {
            let email = self.email.trim();
            if email.is_empty() { None } else { Some(email) }
        })
    }

    /// Inbox used for IMAP / Gmail OAuth login.
    ///
    /// Several taxpayer profiles can share one mailbox. Tokens and polling
    /// key off this address (`imap_email`, else the taxpayer `email`), not
    /// the profile list order.
    pub fn inbox_email(&self) -> &str {
        self.tracking_mailbox().unwrap_or("")
    }

    /// Whether this profile currently holds a refresh token the poller can try.
    ///
    /// An empty string is treated as disconnected. A non-empty value can still
    /// be revoked at Google; reconnect must replace it rather than keep it.
    pub fn has_usable_oauth_refresh(&self) -> bool {
        self.oauth_refresh_token
            .as_deref()
            .is_some_and(|token| !token.trim().is_empty())
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
        self.normalize_profile_version_review_statuses();
    }

    /// Normalize legacy undated migration backfills into an explicit
    /// fail-closed review state.
    ///
    /// Early profile-ledger builds serialized these records as `Confirmed`
    /// while also setting `needs_effective_date_review`. The status is the
    /// authority used by obligation resolution, so the boolean alone was not
    /// sufficient to keep the record out of confirmed-profile UI and APIs.
    pub(crate) fn normalize_profile_version_review_statuses(&mut self) -> bool {
        let mut changed = false;
        for version in &mut self.profile_versions {
            if version.source == TaxProfileVersionSource::MigrationBackfill
                && version.effective_from.is_none()
                && version.status != TaxProfileVersionStatus::Archived
            {
                if version.status != TaxProfileVersionStatus::NeedsReview {
                    version.status = TaxProfileVersionStatus::NeedsReview;
                    changed = true;
                }
                if !version.needs_effective_date_review {
                    version.needs_effective_date_review = true;
                    changed = true;
                }
            }
        }
        changed
    }

    #[allow(dead_code)]
    pub(crate) fn validate_confirmed_profile_timeline(&mut self) -> Result<(), String> {
        self.normalize_profile_version_review_statuses();

        let mut confirmed = self
            .profile_versions
            .iter()
            .filter(|version| version.status == TaxProfileVersionStatus::Confirmed)
            .collect::<Vec<_>>();
        for version in &confirmed {
            let Some(effective_from) = version.effective_from else {
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

    /// Return the clone for tax year `year`. No effective-date overlap scan.
    ///
    /// When `profile_years` is still empty (pre-V1 rows / migrations), fall
    /// back to the stored COR ledger so historical backfills keep working.
    /// Forms that fill Part I must call [`Self::projection_for_year`].
    pub fn resolve_tax_profile_for_year(&self, year: u16) -> ResolvedTaxProfileForYear {
        if self.profile_years.is_empty() {
            let legacy = self.resolve_tax_profile_for_year_from_ledger(year);
            if !legacy.effective_segments.is_empty() || !legacy.issues.is_empty() {
                return legacy;
            }
            return ResolvedTaxProfileForYear {
                taxable_year: year,
                effective_segments: Vec::new(),
                issues: vec![TaxProfileResolutionIssue {
                    kind: TaxProfileResolutionIssueKind::NoProfileYear,
                    version_ids: Vec::new(),
                    message: format!("no {year} profile"),
                }],
            };
        }
        let mut issues = Vec::new();
        let mut segments = Vec::new();
        match self.profile_year_facts(year) {
            Ok(facts) => segments.push(facts.as_version(year, self)),
            Err(message) => {
                let kind = if self.profile_year_allowed(year).is_err() {
                    TaxProfileResolutionIssueKind::ProfileYearBeforeBusinessStart
                } else {
                    TaxProfileResolutionIssueKind::NoProfileYear
                };
                issues.push(TaxProfileResolutionIssue {
                    kind,
                    version_ids: Vec::new(),
                    message,
                });
            }
        }

        ResolvedTaxProfileForYear {
            taxable_year: year,
            effective_segments: segments,
            issues,
        }
    }

    fn resolve_tax_profile_for_year_from_ledger(&self, year: u16) -> ResolvedTaxProfileForYear {
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

    /// Resolve the profile-year clone for the filing period's tax year.
    ///
    /// Periods that stay inside one calendar year use that year. A period that
    /// spans two calendar years (fiscal `?`) uses `period_end`'s year rather
    /// than scanning `effective_from` / `effective_until`. Forms that know
    /// their taxable year should call [`Self::projection_for_year`] instead.
    pub fn resolve_tax_profile_for_period(
        &self,
        period_start: NaiveDate,
        period_end: NaiveDate,
    ) -> ResolvedTaxProfileForPeriod {
        if period_start > period_end {
            return ResolvedTaxProfileForPeriod {
                period_start,
                period_end,
                effective_segment: None,
                issues: vec![TaxProfileResolutionIssue {
                    kind: TaxProfileResolutionIssueKind::NoProfileYear,
                    version_ids: Vec::new(),
                    message: format!(
                        "The filing period {period_start} through {period_end} is invalid"
                    ),
                }],
            };
        }
        let year = u16::try_from(period_end.year()).unwrap_or(0);
        let resolved = self.resolve_tax_profile_for_year(year);
        ResolvedTaxProfileForPeriod {
            period_start,
            period_end,
            effective_segment: resolved.effective_segments.first().cloned(),
            issues: resolved.issues,
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

    pub fn profile_version_confirmation_plan(
        &self,
        version_id: &str,
        effective_from: NaiveDate,
    ) -> Option<TaxProfileVersionConfirmationPlan> {
        let version = self
            .profile_versions
            .iter()
            .find(|version| version.id == version_id)?;
        let effective_until = effective_from.checked_sub_signed(Duration::days(1))?;
        let auto_close_consequences = self
            .profile_versions
            .iter()
            .filter(|prior| {
                prior.id != version_id
                    && prior.status == TaxProfileVersionStatus::Confirmed
                    && prior.effective_until.is_none()
                    && (prior.source == TaxProfileVersionSource::MigrationBackfill
                        || prior
                            .effective_from
                            .is_none_or(|start| start < effective_from))
            })
            .map(|prior| TaxProfileVersionAutoCloseConsequence {
                version_id: prior.id.clone(),
                version_label: prior.label.clone(),
                effective_from: prior.effective_from,
                effective_until,
            })
            .collect();

        Some(TaxProfileVersionConfirmationPlan {
            version_id: version.id.clone(),
            version_label: version.label.clone(),
            effective_from,
            auto_close_consequences,
        })
    }

    pub fn apply_profile_version_confirmation_plan(
        &mut self,
        plan: &TaxProfileVersionConfirmationPlan,
    ) -> bool {
        let Some(current_plan) =
            self.profile_version_confirmation_plan(&plan.version_id, plan.effective_from)
        else {
            return false;
        };
        if &current_plan != plan {
            return false;
        }

        for consequence in &plan.auto_close_consequences {
            let Some(version) = self
                .profile_versions
                .iter_mut()
                .find(|version| version.id == consequence.version_id)
            else {
                return false;
            };
            version.effective_until = Some(consequence.effective_until);
        }

        let Some(version) = self
            .profile_versions
            .iter_mut()
            .find(|version| version.id == plan.version_id)
        else {
            return false;
        };
        version.status = TaxProfileVersionStatus::Confirmed;
        version.effective_from = Some(plan.effective_from);
        version.effective_until = None;
        version.needs_effective_date_review = false;
        self.compliance_source_mode = ComplianceSourceMode::CorVersioned;
        true
    }

    pub fn set_profile_version_confirmed(
        &mut self,
        version_id: &str,
        effective_from: NaiveDate,
    ) -> bool {
        let Some(plan) = self.profile_version_confirmation_plan(version_id, effective_from) else {
            return false;
        };
        self.apply_profile_version_confirmation_plan(&plan)
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
            status: if effective_from.is_some() {
                TaxProfileVersionStatus::Confirmed
            } else {
                TaxProfileVersionStatus::NeedsReview
            },
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

    #[test]
    fn tracking_mailbox_prefers_imap_and_omits_empty() {
        let mut profile = test_profile();
        assert_eq!(profile.tracking_mailbox(), Some("flat@example.com"));
        profile.imap_email = Some("  receipts@example.com  ".into());
        assert_eq!(profile.tracking_mailbox(), Some("receipts@example.com"));
        profile.imap_email = Some("   ".into());
        profile.email = "  ".into();
        assert_eq!(profile.tracking_mailbox(), None);
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

    fn draft_version(profile: &TaxpayerProfile, id: &str, name: &str) -> TaxProfileVersion {
        let mut version = TaxProfileVersion::from_profile_backfill(profile);
        version.id = id.to_string();
        version.label = name.to_string();
        version.source = TaxProfileVersionSource::ManualCor;
        version.status = TaxProfileVersionStatus::Draft;
        version.effective_from = None;
        version.effective_until = None;
        version.needs_effective_date_review = true;
        version.cor.registered_name = name.to_string();
        version
    }

    #[test]
    fn undated_backfill_is_explicitly_needs_review_and_not_confirmed() {
        let mut profile = test_profile();
        profile.business_start_date = None;
        profile.profile_versions.clear();

        profile.ensure_profile_version_ledger();

        assert_eq!(profile.profile_versions.len(), 1);
        assert_eq!(
            profile.profile_versions[0].status,
            TaxProfileVersionStatus::NeedsReview
        );
        assert!(profile.profile_versions[0].needs_effective_date_review);
        assert!(profile.confirmed_profile_versions().is_empty());
        assert_eq!(
            serde_json::to_string(&profile.profile_versions[0].status).unwrap(),
            "\"NeedsReview\""
        );
    }

    #[test]
    fn legacy_undated_confirmed_backfill_normalizes_to_needs_review() {
        let mut profile = test_profile();
        profile.business_start_date = None;
        let mut legacy_version = TaxProfileVersion::from_profile_backfill(&profile);
        legacy_version.status = TaxProfileVersionStatus::Confirmed;
        legacy_version.needs_effective_date_review = false;
        profile.profile_versions = vec![legacy_version];

        profile.ensure_profile_version_ledger();

        assert_eq!(
            profile.profile_versions[0].status,
            TaxProfileVersionStatus::NeedsReview
        );
        assert!(profile.profile_versions[0].needs_effective_date_review);
        assert!(
            profile
                .resolve_tax_profile_for_year(2026)
                .effective_segments
                .is_empty()
        );
    }

    #[test]
    fn election_eligibility_follows_the_selected_years_clone() {
        let mut profile = test_profile();
        profile.taxpayer_type = TaxpayerType::Individual;
        profile.tax_classification = Some(TaxClassification::SelfEmployed);
        profile.capture_current_as_year(2024).unwrap();
        profile.tax_classification = Some(TaxClassification::PurelyCompensation);
        profile.capture_current_as_year(2026).unwrap();

        assert!(profile.eligible_for_income_tax_election_in_year(2024));
        assert!(!profile.eligible_for_income_tax_election_in_year(2026));
        assert!(!profile.eligible_for_income_tax_election_in_year(2019));
    }

    #[test]
    fn election_eligibility_fails_closed_without_a_profile_year() {
        let mut profile = test_profile();
        profile.taxpayer_type = TaxpayerType::Individual;
        profile.tax_classification = Some(TaxClassification::SelfEmployed);
        profile.profile_years.clear();

        assert!(!profile.eligible_for_income_tax_election_in_year(2026));
    }

    #[test]
    fn confirmation_plan_reports_the_exact_auto_close_consequence() {
        let mut profile = test_profile();
        profile.profile_versions = vec![
            confirmed_version(
                &profile,
                "prior",
                "Prior COR",
                NaiveDate::from_ymd_opt(2025, 1, 1),
                None,
            ),
            draft_version(&profile, "replacement", "Replacement COR"),
        ];

        let effective_from = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let plan = profile
            .profile_version_confirmation_plan("replacement", effective_from)
            .unwrap();

        assert_eq!(
            plan,
            TaxProfileVersionConfirmationPlan {
                version_id: "replacement".to_string(),
                version_label: "Replacement COR".to_string(),
                effective_from,
                auto_close_consequences: vec![TaxProfileVersionAutoCloseConsequence {
                    version_id: "prior".to_string(),
                    version_label: "Prior COR".to_string(),
                    effective_from: NaiveDate::from_ymd_opt(2025, 1, 1),
                    effective_until: NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
                }],
            }
        );
    }

    #[test]
    fn building_a_confirmation_plan_leaves_profile_state_unchanged() {
        let mut profile = test_profile();
        profile.profile_versions = vec![
            confirmed_version(
                &profile,
                "prior",
                "Prior COR",
                NaiveDate::from_ymd_opt(2025, 1, 1),
                None,
            ),
            draft_version(&profile, "replacement", "Replacement COR"),
        ];
        let before = serde_json::to_value(&profile).unwrap();

        let _ = profile.profile_version_confirmation_plan(
            "replacement",
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
        );

        assert_eq!(serde_json::to_value(&profile).unwrap(), before);
    }

    #[test]
    fn applying_a_stale_confirmation_plan_fails_without_mutating_state() {
        let mut profile = test_profile();
        profile.profile_versions = vec![
            confirmed_version(
                &profile,
                "prior",
                "Prior COR",
                NaiveDate::from_ymd_opt(2025, 1, 1),
                None,
            ),
            draft_version(&profile, "replacement", "Replacement COR"),
        ];
        let plan = profile
            .profile_version_confirmation_plan(
                "replacement",
                NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
            )
            .unwrap();
        profile.profile_versions[0].label = "Edited while dialog was open".to_string();
        let before = serde_json::to_value(&profile).unwrap();

        let applied = profile.apply_profile_version_confirmation_plan(&plan);

        assert_eq!(
            (applied, serde_json::to_value(&profile).unwrap()),
            (false, before)
        );
    }

    #[test]
    fn confirming_a_missing_version_does_not_close_the_current_version() {
        let mut profile = test_profile();
        profile.profile_versions = vec![confirmed_version(
            &profile,
            "prior",
            "Prior COR",
            NaiveDate::from_ymd_opt(2025, 1, 1),
            None,
        )];
        let before = serde_json::to_value(&profile).unwrap();

        let applied = profile
            .set_profile_version_confirmed("missing", NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());

        assert_eq!(
            (applied, serde_json::to_value(&profile).unwrap()),
            (false, before)
        );
    }

    #[test]
    fn vat_text_classification_prefers_non_vat_negation() {
        for text in [
            "Taxpayer classification: NON-VAT registered",
            "Taxpayer classification: NONVAT",
            "NOT VAT REGISTERED",
            "NOT A VAT REGISTERED TAXPAYER",
            "NOT REGISTERED FOR VAT",
            "VAT REGISTRATION: NO",
            "NO VAT REGISTRATION",
        ] {
            assert_eq!(
                classify_vat_registration_text(text),
                VatRegistrationTextClassification::NonVat,
                "{text:?} must be treated as explicit non-VAT evidence"
            );
        }
    }

    #[test]
    fn vat_text_classification_requires_explicit_positive_phrase() {
        assert_eq!(
            classify_vat_registration_text("Monthly remittance return of VAT withheld"),
            VatRegistrationTextClassification::Unknown
        );
    }

    #[test]
    fn vat_text_classification_accepts_explicit_registration_only() {
        for text in [
            "VAT REGISTERED",
            "REGISTERED FOR VAT",
            "VALUE ADDED TAX REGISTERED",
        ] {
            assert_eq!(
                classify_vat_registration_text(text),
                VatRegistrationTextClassification::VatRegistered,
                "{text:?} is explicit positive VAT-registration evidence"
            );
        }
    }

    #[test]
    fn profile_year_lookup_returns_the_clone_for_that_year() {
        let mut profile = test_profile();
        profile.business_start_date = NaiveDate::from_ymd_opt(2020, 1, 1);
        profile.full_name = "2026 Name".into();
        profile.rdo_code = "018".into();
        profile.capture_current_as_year(2026).unwrap();
        profile.full_name = "2025 Name".into();
        profile.rdo_code = "019".into();
        profile.capture_current_as_year(2025).unwrap();

        let y2026 = profile.resolve_tax_profile_for_year(2026);
        let y2025 = profile.resolve_tax_profile_for_year(2025);
        assert!(!y2026.has_blocking_issues());
        assert_eq!(
            y2026.effective_segments[0].cor.registered_name,
            "2026 Name"
        );
        assert_eq!(y2026.effective_segments[0].cor.rdo_code, "018");
        assert_eq!(
            y2025.effective_segments[0].cor.registered_name,
            "2025 Name"
        );
        assert_eq!(y2025.effective_segments[0].id, "year-2025");
        assert_ne!(
            y2026.effective_segments[0].id,
            y2025.effective_segments[0].id
        );
    }

    #[test]
    fn profile_year_before_business_start_date_is_rejected() {
        let mut profile = test_profile();
        profile.business_start_date = NaiveDate::from_ymd_opt(2024, 6, 1);
        profile.capture_current_as_year(2024).unwrap();

        let resolved = profile.resolve_tax_profile_for_year(2023);
        assert!(resolved.effective_segments.is_empty());
        assert!(resolved.issues.iter().any(|issue| {
            issue.kind == TaxProfileResolutionIssueKind::ProfileYearBeforeBusinessStart
        }));
        assert!(
            profile
                .capture_current_as_year(2019)
                .unwrap_err()
                .contains("before Business Start Date")
        );
    }

    #[test]
    fn profile_year_selector_range_clamps_to_business_start() {
        let with_start = profile_year_selector_range(
            NaiveDate::from_ymd_opt(2024, 6, 1),
            2026,
        );
        assert_eq!(*with_start.start(), 2024);
        assert_eq!(*with_start.end(), 2027);

        let missing = profile_year_selector_range(None, 2026);
        assert_eq!(*missing.start(), 2018);
        assert_eq!(*missing.end(), 2027);
    }

    #[test]
    fn missing_profile_year_is_not_an_effective_range_gap() {
        let profile = test_profile();
        let resolved = profile.resolve_tax_profile_for_year(2026);
        assert!(resolved.effective_segments.is_empty());
        assert!(
            resolved
                .issues
                .iter()
                .any(|issue| issue.kind == TaxProfileResolutionIssueKind::NoProfileYear
                    && issue.message == "no 2026 profile")
        );
        assert!(
            !resolved.issues.iter().any(|issue| {
                issue.kind == TaxProfileResolutionIssueKind::NoEffectiveVersionForPeriod
            })
        );
    }

    #[test]
    fn period_resolution_looks_up_the_calendar_year_clone() {
        let mut profile = test_profile();
        profile.full_name = "Year 2026".into();
        profile.capture_current_as_year(2026).unwrap();

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
            Some("year-2026")
        );
        assert_eq!(
            q3.effective_segment
                .as_ref()
                .map(|version| version.id.as_str()),
            Some("year-2026")
        );
    }

    #[test]
    fn period_resolution_never_uses_the_flat_profile_when_the_year_is_missing() {
        let mut profile = test_profile();
        profile.compliance_source_mode = ComplianceSourceMode::TemporalSuggestion;
        profile.business_start_date = NaiveDate::from_ymd_opt(2020, 1, 1);

        let resolved = profile.resolve_tax_profile_for_period(
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
        );

        assert!(resolved.effective_segment.is_none());
        assert!(
            resolved
                .issues
                .iter()
                .any(|issue| issue.kind == TaxProfileResolutionIssueKind::NoProfileYear)
        );
    }

    #[test]
    fn confirmed_versions_never_synthesize_the_flat_profile() {
        let mut profile = test_profile();
        profile.compliance_source_mode = ComplianceSourceMode::TemporalSuggestion;
        profile.business_start_date = NaiveDate::from_ymd_opt(2020, 1, 1);

        assert!(profile.profile_versions.is_empty());
        assert!(profile.confirmed_profile_versions().is_empty());

        profile.ensure_profile_version_ledger();
        assert_eq!(profile.confirmed_profile_versions().len(), 1);
        assert_eq!(
            profile.confirmed_profile_versions()[0].id,
            "legacy-current-profile"
        );

        profile.ensure_profile_version_ledger();
        assert_eq!(profile.profile_versions.len(), 1);
        assert!(
            profile
                .resolve_tax_profile_for_year(2026)
                .effective_segments
                .iter()
                .any(|version| version.id == "legacy-current-profile")
        );
    }

    #[test]
    fn inbox_email_prefers_imap_over_taxpayer_email() {
        let mut profile = test_profile();
        profile.email = "jane@example.com".to_string();
        profile.imap_email = None;
        assert_eq!(profile.inbox_email(), "jane@example.com");

        profile.imap_email = Some("  codeitlikemiley@gmail.com  ".to_string());
        assert_eq!(profile.inbox_email(), "codeitlikemiley@gmail.com");
        assert!(inbox_emails_match(
            profile.inbox_email(),
            "CodeItLikeMiley@gmail.com"
        ));
        assert!(!profile.has_usable_oauth_refresh());
        profile.oauth_refresh_token = Some("   ".to_string());
        assert!(!profile.has_usable_oauth_refresh());
        profile.oauth_refresh_token = Some("refresh-good".to_string());
        assert!(profile.has_usable_oauth_refresh());
    }
}
