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
}

/// Refined tax classification that drives filing behavior.
///
/// This is a refinement of `TaxpayerType` — it specifies *how* the taxpayer
/// files (which forms are required, which ATC/tax rules apply), whereas
/// `TaxpayerType` specifies *what kind* of entity they are.
///
/// For example, an `Individual` taxpayer could be classified as
/// `PurelyCompensation`, `ProfessionalOrFreelancer`, `SoleProprietorNonVat`,
/// `SoleProprietorVat`, or `MixedIncome`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaxClassification {
    /// Individual with only employment income — files 1700/1701 only.
    PurelyCompensation,
    /// Self-employed professional or freelancer — files 1701Q, 2551Q/2550M.
    ProfessionalOrFreelancer,
    /// Sole proprietor NOT registered for VAT — files 2551Q (percentage tax).
    SoleProprietorNonVat,
    /// Sole proprietor registered for VAT — files 2550M/2550Q.
    SoleProprietorVat,
    /// Individual with both compensation and business/professional income.
    MixedIncome,
    /// Corporation — files 1702Q, 1702RT.
    Corporation,
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

    /// Whether the taxpayer opted for the 8% flat income tax rate
    /// (available for self-employed individuals with gross sales ≤ ₱3M).
    #[serde(default)]
    pub opted_for_8_percent_flat_rate: bool,

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
}

impl TaxpayerProfile {
    /// Returns true if email tracking is active (handles legacy `imap_enabled` field).
    pub fn is_email_tracking_active(&self) -> bool {
        self.email_tracking_enabled || self._imap_enabled_compat.unwrap_or(false)
    }

    /// Returns BIR form codes applicable to this taxpayer based on their
    /// classification, VAT status, and employee status.
    pub fn applicable_forms(&self) -> Vec<&'static str> {
        let mut forms = Vec::new();

        // Payment form — universal
        forms.push("0605");

        // Employer withholding forms
        if self.has_employees {
            forms
                .extend_from_slice(&["1601C", "1601E", "1601F", "1602", "1603", "1604CF", "1604E"]);
        }

        match self.tax_classification.as_ref() {
            Some(TaxClassification::PurelyCompensation) => {
                forms.push("1700");
            }
            Some(TaxClassification::ProfessionalOrFreelancer)
            | Some(TaxClassification::SoleProprietorNonVat) => {
                forms.push("1701Q");
                forms.push("1701");
                forms.push("2551Q");
            }
            Some(TaxClassification::SoleProprietorVat) => {
                forms.push("1701Q");
                forms.push("1701");
                forms.push("2550M");
                forms.push("2550Q");
            }
            Some(TaxClassification::MixedIncome) => {
                forms.push("1701Q");
                forms.push("1701");
                if self.is_vat_registered {
                    forms.push("2550M");
                    forms.push("2550Q");
                } else {
                    forms.push("2551Q");
                }
            }
            Some(TaxClassification::Corporation) => {
                forms.push("1702Q");
                forms.push("1702");
                if self.is_vat_registered {
                    forms.push("2550M");
                    forms.push("2550Q");
                }
            }
            None => {
                // No classification set — show common forms
                forms.push("1701Q");
                forms.push("2551Q");
            }
        }

        forms
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
