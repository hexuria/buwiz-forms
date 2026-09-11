//! Durable scoped filing queue.
//!
//! Enqueue is already shipped (painted Submit, agent `confirm=true`,
//! headless tin/code/year/period). This module does **not** add a second
//! queue verb. It records the grant those paths already create, then gives
//! workers a lease, restart recovery, and Submitted→Confirmed completion.
//!
//! 1. Authorization is scoped to form + TIN + period + Confirmed.
//! 2. Workers retry that same authorized task without asking again.
//! 3. Eventual completion is Submitted → Confirmed (IMAP). Dry-run stops
//!    at Submitted and never pretends to be a BIR confirmation.
//!
//! Persistence lives on the `form_drafts` row (SQLCipher JSON). There is no
//! second queue table.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::forms::form_1601c::Form1601CDraft;
use crate::profile::TaxpayerProfile;

/// How long a claim may sit with **no PUT started** before a worker may
/// recover it. Once PUT is marked started, the claim is fail-closed.
pub const CLAIM_LEASE: Duration = Duration::seconds(120);

/// What the authorizer accepted as "this task is done".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueCompletion {
    /// Live filing: encrypted PUT, then wait for a BIR confirmation receipt.
    Confirmed,
}

/// Surface that collected the explicit authorization.
///
/// Enqueue itself is already shipped (GUI Submit, agent `confirm=true`,
/// headless tin/code/year/period). This enum is recorded on that existing
/// `transition_to_queued` path. Do not add a second queue verb.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueAuthSource {
    /// Recorded by `transition_to_queued` (painted Submit and agent confirm=true).
    Gui,
    /// Reserved. The shipped agent host already calls `transition_to_queued`.
    AgentConfirm,
}

/// Scoped grant recorded at enqueue. Workers reuse this grant on retry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueAuthorization {
    pub form_code: String,
    pub tin: String,
    pub period: String,
    pub completion: QueueCompletion,
    pub source: QueueAuthSource,
    pub authorized_at: String,
}

impl QueueAuthorization {
    pub fn new(
        form_code: impl Into<String>,
        tin: impl Into<String>,
        period: impl Into<String>,
        source: QueueAuthSource,
    ) -> Self {
        Self {
            form_code: form_code.into(),
            tin: tin.into(),
            period: period.into(),
            completion: QueueCompletion::Confirmed,
            source,
            authorized_at: Utc::now().to_rfc3339(),
        }
    }

    pub fn matches(&self, form_code: &str, tin: &str, period: &str) -> bool {
        self.form_code == form_code && self.tin == tin && self.period == period
    }
}

/// Workers only PUT a queued return whose grant still matches identity.
///
/// Missing auth (legacy queued rows) and mismatched grants both fail closed
/// back to Draft at revalidate, so a worker cannot file a different form,
/// TIN, or period than the human authorized.
pub fn scoped_authorization_error(
    auth: Option<&QueueAuthorization>,
    form_code: &str,
    tin: &str,
    period: &str,
) -> Option<(String, String)> {
    match auth {
        Some(auth)
            if auth.matches(form_code, tin, period)
                && auth.completion == QueueCompletion::Confirmed =>
        {
            None
        }
        Some(_) => Some((
            "queue_authorization".to_string(),
            "Queue authorization no longer matches this return; review and queue it again"
                .to_string(),
        )),
        None => Some((
            "queue_authorization".to_string(),
            "Queued return has no scoped authorization; reopen and queue it again before submission"
                .to_string(),
        )),
    }
}

pub fn claim_lease_until(now: DateTime<Utc>) -> String {
    (now + CLAIM_LEASE).to_rfc3339()
}

/// Recover a claim only when PUT never started and the lease has expired.
///
/// Legacy rows with no lease stay fail-closed (human
/// `abandoned_no_bir_filing` only). A claim that recorded PUT start stays
/// fail-closed even after the lease timestamp — bytes may have reached BIR.
pub fn claim_is_recoverable(
    claimed_at: Option<&str>,
    lease_until: Option<&str>,
    put_started_at: Option<&str>,
    now: DateTime<Utc>,
) -> bool {
    if claimed_at.is_none() {
        return false;
    }
    if put_started_at.is_some() {
        return false;
    }
    let Some(lease_until) = lease_until else {
        return false;
    };
    match DateTime::parse_from_rfc3339(lease_until) {
        Ok(until) => now >= until.with_timezone(&Utc),
        Err(_) => false,
    }
}

/// Mandatory lab taxpayer for 1601-C tests and smoke scripts.
///
/// TIN `00000000000000`, Juan Dela Cruz, RDO 018, Olongapo, Software
/// Development, ZIP 2200, `codeitlikemiley@gmail.com`, `09156837000`.
/// Never substitute a real taxpayer. Never invent tax amounts.
pub fn mandatory_lab_1601c_profile() -> TaxpayerProfile {
    serde_json::from_value(serde_json::json!({
        "id": null,
        "full_name": "Juan Dela Cruz",
        "tin": {
            "segment1": "000",
            "segment2": "000",
            "segment3": "000",
            "branch": "00000"
        },
        "rdo_code": "018",
        "line_of_business": "Software Development",
        "registered_address": "Olongapo",
        "zip_code": "2200",
        "phone": "09156837000",
        "email": "codeitlikemiley@gmail.com",
        "default_form_type": "1601Cv2018",
        "taxpayer_type": "Individual"
    }))
    .expect("mandatory lab 1601-C profile must deserialize")
}

/// Zero-tax September 2026 1601-C on the mandatory lab profile.
///
/// Amended No, Any Taxes Withheld No, Category Private. All money fields stay
/// `0.00`. Constructor default withheld=Yes is explicitly cleared.
pub fn mandatory_lab_1601c_draft() -> Form1601CDraft {
    let profile = mandatory_lab_1601c_profile();
    let mut draft = Form1601CDraft::new_from_profile(&profile, 2026, 9);
    draft.is_amended = false;
    draft.any_taxes_withheld = false;
    draft.category_of_agent = "P".to_string();
    draft.compute();
    draft
}

pub fn parse_1601c_period(period: &str) -> Option<(u16, u8)> {
    let digits: String = period.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() != 6 {
        return None;
    }
    let month: u8 = digits[..2].parse().ok()?;
    let year: u16 = digits[2..].parse().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    Some((year, month))
}

pub fn is_audited_1601c_receipt_form_type(form_type: &str) -> bool {
    matches!(form_type, "1601Cv2018" | "1601C")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::FormValidator;

    #[test]
    fn lease_recovers_only_expired_unstarted_claims() {
        let now = Utc::now();
        let future = (now + Duration::seconds(30)).to_rfc3339();
        let past = (now - Duration::seconds(1)).to_rfc3339();
        let claimed = now.to_rfc3339();

        assert!(!claim_is_recoverable(None, Some(&past), None, now));
        assert!(!claim_is_recoverable(Some(&claimed), None, None, now));
        assert!(!claim_is_recoverable(
            Some(&claimed),
            Some(&future),
            None,
            now
        ));
        assert!(claim_is_recoverable(Some(&claimed), Some(&past), None, now));
        assert!(!claim_is_recoverable(
            Some(&claimed),
            Some(&past),
            Some(&claimed),
            now
        ));
        assert!(!claim_is_recoverable(
            Some(&claimed),
            Some("not-rfc3339"),
            None,
            now
        ));
    }

    #[test]
    fn authorization_is_scoped_to_form_tin_period() {
        let auth =
            QueueAuthorization::new("1601C", "00000000000000", "092026", QueueAuthSource::Gui);
        assert!(auth.matches("1601C", "00000000000000", "092026"));
        assert!(!auth.matches("2551Q", "00000000000000", "092026"));
        assert!(!auth.matches("1601C", "00000000000001", "092026"));
        assert!(!auth.matches("1601C", "00000000000000", "082026"));
        assert_eq!(auth.completion, QueueCompletion::Confirmed);
        assert!(
            scoped_authorization_error(Some(&auth), "1601C", "00000000000000", "092026").is_none()
        );
        assert!(
            scoped_authorization_error(Some(&auth), "2551Q", "00000000000000", "092026").is_some()
        );
        assert!(scoped_authorization_error(None, "1601C", "00000000000000", "092026").is_some());
    }

    #[test]
    fn mandatory_fixture_validates_queues_and_emits_zero_tax_xml() {
        let mut draft = mandatory_lab_1601c_draft();
        assert_eq!(draft.tin, "00000000000000");
        assert_eq!(draft.taxpayer_name, "Juan Dela Cruz");
        assert_eq!(draft.rdo_code, "018");
        assert_eq!(draft.registered_address, "Olongapo");
        assert_eq!(draft.line_of_business, "Software Development");
        assert_eq!(draft.zip_code, "2200");
        assert_eq!(draft.email_address, "codeitlikemiley@gmail.com");
        assert_eq!(draft.contact_number, "09156837000");
        assert_eq!(draft.month, 9);
        assert_eq!(draft.taxable_year, 2026);
        assert!(!draft.is_amended);
        assert!(!draft.any_taxes_withheld);
        assert_eq!(draft.category_of_agent, "P");
        assert_eq!(draft.tax_14_total_compensation, 0.0);
        assert_eq!(draft.tax_25_total_taxes_withheld, 0.0);
        assert_eq!(draft.tax_36_total_amount_payable, 0.0);
        assert!(draft.validate().is_empty(), "{:?}", draft.validate());

        let mut withheld_yes = draft.clone();
        withheld_yes.any_taxes_withheld = true;
        assert!(
            withheld_yes
                .validate()
                .iter()
                .any(|(field, _)| field == "tax_14_total_compensation")
        );

        draft
            .transition_to_queued()
            .expect("zero-tax withheld=No fixture must queue");
        assert!(draft.queue_authorization.as_ref().is_some_and(|auth| {
            auth.matches("1601C", "00000000000000", "092026")
                && auth.completion == QueueCompletion::Confirmed
        }));

        let xml = draft
            .try_to_bir_xml_payload()
            .expect("queued fixture must emit XML");
        let fields = draft.to_bir_field_map();
        assert_eq!(fields["frm1601c:txtMonth"], "09");
        assert_eq!(fields["frm1601c:txtYear"], "2026");
        assert_eq!(fields["frm1601c:TaxWithheld_1"], "false");
        assert_eq!(fields["frm1601c:TaxWithheld_2"], "true");
        assert_eq!(fields["frm1601c:CatAgent_P"], "true");
        assert_eq!(fields["frm1601c:txtATC"], "WW010");
        assert_eq!(fields["frm1601c:txtTax14"], "0.00");
        assert_eq!(fields["frm1601c:txtTax25"], "0.00");
        assert_eq!(fields["frm1601c:txtTax36"], "0.00");
        assert_eq!(
            draft.default_submission_filename(),
            "00000000000000-1601Cv2018-092026#codeitlikemiley@gmail.com#.xml"
        );
        assert!(xml.contains("frm1601c:txtMonth"), "{xml}");
        assert_eq!(
            fields.len(),
            crate::forms::form_1601c::EXACT_REVIEWED_PLAIN_XML_FIELD_COUNT
        );
    }

    #[test]
    fn parse_1601c_period_reads_mmyyyy() {
        assert_eq!(parse_1601c_period("092026"), Some((2026, 9)));
        assert_eq!(parse_1601c_period("122026"), Some((2026, 12)));
        assert_eq!(parse_1601c_period("132026"), None);
        assert_eq!(parse_1601c_period("122026Q1"), None);
    }
}
