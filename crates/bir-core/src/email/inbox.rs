//! Shared-inbox Google OAuth: one mailbox, one credential set.
//!
//! The confirmation poller used to take the first `list_profiles()` hit whose
//! email matched. Reconnecting Jane or Juan then left Alejandro's revoked
//! refresh token in the poll path forever.

use crate::db::{Database, InboxOAuthTokens};
use crate::forms::FilingStatus;
use crate::profile::{TaxpayerProfile, inbox_emails_match};

/// Which profile to authenticate as, and whether any sibling still has a
/// Submitted draft the poller should keep waiting on.
#[derive(Debug, Clone)]
pub struct EmailPollPlan {
    pub auth_profile: Option<TaxpayerProfile>,
    pub still_pending: bool,
    pub matching_tins: Vec<String>,
}

/// Pick IMAP/OAuth credentials for a shared inbox.
///
/// Prefer a profile that actually has a refresh token. When several do, the
/// later `profiles.id` wins (the one most recently created, and after reconnect
/// fan-out they all hold the same grant anyway). Never take list order blindly.
pub fn select_imap_auth_profile(
    profiles: &[TaxpayerProfile],
    inbox_email: &str,
) -> Option<TaxpayerProfile> {
    let matching: Vec<&TaxpayerProfile> = profiles
        .iter()
        .filter(|profile| inbox_emails_match(profile.inbox_email(), inbox_email))
        .collect();
    if matching.is_empty() {
        return None;
    }

    let mut with_refresh: Vec<&TaxpayerProfile> = matching
        .iter()
        .copied()
        .filter(|profile| profile.has_usable_oauth_refresh())
        .collect();
    if !with_refresh.is_empty() {
        with_refresh.sort_by(|left, right| right.id.cmp(&left.id));
        return Some(with_refresh[0].clone());
    }

    Some(matching[0].clone())
}

/// Overlay the inbox-keyed grant onto the best matching profile.
///
/// The table is the source of truth after reconnect. Profile copies exist so
/// Email Settings still shows Connected on every sibling.
pub fn resolve_inbox_fetch_profile(
    db: &Database,
    inbox_email: &str,
) -> Result<Option<TaxpayerProfile>, crate::db::DbError> {
    let profiles = db.list_profiles()?;
    resolve_inbox_fetch_profile_from(db, inbox_email, &profiles)
}

fn resolve_inbox_fetch_profile_from(
    db: &Database,
    inbox_email: &str,
    profiles: &[TaxpayerProfile],
) -> Result<Option<TaxpayerProfile>, crate::db::DbError> {
    let selected = select_imap_auth_profile(profiles, inbox_email);
    let table = db
        .inbox_oauth_tokens(inbox_email)?
        .filter(InboxOAuthTokens::has_usable_refresh);

    let Some(mut profile) = selected else {
        return Ok(None);
    };

    if let Some(tokens) = table {
        overlay_inbox_oauth(&mut profile, &tokens);
    }

    Ok(Some(profile))
}

fn overlay_inbox_oauth(profile: &mut TaxpayerProfile, tokens: &InboxOAuthTokens) {
    profile.email_auth_method = crate::profile::EmailAuthMethod::GoogleOAuth;
    profile.oauth_access_token = Some(tokens.access_token.clone());
    profile.oauth_refresh_token = Some(tokens.refresh_token.clone());
    if !tokens.imap_user.trim().is_empty() {
        profile.imap_email = Some(tokens.imap_user.clone());
    }
}

/// Decide whether the cron job should keep running, and which credentials
/// to use. Confirmation matching itself is TIN/form/period from the receipt,
/// so Jane is confirmed even when Juan owns the grant.
pub fn plan_email_poll_for_address(
    db: &Database,
    email_address: &str,
    current_year: u16,
) -> EmailPollPlan {
    let profiles = db.list_profiles().unwrap_or_default();
    let mut matching_tins = Vec::new();
    let mut still_pending = false;
    let mut matching_profiles = Vec::new();

    for profile in profiles {
        if !inbox_emails_match(profile.inbox_email(), email_address) {
            continue;
        }
        matching_tins.push(profile.tin.full());
        if let Ok(summaries) = db.list_draft_summaries(&profile.tin.full(), current_year)
            && summaries
                .iter()
                .any(|summary| summary.status == FilingStatus::Submitted)
        {
            still_pending = true;
        }
        matching_profiles.push(profile);
    }

    let auth_profile = resolve_inbox_fetch_profile_from(db, email_address, &matching_profiles)
        .ok()
        .flatten();

    EmailPollPlan {
        auth_profile,
        still_pending,
        matching_tins,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, ReceiptConfirmationOutcome, alert_kinds};
    use crate::filing_queue::mandatory_lab_1601c_draft;
    use crate::forms::form_1601c::Form1601CDraft;
    use crate::profile::{EmailAuthMethod, TaxpayerProfile};
    use crate::receipt::BirReceiptConfirmation;

    const SHARED_INBOX: &str = "codeitlikemiley@gmail.com";

    fn profile(seg1: &str, name: &str) -> TaxpayerProfile {
        let mut profile: TaxpayerProfile = serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": name,
            "tin": { "segment1": seg1, "segment2": "000", "segment3": "000", "branch": "00000" },
            "rdo_code": "018",
            "line_of_business": "Retail",
            "registered_address": "Manila",
            "zip_code": "1000",
            "phone": "09170000000",
            "email": SHARED_INBOX,
            "default_form_type": "1601Cv2018",
            "taxpayer_type": "Individual",
            "business_start_date": "2020-01-01"
        }))
        .expect("shared-inbox fixture must deserialize");
        profile.email_auth_method = EmailAuthMethod::GoogleOAuth;
        profile.email_tracking_enabled = true;
        profile.imap_email = Some(SHARED_INBOX.to_string());
        profile
    }

    fn save(db: &Database, profile: TaxpayerProfile) -> TaxpayerProfile {
        db.save_profile(profile).expect("profile saves")
    }

    #[test]
    fn poller_uses_second_profile_when_first_refresh_is_empty() {
        let first = {
            let mut profile = profile("111", "Alejandro");
            profile.oauth_refresh_token = None;
            profile.oauth_access_token = Some("access-dead".to_string());
            profile
        };
        let second = {
            let mut profile = profile("222", "Jane");
            profile.oauth_access_token = Some("access-good".to_string());
            profile.oauth_refresh_token = Some("refresh-good".to_string());
            profile
        };

        let selected = select_imap_auth_profile(&[first, second], SHARED_INBOX)
            .expect("a sibling with a refresh token must win");
        assert_eq!(selected.full_name, "Jane");
        assert_eq!(
            selected.oauth_refresh_token.as_deref(),
            Some("refresh-good")
        );
    }

    #[test]
    fn poller_prefers_later_profile_id_when_both_have_refresh_tokens() {
        let mut first = profile("111", "Alejandro");
        first.id = Some(1);
        first.oauth_refresh_token = Some("refresh-stale".to_string());
        let mut second = profile("222", "Jane");
        second.id = Some(4);
        second.oauth_refresh_token = Some("refresh-good".to_string());

        let selected = select_imap_auth_profile(&[first, second], SHARED_INBOX)
            .expect("later reconnect proxy is higher id");
        assert_eq!(selected.full_name, "Jane");
    }

    #[test]
    fn reconnect_overwrites_tokens_on_every_sibling_and_the_inbox_row() {
        let db = Database::open_in_memory_for_tests().expect("in-memory db");
        let alejandro = save(&db, {
            let mut profile = profile("111", "Alejandro");
            profile.oauth_refresh_token = Some("refresh-dead".to_string());
            profile.oauth_access_token = Some("access-dead".to_string());
            profile
        });
        let jane = save(&db, profile("222", "Jane"));
        assert!(alejandro.id.unwrap() < jane.id.unwrap());

        db.record_alert(
            Some(&alejandro.tin.full()),
            alert_kinds::GOOGLE_OAUTH_REFRESH_FAILED,
            crate::db::AlertSeverity::Error,
            "Email confirmation checking has stopped",
            "invalid_grant",
            crate::db::AlertAction::ReconnectGoogleAccount,
        )
        .unwrap();

        db.persist_google_oauth_for_inbox(
            SHARED_INBOX,
            "access-new",
            "refresh-new",
            Some(&jane.tin.full()),
        )
        .expect("reconnect persists");

        let table = db
            .inbox_oauth_tokens(SHARED_INBOX)
            .expect("table read")
            .expect("inbox row exists");
        assert_eq!(table.refresh_token, "refresh-new");
        assert_eq!(table.imap_user, SHARED_INBOX);

        for tin in [alejandro.tin.full(), jane.tin.full()] {
            let stored = db.get_profile(&tin).unwrap().unwrap();
            assert_eq!(
                stored.oauth_refresh_token.as_deref(),
                Some("refresh-new"),
                "sibling {} must receive the new grant",
                stored.full_name
            );
        }

        let plan = plan_email_poll_for_address(&db, SHARED_INBOX, 2026);
        let auth = plan.auth_profile.expect("poller has credentials");
        assert_eq!(
            auth.oauth_refresh_token.as_deref(),
            Some("refresh-new"),
            "poller must not keep Alejandro's dead refresh"
        );
        assert!(
            db.list_active_alerts(Some(&alejandro.tin.full()))
                .unwrap()
                .iter()
                .all(|alert| alert.kind != alert_kinds::GOOGLE_OAUTH_REFRESH_FAILED),
            "reconnect must clear GOOGLE_OAUTH_REFRESH_FAILED for every TIN on this inbox"
        );
    }

    #[test]
    fn empty_refresh_is_rejected_and_does_not_leave_connected_dead_tokens() {
        let db = Database::open_in_memory_for_tests().expect("in-memory db");
        save(&db, {
            let mut profile = profile("111", "Alejandro");
            profile.oauth_refresh_token = Some("refresh-dead".to_string());
            profile
        });

        let err = db
            .persist_google_oauth_for_inbox(SHARED_INBOX, "access-new", "   ", None)
            .expect_err("empty refresh must fail closed");
        assert!(format!("{err}").contains("refresh token"), "{err}");

        let stored = db.get_profile("11100000000000").unwrap().unwrap();
        assert_eq!(
            stored.oauth_refresh_token.as_deref(),
            Some("refresh-dead"),
            "a failed reconnect must not clear or replace the stored refresh"
        );
        assert!(db.inbox_oauth_tokens(SHARED_INBOX).unwrap().is_none());
    }

    #[test]
    fn disconnect_clears_siblings_and_the_inbox_row() {
        let db = Database::open_in_memory_for_tests().expect("in-memory db");
        save(&db, {
            let mut profile = profile("111", "Alejandro");
            profile.oauth_refresh_token = Some("refresh-dead".to_string());
            profile
        });
        save(&db, {
            let mut profile = profile("222", "Jane");
            profile.oauth_refresh_token = Some("refresh-jane".to_string());
            profile
        });
        db.persist_google_oauth_for_inbox(SHARED_INBOX, "access-new", "refresh-new", None)
            .unwrap();

        db.record_alert(
            Some("11100000000000"),
            alert_kinds::GOOGLE_OAUTH_REFRESH_FAILED,
            crate::db::AlertSeverity::Error,
            "Email confirmation checking has stopped",
            "invalid_grant",
            crate::db::AlertAction::ReconnectGoogleAccount,
        )
        .unwrap();

        db.disconnect_google_oauth_for_inbox(SHARED_INBOX)
            .expect("disconnect");

        assert!(db.inbox_oauth_tokens(SHARED_INBOX).unwrap().is_none());
        for tin in ["11100000000000", "22200000000000"] {
            let stored = db.get_profile(tin).unwrap().unwrap();
            assert!(
                !stored.has_usable_oauth_refresh(),
                "{} still has a refresh token",
                stored.full_name
            );
        }
        assert!(
            db.list_active_alerts(Some("11100000000000"))
                .unwrap()
                .iter()
                .all(|alert| alert.kind != alert_kinds::GOOGLE_OAUTH_REFRESH_FAILED)
        );
    }

    #[test]
    fn table_tokens_win_over_first_profile_dead_refresh() {
        let db = Database::open_in_memory_for_tests().expect("in-memory db");
        save(&db, {
            let mut profile = profile("111", "Alejandro");
            profile.oauth_refresh_token = Some("refresh-dead".to_string());
            profile.oauth_access_token = Some("access-dead".to_string());
            profile
        });
        save(&db, profile("222", "Jane"));
        db.persist_google_oauth_for_inbox(
            "CodeItLikeMiley@gmail.com",
            "access-new",
            "refresh-new",
            None,
        )
        .unwrap();

        let auth = resolve_inbox_fetch_profile(&db, SHARED_INBOX)
            .unwrap()
            .expect("inbox grant");
        assert_eq!(auth.oauth_refresh_token.as_deref(), Some("refresh-new"));
        assert!(inbox_emails_match(auth.inbox_email(), SHARED_INBOX));
    }

    fn submit_lab_1601c(db: &Database, mut queued: Form1601CDraft) -> Form1601CDraft {
        queued.transition_to_queued().unwrap();
        db.save_queued_1601c_draft(&queued).unwrap();
        let fingerprint = queued.queued_submission_fingerprint.clone();
        let retry = queued.next_retry_at.clone();
        let claim = db
            .claim_queued_1601c_submission(
                &queued.tin,
                queued.taxable_year,
                queued.month,
                &fingerprint,
                &retry,
                0,
            )
            .unwrap();
        let crate::db::Claim1601CSubmissionResult::Claimed { mut draft, token } = claim else {
            panic!("expected claim");
        };
        let filename = queued.default_submission_filename();
        draft.transition_to_submitted(filename);
        db.finish_claimed_1601c_submission(&draft, &token).unwrap();
        db.get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap()
    }

    #[test]
    fn receipt_confirms_jane_when_juan_owns_the_inbox_grant() {
        let db = Database::open_in_memory_for_tests().expect("in-memory db");
        let mut juan = profile("000", "Juan");
        juan.oauth_refresh_token = Some("refresh-juan".to_string());
        juan.oauth_access_token = Some("access-juan".to_string());
        save(&db, juan);

        let mut jane = crate::filing_queue::mandatory_lab_1601c_profile();
        jane.tin.branch = "00001".to_string();
        jane.full_name = "Jane".to_string();
        jane.email = SHARED_INBOX.to_string();
        jane.imap_email = Some(SHARED_INBOX.to_string());
        jane.email_auth_method = EmailAuthMethod::GoogleOAuth;
        jane.email_tracking_enabled = true;
        jane.oauth_refresh_token = None;
        save(&db, jane);

        let mut queued = mandatory_lab_1601c_draft();
        queued.tin = "00000000000001".to_string();
        let submitted = submit_lab_1601c(&db, queued);
        assert_eq!(submitted.status, FilingStatus::Submitted);

        let plan = plan_email_poll_for_address(&db, SHARED_INBOX, 2026);
        let auth = plan.auth_profile.expect("Juan's grant");
        assert_eq!(auth.full_name, "Juan");
        assert_eq!(auth.oauth_refresh_token.as_deref(), Some("refresh-juan"));
        assert!(
            plan.still_pending,
            "Jane's Submitted 1601-C must keep the inbox poller running"
        );
        assert!(
            plan.matching_tins.iter().any(|tin| tin == "00000000000001"),
            "Jane must stay in the inbox set so her Submitted draft is polled"
        );

        let confirmation = BirReceiptConfirmation {
            filename: "00000000000001-1601Cv2018-092026.xml".to_string(),
            date_received: chrono::NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
            time_received: chrono::NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
            source_from: Some("ebirforms-noreply@bir.gov.ph".to_string()),
            raw_text: "test".to_string(),
            raw_html: None,
        };
        let (saved, _) = db.save_submission_receipt(&confirmation).unwrap();
        assert_eq!(
            db.confirm_submission_from_receipt(&saved).unwrap(),
            ReceiptConfirmationOutcome::Confirmed
        );
        let confirmed = db
            .get_1601c_draft("00000000000001", submitted.taxable_year, submitted.month)
            .unwrap()
            .unwrap();
        assert_eq!(confirmed.status, FilingStatus::Confirmed);
    }

    #[test]
    fn case_insensitive_inbox_match_still_selects_credentials() {
        let mut alejandro = profile("111", "Alejandro");
        alejandro.imap_email = Some("CodeItLikeMiley@gmail.com".to_string());
        alejandro.oauth_refresh_token = None;
        let mut jane = profile("222", "Jane");
        jane.imap_email = Some(SHARED_INBOX.to_string());
        jane.oauth_refresh_token = Some("refresh-good".to_string());

        let selected =
            select_imap_auth_profile(&[alejandro, jane], "CODEITLIKEMILEY@GMAIL.COM").unwrap();
        assert_eq!(selected.full_name, "Jane");
    }
}
