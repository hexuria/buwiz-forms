//! Taxpayer profile management.

use crate::naming::Tin;
use chrono::NaiveDate;
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

    /// Legacy compat: preserve old `opted_for_8_percent_flat_rate` values from JSON.
    #[serde(default, alias = "opted_for_8_percent_flat_rate")]
    #[doc(hidden)]
    pub _opted_for_8_percent_flat_rate_compat: Option<bool>,

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

    // Legacy compat: keep deserializing old `imap_enabled` as an alias
    #[serde(default, alias = "imap_enabled")]
    #[doc(hidden)]
    pub _imap_enabled_compat: Option<bool>,

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
    /// It checks the historical ledger first, and falls back to the legacy compat flag.
    pub fn has_8_percent_election(&self, year: u16) -> bool {
        if let Some(history) = self.tax_elections.iter().find(|h| h.taxable_year == year) {
            matches!(history.election, IncomeTaxElection::EightPercent)
        } else {
            self._opted_for_8_percent_flat_rate_compat.unwrap_or(false)
        }
    }

    /// Returns true if email tracking is active (handles legacy `imap_enabled` field).
    pub fn is_email_tracking_active(&self) -> bool {
        self.email_tracking_enabled || self._imap_enabled_compat.unwrap_or(false)
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
