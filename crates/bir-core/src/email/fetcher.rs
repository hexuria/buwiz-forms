//! Shared IMAP fetch + parse pipeline.
//!
//! Both App Password and Google OAuth2 authenticate, then hand off to this
//! module for the actual search → fetch → parse → confirm workflow.

use crate::db::{Database, SubmissionReceipt};
use crate::profile::{EmailAuthMethod, TaxpayerProfile, inbox_emails_match};
use crate::receipt::parse_bir_receipt_email;
use chrono::Datelike;

use super::auth_oauth::GoogleOAuthAuth;
use super::auth_password::AppPasswordAuth;
use super::inbox::{plan_email_poll_for_address, resolve_inbox_fetch_profile};

/// Trait that both auth backends implement.
pub trait ImapAuthenticator {
    fn authenticate(
        &self,
        client: imap::Client<native_tls::TlsStream<std::net::TcpStream>>,
    ) -> Result<
        (
            imap::Session<native_tls::TlsStream<std::net::TcpStream>>,
            Option<String>,
        ),
        anyhow::Error,
    >;

    /// The IMAP server hostname to connect to.
    fn host(&self) -> &str;
}

/// Fetch and process BIR confirmation emails for the given profile.
///
/// Automatically selects the correct auth strategy based on `email_auth_method`.
/// When several profiles share the inbox, Google OAuth credentials come from
/// the inbox-keyed grant (or a sibling with a refresh token), not list order.
pub fn fetch_and_process_emails(
    profile: &TaxpayerProfile,
    db: std::sync::Arc<std::sync::Mutex<Database>>,
) -> Result<Vec<SubmissionReceipt>, anyhow::Error> {
    if !profile.is_email_tracking_active() {
        return Ok(vec![]);
    }

    let fetch_profile = overlay_shared_inbox_credentials(profile, &db);
    fetch_with_resolved_profile(&fetch_profile, db)
}

fn overlay_shared_inbox_credentials(
    profile: &TaxpayerProfile,
    db: &std::sync::Arc<std::sync::Mutex<Database>>,
) -> TaxpayerProfile {
    let Ok(db_guard) = db.lock() else {
        return profile.clone();
    };
    match resolve_inbox_fetch_profile(&db_guard, profile.inbox_email()) {
        Ok(Some(resolved)) => resolved,
        _ => profile.clone(),
    }
}

fn fetch_with_resolved_profile(
    profile: &TaxpayerProfile,
    db: std::sync::Arc<std::sync::Mutex<Database>>,
) -> Result<Vec<SubmissionReceipt>, anyhow::Error> {
    let email = profile.inbox_email();

    // Build the right authenticator
    let (authenticator, host): (Box<dyn ImapAuthenticator>, String) = match profile
        .email_auth_method
    {
        EmailAuthMethod::AppPassword => {
            let host = profile
                .imap_host
                .as_deref()
                .unwrap_or("imap.gmail.com")
                .to_string();
            let auth =
                AppPasswordAuth::new(email, profile.imap_app_password.as_deref().unwrap_or(""))?;
            (Box::new(auth), host)
        }
        EmailAuthMethod::GoogleOAuth => {
            let access = profile.oauth_access_token.as_deref().unwrap_or("");
            let refresh = profile.oauth_refresh_token.as_deref().unwrap_or("");
            let auth = GoogleOAuthAuth::new(email, access, refresh)?;
            let host = "imap.gmail.com".to_string();
            (Box::new(auth), host)
        }
    };

    fetch_with_auth(authenticator.as_ref(), &host, db, profile)
}

/// Test that a connection can be established and authenticated.
/// Returns `Ok(())` on success or an error describing the failure.
pub fn test_connection(profile: &TaxpayerProfile) -> Result<Option<String>, anyhow::Error> {
    let email = profile.inbox_email();

    let (authenticator, host): (Box<dyn ImapAuthenticator>, String) = match profile
        .email_auth_method
    {
        EmailAuthMethod::AppPassword => {
            let host = profile
                .imap_host
                .as_deref()
                .unwrap_or("imap.gmail.com")
                .to_string();
            let auth =
                AppPasswordAuth::new(email, profile.imap_app_password.as_deref().unwrap_or(""))?;
            (Box::new(auth), host)
        }
        EmailAuthMethod::GoogleOAuth => {
            let access = profile.oauth_access_token.as_deref().unwrap_or("");
            let refresh = profile.oauth_refresh_token.as_deref().unwrap_or("");
            let auth = GoogleOAuthAuth::new(email, access, refresh)?;
            let host = "imap.gmail.com".to_string();
            (Box::new(auth), host)
        }
    };

    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap::connect((&*host, 993_u16), &host, &tls)?;
    let (mut session, new_access_token) = authenticator.authenticate(client)?;
    session.select("INBOX")?;
    session.logout()?;
    Ok(new_access_token)
}

// ── Private ──────────────────────────────────────────────────────────────────

fn fetch_with_auth(
    auth: &dyn ImapAuthenticator,
    host: &str,
    db: std::sync::Arc<std::sync::Mutex<Database>>,
    profile: &TaxpayerProfile,
) -> Result<Vec<SubmissionReceipt>, anyhow::Error> {
    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap::connect((host, 993_u16), host, &tls)?;
    let (mut session, new_access_token) = auth.authenticate(client)?;

    // Save the new access token if it was refreshed. Fan-out so siblings and
    // the inbox-keyed row do not keep a stale access token beside a live refresh.
    if let Some(token) = new_access_token
        && let Ok(db_guard) = db.lock()
    {
        let _ = db_guard.update_inbox_oauth_access_token(profile.inbox_email(), &token);
    }

    session.select("INBOX")?;

    // Search ALL BIR confirmation emails from the last 30 days.
    //
    // IMPORTANT: We intentionally do NOT use `UNSEEN` here. If the user reads
    // a confirmation email on their phone or in Gmail web before our background
    // poller runs, the `UNSEEN` filter would silently skip it, causing the
    // draft to stay stuck in "Submitted" forever. Instead, we search ALL
    // matching emails and rely on the `submission_receipts` table's
    // `UNIQUE(filename)` constraint for deduplication — processing the same
    // email twice is harmless (ON CONFLICT DO UPDATE is a no-op for identical data).
    let since_date = {
        let now = chrono::Utc::now().naive_utc().date();
        let since = now - chrono::Duration::days(30);
        since.format("%d-%b-%Y").to_string()
    };
    let search_query = format!("FROM \"ebirforms-noreply@bir.gov.ph\" SINCE {}", since_date);
    let seqs = session.search(&search_query)?;

    let mut processed = Vec::new();

    for seq in seqs {
        let messages = session.fetch(seq.to_string(), "RFC822")?;
        for msg in messages.iter() {
            if let Some(body) = msg.body()
                && let Some(parsed_mail) = mail_parser::MessageParser::default().parse(body)
            {
                let mut text_content = parsed_mail
                    .body_text(0)
                    .map(|s| s.into_owned())
                    .unwrap_or_default();
                let html_content = parsed_mail.body_html(0).map(|s| s.into_owned());

                // Sanitize HTML
                let safe_html =
                    html_content.map(|html| ammonia::Builder::default().clean(&html).to_string());

                // If text_content is empty but we have HTML, use html2text to render it
                if text_content.is_empty()
                    && let Some(html) = &safe_html
                {
                    text_content = html2text::from_read(html.as_bytes(), 80).unwrap_or_default();
                }

                match parse_bir_receipt_email(&text_content, safe_html) {
                    Ok(receipt) => {
                        let mut pending_notice: Option<(String, String)> = None;
                        if let Ok(db_guard) = db.lock()
                            && let Ok((submission_receipt, _is_new)) =
                                db_guard.save_submission_receipt(&receipt)
                        {
                            // Always attempt confirm. A previous poll may have
                            // saved the receipt while Submitted→Confirmed was
                            // ignored (clock skew, 1601-C matcher gap). UNIQUE
                            // filename makes a second save a no-op; confirm
                            // of an already-Confirmed row is Ignored.
                            let confirmed = match db_guard
                                .confirm_submission_from_receipt(&submission_receipt)
                            {
                                Ok(crate::db::ReceiptConfirmationOutcome::Confirmed) => true,
                                Ok(crate::db::ReceiptConfirmationOutcome::Ignored) => false,
                                Err(error) => {
                                    tracing::warn!(
                                        "Receipt {} was saved but could not confirm a submission: {}",
                                        submission_receipt.filename,
                                        error
                                    );
                                    false
                                }
                            };
                            if confirmed
                                && let Some((tin, form_type, period)) =
                                    crate::receipt::split_bir_filename(&submission_receipt.filename)
                            {
                                let form_code =
                                    crate::background_cron::form_code_from_form_type(&form_type)
                                        .to_string();
                                let period_label =
                                    if crate::filing_queue::is_audited_1601c_receipt_form_type(
                                        &form_type,
                                    ) {
                                        crate::filing_queue::parse_1601c_period(&period).map(
                                            |(year, month)| {
                                                crate::background_cron::monthly_period_label(
                                                    year, month,
                                                )
                                            },
                                        )
                                    } else {
                                        crate::db::parse_2551q_period(&period).map(
                                            |(year, quarter)| {
                                                crate::background_cron::quarterly_period_label(
                                                    year, quarter,
                                                )
                                            },
                                        )
                                    };
                                if let Some(period_label) = period_label {
                                    let taxpayer_name = db_guard
                                        .get_profile(&tin)
                                        .ok()
                                        .flatten()
                                        .map(|profile| profile.full_name.clone())
                                        .unwrap_or_else(|| tin.clone());
                                    let bir_filename = format!("{tin}-{form_type}-{period}.xml");
                                    let (title, body) = crate::background_cron::confirmation_notice(
                                        &taxpayer_name,
                                        &crate::naming::Tin::dashed_display(&tin),
                                        &form_code,
                                        &period_label,
                                        &bir_filename,
                                        &crate::background_cron::display_received_at(
                                            &submission_receipt.received_date,
                                            &submission_receipt.received_time,
                                        ),
                                        profile.inbox_email(),
                                    );
                                    crate::background_cron::record_confirmation_alert(
                                        &db_guard,
                                        &tin,
                                        &form_code,
                                        &period_label,
                                        &title,
                                        &body,
                                        &taxpayer_name,
                                    );
                                    // Sent after the lock below: the banner spawns a process.
                                    pending_notice = Some((title, body));
                                }
                            }
                            processed.push(submission_receipt);
                        }
                        if let Some((title, body)) = pending_notice {
                            crate::notification::send_notification(&title, &body);
                        }
                    }
                    Err(e) => {
                        // If it's truly an email from BIR but we failed to parse it, log it.
                        if text_content.contains("This confirms receipt of your submission") {
                            tracing::error!(
                                "Failed to parse BIR receipt email. Error: {:?}\nRaw Body snippet: {:.200}",
                                e,
                                text_content
                            );
                        }
                    }
                }
            }
        }
    }

    session.logout()?;
    Ok(processed)
}

/// Fetch emails for a specific email address, across all profiles.
/// Returns (poll_success, still_pending_forms, error_message).
pub fn fetch_and_process_emails_for_address(
    email_address: &str,
    db: std::sync::Arc<std::sync::Mutex<Database>>,
) -> (bool, bool, Option<String>) {
    let (profile, still_pending) = {
        let db_guard = match db.lock() {
            Ok(g) => g,
            Err(e) => return (false, false, Some(format!("DB lock failed: {}", e))),
        };
        let current_year = chrono::Utc::now().naive_utc().date().year() as u16;
        let plan = plan_email_poll_for_address(&db_guard, email_address, current_year);
        (plan.auth_profile, plan.still_pending)
    };

    if !still_pending {
        return (true, false, None);
    }

    if let Some(mut profile) = profile {
        // Sibling Submitted drafts (Jane) must still be polled even when the
        // grant lives on Juan and Juan's tracking toggle is off.
        profile.email_tracking_enabled = true;
        match fetch_and_process_emails(&profile, db.clone()) {
            Ok(_) => {
                let db_guard = match db.lock() {
                    Ok(g) => g,
                    Err(_) => return (false, true, Some("DB lock failed after fetch".to_string())),
                };
                let mut remaining_pending = false;
                let current_year = chrono::Utc::now().naive_utc().date().year() as u16;
                let profiles = db_guard.list_profiles().unwrap_or_default();
                for p in profiles {
                    if inbox_emails_match(p.inbox_email(), email_address)
                        && let Ok(summaries) =
                            db_guard.list_draft_summaries(&p.tin.full(), current_year)
                        && summaries
                            .iter()
                            .any(|s| s.status == crate::forms::FilingStatus::Submitted)
                    {
                        remaining_pending = true;
                    }
                }
                // The connection works, so clear any standing alert for every
                // TIN that shares this inbox. Done here rather than only when
                // the user clicks Reconnect, so a token that starts working
                // again for any reason stops nagging.
                let _ = db_guard.resolve_google_oauth_alerts_for_inbox(email_address);
                (true, remaining_pending, None)
            }
            Err(e) => {
                let err_msg = format!("{}", e);
                tracing::warn!("Email polling failed for {}: {}", email_address, err_msg);

                const TITLE: &str = "Email confirmation checking has stopped";
                // A missing refresh is "not connected", not a failed refresh.
                // Reconnect nags belong to a grant Google actually rejected.
                if profile.has_usable_oauth_refresh()
                    && let Ok(db_guard) = db.lock()
                    && let Ok(outcome) = db_guard.record_alert(
                        Some(&profile.tin.full()),
                        crate::db::alert_kinds::GOOGLE_OAUTH_REFRESH_FAILED,
                        crate::db::AlertSeverity::Error,
                        TITLE,
                        &err_msg,
                        crate::db::AlertAction::ReconnectGoogleAccount,
                    )
                {
                    crate::notification::notify_alert_if_newly_active(outcome, TITLE, &err_msg);
                }
                (false, still_pending, Some(err_msg))
            }
        }
    } else {
        (
            false,
            false,
            Some(format!(
                "No profile found matching email: {}",
                email_address
            )),
        )
    }
}
